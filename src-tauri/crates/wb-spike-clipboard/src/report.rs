// SPDX-License-Identifier: GPL-3.0-or-later
//! The run's results (`summary.json`), the RS-05 verdicts computed from
//! them, and the plain-text summary (`summary.txt`).

use serde::Serialize;
use std::collections::BTreeMap;

use crate::apps::Installed;
use crate::clip::keys::modifier_names;
use crate::clip::restore::Finish;
use crate::fidelity::{Backend, GuardCheck, Row};
use crate::owner::OwnerRestore;
use crate::sources::Kind;
use crate::take::TakeReport;
use crate::ui::Ans;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Landed {
    /// The tool read the text back from the app.
    Confirmed,
    /// The tool could not read the app; the owner answered.
    OwnerYes,
    OwnerSome,
    OwnerNo,
    /// Nobody could tell.
    Unverified,
    /// The tool read the app and the text is not there.
    Missing,
    NotPasted,
}

impl Landed {
    pub fn ok(&self) -> bool {
        matches!(self, Landed::Confirmed | Landed::OwnerYes)
    }
}

#[derive(Clone, Debug, Serialize)]
pub struct AppTake {
    pub kind: Kind,
    pub origin: String,
    /// A ⌘V into Chromium 155/156, where the product would type instead:
    /// shows whether this Chrome drops the paste; not counted as a landing.
    pub race_probe: bool,
    pub take: TakeReport,
    pub landed: Landed,
    /// ⌘V → the text was readable in the app.
    pub confirm_ms: Option<f64>,
    /// The app showed source text (the old clipboard) after this paste.
    pub old_clipboard_pasted: Option<bool>,
}

#[derive(Clone, Debug, Serialize)]
pub struct AppRun {
    pub key: String,
    pub name: String,
    pub version: Option<String>,
    pub class: String,
    pub status: String,
    pub takes: Vec<AppTake>,
    pub owner_count: Option<String>,
    pub owner_saw_other: Option<Ans>,
    pub note: Option<String>,
}

#[derive(Clone, Debug, Serialize)]
pub struct HeldVariant {
    pub held_flags: String,
    pub take: Option<TakeReport>,
    pub landed: Landed,
    /// Modifier flags our V key-down arrived with at the session tap.
    pub v_arrived_with: Vec<String>,
}

#[derive(Clone, Debug, Serialize)]
pub struct HeldTest {
    pub tap_available: bool,
    pub no_wait: Option<HeldVariant>,
    pub with_wait: Option<HeldVariant>,
    pub owner_saw_other: Option<Ans>,
    pub note: Option<String>,
}

#[derive(Clone, Debug, Serialize)]
pub struct ImeTest {
    pub input_source: String,
    pub enabled_by_tool: bool,
    pub selected: bool,
    pub plain: Option<(Landed, TakeReport)>,
    pub composing: Option<(Landed, TakeReport)>,
    /// The end of the field after the composing paste.
    pub composing_field_tail: Option<String>,
    /// The saved list of input sources is as before the step.
    pub input_sources_restored: Option<bool>,
    pub note: Option<String>,
}

#[derive(Clone, Debug, Default, Serialize)]
pub struct Machine {
    pub model: String,
    pub macos_version: String,
    pub macos_build: String,
}

#[derive(Clone, Debug, Default, Serialize)]
pub struct Perms {
    pub accessibility: bool,
    pub post_events: bool,
    pub listen_events: bool,
}

#[derive(Clone, Debug, Serialize)]
pub struct SourceInfo {
    pub kind: Kind,
    pub origin: String,
    pub types: Vec<Vec<String>>,
    pub bytes: usize,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Status {
    Pass,
    Fail,
    Partial,
    Untested,
    Info,
}

impl Status {
    pub fn word(&self) -> &'static str {
        match self {
            Status::Pass => "PASS",
            Status::Fail => "FAIL",
            Status::Partial => "PARTIAL",
            Status::Untested => "UNTESTED",
            Status::Info => "INFO",
        }
    }
}

