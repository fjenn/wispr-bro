// SPDX-License-Identifier: GPL-3.0-or-later
//! Policy vectors (R5 S-07b: ≥ 60 across languages, RAM, disk, chip,
//! measured speed and both DEC-26 tables): one row per machine × languages
//! → expected model, the expected values read off RS-09's policy tables.

use super::*;
use crate::fork::models::machine::{Chip, Machine, Tier};

const GB: u64 = 1 << 30;

fn table() -> &'static Table {
    TABLE.as_ref().expect("the bundled policy table loads")
}

fn mac(generation: u8, tier: Tier, ram_gb: u64) -> Machine {
    Machine {
        chip: Chip::AppleSilicon { generation, tier },
        ram_bytes: ram_gb * GB,
        free_disk_bytes: Some(500 * GB),
    }
}

fn intel(ram_gb: u64) -> Machine {
    Machine {
        chip: Chip::Intel,
        ram_bytes: ram_gb * GB,
        free_disk_bytes: Some(500 * GB),
    }
}

fn langs(l: &[&str]) -> Vec<String> {
    l.iter().map(|s| s.to_string()).collect()
}

struct V {
    machine: Machine,
    langs: Vec<String>,
    cc_by: bool,
    measured: Option<f64>,
    on_disk: Vec<String>,
}

impl V {
    fn new(machine: Machine, l: &[&str]) -> V {
        V {
            machine,
            langs: langs(l),
            cc_by: true,
            measured: None,
            on_disk: Vec::new(),
        }
    }
    fn no_cc_by(mut self) -> V {
        self.cc_by = false;
        self
    }
    fn measured(mut self, f: f64) -> V {
        self.measured = Some(f);
        self
    }
    fn free_gb(mut self, gb: u64) -> V {
        self.machine.free_disk_bytes = Some(gb * GB);
        self
    }
    fn free_mb(mut self, mb: u64) -> V {
        self.machine.free_disk_bytes = Some(mb * 1_000_000);
        self
    }
    fn on_disk(mut self, id: &str) -> V {
        self.on_disk.push(id.to_string());
        self
    }
    fn decide(&self) -> Option<Decision> {
        decide(
            table(),
            &Inputs {
                languages: &self.langs,
                machine: &self.machine,
                cc_by: self.cc_by,
                measured_factor: self.measured,
                on_disk: &self.on_disk,
            },
        )
    }
    fn pick(&self) -> String {
        self.decide().map(|d| d.model).unwrap_or_default()
    }
}

const UNIFIED: &str = "parakeet-unified-en-0.6b";
const TURBO: &str = "whisper-large-v3-turbo";
const V3: &str = "parakeet-tdt-0.6b-v3";
const P110: &str = "parakeet-tdt_ctc-110m";
const SMALL: &str = "whisper-small";
const BASE: &str = "whisper-base";

