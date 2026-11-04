# Audit OT2: what works, what does not (rc3, 2026-11-04)

The owner tested rc3 and found four silent failures: vibe coding in Cursor, a
custom transform, a take cancelled by a focus switch, and quiet speech that
was not heard. Other agents are fixing those four, plus the DMG, About and
support polish. This audit asks the owner's follow-up question, "what else
is broken that I haven't noticed?", for every feature and setting.

Branch `fix/audit` (from `wispr-bro` e35fe044), worktree
`~/Projects/wispr-bro-audit`.

## How it was checked

- **Static pass over every control.** For each one I traced the UI text, the
  frontend call, the Rust command, where the stored value is read, and
  whether that read changes behaviour. This covered all fork settings, every
  Handy row shown in Settings, the Hub pages, shortcuts, menus and tray,
  onboarding, the updater and notifications.
- **Takes through the real app, headless.** A debug build ran as test
  identity `com.wisprbro.auditcheck` in portable mode
  (`src-tauri/target/debug/Data`), with all shortcuts unbound (Esc too) and
  takes started by SIGUSR2. Audio came from the test WAV microphone
  (`WISPRBRO_TEST_MIC_WAV`), using the synthetic S-38 bench WAVs from
  `eval/perf/takes.jsonl`. No speakers, no real mic and no `say` were used.
  - **Paste target:** Handy's `external_script` paste method, pointed at a
    script that appends the final text to a file. Every check reads exactly
    what would have been pasted, without typing into any window.
  - **No downloads:** models were cloned or copied from `~/wb-bench`, and
    speech downloads went to a dead loopback `HF_ENDPOINT`. The model policy
    was pinned through `model_policy_state`.
  - Settings changed between app restarts (`fork-settings.json`,
    `settings_store.json`, `fork.db` rows). History came from `history.db`,
    behaviour from the log.
  - Scratch scripts are in the session scratchpad (`bin/{app,take}.sh`,
    `bin/cfg.py`, `bin/capture.sh`); they are not committed.
- **Two headless Rust checks on the real sidecar:**
  - `fork::transforms::long_polish_bench`, an ignored test that makes the
    same call as Scratchpad's Polish.
  - The launch activation guard tests.
- **Cited, not re-run:** earlier Mac-wave PASS results, where they cover a
  row (CONTEXT "no re-testing settled evidence").

**Verdicts:**
- **WORKS:** verified here.
- **WORKS (code):** the read site and the effect are in place and existing
  tests or Mac waves cover it; not re-run here.
- **PARTLY:** works, but not everywhere or not as the UI promises.
- **BROKEN:** does not do what it says.
- **NEEDS GUI:** cannot be judged without a window in front; listed for a
  Mac wave.
- **FIXED:** fixed on this branch; the original verdict is kept next to it.

### Something to know: the first four launches took focus

The first four launches of the test app took keyboard focus from whatever
app was in front. This happened before the cause was understood, and the
cause turned out to be bug B2 below. It is now fixed. Every later launch and
take was checked to leave the frontmost app unchanged (pid before = pid
after).

## Summary

101 rows:

| Verdict | Rows |
| --- | --- |
| WORKS (verified here) | 25 |
| WORKS (code, cited) | 42 |
| PARTLY | 14 |
| BROKEN | 17 |
| NEEDS GUI only | 2 |
| Owner issue with another agent (VAD) | 1 |

- **Fixed here:** 12 of the 17 BROKEN rows, plus the partly broken Dock row
  (B1 to B6 below).
- **Still broken, ours:** rows 18 and 19 (B7, Polish) and row 75 (conflict
  label).
- **Still broken, other agents:** rows 20 and 44 (custom transforms, vibe
  coding).
- **Also open:** every PARTLY row.
- **Owner's four issues:** not recounted.

### Top problems, by severity

1. **HIGH, data loss (fixed):** Settings › Advanced › History storage
   offered Handy's "Recording retention" and "History limit". Choosing
   anything but Never ran Handy's own cleanup at once and after every take.
   "Preserve limit" keeps only the newest **5** unsaved entries, text and
   audio included. This bypassed Data and Privacy's "Text is kept forever".
2. **HIGH (fixed):** every hidden launch took keyboard focus from the app
   in front: the login item, the quiet relaunch after an update, and
   `--start-hidden`. The S-28 "updates never interrupt" promise broke at
   the relaunch.