#[derive(Clone, Debug, Serialize)]
pub struct Verdict {
    pub id: &'static str,
    pub criterion: &'static str,
    pub status: Status,
    pub detail: String,
}

#[derive(Clone, Debug, Default, Serialize)]
pub struct Results {
    pub tool: &'static str,
    pub version: &'static str,
    pub git_sha: String,
    pub mode: String,
    pub started: String,
    pub finished: String,
    pub interrupted: bool,
    pub machine: Machine,
    pub permissions: Perms,
    pub terminal: String,
    pub apps: Vec<Installed>,
    pub chrome_version: Option<String>,
    pub chrome_major: Option<u32>,
    pub owner_saved: Option<(usize, usize)>,
    pub owner_restore: Option<OwnerRestore>,
    pub sources: Vec<SourceInfo>,
    pub real_copy_errors: Vec<(Kind, String)>,
    pub fidelity: Vec<Row>,
    pub guards: Vec<GuardCheck>,
    pub matrix: Vec<AppRun>,
    pub held: Option<HeldTest>,
    pub ime: Option<ImeTest>,
    pub verdicts: Vec<Verdict>,
}

// ---------------------------------------------------------------------------
// Verdicts

fn all_takes(r: &Results) -> impl Iterator<Item = &AppTake> {
    r.matrix.iter().flat_map(|a| a.takes.iter())
}

