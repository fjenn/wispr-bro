// SPDX-License-Identifier: GPL-3.0-or-later
//
// wb-spike-small: the RS-11 small checks (R5 §7), run unattended.
//
//   wb-spike-small mouse        --out DIR   middle/side buttons through Handy's
//                                           shortcut-recorder listener
//   wb-spike-small option       --out DIR   ⌥1 ⌥2 ⌥O ⌥S ⌥M characters, bound or not
//   wb-spike-small mic-loss     --out DIR   the chosen mic disappears mid-take
//   wb-spike-small mic-default  --out DIR   the system default mic disappears mid-take
//   wb-spike-small perm         --out DIR   Microphone permission revoked mid-take
//                                           (runs `perm-take` inside a test .app)
//
// Results: DIR/<check>.json. Keys are typed only into the tool's own TextEdit
// document, and only while it is frontmost and focused.

mod ffi;
mod mic;
mod prompt;

use std::path::{Path, PathBuf};
use std::process::Command;
use std::str::FromStr;
use std::time::{Duration, Instant};

use handy_keys::{BlockingHotkeys, Hotkey, HotkeyManager, HotkeyState, Key, KeyboardListener};
use serde_json::{json, Value};

pub fn sleep_ms(ms: u64) {
    std::thread::sleep(Duration::from_millis(ms));
}

fn arg_value(args: &[String], name: &str) -> Option<String> {
    args.iter()
        .position(|a| a == name)
        .and_then(|i| args.get(i + 1).cloned())
}

pub fn write(out: &Path, name: &str, v: &Value) {
    let _ = std::fs::write(
        out.join(format!("{name}.json")),
        serde_json::to_string_pretty(v).unwrap_or_default(),
    );
}

fn pid_of(name: &str) -> Option<i32> {
    let o = Command::new("pgrep").args(["-x", name]).output().ok()?;
    String::from_utf8_lossy(&o.stdout)
        .lines()
        .next()
        .and_then(|l| l.trim().parse().ok())
}

struct Doc {
    pid: i32,
    file: PathBuf,
    title: String,
    launched: bool,
}

impl Doc {
    fn open(out: &Path, name: &str) -> Option<Doc> {
        let launched = pid_of("TextEdit").is_none();
        let file = out.join(format!("{name}.txt"));
        let _ = std::fs::write(&file, "");
        let title = format!("{name}.txt");
        let _ = Command::new("open")
            .args(["-a", "TextEdit"])
            .arg(&file)
            .status();
        for _ in 0..50 {
            sleep_ms(200);
            if let Some(pid) = pid_of("TextEdit") {
                let d = Doc {
                    pid,
                    file: file.clone(),
                    title: title.clone(),
                    launched,
                };
                if d.ours() {
                    sleep_ms(400);
                    return Some(d);
                }
            }
        }
        None
    }

    /// TextEdit frontmost with our document focused (other agents may be
    /// driving other apps on this Mac at the same time).
    fn ours(&self) -> bool {
        ffi::ax_frontmost(self.pid)
            && ffi::ax_focused_window_title(self.pid)
                .map(|t| t.starts_with(&self.title))
                .unwrap_or(false)
            && ffi::ax_focused_value(self.pid).is_some()
    }

    /// Make sure our document is in front; true when it is.
    fn front(&self) -> bool {
        for _ in 0..5 {
            if self.ours() {
                return true;
            }
            let _ = Command::new("open")
                .args(["-a", "TextEdit"])
                .arg(&self.file)
                .status();
            sleep_ms(700);
        }
        self.ours()
    }

    fn text(&self) -> String {
        ffi::ax_focused_value(self.pid).unwrap_or_default()
    }

    fn close(&self) {
        if self.front() {
            ffi::chord(13, &[ffi::CMD]); // ⌘W (file-backed, autosaved)
            sleep_ms(500);
        }
        if self.launched {
            let _ = Command::new("kill").arg(self.pid.to_string()).status();
        }
    }
}

fn drain(l: &KeyboardListener) {
    while l.try_recv().is_some() {}
}

