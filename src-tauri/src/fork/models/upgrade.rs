// SPDX-License-Identifier: GPL-3.0-or-later
//! Silent upgrades (S-07b, B-47, B-58, B-60, B-64, B-206, B-208).
//!
//! Once a speech model is ready, the setup loop (`super::setup`) calls
//! [`step`] at launch and on every kick (languages changed, a slow-speed
//! verdict, a Handy download or selection change, "Choose automatically"):
//! 1. the policy ([`super::policy`]) names the best model for this Mac and
//!    the user's languages;
//! 2. if it is not the selection, its file is downloaded in the background
//!    (the same state machine as the first model, but nothing on the bar),
//!    then selected at an idle moment: no take recording or transcribing,
//!    and none in the last [`QUIET_MS`] ("swap never during a take").
//!    Handy's `switch_active_model` loads it; a load failure keeps the old
//!    model selected;
//! 3. once the new model has loaded, superseded models are deleted
//!    ([`super::cleanup`]).
//!
//! A selection the user makes on the Models page pins it: the policy then
//! leaves the choice alone until "Choose automatically" is turned back on.
//! A retired selection (policy `retired`, Handy's retired ONNX models)
//! migrates through the same path.
//!
//! Hysteresis (Mac wave 1, finding 12: under load the policy alternated
//! between two models and re-downloaded 135-731 MB each cycle):
//! - the measured factor is the Mac's, not the model's: once confirmed slow
//!   ([`super::speed::confirmed_factor`]) it is kept in `model_policy_state`
//!   for [`VERDICT_TTL_MS`], so the lighter model it led to (with no takes of
//!   its own yet) is not swapped straight back; a fast reading never clears
//!   it early;
//! - a model the fork deleted is not downloaded again within
//!   [`REDOWNLOAD_COOLDOWN_MS`] while the selection still hears the user's
//!   languages ([`plan`]);
//! - every round waits for a quiet moment ([`is_quiet`]) before any model
//!   work (probe, download, checksum, swap, delete), and the checksum runs
//!   at background QoS, so model work never delays a take or its paste.

use std::collections::HashSet;
use std::sync::atomic::{AtomicI64, Ordering};
use std::sync::Mutex;

use serde::{Deserialize, Serialize};
use specta::Type;
use tauri::{AppHandle, Manager, Runtime};

use super::cleanup::{self, Origin};
use super::machine::{Chip, Machine};
use super::policy::{self, Decision, Table};

/// No swap within this long after a take ended.
pub const QUIET_MS: i64 = 10_000;
/// A confirmed slow factor stands this long without new evidence.
pub const VERDICT_TTL_MS: i64 = 14 * 24 * 60 * 60 * 1000;
/// A model the fork deleted is not downloaded again for this long.
pub const REDOWNLOAD_COOLDOWN_MS: i64 = 7 * 24 * 60 * 60 * 1000;

/// What the background work is doing (Settings › Advanced › Models).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "snake_case")]
pub enum PolicyPhase {
    /// Before the first decision, or no policy (plain Handy).
    Checking,
    /// The selection is the policy's pick (or pinned).
    Settled,
    Downloading,
    /// Downloaded; waiting for a moment without dictation.
    WaitingToSwap,
    /// The download stopped; tried again at the next launch or change.
    Paused,
}

/// This Mac, as the Models panel shows it.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, Type)]
pub struct MachineSummary {
    pub chip: Chip,
    pub ram_gb: u32,
    pub free_disk_gb: Option<u32>,
}

/// The `fork-model-policy` event and `fork_model_policy_status` reply.
/// Unlike the bar's status it may name models: it is for Settings ›
/// Advanced only.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, Type, tauri_specta::Event)]
pub struct ForkModelPolicy {
    /// The policy picks (false: the user pinned a model).
    pub automatic: bool,
    pub phase: PolicyPhase,
    /// Display name of the selected speech model.
    pub current_model: Option<String>,
    /// Display name of the policy's pick when it is not the selection yet.
    pub next_model: Option<String>,
    /// 0-99 while the next model downloads.
    pub percent: Option<u8>,
    /// Nothing meets the latency budget on this Mac.
    pub slower: bool,
    /// The languages the pick is for.
    pub languages: Vec<String>,
    pub machine: Option<MachineSummary>,
}

