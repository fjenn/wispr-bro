// SPDX-License-Identifier: GPL-3.0-or-later
//
// RS-03 scenarios, run unattended inside the pill process on a driver thread:
// synthetic HID events (CGEvent), AX reads of TextEdit, the window list and
// screen captures. Each scenario writes `<scenario>.json` to the output dir.

use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::Duration;

use serde_json::{json, Value};
use tauri::AppHandle;

use crate::cg::{self, CGPoint, CGRect};
use crate::pill::{self, events_since, now_ms};

fn sleep_ms(ms: u64) {
    std::thread::sleep(Duration::from_millis(ms));
}

fn sh(cmd: &str, args: &[&str]) -> String {
    Command::new(cmd)
        .args(args)
        .output()
        .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
        .unwrap_or_default()
}

fn pid_of(name: &str) -> Option<i32> {
    sh("pgrep", &["-x", name])
        .lines()
        .next()
        .and_then(|l| l.trim().parse().ok())
}

/// Pill bounds in global CG coordinates (top-left origin), from the window list.
pub fn pill_bounds(app: &AppHandle) -> Option<(i64, CGRect, bool)> {
    let info = pill::info(app)?;
    let wins = cg::onscreen_windows();
    let w = wins.iter().find(|w| w.number == info.window_number);
    Some((
        info.window_number,
        w.map(|w| w.bounds).unwrap_or_default(),
        w.is_some(),
    ))
}

fn pt(r: &CGRect, dx: f64, dy: f64) -> CGPoint {
    CGPoint {
        x: r.origin.x + dx,
        y: r.origin.y + dy,
    }
}

pub struct TextEdit {
    pub pid: i32,
    launched: bool,
    file: PathBuf,
    title: String,
}

impl TextEdit {
    /// TextEdit is frontmost with our document's window focused, so a
    /// synthetic key lands in our document and nowhere else (other agents
    /// may be driving other apps on this Mac at the same time).
    pub fn ours_in_front(&self, app: &AppHandle) -> bool {
        let front = pill::info(app)
            .map(|i| i.frontmost_bundle)
            .unwrap_or_default();
        front == "com.apple.TextEdit"
            && cg::ax_focused_window_title(self.pid)
                .map(|t| t.starts_with(&self.title))
                .unwrap_or(false)
    }

    /// Bring our document back to the front.
    pub fn refocus(&self) {
        let _ = Command::new("open")
            .args(["-a", "TextEdit"])
            .arg(&self.file)
            .status();
        sleep_ms(600);
    }
}

pub fn open_textedit(out: &Path, name: &str) -> Option<TextEdit> {
    let was_running = pid_of("TextEdit").is_some();
    let file = out.join(format!("{name}.txt"));
    let _ = std::fs::write(&file, "");
    let _ = Command::new("open")
        .args(["-a", "TextEdit"])
        .arg(&file)
        .status();
    let title = format!("{name}.txt");
    for _ in 0..50 {
        sleep_ms(200);
        if let Some(pid) = pid_of("TextEdit") {
            let focused = cg::ax_focused_window_title(pid)
                .map(|t| t.starts_with(&title))
                .unwrap_or(false);
            if focused && cg::ax_focused_value(pid).is_some() {
                sleep_ms(500);
                return Some(TextEdit {
                    pid,
                    launched: !was_running,
                    file,
                    title,
                });
            }
        }
    }
    None
}

pub fn close_textedit(app: &AppHandle, t: &TextEdit) {
    // ⌘W closes our (autosaved, file-backed) document, only when it is the
    // focused window; quit TextEdit only if we launched it.
    t.refocus();
    if t.ours_in_front(app) {
        cg::key(13, 0x0010_0000); // ⌘W
        sleep_ms(500);
    }
    if t.launched {
        let _ = Command::new("kill").arg(t.pid.to_string()).status();
    }
}

/// Wait until no app activation was logged for `quiet_ms` (other agents and
/// apps on this Mac), up to `max_ms`. Returns whether it got quiet.
fn wait_quiet(quiet_ms: f64, max_ms: f64) -> bool {
    let start = now_ms();
    loop {
        let t = now_ms();
        let recent = events_since(t - quiet_ms)
            .iter()
            .any(|e| e.kind == "activated");
        if !recent && t - start >= quiet_ms.min(max_ms) {
            return true;
        }
        if t - start > max_ms {
            return false;
        }
        sleep_ms(100);
    }
}

