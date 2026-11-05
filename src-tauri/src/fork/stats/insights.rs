// SPDX-License-Identifier: GPL-3.0-or-later
//! Insights › Your usage (S-25, C-50..C-56): pure aggregation over the
//! stats store (`daily_stats`, `daily_app_stats`; `fork::db::insights`
//! reads them). Everything is the user's own numbers on this Mac: there is
//! no population data, so there is no percentile anywhere (C-51); speed is
//! compared with the user's own typing speed instead.
//!
//! - **Speed:** words per minute on speech time (DEC-11, S-09) next to the
//!   typing speed from the typing test (`typing_wpm`, S-29b), or the
//!   average typist's [`AVERAGE_TYPING_WPM`] until the test has run.
//! - **Fixes:** dictionary fixes (S-11 rule hits) and AI cleanup fixes
//!   (S-12a runs of changed words).
//! - **Categories:** words per `usage_category`, all six always listed.
//! - **Heatmap:** one cell per local day for the last [`HEATMAP_DAYS`]
//!   days, banded by words (0 / 1-249 / 250-499 / 500-749 / 750+).

use chrono::{Duration, NaiveDate};
use serde::{Deserialize, Serialize};
use specta::Type;

use crate::fork::context::categories::UsageCategory;
use crate::fork::stats::streak;

/// Typing speed used until the user has taken the typing test (S-29b).
pub const AVERAGE_TYPING_WPM: u32 = 45;

/// Speaking speed assumed for "Time saved vs typing" until there is enough
/// speech for a measured rate (owner's call, rc8; onboarding's
/// `SPEAKING_WPM`).
pub const DEFAULT_SPEAKING_WPM: f64 = 200.0;

/// Days the streak heatmap covers, today included (one year).
pub const HEATMAP_DAYS: i64 = 365;

/// Lower bounds of heatmap bands 1-4 (words that day). Band 0 is no words.
pub const BAND_FLOORS: [u64; 4] = [1, 250, 500, 750];

/// Heatmap band (0-4) of a day with `words` words.
pub fn band(words: u64) -> u8 {
    BAND_FLOORS.iter().filter(|&&floor| words >= floor).count() as u8
}

/// One `daily_stats` row.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DayTotals {
    pub day: NaiveDate,
    pub words: u64,
    pub dictations: u64,
}

/// One `daily_app_stats` row.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CategoryTotals {
    pub category: UsageCategory,
    pub words: u64,
    pub dictations: u64,
}

/// Lifetime sums of `daily_stats`.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Totals {
    pub words: u64,
    pub dictations: u64,
    pub speech_ms: u64,
    pub timed_words: u64,
    pub fixes_dictionary: u64,
    pub fixes_llm: u64,
}

/// Where the typing speed comes from.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "snake_case")]
pub enum TypingSource {
    /// The user's typing test result (S-29b).
    Measured,
    /// [`AVERAGE_TYPING_WPM`].
    Average,
}

/// Where the speaking speed behind the time saved comes from.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "snake_case")]
pub enum SpeakingSource {
    /// The user's own words per minute on speech time.
    Measured,
    /// [`DEFAULT_SPEAKING_WPM`], until there is enough speech.
    Estimate,
}

/// Speaking speed against typing speed.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, Type)]
pub struct SpeedInsight {
    /// Words per minute on speech time; `None` under 3 s of speech.
    pub words_per_minute: Option<f64>,
    pub typing_wpm: u32,
    pub typing_source: TypingSource,
    /// `words_per_minute / typing_wpm`, rounded to one decimal.
    pub times_typing: Option<f64>,
    /// The speaking speed the time saved uses: `words_per_minute`, or
    /// [`DEFAULT_SPEAKING_WPM`] until there is one.
    pub speaking_wpm: f64,
    pub speaking_source: SpeakingSource,
    /// Time dictating saved over typing every dictated word at
    /// `typing_wpm`: `words × (1 / typing_wpm − 1 / speaking_wpm)`, never
    /// below 0.
    pub time_saved_ms: u64,
}

/// Words in one usage category.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, Type)]
pub struct CategoryInsight {
    pub category: UsageCategory,
    pub words: u64,
    pub dictations: u64,
    /// Share of all categorized words, 0..=1 (0 when there are none).
    pub share: f64,
}

/// One heatmap cell.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, Type)]
pub struct HeatmapDay {
    /// Local day, `YYYY-MM-DD`.
    pub date: String,
    pub words: u64,
    /// Band 0-4 ([`band`]).
    pub level: u8,
}

