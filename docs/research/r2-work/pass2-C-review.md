# Pass 2, area C: review of the Hub / data / settings gap analysis

**Reviewed:** `pass1-C-hub-data.md` (106 rows), against R1 (all 227 lines, re-read) and the code on branch `wispr-bro`. Other pass-1 files (A, B, D, E) were skimmed for overlaps.

**Tags.** `verified-in-code` = I opened the cited lines in this pass. `inferred` = reasoned from code I read, not traced end to end. `unknown` = not determined. Paths are relative to the repo root unless they start with `~/.cargo/registry/...`, which means the crate source used by this lockfile.

## Summary

- **Rows checked:** 106 of 106. I opened every cited `path:line`, except where a row cites a file "not read" (I opened those too where it mattered: `Dropdown.tsx`, `ProgressBar.tsx`, `Mascot.tsx`, `overlay.rs`, `UpdateChecker.tsx`).
- **Evidence errors:** 22. 5 are line ranges that are off. 9 are claims that were inferred or "unknown" in pass 1 and are now verified, sometimes with a different answer. 8 are claims that are wrong or incomplete in a way that changes the work. The most important ones:
  1. **FTS5 is available.** The bundled SQLite 3.50.2 is built with `-DSQLITE_ENABLE_FTS5`, and the `trigram` tokenizer is present.
  2. **The clipboard plugin can write HTML.** Handy's two paste paths still write plain text only.
  3. **Dictation length is already computed.** `TranscriptionCompleteEvent.audio_seconds` exists; it is not stored and not tied to a history entry id.
  4. **The `ADDED_COLUMNS` migration pattern only adds `TEXT` columns.** Pass 1 recommends it for integer columns, which would be wrong.
  5. **The privacy risk row missed parts that exist today:** an on-device Apple Intelligence provider and a localhost (Ollama) endpoint. It also missed that the update-check toggle is only in Debug.
  6. **The `ProgressBar` reuse hint is wrong.** That component is a model-download widget.
  7. **Dictation into Handy's own window already works.** The onboarding Try-it textarea receives real pastes.
- **Class/effort changes:** 15 rows.
- **New rows:** 18 (C-201 to C-218).
- **Duplicates across areas:** 24 C rows overlap rows in A, B, D or E. Some of the overlapping rows disagree on class or effort. A table below proposes one owner for each.
- **Pass-1 class counts** (HAVE 11, TUNE 22, EXTEND 18, BUILD 46, OUT 9) recount correctly. After this review they become HAVE 11, TUNE 22, EXTEND 23, BUILD 41, OUT 9 (C-76 to HAVE, C-27 and C-99 out of HAVE, and five rows from BUILD to EXTEND).

## Evidence corrections