/// RC-07: does Handy's shortcut recorder path (a plain handy-keys
/// `KeyboardListener`, `shortcut/handy_keys.rs` `start_recording`) see the
/// middle and side buttons, and does a binding made from what it records fire?
fn mouse(out: &Path) -> Value {
    let Some(doc) = Doc::open(out, "rs11-mouse") else {
        return json!({"error": "TextEdit did not open"});
    };
    let listener = match KeyboardListener::new() {
        Ok(l) => l,
        Err(e) => return json!({"error": e.to_string()}),
    };
    sleep_ms(300);
    let buttons = [
        (2u32, Key::MouseMiddle),
        (3, Key::MouseX1),
        (4, Key::MouseX2),
    ];
    let mut rows = Vec::new();
    let mut recorded: Vec<(u32, String)> = Vec::new();
    let mut summary = serde_json::Map::new();
    for (button, want) in buttons {
        let (mut seen, mut seen_opt, mut trials, mut trials_opt) = (0, 0, 0, 0);
        let mut strings = std::collections::BTreeSet::new();
        for i in 0..15 {
            let with_opt = i >= 10;
            if !doc.front() {
                rows.push(json!({"button": button, "i": i, "skipped": "TextEdit not in front"}));
                continue;
            }
            let Some(p) = ffi::ax_focused_window_center(doc.pid) else {
                continue;
            };
            drain(&listener);
            if with_opt {
                ffi::chord_mouse(button, p, &[ffi::OPT]);
            } else {
                ffi::chord_mouse(button, p, &[]);
            }
            sleep_ms(150);
            let mut evs = Vec::new();
            while let Some(e) = listener.try_recv() {
                evs.push(e);
            }
            let hit = evs.iter().find(|e| e.key == Some(want) && e.is_key_down);
            let s = hit
                .and_then(|e| e.as_hotkey().ok())
                .map(|h| h.to_handy_string());
            if let Some(s) = &s {
                strings.insert(s.clone());
            }
            if with_opt {
                trials_opt += 1;
                seen_opt += hit.is_some() as usize;
            } else {
                trials += 1;
                seen += hit.is_some() as usize;
            }
            rows.push(json!({"button": button, "i": i, "with_opt": with_opt,
                "captured": hit.is_some(), "hotkey_string": s,
                "events": evs.iter().map(|e| format!("{e:?}")).collect::<Vec<_>>()}));
        }
        if let Some(s) = strings.iter().find(|s| !s.contains("option")) {
            recorded.push((button, s.clone()));
        }
        summary.insert(
            format!("button{button}"),
            json!({"plain": format!("{seen}/{trials}"), "with_opt": format!("{seen_opt}/{trials_opt}"),
                   "recorded_as": strings.into_iter().collect::<Vec<_>>()}),
        );
    }
    drop(listener);
    // Bind what was recorded and press it again: the binding must fire.
    let mut binding = serde_json::Map::new();
    if let Ok(mgr) = HotkeyManager::new() {
        for (button, s) in &recorded {
            let parsed = Hotkey::from_str(s);
            let id = parsed.as_ref().ok().and_then(|h| mgr.register(*h).ok());
            let mut fired = 0;
            for _ in 0..5 {
                if !doc.front() {
                    continue;
                }
                let Some(p) = ffi::ax_focused_window_center(doc.pid) else {
                    continue;
                };
                while mgr.try_recv().is_some() {}
                ffi::chord_mouse(*button, p, &[]);
                sleep_ms(150);
                let mut got = false;
                while let Some(ev) = mgr.try_recv() {
                    if Some(ev.id) == id && ev.state == HotkeyState::Pressed {
                        got = true;
                    }
                }
                fired += got as usize;
            }
            if let Some(id) = id {
                let _ = mgr.unregister(id);
            }
            binding.insert(
                s.clone(),
                json!({"parsed": parsed.is_ok(), "registered": id.is_some(), "fired": format!("{fired}/5")}),
            );
        }
    }
    let text_after = doc.text();
    doc.close();
    let v = json!({"summary": summary, "binding_fires": binding,
                   "textedit_text_after": text_after, "rows": rows});
    write(out, "mouse", &v);
    v
}

