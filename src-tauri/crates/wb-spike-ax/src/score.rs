// SPDX-License-Identifier: GPL-3.0-or-later
//! Scoring against the RS-04 pass criteria (R5 §7), per app:
//!
//! - editable verdict correct in ≥ 95 % of focused text fields (fail-open
//!   counts as pass) and 0 false "no text field" cards;
//! - selected text readable in ≥ 90 % via the chain (AX, text markers);
//! - URL host readable (browsers);
//! - p95 AX call ≤ 100 ms.
//!
//! Pure: works on `Scored` rows (one per focus event), which the session
//! writes into `events.jsonl` so `--summarize` can redo this afterwards.

use serde::{Deserialize, Serialize};

use crate::apps::Expect;

pub const EDITABLE_MIN: f64 = 0.95;
pub const SELECTION_MIN: f64 = 0.90;
pub const URL_MIN: f64 = 0.90;
pub const P95_MAX_MS: f64 = 100.0;

/// One focus event, reduced to what the criteria need.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Scored {
    pub app: String,
    pub family: String,
    pub target: String,
    pub expect: Option<Expect>,
    /// guided | test_page | auto | cold
    pub source: String,
    /// paste | paste_fail_open | card | refuse (lease on / not needed, 0.25 s)
    pub delivery: String,
    pub verdict: String,
    pub role: Option<String>,
    /// Same, with our lease switched off (Chromium/Electron only).
    pub delivery_without_lease: Option<String>,
    /// Same element at the 0.1 s timeout.
    pub delivery_fast: Option<String>,
    pub sel_readable: Option<bool>,
    pub sel_source: Option<String>,
    pub sel_readable_without_lease: Option<bool>,
    /// Test page only: the text read equals the text the page selected.
    pub sel_text_ok: Option<bool>,
    pub edit_copy_enabled: Option<bool>,
    /// The event is on a web page whose URL should be readable.
    pub web_page: bool,
    pub url_kind: String,
    pub url_host: Option<String>,
    pub url_readable_without_lease: Option<bool>,
    /// Every AX call of the 0.25 s probe except the write test.
    pub calls_ms: Vec<f64>,
    pub calls_ms_fast: Vec<f64>,
    /// Calls that ended in kAXErrorCannotComplete at 0.1 s / 0.25 s.
    pub cannot_complete_fast: u32,
    pub cannot_complete: u32,
    pub total_ms: f64,
    pub total_ms_fast: Option<f64>,
    pub set_tried: bool,
    pub set_verified: bool,
    pub set_reverted: bool,
    pub set_error: Option<String>,
    /// NSWorkspace frontmost PID == system-wide AXFocusedApplication PID.
    pub front_agree: Option<bool>,
    pub ax_focused_app_ok: bool,
    pub has_element: bool,
    pub title_readable: bool,
    pub bounds_ok: bool,
    pub caret_window_ok: Option<bool>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Check {
    Pass,
    Fail,
    NoData,
    NotApplicable,
}

impl Check {
    pub fn word(self) -> &'static str {
        match self {
            Check::Pass => "PASS",
            Check::Fail => "FAIL",
            Check::NoData => "no data",
            Check::NotApplicable => "n/a",
        }
    }
}

fn rate(ok: usize, n: usize, min: f64) -> Check {
    if n == 0 {
        Check::NoData
    } else if ok as f64 / n as f64 >= min {
        Check::Pass
    } else {
        Check::Fail
    }
}

/// Nearest-rank percentile; None for no samples.
pub fn percentile(samples: &[f64], p: f64) -> Option<f64> {
    if samples.is_empty() {
        return None;
    }
    let mut v = samples.to_vec();
    v.sort_by(|a, b| a.total_cmp(b));
    let rank = ((p / 100.0) * v.len() as f64).ceil().max(1.0) as usize;
    Some(v[rank.min(v.len()) - 1])
}

fn pastes(delivery: &str) -> bool {
    matches!(delivery, "paste" | "paste_fail_open")
}

#[derive(Clone, Debug, Default, Serialize)]
pub struct Counts {
    pub ok: usize,
    pub n: usize,
}

impl Counts {
    fn add(&mut self, ok: bool) {
        self.n += 1;
        self.ok += usize::from(ok);
    }
    pub fn show(&self) -> String {
        if self.n == 0 {
            "–".into()
        } else {
            format!(
                "{}/{} ({:.0} %)",
                self.ok,
                self.n,
                100.0 * self.ok as f64 / self.n as f64
            )
        }
    }
}

