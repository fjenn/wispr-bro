// SPDX-License-Identifier: GPL-3.0-or-later
//! The automated part of RS-05 (no key events, nothing to click): for every
//! source, snapshot → transcript → restore with both backends (objc2 and
//! hex's Carbon path) and compare the bytes; then the guard cases on the
//! real pasteboard (a copy during the paste, back-to-back takes, a reader
//! that triggers the read receipt).

use serde::Serialize;
use std::time::{Duration, Instant};

use crate::clip::carbon;
use crate::clip::markers;
use crate::clip::nspb;
use crate::clip::restore::{Finish, RestoreDecision, RestoreGuard, TargetClass, Timing};
use crate::clip::snapshot::{self, Fidelity, Item, Snapshot};
use crate::macos;
use crate::sources::{Kind, Source};
use crate::take::{self, Method, TakeReport, TakeSpec};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Backend {
    Objc2,
    Carbon,
}

#[derive(Clone, Debug, Serialize)]
pub struct Row {
    pub kind: Kind,
    pub origin: String,
    pub backend: Backend,
    pub types_before: Vec<Vec<String>>,
    pub bytes: usize,
    pub snapshot_ms: f64,
    pub restore_ms: f64,
    pub fidelity: Option<Fidelity>,
    /// Carbon only: flavors macOS synthesised, left out of the snapshot.
    pub synthesized_skipped: usize,
    pub error: Option<String>,
}

fn ms(d: Duration) -> f64 {
    (d.as_secs_f64() * 10_000.0).round() / 10.0
}

fn one(src: &Source, backend: Backend) -> Row {
    let mut row = Row {
        kind: src.kind,
        origin: src.origin.clone(),
        backend,
        types_before: Vec::new(),
        bytes: 0,
        snapshot_ms: 0.0,
        restore_ms: 0.0,
        fidelity: None,
        synthesized_skipped: 0,
        error: None,
    };
    if let Err(e) = nspb::write(&src.snapshot) {
        row.error = Some(format!("could not put the source on the clipboard: {e}"));
        return row;
    }
    macos::pump(Duration::from_millis(20));
    let (before, _) = nspb::read();
    row.types_before = before.types();
    row.bytes = before.bytes();
    let result: Result<(), String> = (|| {
        match backend {
            Backend::Objc2 => {
                let t = Instant::now();
                let (keep, _) = snapshot::apply_policy(&nspb::read().0);
                row.snapshot_ms = ms(t.elapsed());
                let _p = nspb::publish_transcript("[dictated text]", false)?;
                let t = Instant::now();
                nspb::write(&keep)?;
                row.restore_ms = ms(t.elapsed());
            }
            Backend::Carbon => {
                let t = Instant::now();
                let snap = carbon::capture()?;
                row.snapshot_ms = ms(t.elapsed());
                row.synthesized_skipped = snap.synthesized_skipped;
                let _p = nspb::publish_transcript("[dictated text]", false)?;
                let t = Instant::now();
                carbon::restore(&snap)?;
                row.restore_ms = ms(t.elapsed());
            }
        }
        Ok(())
    })();
    if let Err(e) = result {
        row.error = Some(e);
        return row;
    }
    macos::pump(Duration::from_millis(20));
    let (after, _) = nspb::read();
    row.fidelity = Some(snapshot::compare(&before, &after));
    row
}

pub fn run_fidelity(sources: &[Source]) -> Vec<Row> {
    let mut rows = Vec::new();
    for s in sources {
        for b in [Backend::Objc2, Backend::Carbon] {
            if crate::owner::interrupted() {
                return rows;
            }
            rows.push(one(s, b));
        }
    }
    rows
}

#[derive(Clone, Debug, Serialize)]
pub struct GuardCheck {
    pub name: String,
    pub pass: bool,
    pub detail: String,
}

fn text(s: &str) -> Snapshot {
    Snapshot {
        items: vec![Item::new(&[(markers::TEXT, s.as_bytes())])],
    }
}

fn check(name: &str, r: Result<String, String>) -> GuardCheck {
    match r {
        Ok(detail) => GuardCheck {
            name: name.into(),
            pass: true,
            detail,
        },
        Err(detail) => GuardCheck {
            name: name.into(),
            pass: false,
            detail,
        },
    }
}

