// SPDX-License-Identifier: GPL-3.0-or-later
//
//! A take through the C API from a WAV file, for checks without a
//! microphone (the simulator, CI): set `WB_IOS_SELFTEST_WAV` to one or more
//! 16 kHz mono 16-bit WAV paths (comma-separated) and launch the app
//! (`SIMCTL_CHILD_WB_IOS_SELFTEST_WAV=… xcrun simctl launch …`). Each file
//! goes through `wb_take_start` / `wb_take_feed` / `wb_take_finish` exactly
//! as the Swift session would send it, once a speech model is ready. The
//! results are logged (`ios selftest:`) and written to
//! `<app data>/ios-selftest.json`. `WB_IOS_SELFTEST_REPEAT` runs each file
//! that many times (the first run includes the model load).
//!
//! Self-test takes are ours, not the user's: they leave no History row, no
//! stats and no recording (`core::set_selftest`). More switches, for
//! measurements on a phone: `WB_IOS_SELFTEST_DELAY=<s>` waits before the
//! first take (time to put the app in the background),
//! `WB_IOS_SELFTEST_WAIT_LLM=<s>` waits that long for the cleanup model to
//! be ready, `WB_IOS_SELFTEST_GAP_MS=<ms>` pauses between takes. Each take
//! reports the cleanup step (outcome, model time), the app's footprint and
//! whether the app was in front. `WB_IOS_SELFTEST_REALTIME=1` feeds the
//! audio at speaking speed (the cleanup model loads and caches its
//! instructions while a take is spoken, as with a real take).
//! `WB_IOS_SELFTEST_LEVEL=light|medium|none` cleans the self-test's takes at
//! that level without touching the setting; `WB_IOS_SELFTEST_THREADS=2,4;--no-repack`
//! (debug builds) runs the whole list once per thread count (and extra
//! llama-server arguments after a `;`), restarting the model in between; `WB_IOS_SELFTEST_MEMORY_WARNING_AT=<n>` raises the
//! app's memory-warning handler before take n. A relative WAV path is read
//! from the app's data folder. The report file is rewritten after every
//! take.

use std::time::{Duration, Instant};

use tauri::{AppHandle, Manager};

use super::core;

pub const WAV_ENV: &str = "WB_IOS_SELFTEST_WAV";
pub const REPEAT_ENV: &str = "WB_IOS_SELFTEST_REPEAT";
/// How long the self-test waits for a speech model (a first download).
const MODEL_WAIT: Duration = Duration::from_secs(900);
/// One feed, as the Swift audio tap sends it (~33 ms).
const FEED_SAMPLES: usize = 528;

/// `WB_IOS_SELFTEST_END_SESSION_AFTER=<seconds>`: ends the Swift session
/// that long after launch (a listening take is finished and delivered
/// first), and logs the session state before and after. With the Swift
/// side's `WB_FAKE_MIC_WAV` and a `wisprbro://session/start?…&take=N` link
/// this drives one take through the native session without a keyboard.
pub const END_SESSION_ENV: &str = "WB_IOS_SELFTEST_END_SESSION_AFTER";

fn end_session_later() {
    let Some(seconds) = std::env::var(END_SESSION_ENV)
        .ok()
        .and_then(|v| v.parse::<u64>().ok())
    else {
        return;
    };
    let _ = std::thread::Builder::new()
        .name("wb-ios-selftest-session".into())
        .spawn(move || {
            std::thread::sleep(Duration::from_secs(seconds));
            log::info!(
                "ios selftest: session before end {}",
                super::host::session_state()
            );
            super::host::end_session();
            for _ in 0..40 {
                std::thread::sleep(Duration::from_millis(500));
                let state = super::host::session_state();
                if state.get("state").and_then(|s| s.as_str()) == Some("noSession") {
                    break;
                }
            }
            log::info!(
                "ios selftest: session after end {}",
                super::host::session_state()
            );
        });
}