/// Everything on the Usage tab.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, Type)]
pub struct UsageInsights {
    pub total_words: u64,
    pub total_dictations: u64,
    pub speech_ms: u64,
    pub speed: SpeedInsight,
    pub fixes_dictionary: u64,
    pub fixes_llm: u64,
    /// All six categories in [`UsageCategory::ALL`] order.
    pub categories: Vec<CategoryInsight>,
    /// [`HEATMAP_DAYS`] cells, oldest first, ending today.
    pub heatmap: Vec<HeatmapDay>,
    /// Days with at least one word inside the heatmap.
    pub heatmap_active_days: u32,
    pub current_streak_days: u32,
    pub longest_streak_days: u32,
    /// Local day of the first counted dictation.
    pub first_day: Option<String>,
    /// Local day the numbers were computed for.
    pub today: String,
}

/// Speed against `typing_wpm` (the test result) or the average typist.
pub fn speed(totals: &Totals, typing_wpm: Option<u32>) -> SpeedInsight {
    let wpm = super::wpm::words_per_minute(totals.timed_words, totals.speech_ms);
    let (typing, source) = match typing_wpm {
        Some(v) if v > 0 => (v, TypingSource::Measured),
        _ => (AVERAGE_TYPING_WPM, TypingSource::Average),
    };
    let (speaking, speaking_source) = match wpm {
        Some(w) => (w, SpeakingSource::Measured),
        None => (DEFAULT_SPEAKING_WPM, SpeakingSource::Estimate),
    };
    SpeedInsight {
        words_per_minute: wpm,
        typing_wpm: typing,
        typing_source: source,
        times_typing: wpm.map(|w| (w / f64::from(typing) * 10.0).round() / 10.0),
        speaking_wpm: speaking,
        speaking_source,
        time_saved_ms: time_saved_ms(totals.words, speaking, f64::from(typing)),
    }
}

/// Milliseconds saved by speaking `words` at `speaking_wpm` instead of
/// typing them at `typing_wpm`: `words × (1 / typing − 1 / speaking)`
/// minutes, 0 when typing is as fast or faster (or a rate is not positive).
pub fn time_saved_ms(words: u64, speaking_wpm: f64, typing_wpm: f64) -> u64 {
    let positive = |v: f64| v.is_finite() && v > 0.0;
    if !positive(speaking_wpm) || !positive(typing_wpm) {
        return 0;
    }
    let words = words as f64;
    let minutes = words / typing_wpm - words / speaking_wpm;
    (minutes.max(0.0) * 60_000.0).round() as u64
}

/// All six categories with their share of the words.
pub fn categories(rows: &[CategoryTotals]) -> Vec<CategoryInsight> {
    let total: u64 = rows.iter().map(|r| r.words).sum();
    UsageCategory::ALL
        .into_iter()
        .map(|category| {
            let (words, dictations) = rows
                .iter()
                .filter(|r| r.category == category)
                .fold((0, 0), |(w, d), r| (w + r.words, d + r.dictations));
            CategoryInsight {
                category,
                words,
                dictations,
                share: if total == 0 {
                    0.0
                } else {
                    words as f64 / total as f64
                },
            }
        })
        .collect()
}

/// The heatmap cells for the [`HEATMAP_DAYS`] days ending `today`.
pub fn heatmap(days: &[DayTotals], today: NaiveDate) -> Vec<HeatmapDay> {
    let start = today - Duration::days(HEATMAP_DAYS - 1);
    let mut words = std::collections::HashMap::new();
    for d in days.iter().filter(|d| d.day >= start && d.day <= today) {
        *words.entry(d.day).or_insert(0u64) += d.words;
    }
    (0..HEATMAP_DAYS)
        .map(|i| {
            let day = start + Duration::days(i);
            let w = words.get(&day).copied().unwrap_or(0);
            HeatmapDay {
                date: day.format("%Y-%m-%d").to_string(),
                words: w,
                level: band(w),
            }
        })
        .collect()
}

