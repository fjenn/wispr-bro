// SPDX-License-Identifier: GPL-3.0-or-later
//
// RC-06 mic loss and permission revoke mid-take, through handy-recorder (the
// app's recorder). There is no USB mic or AirPods on the test Mac, so a
// CoreAudio aggregate device wrapping the built-in mic stands in for one and
// is destroyed mid-take: the HAL removes the device exactly as on unplug.

use std::path::Path;
use std::process::Command;
use std::sync::{Arc, Mutex};
use std::time::Instant;

use handy_recorder::{
    list_input_devices, permission_status, CollectingSink, EndReason, Recorder, RecorderConfig,
};
use serde_json::{json, Value};

use crate::{ffi, sleep_ms, write};

const AGG_NAME: &str = "RS-11 test mic";
const AGG_UID: &str = "com.wisprbro.spike.rs11.mic";
const RATE: f64 = 16_000.0;

/// RMS per 250 ms window, rounded, to show where audio stops.
fn rms_windows(s: &[f32]) -> Vec<f32> {
    s.chunks((RATE / 4.0) as usize)
        .map(|c| {
            let e = (c.iter().map(|x| x * x).sum::<f32>() / c.len().max(1) as f32).sqrt();
            (e * 1e5).round() / 1e5
        })
        .collect()
}

/// Fraction of exact-zero samples per 250 ms window (digital silence).
fn zero_windows(s: &[f32]) -> Vec<f32> {
    s.chunks((RATE / 4.0) as usize)
        .map(|c| {
            let z = c.iter().filter(|x| **x == 0.0).count() as f32 / c.len().max(1) as f32;
            (z * 100.0).round() / 100.0
        })
        .collect()
}

fn end_reason(r: &EndReason) -> String {
    match r {
        EndReason::StopCalled => "StopCalled".into(),
        EndReason::RecorderFailed(e) => format!("RecorderFailed({:?})", e.kind()),
        EndReason::SinkPanicked(m) => format!("SinkPanicked({m})"),
    }
}

/// One take: record ~3 s, remove the device, keep the take open 2 s, stop.
fn one_take(as_default: bool) -> Value {
    let Some(orig) = ffi::default_input() else {
        return json!({"error": "no default input"});
    };
    let Some(orig_uid) = ffi::device_uid(orig) else {
        return json!({"error": "no uid"});
    };
    // A fresh aggregate per take; private unless it must become the default.
    let agg = match ffi::create_aggregate(AGG_NAME, AGG_UID, &orig_uid, !as_default) {
        Ok(id) => id,
        Err(e) => return json!({"error": format!("create aggregate: {e}")}),
    };
    sleep_ms(1500);
    if as_default && !ffi::set_default_input(agg) {
        ffi::destroy_aggregate(agg);
        return json!({"error": "could not make the aggregate the default"});
    }
    sleep_ms(500);
    let device = if as_default {
        None
    } else {
        list_input_devices()
            .ok()
            .and_then(|l| l.into_iter().find(|d| d.name == AGG_NAME))
            .map(|d| d.id)
    };
    if !as_default && device.is_none() {
        ffi::destroy_aggregate(agg);
        return json!({"error": "aggregate not listed by handy-recorder"});
    }
    let failure: Arc<Mutex<Option<(Instant, String)>>> = Default::default();
    let f2 = failure.clone();
    let cfg = RecorderConfig {
        device,
        ..RecorderConfig::speech()
    };
    let rec = match Recorder::open_with_failure_handler(cfg, move |e| {
        if let Ok(mut f) = f2.lock() {
            *f = Some((Instant::now(), format!("{:?}", e.kind())));
        }
    }) {
        Ok(r) => r,
        Err(e) => {
            ffi::destroy_aggregate(agg);
            if as_default {
                ffi::set_default_input(orig);
            }
            return json!({"error": format!("open: {e}")});
        }
    };
    let opened = rec.info().device.name.clone();
    if rec.start(CollectingSink::new()).is_err() {
        return json!({"error": "start failed"});
    }
    let t_start = Instant::now();
    sleep_ms(3000);
    let t_kill = Instant::now();
    let destroyed = ffi::destroy_aggregate(agg);
    sleep_ms(2000);
    let stopped = rec.stop();
    let default_after = ffi::default_input().and_then(ffi::device_uid);
    // Put the user's default input back if the take changed it.
    if as_default && ffi::default_input() != Some(orig) {
        ffi::set_default_input(orig);
    }
    let restored = ffi::default_input() == Some(orig);
    drop(rec);
    let fail = failure.lock().ok().and_then(|f| f.clone());
    match stopped {
        Ok(s) => {
            let samples = s.sink.into_samples();
            let secs = samples.len() as f64 / RATE;
            json!({
                "opened": opened, "destroy_err": destroyed,
                "kill_at_s": (t_kill - t_start).as_secs_f64(),
                "audio_kept_s": secs,
                "end_reason": end_reason(&s.end_reason),
                "dropped_frames": s.dropped_frames,
                "failure_after_kill_ms": fail.as_ref().map(|(t, _)| t.saturating_duration_since(t_kill).as_secs_f64() * 1000.0),
                "failure_kind": fail.map(|f| f.1),
                "rms_250ms": rms_windows(&samples),
                "default_after_take": default_after, "default_restored": restored,
            })
        }
        Err(e) => json!({"error": format!("stop: {e}"), "default_restored": restored}),
    }
}