| Row | Problem | Correct evidence (path:line) | Fix |
|---|---|---|---|
| C-20 | Range `theme.css:155-200` is mostly grey-step tokens. verified-in-code | Media query `src/styles/theme.css:168-190`; `data-theme` override `src/styles/theme.css:202-244`; the override applies to the overlay window too (comment `theme.css:199-200`). verified-in-code | Cite 168-244. HAVE stands. Components use tokens (only intentional `text-white`/`bg-black/50` in `Button.tsx:28`, `Dialog.tsx:161`, `Switch.tsx:35`, `Radio.tsx:12`, `UpdateChecker.tsx:212,230`). verified-in-code |
| C-28 | `settings.rs:392-576` overshoots. | `AppSettings` spans `src-tauri/src/settings.rs:392-564`. verified-in-code | Cosmetic. |
| C-31 | Says the sample count is "dropped". It is used: duration is computed and emitted, but not stored. | `TranscriptionCompleteEvent { outcome, audio_seconds, wait_seconds }` at `src-tauri/src/actions.rs:45-51`, built from `sample_count` at `actions.rs:53-64`, emitted at `actions.rs:850-855`. The event has no history entry id. With VAD on (default `settings.rs:627-629`), silence is dropped from the recording (`settings.rs:548-550` comment), so `audio_seconds` is speech time, not wall time. verified-in-code | Store `audio_ms` on the row at `save_entry` (`actions.rs:786-793` has `sample_count` in scope). Decide whether WPM uses speech time (looks fast) or wall time (needs a record-start timestamp, which is not captured). |
| C-33 | Reuse hint "ProgressBar" is wrong: it is a fixed-width multi-item download widget (`progress: ProgressData[]`, `w-20 h-1.5`). | `src/components/shared/ProgressBar.tsx:3-26`. A fitting bar exists: `src/components/shared/ProgressLine.tsx:13-31` (0-1 value, any width). verified-in-code | Reuse `ProgressLine`. |
| C-36 | FTS5 marked "inferred". | `~/.cargo/registry/.../libsqlite3-sys-0.35.0/build.rs:132` `-DSQLITE_ENABLE_FTS5` in the `bundled` build; `rusqlite = { version = "0.37", features = ["bundled"] }` at `src-tauri/Cargo.toml:76`; lock `libsqlite3-sys 0.35.0` (`src-tauri/Cargo.lock:3358-3359`); SQLite `3.50.2` (`libsqlite3-sys-0.35.0/sqlite3/sqlite3.c:468`); `trigram` tokenizer present (`sqlite3.c:240701`). verified-in-code | Mark verified. Use `trigram` (or `unicode61` + trigram) so substring search works for CJK. |
| C-41 | `Dropdown.tsx` "inferred" to be a select. | Props `options`, `selectedValue`, `onSelect` (`src/components/ui/Dropdown.tsx:11-24`): a single-value select. No menu or popover library in `package.json:28-56`. verified-in-code | Mark verified. An action-menu primitive is new work. |
| C-46 | "About 1.9 MB/min" was inferred from AGENTS.md. | `save_wav_file`: mono, 16 000 Hz, 16-bit int (`src-tauri/src/audio_toolkit/audio/utils.rs:31-45`), so 32 000 B/s, 1.92 MB/min of speech. verified-in-code | Mark verified. |
| C-54 | Reuse hint suggests new crates. macOS needs none. | `objc2-app-kit = "0.3"` is already a dependency (`src-tauri/Cargo.toml:166`); lock `0.3.2` (`Cargo.lock:3825-3826`); its default features include `NSWorkspace` and `NSRunningApplication` (`~/.cargo/registry/.../objc2-app-kit-0.3.2/Cargo.toml`, default list). verified-in-code | macOS: `NSWorkspace.sharedWorkspace.frontmostApplication` via objc2. Windows/Linux need their own code later. Same finding as D-21. |
| C-70 | "Clipboard HTML via arboard/clipboard plugin (inferred support)". | The plugin has `write_html(html, alt_text)` on desktop (`~/.cargo/registry/.../tauri-plugin-clipboard-manager-2.3.2/src/desktop.rs:72-87`, arboard `set_html`). Handy never uses it: the paste path writes text (`src-tauri/src/clipboard.rs:93`), the macOS reliable-paste path uses `NSPasteboardTypeString` (`src-tauri/src/paste_tx/macos.rs:22`), and the clipboard restore saves only text or an image (`clipboard.rs:82-90`). verified-in-code | Rich snippets need HTML plus a plain fallback on **both** paste paths, and the restore loses the user's previous rich clipboard (already true today). See the effort change. |
| C-75 | `overlay.rs` "not read"; second-window effort was guessed. | The overlay is an NSPanel built with `tauri_nspanel::PanelBuilder` (`src-tauri/src/overlay.rs:18, 454`), `always_on_top`, `decorations(false)` (`overlay.rs:410-420`); `tauri-nspanel` is a dependency (`src-tauri/Cargo.toml:161`). Capabilities are scoped to the window labels `main` and `recording_overlay` (`src-tauri/capabilities/default.json:5`). verified-in-code | Reuse hint: `tauri-nspanel` for the floating note. A new window label must be added to the capabilities, or it gets no plugin permissions. |
| C-76 | "inferred" that Handy pastes into its own windows. | The onboarding Try-it screen has a `<textarea>` (`src/components/onboarding/screens/TryIt.tsx:600`) that receives the real dictation paste, and it detects "nothing typed" when focus was elsewhere (`src/components/onboarding/useTryEngine.ts:77-83, 187`). The paste thread runs off the main thread for this reason (`actions.rs:815-816`). verified-in-code | Class change to HAVE (see below). |
| C-81 | "'recommended' label: unknown elsewhere". | No component labels a microphone as recommended. "Recommended" only appears in `Tag.tsx` (doc comment) and `shared/modelTable/ModelTableRow.tsx` (models). verified-in-code (grep of `src/components`) | Absent. Already A-38/B-20. |
| C-87 | Cites "guard at `lib.rs:1094`". | Comment `src-tauri/src/lib.rs:1093-1095`, condition `lib.rs:1096-1097`. The launch-time copy is `lib.rs:147-163`, the close handler `lib.rs:1108-1123`. verified-in-code | Cite all three places. A `show_in_dock` setting must be honored in each. |
| C-93 | The risk text says LLM post-processing "can call cloud APIs" but misses the local options. It also misses where the update toggle lives. | On-device Apple Intelligence provider (`src-tauri/src/settings.rs:776-783`, macOS aarch64 only); `custom` provider defaulting to `http://localhost:11434/v1` (`settings.rs:802-805`); cloud providers `settings.rs:714-800`. The update-checks toggle is only on the Debug page (`src/components/settings/debug/DebugSettings.tsx:56`); the check runs from the footer when the window loads (`src/components/update-checker/UpdateChecker.tsx:50, 85`). verified-in-code | The privacy page can be truthful only if cloud providers are removed or hidden (overlaps D-69, E-76) and the update toggle moves out of Debug (overlaps E-75). |
| C-99 | HAVE overstated. | `WhatsNewGate` shows only the newest unseen note, once (`src/components/whats-new/WhatsNewGate.tsx:28-43, 61-67`); only `0.9.0.md` and `0.9.7.md` are bundled (`src/content/release-notes/`). There is no way to reopen it. verified-in-code | TUNE S: add a Help entry that opens `WhatsNewModal` for the current version (and maybe a list). Same as E-89. |
| C-105 | "Default Tauri menu otherwise (inferred)". | Tauri `2.11.5` (`src-tauri/Cargo.lock:6429-6430`); `enable_macos_default_menu: true` by default (`~/.cargo/registry/.../tauri-2.11.5/src/app.rs:1620`); the default menu has File/Edit/View/Window/Help, with Edit holding undo, paste and select-all (`tauri-2.11.5/src/menu/menu.rs:142-241`). Handy never overrides it (no hit for `enable_macos_default_menu`/`set_menu` outside the tray). verified-in-code | A custom app menu **replaces** the default. The Edit items must be rebuilt, or ⌘C/⌘V/⌘Z stop working in Dictionary, Snippets and Scratchpad text fields (inferred from how macOS routes key equivalents through the menu). New row C-203. |
| C-07 | Incomplete gating. | The Post Process toggle only renders when `experimental_enabled` is on (`src/components/settings/advanced/AdvancedSettings.tsx:32, 68-70`). verified-in-code | Two hidden switches today, not one. |
| C-21 | Misses an existing local font pipeline. | A bundled woff2 `@font-face` (`src/App.css:32-37`) and `--font-*` tokens (`App.css:41-50`). verified-in-code | Add a serif the same way: a bundled file, no network fetch. |
| C-24 | Mascot reuse. | `Mascot` renders `HandyMascot`, the Handy glove brand (`src/components/shared/Mascot.tsx:1-22`). verified-in-code | Reusable only as a pattern; the art must be rebranded (E-62/E-63). |
| C-35 | Reuse hint misses existing helpers. | `formatDate` (`src/utils/dateFormat.ts:37-57`) and `formatRelativeTime` with `Intl.RelativeTimeFormat` (`dateFormat.ts:65-127`) already exist. verified-in-code | Reuse them; class change below. |
| Data-model note "follow the `ADDED_COLUMNS` pattern" | Wrong for numeric columns. | The loop runs `ALTER TABLE ... ADD COLUMN {column} TEXT` (`src-tauri/src/managers/history.rs:176-184`), so every added column gets TEXT type. verified-in-code. With TEXT affinity, SQLite stores integers as text, and `row.get::<_, i64>` would then fail (inferred from SQLite affinity rules and rusqlite `FromSql`). | Extend `ADDED_COLUMNS` to `(name, sql_type)` pairs. New row C-209. |
| Rough edge 14 | `lib.rs:969-970`. | `inner_size` / `min_inner_size` at `src-tauri/src/lib.rs:970-971`. verified-in-code | Cosmetic. |

