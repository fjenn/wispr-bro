-- SPDX-License-Identifier: GPL-3.0-or-later
-- fork.db v3 (S-11): what the text pipeline did to each take.
--
-- entry_meta.text_stages: the stages that ran, in order, with their change
-- counts, as JSON: [{"stage":"rules","changes":1},{"stage":"handy",...},...]
-- (fork/text/counts.rs). NULL for rows written before S-11, backfilled rows
-- and rows whose transcription the fork did not see. The dictionary rules
-- that applied are `fix_count_dictionary` and one `corrections` row each
-- (source 'rule').

ALTER TABLE entry_meta ADD COLUMN text_stages TEXT
    CHECK (text_stages IS NULL OR json_valid(text_stages));
