// SPDX-License-Identifier: GPL-3.0-or-later
//! One paste, the way `fork::insert::deliver` will do it (R5 W-07):
//!
//! 1. wait for the hotkey's modifiers to be physically released;
//! 2. snapshot every item and type (skip policy applied);
//! 3. write the transcript (markers + lazy text) and register with the guard;
//! 4. post ⌘V (or type, for Chromium 155/156);
//! 5. wait for the target to read the text, then a quiet period, or for the
//!    deadline (1.5 s native, 7 s Chromium/Electron);
//! 6. restore only if the guard says so (same generation, `changeCount`
//!    unchanged), then read the clipboard back and compare byte for byte.

use serde::Serialize;
use std::time::{Duration, Instant};

use crate::clip::keys::{self, ModifierWait};
use crate::clip::markers::{self, ManagerView};
use crate::clip::nspb;
use crate::clip::restore::{
    evaluate, Finish, RestoreDecision, RestoreGuard, TargetClass, Timing, TxState, Wait,
};
use crate::clip::snapshot::{self, Fidelity, Skipped};
use crate::clip::target;
use crate::macos;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Method {
    CommandV,
    /// hex's Chromium 155/156 path.
    Typing,
    /// No key event at all (automated checks): only the clipboard work.
    None,
}

#[derive(Clone, Debug)]
pub struct TakeSpec {
    pub text: String,
    pub class: TargetClass,
    /// Wait for modifiers to be released first (always on in the product).
    pub wait_modifiers: bool,
    pub max_modifier_wait: Duration,
    pub method: Method,
    /// Paste only if this app is frontmost (never into the wrong window).
    pub expect_bundle: Option<String>,
    /// …and its focused window title contains this (the Terminal target,
    /// whose app is also the one running this tool).
    pub expect_window: Option<String>,
    pub lazy: bool,
    /// Automated checks: a child `pbpaste` reads the clipboard in place of
    /// a target app.
    pub simulate_reader: bool,
}

impl TakeSpec {
    pub fn new(text: &str, class: TargetClass) -> TakeSpec {
        TakeSpec {
            text: text.to_string(),
            class,
            wait_modifiers: true,
            max_modifier_wait: Duration::from_millis(3000),
            method: if target::has_clipboard_race(class) {
                Method::Typing
            } else {
                Method::CommandV
            },
            expect_bundle: None,
            expect_window: None,
            lazy: true,
            simulate_reader: false,
        }
    }
}

#[derive(Clone, Debug, Serialize)]
pub struct TakeReport {
    pub text: String,
    pub method: Method,
    pub v_keycode: Option<u16>,
    pub modifier_wait: Option<ModifierWait>,
    pub snapshot_ms: f64,
    pub snapshot_bytes: usize,
    pub skipped: Vec<Skipped>,
    pub unreadable_types: Vec<String>,
    /// What a clipboard manager saw while the transcript was on the clipboard.
    pub manager_view: Option<ManagerView>,
    /// Reads of our text, ms after the ⌘V (negative: before it).
    pub reads_ms: Vec<f64>,
    pub finish: Option<Finish>,
    pub decision: String,
    /// ⌘V → restore written.
    pub restore_after_ms: Option<f64>,
    pub restore_ms: Option<f64>,
    pub fidelity: Option<Fidelity>,
    /// What the simulated reader got (automated checks only).
    pub reader_output: Option<String>,
    /// ⌘V → the text was seen in the target (when the caller can check).
    pub landed_ms: Option<f64>,
    /// AppKit released our text provider during the wait.
    pub provider_released: bool,
    pub error: Option<String>,
}

fn ms(d: Duration) -> f64 {
    (d.as_secs_f64() * 1000.0 * 10.0).round() / 10.0
}

/// Runs one paste. `guard` persists across takes (back-to-back carry).
pub fn run(guard: &mut RestoreGuard, spec: &TakeSpec, timing: &Timing) -> TakeReport {
    run_probed(guard, spec, timing, &mut || false)
}