Also checked and correct (verified-in-code): C-01, C-02, C-05, C-12, C-14, C-15, C-16, C-17, C-19, C-22, C-23, C-25, C-26, C-27, C-30, C-34, C-37, C-38, C-39, C-40, C-42 to C-45, C-47, C-48, C-52, C-58, C-62, C-63, C-67, C-68, C-72, C-77, C-79, C-80, C-82 to C-86, C-88 to C-90, C-94, C-98, C-102 to C-104, C-106, and rough edges 1 to 13. Rough edge 8 (retention never runs while idle) is now verified: `cleanup_old_entries` is called only from `history.rs:309`, `commands/history.rs:108` and `:137`. File entries (`save_file_entry`, `history.rs:317-332`) never trigger cleanup.

## Class/effort changes

| Row | Pass-1 | New | Reason with evidence |
|---|---|---|---|
| C-76 Dictation into the focused note | EXTEND S | **HAVE** S | Paste into a Handy webview textarea already works and is used in onboarding (`TryIt.tsx:600`, `useTryEngine.ts:77-83`). A direct insertion API (no clipboard) would be an optional EXTEND. verified-in-code |
| C-99 What's New | HAVE | **TUNE** S | No reopen path; shown once (`WhatsNewGate.tsx:28-43`). verified-in-code |
| C-35 History grouped by date | BUILD S | **EXTEND** S | The list exists and only needs grouping; date helpers exist (`dateFormat.ts:37-127`). verified-in-code |
| C-63 Correct a misspelling | BUILD M | **EXTEND** M | The text pipeline and hook exist (`transcription.rs:1643-1651`), so rules are new logic in an existing stage. Agrees with D-48's class; effort stays M (Rust rules, dialog, case/word-boundary/CJK handling, tests, 26 locales). |
| C-52 Fixes made | BUILD M | **EXTEND** M | The fix stages exist (`text.rs:151`, `transcription.rs:1653-1658`) and only need to return counts (D-57 says EXTEND S). M stays because "most corrected word" needs a per-word log table. |
| C-44 Delete transcript | TUNE S | **EXTEND** S | Undo needs new backend logic: `delete_entry` removes the WAV and the row at once (`history.rs:691-720`). Undo needs a soft delete or a delayed purge. verified-in-code |
| C-70 Rich-text snippet dialog | BUILD M | **BUILD L** (rich) / **S** (plain v1) | A rich editor (Tiptap/Lexical), HTML sanitizing, HTML+plain writes on both paste paths (`clipboard.rs:93`, `paste_tx/macos.rs:22`), and Windows/Linux pasteboard behavior. Recommend plain-text snippets in v1. |
| C-101 Help > Microphone, Languages | TUNE S | **EXTEND** S | Needs deep links into sections. Navigation is plain local state (`src/App.tsx:87-89`); there is no router dependency (`package.json:28-56`) and no anchor scrolling. See C-204. |
| C-10 Setup checklist card | BUILD S | **BUILD M** | Items that tick themselves need hooks on many events (first dictation, word added, snippet created, tutorial finished). Matches B-44 (M). inferred |
| C-105 + C-106 App menus | BUILD S + EXTEND S | **BUILD M** (together) | The whole macOS menu bar must be rebuilt, including Edit/Window/Help (C-203), with live counts. Matches A-63 (M). verified-in-code (Tauri default menu) |
| C-36 History search | BUILD M | **BUILD S** (v1) | FTS5 + trigram are compiled in (see corrections). One command plus a search box is S; result highlighting and paging inside a grouped list stays M. |
| C-25 Window chrome | TUNE S | **TUNE M** | Overlay title bar, a header area holding toggle/bell, traffic-light inset, removing the macOS title-bar growth (`lib.rs:988-998`), a larger min size (`utils.rs:26`), and a drag-region permission. `core:default` likely lacks window start-dragging (inferred). Windows/Linux need custom decorations later. |
| C-02 Dictation as landing page | EXTEND S | EXTEND **M** (page) | Changing the default section is trivial (`App.tsx:87-88`). The home page itself (welcome, banner, stats rail, grouped history in a two-column layout) is a new page layout, not the History settings page (`HistorySettings.tsx:307-334` is a single `max-w-3xl` column). inferred |
| C-27 Version + Update | HAVE | **TUNE** S | It works (E-51) but is in the global footer next to the model selector (`Footer.tsx:30-45`). Moving it is a UX change. |
| C-94 Download / delete your data | BUILD S | **BUILD M** | "Delete all" must also cover the new stores (stats, dictionary, snippets, notes, notifications), settings, logs and downloaded models to be honest (E-67 notes). Export must zip WAVs. inferred |

