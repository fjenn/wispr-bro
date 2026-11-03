// SPDX-License-Identifier: GPL-3.0-or-later
//! Notifications (S-18, DEC-21): a local inbox plus a few macOS
//! notifications.
//!
//! - **Inbox** ([`inbox`]): every [`Notice`] becomes a row in `fork.db` ›
//!   `notifications`, unless its toggle group is off ([`policy::inbox_allowed`]:
//!   Suggestions, Announcements, Milestones filter at creation). The Hub's bell
//!   lists them; [`ForkNotificationsChanged`] keeps the badge and the panel live
//!   and carries the new entry so the Hub can show it as a toast.
//! - **macOS** ([`macos`], `objc2-user-notifications`, RS-10): only model
//!   ready, update ready, queued takes ready and errors needing action
//!   ([`policy::system_allowed`]), and only while no window of the app has
//!   focus (someone looking at the Hub sees the toast instead). Authorization
//!   is requested by the first such post, never earlier (S-18 exit
//!   criterion): installing the delegate and the categories at Ready does not
//!   prompt.
//! - **Activation** ([`activate`]): an inbox click, an inbox button, a macOS
//!   click or a macOS action button all go through [`policy::resolve`] and run
//!   the same action (open a Hub page, copy the latest dictation, open the
//!   Accessibility pane, reveal a crash file).
//!
//! Producers wired here at Ready: Handy's `model-download-complete`, crash
//! files from earlier runs, a recreated `fork.db`, milestones and tips after
//! each `ForkStatsUpdated`, the current version's feature announcement, and
//! the Esc registry (Esc dismisses toasts; observed, never blocked). The
//! hotkey engine posts shortcut problems ([`shortcut_problem`]); S-07c and
//! S-28 post queued takes and updates through [`post`].

pub mod catalog;
pub mod inbox;
#[cfg(target_os = "macos")]
pub mod macos;
pub mod policy;
#[cfg(test)]
mod tests;

use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use serde::{Deserialize, Serialize};
use specta::Type;
use tauri::{AppHandle, Listener, Manager, Runtime};
use tauri_specta::Event;

pub use catalog::Notice;
pub use inbox::{NotificationFilter, NotificationItem, NotificationsPage};
pub use policy::{NotificationAction, NotificationButton, NotificationKind};

use crate::fork::db::{DbError, ForkDb};
use crate::fork::settings::{ForkSettingsHandle, NotificationSettings};
use crate::fork::ForkRuntime;

/// Emitted after the inbox changed. `posted` is the entry just created (the
/// Hub shows it as a toast); `None` after reads and clears.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, Type, tauri_specta::Event)]
pub struct ForkNotificationsChanged {
    pub unread: u32,
    pub posted: Option<NotificationItem>,
}

/// Emitted when a macOS notification (or its settings link) was activated
/// and the Hub must show something: the Hub navigates to `action`.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, Type, tauri_specta::Event)]
pub struct ForkNotificationOpened {
    pub id: Option<i64>,
    pub action: NotificationAction,
}

/// Emitted for every Esc key-down the hotkey tap sees: toasts close
/// (R1: Esc dismisses notifications; the key is never blocked).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, Type, tauri_specta::Event)]
pub struct ForkToastsDismiss {
    /// Whether the same Esc cancelled a take.
    pub cancelled_take: bool,
}

/// One macOS notification, ready to show.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SystemNotification {
    /// Inbox id; the request identifier is `wb.n.<id>`.
    pub id: i64,
    pub kind: NotificationKind,
    pub title: String,
    pub body: String,
    /// Its category, when it shows buttons.
    pub category: Option<&'static str>,
    /// Every category with its buttons' localized labels. All are set before
    /// each post, so notifications still on screen keep their buttons.
    pub categories: Vec<(&'static str, Vec<(NotificationButton, String)>)>,
}

/// Where macOS notifications go. The real one is [`macos::Center`]; tests use
/// a recording fake. `deliver` is the only path that may ask macOS for
/// permission.
pub trait SystemSink: Send + Sync {
    /// The process can use `UNUserNotificationCenter` (a bundled app).
    fn available(&self) -> bool;
    /// Shows `n`, asking for authorization first if macOS has not been
    /// asked yet.
    fn deliver(&self, n: SystemNotification);
    /// Current authorization, without prompting.
    fn permission(&self) -> NotificationPermission;
}

