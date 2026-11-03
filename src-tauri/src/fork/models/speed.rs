// SPDX-License-Identifier: GPL-3.0-or-later
//! Measured speed (S-07b, B-59): how long key-up to text takes on this Mac,
//! compared with RS-09's numbers, so a Mac slower than its class gets a
//! lighter model.
//!
//! The `actions.rs` hook `transcribe_take` times each take; warm takes of
//! 2-20 s (the model was already loaded, the length RS-09 measured) are
//! kept in `fork.db` › `asr_speed`, lengths and times only. After 5 such
//! takes with one model the median, divided by RS-09's p50 for that model,
//! is this Mac's measured factor.
//!
//! Hysteresis (Mac wave 1, finding 12: at load 60-96 the factor read 18.2
//! and the policy flipped between two models, re-downloading each time):
//! - a take timed while the Mac is busy ([`machine_quiet`]: the 1-minute
//!   load average per core above [`LOAD_PER_CORE_MAX`]) is not recorded;
//! - a factor counts only when [`confirmed_factor`] sees two sets of
//!   [`SAMPLES`] takes, both slower, spread over at least
//!   [`EVIDENCE_SPAN_MS`] and none older than [`MAX_SAMPLE_AGE_MS`]; it is
//!   the faster of the two sets.

use rusqlite::{params, Connection};

use crate::fork::db::DbError;

/// Takes the factor needs.
pub const SAMPLES: usize = 5;
/// Rows kept per model.
pub const KEPT: i64 = 50;
/// Take lengths that count (RS-09's short takes were 4-12 s).
pub const MIN_AUDIO_MS: u64 = 2_000;
pub const MAX_AUDIO_MS: u64 = 20_000;

/// A take timed with more load than this per core is not recorded.
pub const LOAD_PER_CORE_MAX: f64 = 0.75;
/// The two sets of takes must span at least this long (a busy half hour is
/// not this Mac's speed).
pub const EVIDENCE_SPAN_MS: i64 = 30 * 60 * 1000;
/// Takes older than this no longer count.
pub const MAX_SAMPLE_AGE_MS: i64 = 14 * 24 * 60 * 60 * 1000;

/// Whether the Mac is quiet enough for a take's time to say something about
/// its speed. Unknown load counts as quiet.
pub fn machine_quiet(load_1min: Option<f64>, cores: usize) -> bool {
    match load_1min {
        Some(l) if l.is_finite() => l <= LOAD_PER_CORE_MAX * cores.max(1) as f64,
        _ => true,
    }
}

/// The 1-minute load average and the number of cores, now.
pub fn load_now() -> (Option<f64>, usize) {
    let cores = std::thread::available_parallelism().map_or(1, |n| n.get());
    #[cfg(unix)]
    {
        let mut avg = [0f64; 3];
        // SAFETY: getloadavg writes at most 3 doubles into the array.
        let n = unsafe { libc::getloadavg(avg.as_mut_ptr(), 3) };
        if n >= 1 {
            return (Some(avg[0]), cores);
        }
    }
    (None, cores)
}

/// Whether a take of `audio_ms` counts.
pub fn counts(audio_ms: u64, warm: bool) -> bool {
    warm && (MIN_AUDIO_MS..=MAX_AUDIO_MS).contains(&audio_ms)
}

/// Saves one warm take; keeps the newest [`KEPT`] for the model. Returns
/// how many the model has now.
pub fn record(
    c: &Connection,
    handy_id: &str,
    audio_ms: u64,
    keyup_ms: u64,
    now: i64,
) -> Result<usize, DbError> {
    c.execute(
        "INSERT INTO asr_speed (handy_id, audio_ms, keyup_ms, at) VALUES (?1, ?2, ?3, ?4)",
        params![handy_id, audio_ms as i64, keyup_ms as i64, now],
    )?;
    c.execute(
        "DELETE FROM asr_speed WHERE handy_id = ?1 AND id NOT IN
           (SELECT id FROM asr_speed WHERE handy_id = ?1 ORDER BY id DESC LIMIT ?2)",
        params![handy_id, KEPT],
    )?;
    let n: i64 = c.query_row(
        "SELECT COUNT(*) FROM asr_speed WHERE handy_id = ?1",
        params![handy_id],
        |r| r.get(0),
    )?;
    Ok(n as usize)
}

/// The newest `n` key-up times for a model, newest first.
pub fn recent(c: &Connection, handy_id: &str, n: usize) -> Result<Vec<u64>, DbError> {
    let mut st =
        c.prepare("SELECT keyup_ms FROM asr_speed WHERE handy_id = ?1 ORDER BY id DESC LIMIT ?2")?;
    let rows = st
        .query_map(params![handy_id, n as i64], |r| r.get::<_, i64>(0))?
        .collect::<Result<Vec<_>, _>>()?;
    Ok(rows.into_iter().map(|v| v.max(0) as u64).collect())
}

/// The newest `n` (key-up ms, recorded at) rows for a model, newest first.
pub fn recent_timed(c: &Connection, handy_id: &str, n: usize) -> Result<Vec<(u64, i64)>, DbError> {
    let mut st = c.prepare(
        "SELECT keyup_ms, at FROM asr_speed WHERE handy_id = ?1 ORDER BY id DESC LIMIT ?2",
    )?;
    let rows = st
        .query_map(params![handy_id, n as i64], |r| {
            Ok((r.get::<_, i64>(0)?, r.get::<_, i64>(1)?))
        })?
        .collect::<Result<Vec<_>, _>>()?;
    Ok(rows
        .into_iter()
        .map(|(v, at)| (v.max(0) as u64, at))
        .collect())
}