pub fn verdicts(r: &Results) -> Vec<Verdict> {
    let mut v = Vec::new();

    // 1. Byte-identical restore for every listed type.
    {
        let restored: Vec<&AppTake> = all_takes(r).filter(|t| t.take.fidelity.is_some()).collect();
        let bad: Vec<String> = restored
            .iter()
            .filter(|t| !t.take.fidelity.as_ref().is_some_and(|f| f.byte_identical()))
            .map(|t| {
                format!(
                    "{} ({}): {}",
                    t.kind.label(),
                    t.origin,
                    t.take
                        .fidelity
                        .as_ref()
                        .map(|f| f.describe())
                        .unwrap_or_default()
                )
            })
            .collect();
        let objc_rows: Vec<&Row> = r
            .fidelity
            .iter()
            .filter(|row| row.backend == Backend::Objc2 && Kind::MATRIX.contains(&row.kind))
            .collect();
        let objc_bad: Vec<String> = objc_rows
            .iter()
            .filter(|row| !row.fidelity.as_ref().is_some_and(|f| f.byte_identical()))
            .map(|row| {
                format!(
                    "{} ({}): {}",
                    row.kind.label(),
                    row.origin,
                    row.fidelity
                        .as_ref()
                        .map(|f| f.describe())
                        .or(row.error.clone())
                        .unwrap_or_default()
                )
            })
            .collect();
        let kinds_in_apps: Vec<Kind> = Kind::MATRIX
            .iter()
            .copied()
            .filter(|k| restored.iter().any(|t| t.kind == *k))
            .collect();
        let kinds_checked: Vec<Kind> = Kind::MATRIX
            .iter()
            .copied()
            .filter(|k| objc_rows.iter().any(|row| row.kind == *k) || kinds_in_apps.contains(k))
            .collect();
        let status = if !bad.is_empty() || !objc_bad.is_empty() {
            Status::Fail
        } else if kinds_checked.len() < Kind::MATRIX.len() || restored.is_empty() {
            Status::Partial
        } else {
            Status::Pass
        };
        let mut detail = format!(
            "{} restores after real pastes, {} not identical; automated round trips (objc2): {}, {} not identical",
            restored.len(),
            bad.len(),
            objc_rows.len(),
            objc_bad.len()
        );
        if restored.is_empty() {
            detail.push_str("; no paste into an app was run (automated round trips only)");
        }
        for b in bad.iter().chain(objc_bad.iter()).take(8) {
            detail.push_str(&format!("\n      - {b}"));
        }
        v.push(Verdict {
            id: "restore",
            criterion: "Byte-identical restore for all listed types",
            status,
            detail,
        });
    }

    // 2. Clipboard managers.
    {
        let views: Vec<_> = all_takes(r)
            .filter_map(|t| t.take.manager_view.as_ref())
            .chain(
                r.held
                    .iter()
                    .flat_map(|h| [h.no_wait.as_ref(), h.with_wait.as_ref()])
                    .flatten()
                    .filter_map(|x| x.take.as_ref())
                    .filter_map(|t| t.manager_view.as_ref()),
            )
            .collect();
        let marked = views.iter().filter(|m| m.convention_says_skip()).count();
        let guard = r
            .guards
            .iter()
            .find(|g| g.name.starts_with("listing types"))
            .map(|g| g.pass);
        v.push(Verdict {
            id: "managers",
            criterion: "No clipboard manager records the dictated text",
            status: Status::Untested,
            detail: format!(
                "no clipboard manager is installed on this Mac (Maccy, Raycast, Paste: untested). \
                 What a manager would see: the transcript item carried Transient, AutoGenerated \
                 and Concealed markers with nothing else on the clipboard in {marked} of {} \
                 pastes; listing the types without reading did not trigger a read: {}",
                views.len(),
                match guard {
                    Some(true) => "yes",
                    Some(false) => "NO",
                    None => "not checked",
                }
            ),
        });
    }

    // 3. Held ⌃⌘.
    {
        let (status, detail) = match r
            .held
            .as_ref()
            .and_then(|h| h.with_wait.as_ref().map(|w| (h, w)))
        {
            None => (
                Status::Untested,
                "the held-modifier test was not run".to_string(),
            ),
            Some((h, w)) => {
                let timed_out = w
                    .take
                    .as_ref()
                    .and_then(|t| t.modifier_wait)
                    .is_some_and(|m| m.timed_out);
                let extra = w.v_arrived_with.iter().any(|f| f != "⌘");
                let no_wait = h.no_wait.as_ref().map(|n| {
                    format!(
                        "without the wait: V arrived with {:?}, text {:?}",
                        n.v_arrived_with, n.landed
                    )
                });
                let waited = w
                    .take
                    .as_ref()
                    .and_then(|t| t.modifier_wait)
                    .map(|m| m.waited_ms)
                    .unwrap_or(0);
                let d = format!(
                    "with the wait: waited {waited} ms, timed out: {timed_out}, V arrived with {:?}{}, text {:?}; {}",
                    w.v_arrived_with,
                    if h.tap_available { "" } else { " (no key tap: Input Monitoring off)" },
                    w.landed,
                    no_wait.unwrap_or_else(|| "no-wait variant not run".into())
                );
                let s = if timed_out
                    || extra
                    || matches!(w.landed, Landed::Missing | Landed::OwnerNo)
                {
                    Status::Fail
                } else if w.landed.ok() && h.tap_available {
                    Status::Pass
                } else {
                    Status::Partial
                };
                (s, d)
            }
        };
        v.push(Verdict {
            id: "held_modifiers",
            criterion: "Held ⌃⌘ never turns ⌘V into ⌃⌘V after the modifier wait",
            status,
            detail,
        });
    }

    // 4. IME.
    {
        let (status, detail) = match &r.ime {
            None => (Status::Untested, "the IME step was skipped".to_string()),
            Some(i) => {
                let p = i.plain.as_ref().map(|(l, _)| *l);
                let c = i.composing.as_ref().map(|(l, _)| *l);
                let s = match p {
                    Some(l) if l.ok() => Status::Pass,
                    Some(Landed::Unverified) => Status::Partial,
                    Some(_) => Status::Fail,
                    None => Status::Untested,
                };
                (
                    s,
                    format!(
                        "{}: plain paste {:?}, paste while composing {:?}{}",
                        i.input_source,
                        p,
                        c,
                        i.note
                            .as_ref()
                            .map(|n| format!("; {n}"))
                            .unwrap_or_default()
                    ),
                )
            }
        };
        v.push(Verdict {
            id: "ime",
            criterion: "Paste lands with a CJK IME active (or reports unverified)",
            status,
            detail,
        });
    }

    // 5. Chrome version (155/156 drop ⌘V).
    v.push(Verdict {
        id: "chrome_version",
        criterion: "Chrome version check for 155/156",
        status: Status::Info,
        detail: match (&r.chrome_version, r.chrome_major) {
            (Some(ver), Some(m)) => format!(
                "Google Chrome {ver} (Chromium {m}): {}",
                if (155..=156).contains(&m) {
                    "in the 155/156 range: the product types there instead of pasting (hex); the session still tries ⌘V to see whether it drops"
                } else {
                    "outside 155/156, ⌘V path used"
                }
            ),
            (Some(ver), None) => format!("Google Chrome {ver}; framework version not found"),
            _ => "Google Chrome not found".into(),
        },
    });

    // 6. The Chromium 155/156 ⌘V race on this Mac.
    {
        let probes: Vec<&AppTake> = all_takes(r).filter(|t| t.race_probe).collect();
        if !probes.is_empty() {
            let dropped = probes
                .iter()
                .filter(|t| matches!(t.landed, Landed::Missing | Landed::OwnerNo))
                .count();
            let typed: Vec<&AppTake> = all_takes(r)
                .filter(|t| t.take.method == crate::take::Method::Typing)
                .collect();
            let typed_ok = typed.iter().filter(|t| t.landed.ok()).count();
            v.push(Verdict {
                id: "chromium_race",
                criterion: "Chromium 155/156: ⌘V drops, typing path lands",
                status: Status::Info,
                detail: format!(
                    "⌘V into Chromium 155/156 apps: {} pastes, {dropped} dropped, {} unverified; typing path: {typed_ok} of {} landed",
                    probes.len(),
                    probes.iter().filter(|t| t.landed == Landed::Unverified).count(),
                    typed.len()
                ),
            });
        }
    }

    // 7. Pastes land, and the restore never pastes the old clipboard.
    {
        let takes: Vec<&AppTake> = all_takes(r).filter(|t| !t.race_probe).collect();
        let landed = takes.iter().filter(|t| t.landed.ok()).count();
        let unverified = takes
            .iter()
            .filter(|t| t.landed == Landed::Unverified)
            .count();
        let old = takes
            .iter()
            .filter(|t| t.old_clipboard_pasted == Some(true))
            .count()
            + r.matrix
                .iter()
                .filter(|a| a.owner_saw_other == Some(Ans::Yes))
                .count();
        let apps_done = r.matrix.iter().filter(|a| a.status == "done").count();
        let status = if takes.is_empty() {
            Status::Untested
        } else if old > 0
            || takes.iter().any(|t| {
                matches!(
                    t.landed,
                    Landed::Missing | Landed::OwnerNo | Landed::OwnerSome
                )
            })
        {
            Status::Fail
        } else if unverified > 0 {
            Status::Partial
        } else {
            Status::Pass
        };
        v.push(Verdict {
            id: "matrix",
            criterion: "Paste lands in each app; never the old clipboard",
            status,
            detail: format!(
                "{apps_done} apps, {} pastes: {landed} landed, {unverified} unverified, old clipboard seen {old} time(s)",
                takes.len()
            ),
        });
    }
    v
}

