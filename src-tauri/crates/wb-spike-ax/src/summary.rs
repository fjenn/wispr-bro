// SPDX-License-Identifier: GPL-3.0-or-later
//! `summary.json` and `summary.txt`: per app, pass/fail against each RS-04
//! criterion, plus the measurements behind them. Built only from `Scored`
//! rows (also stored in `events.jsonl`, so `--summarize DIR` can rebuild it).

use std::fmt::Write as _;
use std::path::Path;

use serde::Serialize;
use serde_json::Value;

use crate::apps::NOT_TESTED;
use crate::score::{self, AppScore, Scored, EDITABLE_MIN, P95_MAX_MS, SELECTION_MIN, URL_MIN};

#[derive(Clone, Debug, Default, Serialize)]
pub struct RunInfo {
    pub tool: String,
    pub git_sha: String,
    pub started: String,
    pub mode: String,
    pub machine: Value,
    pub accessibility: bool,
    /// Apps skipped by the owner, not installed, or stopped early.
    pub skipped: Vec<(String, String)>,
    pub activation: Vec<Value>,
    pub interrupted: bool,
}

pub fn read_rows(events: &Path) -> (Vec<Scored>, Option<RunInfo>) {
    let text = std::fs::read_to_string(events).unwrap_or_default();
    let mut rows = Vec::new();
    let mut info: Option<RunInfo> = None;
    for line in text.lines() {
        let Ok(v) = serde_json::from_str::<Value>(line) else {
            continue;
        };
        match v["kind"].as_str() {
            Some("event") => {
                if let Ok(r) = serde_json::from_value::<Scored>(v["scored"].clone()) {
                    rows.push(r);
                }
            }
            Some("run") => {
                let mut i = RunInfo {
                    tool: v["tool"].as_str().unwrap_or_default().into(),
                    git_sha: v["git_sha"].as_str().unwrap_or_default().into(),
                    started: v["started"].as_str().unwrap_or_default().into(),
                    mode: v["mode"].as_str().unwrap_or_default().into(),
                    machine: v["machine"].clone(),
                    accessibility: v["accessibility"].as_bool().unwrap_or(false),
                    ..RunInfo::default()
                };
                i.interrupted = true;
                info = Some(i);
            }
            Some("skip") => {
                if let Some(i) = info.as_mut() {
                    i.skipped.push((
                        v["app"].as_str().unwrap_or_default().into(),
                        v["why"].as_str().unwrap_or_default().into(),
                    ));
                }
            }
            Some("activation") => {
                if let Some(i) = info.as_mut() {
                    i.activation.push(v.clone());
                }
            }
            Some("end") => {
                if let Some(i) = info.as_mut() {
                    i.interrupted = false;
                }
            }
            _ => {}
        }
    }
    (rows, info)
}

fn ms(v: Option<f64>) -> String {
    v.map_or("–".into(), |x| format!("{x:.1} ms"))
}

