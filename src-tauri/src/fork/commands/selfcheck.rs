// SPDX-License-Identifier: GPL-3.0-or-later
//! Startup self-check of the fork command and event path (F-05 exit check,
//! added in F-01).
//!
//! Off unless the app is started with `WISPRBRO_SELFCHECK=1`, in debug and in
//! release builds alike (the point is to prove routing and the ACL behaviour
//! in a release build, where the debug panel is hidden). When on,
//! [`start`] (called from `fork::on_ready`) evaluates a small script in the
//! `main` webview. The script goes through the real IPC path, exactly like the
//! frontend does:
//!
//! 1. subscribes to `fork-ping-event` (`plugin:event|listen`),
//! 2. invokes `fork_ping` and `fork_model_status` through
//!    `fork::compose_invoke`,
//! 3. waits for the `ForkPingEvent` that `fork_ping` emits,
//! 4. sends the results back with `fork_selfcheck_report`, which logs one
//!    `fork selfcheck: PASS|FAIL …` line.

use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use serde::Deserialize;
use serde_json::Value;
use tauri::{AppHandle, Manager, Runtime};

use super::ForkError;

/// Env var that turns the self-check on.
pub const SELFCHECK_ENV: &str = "WISPRBRO_SELFCHECK";

static REPORTED: AtomicBool = AtomicBool::new(false);

/// `true` when `WISPRBRO_SELFCHECK` is set to a truthy value.
pub fn enabled() -> bool {
    crate::utils::env_flag_enabled(SELFCHECK_ENV)
}

/// Script evaluated in the `main` webview. Guarded so a retry or a reload
/// never runs two checks at once.
const SCRIPT: &str = r#"
(async () => {
  const T = window.__TAURI_INTERNALS__;
  if (!T || window.__wbSelfcheckRunning) return;
  window.__wbSelfcheckRunning = true;
  const r = {};
  const err = (e) => (typeof e === "object" ? e : { message: String(e) });
  let eventSeq = null;
  try {
    const handler = T.transformCallback((e) => { eventSeq = e.payload.seq; });
    await T.invoke("plugin:event|listen", { event: "fork-ping-event", target: { kind: "Any" }, handler });
    r.listen = "ok";
  } catch (e) { r.listen = err(e); }
  try { r.ping = await T.invoke("fork_ping"); } catch (e) { r.ping = { error: err(e) }; }
  try { r.model_status = await T.invoke("fork_model_status"); } catch (e) { r.model_status = { error: err(e) }; }
  for (let i = 0; i < 30 && eventSeq === null; i++) await new Promise((ok) => setTimeout(ok, 100));
  r.event_seq = eventSeq;
  try { await T.invoke("fork_selfcheck_report", { report: JSON.stringify(r) }); }
  finally { window.__wbSelfcheckRunning = false; }
})();
"#;

/// Starts the self-check in the background if [`enabled`]. Retries the script
/// every 2 s (up to 30 s) until a report arrives, because the page may still
/// be loading when `on_ready` runs.
pub fn start<R: Runtime>(app: &AppHandle<R>) {
    if !enabled() {
        return;
    }
    log::info!("fork selfcheck: {SELFCHECK_ENV} is set, starting");
    let app = app.clone();
    std::thread::spawn(move || {
        for attempt in 1..=15 {
            std::thread::sleep(Duration::from_secs(2));
            if REPORTED.load(Ordering::Acquire) {
                return;
            }
            match app.get_webview_window("main") {
                Some(window) => {
                    if let Err(e) = window.eval(SCRIPT) {
                        log::warn!("fork selfcheck: eval failed (attempt {attempt}): {e}");
                    }
                }
                None => log::warn!("fork selfcheck: no main window (attempt {attempt})"),
            }
        }
        if !REPORTED.load(Ordering::Acquire) {
            log::error!("fork selfcheck: FAIL no report from the main webview after 30 s");
        }
    });
}

#[derive(Debug, Deserialize)]
struct Report {
    listen: Value,
    ping: Value,
    model_status: Value,
    event_seq: Option<u32>,
}

/// Verdict for a report: `Ok(summary)` or `Err(reason)`.
fn judge(report: &Report) -> Result<String, String> {
    if report.listen != Value::String("ok".into()) {
        return Err(format!("event listen failed: {}", report.listen));
    }
    if report.ping.get("message").and_then(Value::as_str) != Some("pong") {
        return Err(format!("fork_ping failed: {}", report.ping));
    }
    let seq = report.ping.get("seq").and_then(Value::as_u64);
    if report.ping.get("ready") != Some(&Value::Bool(true)) {
        return Err(format!("fork_ping says not ready: {}", report.ping));
    }
    let Some(known) = report.model_status.get("known_models") else {
        return Err(format!("fork_model_status failed: {}", report.model_status));
    };
    match report.event_seq {
        Some(event) if Some(u64::from(event)) == seq => Ok(format!(
            "fork_ping seq={event} ready=true, fork_model_status known_models={known}, ForkPingEvent seq={event} received"
        )),
        Some(event) => Err(format!(
            "ForkPingEvent seq={event} does not match fork_ping seq={seq:?}"
        )),
        None => Err("ForkPingEvent was not received".into()),
    }
}

/// Receives the self-check report from the `main` webview and logs the
/// verdict. Rejected unless the self-check is on.
#[tauri::command]
#[specta::specta]
pub fn fork_selfcheck_report<R: Runtime>(
    app: AppHandle<R>,
    report: String,
) -> Result<(), ForkError> {
    if !enabled() {
        return Err(ForkError::Internal("self-check is off".into()));
    }
    REPORTED.store(true, Ordering::Release);
    let title = app
        .get_webview_window("main")
        .and_then(|w| w.title().ok())
        .unwrap_or_default();
    let tooltip = crate::fork::tray::last_tooltip().unwrap_or_default();
    log::info!(
        "fork selfcheck: identity: product {:?}, identifier {:?}, version {:?}, window title {title:?}, tray tooltip {tooltip:?}",
        app.package_info().name,
        app.config().identifier,
        app.package_info().version.to_string(),
    );
    let verdict = serde_json::from_str::<Report>(&report)
        .map_err(|e| format!("unreadable report ({e})"))
        .and_then(|r| judge(&r));
    match verdict {
        Ok(summary) => log::info!(
            "fork selfcheck: PASS ({} build) {summary}",
            if cfg!(debug_assertions) {
                "debug"
            } else {
                "release"
            }
        ),
        Err(reason) => log::error!("fork selfcheck: FAIL {reason}; raw={report}"),
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn report(v: Value) -> Report {
        serde_json::from_value(v).unwrap()
    }

    #[test]
    fn judges_a_full_round_trip_as_pass() {
        let r = report(json!({
            "listen": "ok",
            "ping": { "message": "pong", "seq": 1, "ready": true },
            "model_status": { "known_models": 3, "downloaded_model_ids": [] },
            "event_seq": 1
        }));
        assert!(judge(&r).is_ok());
    }

    #[test]
    fn judges_missing_event_and_not_ready_as_fail() {
        let base = json!({
            "listen": "ok",
            "ping": { "message": "pong", "seq": 1, "ready": true },
            "model_status": { "known_models": 3 },
            "event_seq": null
        });
        assert!(judge(&report(base.clone())).is_err());
        let mut not_ready = base.clone();
        not_ready["event_seq"] = json!(1);
        not_ready["model_status"] = json!({ "error": { "kind": "NotReady" } });
        assert!(judge(&report(not_ready)).is_err());
    }
}