/// macOS notification authorization as the app sees it.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "snake_case")]
pub enum NotificationPermission {
    /// Not a bundled app (`tauri dev`, tests): inbox only.
    Unavailable,
    /// macOS has not asked yet.
    NotDetermined,
    Denied,
    Authorized,
}

/// A sink that delivers nothing (non-macOS, unbundled runs).
pub struct NoSink;

impl SystemSink for NoSink {
    fn available(&self) -> bool {
        false
    }
    fn deliver(&self, _n: SystemNotification) {}
    fn permission(&self) -> NotificationPermission {
        NotificationPermission::Unavailable
    }
}

type Activity = Box<dyn Fn() -> Option<bool> + Send + Sync>;

/// Notifications state, managed at Ready.
pub struct Notifier {
    sink: Arc<dyn SystemSink>,
    /// Test override of "a window of the app has focus".
    activity: Activity,
    /// Serializes post-and-emit so toasts arrive in insert order.
    post_lock: Mutex<()>,
}

impl Notifier {
    pub fn new(sink: Arc<dyn SystemSink>) -> Self {
        Self::with_activity(sink, Box::new(|| None))
    }

    pub fn with_activity(sink: Arc<dyn SystemSink>, activity: Activity) -> Self {
        Self {
            sink,
            activity,
            post_lock: Mutex::new(()),
        }
    }

    pub fn sink(&self) -> &Arc<dyn SystemSink> {
        &self.sink
    }
}

/// The real sink for this process.
fn platform_sink() -> Arc<dyn SystemSink> {
    #[cfg(target_os = "macos")]
    {
        Arc::new(macos::Center)
    }
    #[cfg(not(target_os = "macos"))]
    {
        Arc::new(NoSink)
    }
}

/// `true` while a visible window of the app has keyboard focus.
fn app_active<R: Runtime>(app: &AppHandle<R>, notifier: &Notifier) -> bool {
    if let Some(forced) = (notifier.activity)() {
        return forced;
    }
    // A hidden window can still report focus (seen with `--start-hidden`).
    app.webview_windows().values().any(|w| {
        w.is_visible().unwrap_or(false)
            && !w.is_minimized().unwrap_or(false)
            && w.is_focused().unwrap_or(false)
    })
}

fn now_ms() -> i64 {
    crate::fork::context::now_ms()
}

fn notification_settings<R: Runtime>(app: &AppHandle<R>) -> NotificationSettings {
    app.try_state::<ForkSettingsHandle>()
        .and_then(|h| h.get(app).ok())
        .map(|s| s.notifications)
        .unwrap_or_default()
}

fn with_db<R: Runtime, T>(
    app: &AppHandle<R>,
    f: impl FnOnce(&mut rusqlite::Connection) -> Result<T, DbError>,
) -> Result<T, DbError> {
    let db = app
        .try_state::<ForkDb>()
        .ok_or_else(|| DbError::Corrupt("fork.db is not managed".into()))?;
    db.with_conn(f)
}

fn emit_changed<R: Runtime>(app: &AppHandle<R>, unread: u32, posted: Option<NotificationItem>) {
    if let Err(e) = (ForkNotificationsChanged { unread, posted }).emit(app) {
        log::debug!("notify: could not emit fork-notifications-changed: {e}");
    }
}

/// Current UI locale for Rust-side strings.
fn locale<R: ForkRuntime>(app: &AppHandle<R>) -> &'static str {
    crate::fork::i18n::current_locale(R::handy_app_language(app).as_deref())
}

/// Resolves `key` with `args` (+ `appName`) for a macOS notification.
pub fn render(locale: &str, key: &str, args: &policy::Args) -> String {
    let app_name = crate::fork::i18n::t(locale, "rust.appName", &[]);
    let mut vars: Vec<(&str, &str)> = args.iter().map(|(k, v)| (k.as_str(), v.as_str())).collect();
    vars.push(("appName", app_name.as_str()));
    // A plural key the catalog picked with English rules (`_one`/`_other`)
    // is resolved again with the locale's CLDR rules, as the inbox does.
    let count = args.get("count").and_then(|c| c.parse::<u64>().ok());
    match (crate::fork::i18n::plural_base(key), count) {
        (Some(base), Some(count)) => crate::fork::i18n::t_plural(locale, base, count, &vars),
        _ => crate::fork::i18n::t(locale, key, &vars),
    }
}