/// `WB_IOS_SELFTEST_OPEN_URL=<url>`: handles that URL 5 s after launch as
/// if iOS had opened the app with it (e.g. a `wisprbro://session/start` link).
pub const OPEN_URL_ENV: &str = "WB_IOS_SELFTEST_OPEN_URL";

extern "C" {
    fn wb_ios_scene_open_url(url: *const std::ffi::c_char);
}

fn open_url_later() {
    let Some(url) = std::env::var(OPEN_URL_ENV)
        .ok()
        .and_then(|url| std::ffi::CString::new(url).ok())
    else {
        return;
    };
    let _ = std::thread::Builder::new()
        .name("wb-ios-selftest-url".into())
        .spawn(move || {
            std::thread::sleep(Duration::from_secs(5));
            log::info!("ios selftest: opening the session link");
            // SAFETY: a NUL-terminated string that outlives the call.
            unsafe { wb_ios_scene_open_url(url.as_ptr()) };
        });
}

static LEVEL: std::sync::Mutex<Option<crate::fork::settings::CleanupLevel>> =
    std::sync::Mutex::new(None);

/// The cleanup level of the self-test's takes while they run, if one was
/// asked for (`WB_IOS_SELFTEST_LEVEL`).
pub fn level_override() -> Option<crate::fork::settings::CleanupLevel> {
    *LEVEL.lock().unwrap_or_else(|p| p.into_inner())
}

fn set_level(level: Option<crate::fork::settings::CleanupLevel>) {
    *LEVEL.lock().unwrap_or_else(|p| p.into_inner()) = level;
}

pub fn start(app: &AppHandle) {
    end_session_later();
    open_url_later();
    let Some(list) = std::env::var_os(WAV_ENV) else {
        return;
    };
    let files: Vec<String> = list
        .to_string_lossy()
        .split(',')
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(str::to_string)
        .collect();
    if files.is_empty() {
        return;
    }
    let repeat = std::env::var(REPEAT_ENV)
        .ok()
        .and_then(|v| v.parse::<usize>().ok())
        .unwrap_or(1)
        .clamp(1, 200);
    let app = app.clone();
    let _ = std::thread::Builder::new()
        .name("wb-ios-selftest".into())
        .spawn(move || run(&app, &files, repeat));
}

