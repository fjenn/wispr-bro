// SPDX-License-Identifier: GPL-3.0-or-later
//! The silent model policy (S-07b, DEC-02, DEC-26, DEC-29, DEC-52, W-21).
//!
//! The user never picks a speech model. [`decide`] is a pure function from
//! the user's languages, this Mac ([`super::machine`]), the free disk, the
//! measured speed and the DEC-26 switch to the model to use; its data is
//! `policy.fork.json` (RS-09's measured latencies and footprints, per-language
//! rankings with and without CC-BY models). Pins (repository, revision,
//! file, size, sha256, languages) come from Handy's catalog.
//!
//! Rules (R5 S-07b):
//! 1. A model is a candidate when every language the user speaks has it in
//!    its ranking (a language without a row uses `*`) and the catalog says
//!    it hears that language. With no common model the first language
//!    decides alone (as the starter does).
//! 2. Candidates are ordered by their worst rank over the languages, then
//!    by the sum of ranks, then by the first language's rank.
//! 3. The first candidate whose expected key-up latency for a short take
//!    (RS-09 latency × this Mac's factor) meets §5.1, whose footprint fits
//!    the RAM tier and whose file fits the free disk minus 5 GB (unless it
//!    is already on disk) is the pick.
//! 4. If none meets the latency budget, the fastest one that fits the RAM
//!    tier and the disk is the pick, marked `slower` ("slower on this Mac",
//!    said once).
//! 5. Intel (DEC-29): the CPU latencies with the Intel budget; a model
//!    without measured CPU numbers is not a candidate there.
//! 6. The factor is the machine class's estimate, or the measured one once
//!    5 warm takes say this Mac is slower (B-59); never faster than the
//!    class (a measured win does not start a bigger download).

use std::collections::HashMap;

use once_cell::sync::Lazy;
use serde::{Deserialize, Serialize};
use specta::Type;

use super::machine::{Chip, Machine, Tier};
use super::starter::Pinned;

/// DEC-26 (owner default accepted): CC-BY-4.0 speech weights may be silent
/// defaults. Both tables are kept and tested.
pub const CC_BY_ALLOWED: bool = true;