fn system_notification(locale: &str, item: &NotificationItem) -> SystemNotification {
    let categories = policy::CATEGORIES
        .iter()
        .map(|(id, buttons)| {
            let labels = buttons
                .iter()
                .map(|b| (*b, crate::fork::i18n::t(locale, b.label_key(), &[])))
                .collect();
            (*id, labels)
        })
        .collect();
    SystemNotification {
        id: item.id,
        kind: item.kind,
        title: render(locale, &item.title_key, &item.args),
        body: item
            .body_key
            .as_deref()
            .map(|k| render(locale, k, &item.args))
            .unwrap_or_default(),
        category: policy::category_id(item.kind, &item.args),
        categories,
    }
}

/// Posts `notice`: an inbox entry (unless its toggle is off or it is a
/// repeat), the change event, and a macOS notification when policy allows
/// and the app is not in front. Returns the new entry.
pub fn post<R: ForkRuntime>(app: &AppHandle<R>, notice: Notice) -> Option<NotificationItem> {
    let notifier = app.try_state::<Notifier>()?;
    let settings = notification_settings(app);
    if !policy::inbox_allowed(notice.kind, &settings) {
        log::debug!("notify: {} skipped (toggle off)", notice.kind.as_str());
        return None;
    }
    let _order = notifier.post_lock.lock().unwrap_or_else(|e| e.into_inner());
    let now = now_ms();
    let inserted = with_db(app, |c| {
        let Some(id) = inbox::insert(c, &notice, now)? else {
            return Ok(None);
        };
        Ok(Some((inbox::get(c, id)?, inbox::unread_count(c)?)))
    });
    let (item, unread) = match inserted {
        Ok(Some((Some(item), unread))) => (item, unread),
        Ok(_) => return None,
        Err(e) => {
            log::warn!("notify: could not record {}: {e}", notice.kind.as_str());
            return None;
        }
    };
    log::info!("notify: inbox entry {} ({})", item.id, item.kind.as_str());
    emit_changed(app, unread, Some(item.clone()));

    if policy::system_allowed(item.kind, &settings) {
        if app_active(app, &notifier) {
            log::debug!(
                "notify: app in front, entry {} shown in the Hub only",
                item.id
            );
        } else if notifier.sink.available() {
            notifier
                .sink
                .deliver(system_notification(locale(app), &item));
        }
    }
    Some(item)
}

/// Text of the newest dictation (Handy's history; post-processed text when
/// there is one).
fn latest_dictation<R: ForkRuntime>(app: &AppHandle<R>) -> Option<String> {
    let history = R::handy_history(app)?;
    let entries = history.entries().ok()?;
    entries
        .into_iter()
        .map(|e| crate::fork::db::entry_meta::final_text_of(&e).to_string())
        .find(|t| !t.trim().is_empty())
}

/// What [`run_backend_action`] did, for logs and tests.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum BackendOutcome {
    /// Nothing to do on the Rust side (a Hub navigation).
    None,
    Copied {
        chars: usize,
    },
    NothingToCopy,
    OpenedSettings,
    Revealed,
    Failed(String),
}

/// Test hook: the clipboard write (the real one is `NSPasteboard`).
#[cfg(test)]
pub(crate) static TEST_CLIPBOARD: Mutex<Option<String>> = Mutex::new(None);

fn write_clipboard(text: &str) -> Result<(), String> {
    #[cfg(test)]
    {
        *TEST_CLIPBOARD.lock().unwrap_or_else(|e| e.into_inner()) = Some(text.to_string());
        Ok(())
    }
    #[cfg(all(not(test), target_os = "macos"))]
    {
        crate::fork::clipboard::nspb::write_plain_text(text).map(|_| ())
    }
    #[cfg(all(not(test), not(target_os = "macos")))]
    {
        let _ = text;
        Err("clipboard not supported on this platform".into())
    }
}

/// System Settings › Privacy & Security › Accessibility (a local URL
/// scheme; opens System Settings, contacts no host).
pub const ACCESSIBILITY_PANE: &str =
    "x-apple.systempreferences:com.apple.preference.security?Privacy_Accessibility";