pub fn render_txt(info: &RunInfo, scores: &[AppScore]) -> String {
    let mut s = String::new();
    let _ = writeln!(s, "RS-04 Accessibility (AX) matrix: summary");
    let _ = writeln!(
        s,
        "{} at {} | {} | started {}{}",
        info.tool,
        info.git_sha,
        info.mode,
        info.started,
        if info.interrupted {
            " | STOPPED EARLY"
        } else {
            ""
        }
    );
    let m = &info.machine;
    let _ = writeln!(
        s,
        "Mac: {} ({}), macOS {} ({}); Accessibility permission: {}",
        m["model"].as_str().unwrap_or("?"),
        m["cpu"].as_str().unwrap_or("?"),
        m["macos_version"].as_str().unwrap_or("?"),
        m["macos_build"].as_str().unwrap_or("?"),
        if info.accessibility { "on" } else { "OFF" }
    );
    let _ = writeln!(
        s,
        "\nCriteria (R5 §7 RS-04): editable verdict right in ≥ {:.0} % of text fields \
         (fail-open counts), 0 false \"no text field\" cards, selected text readable \
         in ≥ {:.0} %, URL host readable (browsers, ≥ {:.0} % of page events), p95 AX \
         call ≤ {P95_MAX_MS:.0} ms (0.25 s timeout, lease on).",
        EDITABLE_MIN * 100.0,
        SELECTION_MIN * 100.0,
        URL_MIN * 100.0
    );
    let _ = writeln!(
        s,
        "\n{:<9} {:<9} {:>6} {:<17} {:>5} {:<17} {:<17} {:>9}  overall",
        "app", "family", "events", "editable", "cards", "selection", "url", "p95 call"
    );
    for a in scores {
        let c = &a.criteria;
        let _ = writeln!(
            s,
            "{:<9} {:<9} {:>6} {:<17} {:>5} {:<17} {:<17} {:>9}  {}",
            a.app,
            a.family,
            a.events,
            format!("{} {}", c.editable.word(), a.editable.show()),
            format!(
                "{}{}",
                a.false_cards,
                if c.no_false_card == score::Check::Fail {
                    "!"
                } else {
                    ""
                }
            ),
            format!("{} {}", c.selection.word(), a.selection.show()),
            format!(
                "{} {}",
                c.url.word(),
                if c.url == score::Check::NotApplicable {
                    String::new()
                } else {
                    a.url.show()
                }
            ),
            ms(a.p95_ms),
            a.overall.word()
        );
    }
    let _ = writeln!(s, "\nDetails per app:");
    for a in scores {
        let _ = writeln!(s, "\n  {} ({}), {} focus events", a.app, a.family, a.events);
        let _ = writeln!(
            s,
            "    text fields: pastes {} (firm \"editable\" {}, fail-open {}), false cards {}, \
             wrongly refused as password {}",
            a.editable.show(),
            a.editable_exact,
            a.editable.ok.saturating_sub(a.editable_exact),
            a.false_cards,
            a.false_refusals
        );
        if a.editable_without_lease.n > 0 {
            let _ = writeln!(
                s,
                "    without our AXManualAccessibility lease: pastes {}, false cards {}; \
                 selection {}; URL {}",
                a.editable_without_lease.show(),
                a.false_cards_without_lease,
                a.selection_without_lease.show(),
                a.url_without_lease.show()
            );
        }
        let _ = writeln!(
            s,
            "    password fields refused {}; non-text places got the card {}",
            a.password.show(),
            a.not_text_card.show()
        );
        let _ = writeln!(
            s,
            "    selection readable {}; exact text (tool's own fields) {}; Edit › Copy enabled {}",
            a.selection.show(),
            a.selection_text_exact.show(),
            a.edit_copy_available.show()
        );
        let _ = writeln!(
            s,
            "    AXSelectedText write: worked {}, put back {}",
            a.set_selected_text.show(),
            a.set_reverted.show()
        );
        let _ = writeln!(
            s,
            "    URL readable {}; hosts: {}",
            a.url.show(),
            if a.hosts.is_empty() {
                "–".to_string()
            } else {
                a.hosts.join(", ")
            }
        );
        let _ = writeln!(
            s,
            "    AX calls ({}): p50 {}, p95 {}, max {}; whole read p95 {}; at 0.1 s: p95 {}, \
             answer changed {}×, timeouts {} (vs {} at 0.25 s)",
            a.calls,
            ms(a.p50_ms),
            ms(a.p95_ms),
            ms(a.max_ms),
            ms(a.probe_p95_ms),
            ms(a.p95_ms_fast),
            a.delivery_changed_at_fast,
            a.cannot_complete_fast,
            a.cannot_complete
        );
        let _ = writeln!(
            s,
            "    frontmostApplication = AXFocusedApplication {}; AXFocusedApplication failed {}×; \
             window title readable {}; caret/selection bounds {}",
            a.front_agree.show(),
            a.ax_focused_app_failed,
            a.title_readable.show(),
            a.bounds.show()
        );
    }
    if !info.activation.is_empty() {
        let _ = writeln!(
            s,
            "\nApp switch freshness (open -a → first seen as frontmost):"
        );
        for v in &info.activation {
            let _ = writeln!(
                s,
                "  {:<9} NSWorkspace {}, AXFocusedApplication {}",
                v["app"].as_str().unwrap_or("?"),
                ms(v["workspace_ms"].as_f64()),
                if v["ax_failed"].as_bool() == Some(true) {
                    "failed".to_string()
                } else {
                    ms(v["ax_ms"].as_f64())
                }
            );
        }
    }
    if !info.skipped.is_empty() {
        let _ = writeln!(s, "\nSkipped:");
        for (app, why) in &info.skipped {
            let _ = writeln!(s, "  {app}: {why}");
        }
    }
    let _ = writeln!(
        s,
        "\nNot tested (not installed on this Mac): {}.",
        NOT_TESTED.join(", ")
    );
    s
}