// ---------------------------------------------------------------------------
// Text summary

fn median(v: &mut [f64]) -> Option<f64> {
    if v.is_empty() {
        return None;
    }
    v.sort_by(|a, b| a.total_cmp(b));
    Some(v[v.len() / 2])
}

pub fn text(r: &Results) -> String {
    let mut s = String::new();
    let mut line = |l: String| {
        s.push_str(&l);
        s.push('\n');
    };
    line(format!(
        "RS-05 clipboard fidelity and paste hygiene: {} {} ({}), mode {}{}",
        r.tool,
        r.version,
        r.git_sha,
        r.mode,
        if r.interrupted { ", STOPPED EARLY" } else { "" }
    ));
    line(format!("Started {}, finished {}", r.started, r.finished));
    line(format!(
        "Mac: {}, macOS {} ({}); terminal {}; Accessibility {}, post events {}, Input Monitoring {}",
        r.machine.model,
        r.machine.macos_version,
        r.machine.macos_build,
        r.terminal,
        r.permissions.accessibility,
        r.permissions.post_events,
        r.permissions.listen_events
    ));
    line(String::new());
    line("Verdicts (R5 §7 RS-05 pass criteria):".into());
    for v in &r.verdicts {
        line(format!("  [{}] {}", v.status.word(), v.criterion));
        line(format!("      {}", v.detail));
    }
    line(String::new());
    line(format!(
        "Your own clipboard: saved {} item(s) / {} type(s); put back: {}",
        r.owner_saved.map(|x| x.0).unwrap_or(0),
        r.owner_saved.map(|x| x.1).unwrap_or(0),
        r.owner_restore
            .as_ref()
            .map(|o| if o.identical {
                "exactly".to_string()
            } else {
                format!("NOT exactly: {}", o.detail)
            })
            .unwrap_or_else(|| "not yet".into())
    ));
    line(String::new());
    line("Apps:".into());
    for a in &r.apps {
        line(format!(
            "  {:<14} {:<10} {}",
            a.name,
            a.version.clone().unwrap_or_default(),
            a.class.describe()
        ));
    }
    line(String::new());
    line("Sources (types on the clipboard before the paste):".into());
    for src in &r.sources {
        line(format!(
            "  {:<16} {:<9} {} bytes: {}",
            src.kind.label(),
            src.origin,
            src.bytes,
            src.types
                .iter()
                .map(|t| t.join(", "))
                .collect::<Vec<_>>()
                .join(" | ")
        ));
    }
    for (k, e) in &r.real_copy_errors {
        line(format!("  real copy of {} failed: {e}", k.label()));
    }
    line(String::new());
    line("Automated round trips (snapshot → transcript → restore; no paste):".into());
    for row in &r.fidelity {
        line(format!(
            "  {:<16} {:<9} {:<6} snap {:>7.1} ms  restore {:>7.1} ms  {}{}",
            row.kind.label(),
            row.origin,
            match row.backend {
                Backend::Objc2 => "objc2",
                Backend::Carbon => "carbon",
            },
            row.snapshot_ms,
            row.restore_ms,
            row.fidelity
                .as_ref()
                .map(|f| f.describe())
                .or(row.error.clone())
                .unwrap_or_default(),
            if row.synthesized_skipped > 0 {
                format!(
                    " ({} synthesised flavors left out)",
                    row.synthesized_skipped
                )
            } else {
                String::new()
            }
        ));
    }
    line(String::new());
    line("Guard checks on the real pasteboard:".into());
    for g in &r.guards {
        line(format!(
            "  [{}] {}: {}",
            if g.pass { "ok" } else { "FAIL" },
            g.name,
            g.detail
        ));
    }
    if !r.matrix.is_empty() {
        line(String::new());
        line("Paste matrix (per app: landed / restore / first read after ⌘V):".into());
        let mut by_class: BTreeMap<String, Vec<f64>> = BTreeMap::new();
        for a in &r.matrix {
            line(format!(
                "  {} {} [{}]: {}{}",
                a.name,
                a.version.clone().unwrap_or_default(),
                a.class,
                a.status,
                a.note
                    .as_ref()
                    .map(|n| format!(" ({n})"))
                    .unwrap_or_default()
            ));
            for t in &a.takes {
                let first = t.take.reads_ms.iter().copied().find(|x| *x >= 0.0);
                if let Some(f) = first {
                    by_class.entry(a.class.clone()).or_default().push(f);
                }
                line(format!(
                    "    {:<16} {:<9} {:?}{} | {} after {} ms ({:?}) | reads {:?}{}",
                    t.kind.label(),
                    t.origin,
                    t.landed,
                    t.confirm_ms
                        .map(|c| format!(" in {c:.0} ms"))
                        .unwrap_or_default(),
                    t.take
                        .fidelity
                        .as_ref()
                        .map(|f| f.describe())
                        .unwrap_or_else(|| t.take.decision.clone()),
                    t.take
                        .restore_after_ms
                        .map(|x| format!("{x:.0}"))
                        .unwrap_or("-".into()),
                    t.take.finish.unwrap_or(Finish::Deadline),
                    t.take.reads_ms,
                    t.take
                        .error
                        .as_ref()
                        .map(|e| format!(" | error: {e}"))
                        .unwrap_or_default()
                ));
            }
            if let Some(c) = &a.owner_count {
                line(format!(
                    "    owner: arrived {c}; something else appeared: {:?}",
                    a.owner_saw_other
                ));
            }
        }
        for (class, mut v) in by_class {
            let n = v.len();
            let max = v.iter().copied().fold(0.0, f64::max);
            line(format!(
                "  first read after ⌘V, {class}: median {:.0} ms, max {max:.0} ms (n={n})",
                median(&mut v).unwrap_or(0.0)
            ));
        }
    }
    if let Some(h) = &r.held {
        line(String::new());
        line(format!(
            "Held ⌃⌘ (TextEdit), key tap {}:",
            if h.tap_available { "on" } else { "off" }
        ));
        for (name, var) in [("no wait", &h.no_wait), ("with wait", &h.with_wait)] {
            if let Some(x) = var {
                line(format!(
                    "  {name}: held {}, V arrived with {:?}, text {:?}, modifier wait {:?}",
                    x.held_flags,
                    x.v_arrived_with,
                    x.landed,
                    x.take.as_ref().and_then(|t| t.modifier_wait)
                ));
            }
        }
        if let Some(a) = h.owner_saw_other {
            line(format!(
                "  owner saw something else during the first hold: {a:?}"
            ));
        }
        if let Some(n) = &h.note {
            line(format!("  note: {n}"));
        }
    }
    if let Some(i) = &r.ime {
        line(String::new());
        line(format!(
            "IME: {} (turned on by the tool: {}, selected: {})",
            i.input_source, i.enabled_by_tool, i.selected
        ));
        if let Some((l, _)) = &i.plain {
            line(format!("  plain paste: {l:?}"));
        }
        if let Some((l, _)) = &i.composing {
            line(format!(
                "  paste while composing: {l:?}; field ends {:?}",
                i.composing_field_tail
            ));
        }
        if let Some(n) = &i.note {
            line(format!("  note: {n}"));
        }
    }
    s
}