/// `name` is a crash file name the crash hook writes (no path parts).
pub fn is_crash_file_name(name: &str) -> bool {
    name.starts_with(crate::fork::diagnostics::crash::CRASH_FILE_PREFIX)
        && name.ends_with(".txt")
        && !name.contains(['/', '\\'])
        && !name.contains("..")
}

/// Runs the Rust part of `action`.
pub fn run_backend_action<R: ForkRuntime>(
    app: &AppHandle<R>,
    action: &NotificationAction,
) -> BackendOutcome {
    use tauri_plugin_opener::OpenerExt;
    match action {
        NotificationAction::OpenPage { .. } | NotificationAction::OpenNotifications => {
            BackendOutcome::None
        }
        NotificationAction::CopyLatest => match latest_dictation(app) {
            Some(text) => match write_clipboard(&text) {
                Ok(()) => BackendOutcome::Copied {
                    chars: text.chars().count(),
                },
                Err(e) => BackendOutcome::Failed(e),
            },
            None => BackendOutcome::NothingToCopy,
        },
        NotificationAction::OpenAccessibilitySettings => {
            if cfg!(test) {
                return BackendOutcome::OpenedSettings;
            }
            match app.opener().open_url(ACCESSIBILITY_PANE, None::<&str>) {
                Ok(()) => BackendOutcome::OpenedSettings,
                Err(e) => BackendOutcome::Failed(e.to_string()),
            }
        }
        NotificationAction::RevealCrashReport { file_name } => {
            if !is_crash_file_name(file_name) {
                return BackendOutcome::Failed(format!("not a crash file: {file_name}"));
            }
            let Some(path) = crash_dir(app).map(|d| d.join(file_name)) else {
                return BackendOutcome::Failed("no crash folder".into());
            };
            if cfg!(test) {
                return if path.is_file() {
                    BackendOutcome::Revealed
                } else {
                    BackendOutcome::Failed("missing".into())
                };
            }
            match app.opener().reveal_item_in_dir(&path) {
                Ok(()) => BackendOutcome::Revealed,
                Err(e) => BackendOutcome::Failed(e.to_string()),
            }
        }
    }
}

/// Where an activation came from.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Origin {
    /// The Hub's panel or toast: the page navigates itself.
    Hub,
    /// A macOS notification: the Hub is shown and told where to go.
    System,
}

/// Activates entry `id` (a click when `button` is `None`): marks it read,
/// resolves the action, runs its Rust part and, from a macOS notification,
/// shows the Hub on the right page. Returns the action (the Hub navigates
/// for [`Origin::Hub`]).
pub fn activate<R: ForkRuntime>(
    app: &AppHandle<R>,
    id: i64,
    button: Option<NotificationButton>,
    origin: Origin,
) -> Result<Option<NotificationAction>, DbError> {
    let now = now_ms();
    let (item, unread) = with_db(app, |c| {
        let item = inbox::get(c, id)?;
        inbox::mark_read(c, &[id], now)?;
        Ok((item, inbox::unread_count(c)?))
    })?;
    emit_changed(app, unread, None);
    let Some(item) = item else {
        // Pruned or cleared since: a macOS click still opens the inbox.
        if origin == Origin::System {
            open_in_hub(app, None, NotificationAction::OpenNotifications);
        }
        return Ok(None);
    };
    let action = policy::resolve(item.kind, &item.args, item.action.as_ref(), button);
    if let Some(action) = &action {
        let outcome = run_backend_action(app, action);
        log::info!(
            "notify: entry {id} ({}) activated via {origin:?}{} → {action:?}: {outcome:?}",
            item.kind.as_str(),
            button
                .map(|b| format!(" button {}", b.id()))
                .unwrap_or_default()
        );
        if origin == Origin::System && action.shows_window() {
            open_in_hub(app, Some(id), action.clone());
        }
    } else if origin == Origin::System {
        // No action of its own: a click shows the entry in the inbox.
        open_in_hub(app, Some(id), NotificationAction::OpenNotifications);
    }
    Ok(action)
}

/// Shows the Hub (rebuilt if S-38 parked it) and tells its page where to
/// go once the page listens.
fn open_in_hub<R: ForkRuntime>(app: &AppHandle<R>, id: Option<i64>, action: NotificationAction) {
    let handle = app.clone();
    crate::fork::hub::park::show_main_then(app, move || {
        if let Err(e) = (ForkNotificationOpened { id, action }).emit(&handle) {
            log::debug!("notify: could not emit fork-notification-opened: {e}");
        }
    });
}