/// The Usage tab from the stats store's rows.
pub fn usage(
    totals: &Totals,
    days: &[DayTotals],
    cats: &[CategoryTotals],
    typing_wpm: Option<u32>,
    today: NaiveDate,
) -> UsageInsights {
    let active: Vec<NaiveDate> = days
        .iter()
        .filter(|d| d.dictations > 0)
        .map(|d| d.day)
        .collect();
    let streaks = streak::streaks(&active, today);
    let cells = heatmap(days, today);
    UsageInsights {
        total_words: totals.words,
        total_dictations: totals.dictations,
        speech_ms: totals.speech_ms,
        speed: speed(totals, typing_wpm),
        fixes_dictionary: totals.fixes_dictionary,
        fixes_llm: totals.fixes_llm,
        categories: categories(cats),
        heatmap_active_days: cells.iter().filter(|c| c.words > 0).count() as u32,
        heatmap: cells,
        current_streak_days: streaks.current,
        longest_streak_days: streaks.longest,
        first_day: active
            .iter()
            .min()
            .map(|d| d.format("%Y-%m-%d").to_string()),
        today: today.format("%Y-%m-%d").to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn d(s: &str) -> NaiveDate {
        NaiveDate::parse_from_str(s, "%Y-%m-%d").unwrap()
    }

    #[test]
    fn bands_follow_the_plan() {
        // R5 S-25: 0 / 1-249 / 250-499 / 500-749 / 750+.
        for (words, level) in [
            (0, 0),
            (1, 1),
            (249, 1),
            (250, 2),
            (499, 2),
            (500, 3),
            (749, 3),
            (750, 4),
            (100_000, 4),
        ] {
            assert_eq!(band(words), level, "{words} words");
        }
    }

    #[test]
    fn speed_against_the_average_or_the_test() {
        let t = Totals {
            timed_words: 300,
            speech_ms: 120_000, // 150 wpm
            ..Totals::default()
        };
        let avg = speed(&t, None);
        assert_eq!(avg.words_per_minute, Some(150.0));
        assert_eq!(avg.typing_wpm, 45);
        assert_eq!(avg.typing_source, TypingSource::Average);
        assert_eq!(avg.times_typing, Some(3.3)); // 3.33
        let measured = speed(&t, Some(60));
        assert_eq!(measured.typing_source, TypingSource::Measured);
        assert_eq!(measured.times_typing, Some(2.5));
        // A zero "test result" is no result.
        assert_eq!(speed(&t, Some(0)).typing_source, TypingSource::Average);
        // Too little speech: no rate, no ratio.
        let none = speed(&Totals::default(), Some(60));
        assert_eq!(none.words_per_minute, None);
        assert_eq!(none.times_typing, None);
        assert_eq!(none.speaking_source, SpeakingSource::Estimate);
        assert_eq!(none.speaking_wpm, DEFAULT_SPEAKING_WPM);
        assert_eq!(none.time_saved_ms, 0);
        assert_eq!(avg.speaking_source, SpeakingSource::Measured);
        assert_eq!(avg.speaking_wpm, 150.0);
    }

    #[test]
    fn time_saved_against_typing() {
        // 1,500 words: typing at 50 wpm takes 30 min, speaking at 150 wpm
        // 10 min, so 20 min saved.
        assert_eq!(time_saved_ms(1_500, 150.0, 50.0), 20 * 60_000);
        // The owner's example: 150,000 words at 150 wpm speaking is 1,000
        // min; typing 4× slower (37.5 wpm) 4,000 min: 3,000 min saved.
        assert_eq!(time_saved_ms(150_000, 150.0, 37.5), 3_000 * 60_000);
        // A typist as fast as the speaker saves nothing, never less.
        assert_eq!(time_saved_ms(1_500, 50.0, 50.0), 0);
        assert_eq!(time_saved_ms(1_500, 40.0, 50.0), 0);
        assert_eq!(time_saved_ms(0, 150.0, 50.0), 0);
        assert_eq!(time_saved_ms(1_500, 0.0, 50.0), 0);
        assert_eq!(time_saved_ms(1_500, 150.0, 0.0), 0);
        assert_eq!(time_saved_ms(1_500, f64::NAN, 50.0), 0);
        assert_eq!(time_saved_ms(1_500, f64::INFINITY, 50.0), 0);
        // Speed uses the lifetime words and the test result.
        let t = Totals {
            words: 300,
            timed_words: 300,
            speech_ms: 120_000, // 150 wpm
            ..Totals::default()
        };
        // 300 / 60 = 5 min typing, 2 min speaking: 3 min.
        assert_eq!(speed(&t, Some(60)).time_saved_ms, 180_000);
        // The average typist (45 wpm): 6⅔ − 2 = 4⅔ min.
        assert_eq!(speed(&t, None).time_saved_ms, 280_000);
        // Words but too little speech for a rate: the 200 wpm estimate.
        // 900 words: 20 min at 45 wpm, 4.5 min at 200: 15.5 min.
        let untimed = Totals {
            words: 900,
            ..Totals::default()
        };
        let s = speed(&untimed, None);
        assert_eq!(s.speaking_source, SpeakingSource::Estimate);
        assert_eq!(s.time_saved_ms, 930_000);
        // The typing test beats the estimate: nothing saved, never negative.
        assert_eq!(speed(&untimed, Some(250)).time_saved_ms, 0);
    }

    #[test]
    fn categories_list_all_six_with_shares() {
        let rows = [
            CategoryTotals {
                category: UsageCategory::Email,
                words: 300,
                dictations: 3,
            },
            CategoryTotals {
                category: UsageCategory::Email,
                words: 100,
                dictations: 1,
            },
            CategoryTotals {
                category: UsageCategory::AiPrompts,
                words: 600,
                dictations: 2,
            },
        ];
        let c = categories(&rows);
        assert_eq!(c.len(), 6);
        assert_eq!(
            c.iter().map(|x| x.category).collect::<Vec<_>>(),
            UsageCategory::ALL.to_vec()
        );
        let email = &c[2];
        assert_eq!((email.words, email.dictations), (400, 4));
        assert!((email.share - 0.4).abs() < 1e-9);
        assert!((c[1].share - 0.6).abs() < 1e-9);
        assert_eq!(c[0].share, 0.0);
        assert!(categories(&[]).iter().all(|x| x.share == 0.0));
    }

    #[test]
    fn heatmap_is_a_year_ending_today() {
        let today = d("2026-10-31");
        let days = [
            DayTotals {
                day: d("2026-10-31"),
                words: 260,
                dictations: 4,
            },
            DayTotals {
                day: d("2026-10-29"),
                words: 12,
                dictations: 1,
            },
            // Outside the window on both sides.
            DayTotals {
                day: d("2025-10-31"),
                words: 900,
                dictations: 9,
            },
            DayTotals {
                day: d("2026-11-02"),
                words: 900,
                dictations: 9,
            },
        ];
        let h = heatmap(&days, today);
        assert_eq!(h.len(), 365);
        assert_eq!(h[0].date, "2025-11-01");
        assert_eq!(h[364].date, "2026-10-31");
        assert_eq!((h[364].words, h[364].level), (260, 2));
        assert_eq!((h[362].words, h[362].level), (12, 1));
        assert_eq!(h[363].level, 0);
        assert_eq!(h.iter().filter(|c| c.words > 0).count(), 2);
    }

    #[test]
    fn heatmap_crosses_a_leap_day() {
        let h = heatmap(&[], d("2028-03-01"));
        assert!(h.iter().any(|c| c.date == "2028-02-29"));
        assert_eq!(h[364].date, "2028-03-01");
    }

    /// The hand-computed fixture of the exit criterion (R5 S-25): four days,
    /// three categories, a measured typing speed.
    #[test]
    fn usage_matches_a_hand_computed_fixture() {
        let today = d("2026-10-31");
        let days = vec![
            DayTotals {
                day: d("2026-10-28"),
                words: 800,
                dictations: 10,
            },
            DayTotals {
                day: d("2026-10-29"),
                words: 300,
                dictations: 5,
            },
            DayTotals {
                day: d("2026-10-30"),
                words: 120,
                dictations: 2,
            },
            DayTotals {
                day: d("2026-10-31"),
                words: 530,
                dictations: 6,
            },
        ];
        let cats = vec![
            CategoryTotals {
                category: UsageCategory::Documents,
                words: 875,
                dictations: 9,
            },
            CategoryTotals {
                category: UsageCategory::Email,
                words: 525,
                dictations: 8,
            },
            CategoryTotals {
                category: UsageCategory::Personal,
                words: 350,
                dictations: 6,
            },
        ];
        let totals = Totals {
            words: 1_750,
            dictations: 23,
            speech_ms: 700_000,
            timed_words: 1_750,
            fixes_dictionary: 7,
            fixes_llm: 31,
        };
        let u = usage(&totals, &days, &cats, Some(50), today);
        // 1,750 words over 700 s of speech = 150 wpm; 150 / 50 = 3.0×.
        assert_eq!(u.speed.words_per_minute, Some(150.0));
        assert_eq!(u.speed.times_typing, Some(3.0));
        // 1,750 / 50 = 35 min typing, 1,750 / 150 = 11⅔ min speaking.
        assert_eq!(u.speed.time_saved_ms, 1_400_000);
        assert_eq!((u.total_words, u.total_dictations), (1_750, 23));
        assert_eq!((u.fixes_dictionary, u.fixes_llm), (7, 31));
        // 875 / 1,750 = 50 %, 525 = 30 %, 350 = 20 %.
        let shares: Vec<f64> = u.categories.iter().map(|c| c.share).collect();
        assert_eq!(shares, vec![0.5, 0.0, 0.3, 0.0, 0.2, 0.0]);
        // Bands: 800 → 4, 300 → 2, 120 → 1, 530 → 3.
        let last4: Vec<u8> = u.heatmap[361..].iter().map(|c| c.level).collect();
        assert_eq!(last4, vec![4, 2, 1, 3]);
        assert_eq!(u.heatmap_active_days, 4);
        assert_eq!((u.current_streak_days, u.longest_streak_days), (4, 4));
        assert_eq!(u.first_day.as_deref(), Some("2026-10-28"));
        assert_eq!(u.today, "2026-10-31");
    }
}