pub fn loss(out: &Path, as_default: bool) -> Value {
    let orig = ffi::default_input();
    let rows: Vec<Value> = (0..5)
        .map(|_| {
            let r = one_take(as_default);
            sleep_ms(1000);
            r
        })
        .collect();
    let restored = ffi::default_input() == orig;
    let kept_ok = rows
        .iter()
        .filter(|r| {
            r["audio_kept_s"]
                .as_f64()
                .map(|s| s >= 2.8)
                .unwrap_or(false)
        })
        .count();
    let failed_reported = rows
        .iter()
        .filter(|r| r["failure_kind"].is_string())
        .count();
    let name = if as_default {
        "mic-default"
    } else {
        "mic-loss"
    };
    let v = json!({"summary": {"takes": rows.len(), "audio_up_to_loss_kept": kept_ok,
        "failure_reported": failed_reported, "default_input_restored": restored},
        "rows": rows});
    write(out, name, &v);
    v
}

pub const BUNDLE_ID: &str = "com.wisprbro.spike.small";

/// Inside the test .app: open the mic (prompting if needed), record 12 s,
/// and log every permission status change and recorder failure.
pub fn perm_take(out: &Path) -> Value {
    let t0 = Instant::now();
    let ms = |t: Instant| (t - t0).as_secs_f64() * 1000.0;
    let status0 = format!("{:?}", permission_status());
    let failure: Arc<Mutex<Option<(f64, String)>>> = Default::default();
    let f2 = failure.clone();
    let rec = Recorder::open_with_failure_handler(RecorderConfig::speech(), move |e| {
        if let Ok(mut f) = f2.lock() {
            *f = Some((
                (Instant::now() - t0).as_secs_f64() * 1000.0,
                format!("{:?}", e.kind()),
            ));
        }
    });
    let rec = match rec {
        Ok(r) => r,
        Err(e) => {
            let v =
                json!({"error": format!("open: {e} ({:?})", e.kind()), "status_before": status0});
            write(out, "perm-take", &v);
            return v;
        }
    };
    if rec.start(CollectingSink::new()).is_err() {
        let v = json!({"error": "start failed"});
        write(out, "perm-take", &v);
        return v;
    }
    let started = ms(Instant::now());
    let mut changes = vec![json!({"ms": started, "status": format!("{:?}", permission_status())})];
    let mut last = format!("{:?}", permission_status());
    // Wait (up to 8 s) for the prompt to be answered before the driver may revoke.
    let wait_end = Instant::now() + std::time::Duration::from_secs(8);
    while Instant::now() < wait_end && last == "NotDetermined" {
        sleep_ms(50);
        let s = format!("{:?}", permission_status());
        if s != last {
            changes.push(json!({"ms": ms(Instant::now()), "status": s}));
            last = s;
        }
    }
    let armed = ms(Instant::now());
    let _ = std::fs::write(out.join("perm-recording"), format!("{armed}"));
    let end = Instant::now() + std::time::Duration::from_secs(10);
    while Instant::now() < end {
        let s = format!("{:?}", permission_status());
        if s != last {
            changes.push(json!({"ms": ms(Instant::now()), "status": s}));
            last = s;
        }
        sleep_ms(50);
    }
    let stopped = rec.stop();
    let fail = failure.lock().ok().and_then(|f| f.clone());
    let v = match stopped {
        Ok(s) => {
            let samples = s.sink.into_samples();
            json!({"status_before": status0, "recording_started_ms": started,
                "status_changes": changes, "audio_kept_s": samples.len() as f64 / RATE,
                "end_reason": end_reason(&s.end_reason),
                "failure": fail.map(|f| json!({"ms": f.0, "kind": f.1})),
                "revoke_armed_ms": armed,
                "rms_250ms": rms_windows(&samples),
                "zero_fraction_250ms": zero_windows(&samples)})
        }
        Err(e) => json!({"error": format!("stop: {e}"), "status_changes": changes}),
    };
    write(out, "perm-take", &v);
    v
}