3. **HIGH (open):** Scratchpad Polish fails on any real note. It is rejected
   for more than one paragraph and times out past about 200 words. The
   Polish shortcut on a multi-paragraph selection uses the same guard.
4. **MEDIUM (fixed):** Shortcuts › Reset to defaults deleted ⌥1, ⌥2 and ⌥O.
5. **MEDIUM (fixed):** a fresh install showed four red "This combination
   can't be used" alerts in Shortcuts, for ⌥1, ⌥2, ⌥O and fn+Ctrl.
6. **MEDIUM (fixed):** "Show app in Dock" (on by default) lost the Dock
   icon on every hidden launch.
7. **MEDIUM (fixed, hidden):** eight more Advanced rows did nothing, or were
   undone at the next start:
   - Show live text in the bar
   - Show overlay
   - Post-processing toggle, provider and prompt editor
   - Experimental
   - Filler word removal
   - Shortcut activation
   - Keyboard backend

## Feature and setting table

### Take pipeline (run through the app)

| # | Feature / setting | Promise | Evidence | Verdict | Sev |
| --- | --- | --- | --- | --- | --- |
| 1 | Take → History → paste | text lands where you type | take.sh: entries 1-11 in `history.db`; final text arrived at the paste target every time | WORKS | |
| 2 | Cleanup level None | no AI cleanup, filler kept | entry 2: `cleanup: skipped none (level_none)`; `filler_word_removal_enabled` mirrored to false at start (`enforced ["filler_word_removal_enabled"]`) | WORKS | |
| 3 | Cleanup level Light | light cleanup | entries 1 and 9: `cleanup: applied light` (1091 ms, 2377 ms) | WORKS | |
| 4 | Cleanup level Medium | | entries 3-7: `applied medium` (313-1102 ms) | WORKS | |
| 5 | Dictionary replacement | "plumber → electrician" | entry 4: "call the electrician", `1 dictionary fix(es)` | WORKS | |
| 6 | Snippets | trigger → body | entry 5: pasted "review my PR number forty-two"; `snippets: 1 expanded`. History keeps the unexpanded text (by design?) | WORKS | |
| 7 | Style preset (Other = Relaxed) | relaxed punctuation | entries 5 and 7: `style: Relaxed for other (applied)`; final period dropped | WORKS | |
| 8 | Context awareness on | reads near the caret | `context: read terminal (… 5.8 ms)` | WORKS | |
| 9 | Context awareness off | never reads | entries 8 and 9: `context: read off` | WORKS | |
| 10 | Keep text / Keep recordings | retention | text 1 day, audio 0, clock +3 days: `retention: 9 row(s) deleted`; 9 → 0 rows and WAVs | WORKS | |
| 11 | Append trailing space (Handy) | adds a space | pasted text ends in a space (entry 8) | WORKS | |
| 12 | Paste method (external script) | | every take; ⌃V path soak cited (S-04, S-38.4) | WORKS | |
| 13 | Apply a transform after every dictation | Polish after each take | entries 10 and 11: `auto-apply 1 left the text unchanged in 726/755 ms`; no visible change on 2 of 2 short takes; logged nothing before this branch | PARTLY | low |
| 14 | Speech model policy and pin | silent upgrade; pin stops it | unpinned: `Download parakeet-unified-en`; offline retry back-off 5/15/30 s; pinned: `→ Stay` | WORKS | |
| 15 | Cleanup sidecar warm-up | | `llama-server ready in 5867 ms`; first take used it | WORKS | |
| 16 | Hidden launch keeps the app in front | quiet, never interrupts (S-28) | before the fix: test pid frontmost after `--start-hidden`. After: frontmost pid unchanged over 12 s of polling (0.3 s steps) and across takes | BROKEN → FIXED (B2) | high |
| 17 | Show app in Dock at a hidden launch | | before: `type="UIElement"` with show_in_dock true. After: `type="Foreground"` | BROKEN → FIXED (B6) | med |
| 18 | Scratchpad Polish | Polish the note | `long_polish_bench`: 59 words ok 703 ms; 135 `Guard([LineCount])`; 226 `Timeout` 2801 ms; 336 `Guard([LineCount, ContentWords])`; 490 and 675 `Timeout` | BROKEN (B7) | high |
| 19 | Polish shortcut on a multi-paragraph selection | | same `generate_within(flexible=false)` path, same LineCount guard (row 18) | BROKEN (B7) | med |
| 20 | Custom transforms, Prompt Engineer | | TooLong (3× input) and ExecutedInstruction stay on for flexible definitions (`run.rs:56-90`, `guards.rs:205-280`). This is the likely root cause of the owner's custom-transform failure | BROKEN (other agent) | high |
| 21 | Command Mode "expand / write" | | `command.rs:359-364` test asserts expansion is rejected as TooLong | PARTLY | med |

