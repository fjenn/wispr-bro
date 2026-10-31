-- SPDX-License-Identifier: GPL-3.0-or-later
-- fork.db v5 (S-18): the notifications inbox on top of the v1 table.
--
-- notifications.dedupe_key: a stable key for entries that must exist at most
-- once ("crash:<file>", "milestone:words:10000", "tip:<id>",
-- "feature:<id>"). NULL for entries that may repeat (model ready, update
-- ready, …). The partial unique index lets `INSERT … ON CONFLICT DO NOTHING`
-- drop a repeat without a read first.
ALTER TABLE notifications ADD COLUMN dedupe_key TEXT;
CREATE UNIQUE INDEX notifications_dedupe_key ON notifications (dedupe_key)
    WHERE dedupe_key IS NOT NULL;