fn write(out: &Path, name: &str, v: &Value) {
    let _ = std::fs::write(
        out.join(format!("{name}.json")),
        serde_json::to_string_pretty(v).unwrap_or_default(),
    );
}

/// RT-13 / RC-01: first click acts, TextEdit keeps focus, typing still lands.
pub fn click(app: &AppHandle, out: &Path, variant: &str, n: usize) -> Value {
    // "alternate" toggles `_setPreventsActivation:` trial by trial in one
    // process; "plain" and "prevent" keep one setting for the whole run.
    let variants: &[bool] = match variant {
        "alternate" => &[false, true],
        "prevent" => &[true],
        _ => &[false],
    };
    let Some(te) = open_textedit(out, &format!("click-{variant}")) else {
        return json!({"error": "TextEdit did not open"});
    };
    let Some((_, b, _)) = pill_bounds(app) else {
        return json!({"error": "pill not on screen"});
    };
    let targets = [
        ("cancel", pt(&b, 20.0, 52.0)),
        ("done", pt(&b, 180.0, 52.0)),
    ];
    let mut rows = Vec::new();
    // (variant, target) -> [trials, acted, kept, typed, never_active, focus_moved_by_click, skipped]
    let mut tally: std::collections::BTreeMap<String, [usize; 7]> = Default::default();
    // Both variants alternate trial by trial in one process, so other activity
    // on the Mac hits both alike.
    for i in 0..n {
        for (name, p) in targets {
            for &prevent in variants {
                let key = format!("{}/{name}", if prevent { "prevent" } else { "plain" });
                // "plain" never calls the private setter at all.
                if variant != "plain" {
                    pill::on_main(app, move |h| {
                        if let Some(w) = tauri::Manager::get_webview_window(h, pill::LABEL) {
                            pill::set_prevents_activation(&w, prevent);
                        }
                    });
                }
                let entry = tally.entry(key.clone()).or_default();
                // Our document in front and the Mac quiet for 1.5 s.
                let mut ready = false;
                for _ in 0..5 {
                    if !te.ours_in_front(app) {
                        te.refocus();
                    }
                    if wait_quiet(1500.0, 15000.0) && te.ours_in_front(app) {
                        ready = true;
                        break;
                    }
                }
                if !ready {
                    entry[6] += 1;
                    rows.push(json!({"variant": key, "i": i, "skipped": "not quiet"}));
                    continue;
                }
                let before = cg::ax_focused_value(te.pid).unwrap_or_default();
                let t0 = now_ms();
                cg::click(p);
                sleep_ms(300);
                let evs = events_since(t0);
                let click_ev = evs
                    .iter()
                    .find(|e| e.kind == "click" && e.detail == name)
                    .map(|e| e.t_ms - t0);
                let became_key = evs.iter().any(|e| e.kind == "became_key");
                // An activation within 250 ms of the click is the click's doing.
                let activations: Vec<_> = evs
                    .iter()
                    .filter(|e| e.kind == "activated")
                    .map(|e| json!({"ms": e.t_ms - t0, "app": e.detail}))
                    .collect();
                let moved = evs
                    .iter()
                    .any(|e| e.kind == "activated" && e.t_ms - t0 <= 250.0);
                let inf = pill::info(app);
                let front = inf
                    .as_ref()
                    .map(|i| i.frontmost_bundle.clone())
                    .unwrap_or_default();
                let app_active = inf.as_ref().map(|i| i.app_active).unwrap_or(false);
                // Type only into our own document.
                let ours = te.ours_in_front(app);
                let typed_ok = if ours {
                    cg::key(7, 0); // "x"
                    sleep_ms(200);
                    let after = cg::ax_focused_value(te.pid).unwrap_or_default();
                    after.chars().count() == before.chars().count() + 1
                } else {
                    false
                };
                entry[0] += 1;
                entry[1] += click_ev.is_some() as usize;
                entry[2] += ours as usize;
                entry[3] += typed_ok as usize;
                entry[4] += (!app_active && !became_key) as usize;
                entry[5] += moved as usize;
                rows.push(json!({
                    "variant": key, "i": i, "click_ms": click_ev, "frontmost": front,
                    "pill_app_active": app_active, "pill_became_key": became_key,
                    "activations": activations, "focus_moved_by_click": moved,
                    "typed_ok": typed_ok,
                }));
            }
        }
    }
    close_textedit(app, &te);
    let summary: serde_json::Map<String, Value> = tally
        .into_iter()
        .map(|(k, t)| {
            (
                k,
                json!({"trials": t[0], "first_click_acted": t[1], "textedit_kept_front": t[2],
                       "typing_landed": t[3], "pill_never_active_or_key": t[4],
                       "focus_moved_by_click": t[5], "skipped_not_quiet": t[6]}),
            )
        })
        .collect();
    let v = json!({"variant": variant, "summary": summary, "rows": rows});
    write(out, &format!("click-{variant}"), &v);
    v
}