/// As `run`, with `landed`: whether the text is visible in the target. It is
/// called every 25 ms from the ⌘V until true (for at most 1.5 s), so the
/// report says how soon after the ⌘V the paste could be verified.
pub fn run_probed(
    guard: &mut RestoreGuard,
    spec: &TakeSpec,
    timing: &Timing,
    landed: &mut dyn FnMut() -> bool,
) -> TakeReport {
    let mut r = TakeReport {
        text: spec.text.clone(),
        method: spec.method,
        v_keycode: None,
        modifier_wait: None,
        snapshot_ms: 0.0,
        snapshot_bytes: 0,
        skipped: Vec::new(),
        unreadable_types: Vec::new(),
        manager_view: None,
        reads_ms: Vec::new(),
        finish: None,
        decision: String::new(),
        restore_after_ms: None,
        restore_ms: None,
        fidelity: None,
        reader_output: None,
        landed_ms: None,
        provider_released: false,
        error: None,
    };

    // 1. Modifiers up.
    if spec.wait_modifiers {
        r.modifier_wait = Some(keys::wait_for_modifiers_released(
            spec.max_modifier_wait,
            keys::flags_now,
            Instant::now,
            macos::pump,
        ));
    }
    if let Some(b) = &spec.expect_bundle {
        let front = macos::frontmost();
        let id = front.as_ref().map(|f| f.bundle_id.clone());
        if id.as_deref() != Some(b.as_str()) {
            r.error = Some(format!(
                "not pasted: {} was in front instead of {b}",
                id.unwrap_or_else(|| "nothing".into())
            ));
            r.decision = "not_pasted".into();
            return r;
        }
        if let (Some(w), Some(f)) = (&spec.expect_window, &front) {
            let title = macos::focus(f.pid).window_title.unwrap_or_default();
            if !title.contains(w.as_str()) {
                r.error = Some(format!(
                    "not pasted: the front window is {title:?}, not {w:?}"
                ));
                r.decision = "not_pasted".into();
                return r;
            }
        }
    }

    // 2. Snapshot.
    let t = Instant::now();
    let cc0 = nspb::change_count();
    let (full, unreadable) = nspb::read();
    if nspb::change_count() != cc0 {
        r.error = Some("the clipboard changed while it was being saved".into());
        r.decision = "not_pasted".into();
        return r;
    }
    let (keep, skipped) = snapshot::apply_policy(&full);
    r.snapshot_ms = ms(t.elapsed());
    r.snapshot_bytes = full.bytes();
    r.skipped = skipped;
    r.unreadable_types = unreadable;

    // 3. Transcript.
    let published = match nspb::publish_transcript(&spec.text, spec.lazy) {
        Ok(p) => p,
        Err(e) => {
            let _ = nspb::write(&keep);
            r.error = Some(e);
            r.decision = "write_failed".into();
            return r;
        }
    };
    let gen = guard.register(keep, cc0, published.change_count);
    r.manager_view = Some(markers::manager_view(&nspb::types()));

    // 4. Paste.
    let mut st = TxState::new(Instant::now());
    let injected = Instant::now();
    st.injected_at = Some(injected);
    let posted = match spec.method {
        Method::CommandV => keys::post_command_v().map(|c| {
            r.v_keycode = Some(c);
        }),
        Method::Typing => keys::type_text(&spec.text),
        Method::None => Ok(()),
    };
    if let Err(e) = posted {
        st.injection_failed = true;
        r.error = Some(e);
    }
    let mut reader = spec.simulate_reader.then(|| {
        std::process::Command::new("/usr/bin/pbpaste")
            .stdout(std::process::Stdio::piped())
            .spawn()
    });

    // 5. Wait.
    let checks = spec.method != Method::None;
    let mut last_probe = injected;
    let mut probe = |r: &mut TakeReport, last: &mut Instant| {
        if r.landed_ms.is_none() && last.elapsed() >= Duration::from_millis(25) {
            *last = Instant::now();
            if landed() {
                r.landed_ms = Some(ms(injected.elapsed()));
            }
        }
    };
    let finish = loop {
        macos::pump(Duration::from_millis(5));
        if checks {
            probe(&mut r, &mut last_probe);
        }
        st.receipts = published.reads();
        // Not `published.finished()`: AppKit also calls that once every
        // promised type has been provided (seen in RS-05), which is not a loss.
        st.ownership_lost = nspb::change_count() != published.change_count;
        let now = Instant::now();
        if crate::owner::interrupted() {
            break Finish::Deadline;
        }
        if spec.method == Method::Typing && now.duration_since(injected) >= timing.quiet {
            // Typing never reads the clipboard: restore after a short settle.
            break Finish::Deadline;
        }
        if let Wait::Finish(f) = evaluate(&st, timing, spec.class, now) {
            break f;
        }
    };
    r.finish = Some(finish);
    r.provider_released = published.finished();
    if let Some(Ok(child)) = reader.take() {
        // The child is blocked on our promise until the run loop serves it.
        let mut child = child;
        let end = Instant::now() + Duration::from_secs(3);
        while Instant::now() < end && matches!(child.try_wait(), Ok(None)) {
            macos::pump(Duration::from_millis(5));
        }
        let _ = child.kill();
        r.reader_output = child
            .wait_with_output()
            .ok()
            .map(|o| String::from_utf8_lossy(&o.stdout).into_owned());
    }
    r.reads_ms = published
        .reads()
        .iter()
        .map(|t| {
            if *t >= injected {
                ms(t.duration_since(injected))
            } else {
                -ms(injected.duration_since(*t))
            }
        })
        .collect();

    // 6. Restore.
    let decision = guard.decide(gen, nspb::change_count());
    r.decision = decision.label().into();
    if let RestoreDecision::Restore(snap) = decision {
        let t = Instant::now();
        r.restore_after_ms = Some(ms(t.duration_since(injected)));
        match nspb::write(&snap) {
            Ok(_) => {
                r.restore_ms = Some(ms(t.elapsed()));
                let (after, _) = nspb::read();
                r.fidelity = Some(snapshot::compare(&full, &after));
            }
            Err(e) => r.error = Some(e),
        }
    }
    drop(published);
    while checks
        && r.landed_ms.is_none()
        && injected.elapsed() < Duration::from_millis(1500)
        && !crate::owner::interrupted()
    {
        macos::pump(Duration::from_millis(5));
        probe(&mut r, &mut last_probe);
    }
    r
}