impl ForkModelPolicy {
    const fn checking() -> ForkModelPolicy {
        ForkModelPolicy {
            automatic: true,
            phase: PolicyPhase::Checking,
            current_model: None,
            next_model: None,
            percent: None,
            slower: false,
            languages: Vec::new(),
            machine: None,
        }
    }
}

static STATUS: Mutex<ForkModelPolicy> = Mutex::new(ForkModelPolicy::checking());
/// Models that failed to load this session (never picked again until the
/// next launch).
static FAILED: Mutex<Option<HashSet<String>>> = Mutex::new(None);
/// When the last take ended (Unix ms).
static LAST_TAKE_MS: AtomicI64 = AtomicI64::new(0);
/// After a background download gave up: no new attempt before this (Unix
/// ms), unless the languages or "Choose automatically" change.
static PAUSED_UNTIL_MS: AtomicI64 = AtomicI64::new(0);
/// How long a stopped background download rests.
pub const PAUSE_MS: i64 = 30 * 60 * 1000;

/// The languages or the automatic choice changed: a paused download may
/// start again at once.
pub(super) fn clear_pause() {
    PAUSED_UNTIL_MS.store(0, Ordering::Release);
}

fn lock_status() -> std::sync::MutexGuard<'static, ForkModelPolicy> {
    STATUS.lock().unwrap_or_else(|e| e.into_inner())
}

pub fn status() -> ForkModelPolicy {
    lock_status().clone()
}

pub(super) fn update<R: Runtime>(app: &AppHandle<R>, f: impl FnOnce(&mut ForkModelPolicy)) {
    use tauri_specta::Event;
    let s = {
        let mut s = lock_status();
        let before = s.clone();
        f(&mut s);
        if *s == before {
            return;
        }
        s.clone()
    };
    if let Err(e) = s.emit(app) {
        log::debug!("models: could not emit fork-model-policy: {e}");
    }
}

/// Download progress of the background model.
pub(super) fn on_progress<R: Runtime>(app: &AppHandle<R>, percent: u8) {
    update(app, |s| s.percent = Some(percent));
}

/// A take ended (the swap waits for a quiet moment after it).
pub fn on_take_ended() {
    LAST_TAKE_MS.store(now_ms(), Ordering::Release);
    // S-28: updates wait for a quiet moment after a take too.
    crate::fork::update::idle_gate::note_activity();
}

/// Whether a swap may happen now: nothing records or transcribes and no
/// take ended in the last [`QUIET_MS`].
pub fn is_quiet(busy: bool, last_take_ms: i64, now_ms: i64) -> bool {
    !busy && now_ms.saturating_sub(last_take_ms) >= QUIET_MS
}

pub(super) fn quiet_now(app: &AppHandle) -> bool {
    is_quiet(
        crate::tray::dictation_in_progress(app),
        LAST_TAKE_MS.load(Ordering::Acquire),
        now_ms(),
    )
}

fn now_ms() -> i64 {
    chrono::Utc::now().timestamp_millis()
}

fn failed(id: &str) -> bool {
    FAILED
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .as_ref()
        .is_some_and(|s| s.contains(id))
}

fn mark_failed(id: &str) {
    FAILED
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .get_or_insert_with(HashSet::new)
        .insert(id.to_string());
}

/// The languages the policy picks for: Handy's `spoken_languages` when the
/// user set them, else the macOS preferred languages plus English (DEC-02).
pub fn policy_languages(spoken: &[String], os_preferred: &[String]) -> Vec<String> {
    let chosen = spoken.iter().any(|s| !s.trim().is_empty());
    let mut out = super::starter::user_languages(spoken, os_preferred);
    if !chosen && !out.iter().any(|l| l == "en") {
        out.push("en".into());
    }
    out
}

/// What [`step`] does about the selection.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Plan {
    /// The selection is right (or pinned, or nothing fits): only clean up.
    Stay,
    /// Select `to`, which is on disk.
    Switch { to: String },
    /// Download `to`, then select it.
    Download { to: String },
}