#[derive(Clone, Debug, Serialize)]
pub struct Criteria {
    pub editable: Check,
    pub no_false_card: Check,
    pub selection: Check,
    pub url: Check,
    pub latency: Check,
}

#[derive(Clone, Debug, Serialize)]
pub struct AppScore {
    pub app: String,
    pub family: String,
    pub events: usize,
    /// Text-field events where the delivery pastes (exact or fail-open).
    pub editable: Counts,
    /// ... of which the verdict was a firm "editable" (not fail-open).
    pub editable_exact: usize,
    pub false_cards: usize,
    /// Text field judged a password field.
    pub false_refusals: usize,
    pub editable_without_lease: Counts,
    pub false_cards_without_lease: usize,
    pub password: Counts,
    /// Not-a-text-field events that got the card (informational).
    pub not_text_card: Counts,
    pub selection: Counts,
    pub selection_without_lease: Counts,
    pub selection_text_exact: Counts,
    pub edit_copy_available: Counts,
    pub url: Counts,
    pub url_without_lease: Counts,
    pub hosts: Vec<String>,
    pub p50_ms: Option<f64>,
    pub p95_ms: Option<f64>,
    pub max_ms: Option<f64>,
    pub p95_ms_fast: Option<f64>,
    pub probe_p95_ms: Option<f64>,
    pub calls: usize,
    pub cannot_complete: usize,
    pub cannot_complete_fast: usize,
    pub delivery_changed_at_fast: usize,
    pub set_selected_text: Counts,
    pub set_reverted: Counts,
    pub front_agree: Counts,
    pub ax_focused_app_failed: usize,
    pub title_readable: Counts,
    pub bounds: Counts,
    pub criteria: Criteria,
    pub overall: Check,
}

