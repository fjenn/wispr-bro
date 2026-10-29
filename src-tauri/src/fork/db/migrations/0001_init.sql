-- SPDX-License-Identifier: GPL-3.0-or-later
-- fork.db v1: every M1 table from R5 §4.3, empty. Later slices fill them
-- (S-07a, S-09, S-11, S-16, S-17, S-18, S-19a, S-21, S-24). M2 tables
-- (meetings, meeting_segments, meeting_notes, folders) arrive with N-04.
--
-- Conventions:
-- - Timestamps are INTEGER Unix milliseconds (UTC); `day` is a local
--   'YYYY-MM-DD' date taken at record time.
-- - Booleans are INTEGER 0/1 with a CHECK.
-- - `entry_id` is Handy's `history.db` row id. There is no foreign key across
--   the two databases; consistency is kept by idempotent upserts and the
--   launch-time reconcile job (R5 §4.3).
-- - Both FTS tables are contentless trigram tables (`content=''`,
--   `contentless_delete=1`, SQLite >= 3.43) maintained by the app; their
--   rowid is the key of the content row (`entry_meta.entry_id`, `notes.id`).

-- S-09: one row per Handy history entry.
CREATE TABLE entry_meta (
    entry_id             INTEGER PRIMARY KEY,
    created_at           INTEGER NOT NULL,
    asr_text             TEXT,
    final_text           TEXT,
    shown_variant        TEXT    NOT NULL DEFAULT 'final'
                         CHECK (shown_variant IN ('final', 'raw')),
    previous_text        TEXT,
    audio_ms             INTEGER NOT NULL DEFAULT 0 CHECK (audio_ms >= 0),
    word_count           INTEGER NOT NULL DEFAULT 0 CHECK (word_count >= 0),
    char_count           INTEGER NOT NULL DEFAULT 0 CHECK (char_count >= 0),
    language             TEXT,
    app_bundle_id        TEXT,
    app_name             TEXT,
    style_group          TEXT
                         CHECK (style_group IS NULL
                                OR style_group IN ('personal', 'work', 'email', 'other')),
    usage_category       TEXT
                         CHECK (usage_category IS NULL
                                OR usage_category IN ('documents', 'ai_prompts', 'email',
                                                      'work', 'personal', 'other')),
    url_host             TEXT,
    cleanup_level        TEXT,
    cleanup_engine       TEXT
                         CHECK (cleanup_engine IS NULL
                                OR cleanup_engine IN ('none', 'local', 'apple')),
    cleanup_outcome      TEXT
                         CHECK (cleanup_outcome IS NULL
                                OR cleanup_outcome IN ('applied', 'skipped', 'guard',
                                                       'timeout', 'error')),
    fix_count_dictionary INTEGER NOT NULL DEFAULT 0 CHECK (fix_count_dictionary >= 0),
    fix_count_llm        INTEGER NOT NULL DEFAULT 0 CHECK (fix_count_llm >= 0),
    paste_outcome        TEXT
                         CHECK (paste_outcome IS NULL
                                OR paste_outcome IN ('confirmed', 'unverified', 'card',
                                                     'refused', 'queued')),
    context_used         INTEGER NOT NULL DEFAULT 0 CHECK (context_used IN (0, 1)),
    source               TEXT    NOT NULL DEFAULT 'dictation'
                         CHECK (source IN ('dictation', 'file', 'practice', 'command',
                                           'transform')),
    flagged              INTEGER NOT NULL DEFAULT 0 CHECK (flagged IN (0, 1)),
    backfilled           INTEGER NOT NULL DEFAULT 0 CHECK (backfilled IN (0, 1)),
    deleted_at           INTEGER,
    audio_deleted_at     INTEGER
);
CREATE INDEX entry_meta_created_at ON entry_meta (created_at);
CREATE INDEX entry_meta_app_bundle_id ON entry_meta (app_bundle_id);
CREATE INDEX entry_meta_url_host ON entry_meta (url_host) WHERE url_host IS NOT NULL;
CREATE INDEX entry_meta_deleted_at ON entry_meta (deleted_at) WHERE deleted_at IS NOT NULL;
-- Audio retention (14 days, DEC-07) scans rows whose audio is still on disk.
CREATE INDEX entry_meta_audio_pending ON entry_meta (created_at) WHERE audio_deleted_at IS NULL;

-- S-09: stats that survive retention and deletion of history rows.
CREATE TABLE daily_stats (
    day              TEXT    PRIMARY KEY CHECK (length(day) = 10),
    words            INTEGER NOT NULL DEFAULT 0 CHECK (words >= 0),
    dictations       INTEGER NOT NULL DEFAULT 0 CHECK (dictations >= 0),
    speech_ms        INTEGER NOT NULL DEFAULT 0 CHECK (speech_ms >= 0),
    fixes_dictionary INTEGER NOT NULL DEFAULT 0 CHECK (fixes_dictionary >= 0),
    fixes_llm        INTEGER NOT NULL DEFAULT 0 CHECK (fixes_llm >= 0)
);

CREATE TABLE daily_app_stats (
    day            TEXT    NOT NULL CHECK (length(day) = 10),
    usage_category TEXT    NOT NULL
                   CHECK (usage_category IN ('documents', 'ai_prompts', 'email',
                                             'work', 'personal', 'other')),
    words          INTEGER NOT NULL DEFAULT 0 CHECK (words >= 0),
    dictations     INTEGER NOT NULL DEFAULT 0 CHECK (dictations >= 0),
    PRIMARY KEY (day, usage_category)
);

