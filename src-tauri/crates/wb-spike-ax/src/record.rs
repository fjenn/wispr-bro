// SPDX-License-Identifier: GPL-3.0-or-later
//! One focus event: the probes run for it on the AX thread, and their
//! reduction to a `Scored` row.

use serde::Serialize;

use crate::apps::Expect;
use crate::ax::editable::Delivery;
use crate::ax::probe::{self, LeaseMode, Probe, ProbeOpts};
use crate::ax::thread::Ctx;
use crate::score::Scored;

/// Phases that count toward "p95 AX call": everything a dictation, a
/// transform or a context read would do. Not the write test, not the
/// Edit-menu walk (a one-off fallback check).
const LATENCY_PHASES: &[&str] = &[
    "core",
    "selection",
    "caret",
    "bounds",
    "url",
    "title",
    "lease",
];

#[derive(Clone, Debug, Serialize)]
pub struct Probes {
    pub primary: Probe,
    pub fast: Option<Probe>,
    pub without_lease: Option<Probe>,
    pub write: Option<Probe>,
}

pub struct Plan {
    pub allow_text: bool,
    pub write_test: bool,
    pub menu_copy: bool,
    /// Only the primary probe (the cold first look at an app).
    pub single: bool,
    pub lease_off_only: bool,
}

/// Runs the probes for one event (on the AX thread). Chromium/Electron:
/// without lease first (our lease switched off), then with it (0.25 s, then
/// 0.1 s). Others: 0.25 s then 0.1 s. The `[wb]` write test runs last, in
/// its own probe, only in scratch fields.
pub fn run(ctx: &mut Ctx, plan: &Plan) -> Probes {
    let web = probe::focused_family(ctx).is_some_and(|f| f.builds_tree_on_request());
    let opts = |timeout: f32, lease: LeaseMode| ProbeOpts {
        timeout,
        lease,
        allow_text: plan.allow_text,
        set_test: false,
        menu_copy: false,
    };
    if plan.single || plan.lease_off_only {
        let mode = if web {
            LeaseMode::Off
        } else {
            LeaseMode::NotNeeded
        };
        return Probes {
            primary: probe::probe(ctx, &opts(0.25, mode)),
            fast: None,
            without_lease: None,
            write: None,
        };
    }
    let without_lease = web.then(|| probe::probe(ctx, &opts(0.25, LeaseMode::Off)));
    let mode = if web {
        LeaseMode::On
    } else {
        LeaseMode::NotNeeded
    };
    let mut primary_opts = opts(0.25, mode);
    primary_opts.menu_copy = plan.menu_copy;
    let primary = probe::probe(ctx, &primary_opts);
    let fast = Some(probe::probe(ctx, &opts(0.1, mode)));
    let write = plan.write_test.then(|| {
        let mut o = opts(0.25, mode);
        o.set_test = true;
        probe::probe(ctx, &o)
    });
    Probes {
        primary,
        fast,
        without_lease,
        write,
    }
}

fn delivery_word(d: Delivery) -> String {
    serde_json::to_value(d)
        .ok()
        .and_then(|v| v.as_str().map(str::to_string))
        .unwrap_or_default()
}

fn verdict_word(p: &Probe) -> String {
    serde_json::to_value(p.verdict)
        .ok()
        .and_then(|v| v["verdict"].as_str().map(str::to_string))
        .unwrap_or_default()
}

/// Selected text readable through the chain, consistent with the range.
/// None when no selection was there to read (range known and empty).
pub fn sel_readable(p: &Probe) -> Option<bool> {
    if p.sel_range.is_some_and(|r| r[1] == 0) && p.selected.source.is_none() {
        return None;
    }
    Some(p.selected.source.is_some() && p.selected.matches_range != Some(false))
}

fn latency(p: &Probe) -> Vec<f64> {
    p.calls
        .iter()
        .filter(|c| LATENCY_PHASES.contains(&c.phase))
        .map(|c| c.ms)
        .collect()
}

/// Calls that hit the messaging timeout: kAXErrorCannotComplete after at
/// least 90 % of the timeout. (An instant CannotComplete is a refusal, not
/// a timeout: system-wide `AXFocusedApplication` does that on some setups.)
fn cannot_complete(p: &Probe) -> u32 {
    let floor = f64::from(p.timeout) * 1000.0 * 0.9;
    p.calls
        .iter()
        .filter(|c| c.err == Some("CannotComplete") && c.ms >= floor)
        .count() as u32
}

fn url_readable(p: &Probe) -> bool {
    p.url.kind == "http" || p.url.kind == "file"
}

pub struct Meta<'a> {
    pub app: &'a str,
    pub target: &'a str,
    pub expect: Option<Expect>,
    pub source: &'a str,
    pub web_page: bool,
    pub expected_selection: Option<&'a str>,
    pub front_pid: Option<i32>,
}

pub fn scored(m: &Meta, p: &Probes) -> Scored {
    let a = &p.primary;
    let set = p.write.as_ref().and_then(|w| w.set_test.clone());
    Scored {
        app: m.app.to_string(),
        family: serde_json::to_value(a.family)
            .ok()
            .and_then(|v| v.as_str().map(str::to_string))
            .unwrap_or_default(),
        target: m.target.to_string(),
        expect: m.expect,
        source: m.source.to_string(),
        delivery: delivery_word(a.delivery),
        verdict: verdict_word(a),
        role: a.facts.role.clone(),
        delivery_without_lease: p.without_lease.as_ref().map(|w| delivery_word(w.delivery)),
        delivery_fast: p.fast.as_ref().map(|f| delivery_word(f.delivery)),
        sel_readable: if m.expect == Some(Expect::Select) {
            sel_readable(a)
        } else {
            None
        },
        sel_source: a.selected.source.map(str::to_string),
        sel_readable_without_lease: if m.expect == Some(Expect::Select) {
            p.without_lease.as_ref().and_then(sel_readable)
        } else {
            None
        },
        sel_text_ok: m
            .expected_selection
            .map(|want| a.selected.text.as_deref() == Some(want)),
        edit_copy_enabled: a.edit_copy_enabled,
        web_page: m.web_page,
        url_kind: a.url.kind.to_string(),
        url_host: a.url.host.clone(),
        url_readable_without_lease: if m.web_page {
            p.without_lease.as_ref().map(url_readable)
        } else {
            None
        },
        calls_ms: latency(a),
        calls_ms_fast: p.fast.as_ref().map(latency).unwrap_or_default(),
        cannot_complete_fast: p.fast.as_ref().map(cannot_complete).unwrap_or(0),
        cannot_complete: cannot_complete(a),
        total_ms: a.total_ms,
        total_ms_fast: p.fast.as_ref().map(|f| f.total_ms),
        set_tried: set.is_some(),
        set_verified: set.as_ref().is_some_and(|s| s.verified),
        set_reverted: set.as_ref().is_some_and(|s| s.reverted),
        set_error: set.and_then(|s| s.write_error.map(str::to_string)),
        front_agree: match (m.front_pid, a.ax_app_pid) {
            (Some(f), Some(x)) => Some(f == x),
            _ => None,
        },
        ax_focused_app_ok: a.ax_app_pid.is_some(),
        has_element: a.facts.has_focused_element,
        title_readable: a.window_title_len.is_some(),
        bounds_ok: a.caret_bounds.is_some(),
        caret_window_ok: a.sel_range.map(|_| a.caret_window_units.is_some()),
    }
}