/// A response from the macOS delegate.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SystemResponse {
    /// The notification was clicked (`button == None`) or a button pressed.
    Activated {
        id: i64,
        button: Option<NotificationButton>,
    },
    /// "Notification Settings" for the app in System Settings.
    OpenSettings,
}

/// Routes a macOS response.
pub fn on_system_response<R: ForkRuntime>(app: &AppHandle<R>, response: SystemResponse) {
    match response {
        SystemResponse::Activated { id, button } => {
            if let Err(e) = activate(app, id, button, Origin::System) {
                log::warn!("notify: activation of {id} failed: {e}");
            }
        }
        SystemResponse::OpenSettings => {
            open_in_hub(app, None, NotificationAction::OpenNotifications)
        }
    }
}

// ---------------------------------------------------------------------------
// Producers

/// Databases recreated since launch, with when, waiting for their entry.
static RECOVERED: Mutex<Vec<(PathBuf, i64)>> = Mutex::new(Vec::new());

/// `fork::db::report_db_recovered` hook: post an entry once the new
/// database at `db_path` is open (it is called from inside the open, so it
/// cannot write).
pub fn queue_data_recovered(db_path: &Path) {
    RECOVERED
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .push((db_path.to_path_buf(), now_ms()));
}

fn drain_data_recovered<R: ForkRuntime>(app: &AppHandle<R>) {
    let Some(path) = app.try_state::<ForkDb>().map(|db| db.path().to_path_buf()) else {
        return;
    };
    let mine: Vec<i64> = {
        let mut queue = RECOVERED.lock().unwrap_or_else(|e| e.into_inner());
        let (mine, rest): (Vec<_>, Vec<_>) = queue.drain(..).partition(|(p, _)| *p == path);
        *queue = rest;
        mine.into_iter().map(|(_, at)| at).collect()
    };
    for at in mine {
        post(app, Notice::data_recovered(at));
    }
}

/// Crash files are announced while they are this recent.
pub const CRASH_RECENT_MS: i64 = 7 * 24 * 60 * 60 * 1000;

/// The newest crash file in `dir` written within [`CRASH_RECENT_MS`] of
/// `now_ms` (Unix ms), by name (the names sort by time).
pub fn recent_crash_file(dir: &Path, now_ms: i64) -> Option<String> {
    let mut newest: Option<(i64, String)> = None;
    for entry in std::fs::read_dir(dir).ok()?.flatten() {
        let name = entry.file_name().to_string_lossy().into_owned();
        if !is_crash_file_name(&name) {
            continue;
        }
        let modified = entry
            .metadata()
            .and_then(|m| m.modified())
            .ok()
            .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
            .map(|d| d.as_millis() as i64)
            .unwrap_or(0);
        if now_ms - modified > CRASH_RECENT_MS {
            continue;
        }
        if newest.as_ref().is_none_or(|(_, n)| name > *n) {
            newest = Some((modified, name));
        }
    }
    newest.map(|(_, n)| n)
}

/// The crash folder of this app: its (portable-aware) log dir, where the
/// crash hook writes once the plugin `setup` has run.
fn crash_dir<R: Runtime>(app: &AppHandle<R>) -> Option<PathBuf> {
    crate::fork::app_log_dir(app)
}

fn post_recent_crash<R: ForkRuntime>(app: &AppHandle<R>) {
    let Some(dir) = crash_dir(app) else {
        return;
    };
    // Never scan the owner's DiagnosticReports in a plain Handy build or in
    // unit tests. Product/test identities pick up only their own bundle id.
    if crate::fork::is_product_build() {
        if let Some(reports) = std::env::var_os("HOME")
            .map(PathBuf::from)
            .map(|home| home.join("Library/Logs/DiagnosticReports"))
        {
            if let Err(error) = crate::fork::diagnostics::crash::pickup_ips_reports(
                &reports,
                &dir,
                &crate::fork::compiled_identifier(),
            ) {
                log::debug!("diagnostics: .ips pickup failed: {error}");
            }
        }
    }
    if let Some(name) = recent_crash_file(&dir, now_ms()) {
        post(app, Notice::crash_report(&name));
    }
}