### Settings › General and System

| # | Setting | Evidence | Verdict | Sev |
| --- | --- | --- | --- | --- |
| 22 | Dictation shortcut summary and Change | `fork_set_setting("hotkey")` → `shortcut::reload` | WORKS (code) | |
| 23 | Microphone picker / Auto-detect | `commands/audio.rs`; S-08 Mac | WORKS (code); real devices NEEDS GUI | |
| 24 | My languages | read per take (`fork/mod.rs`, `post_process.rs`, model policy: `policy for ["en"]`) | WORKS | |
| 25 | Chinese script | `managers/transcription.rs` | WORKS (code) | |
| 26 | App language, Theme | | WORKS (code) | |
| 27 | Launch at login | Mac wave 3 S-29a.7 | WORKS (code) | |
| 28 | Always show the bar | `pill/mod.rs:665` via `settings_changed` | WORKS (code) | |
| 29 | Show app in Dock | close path ok (Mac wave 1); hidden launch row 17 | PARTLY → FIXED | med |
| 30 | Show tray icon | turning it off while Dock is off leaves no Dock icon and no tray (`shortcut/mod.rs:1467` never re-checks the policy) | PARTLY | low |
| 31 | Start hidden | every audit launch | WORKS | |
| 32 | Sounds (on, theme, volume, preview), Output device | `audio_feedback.rs` (not played: no speakers) | WORKS (code) | |
| 33 | Notifications: the 8 switches | all honoured (`notify/policy.rs`); "Speech model ready", "Update ready", "Queued dictations ready" and "Problems" only gate Mac banners, and the inbox always gets them. "Problems" covers shortcut problems only | PARTLY (wording) | low |
| 34 | When Scratchpad opens | `scratchpad/window.rs:134` | WORKS (code) | |
| 35 | Add Scratchpad to the bar | `pill/mod.rs:1257` | WORKS (code); NEEDS GUI | |

### Settings › Data and Privacy, Vibe coding, About

| # | Setting | Evidence | Verdict | Sev |
| --- | --- | --- | --- | --- |
| 36 | Where your data lives › Show in Finder | opens Finder | NEEDS GUI | |
| 37 | Keep text / Keep recordings | row 10 | WORKS | |
| 38 | Use context awareness | rows 8 and 9 | WORKS | |
| 39 | Check for app updates | mirrored at once and at start (`ENFORCED`) | WORKS (code) | |
| 40 | Learn from my corrections | Mac wave 2 S-34.1-.7 | WORKS (code) | |
| 41 | Export my data | Mac wave 1 S-27.1 | WORKS (code) | |
| 42 | Delete all my data | S-27.2 | WORKS (code) | |
| 43 | Copy / Save diagnostics | S-27.5 | WORKS (code) | |
| 44 | Vibe coding (code names, file tags) | owner report | BROKEN (other agent) | |
| 45 | About, Report a problem, Send feedback, Share download link | all point at `github.com/fjenn/wispr-bro`, which is empty on purpose | PARTLY until the push | med |

### Settings › Advanced (Handy rows included)

