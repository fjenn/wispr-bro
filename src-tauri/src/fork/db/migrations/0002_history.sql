-- SPDX-License-Identifier: GPL-3.0-or-later
-- fork.db v2 (S-09): what the history hooks, the stats store and the
-- retention job need on top of the v1 skeleton.
--
-- entry_meta:
-- - `day`: the local 'YYYY-MM-DD' date at record time. Stats are bucketed by
--   it, and it never changes after the first write, so a retry on another day
--   still corrects the day the dictation was made on.
-- - `audio_file`: Handy's WAV file name for the row (its `file_name`), kept
--   so the audio retention job does not have to read every Handy row.
-- - `saved`: Handy's "saved" flag is not mirrored; retention asks Handy.
-- - `counted`: 1 while the row's `word_count`, `audio_ms` and fix counts are
--   included in `daily_stats` / `daily_app_stats`. A retry applies the delta
--   between the counted values and the new ones; deleting the row never
--   subtracts (DEC-07).
--
-- daily_stats.timed_words: words from dictations whose speech time is known
-- (`audio_ms > 0`). Words per minute is `timed_words / speech minutes`, so a
-- backfilled row whose audio is gone cannot inflate the rate (SpeakoFlow
-- `insights.rs` / `usage_daily.timed_words`, MIT, pattern).

ALTER TABLE entry_meta ADD COLUMN day TEXT
    CHECK (day IS NULL OR length(day) = 10);
ALTER TABLE entry_meta ADD COLUMN audio_file TEXT;
ALTER TABLE entry_meta ADD COLUMN counted INTEGER NOT NULL DEFAULT 0
    CHECK (counted IN (0, 1));

-- The text retention job scans live rows by age.
CREATE INDEX entry_meta_live_created_at ON entry_meta (created_at) WHERE deleted_at IS NULL;

ALTER TABLE daily_stats ADD COLUMN timed_words INTEGER NOT NULL DEFAULT 0
    CHECK (timed_words >= 0);