/// Handy's `model-download-complete` payload is the model id; the language
/// detector is not a speech model.
pub fn is_speech_model_id(payload: &str) -> bool {
    let id = payload.trim().trim_matches('"');
    !id.is_empty() && id != crate::managers::model::LANGUAGE_DETECTOR_ID
}

/// Milestones and tips after the stats changed.
pub fn check_progress<R: ForkRuntime>(app: &AppHandle<R>) {
    let today = chrono::Local::now().date_naive();
    let summary = match with_db(app, |c| crate::fork::db::stats::summary(c, today)) {
        Ok(s) => s,
        Err(e) => {
            log::debug!("notify: no stats for milestones: {e}");
            return;
        }
    };
    for notice in progress_notices(app, &summary, today) {
        post(app, notice);
    }
}

/// The milestone and tip notices `summary` earns now, not posted yet.
pub fn progress_notices<R: Runtime>(
    app: &AppHandle<R>,
    summary: &crate::fork::db::stats::StatsSummary,
    today: chrono::NaiveDate,
) -> Vec<Notice> {
    let mut out = Vec::new();
    let has = |key: &str| with_db(app, |c| inbox::has_key(c, key)).unwrap_or(true);
    if let Some(words) = catalog::highest_reached(&catalog::WORD_MILESTONES, summary.total_words) {
        let n = Notice::words_milestone(words);
        if !n.dedupe_key.as_deref().is_some_and(has) {
            out.push(n);
        }
    }
    if let Some(days) =
        catalog::highest_reached(&catalog::STREAK_MILESTONES, summary.current_streak_days)
    {
        let start = today - chrono::Duration::days(i64::from(summary.current_streak_days) - 1);
        let n = Notice::streak_milestone(days, &start.format("%Y-%m-%d").to_string());
        if !n.dedupe_key.as_deref().is_some_and(has) {
            out.push(n);
        }
    }
    let last_tip = with_db(app, |c| inbox::last_created(c, NotificationKind::Tip))
        .ok()
        .flatten();
    let posted = |id: &str| has(&format!("tip:{id}"));
    if let Some(tip) = catalog::next_tip(summary.total_dictations, &posted, last_tip, now_ms()) {
        out.push(Notice::tip(tip));
    }
    out
}

/// The feature this version announces, for the Hub's one-time popup.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, Type)]
pub struct FeatureAnnouncement {
    pub id: String,
    /// Popup dismissal id stored in `dismissed_banners`.
    pub dismiss_id: String,
    pub title_key: String,
    pub body_key: String,
    pub action: NotificationAction,
    /// Its inbox entry, marked read when the popup closes.
    pub entry_id: Option<i64>,
}

/// Popup dismissal id of a feature.
pub fn feature_dismiss_id(id: &str) -> String {
    format!("feature.{id}")
}

/// The feature announced by `version`, unless Announcements are off.
pub fn feature_announcement<R: Runtime>(
    app: &AppHandle<R>,
    version: &str,
) -> Option<FeatureAnnouncement> {
    if !notification_settings(app).announcements {
        return None;
    }
    let feature = catalog::feature_for_version(version)?;
    let notice = Notice::announcement(feature);
    let entry_id = notice.dedupe_key.as_deref().and_then(|key| {
        with_db(app, |c| {
            Ok(c.query_row(
                "SELECT id FROM notifications WHERE dedupe_key = ?1",
                [key],
                |r| r.get::<_, i64>(0),
            )
            .ok())
        })
        .ok()
        .flatten()
    });
    Some(FeatureAnnouncement {
        id: feature.id.into(),
        dismiss_id: feature_dismiss_id(feature.id),
        title_key: notice.title_key,
        body_key: notice.body_key.unwrap_or_default(),
        action: feature.action(),
        entry_id,
    })
}

/// Called by the hotkey engine when its tap cannot start (after onboarding
/// only: before it, onboarding asks for the permission itself).
pub fn shortcut_problem<R: ForkRuntime>(app: &AppHandle<R>, permission_missing: bool) {
    let onboarded = R::handy_settings(app).is_some_and(|s| s.0.onboarding_completed);
    if !onboarded {
        log::debug!("notify: shortcut problem before onboarding is done; not posted");
        return;
    }
    post(app, Notice::shortcut_problem(permission_missing));
}