#[derive(Deserialize, Clone, Copy, Debug)]
#[serde(rename_all = "camelCase")]
pub struct Budget {
    pub p50_ms: f64,
    pub p95_ms: f64,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Budgets {
    apple_silicon: Budget,
    intel: Budget,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct RamTier {
    max_ram_gb: Option<u64>,
    max_footprint_mb: Option<u64>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Factors {
    base: Vec<f64>,
    pro: Vec<f64>,
    max: Vec<f64>,
    ultra: Vec<f64>,
    unknown_apple_silicon: f64,
    intel_cpu: f64,
}

/// RS-09's numbers for one model.
#[derive(Deserialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct ModelData {
    pub p50_ms: f64,
    pub p95_ms: f64,
    #[serde(default)]
    pub cpu_p50_ms: Option<f64>,
    #[serde(default)]
    pub cpu_p95_ms: Option<f64>,
    pub footprint_mb: u64,
    pub cc_by: bool,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Rankings {
    with_cc_by: HashMap<String, Vec<String>>,
    without_cc_by: HashMap<String, Vec<String>>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct PolicyJson {
    budget: Budgets,
    disk_reserve_bytes: u64,
    ram_tiers: Vec<RamTier>,
    factors: Factors,
    models: HashMap<String, ModelData>,
    ranking: Rankings,
    #[serde(default)]
    retired: Vec<String>,
}

/// The policy table with every model pinned.
pub struct Table {
    json: PolicyJson,
    pins: HashMap<String, Pinned>,
}

impl Table {
    /// Builds the table, pinning each model through `pin` (Handy's catalog).
    pub fn load(json: &str, pin: impl Fn(&str) -> Option<Pinned>) -> Result<Table, String> {
        let json: PolicyJson =
            serde_json::from_str(json).map_err(|e| format!("policy.fork.json: {e}"))?;
        let mut pins = HashMap::new();
        for id in json.models.keys() {
            let p = pin(id).ok_or_else(|| format!("{id} is not in Handy's catalog"))?;
            pins.insert(id.clone(), p);
        }
        for table in [&json.ranking.with_cc_by, &json.ranking.without_cc_by] {
            if !table.contains_key("*") {
                return Err("a ranking has no `*` row".into());
            }
            for (lang, ids) in table {
                for id in ids {
                    let data = json
                        .models
                        .get(id)
                        .ok_or_else(|| format!("{lang}: {id} has no model data"))?;
                    if lang != "*" && !pins[id].languages.iter().any(|l| l == lang) {
                        return Err(format!("{id} is ranked for {lang} but does not hear it"));
                    }
                    if std::ptr::eq(table, &json.ranking.without_cc_by) && data.cc_by {
                        return Err(format!("{id} is CC-BY but in the table without CC-BY"));
                    }
                }
            }
        }
        Ok(Table { json, pins })
    }

    pub fn pin(&self, id: &str) -> Option<&Pinned> {
        self.pins.get(id)
    }

    pub fn data(&self, id: &str) -> Option<&ModelData> {
        self.json.models.get(id)
    }

    /// Catalog ids the policy retires: hidden, and a selection migrates.
    pub fn retired(&self) -> &[String] {
        &self.json.retired
    }

    pub fn is_retired(&self, catalog_id: &str) -> bool {
        self.json.retired.iter().any(|r| r == catalog_id)
    }

    /// Every model the policy may pick.
    pub fn model_ids(&self) -> impl Iterator<Item = &String> {
        self.json.models.keys()
    }

    fn ranking(&self, cc_by: bool, lang: &str) -> &[String] {
        let t = if cc_by {
            &self.json.ranking.with_cc_by
        } else {
            &self.json.ranking.without_cc_by
        };
        t.get(lang)
            .or_else(|| t.get("*"))
            .map(Vec::as_slice)
            .unwrap_or(&[])
    }

    /// 1-based rank of `id` for `lang`, if it is usable there.
    fn rank(&self, cc_by: bool, lang: &str, id: &str) -> Option<usize> {
        let hears = self.pins.get(id)?.languages.iter().any(|l| l == lang);
        if !hears {
            return None;
        }
        self.ranking(cc_by, lang)
            .iter()
            .position(|m| m == id)
            .map(|p| p + 1)
    }

    /// The machine class's latency factor (relative to RS-09's M5 Max).
    pub fn class_factor(&self, chip: &Chip) -> f64 {
        let f = &self.json.factors;
        match chip {
            Chip::Intel => f.intel_cpu,
            Chip::UnknownAppleSilicon => f.unknown_apple_silicon,
            Chip::AppleSilicon { generation, tier } => {
                let row = match tier {
                    Tier::Base => &f.base,
                    Tier::Pro => &f.pro,
                    Tier::Max => &f.max,
                    Tier::Ultra => &f.ultra,
                };
                // A newer generation than the table knows is at least as
                // fast as the newest it knows.
                let i = (*generation as usize).clamp(1, row.len()) - 1;
                row.get(i).copied().unwrap_or(f.unknown_apple_silicon)
            }
        }
    }

    /// The footprint ceiling for this much RAM, in MB (`None`: no limit).
    pub fn footprint_cap_mb(&self, ram_gb: u64) -> Option<u64> {
        self.json
            .ram_tiers
            .iter()
            .find(|t| t.max_ram_gb.is_none_or(|m| ram_gb <= m))
            .and_then(|t| t.max_footprint_mb)
    }

    pub fn budget(&self, chip: &Chip) -> Budget {
        if *chip == Chip::Intel {
            self.json.budget.intel
        } else {
            self.json.budget.apple_silicon
        }
    }

    /// Expected (p50, p95) key-up ms for a short take, `None` when the model
    /// cannot run on this chip class (no CPU numbers on Intel).
    pub fn expected_ms(&self, id: &str, chip: &Chip, factor: f64) -> Option<(f64, f64)> {
        let d = self.data(id)?;
        if *chip == Chip::Intel {
            Some((d.cpu_p50_ms? * factor, d.cpu_p95_ms? * factor))
        } else {
            Some((d.p50_ms * factor, d.p95_ms * factor))
        }
    }

    /// RS-09's p50 for `id` on this chip class at factor 1: what a measured
    /// take is compared with.
    pub fn reference_p50_ms(&self, id: &str, chip: &Chip) -> Option<f64> {
        self.expected_ms(id, chip, 1.0).map(|(p50, _)| p50)
    }
}

/// The bundled table. A broken table is a test failure; at runtime it means
/// no policy (the starter stays).
pub static TABLE: Lazy<Option<Table>> = Lazy::new(|| {
    Table::load(include_str!("policy.fork.json"), |id| {
        super::starter::pinned(id, None)
    })
    .map_err(|e| log::error!("models: policy table unusable: {e}"))
    .ok()
});

/// Everything the decision reads.
#[derive(Clone, Debug)]
pub struct Inputs<'a> {
    /// Base language codes, in the user's order.
    pub languages: &'a [String],
    pub machine: &'a Machine,
    pub cc_by: bool,
    /// Measured factor after 5 warm takes, if any.
    pub measured_factor: Option<f64>,
    /// Policy ids whose file is already on disk.
    pub on_disk: &'a [String],
}

/// Why a candidate was passed over (for the log and the Advanced panel).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "snake_case")]
pub enum Skip {
    TooSlow,
    TooBig,
    NoDisk,
    NoCpu,
}

/// The policy's answer.
#[derive(Clone, Debug, PartialEq)]
pub struct Decision {
    /// Policy (catalog) id of the model to use.
    pub model: String,
    /// Nothing met the latency budget: the fastest that fits was picked.
    pub slower: bool,
    /// The factor used.
    pub factor: f64,
    /// Expected (p50, p95) key-up ms on this Mac.
    pub expected_ms: (f64, f64),
    /// The languages the pick was made for (the first alone when no model
    /// hears them all).
    pub languages: Vec<String>,
    /// Better-ranked candidates and why they were passed over.
    pub skipped: Vec<(String, Skip)>,
}

/// The factor for this Mac: the class estimate, or the measured one when
/// slower.
pub fn factor(table: &Table, machine: &Machine, measured: Option<f64>) -> f64 {
    let class = table.class_factor(&machine.chip);
    match measured {
        Some(m) if m.is_finite() && m > class => m,
        _ => class,
    }
}

fn candidates(table: &Table, cc_by: bool, langs: &[String]) -> Vec<String> {
    let mut ids: Vec<&String> = table
        .model_ids()
        .filter(|id| cc_by || !table.data(id).is_some_and(|d| d.cc_by))
        .filter(|id| langs.iter().all(|l| table.rank(cc_by, l, id).is_some()))
        .collect();
    let key = |id: &str| {
        let ranks: Vec<usize> = langs
            .iter()
            .map(|l| table.rank(cc_by, l, id).unwrap_or(usize::MAX))
            .collect();
        (
            ranks.iter().copied().max().unwrap_or(usize::MAX),
            ranks.iter().sum::<usize>(),
            ranks.first().copied().unwrap_or(usize::MAX),
        )
    };
    ids.sort_by(|a, b| key(a).cmp(&key(b)).then_with(|| a.cmp(b)));
    ids.into_iter().cloned().collect()
}

/// The model for these inputs, or `None` when nothing fits at all (keep
/// what is there).
pub fn decide(table: &Table, inputs: &Inputs<'_>) -> Option<Decision> {
    let english = ["en".to_string()];
    let all = if inputs.languages.is_empty() {
        &english[..]
    } else {
        inputs.languages
    };
    let mut langs: Vec<String> = all.to_vec();
    let mut list = candidates(table, inputs.cc_by, &langs);
    if list.is_empty() {
        langs.truncate(1);
        list = candidates(table, inputs.cc_by, &langs);
    }
    let chip = inputs.machine.chip;
    let factor = factor(table, inputs.machine, inputs.measured_factor);
    let budget = table.budget(&chip);
    let cap = table.footprint_cap_mb(inputs.machine.ram_gb());
    let reserve = table.json.disk_reserve_bytes;

    let mut skipped = Vec::new();
    let mut fallback: Option<(String, (f64, f64))> = None;
    for id in &list {
        let (Some(pin), Some(data)) = (table.pin(id), table.data(id)) else {
            continue;
        };
        let Some(expected) = table.expected_ms(id, &chip, factor) else {
            skipped.push((id.clone(), Skip::NoCpu));
            continue;
        };
        if cap.is_some_and(|c| data.footprint_mb > c) {
            skipped.push((id.clone(), Skip::TooBig));
            continue;
        }
        let on_disk = inputs.on_disk.iter().any(|d| d == id);
        let fits_disk = on_disk
            || inputs
                .machine
                .free_disk_bytes
                .is_none_or(|free| pin.size.saturating_add(reserve) <= free);
        if !fits_disk {
            skipped.push((id.clone(), Skip::NoDisk));
            continue;
        }
        if expected.0 <= budget.p50_ms && expected.1 <= budget.p95_ms {
            return Some(Decision {
                model: id.clone(),
                slower: false,
                factor,
                expected_ms: expected,
                languages: langs,
                skipped,
            });
        }
        skipped.push((id.clone(), Skip::TooSlow));
        if fallback.as_ref().is_none_or(|(_, f)| expected.0 < f.0) {
            fallback = Some((id.clone(), expected));
        }
    }
    fallback.map(|(model, expected_ms)| Decision {
        skipped: skipped.into_iter().filter(|(s, _)| *s != model).collect(),
        model,
        slower: true,
        factor,
        expected_ms,
        languages: langs,
    })
}

#[cfg(test)]
#[path = "policy_tests.rs"]
mod tests;
