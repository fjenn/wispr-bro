-- SPDX-License-Identifier: GPL-3.0-or-later
-- Scratchpad notes sync between the user's devices (fork/sync).
--
-- notes.sync_id: the note's id on every device (a random UUID, made here
--   for a note written here, taken from the other device for one that came
--   from there). Existing notes get one below.
-- notes.changed_at / changed_by: when, and on which device (sync_devices
--   id), the note's synced fields (title, body, pinned) last changed. The
--   merge compares these, never `updated_at` (which orders Recents).
-- notes.base_at: the `changed_at` of the other device's version this one
--   was edited from (0: none). With `changed_by` it tells the merge whether
--   one side's version already contains the other's (the same device's
--   versions follow each other), or whether both edited the note since they
--   last agreed: then the newer stays and the other becomes a copy, so no
--   text is lost.
-- sync_note_tombstones: a deleted note's id, when it went, and the version
--   that was deleted, so a merge removes it there unless it was edited there
--   in a way the deleting device never saw. Written by the triggers below.

ALTER TABLE notes ADD COLUMN sync_id TEXT;
ALTER TABLE notes ADD COLUMN changed_at INTEGER NOT NULL DEFAULT 0;
ALTER TABLE notes ADD COLUMN changed_by TEXT NOT NULL DEFAULT '';
ALTER TABLE notes ADD COLUMN base_at INTEGER NOT NULL DEFAULT 0;

UPDATE notes
SET sync_id = lower(hex(randomblob(4))) || '-' || lower(hex(randomblob(2))) || '-4'
              || substr(lower(hex(randomblob(2))), 2) || '-'
              || substr('89ab', 1 + (abs(random()) % 4), 1)
              || substr(lower(hex(randomblob(2))), 2) || '-' || lower(hex(randomblob(6)))
WHERE sync_id IS NULL;

UPDATE notes
SET changed_at = updated_at,
    changed_by = COALESCE((SELECT device_id FROM sync_devices WHERE is_self = 1), '')
WHERE changed_by = '';

CREATE UNIQUE INDEX notes_sync_id ON notes (sync_id);

CREATE TABLE sync_note_tombstones (
    sync_id    TEXT    PRIMARY KEY,
    deleted_at INTEGER NOT NULL,
    changed_at INTEGER NOT NULL DEFAULT 0,
    changed_by TEXT    NOT NULL DEFAULT '',
    base_at    INTEGER NOT NULL DEFAULT 0
);

-- A note written here: a new id, this device's version.
CREATE TRIGGER sync_note_created AFTER INSERT ON notes
WHEN NEW.sync_id IS NULL
BEGIN
    UPDATE notes
    SET sync_id = lower(hex(randomblob(4))) || '-' || lower(hex(randomblob(2))) || '-4'
                  || substr(lower(hex(randomblob(2))), 2) || '-'
                  || substr('89ab', 1 + (abs(random()) % 4), 1)
                  || substr(lower(hex(randomblob(2))), 2) || '-' || lower(hex(randomblob(6))),
        changed_at = NEW.updated_at,
        changed_by = COALESCE((SELECT device_id FROM sync_devices WHERE is_self = 1), ''),
        base_at = 0
    WHERE id = NEW.id;
END;

-- A note edited here (title, body or pin): a new version by this device.
-- The merge writes all three version columns itself, so the trigger leaves
-- its rows alone.
CREATE TRIGGER sync_note_edited AFTER UPDATE OF title, body, pinned ON notes
WHEN NEW.deleted_at IS NULL
 AND NEW.changed_at = OLD.changed_at AND NEW.changed_by = OLD.changed_by
 AND (NEW.title <> OLD.title OR NEW.body <> OLD.body OR NEW.pinned <> OLD.pinned)
BEGIN
    UPDATE notes
    SET changed_at = MAX(OLD.changed_at + 1,
                         CASE WHEN NEW.updated_at <> OLD.updated_at THEN NEW.updated_at
                              ELSE CAST(unixepoch('subsec') * 1000 AS INTEGER) END),
        changed_by = COALESCE((SELECT device_id FROM sync_devices WHERE is_self = 1), ''),
        base_at = CASE
            WHEN OLD.changed_by = COALESCE((SELECT device_id FROM sync_devices WHERE is_self = 1), '')
            THEN OLD.base_at ELSE OLD.changed_at END
    WHERE id = NEW.id;
END;

-- Deleted (Scratchpad deletes by setting deleted_at): a tombstone with the
-- version that went.
CREATE TRIGGER sync_note_deleted AFTER UPDATE OF deleted_at ON notes
WHEN OLD.deleted_at IS NULL AND NEW.deleted_at IS NOT NULL AND NEW.sync_id IS NOT NULL
BEGIN
    INSERT INTO sync_note_tombstones (sync_id, deleted_at, changed_at, changed_by, base_at)
    VALUES (NEW.sync_id, NEW.deleted_at, OLD.changed_at, OLD.changed_by, OLD.base_at)
    ON CONFLICT (sync_id) DO UPDATE SET
        deleted_at = excluded.deleted_at, changed_at = excluded.changed_at,
        changed_by = excluded.changed_by, base_at = excluded.base_at;
END;

CREATE TRIGGER sync_note_removed AFTER DELETE ON notes
WHEN OLD.deleted_at IS NULL AND OLD.sync_id IS NOT NULL
BEGIN
    INSERT INTO sync_note_tombstones (sync_id, deleted_at, changed_at, changed_by, base_at)
    VALUES (OLD.sync_id, CAST(unixepoch('subsec') * 1000 AS INTEGER),
            OLD.changed_at, OLD.changed_by, OLD.base_at)
    ON CONFLICT (sync_id) DO UPDATE SET
        deleted_at = excluded.deleted_at, changed_at = excluded.changed_at,
        changed_by = excluded.changed_by, base_at = excluded.base_at;
END;

-- Back (a merge brought a deleted note back): no tombstone.
CREATE TRIGGER sync_note_restored AFTER UPDATE OF deleted_at ON notes
WHEN OLD.deleted_at IS NOT NULL AND NEW.deleted_at IS NULL
BEGIN
    DELETE FROM sync_note_tombstones WHERE sync_id = NEW.sync_id;
END;
