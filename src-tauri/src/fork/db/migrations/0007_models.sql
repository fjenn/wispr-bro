-- SPDX-License-Identifier: GPL-3.0-or-later
-- fork.db v7 (S-07b): the silent model policy's own records.
--
-- speech_models: speech models the fork chose and downloaded (Handy's model
-- id, `<repo>/<file>`). Only these, and a retired or superseded selection,
-- are ever deleted by the fork; a model the user downloaded on the Models
-- page is never touched. `loaded_at` is the first successful load after it
-- was selected (the old model is kept until then); `superseded_at` is when
-- a newer pick replaced it; `deleted_at` when its file was removed.
--
-- asr_speed: key-up to text for warm takes (model already loaded), per
-- model, so the policy can tell a Mac that is slower than its class
-- (B-59). Only lengths and times, never text. Pruned to the newest 50 per
-- model.
--
-- model_policy_state: small keyed facts (the user pinned a model on the
-- Models page; "slower on this Mac" was said).

CREATE TABLE speech_models (
    handy_id TEXT PRIMARY KEY,
    origin TEXT NOT NULL CHECK (origin IN ('starter', 'policy', 'selection')),
    added_at INTEGER NOT NULL,
    loaded_at INTEGER,
    superseded_at INTEGER,
    deleted_at INTEGER
);

CREATE TABLE asr_speed (
    id INTEGER PRIMARY KEY,
    handy_id TEXT NOT NULL,
    audio_ms INTEGER NOT NULL,
    keyup_ms INTEGER NOT NULL,
    at INTEGER NOT NULL
);
CREATE INDEX asr_speed_by_model ON asr_speed (handy_id, id);

CREATE TABLE model_policy_state (
    key TEXT PRIMARY KEY,
    value TEXT NOT NULL
);