pub fn score_app(app: &str, rows: &[&Scored]) -> AppScore {
    let family = rows.first().map(|r| r.family.clone()).unwrap_or_default();
    let mut s = AppScore {
        app: app.to_string(),
        family: family.clone(),
        // Scored events only (the cold and as-opened looks have no ground truth).
        events: rows.iter().filter(|r| r.expect.is_some()).count(),
        editable: Counts::default(),
        editable_exact: 0,
        false_cards: 0,
        false_refusals: 0,
        editable_without_lease: Counts::default(),
        false_cards_without_lease: 0,
        password: Counts::default(),
        not_text_card: Counts::default(),
        selection: Counts::default(),
        selection_without_lease: Counts::default(),
        selection_text_exact: Counts::default(),
        edit_copy_available: Counts::default(),
        url: Counts::default(),
        url_without_lease: Counts::default(),
        hosts: Vec::new(),
        p50_ms: None,
        p95_ms: None,
        max_ms: None,
        p95_ms_fast: None,
        probe_p95_ms: None,
        calls: 0,
        cannot_complete: 0,
        cannot_complete_fast: 0,
        delivery_changed_at_fast: 0,
        set_selected_text: Counts::default(),
        set_reverted: Counts::default(),
        front_agree: Counts::default(),
        ax_focused_app_failed: 0,
        title_readable: Counts::default(),
        bounds: Counts::default(),
        criteria: Criteria {
            editable: Check::NoData,
            no_false_card: Check::NoData,
            selection: Check::NoData,
            url: Check::NotApplicable,
            latency: Check::NoData,
        },
        overall: Check::NoData,
    };
    let mut all = Vec::new();
    let mut fast = Vec::new();
    let mut totals = Vec::new();
    for r in rows {
        all.extend_from_slice(&r.calls_ms);
        fast.extend_from_slice(&r.calls_ms_fast);
        totals.push(r.total_ms);
        s.cannot_complete += r.cannot_complete as usize;
        s.cannot_complete_fast += r.cannot_complete_fast as usize;
        if let Some(d) = &r.delivery_fast {
            s.delivery_changed_at_fast += usize::from(*d != r.delivery);
        }
        if let Some(a) = r.front_agree {
            s.front_agree.add(a);
        }
        s.ax_focused_app_failed += usize::from(!r.ax_focused_app_ok);
        s.title_readable.add(r.title_readable);
        if r.has_element {
            s.bounds.add(r.bounds_ok);
        }
        if r.set_tried {
            s.set_selected_text.add(r.set_verified);
            if r.set_verified {
                s.set_reverted.add(r.set_reverted);
            }
        }
        if let Some(h) = &r.url_host {
            if !s.hosts.contains(h) {
                s.hosts.push(h.clone());
            }
        }
        if r.web_page {
            s.url.add(r.url_kind == "http" || r.url_kind == "file");
            if let Some(w) = r.url_readable_without_lease {
                s.url_without_lease.add(w);
            }
        }
        let Some(expect) = r.expect else { continue };
        match expect {
            Expect::Text | Expect::Select => {
                let ok = pastes(&r.delivery);
                s.editable.add(ok);
                s.editable_exact += usize::from(r.verdict == "editable" || r.verdict == "terminal");
                s.false_cards += usize::from(r.delivery == "card");
                s.false_refusals += usize::from(r.delivery == "refuse");
                if let Some(d) = &r.delivery_without_lease {
                    s.editable_without_lease.add(pastes(d));
                    s.false_cards_without_lease += usize::from(d == "card");
                }
                if expect == Expect::Select {
                    // None: no selection was there (the owner did not
                    // select, and the range says so). Not counted.
                    if let Some(x) = r.sel_readable {
                        s.selection.add(x);
                    }
                    if let Some(w) = r.sel_readable_without_lease {
                        s.selection_without_lease.add(w);
                    }
                    if let Some(x) = r.sel_text_ok {
                        s.selection_text_exact.add(x);
                    }
                    if let Some(c) = r.edit_copy_enabled {
                        s.edit_copy_available.add(c);
                    }
                }
            }
            Expect::Password => s.password.add(r.delivery == "refuse"),
            Expect::NotText => s.not_text_card.add(r.delivery == "card"),
            Expect::Info => {}
        }
    }
    s.calls = all.len();
    s.p50_ms = percentile(&all, 50.0);
    s.p95_ms = percentile(&all, 95.0);
    s.max_ms = percentile(&all, 100.0);
    s.p95_ms_fast = percentile(&fast, 95.0);
    s.probe_p95_ms = percentile(&totals, 95.0);

    let browser = family == "chromium" || family == "webkit";
    s.criteria = Criteria {
        editable: rate(s.editable.ok, s.editable.n, EDITABLE_MIN),
        no_false_card: if s.editable.n == 0 {
            Check::NoData
        } else if s.false_cards == 0 {
            Check::Pass
        } else {
            Check::Fail
        },
        selection: rate(s.selection.ok, s.selection.n, SELECTION_MIN),
        url: if browser {
            rate(s.url.ok, s.url.n, URL_MIN)
        } else {
            Check::NotApplicable
        },
        latency: match s.p95_ms {
            None => Check::NoData,
            Some(p) if p <= P95_MAX_MS => Check::Pass,
            Some(_) => Check::Fail,
        },
    };
    let c = &s.criteria;
    let checks = [c.editable, c.no_false_card, c.selection, c.url, c.latency];
    s.overall = if checks.contains(&Check::Fail) {
        Check::Fail
    } else if checks.contains(&Check::NoData) {
        Check::NoData
    } else {
        Check::Pass
    };
    s
}

