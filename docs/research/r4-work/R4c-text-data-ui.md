# R4c: Parts bin, text pipeline, data and UI building blocks

Deliverable R4, part c. Concrete, license-checked open-source parts for the text pipeline, diff/undo, data layer, Hub UI, onboarding and i18n/a11y gaps named in R2 (`docs/research/R2-gap-analysis.md`). The goal is to write as little code as possible. Each need gets one recommended part, or a short "write ourselves" item when the need is too product-specific to adopt.

## 0. Method and tags

**Sources read in the repo (opened for this pass):** `package.json`; `src-tauri/Cargo.toml` and `Cargo.lock`; `src-tauri/src/audio_toolkit/text.rs` (whole file); `src-tauri/src/managers/history.rs:1-60`, `:160-270`; `src-tauri/src/managers/transcription.rs:1600-1662`; `src/App.css:1-60`; `src/components/ui/` (Dialog, Tooltip, Dropdown headers); `src/components/onboarding/` layout and `viewTransitions.ts`; `src/i18n/index.ts:80-130`; `scripts/check-translations.ts:1-60`. Also R2 §1-4 and the Area B, C and D tables, plus E-64-E-68, E-80, E-81, A-14, A-16 and A-47.

**External checks (fetched 2026-10-28):**
- npm: registry metadata (license field of the latest version, release date, React peer range) and npm weekly downloads.
- crates.io: API metadata (license, last update, total and recent downloads).
- GitHub: API for stars, last push, archived flag and the LICENSE file. For any package where the npm and GitHub licenses could disagree, I read the LICENSE file.
- bundlephobia: min and gzip size of the whole package. This is an upper bound, because tree-shaking is not counted.

**Clones read:** shallow clones under `scratchpad/clones/r4c-*`: text-processing-rs, text2num-rs, wetext-rs, hyprwhspr-rs, jsdiff, similar, react-activity-calendar, driver.js, rusqlite_migration. Nothing was built, installed or run. GPL/AGPL projects (espanso, monkeytype, shepherd, intro.js, i18n-ai-translate) were checked only through their LICENSE file or docs.

**Tags:** **(O)** observed: I read the file, metadata or doc myself. **(I)** inferred from code or docs, not run. **(U)** unknown. "wk dl" = npm downloads in the last week. "S/M/L" = integration cost: S ≤ 2 days, M ≤ 2 weeks, L > 2 weeks.

**License policy applied:** MIT, Apache-2.0, BSD, ISC, Zlib, Unlicense, OFL-1.1 (fonts only) are fine. GPL, AGPL and LGPL are rejected (docs only). Three edge cases are flagged for the owner:
- **Unicode-3.0** (ICU4X). It is a permissive license, and Handy already ships Unicode-3.0 crates (`icu_normalizer`, `icu_properties` etc. in `Cargo.lock`, pulled in by url/idna) (O).
- **MPL-2.0.** DOMPurify is dual-licensed (MPL-2.0 OR Apache-2.0), so we take it under Apache-2.0 (O). `@axe-core/playwright` is MPL-2.0 only and would be a dev-only test tool that never ships.
- Packages with **no LICENSE file in the published package or repo** are rejected for anything we would ship.

**Baseline facts that change R2's assumptions (O):**
- `rusqlite_migration = "2.3"` is **already a direct dependency** (`src-tauri/Cargo.toml:45`; used at `history.rs:5`, `:22-36`, `:165`). A second fork database can reuse it with zero new crates.
- `unicode-segmentation 1.12.0` (via tao/keyboard-types), `aho-corasick 1.1.4` (via regex) and `zip 4.6.1` (via tauri-plugin-updater) are already compiled into the app as transitive dependencies (`Cargo.lock`). Making them direct dependencies adds no new code to the binary, as long as we pin compatible versions.
- `@tauri-apps/plugin-sql` is listed in `package.json`, but nothing under `src/` imports it, and there is no `tauri-plugin-sql` in `Cargo.toml`. It is a leftover from the move to rusqlite (`history.rs:197-245`).
- Handy's UI already has hand-built `Dialog` (focus trap), `Tooltip`, `Dropdown` (a value select, not a menu), sonner toasts and lucide icons. Onboarding animation is CSS plus the View Transitions API plus a scripted "beat" state machine (`onboarding/viewTransitions.ts`, `screens/Demo.tsx`), with no animation library.

---

## 1. Text pipeline

### 1.1 Handy baseline (what to extend, not replace)

- **Pipeline order** (`transcription.rs:1600-1662`, O): script conversion → `apply_custom_words` (fuzzy; skipped when words went to the decoder as vocabulary) → `remove_filler_words` → `normalize_transcription_output`. All of it runs inside `fail_open_text_transform`.
- **Custom-word engine** (`text.rs:151-220`, O): Levenshtein (strsim) + Soundex (natural) on 1-3-word n-grams. It does not join n-grams across punctuation, preserves case and punctuation, and handles "R and D" → "R&D". It is ASCII-only by design (`text.rs:34-60`). It is a good engine for *fuzzy single terms*. It has no notion of exact "from → to" pairs and returns only a `String` (no counts).
- **Newlines** (AF-06, re-confirmed, O): `apply_custom_words` (`text.rs:163`, `:219`) and `collapse_stutters` (`:331`, `:364`) both use `split_whitespace().join(" ")`, and `MULTI_SPACE_PATTERN` `\s{2,}` (`:326`, `:474`) folds any newline pair into a space. Every stage flattens lines.

### 1.2 Candidates