-- S-11, S-25: every change from the ASR text to the final text.
CREATE TABLE corrections (
    id         INTEGER PRIMARY KEY,
    entry_id   INTEGER,
    from_text  TEXT    NOT NULL,
    to_text    TEXT    NOT NULL,
    source     TEXT    NOT NULL CHECK (source IN ('rule', 'llm', 'flag', 'learn')),
    created_at INTEGER NOT NULL
);
CREATE INDEX corrections_entry_id ON corrections (entry_id);
CREATE INDEX corrections_created_at ON corrections (created_at);

-- S-09: search over entry_meta.final_text; rowid = entry_meta.entry_id.
CREATE VIRTUAL TABLE history_fts USING fts5 (
    final_text,
    content = '',
    contentless_delete = 1,
    tokenize = 'trigram'
);

-- S-16
CREATE TABLE dictionary_entries (
    id          INTEGER PRIMARY KEY,
    kind        TEXT    NOT NULL CHECK (kind IN ('word', 'rule')),
    term        TEXT,
    from_text   TEXT,
    to_text     TEXT,
    source      TEXT    NOT NULL DEFAULT 'user'
                CHECK (source IN ('user', 'import', 'flag', 'learn', 'migrated')),
    status      TEXT    NOT NULL DEFAULT 'active'
                CHECK (status IN ('active', 'suggested', 'rejected')),
    created_at  INTEGER NOT NULL,
    hit_count   INTEGER NOT NULL DEFAULT 0 CHECK (hit_count >= 0),
    last_hit_at INTEGER,
    CHECK ((kind = 'word' AND term IS NOT NULL)
           OR (kind = 'rule' AND from_text IS NOT NULL AND to_text IS NOT NULL))
);
CREATE INDEX dictionary_entries_kind_status ON dictionary_entries (kind, status);

-- S-17. `trigger` is an SQL keyword, so it is always quoted.
CREATE TABLE snippets (
    id           INTEGER PRIMARY KEY,
    "trigger"    TEXT    NOT NULL,
    trigger_norm TEXT    NOT NULL UNIQUE,
    body         TEXT    NOT NULL,
    body_html    TEXT,
    created_at   INTEGER NOT NULL,
    updated_at   INTEGER NOT NULL,
    hit_count    INTEGER NOT NULL DEFAULT 0 CHECK (hit_count >= 0)
);

-- S-24
CREATE TABLE notes (
    id         INTEGER PRIMARY KEY,
    title      TEXT    NOT NULL DEFAULT '',
    body       TEXT    NOT NULL DEFAULT '',
    pinned     INTEGER NOT NULL DEFAULT 0 CHECK (pinned IN (0, 1)),
    created_at INTEGER NOT NULL,
    updated_at INTEGER NOT NULL,
    deleted_at INTEGER
);
CREATE INDEX notes_updated_at ON notes (updated_at);

-- rowid = notes.id
CREATE VIRTUAL TABLE notes_fts USING fts5 (
    title,
    body,
    content = '',
    contentless_delete = 1,
    tokenize = 'trigram'
);

-- S-18
CREATE TABLE notifications (
    id         INTEGER PRIMARY KEY,
    kind       TEXT    NOT NULL,
    title_key  TEXT    NOT NULL,
    body_key   TEXT,
    args_json  TEXT    CHECK (args_json IS NULL OR json_valid(args_json)),
    action     TEXT,
    created_at INTEGER NOT NULL,
    read_at    INTEGER
);
CREATE INDEX notifications_created_at ON notifications (created_at);
CREATE INDEX notifications_unread ON notifications (created_at) WHERE read_at IS NULL;

-- S-19a. `match` is an SQL keyword, so it is always quoted.
CREATE TABLE app_rules (
    "match"     TEXT    PRIMARY KEY,
    style_group TEXT    NOT NULL
                CHECK (style_group IN ('personal', 'work', 'email', 'other')),
    auto_send   INTEGER NOT NULL DEFAULT 0 CHECK (auto_send IN (0, 1)),
    source      TEXT    NOT NULL DEFAULT 'user' CHECK (source IN ('builtin', 'user'))
);

-- S-21, S-22
CREATE TABLE transforms (
    id            INTEGER PRIMARY KEY,
    builtin_key   TEXT    UNIQUE,
    name          TEXT    NOT NULL,
    hotkey        TEXT,
    rules_json    TEXT    CHECK (rules_json IS NULL OR json_valid(rules_json)),
    prompt        TEXT,
    examples_json TEXT    CHECK (examples_json IS NULL OR json_valid(examples_json)),
    updated_at    INTEGER NOT NULL
);

-- S-21. Sensitive: holds source text, so it is bounded to the last 20 rows
-- by the trigger below.
CREATE TABLE transform_runs (
    id            INTEGER PRIMARY KEY,
    transform_id  INTEGER REFERENCES transforms (id) ON DELETE CASCADE,
    app_bundle_id TEXT,
    source_text   TEXT    NOT NULL,
    result_text   TEXT,
    rating        INTEGER,
    created_at    INTEGER NOT NULL
);
CREATE INDEX transform_runs_transform_id ON transform_runs (transform_id);

CREATE TRIGGER transform_runs_keep_last_20 AFTER INSERT ON transform_runs
BEGIN
    DELETE FROM transform_runs
    WHERE id NOT IN (SELECT id FROM transform_runs ORDER BY id DESC LIMIT 20);
END;

-- S-07a (session), S-07c (durable)
CREATE TABLE queued_takes (
    id         INTEGER PRIMARY KEY,
    wav_path   TEXT    NOT NULL,
    created_at INTEGER NOT NULL,
    status     TEXT    NOT NULL DEFAULT 'waiting'
               CHECK (status IN ('waiting', 'transcribing', 'ready', 'failed')),
    entry_id   INTEGER
);
CREATE INDEX queued_takes_status ON queued_takes (status, created_at);