Unchanged but flagged:
- **C-46** stays EXTEND M. E-69 calls the same gap TUNE S, which only covers changing the default. Splitting text and audio retention needs new cleanup logic, and the UI must handle a row whose audio is gone. verified-in-code (`history.rs:455-483` deletes both together)
- **C-50** stays M for the page, but Insights in total (C-30 to C-33, C-50 to C-57) is L: there is no chart library (`package.json:28-56`), it needs a stats backend and app capture, and every string goes into 26 locales (`src/i18n/locales/`, 26 dirs).

## Data model: verified current schema and required additions

### What is stored per dictation today (verified-in-code)

Table `transcription_history`. It is created by migration 1 and gets three columns from migrations 2-4 (`history.rs:22-36`). Three more nullable TEXT columns are added outside the versioned migrations (`history.rs:40, 176-184`). `user_version` = 4.

| Column | Type | Written by | Notes |
|---|---|---|---|
| `id` | INTEGER PK AUTOINCREMENT | insert | Paging orders by `id DESC` (`history.rs:565-604`). |
| `file_name` | TEXT NOT NULL | `actions.rs:701` | `handy-<unix seconds>.wav`; 16 kHz mono 16-bit, after VAD trimming (inferred from `settings.rs:548-550`). Two dictations stopped in the same second would get the same file name (inferred; low risk). |
| `timestamp` | INTEGER NOT NULL | `history.rs:297` | UTC seconds **at save time**, after transcription and LLM, not at record start. |
| `saved` | BOOLEAN | toggle | Star: exempt from retention (`history.rs:491, 533`). |
| `title` | TEXT NOT NULL | `history.rs:336, 722-730` | English-format local time string; the UI does not use it (grep of the history components). |
| `transcription_text` | TEXT NOT NULL | `actions.rs:786-793` | ASR text **after** Chinese script conversion, fuzzy custom-word correction, filler removal and normalization (`transcription.rs:1600-1662`). Empty string = failed (`actions.rs:885-897`). |
| `post_processed_text` | TEXT NULL | same | LLM output when the post-process binding was used. |
| `post_process_prompt` | TEXT NULL | same | Prompt text used (full text, not an id). |
| `post_process_requested` | BOOLEAN | same | Whether the LLM was requested (retry re-runs it, `transcription_jobs.rs:309-313`). |
| `source_name` | TEXT NULL | `save_file_entry` | Non-null = file transcription; excluded from retention and from "last transcript" (`history.rs:491, 638`). |
| `language` | TEXT NULL | split from `LanguageDecision` | Not shown in the UI (inferred from grep). |
| `language_source` | TEXT NULL | same | `LanguageSource::as_str`. |