/// The factor the policy may act on (module docs): from the newest
/// 2 × [`SAMPLES`] rows (newest first), both sets' medians over RS-09's p50,
/// the smaller of the two; `None` without enough recent, spread-out takes.
pub fn confirmed_factor(rows: &[(u64, i64)], reference_p50_ms: f64, now: i64) -> Option<f64> {
    let fresh: Vec<(u64, i64)> = rows
        .iter()
        .copied()
        .filter(|(_, at)| now.saturating_sub(*at) <= MAX_SAMPLE_AGE_MS)
        .take(2 * SAMPLES)
        .collect();
    if fresh.len() < 2 * SAMPLES {
        return None;
    }
    let span = fresh[0].1.saturating_sub(fresh[2 * SAMPLES - 1].1);
    if span < EVIDENCE_SPAN_MS {
        return None;
    }
    let times: Vec<u64> = fresh.iter().map(|(k, _)| *k).collect();
    let newer = factor(&times[..SAMPLES], reference_p50_ms)?;
    let older = factor(&times[SAMPLES..], reference_p50_ms)?;
    Some(newer.min(older))
}

/// The measured factor from the newest [`SAMPLES`] key-up times and RS-09's
/// p50 for the model, or `None` with fewer takes.
pub fn factor(keyup_ms: &[u64], reference_p50_ms: f64) -> Option<f64> {
    if keyup_ms.len() < SAMPLES || reference_p50_ms <= 0.0 {
        return None;
    }
    let mut v: Vec<u64> = keyup_ms[..SAMPLES].to_vec();
    v.sort_unstable();
    Some(v[SAMPLES / 2] as f64 / reference_p50_ms)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_warm_dictation_sized_takes_count() {
        assert!(counts(5_000, true));
        assert!(!counts(5_000, false));
        assert!(!counts(1_000, true));
        assert!(!counts(60_000, true));
        assert!(counts(MIN_AUDIO_MS, true) && counts(MAX_AUDIO_MS, true));
    }

    #[test]
    fn the_factor_is_the_median_of_five_over_rs09() {
        assert_eq!(factor(&[100, 100, 100, 100], 50.0), None);
        assert_eq!(factor(&[100, 300, 200, 900, 150], 50.0), Some(4.0));
        // Only the newest five count.
        assert_eq!(factor(&[60, 60, 60, 60, 60, 9_000, 9_000], 30.0), Some(2.0));
        assert_eq!(factor(&[60; 5], 0.0), None);
    }

    #[test]
    fn a_busy_mac_is_not_measured() {
        // Mac wave 1: load 60-96 on 18 cores.
        assert!(!machine_quiet(Some(60.0), 18));
        assert!(!machine_quiet(Some(96.0), 18));
        assert!(machine_quiet(Some(4.0), 18));
        assert!(machine_quiet(Some(13.5), 18));
        assert!(machine_quiet(None, 18));
        assert!(machine_quiet(Some(f64::NAN), 0));
        assert!(!machine_quiet(Some(2.0), 0));
    }

    #[test]
    fn a_factor_needs_two_slow_sets_over_half_an_hour() {
        const MIN: i64 = 60 * 1000;
        let now = 100 * 24 * 60 * MIN;
        // Ten takes a minute apart: too close together.
        let burst: Vec<(u64, i64)> = (0..10).map(|i| (900, now - i * MIN)).collect();
        assert_eq!(confirmed_factor(&burst, 50.0, now), None);
        // Five slow takes now, five slow ones an hour ago: confirmed.
        let spread: Vec<(u64, i64)> = (0..10)
            .map(|i| (900, now - if i < 5 { i * MIN } else { 60 * MIN + i * MIN }))
            .collect();
        assert_eq!(confirmed_factor(&spread, 50.0, now), Some(18.0));
        // One slow set and one normal set: the faster one counts (a spike
        // never moves the factor on its own).
        let mixed: Vec<(u64, i64)> = spread
            .iter()
            .enumerate()
            .map(|(i, (k, at))| (if i < 5 { *k } else { 60 }, *at))
            .collect();
        assert_eq!(confirmed_factor(&mixed, 50.0, now), Some(1.2));
        // Fewer than ten takes.
        assert_eq!(confirmed_factor(&spread[..9], 50.0, now), None);
        // Old takes no longer count.
        assert_eq!(
            confirmed_factor(&spread, 50.0, now + MAX_SAMPLE_AGE_MS),
            None
        );
    }

    #[test]
    fn rows_are_kept_per_model_and_pruned() {
        let dir = tempfile::tempdir().unwrap();
        let db = crate::fork::db::ForkDb::in_dir(dir.path());
        db.with_conn(|c| {
            for i in 0..60 {
                record(c, "a/m.gguf", 5_000, 100 + i, i as i64)?;
            }
            assert_eq!(record(c, "b/m.gguf", 5_000, 7, 0)?, 1);
            let a = recent(c, "a/m.gguf", 100)?;
            assert_eq!(a.len(), KEPT as usize);
            assert_eq!(a[0], 159);
            assert_eq!(recent(c, "b/m.gguf", 5)?, vec![7]);
            Ok(())
        })
        .unwrap();
    }
}
