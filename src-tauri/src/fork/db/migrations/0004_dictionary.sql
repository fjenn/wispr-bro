-- SPDX-License-Identifier: GPL-3.0-or-later
-- fork.db v4 (S-16): the Dictionary page's store.
--
-- dictionary_entries.norm: the entry's key for duplicates, the word (kind
-- 'word') or the "from" (kind 'rule') trimmed, inner whitespace collapsed
-- and Unicode case-folded (fork/text/fold.rs). It is computed in Rust, so
-- the migration's hook fills it for rows written before v4 (and removes a
-- newer duplicate of an older row: the older rule already won, S-11).
-- One entry per kind and key, whatever its status: adding a suggested or
-- rejected entry again turns it into an active one (S-34).
--
-- dictionary_entries.updated_at: last edit (Unix ms); the sort "recently
-- changed" and export order use it.
--
-- dictionary_handy_mirror: the word list the fork last wrote into Handy's
-- `custom_words` setting (fork/dictionary/migrate.rs). A word in Handy's
-- list but not here was added in Handy's own settings and is taken into
-- the dictionary; a word here but no longer in Handy's list was removed
-- there and leaves the dictionary too.

ALTER TABLE dictionary_entries ADD COLUMN norm TEXT;
ALTER TABLE dictionary_entries ADD COLUMN updated_at INTEGER;

CREATE TABLE dictionary_handy_mirror (
    position INTEGER PRIMARY KEY,
    word     TEXT    NOT NULL
);