/// The pure part of [`step`]: from the selection and the policy's pick.
/// `cooling` = the pick was deleted within [`REDOWNLOAD_COOLDOWN_MS`] and
/// the selection still serves the user's languages: no new download.
pub fn plan(
    selected: &str,
    pick: Option<&str>,
    pick_on_disk: bool,
    pinned: bool,
    cooling: bool,
) -> Plan {
    match pick {
        _ if pinned => Plan::Stay,
        None => Plan::Stay,
        Some(p) if p == selected || failed(p) => Plan::Stay,
        Some(p) if pick_on_disk => Plan::Switch { to: p.to_string() },
        Some(_) if cooling => Plan::Stay,
        Some(p) => Plan::Download { to: p.to_string() },
    }
}

/// The measured factor the policy uses: a confirmed one slower than the
/// class (returned with `true`: save it), else a saved one younger than
/// [`VERDICT_TTL_MS`], else none (the class factor).
pub fn measured(
    confirmed: Option<f64>,
    class: f64,
    saved: Option<(f64, i64)>,
    now: i64,
) -> (Option<f64>, bool) {
    match confirmed {
        Some(c) if c.is_finite() && c > class => (Some(c), true),
        _ => match saved {
            Some((f, at)) if f.is_finite() && now.saturating_sub(at) <= VERDICT_TTL_MS => {
                (Some(f), false)
            }
            _ => (None, false),
        },
    }
}

fn parse_verdict(s: &str) -> Option<(f64, i64)> {
    let (f, at) = s.split_once('@')?;
    Some((f.parse().ok()?, at.parse().ok()?))
}

/// Whether a selection that is not the one the fork made is the user's
/// choice (pin it) rather than a fallback: the fork's model is still on
/// disk, so the user switched away from it.
pub fn user_picked(selected: &str, fork_selected: Option<&str>, fork_on_disk: bool) -> bool {
    match fork_selected {
        Some(f) => !selected.is_empty() && f != selected && fork_on_disk,
        None => false,
    }
}

/// The Handy ids of policy models whose files are on disk, and the policy
/// ids among them.
fn on_disk(table: &Table, mm: &crate::managers::model::ModelManager) -> Vec<String> {
    table
        .model_ids()
        .filter(|id| {
            table
                .pin(id)
                .and_then(|p| mm.get_model_info(&p.handy_id()))
                .is_some_and(|m| m.is_downloaded)
        })
        .cloned()
        .collect()
}

/// The policy id of a Handy id, if the policy knows it.
fn policy_id_of(table: &Table, handy_id: &str) -> Option<String> {
    table
        .model_ids()
        .find(|id| table.pin(id).is_some_and(|p| p.handy_id() == handy_id))
        .cloned()
}

fn db<R: Runtime>(app: &AppHandle<R>) -> Option<tauri::State<'_, crate::fork::db::ForkDb>> {
    app.try_state::<crate::fork::db::ForkDb>()
}

fn state<R: Runtime>(app: &AppHandle<R>, key: &str) -> Option<String> {
    db(app)?
        .with_conn(|c| cleanup::get_state(c, key))
        .ok()
        .flatten()
}

fn set_state<R: Runtime>(app: &AppHandle<R>, key: &str, value: Option<&str>) {
    if let Some(db) = db(app) {
        if let Err(e) = db.with_conn(|c| cleanup::set_state(c, key, value)) {
            log::warn!("models: could not save {key}: {e}");
        }
    }
}

/// Records a model the fork manages.
pub(super) fn record<R: Runtime>(app: &AppHandle<R>, id: &str, origin: Origin) {
    if let Some(db) = db(app) {
        let _ = db.with_conn(|c| cleanup::add(c, id, origin, now_ms()));
    }
}

/// The selection loaded (a switch or a transcribed take).
pub(super) fn record_loaded<R: Runtime>(app: &AppHandle<R>, id: &str) {
    if let Some(db) = db(app) {
        let _ = db.with_conn(|c| {
            cleanup::add(c, id, Origin::Selection, now_ms())?;
            cleanup::mark_loaded(c, id, now_ms())
        });
    }
}

/// The fork selected `id` (so a later different selection is the user's).
pub(super) fn note_fork_selection<R: Runtime>(app: &AppHandle<R>, id: &str) {
    set_state(app, cleanup::KEY_FORK_SELECTED, Some(id));
}

/// "Choose automatically" on the Models panel.
pub fn set_automatic(app: &AppHandle, on: bool) {
    let selected = crate::settings::get_settings(app).selected_model;
    if on {
        set_state(app, cleanup::KEY_PINNED, None);
        note_fork_selection(app, &selected);
    } else if !selected.is_empty() {
        set_state(app, cleanup::KEY_PINNED, Some(&selected));
    }
    log::info!(
        "models: choose automatically {}",
        if on { "on" } else { "off" }
    );
    update(app, |s| s.automatic = on);
}