There are no indexes besides the primary key, no FTS, and no other tables. Each call opens a new `Connection` without setting a journal mode or busy timeout (`history.rs:254-256`). Foreign keys default to on in this SQLite build (`-DSQLITE_DEFAULT_FOREIGN_KEYS=1`, `libsqlite3-sys-0.35.0/build.rs:126`). All verified-in-code.

**Not stored:** duration (computed and emitted only, `actions.rs:45-64`), word count, target app, paste outcome (the outcome is known only after the row is saved: save at `actions.rs:786`, paste at `actions.rs:829-843`), the pre-fix ASR text, fix counts, a previous text after Retry (`history.rs:394-410` overwrites it), and an edited or flagged state.

### Minimal additions per feature

New per-row columns need the typed variant of `ADDED_COLUMNS` (C-209), so that older builds still open the database and `user_version` is not raised.

| Feature | Minimal schema addition | Notes |
|---|---|---|
| Undo AI edit (C-42/D-07) | `shown_variant TEXT NULL` (`'final'` / `'original'`). Both texts already exist. | For an "exact transcript" that also undoes dictionary and filler fixes, add `asr_text TEXT NULL` captured before `post_process_transcription_text`. That requires returning the raw text from `transcription.rs:1600` to the caller. inferred |
| Retry (C-43) | `previous_text TEXT NULL`, `previous_post_processed_text TEXT NULL` (one level of undo). | Retry already works; this only makes it reversible. |
| Extract audio (C-45) | None. The path is derivable (`get_audio_file_path`, `history.rs:674-676`). For audio-only retention: `audio_deleted_at INTEGER NULL`. | `file_name` is NOT NULL, so keep the name and mark the deletion instead. |
| Flag (C-40) | `flagged INTEGER NOT NULL DEFAULT 0`, or reuse `saved` if Flag means "keep". | Product decision. |
| WPM (C-31/C-51) | `audio_ms INTEGER NULL` (from `sample_count`), optionally `recorded_at INTEGER NULL` (record start) for wall-time WPM. | `audio_ms` is speech-only when VAD is on. |
| Word counts (C-30) | `word_count INTEGER NULL` of the final text, computed at insert and on retry. | Segmentation rule needed for CJK/Thai (see Still open). |
| Streaks / calendar (C-32/C-55) | A new table `daily_stats(day TEXT PK /*local YYYY-MM-DD*/, dictations INTEGER, words INTEGER, audio_ms INTEGER, fixes_dictionary INTEGER, fixes_fillers INTEGER, fixes_ai INTEGER)`. | Must survive row deletion and retention. Update it in the same transaction as the insert. Choose the local day at record time. |
| Per-app usage categories (C-54) | `app_bundle_id TEXT NULL`, `app_name TEXT NULL` per row; a `daily_app_stats(day, app_bundle_id, words, dictations)` table; categories come from a bundled map (bundle id to category), not stored. | Capture at record start (D-21). |
| Fixes made (C-52) | `fixes_dictionary INTEGER`, `fixes_fillers INTEGER`, `fixes_ai INTEGER` per row, plus `corrections(id, entry_id REFERENCES transcription_history(id) ON DELETE SET NULL, day TEXT, heard TEXT, written TEXT, kind TEXT)` for "most corrected word". | Requires `apply_custom_words` and `remove_filler_words` to return change counts (`text.rs:151`). |
| Paste outcome (C-213 / A-area) | `outcome TEXT NULL` set by an UPDATE after the paste thread finishes. | |
| Search (C-36) | `CREATE VIRTUAL TABLE history_fts USING fts5(transcription_text, post_processed_text, content='transcription_history', content_rowid='id', tokenize='trigram')` plus insert/update/delete triggers. | FTS5 and trigram are compiled in (verified). Trigram needs queries of at least 3 characters; fall back to `LIKE` for shorter ones (inferred from SQLite docs). |
| Long retention (C-46) | Settings, not schema: split `history_limit`/`recording_retention_period` into text retention (default: keep) and audio retention (for example 2 weeks), plus `audio_deleted_at`. | At 1.92 MB per speech-minute, 30 min/day of speech is about 1.7 GB/month of WAV (inferred arithmetic). |
| Stores that live outside history | `dictionary_entries(id, kind, from_text, to_text, source, created_at, hit_count)`, `snippets(id, trigger, body_plain, body_html, created_at, use_count)`, `notes(id, title, body, pinned, created_at, updated_at, last_opened_at)`, `notifications(id, kind, title, body, created_at, read_at)`. | Pass 1 listed these correctly. |

**Placement recommendation (inferred).** Put the fork-owned tables (stats, dictionary, snippets, notes, notifications, FTS) in a **separate database file** or in `CREATE TABLE IF NOT EXISTS` statements outside `MIGRATIONS`. This keeps `history.rs` close to upstream and avoids raising `user_version`. Upstream's comment says a higher `user_version` makes released builds refuse the file (`history.rs:174-175`). Before adding writers (notes autosave, stats), turn on `PRAGMA journal_mode=WAL` and set a `busy_timeout` (C-208).

