-- SPDX-License-Identifier: GPL-3.0-or-later
-- fork.db v6 (S-17): the Snippets page's store and where a take's
-- snippets are recorded.
--
-- snippets (created empty by v1): `trigger_norm` is the duplicate key, the
-- trigger in NFC, whitespace collapsed, Unicode case-folded with the
-- Turkish dotted and dotless i read as i (fork/text/snippets.rs). It is
-- computed in Rust; no earlier version wrote the table, so no row needs it
-- filled. `body` is plain text with `\n` line breaks (DEC-12); `body_html`
-- stays NULL in M1.
--
-- snippets.last_hit_at: the last take that expanded it (Unix ms).
-- snippets.source: 'user' (added on the page) or 'import' (from a CSV).
--
-- entry_meta.inserted_text: what was inserted when the stages after the
-- LLM changed the take's text (a snippet expanded); NULL when it equals
-- `final_text`. History keeps the spoken words in `final_text` (the stats
-- count those); copying a take (S-15b) uses this.

ALTER TABLE snippets ADD COLUMN last_hit_at INTEGER;
ALTER TABLE snippets ADD COLUMN source TEXT NOT NULL DEFAULT 'user'
    CHECK (source IN ('user', 'import'));
ALTER TABLE entry_meta ADD COLUMN inserted_text TEXT;