/// One warm take's key-up time, for the measured factor. `true` when it
/// completed a set of [`super::speed::SAMPLES`] (worth a re-pick).
pub fn record_speed<R: Runtime>(
    app: &AppHandle<R>,
    model: &str,
    audio_ms: u64,
    keyup_ms: u64,
) -> bool {
    let Some(db) = db(app) else { return false };
    match db.with_conn(|c| super::speed::record(c, model, audio_ms, keyup_ms, now_ms())) {
        Ok(n) => n % super::speed::SAMPLES == 0,
        Err(e) => {
            log::debug!("models: could not record a take's speed: {e}");
            false
        }
    }
}

fn measured_factor<R: Runtime>(
    app: &AppHandle<R>,
    table: &Table,
    chip: &Chip,
    selected: &str,
) -> Option<f64> {
    let now = now_ms();
    let confirmed = policy_id_of(table, selected)
        .and_then(|id| table.reference_p50_ms(&id, chip))
        .and_then(|reference| {
            let rows = db(app)?
                .with_conn(|c| super::speed::recent_timed(c, selected, 2 * super::speed::SAMPLES))
                .ok()?;
            super::speed::confirmed_factor(&rows, reference, now)
        });
    let saved = state(app, cleanup::KEY_MEASURED).and_then(|s| parse_verdict(&s));
    let (factor, save) = measured(confirmed, table.class_factor(chip), saved, now);
    if save {
        if let Some(f) = factor {
            log::info!("models: this Mac measured slower than its class (factor {f:.1})");
            set_state(app, cleanup::KEY_MEASURED, Some(&format!("{f}@{now}")));
        }
    }
    factor
}

fn summary(m: &Machine) -> MachineSummary {
    MachineSummary {
        chip: m.chip,
        ram_gb: m.ram_gb() as u32,
        free_disk_gb: m.free_disk_bytes.map(|b| (b / 1_000_000_000) as u32),
    }
}

fn model_name(mm: &crate::managers::model::ModelManager, id: &str) -> Option<String> {
    mm.get_model_info(id).map(|m| m.name)
}

/// The decision for this moment (machine probe, languages, measured speed).
pub(super) async fn decide(
    app: &AppHandle,
    mm: &crate::managers::model::ModelManager,
    selected: &str,
) -> Option<(Decision, Machine)> {
    let table = policy::TABLE.as_ref()?;
    let dir = super::setup::hub_dir();
    let machine = tauri::async_runtime::spawn_blocking(move || super::machine::probe(&dir))
        .await
        .ok()?;
    let spoken = crate::settings::get_settings(app).spoken_languages;
    let os: Vec<String> = sys_locale::get_locales().collect();
    let languages = policy_languages(&spoken, &os);
    let measured = measured_factor(app, table, &machine.chip, selected);
    let disk = on_disk(table, mm);
    let d = policy::decide(
        table,
        &policy::Inputs {
            languages: &languages,
            machine: &machine,
            cc_by: policy::CC_BY_ALLOWED,
            measured_factor: measured,
            on_disk: &disk,
        },
    )?;
    Some((d, machine))
}

/// The Handy id the policy wants now (the download machine re-checks it
/// when the languages change).
pub(super) async fn target_handy_id(
    app: &AppHandle,
    mm: &crate::managers::model::ModelManager,
) -> Option<String> {
    let selected = crate::settings::get_settings(app).selected_model;
    let (d, _) = decide(app, mm, &selected).await?;
    policy::TABLE.as_ref()?.pin(&d.model).map(|p| p.handy_id())
}