/// RT-14 / RC-03: hover with TextEdit frontmost, native tracking area vs CSS.
pub fn hover(app: &AppHandle, out: &Path, n: usize) -> Value {
    let Some(te) = open_textedit(out, "hover") else {
        return json!({"error": "TextEdit did not open"});
    };
    let Some((_, b, _)) = pill_bounds(app) else {
        return json!({"error": "pill not on screen"});
    };
    let inside = pt(&b, 100.0, 52.0);
    let away = pt(&b, 100.0, -250.0);
    let mut rows = Vec::new();
    let (mut native_ok, mut css_ok, mut painted_ok) = (0, 0, 0);
    let mut lat = Vec::new();
    for i in 0..n {
        cg::move_mouse(away);
        sleep_ms(500);
        let t0 = now_ms();
        // A short glide in, like a hand: 5 moves over ~40 ms.
        for k in 1..=5 {
            let f = k as f64 / 5.0;
            cg::move_mouse(CGPoint {
                x: away.x + (inside.x - away.x) * f,
                y: away.y + (inside.y - away.y) * f,
            });
            sleep_ms(8);
        }
        sleep_ms(600);
        let evs = events_since(t0);
        let first = |k: &str| evs.iter().find(|e| e.kind == k).map(|e| e.t_ms - t0);
        let (ne, ce, tp) = (
            first("native_enter"),
            first("css_enter"),
            first("tip_painted"),
        );
        if i == 0 || i == n - 1 {
            let f = out.join(format!("hover-{i}.png"));
            let r = format!(
                "{},{},{},{}",
                b.origin.x, b.origin.y, b.size.width, b.size.height
            );
            let _ = Command::new("screencapture")
                .args(["-x", "-o", "-R", &r])
                .arg(&f)
                .status();
        }
        let front = pill::info(app)
            .map(|i| i.frontmost_bundle)
            .unwrap_or_default();
        native_ok += ne.is_some() as usize;
        css_ok += ce.is_some() as usize;
        if let Some(t) = tp {
            if t <= 200.0 {
                painted_ok += 1;
            }
            lat.push(t);
        }
        rows.push(json!({"i": i, "native_enter_ms": ne, "css_enter_ms": ce,
                         "tip_painted_ms": tp, "frontmost": front}));
    }
    cg::move_mouse(away);
    close_textedit(app, &te);
    lat.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    let pct = |p: f64| -> Option<f64> {
        if lat.is_empty() {
            None
        } else {
            Some(lat[((lat.len() - 1) as f64 * p).round() as usize])
        }
    };
    let v = json!({"summary": {"n": n, "native_enter": native_ok, "css_enter": css_ok,
        "tip_painted_within_200ms": painted_ok, "tip_p50_ms": pct(0.5),
        "tip_p95_ms": pct(0.95), "tip_max_ms": pct(1.0)}, "rows": rows});
    write(out, "hover", &v);
    v
}

fn pill_visible_check(app: &AppHandle) -> Value {
    let inf = pill::info(app);
    let bounds = pill_bounds(app);
    let displays = cg::displays();
    let on_screen = bounds.as_ref().map(|b| b.2).unwrap_or(false);
    let inside = bounds
        .as_ref()
        .map(|(_, r, _)| displays.iter().any(|(_, d)| d.contains_rect(r)))
        .unwrap_or(false);
    json!({"on_screen_list": on_screen, "inside_a_display": inside,
           "on_active_space": inf.as_ref().map(|i| i.on_active_space),
           "frontmost": inf.as_ref().map(|i| i.frontmost_bundle.clone()),
           "bounds": bounds.map(|b| b.1)})
}