| # | Setting | Evidence | Verdict | Sev |
| --- | --- | --- | --- | --- |
| 46 | Speech models panel / pin | row 14 | WORKS | |
| 47 | AI engine panel | `forkLlm*` | WORKS (code) | |
| 48 | Cleanup level picker | rows 2-4 | WORKS | |
| 49 | Paste method | row 12; methods other than ⌃V lose verified paste and Return (by design) | WORKS | |
| 50 | Typing tool, Clipboard handling | `clipboard.rs`, `insert/mac.rs:81` | WORKS (code) | |
| 51 | Auto submit | `insert/mac.rs:66`; test `auto_submit_only_after_the_text_arrived` | WORKS (code) | |
| 52 | Append trailing space | row 11 | WORKS | |
| 53 | Shortcut activation (Handy) | read only in `shortcut/handler.rs:47` for the `transcribe` binding, which the fork owns | BROKEN → FIXED (row hidden) | med |
| 54 | Channel, Long-form chunking, Unload timeout, Acceleration, Lazy close, VAD backend, Language detection | read in Handy's managers | WORKS (code) | |
| 55 | Voice activity detection / quiet speech | owner report | other agent | |
| 56 | Translate to English | both parakeet starters and unified-en have `translate: false` (`managers/transcription.rs:1765`); the row is not disabled | PARTLY | low |
| 57 | Filler word removal | re-mirrored from the cleanup level at every start (`ENFORCED`) | BROKEN → FIXED (row hidden) | low |
| 58 | Custom words (Handy) | takes see it at once; the Dictionary page only on the next `migrate::sync` | PARTLY | low |
| 59 | Bar position, Hide for an hour, Hide in recordings, Reset position | `pill/mod.rs`, `panel.rs` | WORKS (code); NEEDS GUI | |
| 60 | Show live text in the bar | `live_text_in_pill` read nowhere (Rust and `src/fork/pill`) | BROKEN → FIXED (row hidden) | med |
| 61 | Show overlay (style, position) | `overlay_style` forced None; Handy's overlay never built | BROKEN → FIXED (row hidden) | low |
| 62 | Keyboard backend | the fork tap always uses handy-keys (`fork/shortcut/listener.rs`) | BROKEN → FIXED (row hidden) | low |
| 63 | Post-processing toggle, provider, prompt editor | toggle forced off; the fork's `plan()` never reads Handy's provider or prompts | BROKEN → FIXED (rows hidden) | med |
| 64 | History limit, Recording retention | `commands/history.rs:98-139` → `cleanup_old_entries` at once and after every take until the next start | BROKEN → FIXED (rows hidden) | high |
| 65 | Update channel | `update/mod.rs:98` → `channel.rs` | WORKS (code) | |
| 66 | App data / log folders | display | WORKS (code) | |
| 67 | Experimental | forced off at start | BROKEN → FIXED (row hidden) | low |
| 68 | Debug tools | | WORKS (code) | |

### Shortcuts

| # | Feature | Evidence | Verdict | Sev |
| --- | --- | --- | --- | --- |
| 69 | Hold to talk, hands-free double-tap and fn+Space, Esc | `bindings.rs:180-183`; S-01 Mac | WORKS (code) | |
| 70 | Paste last, Copy last, Press Return, Undo a cancel, mouse buttons | S-06 Mac 10/10 | WORKS (code) | |
| 71 | Polish, Prompt, View changes | Mac wave 1 Polish and ⌥O PASS | WORKS (code) | |
| 72 | Reset to defaults | `HotkeysPanel.tsx:71-87` saved `defaults[id] ?? []`, and the TS table lacked the transforms. New test `scripts/wb-hotkey-defaults.test.ts` fails on the old table and passes on the new one | BROKEN → FIXED (B4) | med |
| 73 | Default install shows no errors | `bindings.rs:185-226` reported transforms and Command Mode as Unsupported while they were off. New test `opt_in_features_that_are_off_report_no_issues` | BROKEN → FIXED (B5) | med |
| 74 | "Add up to 4 shortcuts for each action" | dictate and hands-free accept one trigger, cancel only Esc; the "+" still shows | PARTLY | low |
| 75 | Conflict text for a custom transform | `BindingRow.tsx:139-141` shows the raw key `transform.custom-N` | BROKEN (open) | low |
| 76 | Scratchpad shortcut | Mac wave 2 S-24.1 | WORKS (code) | |
| 77 | Command Mode fn+Ctrl | reload on toggle | WORKS (code) | |

### Hub pages