/// One round of the policy (module docs). Runs inside the setup loop.
pub(super) async fn step(
    app: &AppHandle,
    mm: &std::sync::Arc<crate::managers::model::ModelManager>,
    selected: &str,
    rx: &mut tokio::sync::mpsc::UnboundedReceiver<super::setup::Kick>,
) {
    let Some(table) = policy::TABLE.as_ref() else {
        return;
    };
    // Model work never runs while a take records, transcribes or pastes.
    wait_quiet(app).await;
    // Who chose the selection?
    let fork_selected = state(app, cleanup::KEY_FORK_SELECTED);
    let fork_on_disk = fork_selected
        .as_deref()
        .and_then(|f| mm.get_model_info(f))
        .is_some_and(|m| m.is_downloaded);
    let mut pinned = state(app, cleanup::KEY_PINNED);
    if pinned.is_none() && user_picked(selected, fork_selected.as_deref(), fork_on_disk) {
        log::info!(
            "models: the user chose {selected} on the Models page; choosing automatically is off"
        );
        set_state(app, cleanup::KEY_PINNED, Some(selected));
        pinned = Some(selected.to_string());
    }
    if fork_selected.is_none() || (!fork_on_disk && pinned.is_none()) {
        note_fork_selection(app, selected);
    }
    if pinned.as_deref().is_some_and(|p| p != selected) {
        // The pinned model is gone (deleted on the Models page): the
        // selection Handy or the fork fell back to is automatic again.
        set_state(app, cleanup::KEY_PINNED, None);
        pinned = None;
    }
    record(app, selected, Origin::Selection);

    let decision = decide(app, mm, selected).await;
    let pick = decision
        .as_ref()
        .and_then(|(d, _)| table.pin(&d.model))
        .map(|p| p.handy_id());
    let pick_on_disk = pick
        .as_deref()
        .and_then(|p| mm.get_model_info(p))
        .is_some_and(|m| m.is_downloaded);
    let cooling = match (&pick, &decision) {
        (Some(p), Some((d, _))) => {
            let rows = db(app)
                .and_then(|db| db.with_conn(|c| cleanup::rows(c)).ok())
                .unwrap_or_default();
            cleanup::deleted_within(&rows, p, now_ms(), REDOWNLOAD_COOLDOWN_MS)
                && serves(table, selected, &d.languages)
        }
        _ => false,
    };
    let the_plan = plan(
        selected,
        pick.as_deref(),
        pick_on_disk,
        pinned.is_some(),
        cooling,
    );
    if cooling && the_plan == Plan::Stay && pick.as_deref() != Some(selected) {
        log::info!(
            "models: not downloading {} again yet (deleted less than {} days ago)",
            pick.as_deref().unwrap_or_default(),
            REDOWNLOAD_COOLDOWN_MS / 86_400_000
        );
    }
    if let Some((d, m)) = &decision {
        log::info!(
            "models: policy for {:?} on {:?} (factor {:.1}): {} expected {:.0}/{:.0} ms{}; passed over {:?} → {:?}",
            d.languages,
            m.chip,
            d.factor,
            d.model,
            d.expected_ms.0,
            d.expected_ms.1,
            if d.slower { ", slower on this Mac" } else { "" },
            d.skipped,
            the_plan
        );
    } else {
        log::info!("models: the policy has no pick that fits this Mac; keeping {selected}");
    }
    let (slower, languages, machine) = decision
        .as_ref()
        .map(|(d, m)| (d.slower, d.languages.clone(), Some(summary(m))))
        .unwrap_or_default();
    let next_name = match &the_plan {
        Plan::Stay => None,
        Plan::Switch { to } | Plan::Download { to } => model_name(mm, to),
    };
    update(app, |s| {
        s.automatic = pinned.is_none();
        s.current_model = model_name(mm, selected);
        s.next_model = next_name;
        s.slower = slower && pinned.is_none();
        s.languages = languages;
        s.machine = machine;
        s.phase = match &the_plan {
            Plan::Stay => PolicyPhase::Settled,
            Plan::Switch { .. } => PolicyPhase::WaitingToSwap,
            Plan::Download { .. } => PolicyPhase::Downloading,
        };
        if s.phase != PolicyPhase::Downloading {
            s.percent = None;
        }
    });
    if slower && pinned.is_none() && state(app, cleanup::KEY_SLOWER_SAID).is_none() {
        set_state(app, cleanup::KEY_SLOWER_SAID, Some("1"));
        crate::fork::notify::post(
            app,
            crate::fork::notify::catalog::Notice::slower_on_this_mac(),
        );
    }

    let to = match the_plan {
        Plan::Stay => {
            clean_up(app, mm, table, selected);
            return;
        }
        Plan::Switch { to } => to,
        Plan::Download { to } => {
            let Some(pinned_file) = decision.as_ref().and_then(|(d, _)| table.pin(&d.model)) else {
                return;
            };
            if now_ms() < PAUSED_UNTIL_MS.load(Ordering::Acquire) {
                update(app, |s| s.phase = PolicyPhase::Paused);
                return;
            }
            log::info!(
                "models: downloading {} ({} MB) in the background",
                pinned_file.catalog_id,
                pinned_file.size / 1_000_000
            );
            match super::setup::download_background(app, mm, pinned_file, rx).await {
                super::setup::Background::Done => {}
                super::setup::Background::Failed => {
                    PAUSED_UNTIL_MS.store(now_ms() + PAUSE_MS, Ordering::Release);
                    update(app, |s| s.phase = PolicyPhase::Paused);
                    return;
                }
                // A new pick: the next round starts it.
                super::setup::Background::Stopped => return,
            }
            record(app, &to, Origin::Policy);
            update(app, |s| {
                s.phase = PolicyPhase::WaitingToSwap;
                s.percent = None;
            });
            to
        }
    };
    swap(app, mm, table, selected, &to).await;
}