/// Space switch (⌃→/⌃←) and full-screen TextEdit, without and with re-show.
pub fn spaces(app: &AppHandle, out: &Path, n: usize) -> Value {
    use std::sync::atomic::Ordering;
    let mut results = serde_json::Map::new();
    for reshow in [false, true] {
        pill::RESHOW_ON_SPACE.store(reshow, Ordering::SeqCst);
        let tag = if reshow { "reshow" } else { "plain" };
        // Desktop switches.
        let mut rows = Vec::new();
        let mut ok = 0;
        let mut switched = 0;
        for i in 0..n {
            for (dir, code) in [("right", 124u16), ("left", 123u16)] {
                let t0 = now_ms();
                cg::key(code, cg::kCGEventFlagMaskControl);
                sleep_ms(1500);
                let changed = events_since(t0).iter().any(|e| e.kind == "space_changed");
                let chk = pill_visible_check(app);
                let good = chk["on_screen_list"] == true && chk["inside_a_display"] == true;
                switched += changed as usize;
                ok += (changed && good) as usize;
                if i == 0 {
                    let f = out.join(format!("space-{tag}-{dir}.png"));
                    let _ = Command::new("screencapture")
                        .args(["-x", "-o"])
                        .arg(&f)
                        .status();
                }
                rows.push(json!({"i": i, "dir": dir, "space_changed": changed, "check": chk}));
            }
        }
        // Full-screen TextEdit.
        let mut fs_rows = Vec::new();
        let mut fs_ok = 0;
        if let Some(te) = open_textedit(out, &format!("fullscreen-{tag}")) {
            for i in 0..n {
                let t0 = now_ms();
                if !te.ours_in_front(app) {
                    te.refocus();
                }
                // Only ever full-screen our own document window.
                let set = te.ours_in_front(app) && cg::ax_set_fullscreen(te.pid, true);
                sleep_ms(2500);
                let changed = events_since(t0).iter().any(|e| e.kind == "space_changed");
                let chk = pill_visible_check(app);
                let good = chk["on_screen_list"] == true && chk["inside_a_display"] == true;
                fs_ok += (set && changed && good) as usize;
                if i == 0 {
                    let f = out.join(format!("fullscreen-{tag}.png"));
                    let _ = Command::new("screencapture")
                        .args(["-x", "-o"])
                        .arg(&f)
                        .status();
                }
                fs_rows
                    .push(json!({"i": i, "ax_set": set, "space_changed": changed, "check": chk}));
                cg::ax_set_fullscreen(te.pid, false);
                sleep_ms(2500);
            }
            close_textedit(app, &te);
        }
        results.insert(
            tag.into(),
            json!({"switches": 2 * n, "space_changed": switched, "pill_shown_after_switch": ok,
                   "fullscreen_n": n, "fullscreen_ok": fs_ok,
                   "rows": rows, "fullscreen_rows": fs_rows}),
        );
    }
    let v = Value::Object(results);
    write(out, "spaces", &v);
    v
}

/// Count magenta (#FF00FF) pixels of the pill's waveform in a PNG capture.
pub fn magenta_in(png_path: &Path, rect_pts: &CGRect, scale: f64) -> Option<usize> {
    let file = std::fs::File::open(png_path).ok()?;
    let dec = png::Decoder::new(std::io::BufReader::new(file));
    let mut reader = dec.read_info().ok()?;
    let mut buf = vec![0; reader.output_buffer_size()?];
    let info = reader.next_frame(&mut buf).ok()?;
    let ch = match info.color_type {
        png::ColorType::Rgba => 4,
        png::ColorType::Rgb => 3,
        _ => return None,
    };
    if info.bit_depth != png::BitDepth::Eight {
        return None;
    }
    let (w, h) = (info.width as i64, info.height as i64);
    let x0 = (rect_pts.origin.x * scale) as i64;
    let y0 = (rect_pts.origin.y * scale) as i64;
    let x1 = ((rect_pts.origin.x + rect_pts.size.width) * scale) as i64;
    let y1 = ((rect_pts.origin.y + rect_pts.size.height) * scale) as i64;
    let mut count = 0;
    for y in y0.max(0)..y1.min(h) {
        for x in x0.max(0)..x1.min(w) {
            let i = ((y * w + x) * ch) as usize;
            let (r, g, b) = (buf[i], buf[i + 1], buf[i + 2]);
            if r > 200 && g < 80 && b > 200 {
                count += 1;
            }
        }
    }
    Some(count)
}