pub fn write(dir: &Path, info: &RunInfo, rows: &[Scored]) -> std::io::Result<String> {
    let scores = score::score_all(rows);
    let json = serde_json::json!({
        "run": info,
        "apps": scores,
        "not_tested": NOT_TESTED,
        "thresholds": {
            "editable_min": EDITABLE_MIN,
            "selection_min": SELECTION_MIN,
            "url_min": URL_MIN,
            "p95_max_ms": P95_MAX_MS,
        },
    });
    std::fs::write(
        dir.join("summary.json"),
        serde_json::to_string_pretty(&json).unwrap_or_default(),
    )?;
    let txt = render_txt(info, &scores);
    std::fs::write(dir.join("summary.txt"), &txt)?;
    Ok(txt)
}

/// `--summarize DIR`: rebuild both files from `events.jsonl`.
pub fn summarize_dir(dir: &Path) -> i32 {
    let (rows, info) = read_rows(&dir.join("events.jsonl"));
    let info = info.unwrap_or_default();
    match write(dir, &info, &rows) {
        Ok(txt) => {
            println!("{txt}");
            0
        }
        Err(e) => {
            eprintln!("could not write the summary in {}: {e}", dir.display());
            1
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::apps::Expect;

    #[test]
    fn rebuilds_from_events() {
        let dir = std::env::temp_dir().join(format!("wb-spike-ax-sum-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let row = Scored {
            app: "textedit".into(),
            family: "native".into(),
            expect: Some(Expect::Text),
            delivery: "paste".into(),
            verdict: "editable".into(),
            calls_ms: vec![1.0],
            ..Scored::default()
        };
        let lines = [
            serde_json::json!({"kind": "run", "tool": "wb-spike-ax 0.1.0", "machine": {}, "accessibility": true}),
            serde_json::json!({"kind": "event", "scored": row}),
            serde_json::json!({"kind": "skip", "app": "gmail", "why": "owner not signed in"}),
        ];
        let text: Vec<String> = lines.iter().map(|l| l.to_string()).collect();
        std::fs::write(dir.join("events.jsonl"), text.join("\n")).unwrap();
        assert_eq!(summarize_dir(&dir), 0);
        let txt = std::fs::read_to_string(dir.join("summary.txt")).unwrap();
        assert!(txt.contains("textedit"));
        assert!(txt.contains("STOPPED EARLY"), "no end line");
        assert!(txt.contains("gmail: owner not signed in"));
        let json: Value =
            serde_json::from_str(&std::fs::read_to_string(dir.join("summary.json")).unwrap())
                .unwrap();
        assert_eq!(json["apps"][0]["criteria"]["editable"], "pass");
        std::fs::remove_dir_all(&dir).unwrap();
    }
}
