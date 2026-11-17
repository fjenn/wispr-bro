-- SPDX-License-Identifier: GPL-3.0-or-later
-- Sync between this device and the user's other devices (fork/sync): a file
-- carried by hand (AirDrop), merged here. Nothing leaves the device by
-- itself.
--
-- sync_devices: this device (`is_self = 1`, a random id made here) and every
--   device a file was merged from. `last_import_at`: when its last file was
--   merged here; `last_export_at` (own row): when a file was last made.
-- sync_device_stats / sync_device_app_stats: the other devices' daily
--   numbers, one snapshot per device. A new file from a device replaces its
--   snapshot, so merging twice never counts twice.
-- all_daily_stats / all_daily_app_stats: this device plus the snapshots, per
--   day. Home and Insights read these.
-- sync_tombstones: the key of a deleted dictionary entry or snippet and
--   when it went, so a merge does not bring it back. Written by the
--   triggers below; no code path has to remember.
-- sync_settings: the last seen value of each synced setting and when it
--   changed (0: never changed from the default; such a key is not sent).

CREATE TABLE sync_devices (
    device_id      TEXT    PRIMARY KEY,
    is_self        INTEGER NOT NULL DEFAULT 0 CHECK (is_self IN (0, 1)),
    name           TEXT    NOT NULL DEFAULT '',
    platform       TEXT    NOT NULL DEFAULT '',
    app_version    TEXT    NOT NULL DEFAULT '',
    last_import_at INTEGER,
    last_export_at INTEGER
);

INSERT INTO sync_devices (device_id, is_self)
SELECT lower(hex(randomblob(16))), 1
WHERE NOT EXISTS (SELECT 1 FROM sync_devices WHERE is_self = 1);

CREATE TABLE sync_device_stats (
    device_id        TEXT    NOT NULL,
    day              TEXT    NOT NULL CHECK (length(day) = 10),
    words            INTEGER NOT NULL DEFAULT 0 CHECK (words >= 0),
    dictations       INTEGER NOT NULL DEFAULT 0 CHECK (dictations >= 0),
    speech_ms        INTEGER NOT NULL DEFAULT 0 CHECK (speech_ms >= 0),
    timed_words      INTEGER NOT NULL DEFAULT 0 CHECK (timed_words >= 0),
    fixes_dictionary INTEGER NOT NULL DEFAULT 0 CHECK (fixes_dictionary >= 0),
    fixes_llm        INTEGER NOT NULL DEFAULT 0 CHECK (fixes_llm >= 0),
    PRIMARY KEY (device_id, day)
);

CREATE TABLE sync_device_app_stats (
    device_id      TEXT    NOT NULL,
    day            TEXT    NOT NULL CHECK (length(day) = 10),
    usage_category TEXT    NOT NULL
                   CHECK (usage_category IN ('documents', 'ai_prompts', 'email',
                                             'work', 'personal', 'other')),
    words          INTEGER NOT NULL DEFAULT 0 CHECK (words >= 0),
    dictations     INTEGER NOT NULL DEFAULT 0 CHECK (dictations >= 0),
    PRIMARY KEY (device_id, day, usage_category)
);

CREATE VIEW all_daily_stats AS
SELECT day,
       SUM(words) AS words,
       SUM(dictations) AS dictations,
       SUM(speech_ms) AS speech_ms,
       SUM(timed_words) AS timed_words,
       SUM(fixes_dictionary) AS fixes_dictionary,
       SUM(fixes_llm) AS fixes_llm
FROM (SELECT day, words, dictations, speech_ms, timed_words, fixes_dictionary, fixes_llm
      FROM daily_stats
      UNION ALL
      SELECT day, words, dictations, speech_ms, timed_words, fixes_dictionary, fixes_llm
      FROM sync_device_stats)
GROUP BY day;

CREATE VIEW all_daily_app_stats AS
SELECT day, usage_category, SUM(words) AS words, SUM(dictations) AS dictations
FROM (SELECT day, usage_category, words, dictations FROM daily_app_stats
      UNION ALL
      SELECT day, usage_category, words, dictations FROM sync_device_app_stats)