pub fn flags_label(flags: u64) -> String {
    let n = modifier_names(flags);
    if n.is_empty() {
        "none".into()
    } else {
        n
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::clip::markers::manager_view;
    use crate::clip::snapshot::{compare, Item, Snapshot};

    fn take(identical: bool) -> TakeReport {
        let a = Snapshot {
            items: vec![Item::new(&[("public.utf8-plain-text", b"a")])],
        };
        let b = if identical {
            a.clone()
        } else {
            Snapshot::default()
        };
        TakeReport {
            text: "[dictated text 1] ".into(),
            method: crate::take::Method::CommandV,
            v_keycode: Some(9),
            modifier_wait: None,
            snapshot_ms: 1.0,
            snapshot_bytes: 1,
            skipped: vec![],
            unreadable_types: vec![],
            manager_view: Some(manager_view(&[vec![
                crate::clip::markers::TRANSIENT.into(),
                crate::clip::markers::AUTO_GENERATED.into(),
                crate::clip::markers::CONCEALED.into(),
                crate::clip::markers::TEXT.into(),
            ]])),
            reads_ms: vec![30.0],
            finish: Some(Finish::ReadThenQuiet),
            decision: "restore".into(),
            restore_after_ms: Some(230.0),
            restore_ms: Some(1.0),
            fidelity: Some(compare(&a, &b)),
            reader_output: None,
            landed_ms: Some(40.0),
            provider_released: true,
            error: None,
        }
    }

    fn results(identical: bool, landed: Landed) -> Results {
        Results {
            matrix: vec![AppRun {
                key: "textedit".into(),
                name: "TextEdit".into(),
                version: None,
                class: "native".into(),
                status: "done".into(),
                takes: Kind::MATRIX
                    .iter()
                    .map(|k| AppTake {
                        kind: *k,
                        origin: "synthetic".into(),
                        race_probe: false,
                        take: take(identical),
                        landed,
                        confirm_ms: Some(20.0),
                        old_clipboard_pasted: Some(false),
                    })
                    .collect(),
                owner_count: None,
                owner_saw_other: None,
                note: None,
            }],
            ..Results::default()
        }
    }

    fn status(v: &[Verdict], id: &str) -> Status {
        v.iter().find(|x| x.id == id).unwrap().status
    }

    #[test]
    fn all_identical_and_landed_passes() {
        let v = verdicts(&results(true, Landed::Confirmed));
        assert_eq!(status(&v, "restore"), Status::Pass);
        assert_eq!(status(&v, "matrix"), Status::Pass);
        assert_eq!(status(&v, "managers"), Status::Untested);
        assert!(v
            .iter()
            .find(|x| x.id == "managers")
            .unwrap()
            .detail
            .contains("in 6 of 6"));
        assert_eq!(status(&v, "held_modifiers"), Status::Untested);
        assert_eq!(status(&v, "ime"), Status::Untested);
    }

    #[test]
    fn a_lost_type_fails_the_restore() {
        let v = verdicts(&results(false, Landed::Confirmed));
        assert_eq!(status(&v, "restore"), Status::Fail);
    }

    #[test]
    fn unverified_pastes_are_partial_and_missing_fails() {
        assert_eq!(
            status(&verdicts(&results(true, Landed::Unverified)), "matrix"),
            Status::Partial
        );
        assert_eq!(
            status(&verdicts(&results(true, Landed::Missing)), "matrix"),
            Status::Fail
        );
    }

    fn held(arrived: &[&str], timed_out: bool, tap: bool) -> Results {
        let mut t = take(true);
        t.modifier_wait = Some(crate::clip::keys::ModifierWait {
            waited_ms: 400,
            timed_out,
            held_at_start: 0,
        });
        Results {
            held: Some(HeldTest {
                tap_available: tap,
                no_wait: None,
                with_wait: Some(HeldVariant {
                    held_flags: "⌃⌘".into(),
                    take: Some(t),
                    landed: Landed::Confirmed,
                    v_arrived_with: arrived.iter().map(|s| s.to_string()).collect(),
                }),
                owner_saw_other: None,
                note: None,
            }),
            ..Results::default()
        }
    }

    #[test]
    fn held_modifier_verdict() {
        assert_eq!(
            status(&verdicts(&held(&["⌘"], false, true)), "held_modifiers"),
            Status::Pass
        );
        assert_eq!(
            status(&verdicts(&held(&["⌃⌘"], false, true)), "held_modifiers"),
            Status::Fail
        );
        assert_eq!(
            status(&verdicts(&held(&["⌘"], true, true)), "held_modifiers"),
            Status::Fail
        );
        assert_eq!(
            status(&verdicts(&held(&[], false, false)), "held_modifiers"),
            Status::Partial
        );
    }

    #[test]
    fn chrome_range() {
        let mut r = Results {
            chrome_version: Some("156.0.1.2".into()),
            chrome_major: Some(156),
            ..Results::default()
        };
        let d = verdicts(&r)
            .into_iter()
            .find(|v| v.id == "chrome_version")
            .unwrap();
        assert!(d.detail.contains("types there instead"));
        r.chrome_major = Some(150);
        let d = verdicts(&r)
            .into_iter()
            .find(|v| v.id == "chrome_version")
            .unwrap();
        assert!(d.detail.contains("outside"));
    }

    #[test]
    fn summary_text_renders() {
        let r = results(true, Landed::Confirmed);
        let mut r = r;
        r.verdicts = verdicts(&r);
        let t = text(&r);
        assert!(t.contains("[PASS] Byte-identical restore"));
        assert!(t.contains("TextEdit"));
        assert_eq!(flags_label(0), "none");
    }
}