**Pass 1's "data model gaps" list:** items 2-8 and 10 are confirmed. Item 1 is corrected: duration is computed and emitted, just not persisted. Item 9 is confirmed with one caveat: the outcome arrives after the insert, so it needs an UPDATE. Item 4 is confirmed: the pre-fix text is lost inside `post_process_transcription_text`.

## New rows

| ID (C-2xx) | Feature | R1 ref | Wispr behavior | Handy today (evidence) | Class | Effort | Notes / risks | Reuse hint |
|---|---|---|---|---|---|---|---|---|
| C-201 | Notifications panel controls: filter, "…" menu, empty state "You're all caught up!" with illustration | §4 Notifications panel | Bell panel has a filter, an overflow menu and an illustrated empty state | Absent; only transient toasts (`src/App.tsx:350-365`). verified-in-code | BUILD | S | On top of C-17. Local "…": mark all read, open notification settings (C-91). | Same menu primitive as C-41 |
| C-202 | Feature-announcement popup ("Dark mode is finally here") | §4 Dark mode | Launch popup announcing a new feature | `WhatsNewModal` shown once per version from bundled Markdown (`WhatsNewGate.tsx:14-71`). verified-in-code | TUNE | S | Use What's New for feature launches; needs our own notes and art. | Reuse `WhatsNewModal`, `MarkdownContent` |
| C-203 | Keep standard Edit/Window/Help menus when adding custom app menus | §6 | Full app menu bar: File, Edit, Dictation, Notetaker, My Voice, View, Help, Window | Tauri's default macOS menu is active (`tauri-2.11.5/src/app.rs:1620`, items `menu/menu.rs:142-241`); not overridden. verified-in-code | EXTEND | S | Replacing the menu drops Edit (undo, copy, paste, select all) unless rebuilt, which breaks text editing in Hub fields (inferred). The Help menu search box is native once a "Help" submenu exists (inferred). | Tauri `Menu`, `PredefinedMenuItem` |
| C-204 | Deep links inside the Hub | §10, §11, §12 | Help > Microphone/Languages, Invite opens Settings > Team, checklist opens Settings > MCP, the tour opens Connectors | Navigation is a `useState` section (`src/App.tsx:87-89`); one ad-hoc callback for Models to General (`App.tsx:57-60`); no router (`package.json:28-56`). The tray "Settings" (⌘,) only shows the window (`src-tauri/src/tray.rs:497-510`). verified-in-code | EXTEND | S | A small `navigate(page, section?)` store plus scroll-to-anchor. ⌘, should open the Settings modal once C-12 exists. | zustand (already a dependency) |
| C-205 | Page drop zones vs window-wide file drop | §11 (CSV import "browse or drag and drop") | Import dialogs accept dropped files | `FileDropOverlay` handles every drop in the window and sends files to transcription (`src/components/FileDropOverlay.tsx:19-29`; mounted globally `App.tsx:454`). verified-in-code | EXTEND | S | Dropping a .csv on the Dictionary import step would try to transcribe it. Route drops by the active page or dialog. DOM drop events are likely suppressed while Tauri's handler is on (inferred). | Extend `FileDropOverlay` with a context |
| C-206 | Empty states for every page (Dictionary, Snippets, Scratchpad Recents, Notifications, Insights before enough data) | §4, §11 pattern 3, §14 | One clear action plus an example banner ("No meetings today", "You're all caught up!", Add new word, Start new note) | Only History has one, a single line of text (`HistorySettings.tsx:275-279`). verified-in-code | BUILD | S | One `EmptyState` component; art must be ours (C-24). | Shared component |
| C-207 | History scale with long retention | §4 (history grouped by date) | Long, smooth history | Every loaded row stays in the DOM and mounts an `AudioPlayer` (`HistorySettings.tsx:284-298, 548`; player is 322 lines). Fine for 5 rows (the default); not for months of dictations. verified-in-code (structure), inferred (performance) | EXTEND | S | Virtualize the list, or mount the player only on demand (C-38). | `@tanstack/react-virtual` (MIT) |
| C-208 | SQLite connection settings for more writers | (enabler) | n/a | New `Connection::open` per call; no WAL, no busy timeout (`history.rs:254-256`). verified-in-code | EXTEND | S | Stats, notes autosave and FTS triggers add concurrent writes, which can return SQLITE_BUSY (inferred). | rusqlite `pragma_update`, `busy_timeout` |
| C-209 | Typed added columns | (enabler) | n/a | `ADDED_COLUMNS` only adds TEXT (`history.rs:40, 176-184`). verified-in-code | EXTEND | S | Needed before any INTEGER column (`audio_ms`, `word_count`, `flagged`...). Keep the "newer DB opens in older build" guarantee (`history.rs:805-823` test). | Same function |
| C-210 | Sidebar keyboard and screen-reader access | §8.8 (calm sidebar); quality | Native-feeling navigation | Nav items are `div`s with `onClick`, so they cannot get keyboard focus and have no role (`src/components/Sidebar.tsx:104-111`). verified-in-code | TUNE | S | Use `button` or `nav`/`aria-current`. Overlaps E-81. | n/a |
| C-211 | Failed dictation kept with audio and retry ("failure is soft") | §1.4, §8.3 | Text kept, retry available | An empty-text row is saved with the WAV (`actions.rs:885-897`), labelled "Transcription failed" (`HistorySettings.tsx:530-533`), and retry works (`commands/history.rs:64-70`). verified-in-code | HAVE | n/a | Exclude these rows from stats and search. | n/a |
| C-212 | Stats counting rules | §4, §5 | Totals are consistent | No stats; precedent: files are already excluded from "last transcript" and from retention (`history.rs:491, 638`). verified-in-code | EXTEND | S | Count only dictations with non-empty text; count once at first save (retry does not double count, or recompute the day). Decide whether deletes subtract. | n/a |
| C-213 | Live refresh of the stats rail and menu counts | §4, §6 | Counters update after each dictation | `HistoryUpdatePayload` events exist (`history.rs:53-64`) and the list listens (`HistorySettings.tsx:166-187`); no stats event. verified-in-code | EXTEND | S | Emit a `StatsUpdated` event after the stats transaction; the tray/app menu must rebuild its labels too (`tray.rs:406-409` rebuilds only on change). | tauri-specta event |
| C-214 | Cloud data toggles (Improve the model, Dictation cloud storage, Show public information, Sync notes, "Store data locally" dropdown) | §5 Data and Privacy | Toggles and a dropdown | n/a | OUT | n/a | Local page states plain facts instead (C-93). There is no "Improve the model" equivalent: no telemetry (E-73). | n/a |
| C-215 | Help popover "Contact sales" | §10 | Under "Get in touch" | n/a | OUT | n/a | Drop it. | n/a |
| C-216 | Notes search (floating note rail search icon) | §10 | Search across notes | Absent. verified-in-code (no notes store) | BUILD | S | Same FTS5 setup as C-36 on the `notes` table. | FTS5 |
| C-217 | Greeting name from macOS | §4 | "Welcome back, <name>" | No name in settings (`settings.rs:392-564`). `NSFullUserName` is available in `objc2-foundation 0.3.2` default features (`~/.cargo/registry/.../objc2-foundation-0.3.2/src/generated/NSPathUtilities.rs:130`; lock `Cargo.lock:4017-4018`). verified-in-code | BUILD | S | Use the first word of the full name, with an override field in Settings. Windows/Linux later. Splits C-28. | objc2-foundation |
| C-218 | Selected-card lilac state for choice cards and tabs | §12 (lilac fill and border on picked cards), §11 | Consistent selection color | `--color-plum`, `--color-plum-pick`, `--color-plum-faint` tokens already exist "for keys and picks" (`src/styles/theme.css:1-6, 163-165`). verified-in-code | HAVE | n/a | Reuse for Insights tabs, Dictionary tabs and Style cards. | Tokens |