pub fn run_guards(source: &Source) -> (Vec<GuardCheck>, Option<TakeReport>) {
    let mut out = Vec::new();

    // 1. The user copies while our transcript is on the clipboard.
    out.push(check(
        "user copy during the paste is kept",
        (|| {
            nspb::write(&source.snapshot)?;
            let mut g = RestoreGuard::default();
            let cc0 = nspb::change_count();
            let (before, _) = nspb::read();
            let p = nspb::publish_transcript("[dictated text]", false)?;
            let gen = g.register(snapshot::apply_policy(&before).0, cc0, p.change_count);
            let user = text("WB-SOURCE the user's new copy");
            nspb::write(&user)?;
            let d = g.decide(gen, nspb::change_count());
            let after = nspb::read().0;
            if d != RestoreDecision::ClipboardChanged {
                return Err(format!("decision {}", d.label()));
            }
            if after != user {
                return Err("the user's copy was overwritten".into());
            }
            Ok("no restore; the new copy stayed".into())
        })(),
    ));

    // 2. Two takes back to back, the second before the first restored.
    out.push(check(
        "back-to-back takes restore the original",
        (|| {
            nspb::write(&source.snapshot)?;
            let (before, _) = nspb::read();
            let mut g = RestoreGuard::default();
            let cc0 = nspb::change_count();
            let p1 = nspb::publish_transcript("[dictated text 1]", false)?;
            let gen1 = g.register(snapshot::apply_policy(&before).0, cc0, p1.change_count);
            let cc1 = nspb::change_count();
            let (snap2, _) = nspb::read(); // holds transcript 1
            let p2 = nspb::publish_transcript("[dictated text 2]", false)?;
            let gen2 = g.register(snap2, cc1, p2.change_count);
            let d1 = g.decide(gen1, nspb::change_count());
            if d1 != RestoreDecision::Superseded {
                return Err(format!("first take: {}", d1.label()));
            }
            match g.decide(gen2, nspb::change_count()) {
                RestoreDecision::Restore(s) => {
                    nspb::write(&s)?;
                }
                d => return Err(format!("second take: {}", d.label())),
            }
            let f = snapshot::compare(&before, &nspb::read().0);
            if f.byte_identical() {
                Ok(format!("original back: {}", f.describe()))
            } else {
                Err(f.describe())
            }
        })(),
    ));

    // 3. Listing types (a manager's first look) does not fire the lazy text.
    out.push(check(
        "listing types does not read the text",
        (|| {
            nspb::write(&source.snapshot)?;
            let p = nspb::publish_transcript("[dictated text]", true)?;
            let v = markers::manager_view(&nspb::types());
            macos::pump(Duration::from_millis(100));
            let reads = p.reads().len();
            nspb::write(&source.snapshot)?;
            if reads != 0 {
                return Err(format!("{reads} read(s)"));
            }
            if !v.convention_says_skip() {
                return Err(format!("markers: {v:?}"));
            }
            Ok("0 reads; markers on the text item".into())
        })(),
    ));

    // 4. The skip policy on the real pasteboard: RTFD and PDF are left out,
    // the rest comes back, and an item holding only a skipped type goes.
    out.push(check(
        "RTFD and PDF are left out, the rest restored",
        (|| {
            let src = Snapshot {
                items: vec![
                    Item::new(&[
                        ("public.rtf", b"{\\rtf1 WB-SOURCE}"),
                        ("com.apple.flat-rtfd", b"rtfd bytes"),
                        (markers::TEXT, b"WB-SOURCE"),
                    ]),
                    Item::new(&[("com.adobe.pdf", b"%PDF-1.4 WB-SOURCE")]),
                ],
            };
            nspb::write(&src)?;
            let (before, _) = nspb::read();
            let (keep, skipped) = snapshot::apply_policy(&before);
            let _p = nspb::publish_transcript("[dictated text]", false)?;
            nspb::write(&keep)?;
            let f = snapshot::compare(&before, &nspb::read().0);
            if f.byte_identical() && f.skipped() == 2 && skipped.len() == 2 && f.items_after == 1 {
                Ok(f.describe())
            } else {
                Err(format!("{} (items after: {})", f.describe(), f.items_after))
            }
        })(),
    ));

    // 5. A full take with a reader process in place of a target app.
    nspb::write(&source.snapshot).ok();
    let mut g = RestoreGuard::default();
    let mut spec = TakeSpec::new("[dictated text reader]", TargetClass::Native);
    spec.method = Method::None;
    spec.wait_modifiers = false;
    spec.simulate_reader = true;
    let rep = take::run(&mut g, &spec, &Timing::R5);
    out.push(check("a reader triggers the receipt, then the restore", {
        let ok_read = rep.reader_output.as_deref() == Some("[dictated text reader]");
        let ok_finish = rep.finish == Some(Finish::ReadThenQuiet);
        let ok_fid = rep.fidelity.as_ref().is_some_and(|f| f.byte_identical());
        let d = format!(
            "reads at {:?} ms, finish {:?}, restore {} ms after start, {}",
            rep.reads_ms,
            rep.finish,
            rep.restore_after_ms.unwrap_or(-1.0),
            rep.fidelity
                .as_ref()
                .map(|f| f.describe())
                .unwrap_or_else(|| rep.decision.clone())
        );
        if ok_read && ok_finish && ok_fid {
            Ok(d)
        } else {
            Err(format!("reader got {:?}; {d}", rep.reader_output))
        }
    }));
    (out, Some(rep))
}
