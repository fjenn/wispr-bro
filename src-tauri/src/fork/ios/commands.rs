// SPDX-License-Identifier: GPL-3.0-or-later
//
//! The iPhone-only commands (`ios_*`), routed by name in
//! `fork::compose_invoke`. They are not in the generated bindings (those are
//! the same on every platform); the page calls them through `invoke`
//! (`src/fork/ios/commands.ts`).

use serde::Serialize;
use tauri::ipc::Invoke;
use tauri::Runtime;

use super::{defaults, host};

pub const PREFIX: &str = "ios_";

/// `ios_session_state`: the Swift session's state. `endsAt` is when the
/// session ends by itself, in ms since the epoch.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct IosSession {
    state: String,
    message: Option<String>,
    ends_at: Option<f64>,
    /// Everything the Swift side published, as is.
    native: serde_json::Value,
}

#[tauri::command]
fn ios_session_state() -> IosSession {
    session()
}

fn session() -> IosSession {
    let native = host::session_state();
    IosSession {
        state: native
            .get("state")
            .and_then(|v| v.as_str())
            .unwrap_or("noSession")
            .to_string(),
        message: native
            .get("message")
            .and_then(|v| v.as_str())
            .map(str::to_string),
        // Swift: seconds since the epoch.
        ends_at: native
            .get("expiresAt")
            .and_then(|v| v.as_f64())
            .map(|seconds| (seconds * 1000.0).round()),
        native,
    }
}

#[tauri::command]
fn ios_start_session() {
    host::start_session();
}

#[tauri::command]
fn ios_end_session() {
    host::end_session();
}

/// `ios_setup_state`. `micPermission`: 0 undetermined, 1 denied, 2 granted.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct IosSetup {
    keyboard_enabled: bool,
    /// Full Access is on (false until the keyboard has reported it).
    full_access: bool,
    /// Whether the keyboard has reported Full Access at all.
    full_access_known: bool,
    mic_permission: u8,
}

#[tauri::command]
fn ios_setup_state() -> IosSetup {
    setup()
}

fn setup() -> IosSetup {
    let full_access = host::full_access();
    IosSetup {
        keyboard_enabled: host::keyboard_enabled(),
        full_access: full_access == Some(true),
        full_access_known: full_access.is_some(),
        mic_permission: match host::mic_permission() {
            host::MicPermission::Undetermined => 0,
            host::MicPermission::Denied => 1,
            host::MicPermission::Granted => 2,
        },
    }
}

#[tauri::command]
fn ios_request_mic() {
    host::request_mic_permission();
}

#[tauri::command]
fn ios_open_settings() {
    host::open_settings();
}

#[tauri::command]
fn ios_get_session_timeout() -> i64 {
    defaults::session_timeout()
}

#[tauri::command]
fn ios_set_session_timeout(seconds: i64) -> Result<(), String> {
    defaults::set_session_timeout(seconds)
}

#[tauri::command]
fn ios_get_auto_return() -> bool {
    defaults::auto_return()
}

#[tauri::command]
fn ios_set_auto_return(enabled: bool) -> Result<(), String> {
    defaults::set_auto_return(enabled)
}

/// `ios_core_status`: what `wb_core_status_json` tells the Swift side.
#[tauri::command]
fn ios_core_status() -> serde_json::Value {
    super::core::status()
}

/// How often the Swift session's published state is compared with the last
/// one the page was told about.
const WATCH_EVERY: std::time::Duration = std::time::Duration::from_millis(250);

/// Tells the page at once when the session or the setup state changes
/// (`ios-session-state`, `ios-setup-state`, the payloads of the two
/// commands): a take started from the keyboard, the session's end, the
/// microphone answer, the keyboard being added. The Swift side keeps its
/// state in memory, so reading it is cheap; nothing is sent while it stays
/// the same.
pub fn watch(app: &tauri::AppHandle) {
    use tauri::Emitter;
    let app = app.clone();
    let spawned = std::thread::Builder::new()
        .name("wb-ios-state".into())
        .spawn(move || {
            let mut last_session = String::new();
            let mut last_setup = String::new();
            loop {
                std::thread::sleep(WATCH_EVERY);
                let now = session();
                // `native` carries values that are not the page's concern
                // and would make every comparison differ for no reason.
                let key = format!("{}|{:?}|{:?}", now.state, now.message, now.ends_at);
                if key != last_session {
                    last_session = key;
                    let _ = app.emit("ios-session-state", &now);
                }
                let now = setup();
                let key = format!(
                    "{}|{}|{}|{}",
                    now.keyboard_enabled,
                    now.full_access,
                    now.full_access_known,
                    now.mic_permission
                );
                if key != last_setup {
                    last_setup = key;
                    let _ = app.emit("ios-setup-state", &now);
                }
            }
        });
    if let Err(e) = spawned {
        log::warn!("ios: the state watcher did not start: {e}");
    }
}

pub fn handler<R: Runtime>() -> impl Fn(Invoke<R>) -> bool + Send + Sync + 'static {
    tauri::generate_handler![
        ios_session_state,
        ios_start_session,
        ios_end_session,
        ios_setup_state,
        ios_request_mic,
        ios_open_settings,
        ios_get_session_timeout,
        ios_set_session_timeout,
        ios_get_auto_return,
        ios_set_auto_return,
        ios_core_status,
    ]
}