fn run(app: &AppHandle, files: &[String], repeat: usize) {
    log::info!(
        "ios selftest: {} file(s), waiting for a speech model",
        files.len()
    );
    let waited = Instant::now();
    while !crate::fork::models::setup::is_ready() {
        if waited.elapsed() > MODEL_WAIT {
            log::error!(
                "ios selftest: no speech model after {MODEL_WAIT:?}: {}",
                core::status()
            );
            write_report(
                app,
                &serde_json::json!({"error": "no speech model", "status": core::status()}),
            );
            return;
        }
        std::thread::sleep(Duration::from_millis(500));
    }
    log::info!("ios selftest: status {}", core::status());
    let seconds = |var: &str| std::env::var(var).ok().and_then(|v| v.parse::<u64>().ok());
    if let Some(wait) = seconds("WB_IOS_SELFTEST_WAIT_LLM") {
        let t0 = Instant::now();
        let ready = || crate::fork::llm::runtime::runtime(app).is_some_and(|rt| rt.is_ready());
        while !ready() && t0.elapsed() < Duration::from_secs(wait) {
            std::thread::sleep(Duration::from_millis(500));
        }
        log::info!(
            "ios selftest: cleanup model ready: {} after {:?}",
            ready(),
            t0.elapsed()
        );
    }
    if let Some(delay) = seconds("WB_IOS_SELFTEST_DELAY") {
        log::info!("ios selftest: first take in {delay} s");
        std::thread::sleep(Duration::from_secs(delay));
    }
    let gap = seconds("WB_IOS_SELFTEST_GAP_MS").unwrap_or(0);
    let realtime = std::env::var("WB_IOS_SELFTEST_REALTIME").is_ok_and(|v| v == "1");
    let warn_at = seconds("WB_IOS_SELFTEST_MEMORY_WARNING_AT");
    let level = std::env::var("WB_IOS_SELFTEST_LEVEL")
        .ok()
        .and_then(|v| match v.as_str() {
            "light" => Some(crate::fork::settings::CleanupLevel::Light),
            "medium" => Some(crate::fork::settings::CleanupLevel::Medium),
            "none" => Some(crate::fork::settings::CleanupLevel::None),
            _ => None,
        });
    // Model configurations to compare, `threads[;extra llama-server
    // arguments]` separated by commas (debug builds read `WB_LLM_THREADS`
    // and `WB_LLM_EXTRA_ARGS`).
    let threads: Vec<Option<String>> = match std::env::var("WB_IOS_SELFTEST_THREADS") {
        Ok(list) if cfg!(debug_assertions) => list
            .split(',')
            .map(|n| Some(n.trim().to_string()))
            .collect(),
        _ => vec![None],
    };
    let files: Vec<String> = files
        .iter()
        .map(|file| {
            if std::path::Path::new(file).is_absolute() {
                file.clone()
            } else {
                app.path()
                    .app_data_dir()
                    .map(|dir| dir.join(file).to_string_lossy().into_owned())
                    .unwrap_or_else(|_| file.clone())
            }
        })
        .collect();
    let mut results = Vec::new();
    let mut take_id: u64 = 9_000;
    core::set_selftest(true);
    set_level(level);
    for config in &threads {
        if let Some(config) = config {
            let (n, extra) = config.split_once(';').unwrap_or((config.as_str(), ""));
            std::env::set_var("WB_LLM_THREADS", n.trim());
            if extra.trim().is_empty() {
                std::env::remove_var("WB_LLM_EXTRA_ARGS");
            } else {
                std::env::set_var("WB_LLM_EXTRA_ARGS", extra.trim());
            }
            // The next take starts the model again with that many threads.
            if let Some(sup) =
                crate::fork::llm::runtime::runtime(app).and_then(|rt| rt.supervisor().cloned())
            {
                tauri::async_runtime::block_on(sup.stop());
            }
        }
        for round in 0..repeat {
            for file in &files {
                take_id += 1;
                if warn_at == Some(take_id - 9_000) {
                    log::info!(
                        "ios selftest: memory warning before take {}",
                        take_id - 9_000
                    );
                    super::memory::wb_ios_memory_warning();
                }
                let mut result = one(file, round, take_id, realtime);
                result["threads"] = serde_json::json!(config);
                results.push(result);
                write_report(
                    app,
                    &serde_json::json!({ "running": true, "takes": results }),
                );
                if gap > 0 {
                    std::thread::sleep(Duration::from_millis(gap));
                }
            }
        }
    }
    set_level(None);
    core::set_selftest(false);
    let report = serde_json::json!({
        "status": core::status(),
        "backend": app
            .try_state::<std::sync::Arc<crate::managers::transcription::TranscriptionManager>>()
            .and_then(|tm| tm.current_backend()),
        "llm": crate::fork::llm::runtime::runtime(app).map(|rt| rt.status().state),
        // From the Swift session code, or the weak stand-ins without it.
        "session": super::host::session_state(),
        "session_timeout": super::defaults::session_timeout(),
        "auto_return": super::defaults::auto_return(),
        "takes": results,
    });
    log::info!("ios selftest: done {report}");
    write_report(app, &report);
}