/// Scores per app, in first-seen order.
pub fn score_all(rows: &[Scored]) -> Vec<AppScore> {
    let mut order: Vec<&str> = Vec::new();
    for r in rows {
        if !order.contains(&r.app.as_str()) {
            order.push(&r.app);
        }
    }
    order
        .into_iter()
        .map(|app| {
            let mine: Vec<&Scored> = rows.iter().filter(|r| r.app == app).collect();
            score_app(app, &mine)
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn row(expect: Expect, delivery: &str) -> Scored {
        Scored {
            app: "x".into(),
            family: "native".into(),
            expect: Some(expect),
            delivery: delivery.into(),
            verdict: if delivery == "paste" {
                "editable"
            } else {
                "unknown"
            }
            .into(),
            calls_ms: vec![1.0, 2.0],
            title_readable: true,
            ax_focused_app_ok: true,
            ..Scored::default()
        }
    }

    #[test]
    fn percentiles() {
        let v: Vec<f64> = (1..=100).map(f64::from).collect();
        assert_eq!(percentile(&v, 95.0), Some(95.0));
        assert_eq!(percentile(&v, 50.0), Some(50.0));
        assert_eq!(percentile(&v, 100.0), Some(100.0));
        assert_eq!(percentile(&[7.0], 95.0), Some(7.0));
        assert_eq!(percentile(&[], 95.0), None);
    }

    #[test]
    fn fail_open_counts_as_pass_and_one_card_fails() {
        let mut rows: Vec<Scored> = (0..19).map(|_| row(Expect::Text, "paste")).collect();
        rows.push(row(Expect::Text, "paste_fail_open"));
        let s = score_all(&rows).remove(0);
        assert_eq!(s.criteria.editable, Check::Pass);
        assert_eq!(s.criteria.no_false_card, Check::Pass);
        assert_eq!(s.editable_exact, 19);
        rows.push(row(Expect::Text, "card"));
        let s = score_all(&rows).remove(0);
        assert_eq!(s.false_cards, 1);
        assert_eq!(s.criteria.no_false_card, Check::Fail);
        assert_eq!(s.criteria.editable, Check::Pass, "20/21 = 95.2 %");
        assert_eq!(s.overall, Check::Fail);
    }

    #[test]
    fn editable_threshold() {
        let mut rows: Vec<Scored> = (0..18).map(|_| row(Expect::Text, "paste")).collect();
        rows.push(row(Expect::Text, "refuse"));
        rows.push(row(Expect::Text, "refuse"));
        let s = score_all(&rows).remove(0);
        assert_eq!(s.editable.show(), "18/20 (90 %)");
        assert_eq!(s.criteria.editable, Check::Fail);
        assert_eq!(s.false_refusals, 2);
        assert_eq!(s.criteria.no_false_card, Check::Pass);
    }

    #[test]
    fn selection_and_non_text_rows() {
        let mut a = row(Expect::Select, "paste");
        a.sel_readable = Some(true);
        let mut b = row(Expect::Select, "paste");
        b.sel_readable = Some(false);
        let c = row(Expect::NotText, "card");
        let d = row(Expect::NotText, "paste_fail_open");
        let mut p = row(Expect::Password, "refuse");
        p.calls_ms = vec![150.0];
        let s = score_all(&[a, b, c, d, p]).remove(0);
        assert_eq!(s.selection.show(), "1/2 (50 %)");
        assert_eq!(s.criteria.selection, Check::Fail);
        assert_eq!(s.not_text_card.show(), "1/2 (50 %)");
        assert_eq!(s.password.show(), "1/1 (100 %)");
        assert_eq!(s.criteria.url, Check::NotApplicable);
        // 9 samples: 1,2,1,2,1,2,1,2,150 → p95 = 150.
        assert_eq!(s.p95_ms, Some(150.0));
        assert_eq!(s.criteria.latency, Check::Fail);
    }

    #[test]
    fn browsers_need_the_url() {
        let mut a = row(Expect::Text, "paste");
        a.family = "chromium".into();
        a.web_page = true;
        a.url_kind = "http".into();
        a.url_host = Some("mail.example.com".into());
        let mut b = a.clone();
        b.url_kind = "none".into();
        b.url_host = None;
        let s = score_all(&[a.clone(), b]).remove(0);
        assert_eq!(s.criteria.url, Check::Fail);
        assert_eq!(s.hosts, vec!["mail.example.com".to_string()]);
        let s = score_all(&[a]).remove(0);
        assert_eq!(s.criteria.url, Check::Pass);
    }

    #[test]
    fn missing_data_is_not_a_pass() {
        let s = score_all(&[row(Expect::NotText, "card")]).remove(0);
        assert_eq!(s.criteria.editable, Check::NoData);
        assert_eq!(s.overall, Check::NoData);
    }

    #[test]
    fn without_lease_and_fast_counts() {
        let mut a = row(Expect::Text, "paste");
        a.delivery_without_lease = Some("card".into());
        a.delivery_fast = Some("paste_fail_open".into());
        let s = score_all(&[a]).remove(0);
        assert_eq!(s.false_cards_without_lease, 1);
        assert_eq!(s.editable_without_lease.show(), "0/1 (0 %)");
        assert_eq!(s.delivery_changed_at_fast, 1);
    }

    #[test]
    fn rows_round_trip_through_json() {
        let a = row(Expect::Select, "paste");
        let back: Scored = serde_json::from_str(&serde_json::to_string(&a).unwrap()).unwrap();
        assert_eq!(a, back);
    }
}