fn sh(cmd: &str, args: &[&str]) -> String {
    Command::new(cmd)
        .args(args)
        .output()
        .map(|o| {
            format!(
                "{}{}",
                String::from_utf8_lossy(&o.stdout),
                String::from_utf8_lossy(&o.stderr)
            )
            .trim()
            .to_string()
        })
        .unwrap_or_default()
}

/// Build the test .app (ad-hoc signed, its own TCC identity), grant it the
/// mic through the real prompt, revoke it 3 s into a take with
/// `tccutil reset`, and collect what the take saw. Leaves no TCC entry.
pub fn perm_driver(out: &Path, args: &[String]) -> Value {
    let exe = std::env::current_exe().unwrap_or_default();
    let app = exe
        .parent()
        .map(|p| p.join("WBSpikeSmall.app"))
        .unwrap_or_default();
    let macos = app.join("Contents/MacOS");
    let _ = std::fs::create_dir_all(&macos);
    let _ = std::fs::copy(&exe, macos.join("wb-spike-small"));
    let plist = format!(
        r#"<?xml version="1.0" encoding="UTF-8"?>
<plist version="1.0"><dict>
<key>CFBundleIdentifier</key><string>{BUNDLE_ID}</string>
<key>CFBundleName</key><string>WB Spike Small</string>
<key>CFBundleExecutable</key><string>wb-spike-small</string>
<key>CFBundlePackageType</key><string>APPL</string>
<key>CFBundleVersion</key><string>1</string>
<key>LSUIElement</key><true/>
<key>NSMicrophoneUsageDescription</key><string>RS-11 test: records the mic to check what happens when access is revoked.</string>
</dict></plist>"#
    );
    let _ = std::fs::write(app.join("Contents/Info.plist"), plist);
    let sign = sh(
        "codesign",
        &[
            "--force",
            "--sign",
            "-",
            "--identifier",
            BUNDLE_ID,
            &app.to_string_lossy(),
        ],
    );
    let _ = sh("tccutil", &["reset", "Microphone", BUNDLE_ID]);
    let _ = std::fs::remove_file(out.join("perm-recording"));
    let _ = std::fs::remove_file(out.join("perm-take.json"));
    let revoke_after_ms: u64 = args
        .iter()
        .position(|a| a == "--revoke-ms")
        .and_then(|i| args.get(i + 1))
        .and_then(|v| v.parse().ok())
        .unwrap_or(3000);
    let mut child = match Command::new("open")
        .args(["-n", "-W"])
        .arg(&app)
        .args(["--args", "perm-take", "--out"])
        .arg(out)
        .spawn()
    {
        Ok(c) => c,
        Err(e) => return json!({"error": e.to_string()}),
    };
    // Answer the prompt, if one appears.
    let prompt = crate::prompt::allow_mic_prompt(out, 20_000);
    // Revoke `revoke_after_ms` into the recording.
    let mut revoked_at = Value::Null;
    for _ in 0..300 {
        if out.join("perm-recording").exists() {
            sleep_ms(revoke_after_ms);
            let how = args
                .iter()
                .position(|a| a == "--revoke")
                .and_then(|i| args.get(i + 1))
                .cloned()
                .unwrap_or_else(|| "settings".into());
            let r = if how == "tccutil" {
                json!(sh("tccutil", &["reset", "Microphone", BUNDLE_ID]))
            } else {
                crate::prompt::revoke_in_settings(out, "WBSpikeSmall")
            };
            revoked_at = json!({"after_armed_ms": revoke_after_ms, "how": how, "result": r});
            break;
        }
        sleep_ms(100);
    }
    let _ = child.wait();
    let take: Value = std::fs::read_to_string(out.join("perm-take.json"))
        .ok()
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or(Value::Null);
    let cleanup = sh("tccutil", &["reset", "Microphone", BUNDLE_ID]);
    let v = json!({"summary": {"prompt": prompt, "revoked": revoked_at, "take": take},
                   "codesign": sign, "cleanup": cleanup});
    write(out, "perm", &v);
    v
}