**Handy UI pieces pass 1 did not use** (all verified-in-code by listing `src/components/ui` and `src/components/shared`). Each one maps onto a Hub need:
- `Textarea`: snippets body (plain v1), notes.
- `Switch` / `ToggleSwitch`: System toggles.
- `Select` (react-select): language lists.
- `Tooltip` / `InfoTooltip`: nav tooltips when collapsed.
- `Slider`, `Radio`, `Spinner`, `ResetButton`, `PathDisplay`: Data and Privacy folder paths.
- `ProgressLine` / `ProgressRing`: Voice Profile.
- `GradeChip`, `DoneMark`: checklist ticks.
- `Keycaps`: shortcut summaries.
- The onboarding type scale and radii tokens (`App.css:52-69`), which are already the consumer look.
- `react-markdown` (`package.json:49`) via `MarkdownContent`, for bundled help pages.
- `lucide-react` (`package.json:45`) for nav icons.

Dark mode is complete for token-driven UI (see C-20 correction).

## Cross-area duplicates (proposed single owner)

| C row | Same item in | Disagreement | Proposed owner |
|---|---|---|---|
| C-42 Undo AI edit | D-07 | both EXTEND S | D (logic); C keeps the row menu UI |
| C-43 Retry | D-08 | both HAVE | C |
| C-52 Fixes made | D-57 | BUILD M vs EXTEND S | D for counts; C for the Insights card (see class change) |
| C-54 App categories | D-21, D-62 | M vs S | D for capture; C for the category map and chart |
| C-57 Voice profile cards | D-63 | both M | C |
| C-63 Misspelling rules | D-48 | BUILD vs EXTEND | D (now EXTEND in both) |
| C-64 / C-71 CSV import | D-50, D-55 | | C (UI wizard), D (parsing rules) |
| C-66 Auto-added words | D-52 | | D |
| C-70 / C-72 Snippets | D-53, D-54 | M vs M | D for expansion; C for page and editor |
| C-77 Scratchpad shortcut | A-30 | | A |
| C-80 Shortcuts row | A-33 | | A (panel), C (row) |
| C-81 Microphone | A-37, A-38, B-20 | | A |
| C-82, C-83 Languages | B-61, B-67 | | B |
| C-85 Launch at login | A-64, B-35, E-87 | all TUNE | A |
| C-86 Flow Bar always | A-66 | EXTEND vs BUILD | A |
| C-87 Show in Dock | A-65 | both EXTEND S | A |
| C-89, C-90 Sounds, mute | A-41, A-42 | | A |
| C-93 Privacy page | E-68, E-75, E-76 | EXTEND vs BUILD | E (network inventory); C (page) |
| C-94 Export / delete | E-66, E-67 | | E |
| C-46 Retention | E-69 | EXTEND M vs TUNE S | C (logic); E-69 is only the default |
| C-99 What's New | E-89 | HAVE vs TUNE | TUNE (both) |
| C-103 Feedback | E-90 | | E |
| C-105, C-106 Menus | A-63, D-64 | S vs M | A (menus), using C counts |
| C-10, C-23, C-49 | B-44, B-38, B-42 | S vs M | B for content; C for components |

