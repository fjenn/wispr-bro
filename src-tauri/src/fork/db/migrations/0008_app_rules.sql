-- SPDX-License-Identifier: GPL-3.0-or-later
-- fork.db v7 (S-19a): the Style page's app rules.
--
-- v1 created `app_rules` empty and no build ever wrote it, so it is
-- rebuilt here rather than altered (SQLite cannot drop a NOT NULL):
--
-- "match"     the bundle id as macOS reports it (kind 'app') or a site
--             pattern such as 'mail.google.com' (kind 'host', S-19b);
--             compared case-insensitively. Quoted: `match` is an SQL keyword.
-- style_group the group the user put the app in; NULL keeps the built-in
--             group of fork/context/categories.json (the row then only
--             carries auto_send).
-- auto_send   "Press Enter after dictating" (off by default; sent only after
--             a confirmed paste, never after a cleanup fallback or a card).
-- source      'user' for every row the Style page writes; 'builtin' stays
--             reserved (the built-in table lives in categories.json).
-- app_name    the app's name when the rule was made (shown if the app is
--             no longer installed).
-- updated_at  Unix ms of the last change.
--
-- A row that says nothing (no group, no auto-send) is deleted instead.

DROP TABLE app_rules;

CREATE TABLE app_rules (
    "match"     TEXT    PRIMARY KEY COLLATE NOCASE,
    kind        TEXT    NOT NULL DEFAULT 'app' CHECK (kind IN ('app', 'host')),
    style_group TEXT
                CHECK (style_group IS NULL
                       OR style_group IN ('personal', 'work', 'email', 'other')),
    auto_send   INTEGER NOT NULL DEFAULT 0 CHECK (auto_send IN (0, 1)),
    source      TEXT    NOT NULL DEFAULT 'user' CHECK (source IN ('builtin', 'user')),
    app_name    TEXT,
    updated_at  INTEGER NOT NULL DEFAULT 0,
    CHECK (style_group IS NOT NULL OR auto_send = 1)
);