fn tools_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("target/tools")
}

/// RT-16: does `set_content_protected` hide the pill from three recorders?
pub fn capture(app: &AppHandle, out: &Path, protected: bool) -> Value {
    sleep_ms(800);
    let Some((_, b, _)) = pill_bounds(app) else {
        return json!({"error": "pill not on screen"});
    };
    let tag = if protected { "protected" } else { "control" };
    let scale = pill::info(app).map(|i| i.backing_scale).unwrap_or(2.0);
    let mut rows = serde_json::Map::new();
    // 1. screencapture still (Screenshot app engine).
    let still = out.join(format!("cap-{tag}-screencapture.png"));
    let _ = Command::new("screencapture")
        .args(["-x", "-o"])
        .arg(&still)
        .status();
    rows.insert(
        "screencapture_still".into(),
        json!(magenta_in(&still, &b, scale)),
    );
    // 2. screencapture -v video (the Screenshot / QuickTime recording engine).
    let mov = out.join(format!("cap-{tag}-video.mov"));
    let _ = std::fs::remove_file(&mov);
    let _ = Command::new("screencapture")
        .args(["-x", "-v", "-V", "3"])
        .arg(&mov)
        .status();
    let frame = out.join(format!("cap-{tag}-video-frame.png"));
    let _ = Command::new("ffmpeg")
        .args(["-loglevel", "error", "-y", "-ss", "1.5", "-i"])
        .arg(&mov)
        .args(["-frames:v", "1"])
        .arg(&frame)
        .status();
    rows.insert(
        "screencapture_video".into(),
        json!(magenta_in(&frame, &b, scale)),
    );
    // 3. ScreenCaptureKit SCStream (the API Zoom, OBS and Teams use on 14+).
    let sck = out.join(format!("cap-{tag}-sck.png"));
    let _ = Command::new(tools_dir().join("sck-grab"))
        .arg(&sck)
        .status();
    rows.insert(
        "screencapturekit_stream".into(),
        json!(magenta_in(&sck, &b, scale)),
    );
    let _ = std::fs::remove_file(&mov);
    let v = json!({"protected": protected, "pill_bounds": b, "magenta_pixels": rows});
    write(out, &format!("capture-{tag}"), &v);
    v
}

fn placement_check(app: &AppHandle) -> Value {
    let bounds = pill_bounds(app).map(|b| b.1);
    let inf = pill::info(app);
    let cur = cg::cursor();
    let displays = cg::displays();
    let under = displays.iter().find(|(_, d)| d.contains_point(cur));
    let (ok, why) = match (bounds, under) {
        (Some(r), Some((_, d))) => {
            let centred = (r.mid_x() - d.mid_x()).abs() <= 1.0;
            let inside = d.contains_rect(&r);
            let low = r.max_y() > d.max_y() - 160.0;
            (
                centred && inside && low,
                format!("centred={centred} inside={inside} bottom={low}"),
            )
        }
        _ => (false, "no bounds or display".into()),
    };
    json!({"ok": ok, "why": why, "cursor": cur, "bounds": bounds,
           "display_under_cursor": under.map(|u| u.1), "displays": displays.len(),
           "backing_scale": inf.as_ref().map(|i| i.backing_scale),
           "screen_scale": inf.as_ref().map(|i| i.screen_scale)})
}

fn wait_placed(app: &AppHandle, ms: u64) -> (Value, Option<f64>) {
    let t0 = now_ms();
    let mut last = placement_check(app);
    while now_ms() - t0 < ms as f64 {
        last = placement_check(app);
        if last["ok"] == true {
            return (last, Some(now_ms() - t0));
        }
        sleep_ms(100);
    }
    (last, None)
}

