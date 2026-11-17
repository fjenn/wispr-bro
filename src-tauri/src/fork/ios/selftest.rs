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
        .clamp(1, 20);
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
    let mut results = Vec::new();
    let mut take_id: u64 = 9_000;
    for file in files {
        for round in 0..repeat {
            take_id += 1;
            results.push(one(file, round, take_id));
        }
    }
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

fn one(file: &str, round: usize, take_id: u64) -> serde_json::Value {
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
