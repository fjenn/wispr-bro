-- SPDX-License-Identifier: GPL-3.0-or-later
-- S-21: the two built-in transforms. The tables and their 20-run bound were
-- created empty by 0001; this migration only seeds their definitions.

INSERT INTO transforms (id, builtin_key, name, hotkey, prompt, updated_at)
SELECT 1, 'polish', 'Polish', 'option+1',
       'Polish this text. Fix grammar, spelling, punctuation and awkward wording while preserving its meaning, tone, language, formatting and approximate length. Return the same text when no change is needed.',
       0
WHERE NOT EXISTS (SELECT 1 FROM transforms WHERE builtin_key = 'polish');

INSERT INTO transforms (id, builtin_key, name, hotkey, prompt, updated_at)
SELECT 2, 'prompt_engineer', 'Prompt Engineer', 'option+2',
       'Turn this text into a clear, effective prompt for an AI assistant. Preserve every requirement, constraint, fact and requested output format. Remove ambiguity and unnecessary wording. Do not answer or carry out the prompt.',
       0
WHERE NOT EXISTS (SELECT 1 FROM transforms WHERE builtin_key = 'prompt_engineer');