/// Whether the selection hears every one of `languages` (a cooldown never
/// keeps the user on a model that cannot hear a language they speak).
fn serves(table: &Table, selected: &str, languages: &[String]) -> bool {
    policy_id_of(table, selected)
        .and_then(|id| table.pin(&id))
        .is_some_and(|p| languages.iter().all(|l| p.languages.iter().any(|h| h == l)))
}

/// Waits until [`quiet_now`].
async fn wait_quiet(app: &AppHandle) {
    while !quiet_now(app) {
        tokio::time::sleep(std::time::Duration::from_millis(250)).await;
    }
}

/// Selects `to` at a quiet moment; on a load keeps it and cleans up, else
/// keeps the old selection.
async fn swap(
    app: &AppHandle,
    mm: &crate::managers::model::ModelManager,
    table: &Table,
    old: &str,
    to: &str,
) {
    if !super::setup::select_when_quiet(app, to).await {
        log::warn!("models: {to} did not load; keeping {old}");
        mark_failed(to);
        update(app, |s| s.phase = PolicyPhase::Settled);
        return;
    }
    // Handy loads on select unless the unload timeout is "Immediately";
    // load it once now so the old model goes only after the new one works.
    let loaded = match app
        .try_state::<std::sync::Arc<crate::managers::transcription::TranscriptionManager>>()
    {
        Some(tm) if !tm.is_model_loaded() => {
            let (tm, id) = (tm.inner().clone(), to.to_string());
            tauri::async_runtime::spawn_blocking(move || tm.load_model(&id).is_ok())
                .await
                .unwrap_or(false)
        }
        Some(_) => true,
        None => false,
    };
    if !loaded {
        log::warn!("models: {to} selected but did not load; keeping {old} on disk");
        mark_failed(to);
        return;
    }
    record_loaded(app, to);
    if let Some(db) = db(app) {
        let _ = db.with_conn(|c| cleanup::mark_superseded(c, old, now_ms()));
    }
    log::info!("models: swapped {old} → {to}");
    update(app, |s| {
        s.phase = PolicyPhase::Settled;
        s.current_model = model_name(mm, to);
        s.next_model = None;
    });
    clean_up(app, mm, table, to);
}

/// Deletes what [`cleanup::to_delete`] allows, plus Handy's retired ONNX
/// models once a working model is selected.
fn clean_up(
    app: &AppHandle,
    mm: &crate::managers::model::ModelManager,
    table: &Table,
    selected: &str,
) {
    let Some(db) = db(app) else { return };
    let Ok(rows) = db.with_conn(|c| cleanup::rows(c)) else {
        return;
    };
    let retired = |id: &str| {
        table
            .retired()
            .iter()
            .filter_map(|r| super::starter::pinned(r, None))
            .any(|p| p.handy_id() == id)
    };
    for id in cleanup::to_delete(&rows, selected, retired) {
        let on_disk = mm.get_model_info(&id).is_some_and(|m| m.is_downloaded);
        let result = if on_disk {
            mm.delete_model(&id)
        } else {
            Ok(())
        };
        match result {
            Ok(()) => {
                log::info!("models: deleted superseded model {id}");
                let _ = db.with_conn(|c| cleanup::mark_deleted(c, &id, now_ms()));
            }
            Err(e) => log::warn!("models: could not delete {id}: {e}"),
        }
    }
    let selected_loaded = rows
        .iter()
        .any(|r| r.handy_id == selected && r.loaded_at.is_some());
    if selected_loaded {
        for r in mm.get_retired_models() {
            match mm.delete_retired_model(&r.id) {
                Ok(()) => log::info!("models: removed retired model {}", r.id),
                Err(e) => log::warn!("models: could not remove retired model {}: {e}", r.id),
            }
        }
    }
}