/// Whether this process may ask macOS for Accessibility state.
pub fn accessibility_trusted() -> bool {
    #[cfg(target_os = "macos")]
    {
        #[link(name = "ApplicationServices", kind = "framework")]
        extern "C" {
            fn AXIsProcessTrusted() -> u8;
        }
        // SAFETY: no arguments.
        unsafe { AXIsProcessTrusted() != 0 }
    }
    #[cfg(not(target_os = "macos"))]
    {
        true
    }
}

/// Marks an app whose producers are wired (once per app).
struct Wired;

/// Registers the notifier (unless one is managed already: tests manage one
/// with a fake sink), wires the producers and posts what is pending. Runs
/// from `fork::on_ready`; prompts for nothing.
pub fn on_ready<R: ForkRuntime>(app: &AppHandle<R>) {
    if !app.manage(Wired) {
        return;
    }
    if app.try_state::<Notifier>().is_none() {
        app.manage(Notifier::new(platform_sink()));
    }
    #[cfg(target_os = "macos")]
    if app.state::<Notifier>().sink.available() {
        let handle = app.clone();
        macos::install(move |response| on_system_response(&handle, response));
    }

    let handle = app.clone();
    app.listen("model-download-complete", move |event| {
        // S-07b: only the first model; silent upgrades say nothing.
        if is_speech_model_id(event.payload()) && !crate::fork::models::setup::is_ready() {
            post(&handle, Notice::model_ready());
        }
    });

    let handle = app.clone();
    crate::fork::events::ForkStatsUpdated::listen(app, move |_| {
        let handle = handle.clone();
        // Off the emitting (history) thread.
        std::thread::spawn(move || check_progress(&handle));
    });

    let handle = app.clone();
    crate::fork::shortcut::registry().observe_esc(Arc::new(move |ctx| {
        let event = ForkToastsDismiss {
            cancelled_take: ctx.cancelled_take,
        };
        if let Err(e) = event.emit(&handle) {
            log::debug!("notify: could not emit fork-toasts-dismiss: {e}");
        }
    }));

    // The database opens (and may be recovered) on first use.
    if let Some(db) = app.try_state::<ForkDb>() {
        if let Err(e) = db.open_now() {
            log::warn!("notify: fork.db unavailable: {e}");
        }
    }
    drain_data_recovered(app);
    post_recent_crash(app);
    if let Some(feature) = catalog::feature_for_version(env!("CARGO_PKG_VERSION")) {
        post(app, Notice::announcement(feature));
    }
    let handle = app.clone();
    std::thread::spawn(move || check_progress(&handle));

    let cli_debug = app.try_state::<crate::CliArgs>().is_some_and(|a| a.debug);
    if cfg!(debug_assertions) || cli_debug {
        start_demo(app, std::env::var(DEMO_ENV).ok().as_deref());
    }
}

/// Debug builds or `--debug` only: `WISPRBRO_NOTIFY_DEMO="<delay ms>:<kind>,<kind>…"`
/// posts one sample of each kind, `<delay ms>` apart, through the real path
/// (the Mac test script's unattended S-18 run; the debug panel does the same
/// by hand).
pub const DEMO_ENV: &str = "WISPRBRO_NOTIFY_DEMO";

/// Parses a [`DEMO_ENV`] value.
pub fn parse_demo(value: &str) -> Option<(u64, Vec<NotificationKind>)> {
    let (delay, kinds) = value.split_once(':')?;
    let delay = delay.trim().parse().ok()?;
    let kinds: Option<Vec<_>> = kinds
        .split(',')
        .map(|k| NotificationKind::parse(k.trim()))
        .collect();
    kinds.filter(|k| !k.is_empty()).map(|k| (delay, k))
}

fn start_demo<R: ForkRuntime>(app: &AppHandle<R>, value: Option<&str>) {
    let Some((delay, kinds)) = value.and_then(parse_demo) else {
        return;
    };
    log::info!("notify: demo of {kinds:?}, {delay} ms apart");
    let app = app.clone();
    std::thread::spawn(move || {
        for (i, kind) in kinds.into_iter().enumerate() {
            std::thread::sleep(std::time::Duration::from_millis(delay));
            let _ =
                crate::fork::commands::notifications::post_sample(&app, kind, now_ms() + i as i64);
        }
    });
}