fn one(file: &str, round: usize, take_id: u64, realtime: bool) -> serde_json::Value {
    let samples = match crate::audio_toolkit::read_wav_samples(file) {
        Ok(samples) => samples,
        Err(error) => {
            log::error!("ios selftest: cannot read {file}: {error}");
            return serde_json::json!({"file": file, "error": error.to_string()});
        }
    };
    let audio_seconds = samples.len() as f64 / 16_000.0;
    let rss_before = memory();
    core::wb_take_start(take_id);
    let mut peak_level = 0.0f32;
    let feeding = Instant::now();
    for chunk in samples.chunks(FEED_SAMPLES) {
        // SAFETY: the pointer and length are the chunk's.
        let level = unsafe { core::wb_take_feed(take_id, chunk.as_ptr(), chunk.len()) };
        peak_level = peak_level.max(level);
        if realtime {
            std::thread::sleep(Duration::from_micros(
                chunk.len() as u64 * 1_000_000 / 16_000,
            ));
        }
    }
    let feed_ms = feeding.elapsed().as_millis() as u64;
    let finishing = Instant::now();
    let reply = core::wb_take_finish(take_id);
    let finish_ms = finishing.elapsed().as_millis() as u64;
    let json = if reply.is_null() {
        String::new()
    } else {
        // SAFETY: `reply` is the NUL-terminated string `wb_take_finish`
        // allocated; it is freed once, below.
        unsafe {
            let json = std::ffi::CStr::from_ptr(reply)
                .to_string_lossy()
                .into_owned();
            core::wb_string_free(reply);
            json
        }
    };
    let reply: serde_json::Value = serde_json::from_str(&json).unwrap_or_default();
    let rss_after = memory();
    let (transcribe_ms, cleanup_step_ms) = core::last_timing();
    let cleanup = crate::fork::llm::post_process::last_result();
    const MB: u64 = 1 << 20;
    let result = serde_json::json!({
        "file": file,
        "round": round,
        "audio_seconds": audio_seconds,
        "feed_ms": feed_ms,
        "finish_ms": finish_ms,
        "peak_level": peak_level,
        "reply": reply,
        "resident_mb_before": rss_before.0,
        "resident_mb_after": rss_after.0,
        "peak_resident_mb": rss_after.1,
        "footprint_mb": wb_ios_llm::footprint_bytes().map(|b| b / MB),
        "available_mb": wb_ios_llm::available_bytes().map(|b| b / MB),
        "app_state": super::host::app_state(),
        "transcribe_ms": transcribe_ms,
        "cleanup_step_ms": cleanup_step_ms,
        "cleanup": cleanup.map(|c| serde_json::json!({
            "take": c.take,
            "level": c.level,
            "engine": c.engine,
            "outcome": c.outcome,
            "why": c.why,
            "llm_ms": c.llm_ms,
            "chunks": c.chunks_total,
            "applied": c.chunks_applied,
        })),
    });
    log::info!("ios selftest: take {result}");
    result
}

fn write_report(app: &AppHandle, report: &serde_json::Value) {
    let Ok(dir) = app.path().app_data_dir() else {
        return;
    };
    let _ = std::fs::create_dir_all(&dir);
    let path = dir.join("ios-selftest.json");
    if let Err(error) = std::fs::write(&path, format!("{report:#}\n")) {
        log::warn!("ios selftest: could not write the report: {error}");
    }
}

/// `(resident MB now, peak resident MB)` of this process.
fn memory() -> (u64, u64) {
    const MB: u64 = 1 << 20;
    // SAFETY: plain out-parameter calls on this task with correctly sized
    // structs.
    unsafe {
        let mut info: libc::mach_task_basic_info = std::mem::zeroed();
        let mut count = libc::MACH_TASK_BASIC_INFO_COUNT;
        #[allow(deprecated)]
        let task = libc::mach_task_self();
        let ok = libc::task_info(
            task,
            libc::MACH_TASK_BASIC_INFO,
            (&mut info as *mut libc::mach_task_basic_info).cast(),
            &mut count,
        ) == 0;
        if ok {
            (info.resident_size / MB, info.resident_size_max / MB)
        } else {
            (0, 0)
        }
    }
}