/// The vector table: (machine, languages, DEC-26 CC-BY allowed, expected).
fn vectors() -> Vec<(Machine, Vec<&'static str>, bool, &'static str)> {
    use Tier::*;
    let m5max = || mac(5, Max, 128);
    let m1 = || mac(1, Base, 8);
    let m1_16 = || mac(1, Base, 16);
    let m2pro = || mac(2, Pro, 16);
    let m3max = || mac(3, Max, 36);
    let m4 = || mac(4, Base, 16);
    let m5pro = || mac(5, Pro, 24);
    let m1max = || mac(1, Max, 32);
    let m2ultra = || mac(2, Ultra, 64);
    vec![
        // English: unified everywhere it fits (RS-09's English main pick).
        (m5max(), vec!["en"], true, UNIFIED),
        (m1(), vec!["en"], true, UNIFIED),
        (m1_16(), vec!["en"], true, UNIFIED),
        (m2pro(), vec!["en"], true, UNIFIED),
        (m4(), vec!["en"], true, UNIFIED),
        (intel(16), vec!["en"], true, V3),
        // English without CC-BY: turbo where it meets the budget, else small.
        (m5max(), vec!["en"], false, TURBO),
        (m3max(), vec!["en"], false, TURBO),
        (m2ultra(), vec!["en"], false, TURBO),
        (m5pro(), vec!["en"], false, SMALL),
        (m1max(), vec!["en"], false, SMALL),
        (m1(), vec!["en"], false, SMALL),
        (m2pro(), vec!["en"], false, SMALL),
        (m4(), vec!["en"], false, SMALL),
        (intel(16), vec!["en"], false, BASE),
        // Dutch (DEC-52): turbo on fast Macs, Parakeet v3 elsewhere.
        (m5max(), vec!["nl"], true, TURBO),
        (m3max(), vec!["nl"], true, TURBO),
        (m5pro(), vec!["nl"], true, V3),
        (m1(), vec!["nl"], true, V3),
        (m2pro(), vec!["nl"], true, V3),
        (m4(), vec!["nl"], true, V3),
        (m1max(), vec!["nl"], true, V3),
        (intel(16), vec!["nl"], true, V3),
        (m5max(), vec!["nl"], false, TURBO),
        (m1(), vec!["nl"], false, SMALL),
        (m2pro(), vec!["nl"], false, SMALL),
        // French: turbo, then Parakeet v3.
        (m5max(), vec!["fr"], true, TURBO),
        (m2ultra(), vec!["fr"], true, TURBO),
        (m1(), vec!["fr"], true, V3),
        (m4(), vec!["fr"], true, V3),
        (intel(16), vec!["fr"], true, V3),
        (m5max(), vec!["fr"], false, TURBO),
        (m1(), vec!["fr"], false, SMALL),
        (intel(16), vec!["fr"], false, BASE),
        // German and Spanish: the `*` row (Parakeet v3 hears them).
        (m5max(), vec!["de"], true, TURBO),
        (m1(), vec!["de"], true, V3),
        (m1(), vec!["es"], true, V3),
        (m1(), vec!["de"], false, SMALL),
        // Japanese and Chinese: Whisper only.
        (m5max(), vec!["ja"], true, TURBO),
        (m1(), vec!["ja"], true, SMALL),
        (m2pro(), vec!["zh"], true, SMALL),
        (intel(16), vec!["ja"], true, BASE),
        // Two languages: one model that hears both.
        (m5max(), vec!["en", "nl"], true, TURBO),
        (m1(), vec!["en", "nl"], true, V3),
        (m2pro(), vec!["nl", "en"], true, V3),
        (m5max(), vec!["ja", "en"], true, TURBO),
        (m1(), vec!["ja", "en"], true, SMALL),
        (m4(), vec!["en", "ja"], true, SMALL),
        (m5max(), vec!["en", "fr"], true, TURBO),
        (m1(), vec!["en", "fr"], true, V3),
        (m1(), vec!["en", "fr"], false, SMALL),
        (m1(), vec!["en", "de", "fr"], true, V3),
        (intel(16), vec!["en", "nl"], true, V3),
        (intel(16), vec!["en", "ja"], true, BASE),
        // Only turbo hears Dutch and Japanese: slower on an M1, but both kept.
        (m1(), vec!["nl", "ja"], true, TURBO),
        (m1(), vec!["ja", "nl"], true, TURBO),
        // Unknown languages use `*`; no languages counts as English.
        (m1(), vec![], true, UNIFIED),
        (m5max(), vec![], false, TURBO),
        (m1(), vec!["sw"], true, SMALL),
    ]
}

#[test]
fn the_vector_table_matches_rs09() {
    let vs = vectors();
    assert!(vs.len() >= 55, "{} vectors", vs.len());
    let mut failures = Vec::new();
    for (machine, l, cc_by, expected) in vs {
        let mut v = V::new(machine.clone(), &l);
        if !cc_by {
            v = v.no_cc_by();
        }
        let got = v.pick();
        if got != expected {
            failures.push(format!(
                "{:?} {}GB {l:?} cc_by={cc_by}: got {got}, expected {expected}",
                machine.chip,
                machine.ram_gb()
            ));
        }
    }
    assert!(failures.is_empty(), "\n{}", failures.join("\n"));
}