| # | Feature | Evidence | Verdict | Sev |
| --- | --- | --- | --- | --- |
| 78 | Home greeting, stats, banners, checklist | S-15a; `dismissed_banners` | WORKS (code) | |
| 79 | History list, search, copy, Undo AI edit, Retry, Flag, delete, play | S-15b | WORKS (code) | |
| 80 | Insights Usage/Voice, typing wpm, Save as image | S-25 | WORKS (code) | |
| 81 | Dictionary add/edit/delete, import/export, Suggested | S-16; rule verified here (row 5) | WORKS | |
| 82 | Dictionary / Snippets Undo after a delete | result not checked (`DictionaryPage.tsx:284`, `SnippetsPage.tsx:178`) | PARTLY | low |
| 83 | Snippets page | S-17; expansion verified here (row 6) | WORKS | |
| 84 | Style groups, presets, app chips, sites | S-19a/b; preset verified here (row 7) | WORKS | |
| 85 | Press Enter after dictating | S-19a | WORKS (code) | |
| 86 | Transforms page: enable, custom CRUD, editor | `commands/transforms.rs` | WORKS (code) | |
| 87 | Transform editor Preview | a failed preview says "Couldn't save the latest change" (`TransformModal.tsx:107-108`) | PARTLY | low |
| 88 | Scratchpad page and window | S-24 PASS | WORKS (code) | |
| 89 | Scratchpad text size | client state only; reset after the window is parked (60 s) | PARTLY | low |
| 90 | Share page | link 404 until a release exists | PARTLY | med |
| 91 | Notifications inbox | S-18 | WORKS (code) | |

### Menus, tray, onboarding, updater

| # | Feature | Evidence | Verdict | Sev |
| --- | --- | --- | --- | --- |
| 92 | App menu (Copy last, Microphone, Languages, Add word, New snippet, Help) | S-26a.0-.4 | WORKS (code) | |
| 93 | Tray (Open, Check for updates, Paste last, Quit) | S-26a | WORKS (code) | |
| 94 | Help › Documentation | opens `…/tree/wispr-bro/docs/fork/user` on the empty repo | PARTLY | med |
| 95 | Paste last from a tray click (focus) | | NEEDS GUI | |
| 96 | Onboarding languages → starter model | `starter.rs::pick`; S-07b | WORKS (code) | |
| 97 | Globe consent and restore | RS-01, `globe::reconcile` | WORKS (code) | |
| 98 | Typing test → Insights, time saved | `commands/insights.rs:36` | WORKS (code) | |
| 99 | Practice takes | S-29b | WORKS (code) | |
| 100 | Update checks, channels, quiet relaunch | S-28; relaunch focus see row 16 | WORKS (code) | |
| 101 | Hub reopen and window restoration | S-38.3, S-27.6 | WORKS (code); visual NEEDS GUI | |

## Bugs fixed on this branch

- **B1, data loss: Advanced rows that lied.**
  - *Rows hidden:* History limit, Recording retention, Show live text,
    Show overlay, Keyboard backend, the post-processing toggle, provider and
    prompt editor, Experimental, Filler word removal, Shortcut activation.
  - *Root cause:* Handy's settings were hosted as-is. The fork overrides
    them (`fork::settings::ENFORCED`), bypasses them (its own `plan()` and
    hotkey engine) or never reads them (`live_text_in_pill`). The two
    History rows call Handy's `cleanup_old_entries` at once and after every
    saved take.
  - *Fix:* `src/fork/settings/Advanced.tsx`. The settings themselves and
    Handy's code are unchanged. `tests/fork/settings.spec.ts` asserts the
    live-text row and every number field are gone, and its persistence test
    uses Update checks instead. The S-37.3 number-field a11y test keeps its
    every-section guard.
- **B2, a hidden launch took focus.**
  - *Root cause:* tao's `applicationDidFinishLaunching:` calls
    `activateIgnoringOtherApps:YES`, and Handy's `.setup()` builds the
    hidden Hub (wry calls `activate()`).
  - *Fix:* app activation is off from the fork plugin's `setup` (inside
    `Builder::build`, before the launch) until `fork::on_ready`. A launch
    that showed the Hub focuses it there (`pill/activation.rs`
    `suppress_at_launch` / `end_launch`, `fork/mod.rs`
    `end_launch_activation_guard`).
  - *Tests:* `the_launch_guard_lasts_until_end_launch`, plus the frontmost
    check in row 16.
- **B3, auto-apply ran silently.** `transforms/auto.rs` now logs the outcome
  and time: changed, unchanged, failed open, throttled or no engine. It
  never logs the text. Without this a Mac run cannot tell "never ran" from
  "left the text unchanged" (row 13).
- **B4, Reset to defaults dropped the transform shortcuts.** `TRANSFORM_DEFAULTS`
  was added in `src/fork/settings/hotkeys/model.ts`. The parity test
  `scripts/wb-hotkey-defaults.test.ts` compares it with Rust's
  `default_hotkeys()`.
- **B5, false "can't be used" alerts.**
  `fork::shortcut::active_hotkeys` keeps the bindings of features that are
  off (Transforms, Command Mode) in the settings but does not compile them
  until the feature is on.