GROUP BY day, usage_category;

CREATE TABLE sync_tombstones (
    kind       TEXT    NOT NULL CHECK (kind IN ('word', 'rule', 'snippet')),
    norm       TEXT    NOT NULL,
    deleted_at INTEGER NOT NULL,
    PRIMARY KEY (kind, norm)
);

CREATE TABLE sync_settings (
    key        TEXT    PRIMARY KEY,
    value      TEXT    NOT NULL,
    updated_at INTEGER NOT NULL
);

-- Dictionary: a key that goes (deleted, or renamed away) leaves a tombstone;
-- a key that is (again) there has none. Suggestions are this device's own
-- and are never sent, so they leave none.
CREATE TRIGGER sync_dictionary_deleted AFTER DELETE ON dictionary_entries
WHEN OLD.norm IS NOT NULL AND OLD.status <> 'suggested'
BEGIN
    INSERT INTO sync_tombstones (kind, norm, deleted_at)
    VALUES (OLD.kind, OLD.norm, CAST(unixepoch('subsec') * 1000 AS INTEGER))
    ON CONFLICT (kind, norm) DO UPDATE SET deleted_at = excluded.deleted_at;
END;

CREATE TRIGGER sync_dictionary_added AFTER INSERT ON dictionary_entries
WHEN NEW.norm IS NOT NULL
BEGIN
    DELETE FROM sync_tombstones WHERE kind = NEW.kind AND norm = NEW.norm;
END;

CREATE TRIGGER sync_dictionary_rekeyed AFTER UPDATE OF kind, norm ON dictionary_entries
WHEN NEW.norm IS NOT NULL AND (OLD.norm IS NOT NEW.norm OR OLD.kind <> NEW.kind)
BEGIN
    INSERT INTO sync_tombstones (kind, norm, deleted_at)
    SELECT OLD.kind, OLD.norm, CAST(unixepoch('subsec') * 1000 AS INTEGER)
    WHERE OLD.norm IS NOT NULL AND OLD.status <> 'suggested'
    ON CONFLICT (kind, norm) DO UPDATE SET deleted_at = excluded.deleted_at;
    DELETE FROM sync_tombstones WHERE kind = NEW.kind AND norm = NEW.norm;
END;

-- A learned word the user deletes stays as a `rejected` row; that change
-- needs a time for last-writer-wins like any other edit.
CREATE TRIGGER sync_dictionary_status AFTER UPDATE OF status ON dictionary_entries
WHEN OLD.status <> NEW.status
BEGIN
    UPDATE dictionary_entries
    SET updated_at = CAST(unixepoch('subsec') * 1000 AS INTEGER)
    WHERE id = NEW.id;
END;

CREATE TRIGGER sync_snippet_deleted AFTER DELETE ON snippets
BEGIN
    INSERT INTO sync_tombstones (kind, norm, deleted_at)
    VALUES ('snippet', OLD.trigger_norm, CAST(unixepoch('subsec') * 1000 AS INTEGER))
    ON CONFLICT (kind, norm) DO UPDATE SET deleted_at = excluded.deleted_at;
END;

CREATE TRIGGER sync_snippet_added AFTER INSERT ON snippets
BEGIN
    DELETE FROM sync_tombstones WHERE kind = 'snippet' AND norm = NEW.trigger_norm;
END;

CREATE TRIGGER sync_snippet_rekeyed AFTER UPDATE OF trigger_norm ON snippets
WHEN OLD.trigger_norm <> NEW.trigger_norm
BEGIN
    INSERT INTO sync_tombstones (kind, norm, deleted_at)
    VALUES ('snippet', OLD.trigger_norm, CAST(unixepoch('subsec') * 1000 AS INTEGER))
    ON CONFLICT (kind, norm) DO UPDATE SET deleted_at = excluded.deleted_at;
    DELETE FROM sync_tombstones WHERE kind = 'snippet' AND norm = NEW.trigger_norm;
END;