/// Virtual 1× display plug / unplug (CGVirtualDisplay helper), display sleep
/// and wake, and a temporary 1× mode on the built-in display.
pub fn displays(app: &AppHandle, out: &Path, n: usize) -> Value {
    let main = cg::displays()
        .into_iter()
        .find(|(id, _)| *id == cg::main_display())
        .map(|d| d.1)
        .unwrap_or_default();
    let home = CGPoint {
        x: main.mid_x(),
        y: main.origin.y + main.size.height / 2.0,
    };
    let mut vrows = Vec::new();
    for i in 0..n {
        let mut child = match Command::new(tools_dir().join("vdisplay"))
            .args(["plug", "1920", "1080", "1", "600"])
            .stdout(std::process::Stdio::piped())
            .spawn()
        {
            Ok(c) => c,
            Err(e) => return json!({"error": format!("vdisplay: {e}")}),
        };
        sleep_ms(3000);
        let ds = cg::displays();
        let ext = ds
            .iter()
            .find(|(id, _)| *id != cg::main_display())
            .map(|d| d.1);
        let mut row = json!({"i": i, "displays_after_plug": ds.len()});
        if let Some(e) = ext {
            cg::warp(CGPoint {
                x: e.mid_x(),
                y: e.origin.y + e.size.height / 2.0,
            });
            let (chk, t) = wait_placed(app, 3000);
            row["on_external"] = chk;
            row["follow_ms"] = json!(t);
            if i == 0 {
                let _ = Command::new("screencapture")
                    .args(["-x", "-o"])
                    .arg(out.join("display-ext-a.png"))
                    .arg(out.join("display-ext-b.png"))
                    .status();
            }
            cg::warp(home);
            let (chk, t) = wait_placed(app, 3000);
            row["back_on_builtin"] = chk;
            row["back_ms"] = json!(t);
            // Unplug while the pill sits on the external display.
            cg::warp(CGPoint {
                x: e.mid_x(),
                y: e.origin.y + e.size.height / 2.0,
            });
            let _ = wait_placed(app, 3000);
        }
        let t0 = now_ms();
        let _ = child.kill();
        let _ = child.wait();
        let (chk, t) = wait_placed(app, 5000);
        row["after_unplug"] = chk;
        // From killing the helper (the "unplug") to the pill placed on the built-in display.
        row["after_unplug_ms"] = json!(t);
        row["screen_param_events"] = json!(events_since(t0)
            .iter()
            .filter(|e| e.kind == "screen_params_changed")
            .count());
        vrows.push(row);
        cg::warp(home);
        sleep_ms(1000);
    }
    // Display sleep and wake (screen lock delay on this Mac is 300 s).
    let mut srows = Vec::new();
    for i in 0..n {
        let _ = Command::new("pmset").arg("displaysleepnow").status();
        sleep_ms(6000);
        let _ = Command::new("caffeinate").args(["-u", "-t", "2"]).status();
        sleep_ms(3000);
        let (chk, _) = wait_placed(app, 3000);
        let vis = pill_visible_check(app);
        srows.push(json!({"i": i, "placement": chk, "visible": vis}));
    }
    // A temporary 1× mode on the built-in display (reverts when the helper exits).
    let mut mrows = Vec::new();
    for i in 0..n {
        let mut child = match Command::new(tools_dir().join("vdisplay"))
            .args(["mode1x", "15"])
            .spawn()
        {
            Ok(c) => c,
            Err(e) => return json!({"error": format!("vdisplay mode: {e}")}),
        };
        sleep_ms(4000);
        let inf = pill::info(app);
        let (chk, _) = wait_placed(app, 3000);
        let _ = child.kill();
        let _ = child.wait();
        sleep_ms(4000);
        let (back, _) = wait_placed(app, 3000);
        mrows.push(json!({"i": i, "scale_in_1x": inf.map(|i| i.backing_scale),
                          "in_1x": chk, "after_revert": back}));
    }
    let v = json!({"virtual_display": vrows, "display_sleep": srows, "mode_1x": mrows});
    write(out, "displays", &v);
    v
}

extern "C" {
    fn responsibility_get_pid_responsible_for_pid(pid: i32) -> i32;
}

#[derive(serde::Serialize)]
struct Proc {
    pid: i32,
    rss_kb: u64,
    cpu_s: f64,
    comm: String,
}