- **B6, Dock icon lost at a hidden launch.**
  `fork::keep_dock_icon_at_hidden_launch` sets Regular after the settings
  load when Show in Dock is on. Setting Regular does not activate the app.
  Fork-only: no `lib.rs` budget change.

## Bugs open, with root cause and fix plan

- **B7 (high): Polish fails on long or multi-paragraph text.** This hits
  Scratchpad Polish, the Polish shortcut, Preview and auto-apply.
  - *Root causes:*
    - The fixed `LLM_BUDGET` of 2.8 s (`transforms/run.rs:22`) for every
      transform.
    - Polish keeps the cleanup guards LineCount and ContentWords
      (`flexible=false`), which reject a rewrite that changes the
      paragraph count.
  - *Plan:*
    - Size the budget with `llm::deadline::request`, as cleanup does,
      keeping the fixed 2.8 s only for the interactive selection hotkey.
    - Give Scratchpad and Preview a longer, cancellable budget with
      progress on the bar.
    - Make Polish's LineCount check compare paragraph counts with a
      tolerance, and turn it off when the "headings and lists" rule is on.
    - Re-run `long_polish_bench`.
  - *Related, for the transform agent:* the TooLong and ExecutedInstruction
    guards on flexible definitions (row 20), and Command Mode expansion
    (row 21).
- **Notifications wording (low, row 33):** nest the four kind switches
  under "Mac notifications" ("Also show on the Mac"), or gate the inbox too.
- **Tray off with Dock off (low, row 30):** when the tray goes off with
  `show_in_dock` false, set Regular. This needs a hook in Handy's
  `change_show_tray_icon_setting`, or a fork command for the row.
- **Translate to English (low, row 56):** disable the row with a hint when
  the current model cannot translate.
- **Show live text (row 60):** wire Handy's `StreamTextEvent` (streaming
  models only; unified-en streams) into the pill behind
  `live_text_in_pill`, then show the row again.
- **Custom words live (low, row 58):** run `dictionary::migrate::sync` after
  Handy's `update_custom_words`, or hide that row too, since the Dictionary
  page owns words.
- **Shortcut "+" limits (low, row 74):** `canAddBinding` per action (1 for
  dictate and cancel, 2 forms for hands-free); fix the description.
- **Conflict label (low, row 75):** resolve `transform.custom-N` to the
  transform's name in `BindingRow`.
- **Preview error text (low, row 87):** a separate preview-error state and
  string. This is a new string, so it needs the translation batch.
- **Undo result ignored (low, row 82):** check `status` and show the
  existing save-failed toast.
- **Scratchpad text size (low, row 89):** store it as a fork setting.
- **Empty repo links (medium, rows 45, 90, 94):** resolved by the planned
  push. Until then the links 404.
- **Snippets in History (question, row 6):** History stores the text before
  snippet expansion. Decide whether History should show what was inserted.

## Queued for a Mac wave (needs a window in front)

- Show in Finder for the four folders.
- The bar's position, hide-for-an-hour, hide-in-recordings and Scratchpad
  slot, by eye.
- Paste last from a tray click.
- Real microphones in the picker.
- **Hidden launch re-check on the signed release bundle:** login item and
  update relaunch keep the frontmost app and the Dock icon. Visible launch:
  the Hub comes forward focused.
- Settings › Advanced by eye: the hidden rows are gone, and nothing else
  moved.

## Checks run for the changed code

| Check | Result |
| --- | --- |
| `cargo test --lib` | 1,654 passed, 36 ignored (before a clippy-only rewrite of one new test) |
| fork clippy | 0 findings after that rewrite |
| `cargo fmt --check` | pass |
| `bun run lint` | pass |
| `npx tsc --noEmit` | pass |
| `bun run check:translations` | pass |
| `bun run wb:check:translations` | pass |
| `bun scripts/wb-guards.ts` | pass |
| `bun run wb:format:check` | pass |
| `bun test scripts/wb-hotkey-defaults.test.ts` | pass |
| model test | pass |
| Playwright: the changed and related specs (`settings`, `a11y-sweep`, `hotkeys`; installed Chrome, headless, port 1497) | 93 passed. The full suite did not run: process launches on the Mac stalled for about 20 minutes. Run it before merge |

No new strings and no migration were added. Handy's files are untouched.