#[test]
fn the_table_loads_and_pins_handy_files() {
    let t = table();
    for id in t.model_ids() {
        let p = t.pin(id).unwrap();
        assert_eq!(p.sha256.len(), 64, "{id}");
        assert_eq!(p.revision.len(), 40, "{id}");
        assert!(crate::fork::net::is_allowed_host(&p.resolve_url()), "{id}");
        assert!(
            crate::catalog::CATALOG.iter().any(|d| d.id == p.handy_id()),
            "{id}: {} is not one of Handy's catalog defaults",
            p.handy_id()
        );
        assert!(
            crate::fork::models::license_filter::restriction_for(&p.catalog_id, &p.license)
                .is_none(),
            "{id}: DEC-27"
        );
        let d = t.data(id).unwrap();
        assert_eq!(d.cc_by, p.license == "cc-by-4.0", "{id}: ccBy flag");
    }
    for r in t.retired() {
        assert!(super::super::starter::pinned(r, None).is_some(), "{r}");
        assert!(t.data(r).is_none(), "{r} is retired and ranked");
    }
}

#[test]
fn broken_tables_are_errors() {
    let pin = |id: &str| super::super::starter::pinned(id, None);
    assert!(Table::load("{", pin).is_err());
    let good = include_str!("policy.fork.json");
    // A ranked model without data.
    let bad = good.replace(
        r#""nl": ["whisper-large-v3-turbo", "parakeet-tdt-0.6b-v3"]"#,
        r#""nl": ["whisper-large-v3-turbo", "canary-1b-v2"]"#,
    );
    assert_ne!(bad, good);
    assert!(Table::load(&bad, pin).is_err());
    // A model ranked for a language it does not hear.
    let bad = good.replace(
        r#""nl": ["whisper-large-v3-turbo", "parakeet-tdt-0.6b-v3"]"#,
        r#""nl": ["parakeet-unified-en-0.6b"]"#,
    );
    assert!(Table::load(&bad, pin).is_err());
    // A CC-BY model in the table without CC-BY.
    let bad = good.replace(
        r#""nl": ["whisper-large-v3-turbo", "whisper-small"]"#,
        r#""nl": ["whisper-large-v3-turbo", "parakeet-tdt-0.6b-v3"]"#,
    );
    assert!(Table::load(&bad, pin).is_err());
    // An unknown catalog id.
    let bad = good.replace(r#""whisper-base": {"#, r#""whisper-nope": {"#);
    assert!(Table::load(&bad, pin).is_err());
}

#[test]
fn class_factors_follow_the_chip() {
    let t = table();
    let f = |c| t.class_factor(&c);
    let a = |g, tier| Chip::AppleSilicon {
        generation: g,
        tier,
    };
    assert!(f(a(1, Tier::Base)) > f(a(5, Tier::Base)));
    assert!(f(a(5, Tier::Base)) > f(a(5, Tier::Pro)));
    assert!(f(a(5, Tier::Pro)) > f(a(5, Tier::Max)));
    assert_eq!(f(a(5, Tier::Max)), 1.0);
    // A newer generation than the table knows: the newest row.
    assert_eq!(f(a(9, Tier::Max)), f(a(5, Tier::Max)));
    assert_eq!(f(Chip::UnknownAppleSilicon), f(a(1, Tier::Base)));
    assert!(f(Chip::Intel) > 1.0);
}

#[test]
fn ram_tiers() {
    let t = table();
    assert_eq!(t.footprint_cap_mb(8), Some(1400));
    assert_eq!(t.footprint_cap_mb(16), Some(3000));
    assert_eq!(t.footprint_cap_mb(18), None);
    assert_eq!(t.footprint_cap_mb(128), None);
}

#[test]
fn turbo_never_fits_8_gb_or_slow_chips() {
    // Turbo's 1.32 GB footprint is under the 8 GB cap, but its latency
    // misses on every base chip.
    for g in 1..=5 {
        let v = V::new(mac(g, Tier::Base, 8), &["nl"]);
        assert_ne!(v.pick(), TURBO, "M{g}");
    }
}

#[test]
fn a_full_disk_skips_big_models() {
    use Tier::*;
    // 5.5 GB free: 0.5 GB after the 5 GB reserve; unified (731 MB) does
    // not fit, parakeet-110m (135 MB) does.
    let v = V::new(mac(5, Max, 128), &["en"]).free_mb(5_500);
    let d = v.decide().unwrap();
    assert_eq!(d.model, P110);
    assert!(d.skipped.contains(&(UNIFIED.to_string(), Skip::NoDisk)));
    // Already on disk: its size does not count.
    assert_eq!(
        V::new(mac(5, Max, 128), &["en"])
            .free_mb(5_500)
            .on_disk(UNIFIED)
            .pick(),
        UNIFIED
    );
    // Nothing fits: no decision (keep what is there).
    assert!(V::new(mac(5, Max, 128), &["en"])
        .free_gb(4)
        .decide()
        .is_none());
    assert!(V::new(mac(1, Base, 8), &["nl"])
        .free_gb(5)
        .decide()
        .is_none());
    // Dutch with 5.8 GB free: v3 (739 MB) fits, turbo (886 MB) does not.
    assert_eq!(V::new(mac(5, Max, 128), &["nl"]).free_mb(5_800).pick(), V3);
    // Unknown free space counts as enough.
    let mut v = V::new(mac(1, Base, 8), &["en"]);
    v.machine.free_disk_bytes = None;
    assert_eq!(v.pick(), UNIFIED);
}

#[test]
fn measured_slowness_steps_down() {
    use Tier::*;
    // An M5 Max that turns out 3× slower than its class: turbo misses.
    assert_eq!(V::new(mac(5, Max, 128), &["nl"]).measured(3.0).pick(), V3);
    // Measured faster than the class: the class estimate stays.
    assert_eq!(V::new(mac(1, Base, 8), &["nl"]).measured(0.5).pick(), V3);
    assert_eq!(
        V::new(mac(1, Base, 8), &["en"])
            .no_cc_by()
            .measured(0.5)
            .pick(),
        SMALL
    );
    // Much slower: English steps from unified down to parakeet-110m.
    assert_eq!(V::new(mac(1, Base, 8), &["en"]).measured(12.0).pick(), P110);
    // Nonsense measurements are ignored.
    assert_eq!(
        V::new(mac(5, Max, 128), &["nl"]).measured(f64::NAN).pick(),
        TURBO
    );
}

#[test]
fn nothing_fast_enough_picks_the_fastest_and_says_slower() {
    use Tier::*;
    // 40× slower than the M5 Max: no model meets 500 ms p50.
    let d = V::new(mac(1, Base, 8), &["en"])
        .measured(40.0)
        .decide()
        .unwrap();
    assert_eq!(d.model, P110);
    assert!(d.slower);
    let d = V::new(mac(1, Base, 8), &["nl"])
        .measured(40.0)
        .decide()
        .unwrap();
    assert_eq!(d.model, V3);
    assert!(d.slower);
    let d = V::new(mac(1, Base, 8), &["ja"])
        .measured(40.0)
        .decide()
        .unwrap();
    assert_eq!(d.model, BASE);
    assert!(d.slower);
    // Normal case: not slower.
    assert!(!V::new(mac(1, Base, 8), &["en"]).decide().unwrap().slower);
    // Intel 8 GB, slow CPU: the fastest CPU model.
    let d = V::new(intel(8), &["fr"]).measured(20.0).decide().unwrap();
    assert!(d.slower);
    assert_eq!(d.model, BASE);
}

#[test]
fn intel_uses_cpu_numbers_only() {
    // Every model without CPU numbers is skipped as NoCpu on Intel.
    let d = V::new(intel(16), &["en"]).decide().unwrap();
    assert_eq!(d.model, V3);
    assert!(d.skipped.contains(&(UNIFIED.to_string(), Skip::NoCpu)));
    assert!(d.skipped.contains(&(TURBO.to_string(), Skip::NoCpu)));
    let budget = table().budget(&Chip::Intel);
    assert!(d.expected_ms.0 <= budget.p50_ms);
    assert!(d.expected_ms.1 <= budget.p95_ms);
}

#[test]
fn the_8_gb_tier_caps_the_footprint() {
    // A hypothetical fast 8 GB chip: turbo's latency fits, its footprint
    // (1,319 MB) is under the cap (1,400 MB).
    let v = V::new(mac(5, Tier::Max, 8), &["nl"]);
    assert_eq!(v.pick(), TURBO);
    // 16 GB and up: no footprint problem for any listed model.
    for id in table().model_ids() {
        assert!(table().data(id).unwrap().footprint_mb <= 3000, "{id}");
    }
}

#[test]
fn decisions_carry_their_reasons() {
    let d = V::new(mac(1, Tier::Base, 8), &["nl", "en"])
        .decide()
        .unwrap();
    assert_eq!(d.model, V3);
    assert_eq!(d.languages, langs(&["nl", "en"]));
    assert!(d.skipped.contains(&(TURBO.to_string(), Skip::TooSlow)));
    assert_eq!(d.factor, 6.0);
    let d = V::new(mac(1, Tier::Base, 8), &["nl", "xx"])
        .decide()
        .unwrap();
    assert_eq!(d.model, V3);
    assert_eq!(d.languages, langs(&["nl"]));
    let d = V::new(mac(1, Tier::Base, 8), &["nl", "ja"])
        .decide()
        .unwrap();
    assert!(d.slower);
}

#[test]
fn without_cc_by_no_cc_by_model_is_ever_picked() {
    use Tier::*;
    let machines = [
        mac(1, Base, 8),
        mac(2, Pro, 16),
        mac(5, Max, 128),
        intel(16),
    ];
    for m in machines {
        for l in [
            vec!["en"],
            vec!["nl"],
            vec!["fr"],
            vec!["de"],
            vec!["ja"],
            vec!["en", "nl"],
        ] {
            for measured in [None, Some(3.0), Some(40.0)] {
                let mut v = V::new(m.clone(), &l).no_cc_by();
                v.measured = measured;
                if let Some(d) = v.decide() {
                    assert!(!table().data(&d.model).unwrap().cc_by, "{l:?}: {}", d.model);
                }
            }
        }
    }
}

#[test]
fn every_pick_hears_every_language_it_was_made_for() {
    use Tier::*;
    for m in [mac(1, Base, 8), mac(5, Max, 128), intel(16)] {
        for l in [
            vec!["en"],
            vec!["nl", "en"],
            vec!["fr", "de"],
            vec!["ja", "en"],
            vec!["zh"],
            vec!["en", "es", "it"],
        ] {
            for cc_by in [true, false] {
                let mut v = V::new(m.clone(), &l);
                v.cc_by = cc_by;
                // Intel without CC-BY has no CPU-measured Dutch model: no pick.
                let Some(d) = v.decide() else {
                    assert!(
                        m.chip == Chip::Intel && !cc_by,
                        "{:?} {l:?}: no pick",
                        m.chip
                    );
                    continue;
                };
                let pin = table().pin(&d.model).unwrap();
                for lang in &d.languages {
                    assert!(
                        pin.languages.contains(lang),
                        "{:?} {l:?}: {}",
                        m.chip,
                        d.model
                    );
                }
            }
        }
    }
}

#[test]
fn whisper_base_never_serves_dutch() {
    use Tier::*;
    for m in [mac(1, Base, 8), mac(5, Max, 128), intel(8), intel(16)] {
        for measured in [None, Some(10.0), Some(100.0)] {
            for cc_by in [true, false] {
                let mut v = V::new(m.clone(), &["nl"]);
                v.measured = measured;
                v.cc_by = cc_by;
                if let Some(d) = v.decide() {
                    assert_ne!(d.model, BASE);
                }
            }
        }
    }
}