fn parse_time(s: &str) -> f64 {
    // [[dd-]hh:]mm:ss.cc
    let mut total = 0.0;
    let (days, rest) = match s.split_once('-') {
        Some((d, r)) => (d.parse::<f64>().unwrap_or(0.0), r),
        None => (0.0, s),
    };
    for part in rest.split(':') {
        total = total * 60.0 + part.parse::<f64>().unwrap_or(0.0);
    }
    total + days * 86400.0
}

/// WebKit process ids alive before the pill's webview existed (set from
/// `main` at startup). A bare binary run from a terminal has no bundle, so
/// macOS makes the terminal responsible for its WebKit processes; ours are
/// the ones that appeared after startup with the same responsible process.
pub static WEBKIT_BEFORE: std::sync::Mutex<Vec<i32>> = std::sync::Mutex::new(Vec::new());

pub fn webkit_pids() -> Vec<i32> {
    sh("pgrep", &["-f", "com.apple.WebKit"])
        .lines()
        .filter_map(|l| l.trim().parse().ok())
        .collect()
}

fn our_processes() -> Vec<Proc> {
    let me = std::process::id() as i32;
    let my_resp = unsafe { responsibility_get_pid_responsible_for_pid(me) };
    let before = WEBKIT_BEFORE.lock().map(|b| b.clone()).unwrap_or_default();
    let ps = sh("ps", &["-A", "-o", "pid=,rss=,time=,comm="]);
    ps.lines()
        .filter_map(|l| {
            let mut it = l.split_whitespace();
            let pid: i32 = it.next()?.parse().ok()?;
            let rss: u64 = it.next()?.parse().ok()?;
            let time = it.next()?;
            let comm: String = it.collect::<Vec<_>>().join(" ");
            let resp = unsafe { responsibility_get_pid_responsible_for_pid(pid) };
            let ours_webkit = comm.contains("com.apple.WebKit")
                && !before.contains(&pid)
                && (resp == me || resp == my_resp);
            let mine = pid == me || resp == me || ours_webkit;
            mine.then(|| Proc {
                pid,
                rss_kb: rss,
                cpu_s: parse_time(time),
                comm,
            })
        })
        .collect()
}

/// RT-17: resting pill for `hours` with `takes` simulated takes; samples RSS
/// and CPU of this process and every WebKit process it is responsible for,
/// once a minute, to `idle.jsonl`.
pub fn idle(app: &AppHandle, out: &Path, hours: f64, takes: usize) -> Value {
    use std::io::Write;
    let total_s = hours * 3600.0;
    let take_every = total_s / takes.max(1) as f64;
    let mut f = match std::fs::File::create(out.join("idle.jsonl")) {
        Ok(f) => f,
        Err(e) => return json!({"error": e.to_string()}),
    };
    let start = std::time::Instant::now();
    let mut next_take = take_every / 2.0;
    let mut next_sample = 0.0;
    let mut done_takes = 0;
    let mut seed: u32 = 1;
    while start.elapsed().as_secs_f64() < total_s {
        let t = start.elapsed().as_secs_f64();
        if t >= next_sample {
            let procs = our_processes();
            let line = json!({"t_s": t, "takes": done_takes,
                "rss_total_kb": procs.iter().map(|p| p.rss_kb).sum::<u64>(),
                "cpu_total_s": procs.iter().map(|p| p.cpu_s).sum::<f64>(),
                "procs": procs});
            let _ = writeln!(f, "{line}");
            let _ = f.flush();
            next_sample += 60.0;
        }
        if t >= next_take && done_takes < takes {
            // Recording 5 s with 30 fps levels, processing 1 s, then resting.
            pill::emit_state(app, "recording");
            for _ in 0..150 {
                let lv: Vec<f32> = (0..16)
                    .map(|_| {
                        seed = seed.wrapping_mul(1_103_515_245).wrapping_add(12345);
                        (seed >> 16) as f32 / 65536.0
                    })
                    .collect();
                pill::emit_levels(app, &lv);
                sleep_ms(33);
            }
            pill::emit_state(app, "processing");
            sleep_ms(1000);
            pill::emit_state(app, "resting");
            pill::emit_levels(app, &[0.0; 16]);
            done_takes += 1;
            next_take += take_every;
            pill::clear_log();
        }
        sleep_ms(250);
    }
    let v = json!({"hours": hours, "takes": done_takes});
    write(out, "idle", &v);
    v
}
