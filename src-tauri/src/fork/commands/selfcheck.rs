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
//! 4. reads the fork settings (`fork_get_settings`), writes one unchanged
//!    value and one out-of-range value (`fork_set_setting`, F-07),
//! 5. runs the CSP smoke check (`window.__wbCspSmoke`, F-08): plays an audio
//!    file through the asset protocol, renders `data:`/`blob:` images, calls a
//!    fork command and collects `securitypolicyviolation` events,
//! 6. sends the results back with `fork_selfcheck_report`, which logs one
//!    `fork selfcheck: PASS|FAIL …` line. Debug builds also log the effective
//!    fork settings and the `ENFORCED` values read back from Handy's settings.

use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use serde::Deserialize;
use serde_json::Value;
use tauri::{AppHandle, Manager, Runtime};

use super::ForkError;
use crate::fork::ForkRuntime;

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
  try { r.llm_status = await T.invoke("fork_llm_status"); } catch (e) { r.llm_status = { error: err(e) }; }
  try { r.settings = await T.invoke("fork_get_settings"); } catch (e) { r.settings = { error: err(e) }; }
  try {
    const value = r.settings && r.settings.context_awareness;
    r.set_same = await T.invoke("fork_set_setting", { update: { key: "context_awareness", value } });
  } catch (e) { r.set_same = { error: err(e) }; }
  try {
    await T.invoke("fork_set_setting", { update: { key: "typing_wpm", value: 100000 } });
    r.set_invalid = "accepted";
  } catch (e) { r.set_invalid = err(e); }
  for (let i = 0; i < 30 && eventSeq === null; i++) await new Promise((ok) => setTimeout(ok, 100));
  r.event_seq = eventSeq;
  try {
    for (let i = 0; i < 50 && !window.__wbCspSmoke; i++) await new Promise((ok) => setTimeout(ok, 100));
    r.csp = window.__wbCspSmoke ? await window.__wbCspSmoke() : { ok: false, error: "CSP smoke not loaded" };
  } catch (e) { r.csp = { ok: false, error: String(e) }; }
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
    #[serde(default)]
    settings: Value,
    #[serde(default)]
    set_same: Value,
    #[serde(default)]
    set_invalid: Value,
    /// F-08: result of `window.__wbCspSmoke()` (`src/fork/debug/cspSmokeChecks.ts`).
    #[serde(default)]
    csp: Value,
    /// S-10: `fork_llm_status`.
    #[serde(default)]
    llm_status: Value,
}

/// Verdict for a report: `Ok(summary)` or `Err(reason)`.
///
/// `tauri dev` on desktop loads the Vite dev server directly and applies no
/// CSP at all (tauri 2.11.5 `protocol/tauri.rs`: only mobile proxies the dev
/// server; `devCsp` is used for pages Tauri serves itself). So debug builds
/// accept an unenforced CSP (the blocked-source probe loads) as long as every
/// other check passes and nothing was blocked; release builds must enforce it.
fn judge(report: &Report) -> Result<String, String> {
    judge_with(report, !cfg!(debug_assertions))
}

/// `true` when every CSP smoke check except the probe passed and no
/// violation was recorded.
fn csp_allowed_sources_ok(csp: &Value) -> bool {
    let checks = &csp["checks"];
    ["media", "dataImage", "blobImage", "forkCommand"]
        .iter()
        .all(|k| checks[*k]["state"] == "ok")
        && csp["violations"].as_array().is_some_and(Vec::is_empty)
}