/// Whether `handy_id` is a model the policy retired.
pub fn is_retired_handy_id(handy_id: &str) -> bool {
    policy::TABLE.as_ref().is_some_and(|t| {
        t.retired()
            .iter()
            .filter_map(|r| super::starter::pinned(r, None))
            .any(|p| p.handy_id() == handy_id)
    })
}

/// Handy ids the Models panel hides (B-57, DEC-27): retired by the policy
/// or restricted by the licence filter, unless the file is on disk.
pub fn hidden_ids(models: Vec<crate::managers::model::ModelInfo>) -> Vec<String> {
    let shown: HashSet<String> = super::license_filter::catalog_view(models.clone())
        .into_iter()
        .map(|m| m.id)
        .collect();
    models
        .into_iter()
        .filter(|m| !m.is_downloaded && (!shown.contains(&m.id) || is_retired_handy_id(&m.id)))
        .map(|m| m.id)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn langs(l: &[&str]) -> Vec<String> {
        l.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn os_languages_add_english_chosen_ones_do_not() {
        assert_eq!(
            policy_languages(&[], &langs(&["nl-NL"])),
            langs(&["nl", "en"])
        );
        assert_eq!(
            policy_languages(&[], &langs(&["en-GB", "fr"])),
            langs(&["en", "fr"])
        );
        assert_eq!(
            policy_languages(&langs(&["nl"]), &langs(&["fr"])),
            langs(&["nl"])
        );
        assert_eq!(policy_languages(&[], &[]), langs(&["en"]));
    }

    #[test]
    fn plans() {
        assert_eq!(plan("a", Some("a"), true, false, false), Plan::Stay);
        assert_eq!(plan("a", None, false, false, false), Plan::Stay);
        assert_eq!(plan("a", Some("b"), true, true, false), Plan::Stay);
        assert_eq!(
            plan("a", Some("b"), true, false, false),
            Plan::Switch { to: "b".into() }
        );
        assert_eq!(
            plan("a", Some("b"), false, false, false),
            Plan::Download { to: "b".into() }
        );
        mark_failed("broken");
        assert_eq!(plan("a", Some("broken"), true, false, false), Plan::Stay);
    }

    #[test]
    fn a_deleted_model_is_not_downloaded_again_during_the_cooldown() {
        // Mac wave 1: 110m was deleted by the swap back and downloaded again
        // minutes later, cycle after cycle.
        assert_eq!(plan("a", Some("b"), false, false, true), Plan::Stay);
        // On disk it costs no download.
        assert_eq!(
            plan("a", Some("b"), true, false, true),
            Plan::Switch { to: "b".into() }
        );
        let mut r = cleanup::Row {
            handy_id: "b".into(),
            origin: Origin::Policy,
            loaded_at: Some(1),
            superseded_at: Some(2),
            deleted_at: Some(1_000),
        };
        let rows = [r.clone()];
        assert!(cleanup::deleted_within(
            &rows,
            "b",
            1_000 + REDOWNLOAD_COOLDOWN_MS - 1,
            REDOWNLOAD_COOLDOWN_MS
        ));
        assert!(!cleanup::deleted_within(
            &rows,
            "b",
            1_000 + REDOWNLOAD_COOLDOWN_MS,
            REDOWNLOAD_COOLDOWN_MS
        ));
        assert!(!cleanup::deleted_within(
            &rows,
            "c",
            1_000,
            REDOWNLOAD_COOLDOWN_MS
        ));
        r.deleted_at = None;
        assert!(!cleanup::deleted_within(
            &[r],
            "b",
            1_000,
            REDOWNLOAD_COOLDOWN_MS
        ));
    }

    #[test]
    fn a_slow_verdict_outlives_the_swap_it_caused() {
        let class = 1.0;
        let day = 86_400_000;
        // Confirmed slow on the big model: used and saved.
        assert_eq!(measured(Some(18.2), class, None, 0), (Some(18.2), true));
        // After the swap the light model has no takes yet: the saved verdict
        // keeps it (before the fix the class factor picked the big model
        // again at once).
        assert_eq!(
            measured(None, class, Some((18.2, 0)), 3 * day),
            (Some(18.2), false)
        );
        // A fast reading does not clear it early.
        assert_eq!(
            measured(Some(0.9), class, Some((18.2, 0)), 3 * day),
            (Some(18.2), false)
        );
        // It expires.
        assert_eq!(
            measured(None, class, Some((18.2, 0)), VERDICT_TTL_MS + 1),
            (None, false)
        );
        assert_eq!(measured(None, class, None, 0), (None, false));
        assert_eq!(
            parse_verdict("18.2@1730000000000"),
            Some((18.2, 1730000000000))
        );
        assert_eq!(parse_verdict("junk"), None);
        // The cooldown is shorter than the verdict, so the expired verdict
        // can bring the better model back once, and fresh evidence decides.
        const { assert!(REDOWNLOAD_COOLDOWN_MS < VERDICT_TTL_MS) };
    }

    #[test]
    fn the_load_spike_of_mac_wave_1_no_longer_loops() {
        // Replays finding 12 through the pure parts: an English user on the
        // M5 Max (class factor 1), unified-en selected, ten takes timed
        // while the load was 60-96.
        let table = policy::TABLE.as_ref().unwrap();
        let (load, cores) = (Some(80.0), 18);
        // 1. The busy takes are never recorded, so nothing is confirmed.
        assert!(!crate::fork::models::speed::machine_quiet(load, cores));
        let (factor, _) = measured(None, 1.0, None, 0);
        assert_eq!(factor, None);
        let m = Machine {
            chip: Chip::AppleSilicon {
                generation: 5,
                tier: crate::fork::models::machine::Tier::Max,
            },
            ram_bytes: 128 << 30,
            free_disk_bytes: Some(500 << 30),
        };
        let langs = vec!["en".to_string()];
        let pick = |measured_factor: Option<f64>, on_disk: &[String]| {
            policy::decide(
                table,
                &policy::Inputs {
                    languages: &langs,
                    machine: &m,
                    cc_by: policy::CC_BY_ALLOWED,
                    measured_factor,
                    on_disk,
                },
            )
            .unwrap()
            .model
        };
        let big = pick(factor, &[]);
        // 2. Even if a confirmed slow verdict did move to a lighter model,
        // the saved verdict keeps the pick there while the light model has
        // no takes, instead of flipping back.
        let light = pick(Some(18.2), &[]);
        assert_ne!(big, light);
        let (after_swap, _) = measured(None, 1.0, Some((18.2, 0)), 60_000);
        assert_eq!(pick(after_swap, &[]), light);
    }

    #[test]
    fn swaps_wait_for_a_quiet_moment() {
        // Never during a take.
        assert!(!is_quiet(true, 0, 1_000_000));
        // Not right after one.
        assert!(!is_quiet(false, 1_000_000, 1_000_000 + QUIET_MS - 1));
        assert!(is_quiet(false, 1_000_000, 1_000_000 + QUIET_MS));
        assert!(is_quiet(false, 0, 1_000_000));
    }

    #[test]
    fn a_different_selection_is_the_users_only_if_ours_is_still_there() {
        assert!(user_picked("mine", Some("ours"), true));
        // Our model was deleted: Handy fell back, not the user.
        assert!(!user_picked("other", Some("ours"), false));
        assert!(!user_picked("ours", Some("ours"), true));
        assert!(!user_picked("x", None, true));
        assert!(!user_picked("", Some("ours"), true));
    }

    #[test]
    fn a_language_change_in_handy_wakes_the_policy() {
        assert!(crate::fork::observe_handy_command("change_spoken_languages_setting").is_some());
        assert!(crate::fork::observe_handy_command("get_app_settings").is_none());
        // Handy still has that command (the name is the contract).
        assert!(include_str!("../../lib.rs").contains("shortcut::change_spoken_languages_setting"));
    }

    #[test]
    fn the_status_is_for_advanced_only() {
        // The bar's status never names a model (S-07a); this one may, and
        // its keys say so.
        let json = serde_json::to_value(status()).unwrap();
        assert!(json.get("current_model").is_some());
        assert!(json.get("automatic").is_some());
    }
}
