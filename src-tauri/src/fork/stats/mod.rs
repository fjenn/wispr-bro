// SPDX-License-Identifier: GPL-3.0-or-later
//! Stats math (W-14): word counts, streaks and words per minute. Pure
//! functions; the tables they feed (`daily_stats`, `daily_app_stats`) are
//! written by `fork::db::stats`. S-25 adds [`insights`] (the Usage tab) and
//! [`voice`] (the voice profile); `fork::db::insights` reads their rows.

pub mod insights;
pub mod streak;
pub mod voice;
pub mod wordcount;
pub mod wpm;

use chrono::{DateTime, Local, TimeZone};

/// The local calendar day (`YYYY-MM-DD`) of a Unix time in milliseconds in
/// `tz`. Stats are bucketed by the day the user experienced, at record time.
pub fn day_in<Tz: TimeZone>(unix_ms: i64, tz: &Tz) -> String
where
    Tz::Offset: std::fmt::Display,
{
    let utc = DateTime::from_timestamp_millis(unix_ms).unwrap_or_default();
    utc.with_timezone(tz).format("%Y-%m-%d").to_string()
}

/// [`day_in`] the Mac's current time zone.
pub fn local_day(unix_ms: i64) -> String {
    day_in(unix_ms, &Local)
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::FixedOffset;

    #[test]
    fn day_is_local_not_utc() {
        // 2026-10-30T23:30:00Z
        let ms = 1_793_403_000_000;
        assert_eq!(day_in(ms, &FixedOffset::east_opt(0).unwrap()), "2026-10-30");
        assert_eq!(
            day_in(ms, &FixedOffset::east_opt(2 * 3600).unwrap()),
            "2026-10-31"
        );
        assert_eq!(
            day_in(ms, &FixedOffset::west_opt(8 * 3600).unwrap()),
            "2026-10-30"
        );
    }
}