/// RC-13: typing ¡ ™ ø ß µ with ⌥1 ⌥2 ⌥O ⌥S ⌥M bound or not.
fn option(out: &Path) -> Value {
    let keys: [(&str, u16, &str); 5] = [
        ("Opt+1", 18, "¡"),
        ("Opt+2", 19, "™"),
        ("Opt+O", 31, "ø"),
        ("Opt+S", 1, "ß"),
        ("Opt+M", 46, "µ"),
    ];
    let Some(doc) = Doc::open(out, "rs11-option") else {
        return json!({"error": "TextEdit did not open"});
    };
    let mut phases = serde_json::Map::new();
    let mut rows = Vec::new();
    for phase in [
        "off",
        "on_fork_tap",
        "on_handy_manager",
        "ctrl_opt_alternative",
    ] {
        // The fork's own tap, as R5 §4.8 runs it: FN always in the blocking set.
        let set: BlockingHotkeys = Default::default();
        if let Ok(mut s) = set.lock() {
            if let Ok(h) = Hotkey::from_str("Fn") {
                s.insert(h);
            }
            for (k, _, _) in keys {
                let name = match phase {
                    "on_fork_tap" => k.to_string(),
                    "ctrl_opt_alternative" => format!("Ctrl+{k}"),
                    _ => continue,
                };
                if let Ok(h) = Hotkey::from_str(&name) {
                    s.insert(h);
                }
            }
        }
        let tap = KeyboardListener::new_with_blocking(set.clone()).ok();
        // Handy's manager as shipped (blocking variant), with ⌥ keys
        // registered only in the "on_handy_manager" phase.
        let mgr = HotkeyManager::new_with_blocking().ok();
        if phase == "on_handy_manager" {
            if let Some(m) = &mgr {
                for (k, _, _) in keys {
                    if let Ok(h) = Hotkey::from_str(k) {
                        let _ = m.register(h);
                    }
                }
            }
        }
        sleep_ms(400);
        let (mut landed, mut trials, mut seen) = (0, 0, 0);
        for (k, code, ch) in keys {
            for i in 0..5 {
                if !doc.front() {
                    rows.push(json!({"phase": phase, "key": k, "i": i, "skipped": true}));
                    continue;
                }
                if let Some(t) = &tap {
                    drain(t);
                }
                let before = doc.text();
                ffi::chord(code, &[ffi::OPT]);
                sleep_ms(200);
                let after = doc.text();
                let added: String = after
                    .chars()
                    .skip(before.chars().count())
                    .collect::<String>();
                let ok = added == ch;
                let mut tap_saw = false;
                if let Some(t) = &tap {
                    while let Some(e) = t.try_recv() {
                        if e.is_key_down && e.key.is_some() {
                            tap_saw = true;
                        }
                    }
                }
                trials += 1;
                landed += ok as usize;
                seen += tap_saw as usize;
                rows.push(json!({"phase": phase, "key": k, "i": i, "typed": added,
                                 "expected": ch, "landed": ok, "tap_saw_key": tap_saw}));
            }
        }
        // In the alternative phase, the ⌃⌥ binding itself must still be caught.
        let mut ctrl_opt = Value::Null;
        if phase == "ctrl_opt_alternative" && doc.front() {
            let before = doc.text();
            ffi::chord(18, &[ffi::CTRL, ffi::OPT]);
            sleep_ms(200);
            ctrl_opt = json!({"text_unchanged": doc.text() == before});
        }
        phases.insert(
            phase.into(),
            json!({"characters_landed": format!("{landed}/{trials}"),
                   "tap_saw_key": format!("{seen}/{trials}"), "ctrl_opt_1": ctrl_opt}),
        );
        drop(tap);
        drop(mgr);
        sleep_ms(300);
    }
    let text = doc.text();
    doc.close();
    let v = json!({"summary": phases, "text": text, "rows": rows});
    write(out, "option", &v);
    v
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let cmd = args.get(1).cloned().unwrap_or_default();
    let out = PathBuf::from(arg_value(&args, "--out").unwrap_or_else(|| ".".into()));
    let _ = std::fs::create_dir_all(&out);
    let t0 = Instant::now();
    let v = match cmd.as_str() {
        "mouse" => mouse(&out),
        "option" => option(&out),
        "mic-loss" => mic::loss(&out, false),
        "mic-default" => mic::loss(&out, true),
        "perm" => mic::perm_driver(&out, &args),
        "perm-take" => mic::perm_take(&out),
        _ => {
            eprintln!("usage: wb-spike-small mouse|option|mic-loss|mic-default|perm --out DIR");
            std::process::exit(2);
        }
    };
    let summary = v.get("summary").cloned().unwrap_or(v);
    println!("{cmd} ({:.0} s): {summary}", t0.elapsed().as_secs_f64());
}