| Candidate | License (evidence) | Popularity / maintenance | Fit and files touched | Cost | Risks | Verdict |
|---|---|---|---|---|---|---|
| **text-processing-rs** (Rust port of NVIDIA NeMo ITN+TN) https://github.com/FluidInference/text-processing-rs | Apache-2.0: LICENSE + NOTICE (credits NeMo, Apache-2.0); THIRD-PARTY-LICENSES lists rustfst and flate2, both MIT/Apache (O) | crates.io 0.2.2 (2026-04-27), repo at 0.3.0, last push 2026-10-27; 48 stars; 7.8k total downloads (O) | Pure-Rust rule engine by default; the optional `fst-engine` feature adds ~6.9 MB of grammars (O, `du grammars`). Sentence mode: `normalize_sentence_lang(text, "en"/"fr"/"es"/"de"/"zh"/"hi"/"ja")` (O, README). Numbers, money, dates, time, measures, ordinals, electronic, phone (O, `src/itn/en/`). Touches `transcription.rs:1600-1662` (new stage after fillers) and `Cargo.toml`. | M (en), M+ per extra language for QA | (1) It tokenizes with `split_whitespace` and rebuilds with `" "` (`lib.rs:1182`, `pretokenize`; O), so it **destroys newlines**: run it per line. (2) Sentence mode turns spoken punctuation into symbols with a space before them: `"yes comma I agree"` → `"yes , I agree"` (test `en_tests.rs:426`, O). Also, single words like `period`, `dot`, `dash`, `plus`, `hash`, `percent` are in the punctuation tagger (`itn/en/punctuation.rs`, O), so "he said period" → "he said ." (test `:422`, O). False-positive risk is real; we would need a filtered tagger set or a post-pass. (3) `custom_rules` is a **global** `RwLock<Vec<..>>` (`custom_rules.rs:13-16`, O), so do not use it for per-user rules. (4) Small project: bus factor of one org. (5) Most of Handy's ASR models already emit digits (I), so the gain is mainly for spoken-form models and the "formats for you" promise (D-14). | **Adopt (later milestone, behind a setting), en first.** Use only its number/money/date/time/measure taggers; do our own punctuation and voice commands (§1.5). |
| text2num-rs https://github.com/allo-media/text2num-rs | MIT: LICENSE "Copyright (c) 2021-2024 Groupe Allo-Media" (O) | 2.8.0, 2026-08-21; 9 stars; 62k downloads (O) | Numbers only (cardinal, ordinal, decimal) in da, nl, en, es, pt, de, fr, it (O, README). Has a **threshold to leave isolated small numbers as words** ("one of the…"; O, README). Token-stream API `replace_numbers_in_stream` (`word_to_digit.rs:508`, O) can keep separators (I). | S | No dates, currency or time; languages differ from text-processing-rs (it has it/pt/nl/da, no zh/ja/hi). | **Adapt** as the fallback/complement for it, pt, nl, da numbers, or as the *only* ITN if we want numbers without money/dates. |
| NVIDIA NeMo-text-processing https://github.com/NVIDIA/NeMo-text-processing | Apache-2.0 (O, GitHub API) | 515 stars, pushed 2026-10-28 (O) | Python + Pynini/OpenFst grammars; not embeddable in a Tauri app without Python (I). | – | Runtime weight. | **Study only** (the source of truth for text-processing-rs fixtures). |
| WeTextProcessing / wetext / wetext-rs https://github.com/wenet-e2e/WeTextProcessing, https://github.com/SpenserCai/wetext-rs | Apache-2.0 (O, all three) | WeTextProcessing 832 stars; wetext-rs 0.1.2, 144 downloads, last push 2025-12-30 (O) | zh/en/ja TN+ITN via FSTs. wetext-rs needs FST weight files fetched separately (README "FST Weight Files / Download Options"; `data/` is 16 KB, O). | M | Extra asset download; TTS focus; tiny Rust port. | **Study only**; consider it only if zh/ja ITN becomes a priority (text-processing-rs already covers zh/ja). |
| sherpa-onnx ITN (rule FSTs) https://github.com/k2-fsa/sherpa-onnx | Apache-2.0 (O) | 15.6k stars (O) | Would mean a whole second ASR runtime just for ITN. | L | Out of proportion. | **Reject.** |
| **aho-corasick** https://github.com/BurntSushi/aho-corasick | Unlicense OR MIT (O, crates.io) | 1.1.5; 1.3 B downloads; already in `Cargo.lock` 1.1.4 via regex (O) | Multi-pattern, leftmost-longest matching for snippet triggers, exact dictionary pairs and voice commands in one pass. `ascii_case_insensitive` only (I, docs), so we lowercase and keep a byte map ourselves. Touches new `src-tauri/src/fork/text/{rules,snippets}.rs` and `Cargo.toml` (direct dep). | S | Unicode case folding is up to us. | **Adopt.** |
| daachorse https://github.com/daac-tools/daachorse | MIT OR Apache-2.0 (O) | 5.1.0, 2026-10-18 (O) | Charwise double-array Aho-Corasick; good for CJK; used by text2num (O, `text2num/Cargo.toml`). | S | A second matcher crate. | **Study / fallback** if CJK trigger performance matters. |
| strsim (present) https://github.com/rapidfuzz/strsim-rs | MIT (O) | Already a direct dependency, 0.11 (O) | Already powers fuzzy custom words. Reuse it for an optional fuzzy snippet trigger (normalized Levenshtein ≤ threshold on windows the length of the trigger). | – | – | **Keep.** |
| rapidfuzz-rs | MIT (crates) / Apache-2.0 (GitHub) mismatch (O) | 0.5.0, last release 2023-12 (O) | Faster fuzzy scoring. | – | License metadata mismatch, stale. | **Reject** (strsim suffices). |
| espanso https://github.com/espanso/espanso | **GPL-3.0** (O, LICENSE) | 14.7k stars (O) | Text expander. Docs-only lessons (O, https://espanso.org/docs/matches/basics/): `word: true` (expand only between word separators), `left_word`/`right_word`, `propagate_case`, `uppercase_style: capitalize_words`, configurable `word_separators`. | – | GPL. | **Study docs only.** We copy the *concepts*: word-boundary triggers and case propagation. |
| hyprwhspr-rs (MIT Rust dictation app) https://github.com/better-slop/hyprwhspr-rs | MIT (O, LICENSE) | 0.3.33, 2026-08-30; 35 stars (O) | Working reference for exactly our pipeline (`src/text/normalize.rs`, O). It flattens newlines first, then runs ITN (text-processing-rs 0.2.2), then **voice commands after ITN** ("new line" → `\n`, "tab" → `\t`), then per-step cleanups (trim spaces around newlines, capitalize after period), then **word overrides with per-step change counts** (`StepOutput.change_count`). It guards text-processing-rs's global `custom_rules` with a mutex (`:11`, `:278-292`). | – | "tab" as a bare command word is risky (I). | **Study only** (MIT, so small pieces may be copied with attribution). The per-step change-count design is the model for C-52 "fixes made". |
| **unicode-segmentation** https://github.com/unicode-rs/unicode-segmentation | MIT OR Apache-2.0 (O, crates.io + LICENSE-APACHE) | 1.13.3; 641 M downloads; 1.12.0 already in `Cargo.lock` (O) | UAX #29 word and grapheme boundaries with byte offsets (`unicode_word_indices`): tokenizer for triggers, rules and word counts. Touches `fork/text/*.rs`, `Cargo.toml`. | S | UAX #29 does not split Thai/Lao/Khmer/Myanmar runs into words, and treats each Han ideograph as its own segment (I, from the spec https://unicode.org/reports/tr29/). | **Adopt** for tokenizing and for word counts in space-delimited scripts. |
| icu_segmenter (ICU4X) https://github.com/unicode-org/icu4x | **Unicode-3.0** (O, LICENSE "UNICODE LICENSE V3"; crates.io) | 2.4.0, 2026-10-22 (O) | Dictionary/LSTM word segmentation for Thai, Lao, Khmer, Myanmar, Chinese and Japanese (I, ICU4X docs). | M | Unicode-3.0 (permissive; precedent already in tree). Size of the compiled data with dictionaries is **unknown (U)**; measure before adopting. | **Adapt only if** DEC-11 decides Thai/Khmer word counts matter. Otherwise use the CJK char rule (§1.6). |
| Intl.Segmenter (built into WebKit) | n/a (platform) | Safari 14.1+ (I, https://developer.mozilla.org/docs/Web/JavaScript/Reference/Global_Objects/Intl/Segmenter) | Frontend-only word segmentation including Thai/CJK dictionaries (I). Usable for UI counters (snippet 4,000-char counter, typing test) and as jsdiff's `intlSegmenter`. | – | Stats must be computed in Rust at save time (AF-07), so it cannot be the source of truth. Behavior may differ by WebKit version (O, jsdiff README warns the same). | **Use where the UI needs it**; never for stored stats. |
| stopwords-iso https://github.com/stopwords-iso/stopwords-iso | MIT (O) | 1.1.0 (2020), 1.3 M wk dl (O) | Stop lists for "most used word" (C-57). | S | Stale but static data. | **Adopt (data only)** when C-57 is built. |
| chrono-english / interim | MIT (O) | 0.2.x (O) | Parse "next Tuesday" into a `DateTime`. That is not ITN (we need written-form text, not a timestamp). | – | Wrong tool. | **Reject.** |

### 1.3 Newline-safe pipeline (proposal; closes AF-06, D-210, enables D-53, D-56)

Rule: **no stage may call `split_whitespace().join(" ")` on the whole text.** Two mechanical changes make Handy's existing stages line-safe, and the new stages follow the same contract (I, from reading `text.rs`):

1. Wrap the existing calls in `post_process_transcription_text` in a per-line helper (`text.split('\n').map(stage).collect::<Vec<_>>().join("\n")`). This keeps `text.rs`, an upstream file, untouched: the helper lives in a fork module and is called from one place in `transcription.rs`.
2. Replace `MULTI_SPACE_PATTERN` use with "collapse `[ \t]{2,}` within a line; collapse 3+ `\n` to 2; trim each line". This is one fork function.

Proposed order (each step fail-open, like `fail_open_text_transform`):

| # | Stage | Part | Why here |
|---|---|---|---|
| 1 | Script conversion (existing) | – | Rules must match in the user's script (existing comment `transcription.rs:1630-1632`, O). |
| 2 | **Exact dictionary "from → to"** | aho-corasick, own code | R2 M-03: before fuzzy, outputs protected. Protection: replace each hit with a Private Use Area placeholder (U+E000+n). The fuzzy matcher only considers alphanumeric keys (`build_match_key`, `text.rs:18-23`, O), so placeholders are skipped (I). Restore at the end. |
| 3 | Fuzzy custom words (existing `apply_custom_words`), per line | – | Unchanged engine. |
| 4 | Fillers (existing), then line-safe normalize | – | – |
| 5 | ITN (optional, per language, per line) | text-processing-rs (+ text2num) | After fillers so "um" does not break number spans. Skip it when the model already writes digits (I). |
| 6 | **Voice commands** ("new line", "new paragraph", spoken punctuation, "scratch that") | aho-corasick, own table per language | After ITN, as in hyprwhspr-rs (O). This is the first stage allowed to *create* `\n`. |
| 7 | LLM cleanup (D-01), when on | – | The prompt must say "keep line breaks". If the model drops them, accept or keep the deterministic text (D-12 guards). |
| 8 | **Snippet expansion** | aho-corasick, own code | Last, after the LLM (AF-06), so multi-line bodies are never normalized or rewritten. |
| 9 | Paste | existing | Direct typing turns `\n` into Return (AF-06). For chat apps, convert to Shift+Return or force clipboard paste when the text contains `\n` (I). |

Each fork stage returns `(String, change_count)`, like hyprwhspr-rs's `StepOutput`. That gives C-52 "fixes made" its counts for free.

### 1.4 Snippet trigger matching (D-53; write ourselves on top of aho-corasick)

Design (I):
- Normalize the transcript into word tokens with `unicode_word_indices` (keep byte spans), lowercase them, and drop punctuation. Join the tokens with single spaces into a "match string", with a map from each match-string offset back to an original byte span.
- Build one `AhoCorasick` with `MatchKind::LeftmostLongest` over the normalized triggers.
- Accept a hit only on token boundaries (espanso's `word: true` concept).
- Replace the original span, including trailing punctuation the ASR added ("my calendly link." → body + "."). Propagate case only for single-line plain bodies (espanso `propagate_case` concept).
- CJK triggers have no spaces: match them on the raw char stream (daachorse charwise, or aho-corasick on the lowercased string).
- Optional fuzzy fallback: strsim normalized Levenshtein on token windows of the trigger's length, behind a stricter threshold than custom words.

Cost: S-M. ~150 lines plus tests. Files: new `src-tauri/src/fork/text/snippets.rs`, call site in `actions.rs` after post-processing (the LLM output), snippets store (§3).

### 1.5 Dictionary "from → to" (D-48, C-58, C-62, C-68): extend, do not replace

Keep `apply_custom_words` for single-term fuzzy entries (C-67 vocabulary prompting depends on `custom_words: Vec<String>`; R2 notes replacement rules must not be fed as vocabulary). Add a separate rule list in the fork store (§3), applied as stage 2 above with aho-corasick and whole-word boundaries. Case handling should reuse the *idea* of `preserve_case_pattern` (`text.rs:223-235`, O), which is private, so we reimplement ~10 lines in the fork module. Apostrophe stripping in `CustomWords.tsx` ("O'Brien", C-58) is a UI fix, not a library question. Cost M (as R2).

### 1.6 Word counting (DEC-11, C-30, C-31, B-29)

Recommendation (I), computed in Rust at save time and stored in `daily_stats`:
- Count `unicode_words()` tokens.
- Then, for every token made only of Han, Hiragana or Katakana, count its chars instead of 1. This is the common word-processor convention for CJK (I).
- Thai/Lao/Khmer/Myanmar get char-count / 4 as an estimate, until icu_segmenter is evaluated (U: no measured Wispr rule).

Store `word_count` and `char_count` per entry so the rule can change later without losing history. Cost S.

### 1.7 Voice commands (D-56; not Wispr parity)

No permissive library gives a multi-language voice-command table (searched crates.io for "itn", "inverse text normalization", "text normalization", "words to numbers"; O). Write ourselves: a per-language table of phrases (`new line` → `\n`, `new paragraph` → `\n\n`, `comma`, `period`, `question mark`, ...). Require the phrase as its own token span and the match **at clause edges** (after a pause/punctuation or at the end of the utterance), so "the period of time" stays (I).

"Scratch that": within one utterance only, delete back to the previous sentence boundary. Cross-utterance undo needs D-31-style ⌘Z and is out of scope (I).

---

## 2. Diff and undo (D-40, D-41, C-42, C-52, DEC-19)

| Candidate | License (evidence) | Popularity / maintenance | Fit and files touched | Cost | Risks | Verdict |
|---|---|---|---|---|---|---|
| **similar** (Rust) https://github.com/mitsuhiko/similar | Apache-2.0 (O, LICENSE) | 3.2.1, 2026-10-24; 233 M downloads; 1.3k stars (O) | `TextDiff::from_words` / `from_unicode_words` (feature `unicode`, which uses unicode-segmentation, already in tree) and `iter_inline_changes` (feature `inline`) (O, `src/text/mod.rs:1070-1112`, `Cargo.toml` features). Default features `std`, `text` (O). MSRV 1.85 (O). Run it in Rust at save time: store `fix_count` (C-52) and return `[{tag, text}]` ops to the UI for the "N Changes" card (D-40). Files: new `fork/text/diff.rs`, `history.rs` save path (via fork DB), a new command in the fork command module (regenerate `bindings.ts`). | S | Word-level output on long texts: use the `_deadline` variants (O, API exists). | **Adopt** (one diff engine for counts and display). |
| dissimilar https://github.com/dtolnay/dissimilar | Apache-2.0 (O) | 1.0.11; 67 M downloads (O) | Character-level diff-match-patch port with semantic cleanup. | S | Char-level; not word tokens. | Reject (similar covers words). |
| imara-diff | Apache-2.0 (O) | 0.2.0 (O) | Fast line/token diff for large inputs. | S | Lower-level API. | Reject. |
| **jsdiff (`diff`)** https://github.com/kpdecker/jsdiff | BSD-3-Clause (O, LICENSE) | 9.0.0, 2026-04-13; 155 M wk dl; 9.2k stars; 7.9 KB gz (O) | `diffWords` with `intlSegmenter` option (O, `src/diff/word.ts:67`, README) gives CJK word diff via WebKit's ICU; `diffWordsWithSpace` keeps newlines as tokens (O, README). Files: `package.json`, the new diff card component. | S | Intl.Segmenter results vary by engine (O, README warns). | **Fallback**: use it only if the diff is computed purely in the UI (e.g. Scratchpad AI sparkle, D-209). |
| diff-match-patch / @sanity/diff-match-patch / fast-diff | Apache-2.0 (O) | dmp 1.0.5 last release 2020 (O); sanity fork 3.2.1 2026 (O); fast-diff 1.3.0 2023 (O) | Char-level. | – | Wrong granularity; original is stale. | Reject. |

**Rendering strike/insert (write ourselves, S):** map ops to `<del>`/`<ins>` with Tailwind classes on existing tokens (`--color-error-wash` with `line-through` for deletions, `--color-moss-wash` for insertions; `theme.css` already defines both, O via `App.css:8-28`). Add a visually hidden "removed:"/"added:" prefix, because VoiceOver does not announce `<del>`/`<ins>` by default (I). "N Changes" = count of change *groups*: a delete immediately followed by an insert counts as one change (I; Wispr's rule is unknown, U).

**Undo AI edit (C-42):** no library. Both texts are already stored (`history.rs:33-35`, O). Add `shown_variant` plus a copy/re-paste action per DEC-19.

---

## 3. Data (AF-07, DEC-07, DEC-08, C-36, C-46, C-64, C-65, C-208, C-209, E-66, E-67)

| Candidate | License (evidence) | Popularity / maintenance | Fit and files touched | Cost | Risks | Verdict |
|---|---|---|---|---|---|---|
| **rusqlite 0.37 `bundled`** (present) https://github.com/rusqlite/rusqlite | MIT (O) | Latest 0.41.1 (2026-10-28); 4.5k stars (O) | Already there; bundled SQLite 3.50.2 has FTS5 and trigram (R2 C-36, verified there). | – | Upgrading rusqlite is an upstream-wide change; don't. | **Keep.** |
| **rusqlite_migration 2.3** (present) https://github.com/cljoly/rusqlite_migration | Apache-2.0 (O, LICENSE) | 2.6.0 (2026-05-28), pushed 2026-10-26 (O) | Uses `PRAGMA user_version` per database file (O, `lib.rs:934-965`). A **separate fork DB** gets its own independent version counter, so there are no conflicts with upstream's `MIGRATIONS` (AF-07, AF-13). Files: new `src-tauri/src/fork/db.rs` with its own `static MIGRATIONS: &[M]`. | S | Must not bump the crate version independently of upstream's pin (shared dep). | **Adopt (reuse).** |
| refinery https://github.com/rust-db/refinery | MIT (O) | 0.10.0 (O) | Migration table plus SQL files. | S | A second migration system next to rusqlite_migration. | Reject. |
| tauri-plugin-sql (JS + Rust) | MIT OR Apache-2.0 (O) | 2.6.0 (O) | Frontend SQL. | – | Handy deliberately moved off it (`history.rs:197-245`, O); the JS package is now unused. | Reject. Remove the unused JS dep only as part of a cleanup, if wanted. |
| **csv** (Rust) https://github.com/BurntSushi/rust-csv | Unlicense/MIT (O) | 1.4.0; 278 M downloads (O) | Export inside the Rust "download your data" zip (E-66). Files: `fork/export.rs`. | S | Excel formula injection on export: prefix cells starting with `= + - @` with `'` (I). | **Adopt** for export. |
| **papaparse** (JS) https://github.com/mholt/PapaParse | MIT (O) | 5.7.0, 2026-08-24; 19.7 M wk dl; 13.6k stars; 6.8 KB gz (O) | Import wizard preview (C-64, C-71): parse in the webview, dedupe and preview, then send rows to a Rust command. Files: `package.json`, new import dialog. | S | 3 MB cap (R1) is enforced by us. | **Adopt** for import. |
| **zip** https://github.com/zip-rs/zip2 | MIT (O) | Latest 9.1.3; 4.6.1 already in tree via updater (O) | Export bundle (history JSON/CSV, audio, notes, dictionary, snippets). | S | Pin `4.x` to share the compiled copy, or accept a second version (I). | **Adopt.** |
| ammonia (Rust HTML sanitizer) https://github.com/rust-ammonia/ammonia | MIT OR Apache-2.0 (O) | 4.2.4, 2026-10-17 (O) | Only if rich snippets (DEC-12): sanitize stored HTML before writing HTML to the clipboard. | S | – | **Adopt only with rich snippets.** |
| html2text (Rust) | MIT (O) | 0.17.4 (O) | Plain-text fallback of rich snippets for the plain clipboard flavor. | S | – | **Adopt only with rich snippets.** |
| serde_rusqlite, r2d2_sqlite | MIT/Apache (O for serde_rusqlite) | – | Convenience. | – | Not needed. | Reject. |

**Patterns (write ourselves; all inferred from SQLite docs and the code):**

- **Separate fork DB (DEC-08 → recommend a separate file):** e.g. `fork.db` next to `history.db`, holding `daily_stats`, `entry_meta` (history id → audio_ms, word_count, char_count, app_bundle_id, fix_count, shown_variant, asr_text), `dictionary_rules`, `snippets`, `notes`, `notifications`, `transforms`. Upstream `history.db` stays untouched, so upstream merges and upstream builds keep working.
  - Cost: two files to export and delete, and no cross-file foreign keys.
  - SQLite triggers cannot reference tables in another attached schema (non-TEMP triggers; I, https://sqlite.org/lang_createtrigger.html). So **history FTS is application-maintained**: on `save_entry`/retry/delete, the fork code writes `history_fts(rowid = history id, text)` in `fork.db`. No triggers in `history.db`.
- **Connection settings (C-208):** one long-lived `Connection` per DB behind a `Mutex`, opened with `PRAGMA journal_mode=WAL; PRAGMA busy_timeout=5000; PRAGMA synchronous=NORMAL; PRAGMA foreign_keys=ON`. Do not set pragmas inside migrations (O, rusqlite_migration docs `lib.rs:300-339`).
  - For `history.db`, adding WAL changes a file upstream builds also open. WAL is persistent but backward-compatible (I). Delete-all must also remove the `-wal`/`-shm` files.
- **FTS5 search (C-36, C-216):** `CREATE VIRTUAL TABLE ... USING fts5(text, tokenize='trigram')` for substring search in any script. Queries shorter than 3 chars fall back to `LIKE` (R2). Notes use the same pattern.
- **Typed columns (C-209):** in `fork.db`, normal `INTEGER` columns through migrations; nothing is added to `history.db`'s `ADDED_COLUMNS`.
- **Stats that survive retention (C-30-C-32, DEC-07):** `daily_stats(day TEXT PRIMARY KEY, words, dictations, audio_ms, fixes)`, upserted at save. Retention deletes history rows only.
- **Export (E-66):** Rust command that reads both DBs, writes CSV/JSON plus recordings into a zip at a path from `tauri-plugin-dialog` (already a dependency, `package.json`, O).
- **Delete all (E-67):** stop managers; close connections; delete `history.db*`, `fork.db*`, `recordings/`, the settings store and logs; offer separate removal of HF-cache models (AF-19); relaunch.

---

## 4. UI kit and patterns for the Hub

Placement rule (AF-13): new primitives go in a fork folder (e.g. `src/fork/ui/`), not into upstream's `src/components/ui/`. Keep Handy's own `Dialog` and `Tooltip` (they work, focus-trapped, reduced-motion aware; O) instead of swapping them.

### 4.1 Headless components, palette, toasts

| Candidate | License (evidence) | Popularity / maintenance | React 18 | Size (gz, whole pkg) | Fit | Cost | Risks | Verdict |
|---|---|---|---|---|---|---|---|---|
| **Radix Primitives** (`@radix-ui/react-dropdown-menu`, `-popover`, `-tabs`, `-context-menu`, `-collapsible`) https://github.com/radix-ui/primitives | MIT (O, npm + GitHub) | dropdown-menu 2.2.1, 2026-10-12, 56.6 M wk dl; repo 19.4k stars, pushed 2026-10-14 (O) | Yes, peer `^16.8 \|\| ... ^19` (O) | dropdown 30 KB, popover 23 KB, tabs 9 KB (O, bundlephobia; shared internals deduplicate) | Row ⋮ menu (C-41), Help popover (C-13), notification panel (C-17), Insights/Dictionary/Style tabs (C-50, C-59, D-18), sidebar collapse (C-14). Keyboard roving focus and RTL built in (I, docs). | S per component | Unstyled; we style with Handy tokens. | **Adopt** (one package per need). |
| shadcn/ui https://github.com/shadcn-ui/ui | MIT (O) | 126.8k stars (O) | Its docs say new components target React 19 and dropped `forwardRef` (O, https://ui.shadcn.com/docs/tailwind-v4). React 18 cannot pass `ref` as a prop (I), so copied components need forwardRef restored. | – | Source to copy, built on Radix + Tailwind. | – | React 19 drift. | **Study only**: copy class patterns into our Radix wrappers, not the CLI output. |
| Base UI https://github.com/mui/base-ui | MIT (O) | 1.9.0, 21.5 M wk dl, pushed 2026-10-28 (O) | Yes `^17\|\|^18\|\|^19` (O) | 166 KB whole pkg (O) | Same role as Radix, newer. | S | Fine, but Radix has more examples and cmdk builds on it. | Alternative (reject for now). |
| React Aria Components https://github.com/adobe/react-spectrum | Apache-2.0 (O) | 1.22.1, 6.1 M wk dl (O) | Yes (O) | 279 KB whole pkg (O) | Best a11y and i18n, including date formatting. | M | Heavier, different idiom. | Reject (keep for reference on a11y patterns). |
| Ark UI (Zag) https://github.com/chakra-ui/ark | MIT (O) | 5.39.3, 1.2 M wk dl (O) | `>=18` (O) | 290 KB whole pkg (O) | State-machine components. | M | Bigger, smaller community. | Reject. |
| Headless UI https://github.com/tailwindlabs/headlessui | MIT (O) | 2.2.10, last release 2026-04 (O) | `^18\|\|^19` (O) | – | Fewer components (no tooltip/context menu). | S | Narrower. | Reject. |
| cmdk https://github.com/pacocoursey/cmdk | MIT (O) | 1.1.1 (2025-03-14), last push 2025-10-29; 38.9 M wk dl (O) | `^18\|\|^19` (O) | 16.7 KB (O) | ⌘K palette over pages, settings and history. **Not in R1** (no Wispr palette observed). | S | Slower release cadence. | **Optional** (adopt only if R5 wants a palette). |
| kbar | MIT (O) | 1.0.0 (O) | Yes (O) | – | Same need. | S | – | Reject (cmdk preferred). |
| **sonner** (present) https://github.com/emilkowalski/sonner | MIT (O) | 2.0.8, 47 M wk dl (O) | Yes | – | Already used (`App.tsx`). Its action button gives "Deleted · Undo" for C-44 with a delayed purge. | – | – | **Keep.** |

### 4.2 Charts and streak calendar (C-50-C-57)

| Candidate | License | Popularity / maintenance | React 18 | Size (gz) | Fit | Cost | Verdict |
|---|---|---|---|---|---|---|---|
| **react-activity-calendar** https://github.com/grubersjoe/react-activity-calendar | MIT (O, LICENSE "Copyright (c) 2021 Jonathan Gruber") | 3.2.1 (2026-07-07), pushed 2026-10-25; 67k wk dl (O) | `^18\|\|^19` (O) | 6 KB (O); deps `@floating-ui/react`, `date-fns`, both MIT (O) | Streak heatmap (C-55) with `labels` (i18n), `weekStart`, `colorScheme`/`theme`, `renderBlock`, tooltips (O, `ActivityCalendar.tsx:80-227`). Data comes straight from `daily_stats`. | S | **Adopt.** |
| react-calendar-heatmap (R2 hint) | MIT (O) | 1.10.0, last push 2025-02 (O) | peer `>=0.14` (O) | – | Older, unmaintained-looking. | S | Reject. |
| @uiw/react-heat-map | MIT (O) | 2.3.4 (O) | Yes | – | Fewer i18n hooks (U). | S | Reject. |
| **Recharts** https://github.com/recharts/recharts | MIT (O) | 3.10.1, 54 M wk dl, 27.7k stars (O) | Yes (O) | 150 KB whole pkg (O) | Usage-by-category bars, WPM over time (C-50, C-54). Declarative, least code. Size is fine for a locally loaded webview if the Insights page is lazy-loaded (I). | S | **Adopt.** |
| visx https://github.com/airbnb/visx | MIT (O) | 4.0.0 (O) | `^18\|\|^19` (O) | Modular, small (O) | Lower-level: more code per chart. | M | Alternative. |
| nivo `@nivo/calendar` | MIT (O) | 0.99.0, last release 2025-05 (O) | Yes | – | Heavier ecosystem. | S | Reject. |
| html-to-image https://github.com/bubkoo/html-to-image | MIT (O) | 1.11.13 (2025-02), pushed 2026-10 (O) | n/a | – | "Save insights as image" (C-56). WebKit `foreignObject` quirks with fonts are possible (U). | S | **Adopt** when C-56 is built. |

### 4.3 Rich text for snippets and notes (C-70, C-74, DEC-12)

| Candidate | License | Popularity / maintenance | React 18 | Size | Fit | Cost | Risks | Verdict |
|---|---|---|---|---|---|---|---|---|
| **TipTap 3** (`@tiptap/react`, `@tiptap/starter-kit`, `@tiptap/extension-link`, `@tiptap/markdown`) https://github.com/ueberdosis/tiptap | MIT (O, LICENSE.md "Copyright (c) 2025, Tiptap GmbH"; npm MIT for all four packages) | 3.32.1 (2026-10-28), 22 M wk dl, 38.9k stars (O) | `^17\|\|^18\|\|^19` (O) | react 9 KB plus ProseMirror (U total) | Bold, italic, lists, link, 4,000-char counter; HTML or Markdown output. Paid "Pro" extensions exist but are not needed (I). | M (plus AF-17 HTML paste) | Pro/cloud upsell in docs; HTML must be sanitized (ammonia, DOMPurify). | **Adopt only if DEC-12 = rich.** |
| Lexical https://github.com/facebook/lexical | MIT (O) | 0.53.0 (2026-10-28), 6.9 M wk dl (O) | `>=18.x` (O) | 69 KB core (O) | Same need. | M | Pre-1.0 API churn (I). | Alternative. |
| Plate https://github.com/udecode/plate | MIT root LICENSE, with "unless otherwise specified in a LICENSE file within an individual package directory" (O) | 53.x (O) | `>=18` (O) | – | Slate-based, shadcn-styled. | M | Per-package licenses must be audited. | Reject. |
| Milkdown https://github.com/Milkdown/milkdown | MIT (O) | 7.23.0 (O) | n/a | – | Markdown-first. | M | Smaller ecosystem. | Reject. |
| DOMPurify https://github.com/cure53/DOMPurify | MPL-2.0 OR Apache-2.0 (O); take Apache-2.0 | 3.4.17, 79 M wk dl (O) | n/a | – | Sanitize HTML before rendering snippet previews. | S | – | **Adopt only with rich snippets.** |

**Recommendation for DEC-12:** plain text in v1. The existing `Textarea` keeps newlines; snippet bodies are already multi-line-safe after §1.3. Rich text costs an editor, HTML on both paste paths and a full clipboard snapshot (AF-17).

### 4.4 Lists, animation, icons, fonts

| Candidate | License | Popularity / maintenance | React 18 | Size (gz) | Fit | Cost | Verdict |
|---|---|---|---|---|---|---|---|
| **TanStack Virtual** https://github.com/TanStack/virtual | MIT (O) | 3.14.23 (2026-10-26), 32 M wk dl (O) | Yes (O) | 10 KB (O) | Virtualized history with variable row heights (`measureElement`), on top of the existing cursor paging (C-207). Files: `HistorySettings.tsx` or the new Dictation home list. | S | **Adopt.** |
| react-virtuoso | MIT (O, LICENSE in the npm package; the GitHub API reports no license) | 4.18.19 (O) | Yes | – | Easier dynamic heights. | S | Alternative. |
| react-window | MIT (O) | 2.5.1 (O) | `^18\|\|^19` | – | Fixed-size focus. | S | Reject. |
| Motion (`motion`) https://github.com/motiondivision/motion | MIT (O) | 14.7.1, 28 M wk dl (O) | `^18\|\|^19` (O) | 58 KB full (O); smaller with `LazyMotion` (I) | Layout/presence animations (banner dismiss, list insert, onboarding scenes). Handy currently uses CSS + View Transitions, which are disabled on Linux for WebKitGTK reasons (`viewTransitions.ts`, O). | S | **Optional.** Keep CSS/View Transitions first; adopt Motion only where CSS cannot do exit/layout animation. |
| **lucide-react** (present) https://github.com/lucide-icons/lucide | ISC (O, LICENSE) | Installed ^0.542; latest 1.63.0 (O) | Yes | tree-shaken | Icons. | – | **Keep** (no major upgrade needed). |
| **Fonts** (Google Fonts `ofl/*/OFL.txt` read; Fontsource packages `@fontsource-variable/*` declare OFL-1.1, O) | OFL-1.1 (O) | – | – | woff2 subsets | Serif display headings (C-21). | S | See below. |

**Font recommendation (C-21, C-19):**
- **Display serif: Newsreader** (variable 200-800, italic, latin + latin-ext + vietnamese; O, Fontsource API). It is close to the warm editorial serif in R1. Wispr's exact face is unknown (U).
- **Instrument Serif** (only 400 + italic, latin only; O) is a lighter alternative for very large headings.
- **Source Serif 4** adds Cyrillic and Greek (O). Use it as the serif fallback for ru/uk/bg UI languages.
- **UI sans: keep `system-ui` / SF Pro** (Handy's `--font-ui`, `App.css`, O). It looks native on macOS and covers all 26 locales. Inter (OFL, Cyrillic + Greek, O) is the cross-platform fallback.
- **Integration:** bundle woff2 through the existing `@font-face` pipeline (`App.css:32-37`) and subset with the existing `subset-font` devDependency. Never fetch fonts from the network (C-21).

---

## 5. Onboarding and tutorials (B-01, B-29, B-31, B-40-B-44, A-16, DEC-15)

| Candidate | License (evidence) | Popularity / maintenance | React 18 | Size (gz) | Fit | Cost | Risks | Verdict |
|---|---|---|---|---|---|---|---|---|
| **driver.js** https://github.com/nilbuild/driver.js (formerly kamranahmedse/driver.js) | MIT (O, `license` "Copyright (c) Kamran Ahmed"; npm MIT) | 1.10.2 (2026-10-18), 2.8 M wk dl, 27k stars (O) | Framework-free, no deps (O) | 8 KB (O) | In-Hub coach marks and "Show me" steps (B-40 last step, D-43, C-29 "Get started"). Button texts and progress text are configurable (`nextBtnText`, `doneBtnText`, `progressText`, `closeBtnLabel`; O, `popover.ts:27-29`, `:107`), so we pass `t()` strings. New `checklist.ts` (items with `id`, `tour`, `completeWhen`; O) could back B-44, but Wispr's checklist is a sidebar card, so write that card ourselves. | S | Sets `innerHTML` for title and description (O, `popover.ts:104-121`): never pass user text. Cannot spotlight the **native pill** (A-16), which is outside the webview. | **Adopt** for in-window tours. |
| react-joyride https://github.com/gilbarbara/react-joyride | MIT (O) | 3.2.0 (2026-07), 1.6 M wk dl (O) | `16.8 - 19` (O) | 25.6 KB, 10 deps (O) | Same need, React-native API. | S | Heavier. | Alternative. |
| @reactour/tour | MIT (O) | 3.8.0 (2025-05) (O) | Yes | – | Same. | S | Slower cadence. | Reject. |
| onborda / nextstepjs | onborda: npm says MIT but there is no LICENSE file in the repo or the published package (O, GitHub API 404, unpkg 404). nextstepjs: MIT (O) | Small | `>=18` | – | Next.js-oriented. | S | onborda has no LICENSE file. | Reject both. |
| shepherd.js / react-shepherd https://github.com/shipshapecode/shepherd | **AGPL-3.0 + commercial** (O, LICENSE.md; npm "AGPL-3.0") | 13.8k stars (O) | – | – | – | – | Copyleft. | **Reject.** |
| intro.js https://github.com/usablica/intro.js | **AGPL-3.0 + commercial** (O, npm "AGPL-3.0"; license.md "commercial license") | – | – | – | – | – | Copyleft. | **Reject.** |
| @stepperize/react | npm says MIT, but there is no LICENSE file in the repo or the published package (O, 404s) | 8.0.1 (O) | Yes | – | Stepper state. | – | No license file; trivial to write. | Reject. Handy's `STEP_ORDER` and `OnboardingShell.tsx` already are the stepper (B-01 is TUNE). |
| Lottie: lottie-web / lottie-react | MIT (O) | lottie-web 5.13.0, last release 2025-05, last push 2025-09 (O) | lottie-react `^18.2\|\|^19` (O) | lottie-web 25 MB unpacked (O) | Designer-made animations. | M | Authoring needs After Effects or a SaaS editor (I); slowing upstream. | Reject for v1. |
| dotLottie (`@lottiefiles/dotlottie-react`) | MIT (O) | 0.20.0 (O) | Yes | 34 KB + WASM (O) | Same. | M | **Downloads its WASM from cdn.jsdelivr.net / unpkg.com by default** (O, strings in `dotlottie-web@0.81.0/dist/index.js`); it must be self-hosted to stay offline (AF-12). | Reject for v1. |
| Rive (`@rive-app/react-canvas`) | MIT runtime (O) | 4.38.0 (O) | Yes (O) | – | Interactive scenes. | M | Runtime WASM default URL on unpkg (O, `@rive-app/canvas` bundle string); the authoring editor is a proprietary SaaS (I). | Reject. |
| **Handy's own scene engine** (`Demo.tsx`, `useTryEngine.ts`, `WindowBar.tsx`, `DesktopBar.tsx`) | MIT (Handy) | – | – | 0 | DEC-15 "animated in-app scenes": light, translatable, reduced-motion stills already exist (R2 B-24, O in code layout). | M per scene | Coupled to the onboarding store (R2 B-40). | **Extend (recommended for DEC-15).** |
| **canvas-confetti** https://github.com/catdad/canvas-confetti | ISC (O, LICENSE) | 1.9.4, 8.1 M wk dl, 12.8k stars (O) | n/a | 4.3 KB (O) | Celebration moments (B-27, streaks). Has `disableForReducedMotion` (O, `src/confetti.js:247`, `:611`). | S | – | **Adopt.** |
| react-confetti / js-confetti | MIT (O) | – | – | – | Same. | S | – | Reject (canvas-confetti is smaller and honors reduced motion). |
| Typing test: monkeytype | **GPL-3.0** (O) | 21k stars | – | – | Reference UX only. | – | Copyleft. | **Study (UX only, no code).** |
| Typing test: react-typing-game-hook | npm says Apache-2.0, GitHub says MIT (mismatch, O) | 1.4.2 (2023), 13 wk dl (O) | `>=17` | – | – | – | Stale, license mismatch. | Reject. Write B-29 ourselves (~150 lines: timer, WPM, per-language sample text). |

**Native spotlight on the pill (A-16, B-32, B-41):** no web tour library applies. It needs a click-through NSPanel dim layer anchored to the pill (R2: tauri-nspanel). Write ourselves.

---

## 6. i18n and accessibility (E-80, E-81, C-210, AF-13)

| Candidate | License (evidence) | Popularity / maintenance | Fit and files touched | Cost | Risks | Verdict |
|---|---|---|---|---|---|---|
| **eslint-plugin-i18next** (present) | MIT (O, devDependency `^6.1.3`) | – | Already enforces "no hardcoded JSX strings" (AGENTS.md). | – | – | **Keep.** |
| **scripts/check-translations.ts** (present) | Handy MIT | – | CI fails on missing or extra keys across 26 locales (O, script header; R2 E-80). | – | Every new key needs 25 translations before CI passes. | **Keep.** |
| **i18next-cli** https://github.com/i18next/i18next-cli | MIT (O) | 1.80.5 (2026-10-26), 922k wk dl (O) | `status` (per-locale completeness), `lint` (hardcoded strings), `extract --ci` (O, README). Official successor: **i18next-parser is archived** (O, GitHub API `archived=True`). Files: `package.json` (devDependency), new `i18next.config.ts`. | S | Its `localize`/`locize-*` commands push to a commercial cloud (O, README); do not use them. `extract` cannot see dynamic keys (I): run `status`/`lint`, not destructive extract. | **Adopt (dev only, status/lint).** |
| i18next-parser / i18next-scanner | MIT (O) | parser archived; scanner last release 2024-10 (O) | – | – | Unmaintained. | Reject. |
| @lingual/i18n-check, i18n-unused | MIT (O) | 0.9.5 / 0.21.0 (O) | Unused-key detection. | S | Overlaps i18next-cli. | Optional. |
| i18n-ai-translate | **GPL-3.0** (O) | – | Machine translation of JSON. | – | Copyleft. | **Reject.** |
| @inlang/cli | npm Apache-2.0, but the GitHub API reports no license (O) | 3.4.4 (O) | Has machine-translate commands (I) that call a hosted service (U). | – | Network, unclear licence file. | Reject. |
| **Own `scripts/translate-missing.ts`** | ours | – | For each locale, find keys missing vs `en` and send them in batches to an OpenAI-compatible endpoint (the D-67 local `llama-server`, or any endpoint a maintainer chooses). Require `{{placeholders}}` to match exactly, write the JSON, and mark entries in a sidecar `machine.json` for later human review. Fork keys stay under one namespace (AF-13). | S (~120 lines) | Small local models translate UI copy unevenly (I): human review for top locales. | **Write ourselves.** |
| **Radix primitives** (from §4) | MIT | – | Roving focus in menus/tabs, focus return on close, `Esc` handling, RTL `dir` (I, Radix docs). | – | – | Covers most keyboard needs. |
| eslint-plugin-jsx-a11y https://github.com/jsx-eslint/eslint-plugin-jsx-a11y | MIT (O) | 6.10.2 (2024-10), 55 M wk dl (O) | Lint rule `click-events-have-key-events` etc. would have caught the `div` nav items (C-210). Files: `eslint.config.js`. | S | Release cadence slow but stable. | **Adopt (dev only).** |
| @axe-core/playwright | **MPL-2.0** (O) | 4.14.0, 17 M wk dl (O) | Automated a11y checks in the existing Playwright setup (`package.json` scripts, O). | S | MPL-2.0 is file-level copyleft. Dev-only, never shipped. | **Owner decision** (fine in my view: test-only, unmodified). |
| react-focus-lock / focus-trap-react / tabbable | MIT (O) | Active (O) | Focus traps. | – | Handy's `Dialog.tsx` already traps focus; Radix handles the rest. | Reject (not needed). |
| react-hotkeys-hook | MIT (O) | 5.3.3 (O) | In-window shortcuts (e.g. ⌘K, ⌘F in history). | S | – | Optional. |

---

## 7. Recommended set (one choice per need)

| Need | Choice | License | Cost | R2 IDs it closes or unblocks |
|---|---|---|---|---|
| Newline-safe deterministic stage | Fork helper that runs existing stages per line + line-safe whitespace collapse (own code, §1.3) | – | S | AF-06, D-210 (enables D-53, D-56) |
| Multi-pattern matching (dictionary pairs, voice commands, snippet triggers) | **aho-corasick** (already compiled in) | Unlicense/MIT | S | D-48, D-53, D-56, C-63, C-72 |
| Tokenizing + word counts | **unicode-segmentation** (already compiled in) + CJK char rule | MIT/Apache-2.0 | S | C-30, C-31, C-212, DEC-11, B-29 |
| Fuzzy single terms | Keep Handy's engine (strsim + natural) | MIT | – | D-47, C-67 |
| ITN (numbers, money, dates, time) | **text-processing-rs** (rule engine, en first, per line, punctuation tagger off) + **text2num** for it/pt/nl/da numbers | Apache-2.0 / MIT | M | D-14 |
| Word diff for "N Changes", fix counts | **similar** (`unicode`, `inline`) | Apache-2.0 | S | D-40, C-52, D-57, C-57 "most corrected" |
| Undo AI edit | No library; stored texts + `shown_variant` | – | S | C-42, D-06, DEC-19 |
| Migrations for fork data | **rusqlite_migration** (already a dependency) on a separate `fork.db` | Apache-2.0 | S | DEC-08, AF-07, AF-13, C-209 |
| Search | SQLite **FTS5 trigram** (bundled), app-maintained index | public domain | S | C-36, C-216 |
| Connection hardening | rusqlite pragmas (WAL, busy_timeout) | – | S | C-208 |
| CSV import | **papaparse** | MIT | S | C-64, C-71, D-50, D-55 |
| CSV/zip export | **csv** + **zip** (zip already compiled in) | Unlicense/MIT, MIT | S | C-65, E-66 |
| Delete all data | Own command (§3) | – | M | E-67 |
| Menus, popovers, tabs | **Radix Primitives** (per package) | MIT | S each | C-41, C-13, C-17, C-201, C-50, C-59, D-18, C-14 |
| Toasts with Undo | **sonner** (present) | MIT | – | C-44 |
| Long history list | **TanStack Virtual** | MIT | S | C-207 |
| Streak calendar | **react-activity-calendar** | MIT | S | C-55, C-32 |
| Charts | **Recharts** (lazy-loaded Insights page) | MIT | S | C-50, C-54, C-31 |
| Save insights as image | **html-to-image** | MIT | S | C-56 |
| Snippet body editor | **Plain `Textarea` v1**; TipTap 3 + ammonia + html2text only if DEC-12 = rich | MIT | S / M | C-70, D-54, DEC-12 |
| Fonts | **Newsreader** (display) + Source Serif 4 (Cyrillic fallback) + system-ui | OFL-1.1 | S | C-21, C-19 |
| Icons | **lucide-react** (present) | ISC | – | – |
| Animation | Existing CSS + View Transitions; Motion only where needed | MIT | – | B-24, C-23 |
| In-window tours / coach marks | **driver.js** | MIT | S | B-40, D-43, C-29, B-45 |
| Tutorial scenes | Extend Handy's `Demo.tsx` scene engine (no Lottie/Rive) | MIT (Handy) | M per scene | DEC-15, B-31, B-39, B-42, D-43 |
| Celebration | **canvas-confetti** | ISC | S | B-27 (tone), streak milestones |
| i18n tooling | Keep eslint-plugin-i18next + `check-translations.ts`; add **i18next-cli** (status/lint) | MIT | S | E-80 |
| Accessibility lint | **eslint-plugin-jsx-a11y** (+ @axe-core/playwright if MPL is accepted for tests) | MIT (MPL) | S | C-210, E-81 |

## 8. Write ourselves (too product-specific to adopt)

1. Newline-safe stage wrapper and the stage order in §1.3, with per-stage change counts (pattern from MIT hyprwhspr-rs).
2. Dictionary rule store, matcher glue and case preservation; the "Add to vocabulary" dialog fields (C-62, C-68).
3. Snippet trigger normalizer and offset map, boundary rules and case propagation (concepts from espanso docs only).
4. Voice-command phrase tables per language, clause-edge rule and in-utterance "scratch that".
5. Word-count rule (CJK chars, Thai estimate) and `daily_stats` upserts; streak and WPM math.
6. Fork DB schema and migrations, FTS maintenance, export zip layout, delete-all sequence.
7. Diff card UI (`<del>`/`<ins>` styling, "N Changes" grouping, screen-reader prefixes) and the Undo AI edit toggle.
8. Hub layout pieces: Dictation home, stats rail, dismissible example banner (C-23), empty states (C-206), setup checklist card (B-44), sidebar groups and collapse.
9. Typing test (B-29) and time-saved slider (B-28).
10. Native pill spotlight panel (A-16).
11. `translate-missing.ts` machine-translation script with placeholder validation.

## 9. Open points

- **(U)** Whether Handy's default models (Parakeet, Whisper) ever emit spoken-form numbers in practice. This decides whether ITN is worth shipping on by default. Check with 20 sample dictations.
- **(U)** Size of ICU4X segmenter data with Thai/CJK dictionaries; measure only if DEC-11 needs Thai.
- **(U)** Wispr's rules for "N Changes" counting and for word counts in CJK.
- **(I)** text-processing-rs false positives for single punctuation words ("period", "dash", "plus"). Plan: call the number/money/date/time/measure taggers only (requires either a feature flag upstream or a thin fork). Raise an issue upstream before adopting.
- **Owner decision:** MPL-2.0 for the dev-only `@axe-core/playwright`.

## 10. Summary (10 lines)

1. The text pipeline needs mostly glue, not new engines. aho-corasick and unicode-segmentation are already compiled into Handy, and the existing fuzzy custom-word engine stays as it is.
2. Newlines: wrap Handy's existing stages per line, and expand snippets last (after the LLM). That closes AF-06/D-210 without editing `text.rs`.
3. ITN: text-processing-rs (Apache-2.0, a NeMo port) is the only maintained Rust option. It destroys newlines and turns lone words like "period" into symbols, so run it per line with only its number, money, date, time and measure taggers. text2num (MIT) covers it/pt/nl/da numbers.
4. espanso, monkeytype, shepherd, intro.js and i18n-ai-translate are GPL or AGPL. We studied only their docs and licenses.
5. Diff: the `similar` crate (Apache-2.0) gives word diffs in Rust, both for stored fix counts and for the "N Changes" card. jsdiff (BSD-3) is the fallback if a diff is only ever needed in the UI.
6. Data: rusqlite_migration is already a dependency. A separate `fork.db` with its own migrations, WAL, an app-maintained FTS5 trigram index and `daily_stats` resolves DEC-08 and AF-07 with no new crates.
7. Import and export: papaparse (MIT) parses CSV for the import preview, and the Rust csv and zip crates write the export (zip is already compiled in). Delete-all is our own code.
8. UI: Radix primitives (MIT, React 18 OK) for menus, popovers and tabs; TanStack Virtual for history; react-activity-calendar for the streak heatmap; Recharts for Insights charts; sonner and lucide are already there. Use shadcn only as styling reference, because its new components assume React 19.
9. Onboarding: driver.js (MIT, 8 KB) for in-window tours, Handy's own Demo scene engine for animations (dotLottie and Rive fetch WASM from CDNs by default), and canvas-confetti for celebrations. The typing test, checklist card and pill spotlight are our own code.
10. i18n and a11y: keep the existing lint and CI checks, add i18next-cli (status/lint), eslint-plugin-jsx-a11y and a small machine-translation script of our own. Fonts: Newsreader + Source Serif 4 (OFL) + system-ui.

**Recommended set:** aho-corasick, unicode-segmentation, text-processing-rs (+text2num), similar, rusqlite_migration on `fork.db` + FTS5, papaparse, csv, zip, Radix Primitives, sonner, TanStack Virtual, react-activity-calendar, Recharts, html-to-image, driver.js, canvas-confetti, Newsreader/Source Serif 4, i18next-cli, eslint-plugin-jsx-a11y. TipTap, ammonia and html2text only if DEC-12 = rich; Motion and cmdk optional.
