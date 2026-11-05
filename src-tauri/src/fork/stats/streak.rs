// SPDX-License-Identifier: MIT AND GPL-3.0-or-later
// Copyright (c) 2026 Abhishek Barali (SpeakoFlow: streak computation)
// Modified by Wispr Bro contributors, 2026-10-30 (S-09).
//
// `streaks` is ported from SpeakoFlow `src-tauri/src/managers/history.rs`
// (`compute_streaks`; AbhishekBarali/SpeakoFlow on GitHub, commit
// 9818a17fcd124e5f1ea6d34780ddea9b154322d8, MIT). Changes: returns a named
// struct, takes the days as `daily_stats.day` strings, and adds tests. MIT
// text: LICENSES/MIT-SpeakoFlow.txt. See docs/fork/PORTS.md.

//! Day streaks from the local days that have at least one counted dictation
//! (`daily_stats`, which survives retention and deletes).

use chrono::NaiveDate;

/// Current and longest runs of consecutive active days.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Streaks {
    /// Consecutive active days ending today, or yesterday when today has no
    /// dictation yet (otherwise every streak would read as broken each
    /// morning until the first dictation).
    pub current: u32,
    pub longest: u32,
}

/// Streaks over `active_days` as of `today`. Input may be unsorted and
/// contain duplicates; days after `today` (a clock change) count toward the
/// longest run only.
pub fn streaks(active_days: &[NaiveDate], today: NaiveDate) -> Streaks {
    let mut days = active_days.to_vec();
    days.sort_unstable();
    days.dedup();

    let mut longest = 0u32;
    let mut run = 0u32;
    let mut prev: Option<NaiveDate> = None;
    for &day in &days {
        run = match prev {
            Some(p) if p.succ_opt() == Some(day) => run + 1,
            _ => 1,
        };
        longest = longest.max(run);
        prev = Some(day);
    }

    let is_active = |d: NaiveDate| days.binary_search(&d).is_ok();
    let mut cursor = if is_active(today) {
        Some(today)
    } else {
        today.pred_opt().filter(|&y| is_active(y))
    };
    let mut current = 0u32;
    while let Some(day) = cursor.filter(|&d| is_active(d)) {
        current += 1;
        cursor = day.pred_opt();
    }
    Streaks { current, longest }
}

/// Parses `daily_stats.day` values, skipping malformed ones.
pub fn parse_days<'a>(days: impl IntoIterator<Item = &'a str>) -> Vec<NaiveDate> {
    days.into_iter()
        .filter_map(|d| NaiveDate::parse_from_str(d, "%Y-%m-%d").ok())
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn d(s: &str) -> NaiveDate {
        NaiveDate::parse_from_str(s, "%Y-%m-%d").unwrap()
    }

    #[test]
    fn empty() {
        assert_eq!(streaks(&[], d("2026-10-30")), Streaks::default());
    }

    #[test]
    fn current_streak_ends_today_or_yesterday() {
        let days = [d("2026-10-27"), d("2026-10-28"), d("2026-10-29")];
        // Today not yet active: the run through yesterday still counts.
        assert_eq!(
            streaks(&days, d("2026-10-30")),
            Streaks {
                current: 3,
                longest: 3
            }
        );
        // Including today.
        let mut with_today = days.to_vec();
        with_today.push(d("2026-10-30"));
        assert_eq!(streaks(&with_today, d("2026-10-30")).current, 4);
        // A missed day breaks it.
        assert_eq!(streaks(&days, d("2026-10-31")).current, 0);
        assert_eq!(streaks(&days, d("2026-10-31")).longest, 3);
    }

    #[test]
    fn longest_run_and_unsorted_duplicates() {
        let days = [
            d("2026-01-05"),
            d("2026-01-01"),
            d("2026-01-02"),
            d("2026-01-02"),
            d("2026-01-03"),
            d("2026-01-06"),
        ];
        assert_eq!(
            streaks(&days, d("2026-01-06")),
            Streaks {
                current: 2,
                longest: 3
            }
        );
    }

    #[test]
    fn crosses_month_and_leap_day() {
        let days = [d("2028-02-28"), d("2028-02-29"), d("2028-03-01")];
        assert_eq!(streaks(&days, d("2028-03-01")).current, 3);
    }

    #[test]
    fn parse_skips_bad_days() {
        assert_eq!(
            parse_days(["2026-10-30", "nope", "2026-13-01"]),
            vec![d("2026-10-30")]
        );
    }
}