## Open questions resolved

1. **FTS5 in the bundled SQLite (pass-1 Q13).** Yes. `libsqlite3-sys 0.35.0` `bundled` compiles with `SQLITE_ENABLE_FTS5`, FTS3, JSON1 and RTREE (`build.rs:125-140`); SQLite 3.50.2; `trigram` tokenizer present. verified-in-code
2. **Rich-text clipboard (Q5).** The plugin can write HTML with plain alt text on desktop (`tauri-plugin-clipboard-manager-2.3.2/src/desktop.rs:72-87`). Handy's paste paths are text-only (`clipboard.rs:93`, `paste_tx/macos.rs:22`), and the restore keeps only text or an image (`clipboard.rs:82-90`). So rich snippets mean changes to both paste paths. Recommend plain text in v1. verified-in-code
3. **Should file transcriptions count toward stats (Q7)?** The code already treats files as "not dictations" for last-transcript and retention (`history.rs:491, 533, 638`). Excluding them is consistent. verified-in-code (precedent); the decision is still the product's.
4. **Greeting name (Q11).** `NSFullUserName()` is reachable through the existing `objc2-foundation 0.3.2` dependency (C-217). A name can be shown with no prompt. verified-in-code
5. **"Show in dock" vs the tray/dock guard (Q12).** The Dock policy is decided in three places: launch (`lib.rs:147-163`), show (`lib.rs:113-117`) and close (`lib.rs:1108-1123`). `show_in_dock = true` can mean "never demote to Accessory" in the launch and close paths. `show_in_dock = false` keeps today's behavior. The rule that the Dock stays visible without a tray (`lib.rs:1093-1097`) still holds. inferred (no other activation-policy calls found besides these)
6. **Q13 leftovers.**
   - `Dropdown.tsx` is a value select (`Dropdown.tsx:11-24`).
   - `UpdateChecker` checks when the window loads (`UpdateChecker.tsx:50, 85`).
   - `overlay.rs` builds an NSPanel through `tauri-nspanel` (`overlay.rs:18, 410-420, 454`).
   - `Mascot.tsx` is the Handy glove (`Mascot.tsx:14-22`).
   - `ProgressBar.tsx` is a download-progress widget (`ProgressBar.tsx:3-26`).

   All verified-in-code.
7. **Frontmost app on macOS.** Needs no new crate: `NSWorkspace` / `NSRunningApplication` are in `objc2-app-kit 0.3.2` default features, already a dependency. verified-in-code
8. **Does dictation into a Handy window work?** Yes (C-76). verified-in-code
9. **Default app menu.** The Tauri default macOS menu is active and must be preserved when customizing it (C-203). verified-in-code

## Still open

1. **Flag** meaning: keep forever vs "wrong, fix it" (Q1). Product.
2. **Retention defaults:** how long to keep text and audio (Q2). Product. Disk math is now verified (1.92 MB per speech-minute).
3. **Undo AI edit:** does it re-paste into the target app (Q3)? Wispr behavior is `unknown`.
4. **Auto-added dictionary words:** which signal to learn from (Q4). Product/D.
5. **Word counting for CJK/Thai (Q6).** `unicode-segmentation 1.12.0` is in the lockfile as a transitive dependency (`Cargo.lock:7461-7462`, not a direct dependency). Its UAX #29 word rules likely split each Han character into its own "word" (inferred). WebKit's `Intl.Segmenter` may segment CJK with a dictionary (unknown for the macOS versions we target). Needs a decision, for example characters/2 for CJK.
6. **WPM basis:** speech time (`audio_ms`, VAD-trimmed) vs wall time (needs record start). Product, then a small capture change.
7. **Notifications:** in-app inbox only, or also macOS system notifications (Q8; E-91).
8. **Settings as a modal vs a page** (Q9). It decides the cost of C-12 and of the C-204 deep links.
9. **Update checks and cloud LLM providers** (Q10). Remove, default off, or disclose. Apple Intelligence and localhost are viable local providers (C-93 correction).
10. **Whether `core:default` grants window start-dragging** for an overlay title bar. Not checked in the Tauri ACL manifests. unknown
11. **Per-app categories for web apps** (Gmail in a browser counted as "email"). Needs the browser URL, which raises a privacy and Accessibility question (D-22). unknown