fn judge_with(report: &Report, require_enforced_csp: bool) -> Result<String, String> {
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
    if report.settings.get("schema_version").is_none() {
        return Err(format!("fork_get_settings failed: {}", report.settings));
    }
    if report.set_same.get("schema_version").is_none() {
        return Err(format!("fork_set_setting failed: {}", report.set_same));
    }
    if report.set_invalid.get("kind").and_then(Value::as_str) != Some("InvalidSetting") {
        return Err(format!(
            "fork_set_setting accepted an invalid value: {}",
            report.set_invalid
        ));
    }
    let Some(llm_state) = report.llm_status.get("state").and_then(Value::as_str) else {
        return Err(format!("fork_llm_status failed: {}", report.llm_status));
    };
    let csp_enforced = report.csp.get("ok") == Some(&Value::Bool(true));
    if !csp_enforced && (require_enforced_csp || !csp_allowed_sources_ok(&report.csp)) {
        return Err(format!("CSP smoke failed: {}", report.csp));
    }
    let csp_summary = if csp_enforced {
        "CSP smoke ok (enforced, probe blocked)"
    } else {
        "CSP smoke ok (allowed sources load; no CSP under tauri dev)"
    };
    match report.event_seq {
        Some(event) if Some(u64::from(event)) == seq => Ok(format!(
            "fork_ping seq={event} ready=true, fork_model_status known_models={known}, ForkPingEvent seq={event} received, fork_get_settings ok, fork_set_setting ok and rejects invalid values, fork_llm_status state={llm_state}, {csp_summary}"
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
pub fn fork_selfcheck_report<R: ForkRuntime>(
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
    if cfg!(debug_assertions) {
        log_settings(&app);
    }
    if let Ok(raw) = serde_json::from_str::<Value>(&report) {
        log::info!("fork selfcheck: CSP smoke: {}", raw["csp"]);
    }
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

/// Debug builds only: the effective fork settings and the `ENFORCED` values
/// as Handy's own settings API reads them back.
fn log_settings<R: ForkRuntime>(app: &AppHandle<R>) {
    use crate::fork::settings::{enforced_snapshot, ForkSettingsHandle, HandySettings};
    match app.try_state::<ForkSettingsHandle>().map(|h| h.get(app)) {
        Some(Ok(settings)) => log::info!(
            "fork selfcheck: fork settings: {}",
            serde_json::to_string(&settings).unwrap_or_default()
        ),
        Some(Err(e)) => log::error!("fork selfcheck: fork settings unreadable: {e}"),
        None => log::error!("fork selfcheck: fork settings not managed"),
    }
    match R::handy_settings(app) {
        Some(HandySettings(handy)) => log::info!(
            "fork selfcheck: Handy ENFORCED values: {}",
            enforced_snapshot(&handy)
        ),
        None => log::warn!("fork selfcheck: Handy settings unavailable"),
    }
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
            "event_seq": 1,
            "settings": { "schema_version": 1 },
            "set_same": { "schema_version": 1 },
            "set_invalid": { "kind": "InvalidSetting", "message": "typing_wpm" },
            "csp": { "ok": true, "violations": [] },
            "llm_status": { "state": "ready" }
        }));
        assert!(judge(&r).unwrap().contains("fork_llm_status state=ready"));
        // S-10: a failed fork_llm_status fails the check.
        let mut no_llm = r;
        no_llm.llm_status = json!({ "error": { "kind": "NotReady" } });
        assert!(judge(&no_llm).unwrap_err().contains("fork_llm_status"));
        let mut failed = serde_json::to_value(json!({
            "listen": "ok",
            "ping": { "message": "pong", "seq": 1, "ready": true },
            "model_status": { "known_models": 3 },
            "event_seq": 1,
            "settings": { "schema_version": 1 },
            "set_same": { "schema_version": 1 },
            "set_invalid": { "kind": "InvalidSetting" },
            "llm_status": { "state": "unsupported" }
        }))
        .unwrap();
        assert!(judge(&report(failed.clone())).is_err(), "missing CSP smoke");
        failed["csp"] = json!({ "ok": false, "violations": [{ "directive": "media-src" }] });
        assert!(judge(&report(failed.clone())).is_err());

        // Unenforced CSP (tauri dev): fine in debug, a failure in release.
        let ok = json!({ "state": "ok" });
        failed["csp"] = json!({
            "ok": false,
            "checks": { "media": ok, "dataImage": ok, "blobImage": ok, "forkCommand": ok,
                        "probe": { "state": "failed" } },
            "violations": []
        });
        assert!(judge_with(&report(failed.clone()), false).is_ok());
        assert!(judge_with(&report(failed), true).is_err());
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
