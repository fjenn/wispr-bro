# Manual Mac test script

The checks that automated tests cannot prove: real hotkeys, Accessibility,
the pasteboard, the menu bar, permissions, signing, memory and latency on real
hardware. Source: R5 §11.3 (outline) and every slice's **Real-Mac
acceptance** line in R5 §6 and §8.

**Status: skeleton (F-10).** Sections and check ids are fixed; each slice
replaces its `TODO` procedure with exact steps when it lands (R5 §8: "every
slice … updates `docs/fork/MAC-TEST-SCRIPT.md` with its real-Mac checks"). A
check whose slice is not built yet is recorded as `N/A (not built)`.

## When to run

| Run                | Scope                                                                               | Machines                     |
| ------------------ | ----------------------------------------------------------------------------------- | ---------------------------- |
| Slice done         | That slice's block in Part B                                                        | At least one reference Mac   |
| Release candidate  | All of Part A (≈ 3 h per machine), plus Part B for slices changed since the last RC | All reference Macs           |
| Patch release      | Part A §A1, §A2, §A4, §A9, §A10 (abridged) plus the fixed area                      | M1 8 GB and one macOS 26 Mac |
| Before 1.0 (R5 §9) | Everything, plus the full soak (500 takes, 50-take hardware run, 8 h idle)          | All reference Macs           |

Spikes (RS-01 … RS-14) are not run from this script; each writes
`docs/fork/spikes/RS-xx.md`. Part B references them where a slice re-runs a
spike's procedure "on the real slice".

## Reference machines

Apple Silicon only (DEC-29: no Intel build in M1/M2; R5 §11.3 and §9 still
name an Intel Mac, which DEC-29 skips). Together the machines must cover
macOS 13, 14, 15 and 26 (R5 §9).

| Id   | Machine                 | macOS                              | Used for                                                                    |
| ---- | ----------------------- | ---------------------------------- | --------------------------------------------------------------------------- |
| M1-8 | M1, 8 GB                | 14 (Sonoma)                        | Low-RAM tier: latency, memory, LLM tier (§5.1, §5.2, RS-06)                 |
| M-15 | M-series, 16 GB or more | 15 (Sequoia)                       | Default tier                                                                |
| M-26 | M-series, 16 GB or more | 26 (Tahoe)                         | Current macOS: Apple FM (S-12b), Dock toggle (S-14), Globe settings (RS-01) |
| M-13 | Any Apple Silicon Mac   | 13 (Ventura), the minimum (DEC-22) | Baseline: launch, permissions, Metal, notifications (RS-10)                 |

M-15 and M-26 may be the same Mac on two volumes. Record the exact model
identifier, RAM and macOS build (`sw_vers`) in every run.

## Evidence format

One file per run: `docs/fork/test-runs/<version>-<machine-id>.md`
(e.g. `1.0.0-alpha.3-M1-8.md`). Attachments (log excerpts, measurement CSVs,
screenshots) go in `docs/fork/test-runs/<version>-<machine-id>/`.

**No personal data in evidence.** Use a test macOS account with no real mail,
contacts or messages; dictate only the scripted sentences; crop or blur
anything else on screen. Never put files in `docs/research/evidence/` (local
only, gitignored) and never include Wispr Flow screenshots.

Header of the run file:

```markdown
# Mac test run: <version> on <machine-id>

| Field   | Value                                         |
| ------- | --------------------------------------------- |
| Version | 1.0.0-alpha.N (git <short sha>)               |
| Build   | signed + notarized DMG / Homebrew / dev build |
| Machine | MacBookAir10,1, M1, 8 GB                      |
| macOS   | 14.7.1 (23H222)                               |
| Tester  | <name or handle>                              |
| Date    | YYYY-MM-DD                                    |
| Scope   | RC full / patch abridged / slice S-xx         |
| Result  | PASS / FAIL (n failed, m blocked)             |
```

One row per check, in the order of this script:

```markdown
| Check   | Result | Evidence                                            | Notes      |
| ------- | ------ | --------------------------------------------------- | ---------- |
| A2.3    | PASS   | 100/100 chords native, 0 starts; `A2.3-chords.csv`  |            |
| S-04.2  | FAIL   | 2 lost insertions in 100 (Notion); log `S-04.2.log` | issue #123 |
| S-12b.1 | N/A    | not built                                           |            |
```

- **Result** is one of `PASS`, `FAIL`, `BLOCKED` (could not run; say why) or
  `N/A` (slice not built, or the check does not apply to this machine).
- **Evidence** names the measurement against the pass criterion (counts,
  p50/p95 in ms, RSS in MB) and the attachment file, if any. A `PASS` without
  a number is accepted only for yes/no checks.
- A `FAIL` links an issue. The run's overall result is `FAIL` if any check
  fails.
- Timings use the debug panel or log timestamps, not a stopwatch, unless the
  check says otherwise.

---

# Part A: release-candidate script (R5 §11.3)

Each item lists the slices whose Part B checks it bundles. Fill the
procedures as those slices land.

### A1. Clean install, Gatekeeper, permissions, onboarding timing

Slices: F-01, F-02, S-29a, S-29b, S-39.

- [ ] A1.1 Clean install from the DMG on a Mac that never had Wispr Bro (fresh macOS user): Gatekeeper opens it without a warning. TODO (F-02)
- [ ] A1.2 Clean install with Homebrew (`brew install --cask …`). TODO (S-39)
- [ ] A1.3 Microphone and Accessibility prompts name "Wispr Bro"; Handy, if installed, keeps its own settings and TCC entries (F-01).
- [ ] A1.4 Onboarding timing: first dictation ≤ 2 min after install and ≤ 60 s after permissions (§5.3). TODO (S-29a, S-29b)

### A2. Hotkeys

Slices: S-01, S-02, RS-01, RS-14.

- [ ] A2.1 Grammar table: hold, tap, double-tap, fn+Space, Esc (with and without fn held). Procedure: Part B S-01 steps 3, 5, 6, with real keys.
- [ ] A2.2 Fallback profile Ctrl+Opt on an external non-Apple keyboard. Procedure: Part B S-01 step 7, with real keys.
- [ ] A2.3 Chords: 100 fn+arrow / fn+F-key / fn+Delete keep their native function, 0 insertions; fluent chords never show the orange indicator. Procedure: Part B S-01 step 4, with real keys.
- [ ] A2.4 Secure Input cases (RS-14: Safari password field, Terminal Secure Keyboard Entry, iTerm2). TODO (S-02)
- [ ] A2.5 Sleep/wake ×10: hotkey works within 2 s of wake. TODO (S-02)
- [ ] A2.6 Revoke and re-grant Accessibility while running: told within 2 s, back within 5 s without relaunch. TODO (S-02)

### A3. Flow Bar

Slices: S-03, S-30.

- [ ] A3.1 States: resting, listening, locked, processing, messages, errors (S-03.6).
- [ ] A3.2 Click to start without stealing focus; hover tooltip; drag; "Hide for an hour" (S-03.2, S-03.7, S-03.8).
- [ ] A3.3 Displays: two displays with mixed scale, unplug/replug, full screen, Spaces, top position (S-03.4, S-03.11).

### A4. Insertion matrix

Slices: S-04, S-05, S-06, S-17. Apps (§5.4): Mail, Notes, TextEdit, Slack, VS Code, Chrome (Gmail), Safari, Notion, Messages, Terminal.

- [ ] A4.1 10 apps × (short sentence, multi-line, snippet, no text field focused, password field). TODO (S-04, S-05, S-17)
- [ ] A4.2 Clipboard restore types: plain text, RTF, HTML, file URL, image, multi-item. See S-04.1 (`wb-spike-clipboard --s04-soak`).
- [ ] A4.3 Clipboard managers (Maccy, Raycast, Paste) do not record dictated text. TODO (S-04)
- [ ] A4.4 Japanese IME and Pinyin active during paste. See S-04.5 (`--s04-ime`; Japanese only, Pinyin by hand).
- [ ] A4.5 App switch during processing → card, never the wrong field. TODO (S-05)

### A5. First run and network conditions

Slices: S-07a, S-07b, S-07c.

- [ ] A5.1 First run at 10 and 50 Mbit/s (Network Link Conditioner) meets §5.3. See S-07a.1.
- [ ] A5.2 Offline first launch with queued takes, then online. TODO (S-07c)
- [ ] A5.3 Captive portal Wi-Fi. See S-07a.3 (simulated) and S-07a.5 (real, a person).
- [ ] A5.4 Language change re-picks the model. See S-07b.3.

### A6. Cleanup, styles, context, voice commands

Slices: S-10, S-11, S-12a, S-12b, S-19a, S-19b, S-20.

- [ ] A6.1 Cleanup levels None / Light / Medium; "Inserted without cleanup" on a forced timeout (debug build: `WB_DEBUG_CLEANUP_DEADLINE_MS=1`; see S-12a below).
- [ ] A6.2 Styles per app group; AI-chat apps skip LLM formatting. TODO (S-19a, S-12a)
- [ ] A6.3 Context awareness. TODO (S-20)
- [ ] A6.4 Voice commands: "new line", "new paragraph", "scratch that". TODO (S-11)

### A7. Transforms and Command Mode

Slices: S-21, S-22, S-23.

- [ ] A7.1 Transforms, diff, auto-apply. TODO (S-21, S-22)
- [ ] A7.2 Command Mode on and off; ⌘Z restores. TODO (S-23)

### A8. Hub

Slices: S-13 … S-18, S-24, S-25, S-26a, S-26b.

- [ ] A8.1 History actions and search. TODO (S-15b)
- [ ] A8.2 Dictionary, snippets, insights, scratchpad. TODO (S-16, S-17, S-24, S-25)
- [ ] A8.3 Notifications. TODO (S-18)
- [ ] A8.4 Settings, menus, tray. TODO (S-14, S-26a)

### A9. Updates

Slices: F-02, S-28, RS-12.

- [ ] A9.1 Update installs only when idle (never while recording, processing or within 20 min of a take). TODO (S-28)
- [ ] A9.2 Beta channel; a stable install ignores beta. TODO (F-02, S-28)
- [ ] A9.3 Microphone and Accessibility permissions survive the update. TODO (RS-12)

### A10. Privacy

Slices: F-03, F-08, S-27.

- [ ] A10.1 Egress capture (Little Snitch or `nettop`) for 10 min of use: only `github.com` (update check) and, during downloads, Hugging Face hosts (R5 §1). TODO (F-08)
- [ ] A10.2 Offline run: every M1 feature works with networking off (§5.4). TODO
- [ ] A10.3 Export, delete all. TODO (S-27)

### A11. Accessibility

Slices: S-37.

- [ ] A11.1 VoiceOver checklist (onboarding, Hub, settings, one dictation). TODO (S-37)
- [ ] A11.2 Keyboard-only use; Increase Contrast; Reduce Motion. TODO (S-37)

### A12. Soak

Slices: S-38.

- [ ] A12.1 100 takes across the §5.4 apps: 0 lost, 0 duplicated, 0 wrong-target insertions. TODO (S-38)
- [ ] A12.2 2 h idle: RSS and CPU within §5.2. TODO (S-38)
- [ ] A12.3 Before 1.0 only: 500-take soak, 50-take hardware run, 8 h idle. TODO (S-38)

---

# Part B: per-slice real-Mac acceptance

Copied from each slice's **Real-Mac acceptance** line in R5 (the source of
truth; if R5 changes, update here). Ids are `<slice>.<n>`. "Procedure" is
filled by the slice; "Machines" defaults to M1-8 and M-26 unless stated.

## M0 foundations

### F-01 Identity overlay and rebrand basics

- [ ] F-01.1 Clean Mac: the Microphone and Accessibility prompts show "Wispr Bro".
- [ ] F-01.2 The tray tooltip shows our version.
- [ ] F-01.3 The app installs side by side with Handy without sharing settings or TCC entries.
- Procedure: TODO (F-01). Machines: any.

### F-02 Signing, notarization, updater, release workflows

- [ ] F-02.1 RS-12: DMG stapled; `1.0.0-alpha.1 → alpha.2` update keeps permissions.
- Procedure: see `docs/fork/spikes/RS-12.md`. Machines: M-13, M-26.

### F-03 CI guards

- [ ] F-03.1 `wb-egress.yml` passes on the macOS runner with the alpha build.
- Procedure: CI run link. Machines: macOS runner.

### F-04 GPL compliance, F-05 scaffolding, F-06 `fork.db`, F-07 settings store, F-09 upstream sync

- R5 lists no real-Mac acceptance for these slices; their checks are automated.

### F-08 Privacy cleanups

- [ ] F-08.1 In a real `wb:build`, the CSP smoke panel plays a recording through `convertFileSrc`, renders an element to a `data:` and a `blob:` image, and calls a fork command, with no CSP violation in the console.
- [ ] F-08.2 Handy's own history page still plays audio.
- [ ] F-08.3 10-minute session with `nettop` / Little Snitch: only `github.com` (update check) contacted.
- Procedure: TODO (F-08).

### F-10 Test and eval harness

- [ ] F-10.1 `bun run wb:eval:cleanup` against a local `llama-server` produces a report for one model in under 15 min on M1-8 (R5 F-10 exit criterion). Procedure: `eval/cleanup/README.md` → "Running against a model" (first real run: RS-07).
- [ ] F-10.2 Inter-labeller agreement for `eval/cleanup/items.jsonl` recorded in `eval/cleanup/README.md` (human task; pending).

## M1 slices

### S-01 Hotkey engine

- [ ] S-01.1 RS-01 and RS-14 outcomes applied: with "Press 🌐 key to" = Do
      Nothing the status (`fork_hotkey_status`) says `fn_blocked: false`; with
      Start Dictation it says `true` (re-read when a take ends).
- [ ] S-01.2 20 push-to-talk takes, 20 double-tap locks, 20 fn+Space locks.
- [ ] S-01.3 100 fn chords (fn+arrows = Home/End/PgUp/PgDn, fn+Delete, fn+F-keys):
      0 insertions, the key reaches the app (native function), 50 typed fluently
      (key within 150 ms: no take, mic never runs) and 50 with fn held first
      (take dropped, never pasted, DEC-54).
- [ ] S-01.4 50 quick taps (< 150 ms): no take, mic never runs.
- [ ] S-01.5 Esc with fn held cancels; Esc while transcribing cancels; idle
      Esc reaches the app.
- [ ] S-01.6 Ctrl+Opt fallback on an external non-Apple keyboard.
- [ ] S-01.7 Under Secure Input (Terminal Secure Keyboard Entry, a Safari
      password field): fn hold and double-tap work.
- [ ] S-01.8 fn-down → capture running: p50 ≤ 250 ms, p95 ≤ 350 ms (§5.1).
- [ ] S-01.9 Idle: the app never runs the microphone (no orange dot) while
      no take is active, also right after a take.
- [ ] S-01.10 Text lands in TextEdit after a push-to-talk take.
- [ ] S-01.11 `--toggle-transcription` and `--cancel` on a running instance
      still start/stop and cancel.

Procedure (automated where it can be; a person repeats S-01.3, S-01.6 and
S-01.7 with real keys on each reference Mac):

1. Build the product app (`bun run wb:build -- --bundles app`) and start
   its binary from a Terminal that has Accessibility and Microphone access
   (`"…/Wispr Bro.app/Contents/MacOS/handy" --debug`), so the log has the
   trace lines. The log says `fork hotkeys: tap running (trigger Some("fn"),
Globe …, fn blocked …)` and `fork audio: microphone prepared`.
2. Driver: `swiftc -O scripts/wb-s01-keys.swift -o /tmp/wbkeys`. It posts
   synthetic fn / key events (CGEvent, fn = flagsChanged keycode 63 with the
   SecondaryFn flag) and reads per-process microphone use
   (`WB_MIC_PID=<app pid>`; other apps may hold the mic). Synthetic events
   do not trigger the Globe action (RS-01 covers that with real keys) and a
   real fn chord arrives as Home/End/PageUp/PageDown/Forward Delete with the
   fn flag, so post those keys (`key home` while fn is down).
   `WB_LISTEN=1` reports which key-downs got past the app's tap.
3. S-01.2: `fn down; waitmic on 1000; wait 1000; fn up; waitmic off 1500`
   ×20; double-tap `fn down; wait 80; fn up; wait 200; fn down; wait 60; fn
up; wait 1200; fn down; wait 70; fn up` ×20; fn+Space `fn down; wait N;
key space; wait 100; fn up; wait 1000; fn down; wait 70; fn up` ×20. In
   the log each gives `External command Start, Stop` / `Start, Lock, Stop`.
4. S-01.3: `fn down; wait 40-100; key <home|end|pageup|pagedown|
forwarddelete|f1…f12>; wait 80; fn up; watchmic 400` ×50 and the same
   with `wait 600-1300` ×50, with `WB_LISTEN=1`: the key code is in
   `passed_keycodes`; fluent → no `External command`, held → `Start,
Cancel`. Text check: TextEdit "hello", Cmd+Up, fn+Forward Delete →
   "ello" in both timings.
5. S-01.4: `fn down; wait 50-140; fn up; watchmic 700` ×50 → `mic_ran`
   false, no command.
6. S-01.5: `fn down; waitmic on 1000; wait 400-1200; key escape; wait 200;
fn up` → `Start, Cancel`.
7. S-01.6: switch the profile (`fork_use_hotkey_profile(true)`, or set
   `hotkeys.dictate` to `ctrl+option` hold and `hotkeys.hands_free` to
   `ctrl+option` double-tap + `ctrl+option+space` in `fork-settings.json`
   and relaunch); `mods down ctrl,option; … ; mods up option; mods up ctrl`.
8. S-01.7: turn on Terminal > Secure Keyboard Entry (or focus a Safari
   password field), check `ioreg -l -w 0 | grep kCGSSessionSecureInputPID`,
   repeat the hold and double-tap; turn it off again. Do not post Space or
   Esc while Terminal is frontmost.
9. S-01.8: pair each `fork hotkeys: modifiers Fn down=true (unix_us …)`
   with the following `first audio of the recording drained … (unix_us …)`
   in the log (release build).
10. S-01.10: TextEdit frontmost, `say` a sentence during a 4 s hold; the
    text appears.
11. S-01.11: run the binary again with `--toggle-transcription` (twice) and
    with `--cancel` while recording.

Machines: M1-8, M-15, M-26. Results of the first run (M5 Max, macOS
26.6.2, synthetic events) are in R5 under S-01 "As built".

### S-02 Hotkey self-healing, permission health, Secure Input

- [ ] S-02.1 RS-02 procedure on the signed build passes.
- [ ] S-02.2 RS-14 procedure on the signed build passes.
- Procedure: TODO (S-02).

### S-03 Flow Bar

- [ ] S-03.1 RS-03 criteria on the real slice: first click (resting bar and
      ✓) acts without focus loss 20/20; hover bubble ≤ 200 ms 20/20; bar on
      every Space and over a full-screen app; after display sleep/wake and
      display plug/unplug, placed correctly on 1× and 2× displays; "Hide the
      bar in screen recordings" hides it from `screencapture` (still and
      `-v`) and a ScreenCaptureKit recorder; idle RSS and CPU within §5.2.
- [ ] S-03.2 Click-to-start in TextEdit, Slack, Chrome inserts into that app
      (keeps focus).
- [ ] S-03.3 The waveform moves with speech.
- [ ] S-03.4 The bar follows the cursor's display within 1 s.
- [ ] S-03.5 Idle 8 h memory run (100 takes): pill web view growth ≤ 50 MB.
- [ ] S-03.6 States look right in light and dark, with Reduce Motion on and
      off: resting, hover, held (mic label for 1.5 s, then waveform),
      hands-free (✕ waveform ✓), Transcribing / Cleaning up, "No speech
      heard", "Microphone access is off" with Fix (opens Privacy ›
      Microphone).
- [ ] S-03.7 Right-click → Hide for an hour hides the resting bar (active
      states still show); the debug panel's "Show it again" brings it back.
- [ ] S-03.8 Drag along the edge; the position is kept per display and after
      a relaunch; dragging back to the centre forgets it.
- [ ] S-03.9 DEC-38: with setup not finished the resting bar stays hidden,
      takes still show the bar.
- [ ] S-03.10 VoiceOver: the resting bar reads "Dictation bar, button" with
      its hint; starting, transcribing, messages and cancel are announced.
- [ ] S-03.11 Two desktops (Mission Control): the bar is on both after
      switching; system sleep and wake; a real external display.

Procedure (automated where it can be; a person does S-03.6 by eye with
VoiceOver for S-03.10, and S-03.11):

1. Build the product app (`bun run wb:build -- --bundles app`; without
   F-02 keys the updater signing step fails after the `.app` is written,
   which is enough). Copy `Wispr Bro.app` to a scratch folder, add a file
   `Contents/MacOS/portable` containing `Handy Portable Mode` and a model in
   `Contents/MacOS/Data/models/` (e.g. `parakeet-tdt-0.6b-v3-Q8_0.gguf`), so
   the run never touches the owner's data. Launch once, quit, set
   `onboarding_completed: true` and `selected_model` in
   `Data/settings_store.json`. Start `…/Contents/MacOS/handy --start-hidden
--debug` from a terminal with Accessibility, Microphone and Screen
   Recording access. The log says `pill: Flow Bar started` and
   `pill: on screen native=Some(Rest)`.
2. Driver: `xcrun swiftc -O scripts/wb-s03-pill.swift -o /tmp/wbpill`;
   `WB_PID=<app pid> WB_FRONT=TextEdit /tmp/wbpill -e '<commands>'` (see
   its header). Open a TextEdit document of your own; run every GUI step
   under `/usr/bin/lockf -k ~/wb-bench/gui.lock`. Never write the open
   document's file from outside (TextEdit then shows a "changed by another
   application" sheet that takes the keyboard); read it through AX.
3. S-03.1 appear latency and hover: `rest; hover 20; appear 20 900` →
   `bubble_ms`, `active_ms` (fn-down → active bar on screen), `web_ms`
   (→ web view on screen). Log: `pill: on screen … (unix_us …)`.
4. S-03.1/S-03.2 first click: `rest; clickbar; wait 1900; front; clickat
151 17; watchfront 1500` ×20 (✓ is at x 151 of the 168 pt hands-free
   bar); every `front` must stay TextEdit and the log must show one
   `pill: Stop` per trial. With speech: speak (or `say`) between the two
   clicks; the insert log says `confirmed via CommandV into
com.apple.TextEdit`. Repeat in Slack and Chrome. DEC-28 check on a new
   macOS: launch with `WISPRBRO_PILL_ALLOW_ACTIVATION=1`; if `watchfront`
   still shows TextEdit after ✓, the private flag is no longer needed.
5. S-03.3: hands-free take with speech; `shot` during speech; the bars
   move (the web view's bars follow `ForkPillLevels`).
6. S-03.4: plug a 1× virtual display (`crates/wb-spike-pill/tools/vdisplay.m`,
   `vdisplay plug 1920 1080 1 30`), `follow <x> <y>` onto it and back
   (≤ 1,000 ms), kill the helper while the bar is on it (re-placed on the
   built-in display).
7. Capture: set `bar_hide_in_recordings: true` (debug panel › Dictation
   bar), `windows` shows `sharing: 0`, `shot` shows no bar at rest and in a
   take; turn it off again.
8. S-03.5: `procs` after each take and once a minute at rest (RSS of the app
   and its WebKit processes; `footprint -p <pid>` for physical footprint);
   100 takes spaced 20 s, then ≥ 10 min at rest.
9. S-03.7/S-03.8: `rclickbar`, then click the menu item; `drag 300`,
   `drag -300`; check `bar_hidden_until_ms` and `bar_x_by_display` in
   `Data/fork-settings.json`.

Machines: M1-8, M-15, M-26. Results of the first run (M5 Max, macOS
26.6.2) are in R5 under S-03 "As built".

### S-04 Insertion and clipboard

- [ ] S-04.1 RS-05 matrix on the real slice: the six RS-05 source types
      (plain text, RTF, HTML, a file, an image, two files) each before
      pastes in all ten apps; the clipboard is byte-identical afterwards
      (the soak below compares every take).
- [ ] S-04.2 100-take insertion run across the 10 apps of §5.4: 0 lost,
      0 duplicated, 0 old-clipboard pastes.
- [ ] S-04.3 Lid close mid-take: the text is not pasted; after wake it is on
      the card (S-05) and in History; the recording stopped at sleep.
- [ ] S-04.4 Fast user switch mid-take: same as S-04.3.
- [ ] S-04.5 Japanese IME active: the paste lands, plain and while a word
      is being composed (or the take is `unverified`).
- [ ] S-04.6 The log has one `insert: take N <paste_outcome>` line per take.

**Procedure (S-04.1, S-04.2, S-04.5): unattended, about 5 minutes.** On a
Mac with Accessibility granted to the terminal, nobody using it, and no
other test tool driving the screen:

```sh
cd src-tauri/crates/wb-spike-clipboard && cargo build --release
./target/release/wb-spike-clipboard --s04-soak            # 10 takes × 10 apps
./target/release/wb-spike-clipboard --s04-ime             # S-04.5
./target/release/wb-spike-clipboard --s04-leftovers       # closes test tabs left open
```

The soak compiles the product's `fork/clipboard` and `fork/insert`
(`deliver.rs`, `verify.rs`) and delivers each take exactly as the app does.
Apps (stand-ins for §5.4 where the original is not installed): TextEdit,
Notes (a new note), Mail (a new message, never sent), Messages (a new message
with no recipient), Terminal (a `cat` window), Safari and Chrome (a local test
page instead of Gmail), Cursor (for VS Code; a test file in its editor panel),
Claude (for Slack: an Electron chat box), ChatGPT (for Notion: a Chromium 156
app). It saves the clipboard first and puts it back at the end, refuses to
start if the pointer moved in the last 3 s, stops if it moves during the run,
brings the target back if another app takes the front, and only removes its
own text afterwards (drafts that were already in a chat box stay). Results:
`~/wb-bench/results/s04/<time>/summary.txt` and `summary.json`; one row per
app, and for each take the delivery, verification, read receipt, restore time
and byte comparison. `--per-app N` and `--only textedit,notes,…` run part of
it; an app interrupted by someone else is re-run on its own.

**Procedure (S-04.3, S-04.4): a person, M-26.** Open TextEdit with a new
document. Hold fn and start speaking, then (a) close the lid for 10 s and open
it again, (b) switch to another user from the menu bar and back. Release fn
after wake / switch-back. Expected: nothing is pasted into TextEdit; the log
has `insert: session Begin(Sleep)` (or `Begin(SessionInactive)`), `insert:
stopping the recording`, and `insert: take N card`; the text is in History.
The card itself is S-05. Unattended, `wb-spike-clipboard --s04-session` checks
the wiring: the product's observer receives NSWorkspace sleep/wake and session
notifications (posted in-process) and the engine then returns a card and does
not touch the clipboard.

### S-05 Accessibility layer, "no text field" card, secure fields

- [ ] S-05.1 RS-04 matrix on the real slice.
- [ ] S-05.2 0 false cards with a text field focused.
- [ ] S-05.3 Card appears on the desktop and in Finder.
- [ ] S-05.4 Start a take in Slack, release in Notes → text in Notes.
- [ ] S-05.5 Switch apps during processing → card.
- [ ] S-05.6 Password field → refused card with the text hidden; Copy puts
      the text on the clipboard with `org.nspasteboard.ConcealedType`.
- [ ] S-05.7 Card Copy replaces the clipboard (no restore) and the card says
      "Copied", then goes; Esc dismisses the card (observed, not blocked).
- [ ] S-05.8 With VoiceOver on, the card is announced ("Click into a text
      field first") and Copy is a named button (a person).

**Profile: an isolated portable debug build** (as S-09; the owner's app and
data are never touched). In the worktree:

```sh
ci/llama-server/build.sh --if-missing        # tauri build needs the sidecar
echo '{"identifier":"com.wisprbro.s05check","plugins":{"updater":{"active":false}}}' > /tmp/s05.conf.json
bun run tauri build --debug --no-bundle --config src-tauri/tauri.fork.conf.json --config /tmp/s05.conf.json
cd src-tauri/target/debug
printf "Handy Portable Mode\n" > portable
mkdir -p Data/models && cp ~/wb-bench/asr/parakeet-tdt-0.6b-v3/parakeet-tdt-0.6b-v3-Q8_0.gguf Data/models/s05-parakeet-v3.gguf
cd ../../.. && scripts/wb-s05/app.sh init      # creates Data/settings_store.json and fork.db
python3 scripts/wb-s05/setup.py "$PWD/src-tauri/target/debug/Data" s05-parakeet-v3
scripts/wb-s05/app.sh start   # a test identity runs as a product build (S-07a)
```

Helpers (compiled once): `for h in clip drive; do swiftc -O -o $BIN/$h
scripts/wb-s05/$h.swift; done; swiftc -O -o $BIN/pill scripts/wb-s03-pill.swift`.

**Procedure (S-05.2 to S-05.7, unattended, ~1 min per take, speakers and
mic):** `WB_S05_BIN=$BIN scripts/wb-s05/run.sh <out dir> [rounds]` (or `python3 -I scripts/wb-s05/run.py …` when executing scripts made on this Mac hangs: syspolicyd down). Before the stop each take checks again that its own window has focus and cancels the take otherwise (`handy --cancel`), so nothing goes to a window the run did not open. Keep speaker runs to one round. Each
take holds `~/wb-bench/gui.lock`, opens its own window, checks the
frontmost app before recording, records through Handy's toggle (SIGUSR2)
while `say`'s speech plays through the speakers, and closes only what it
opened; card cases save the whole clipboard first and put it back after
(byte-identical, checked). Cases: the fields (a new TextEdit document;
`fields.html` input, textarea, contenteditable and search in Safari; input,
textarea and contenteditable in a separate Chrome instance) must paste with
no card; our own Finder window and the desktop (an empty spot clicked) must
show the card (Finder: Copy clicked, clipboard = the History text, no
markers; desktop: ✕ clicked, the card goes; Esc needs the hotkey engine, so a person checks it in a product build); `fields.html#password` must be
refused with the text hidden and Copy concealed; `switch` starts in Chrome
and activates TextEdit before the stop (stand-ins for Slack and Notes, not
installed): the text must land in TextEdit only; `moved:40` stops in
TextEdit and activates Safari 40 ms later: the card "You moved on, so
nothing was pasted", neither field changed. `summarize.py` prints PASS/FAIL
per take from the files and the app log (`insert: take N target {…}`: route,
app, role, verdict, `same_as_key_up`, AX ms; never text).

**Procedure (S-05.1):** `src-tauri/crates/wb-spike-ax`: `cargo build
--release`, then `wb-spike-ax --auto` (TextEdit + the RS-04 test page in
Safari and Chrome; the spike now compiles the product's `fork/ax` verdict)
and `wb-spike-ax --watch 3` while a window of each installed app has a
field, a button and nothing focused. Finder: `scripts/wb-s05/finder-probe.sh
<out>` under the GUI lock.

**With a person (S-05.8 and the real apps):** VoiceOver on; dictate with
the desktop focused, in Slack and Notes (start in one, release in the
other), into a password field in Safari, and switch apps while
"Transcribing…" shows.

### S-06 Recovery shortcuts

- [ ] S-06.1 RS-05 held-modifier case: ⌃⌘V with ⌃⌘ still held pastes the
      plain text once the keys come up (no ⌃⌘V reaches the app); held 3 s
      or more → the card ("Couldn't paste this text"), nothing pasted.
- [ ] S-06.2 ⌃⌘V into Slack, Chrome, Terminal: the last dictation, exactly,
      and the clipboard byte-identical afterwards.
- [ ] S-06.3 ⌃⌘C: the clipboard holds the last dictation (plain, not
      transient), and stays (no restore).
- [ ] S-06.4 Mouse-button Press Enter per RS-11: the back button types one
      Return in the focused field; the settings say a mouse button still
      clicks in the app under the pointer.
- [ ] S-06.5 Cancel → Undo in Mail: Esc, "Cancelled · Undo" for 4 s,
      Undo → the text in the field and in History.
- [ ] S-06.6 Undo after focus moved → the card (`focus_moved`), nothing
      pasted anywhere.
- [ ] S-06.7 Nobody undoes: after 4 s the offer goes, Undo does nothing, no
      History row; a tap with no second press or a fn chord never offers
      Undo.

**Profile: an isolated portable debug build** (as S-05; never the owner's
app, data or clipboard). In the worktree:

```sh
ci/llama-server/build.sh --if-missing        # tauri build needs the sidecar
echo '{"identifier":"com.wisprbro.s06check","plugins":{"updater":{"active":false}}}' > $SCRATCH/s06.conf.json
bun run tauri build --debug --no-bundle --config src-tauri/tauri.fork.conf.json --config $SCRATCH/s06.conf.json
cd src-tauri/target/debug
printf "Handy Portable Mode\n" > portable
mkdir -p Data/models && cp ~/wb-bench/asr/parakeet-tdt-0.6b-v3/parakeet-tdt-0.6b-v3-Q8_0.gguf Data/models/s06-parakeet-v3.gguf
cd ../../.. && scripts/wb-s05/app.sh init
python3 -I scripts/wb-s06/setup.py "$PWD/src-tauri/target/debug/Data" s06-parakeet-v3
scripts/wb-s05/app.sh start
```

`setup.py` turns cleanup off, binds Press Enter to the back mouse button
and Undo to ⌃⌥Z, keeps recordings, and seeds one History dictation with a
fixed text (no personal data) for paste-last and copy-last.

Helpers (compiled once): `swiftc -O -o $BIN/post scripts/wb-s06/post.swift;
swiftc -O -o $BIN/clip scripts/wb-s05/clip.swift; swiftc -O -o $BIN/pill
scripts/wb-s03-pill.swift`. The Undo cases need a WAV of a short sentence
(`say -o $SCRATCH/undo.aiff "<sentence>" && afconvert -f WAVE -d LEI16@16000
$SCRATCH/undo.aiff $SCRATCH/undo.wav`) and `WB_S06_SAID="<sentence>"`.

**Procedure (unattended):** `WB_S06_BIN=$BIN scripts/wb-s06/case.sh <case>
<out dir> [wav]` for `paste:textedit`, `paste:held:1200`,
`paste:held:3500`, `paste:chrome`, `paste:terminal`, `copy:textedit`,
`enter:mouse` (no audio), then `undo:bar`, `undo:moved`, `undo:expire` with
the WAV (through the speakers into the mic: run each once, at a moderate
volume). Every case holds `~/wb-bench/gui.lock`, opens its own window
(TextEdit document, a separate Chrome instance on `fields.html`, a Terminal
window running `cat` into a file), posts each synthetic event only while
that window's app is frontmost (`post` refuses otherwise), saves the whole
clipboard first and checks it is byte-identical after, and closes what it
opened. `summarize.py` prints PASS/FAIL per case from the field's text, the
clipboard and the app log (`recovery: …`, `insert: take N (Again|Undo …)`;
never text).

**With a person:** ⌃⌘V in Slack and Mail, a real mouse's side button for
Press Enter (and a middle click in a browser: it also opens the link), Undo
in Mail's compose window, VoiceOver on the offer ("Undo is on the dictation
bar for 4 seconds"), Esc on the offer.

### S-07a Starter model, mic gate, "Setting up" first run

- [ ] S-07a.1 RS-09 first-run part: clean installs at 10 and 50 Mbit/s meet §5.3.
- [ ] S-07a.2 A press during the download records and shows the card when ready.
- [ ] S-07a.3 Captive-portal Wi-Fi simulation.
- [ ] S-07a.4 A 429 with `Retry-After`, a reset mid-transfer, a quit and
      relaunch mid-download (resume), and a disk too full for the model.
- [ ] S-07a.5 (person) A real captive portal (hotel or café Wi-Fi), and the
      onboarding by eye: no Model screen, no model name in the bar.

**Profile: an isolated portable debug build with a test identity**
(`com.wisprbro.s07acheck`; `fork::is_test_identity` makes it a product
build: fork hotkeys, the bar and product settings run, with its own app
data, WebKit storage and TCC rows). In the worktree:

```sh
echo '{"identifier":"com.wisprbro.s07acheck","productName":"Wispr Bro S07a Check","plugins":{"updater":{"active":false}}}' > <scratch>/s07a.conf.json
bun run tauri build --debug --no-bundle --config src-tauri/tauri.fork.conf.json --config <scratch>/s07a.conf.json
mkdir -p <scratch>/app && cp src-tauri/target/debug/handy <scratch>/app/
printf "Handy Portable Mode\n" > <scratch>/app/portable   # data in <scratch>/app/Data
```

No model is copied in: the first launch must find none. Each run starts
from an empty `Data/`. The model files come from a local Hugging Face
stand-in, `scripts/wb-s07a/hf-mirror.py` (serves `~/wb-bench/asr` with the
headers hf-hub reads, one throttled link, runtime modes `normal`,
`captive`, `429`, `drop`), and the debug build follows a loopback
`HF_ENDPOINT` for both Handy's downloader and the fork's probe (release
builds always use `huggingface.co`). One run also downloads from the real
Hugging Face (no `HF_ENDPOINT`). Takes go through Handy's record path with
SIGUSR2 and `say` through the speakers (`scripts/wb-s07a/take.sh`), so no
key events are posted; clicks on the bar's card use `scripts/wb-s03-pill.swift`
with `WB_FRONT` set to a TextEdit window the run opened, under
`~/wb-bench/gui.lock`. Save the clipboard first and put it back at the end.

**Procedure (unattended, ~20 min).**

1. S-07a.1: `python3 -I scripts/wb-s07a/hf-mirror.py --root ~/wb-bench/asr
--rate 6250000` (50 Mbit/s), then
   `HF_ENDPOINT=http://127.0.0.1:18077 ./handy --debug` with an empty
   `Data/`. Pass: the log has `speech setup at launch: Missing`, `downloading
the first speech model parakeet-tdt_ctc-110m`, `checksum … ok`,
   `selected …`, and launch → ready ≤ 25 s. Again at `--rate 1250000`
   (10 Mbit/s): ≤ 90 s.
2. S-07a.2: at 10 Mbit/s, start a take 3 s after launch, play a sentence,
   stop it. Pass: the bar shows "Setting up… N%" (screenshot), the log has
   `take queued`, the clipboard's change count is unchanged; when the model
   is ready the card shows the sentence; the clipboard is still unchanged;
   a click on Copy puts exactly the card text on it; History has the row
   with its text and `paste_outcome = queued`.
3. S-07a.3: mirror mode `captive` before launch. Pass: `probe after the
failure: CaptivePortal`, a take shows "Couldn't download. Check your
   connection." with Retry; mode `normal`, Retry → the download finishes and
   the card shows the take.
4. S-07a.4: mode `429` with Retry-After 20 → the log waits 20 s; mode `drop`
   mid-transfer → backoff and a later transfer finishes; quit at ~50 % and
   relaunch → the progress continues from about the same percent (hf-hub's
   `.sync.part`); a checksum mismatch (serve a truncated or altered copy) →
   deleted, downloaded once more, then "Couldn't download".
5. Real Hugging Face: empty `Data/`, no `HF_ENDPOINT`: the starter
   downloads from `huggingface.co` and its checksum matches.

Scripted: `scripts/wb-s07a/run.sh` (one launch, optional take, screenshots
of the bar only), `scripts/wb-s07a/copy.sh` (Copy on the card, clipboard
saved and restored) and `scripts/wb-s07a/scenarios.sh <app dir> <seed
settings> <out> t50 t10 429 drop resume corrupt realhf` (no audio). The
Python stand-in tops out near 18 Mbit/s on M-26, so time the 50 Mbit/s case
on a real link (or Network Link Conditioner by a person). Use the speakers
only for the take that needs the mic (S-07a.2).

### S-07b Silent model policy, upgrades and cleanup

- [ ] S-07b.1 English, empty profile: the starter (parakeet-110m Q4_K_M,
      90 MB) is ready, then the policy's pick downloads in the background
      with nothing on the bar, is swapped in at a quiet moment after it
      loaded, and the starter file is deleted.
- [ ] S-07b.2 A take during the background download is transcribed by the
      starter; the swap waits until no take has ended for 10 s.
- [ ] S-07b.3 Language change to ja + en re-picks (RS-09 policy part on the
      real slice); the superseded model is deleted after the new one loaded.
- [ ] S-07b.4 Settings › Advanced › Speech models (debug panel) names the
      model, shows the background phase; picking a model in "All models"
      turns "Choose the model automatically" off and the policy leaves it.
- [ ] S-07b.5 No model name outside Advanced: bar, tray (no Model ▸, no
      Unload), onboarding All set panel (no Model row), notifications ("Dictation
      is ready" only for the first model).
- [ ] S-07b.6 §5.3 at 10 Mbit/s in English with the Q4_K_M starter (≤ 90 s).
- [ ] S-07b.7 (Intel, DEC-29 later milestone) Intel picks a CPU model.

**Profile:** as S-07a with the identity `com.wisprbro.s07bcheck`, the
stand-in `scripts/wb-s07a/hf-mirror.py --root ~/wb-bench/asr` (it needs
`parakeet-tdt_ctc-110m-Q4_K_M.gguf`, `parakeet-unified-en-0.6b-Q8_0.gguf`
and `whisper-large-v3-turbo-Q8_0.gguf` under that root) and a seed
`settings_store.json` with onboarding done.

**Procedure (unattended, ~25 min).**

1. S-07b.1: `scripts/wb-s07b/run.sh <app> <seed> <out> fresh en`. Pass: the
   log has `downloading the first speech model parakeet-tdt_ctc-110m`,
   `selected …Q4_K_M.gguf`, `policy for ["en"] … parakeet-unified-en-0.6b
… → Download`, `downloading a better speech model`, `checksum … ok`,
   `swapped … → …unified…`, `deleted superseded model …110m…Q4_K_M.gguf`;
   the cache holds only the unified file; `speech_models` has the starter
   `superseded`+`deleted`, the unified row `loaded`. The bar never showed
   "Setting up" after the starter was ready (`pill:` lines).
2. S-07b.2: during step 1's background download play one `say` sentence
   through `scripts/wb-s07a/take.sh <pid> <aiff>`. Pass: it is pasted /
   in History with text from the starter; `swapped` is ≥ 10 s after the
   take ended.
3. S-07b.3: quit; `scripts/wb-s07b/run.sh <app> <seed> <out> relaunch ja,en`.
   Pass: `policy for ["ja", "en"] … whisper-large-v3-turbo … → Download`,
   `swapped`, the unified file deleted. (A live change in Settings ›
   General goes through `change_spoken_languages_setting`, which the fork
   observes; the relaunch is the unattended stand-in.)
4. S-07b.4 / .5: open the debug panel (⌘⇧D) in the test build, screenshot
   the Speech models section; screenshot the tray menu; read the inbox.
5. S-07b.6: mirror `--rate 1250000`, `fresh en`, time launch → `selected`.

Last run: `docs/fork/test-runs/1.0.0-alpha.1-M-26-S-07a.md`.

### S-07b Silent model policy

- [ ] S-07b.1 RS-09 policy part on the real slice: language change to ja+en re-picks.
- (R5 also lists "Intel picks CPU"; skipped under DEC-29.)
- Procedure: TODO (S-07b).

### S-07c Offline queue and late delivery

- [ ] S-07c.1 Offline first launch with networking off, 3 takes, quit, relaunch online.
- [ ] S-07c.2 The notification action copies the latest (the button itself is
      verified by S-18.1 with a sample; S-07c adds the real producer).
- Procedure: TODO (S-07c).

### S-08 Microphone and sound polish

- [ ] S-08.1 RS-11 mic-loss and permission-revoke cases: the take's mic
      disappears mid-take → the bar says "Mic lost, keeping what you said"
      and the text before the loss reaches History; Microphone revoked
      mid-take, then **Quit & Reopen** → the take's audio is saved at quit
      and is in History after the relaunch (inbox entry); the next take
      without access shows "Microphone access is off" with Fix.
- [ ] S-08.1b (person, hardware) The same with a real USB mic unplugged and
      AirPods dropping out mid-take.
- [ ] S-08.2 AirPods connected as the macOS default, never chosen → takes
      use the built-in mic (log `the system microphone is wireless …`, the
      bar's label), the list shows the "skipped headset" note; "Use … anyway"
      switches to the headset.
- [ ] S-08.3 "Mute all audio while dictating": muted during the take,
      afterwards the output's mute flag and volume scalars are back bit for
      bit; a user who had muted stays muted; no `osascript` process runs.
- [ ] S-08.4 Settings › Sound on the debug panel: list order (built-in
      "(recommended)", Auto-detect, wired, Bluetooth with its caution), the
      level meter moves with speech, sounds on by default.

**Profile: an isolated portable release bundle** with its own identity
(`com.wisprbro.s08check`, "Wispr Bro S08 Check"), launched with `open` so
macOS keeps its own Microphone permission (a binary started from a
terminal shares the terminal's, and revoking it would revoke the
terminal's). Never the owner's app or data. In the worktree:

```sh
echo '{"identifier":"com.wisprbro.s08check","productName":"Wispr Bro S08 Check","plugins":{"updater":{"active":false}}}' > $SCRATCH/s08.conf.json
bun run tauri build --bundles app --config src-tauri/tauri.fork.conf.json --config $SCRATCH/s08.conf.json
swiftc -O -o $SCRATCH/wbhal scripts/wb-s08/hal.swift     # the terminal needs Accessibility
```

Copy the `.app` to `~/Applications/`, add `Contents/MacOS/portable`
(`Handy Portable Mode`), put a speech model in `Contents/MacOS/Data/models/`,
re-sign (`codesign --force --deep --options runtime --entitlements
src-tauri/Entitlements.plist -s -`), `lsregister -f`, launch once and quit,
then `python3 -I scripts/wb-s08/setup.py <app>/Contents/MacOS/Data <model> [1]`
(never chosen, sounds on, no paste, optional mute). Takes are toggled with
`kill -USR2`. Keep speaker playback to the one check that needs real mic
audio (the mic loss: one short WAV made once with `say -o`, played at a
moderate volume); everything else runs silent. Hold `~/wb-bench/gui.lock`
for every run that plays sound or changes the default input; put the
default input and the output volume back. `scripts/wb-s08/run.sh` has a
case per step (`HAL`, `APP`, `OUT` set; `grant.sh` answers the first
Microphone prompt with Allow).

**Procedure (unattended, ~10 min).**

1. Mic loss (`run.sh loss`): a public aggregate over the built-in mic (the
   stand-in USB mic) as the default input, relaunch so it is prepared, a
   take with the WAV, destroy the aggregate right after it, stop the take.
   Pass: log `the take's microphone disappeared` at once, the History row
   holds the words before the loss, the watch re-prepares on the built-in
   mic and the next take records.
2. Mute (`run.sh relaunch 1`, `mute`, `premuted`): `wbhal state` before,
   during and after a take. Pass: `mute` 1 during, the same `mute` and
   volume `bits` after; with the output muted beforehand it stays muted; no
   `osascript` process.
3. Revoke + quit (`grant.sh`, `run.sh revoke`): `tccutil reset Microphone
com.wisprbro.s08check` 3 s into a take. If macOS shows "Quit & Reopen",
   press it; otherwise (2026-10-31: no dialog appeared) send the quit Apple
   event and reopen. Pass: log `saved the take running at quit`, after the
   relaunch `take(s) saved at the last quit are in History`, a History row
   with `source_name` `wispr-bro-recovered-….wav`, inbox `take_recovered`,
   `Data/recovered/` empty.
4. Denied (`run.sh denied`): answer the prompt with Don't Allow. In the same
   process the bar says "Only silence came in" (the open succeeds, silent);
   after a relaunch it shows "Microphone access is off" with Fix.
5. Clean up: quit, `tccutil reset All com.wisprbro.s08check`,
   `lsregister -u`, delete the bundle and its `~/Library/{WebKit,Caches}`
   folders, compare `wbhal state` with the one before the run.

Not reproducible without hardware: S-08.1b and S-08.2 (a Bluetooth
transport cannot be faked; the rule is covered by `fork::audio::choice`
tests).

Last run: `docs/fork/test-runs/1.0.0-alpha.1-M-26-S-08.md`.

### S-09 History data model, retention, stats

- [ ] S-09.1 50 takes in 5 apps produce correct per-app rows: each row has
      the app that was frontmost at record start, its `usage_category`, its
      words and speech time, a `paste_outcome`; `daily_stats` and
      `daily_app_stats` add up to the rows.
- [ ] S-09.2 14-day audio expiry simulated by clock override: WAVs older
      than 14 days are deleted, their rows and text stay, the stats do not
      change.
- [ ] S-09.3 Delete a row from History, undo within 10 s: it comes back;
      delete again and wait: it is gone from Handy's `history.db` and
      `fork.db`, the stats keep it. (Needs S-15b's UI; until then
      `fork_history_delete` / `fork_history_undo_delete` from the debug
      console.)
- [ ] S-09.4 Fill the disk to under 50 MB free during a take: the
      recording stops, the text goes to the card (S-05) and History; the
      log has `disk: … stopping the recording`.

**Profile (all S-09 runs): an isolated portable debug build**, so the
owner's app and data are never touched. In the worktree:

```sh
echo '{"identifier":"com.wisprbro.s09check","plugins":{"updater":{"active":false}}}' > /tmp/s09.conf.json
bun run tauri build --debug --no-bundle --config src-tauri/tauri.fork.conf.json --config /tmp/s09.conf.json
cd src-tauri/target/debug
printf "Handy Portable Mode\n" > portable          # data goes to ./Data
mkdir -p Data/models && cp ~/wb-bench/asr/whisper-base/whisper-base-Q8_0.gguf Data/models/s09-whisper-base.gguf
./handy --start-hidden & sleep 15; kill %1        # creates Data/settings_store.json
```

The test identifier (its own single-instance socket) means the product
settings are not applied, so set in `Data/settings_store.json` >
`settings`: `selected_model: "s09-whisper-base"`,
`recording_retention_period: "never"` (Handy's own retention off, as the
product enforces), `paste_method: "none"` (nothing is typed into the five
apps), `vad_enabled: false` (speaker-to-mic audio is too quiet for VAD),
`selected_language: "en"`, `log_level: "debug"`.

**Procedure S-09.1 (unattended, ~13 min, needs speakers and the mic):**
`./handy --start-hidden &`, then
`scripts/wb-s09-soak.sh ~/wb-bench/audio/segments 50`. Each take holds
`~/wb-bench/gui.lock`, brings the app to the front and checks it is
frontmost before sending SIGUSR2 (Handy's toggle); a skipped take is
retried. Then:

```sh
sqlite3 Data/fork.db "SELECT app_bundle_id, usage_category, COUNT(*), SUM(word_count), SUM(counted)
                      FROM entry_meta GROUP BY 1, 2"
sqlite3 Data/fork.db "SELECT paste_outcome, COUNT(*) FROM entry_meta GROUP BY 1"
sqlite3 Data/fork.db "SELECT SUM(words), SUM(dictations), SUM(speech_ms) FROM daily_stats"
sqlite3 Data/fork.db "SELECT usage_category, words, dictations FROM daily_app_stats"
```

Pass: 10 rows per app with the right category (TextEdit and Notes
`documents`, Mail `email`, Messages `personal`, Claude `ai_prompts`);
every row has a `paste_outcome` (`unverified` with paste method none);
`daily_stats` words = the sum of counted rows' `word_count`, dictations =
counted rows, `speech_ms` = their `audio_ms`; `daily_app_stats` matches per
category; `history.db` has the same 50 ids.

**Procedure S-09.2 (unattended, 1 min):** quit the app, relaunch it with
`WB_DEBUG_RETENTION_CLOCK_OFFSET_DAYS=15 ./handy --start-hidden` (debug
builds only). Pass: the log has `retention: 50 audio file(s) and 0 row(s)
deleted`, `Data/recordings/` is empty, every `entry_meta` row has
`audio_deleted_at`, `history.db` still has 50 rows, and the `daily_stats`
query gives the same numbers as before. With offset 13 nothing is
deleted.

Last run: `docs/fork/test-runs/1.0.0-alpha.1-M-26-S-09.md` (S-09.1, S-09.2
pass; S-09.3 purge path only; S-09.4 not run).

### S-10 Local LLM runtime

- [ ] S-10.1 RS-06 on the real slice: signed binary under Gatekeeper on
      macOS 13 and 26: `spctl --assess --type execute -vv` on
      `Contents/MacOS/llama-server` says `accepted` (source Notarized
      Developer ID), and the app launches from a quarantined download.
- [ ] S-10.2 RSS and latency per tier, incl. the cold-cold row: the RS-06
      bench (below) on M1-8 and M-26; Light p95 ≤ 2.2 s end to end on M1-8,
      combined peak (ASR + LLM) ≤ 3.2 GB, cold-cold extra ≤ +2.5 s p95.
- [ ] S-10.3 Esc abort ≤ 100 ms: bench `abort.slot_free_extra_ms_p95` ≤ 100.
- [ ] S-10.4 Kill the sidecar mid-take → the take fails open and the
      sidecar restarts (`real_server_killed_mid_request_fails_open_and_restarts`,
      then in the app: `pkill -9 llama-server` while a Try-it runs; the panel
      shows the error, the next Try-it works).
- [ ] S-10.5 Sleep mid-take → recovered on wake: close the lid (or Apple
      menu › Sleep) for 1 min while the engine runs; after wake the log shows
      `llm: server healthy after wake` (or a restart) and Try it works.
- [ ] S-10.6 First run: on a fresh profile with a speech model, the cleanup
      model downloads silently (Settings › Advanced › AI engine, on the fork
      debug panel until S-14, shows the progress), then **Ready**; Try it
      cleans the sample sentence.
- [ ] S-10.7 No network from the sidecar: `lsof -a -i -p $(pgrep llama-server)`
      prints nothing while it runs; `nettop -P -L 1 -p $(pgrep llama-server)`
      shows 0 bytes; the `wb-egress` run passes.
- [ ] S-10.8 Idle: 120 s after the last request `pgrep llama-server` finds
      nothing; quitting the app (⌘Q, and `kill -9` of the app) leaves no
      `llama-server` behind.

**Procedure (S-10.2-S-10.4): unattended, about 3 minutes.** Build the sidecar
and run the RS-06 bench against the real binary and model (any reference
Mac; no window is opened):

```sh
ci/llama-server/build.sh                     # static, signed ad hoc; prints the path
cd src-tauri
export WB_LLAMA_SERVER=$PWD/target/wb-sidecar/llama-server-aarch64-apple-darwin
export WB_LLM_MODEL=<HF cache>/qwen35-2b-muesli-v12-Q4_K_M.gguf
export WB_ASR_MODEL=<a speech GGUF, e.g. parakeet-tdt-0.6b-v3-Q8_0.gguf>
export WB_ASR_WAV=<a 16 kHz mono WAV of scripted speech>
export WB_RS06_OUT=../docs/fork/test-runs/<version>-<machine-id>/rs06.json
cargo test --lib fork::llm -- --ignored --nocapture --test-threads 1 rs06 real_
```

Record `cold_start_to_health`, `light_warm`, `contention`, `abort` and
`wake` from the JSON. The model file is in the app's HF cache after S-10.6
(`~/Library/Application Support/com.wisprbro.desktop/huggingface/hub/models--Muesli-HQ--qwen35-2b-muesli-cleanup-v12-gguf/snapshots/e35af097…/`).

**Procedure (S-10.6, S-10.8): in the app.** Fresh profile, then launch with
the AI-engine self-check:

```sh
WISPRBRO_SELFCHECK=1 WISPRBRO_SELFCHECK_LLM=1 "/Applications/Wispr Bro.app/Contents/MacOS/wispr-bro"
```

The log (`~/Library/Logs/com.wisprbro.desktop/wispr-bro.log`) must show
`fork selfcheck: PASS … fork_llm_status state=…`, `llm: muesli-2b-v12-q4km
ready`, then `fork llm selfcheck: PASS cold: …` and `PASS warm: …`. Machines:
M-13, M1-8, M-26.

### S-11 Text pipeline v2

- [ ] S-11.1 Dictate a three-paragraph e-mail into Mail: say "Hi Anna,
      new paragraph, thanks for the notes … I'll send the draft Monday,
      scratch that, I'll send it Tuesday. New paragraph. Best, new line,
      Finn" (pausing briefly around each command). The field shows three
      paragraphs, the last one on two lines, no command words, no Monday
      clause; a from→to rule in the dictionary (S-16, or `fork.db` until
      then) is applied.
- [ ] S-11.2 Dictate a two-line Slack message ("Running late. New line.
      Start without me."): both lines are in the message box and nothing
      was sent (Slack is a Chromium app: in typing mode the line break is
      Shift+Return, S-04).
- [ ] S-11.3 Ordinary words stay words: "We launched a new line of
      shoes" is unchanged; "literally new line" types the words "new line".
- [ ] S-11.4 Per take, `fork.db` has `entry_meta.text_stages` (stage order
      and counts), `fix_count_dictionary` = the rules that applied, and one
      `corrections` row (source `rule`) per hit.

**Profile: an isolated portable debug build** (as S-09; the owner's app and
data are never touched). In the worktree:

```sh
ci/llama-server/build.sh --if-missing        # tauri build needs the sidecar
echo '{"identifier":"com.wisprbro.s11check","plugins":{"updater":{"active":false}}}' > /tmp/s11.conf.json
bun run tauri build --debug --no-bundle --config src-tauri/tauri.fork.conf.json --config /tmp/s11.conf.json
cd src-tauri/target/debug
printf "Handy Portable Mode\n" > portable
mkdir -p Data/models && cp ~/wb-bench/asr/parakeet-tdt-0.6b-v3/parakeet-tdt-0.6b-v3-Q8_0.gguf Data/models/s11-parakeet-v3.gguf
./handy --start-hidden & sleep 15; kill %1     # creates Data/settings_store.json and fork.db
python3 ../../../scripts/wb-s11/setup.py "$PWD/Data" s11-parakeet-v3   # settings + 4 rules
WB_DEBUG_TEXT_TRACE=1 ./handy --start-hidden &  # debug builds log each take's texts
```

**Procedure (unattended, ~2 min per round, needs the speakers and the
mic):** `scripts/wb-s11/run.sh 3`. Each round dictates six takes with
macOS TTS (`say`): the e-mail into a new TextEdit document and into a
WebKit contenteditable page in a new Safari window (`compose.html`, the
stand-in for Mail's editor), the chat message into TextEdit and into
`chat.html` in a separate Chrome instance with its own profile (Enter
sends, Shift+Enter is a new line: the stand-in for Slack; Chrome 155/156
is S-04's typing path), and S-11.3's two sentences. Every take holds
`~/wb-bench/gui.lock`, checks the frontmost app before recording and
closes only the window it opened; `check.py` prints PASS/FAIL per take.
Then:

```sh
sqlite3 Data/fork.db "SELECT entry_id, app_bundle_id, fix_count_dictionary, paste_outcome, text_stages FROM entry_meta"
sqlite3 Data/fork.db "SELECT entry_id, from_text, to_text FROM corrections WHERE source = 'rule'"
grep -E "text: rules|text trace|insert: take [0-9]+ [a-z]+ via" Data/logs/wispr-bro.log
```

Pass: every take PASS; the Chrome takes are `via Typing` and the title
says `sent=0`; every row has `text_stages` and a `paste_outcome`; the
e-mail rows have `fix_count_dictionary` 2 and two `corrections` rows.
With a person (and Mail with a throwaway account, Slack with a test
workspace) say the same sentences.

Last run: `docs/fork/test-runs/1.0.0-alpha.1-M-26-S-11.md`.

### S-12a Cleanup levels on the local LLM

- [ ] S-12a.1 RS-07 thresholds on the real build: the eval through the
      shipped path (`fork::llm::eval_run` + `wb-eval-cleanup.ts --replay`,
      `eval/cleanup/README.md` "The shipped path"), all Light gates and the
      Medium gates pass, per language too; real takes in en, nl and fr come
      out cleaned (fillers and lead-ins gone, repairs applied, questions not
      answered, dictionary spellings kept).
- [ ] S-12a.2 §5.1 Light latency on M1 8 GB and 16 GB (key-up → text).
- [ ] S-12a.3 Cleanup works offline: the sidecar listens only on loopback
      (`lsof -nP -a -c llama-serv -i`) and nothing else is contacted.
- [ ] S-12a.4 Forced timeout and level None: the take goes in as spoken
      (lead-ins removed on timeout), `cleanup_outcome` is `timeout` /
      `skipped`; in a product build the bar says "Inserted without
      cleanup" and Handy's filler removal is off at None.

**Profile: an isolated portable debug build** (as S-11; never the owner's
app or data). In the worktree:

```sh
ci/llama-server/build.sh --if-missing
echo '{"identifier":"com.wisprbro.s12acheck","plugins":{"updater":{"active":false}}}' > /tmp/s12a.conf.json
bun run tauri build --debug --no-bundle --config src-tauri/tauri.fork.conf.json --config /tmp/s12a.conf.json
cd src-tauri/target/debug
printf "Handy Portable Mode\n" > portable
# The catalog's own Parakeet file, dropped in (a custom-named model does not
# count as a speech model for the cleanup download):
mkdir -p Data/models && cp ~/wb-bench/asr/parakeet-tdt-0.6b-v3/parakeet-tdt-0.6b-v3-Q8_0.gguf Data/models/
./handy --start-hidden & sleep 20; kill %1
/usr/bin/python3 ../../../scripts/wb-s12a/setup.py "$PWD/Data" parakeet-tdt-0.6b-v3 ~/wb-bench/llm/qwen35-2b-muesli-v12-Q4_K_M.gguf light
WB_DEBUG_TEXT_TRACE=1 ./handy --start-hidden &
```

`setup.py` sets Parakeet, VAD off, spoken languages en/nl/fr, puts the
pinned Muesli GGUF in the profile's HF cache (its sha256 is checked, nothing
is downloaded), sets the level and two dictionary replacements. Stop the
app by its main process only (the other `handy --start-hidden` is S-10's
watchdog, which stops the sidecar).

**Procedure (unattended, needs the speakers and the mic):**
`scripts/wb-s12a/run.sh light 2` (seven takes per round: en fillers +
repair, an en question, an en dictionary spelling, nl lead-ins, an nl
repair, fr lead-ins, an fr repair), then restart the app with
`WB_DEBUG_CLEANUP_DEADLINE_MS=1` and run `run.sh timeout`, then
`setup.py … none`, restart, `run.sh none`. Each take opens and closes its
own TextEdit document under `~/wb-bench/gui.lock` and prints the key-up →
text time. Check the log's `cleanup:` lines and `fork.db`
(`entry_meta.cleanup_*`, `fix_count_llm`, `daily_stats.fixes_llm`).
Machines: M1-8, M-15, M-26. First run: `docs/fork/test-runs/1.0.0-alpha.1-M-26-S-12a.md`.

### S-12b Apple Foundation Models path

- [ ] S-12b.1 RS-08 on the real build: the eval through the shipped path
      on Apple's model (`scripts/wb-s12b/rs08.sh`; quality without the
      latency budget, the app's deadlines with the key-down prewarm, default
      vs permissive guardrails, guided vs plain), the 50-word cold /
      prewarmed / warm latency and a burst, and the app's own self-check in
      the background (`WISPRBRO_SELFCHECK_APPLE=1`: engine, outcome,
      `rateLimited`). The RS-08 decision (`fork::llm::apple::ROLE`) matches
      the numbers.
- [ ] S-12b.2 With the local model not ready (`WB_DEBUG_LLM_HOLD_MODEL=1`,
      debug builds) a real dictation goes to Apple's model (log
      `cleanup: … (apple, …)`; engine `apple` in `entry_meta`); the AI
      engine panel shows the Apple Intelligence row and Try it says "Ran on
      Apple Intelligence".
- [ ] S-12b.3 With the local model ready nothing changes: every take is
      `local` (self-check with the model in place).
- [ ] S-12b.4 Apple Intelligence off, or a Mac without it (M1-8 on macOS
      15, M-26 with it turned off by its owner): the row says why, takes use
      the local model, nothing else changes. Never turn the setting off on
      someone else's Mac for this.

**Profile:** the S-12a portable debug build with its own identifier
`com.wisprbro.s12bcheck` (`/tmp/s12b.conf.json` as in S-12a), Parakeet in
`Data/models`, `scripts/wb-s12a/setup.py … light` (Muesli in the profile's
HF cache).

**Procedure (M-26, unattended, ~40 min + 10 min):**

1. `scripts/wb-s12b/rs08.sh all` (no app running; Apple's model only;
   results in `eval/cleanup/reports/rs08/`, local).
2. `WB_DEBUG_LLM_HOLD_MODEL=1 WISPRBRO_SELFCHECK_APPLE=1 ./handy --start-hidden`
   and leave another app in front: after ~4 min the log has
   `fork apple selfcheck: PASS engines {"apple": 38} …` with the latencies and
   `rate_limited`.
3. Restart with `WB_DEBUG_LLM_HOLD_MODEL=1` only and run
   `scripts/wb-s12b/takes.sh <out dir>` (three takes through the speakers at
   volume 40, restored after; the clipboard is saved and restored by
   `scripts/wb-s12b/clip.swift`). Open the debug panel (⌘⇧D) › AI engine:
   the Apple Intelligence row, Try it → "Ran on Apple Intelligence".
4. Restart with `WISPRBRO_SELFCHECK_APPLE=local` only (the model verified
   from the HF cache): `fork apple selfcheck: PASS engines {"local": 38}`.

Machines: M-26 (and M-15/M1-8 for S-12b.4). First run:
`docs/fork/test-runs/1.0.0-alpha.1-M-26-S-12b.md`.

### S-13 Hub shell

- [ ] S-13.1 The overlay title bar drags the window (C-25 drag permission check).
- [ ] S-13.2 VoiceOver reads the sidebar.
- [ ] S-13.3 Window chrome: overlay title bar with no title text; the collapse button sits level with the traffic lights; default size and minimum size as below.
- [ ] S-13.4 Light and dark: the header toggle switches the whole window (and the recording overlay) and the choice survives a relaunch.
- [ ] S-13.5 A file dropped outside History is ignored; an audio file dropped on Settings › History is transcribed (RE-33).

Procedure (any Mac; a dev build is enough: `bun run wb:dev`, or the
`wb:build` app). Use a profile that has finished onboarding.

1. **Chrome and size (S-13.3).** Launch. The window opens centred with a
   1040 × 720 page area (smaller screens: the work area less 48 pt) and the
   sidebar running under the traffic lights; no title text is drawn. The
   collapse button (panel icon) sits just right of the green button, on the
   same line. Drag the window corner inwards: it stops at a 720 × 600 page.
   Click the collapse button: icons only; click again: labels back. Relaunch:
   the collapsed/expanded choice is kept.
2. **Drag (S-13.1).** Drag the window by the empty strip at the top of the
   sidebar and by the empty part of the header (left of the moon icon): the
   window moves. Double-click the strip: the window zooms (and back). Check
   `~/Library/Logs/com.wisprbro.desktop/wispr-bro.log` has no `start_dragging`
   or `set_title_bar_style` "not allowed" error.
3. **Onboarding keeps a normal title bar.** Settings › Debug (turn on debug
   mode with ⌘⇧D) › Run onboarding: the standard title bar returns while the
   onboarding screens show, and the overlay comes back with the Hub.
4. **Theme (S-13.4).** Click the moon icon: the whole window turns dark
   (sidebar, page, footer, Handy's settings tabs) and the icon becomes a sun.
   Start a dictation: the recording overlay is dark too. Relaunch: still dark.
   Click the sun: light. Settings › About › Theme still offers System.
5. **Keyboard.** Click the page once, then Tab: focus goes to the collapse
   button, then each sidebar item, Help, the theme toggle and the bell, each
   with a visible ring. Enter on a sidebar item opens its page; Enter on Help
   opens the menu, ↓ and Enter pick an item; Enter on the bell opens the
   panel and Esc closes it with focus back on the bell.
6. **VoiceOver (S-13.2).** ⌘F5. VO-→ through the sidebar: VoiceOver reads the
   "Main" navigation landmark, each item as "<name>, button" and the current
   page as "current page"; the tags read "New!" / "Beta"; collapsed, each icon
   still reads its name. VO-U › Landmarks lists Main, the banner (header),
   main and content information (footer). ⌘F5 to stop.
7. **File drops (S-13.5).** Drag any `.csv` from Finder onto the Dictation
   page and drop it: nothing happens (no toast, no History entry). Open
   Settings › History and drop an `.m4a` or `.wav` on it: the dashed "drop"
   frame shows while over the list, a "Reading…" toast appears and the file
   is added to History. Drop the same file on the sidebar: nothing happens.
8. **Contrast.** System Settings › Accessibility › Display › Increase
   contrast on: every label stays readable in light and dark.

Record window sizes, screenshots (mocked or empty profile only, no personal
data) and the log lines in the run file.

### S-14 Settings modal and hotkeys panel

- [ ] S-14.1 Dock toggle on macOS 26 (activation policy flips).
- [ ] S-14.2 Launch at login after reboot.
- Procedure: TODO (S-14). Machines: M-26.

### S-15a Dictation home and stats rail

- [ ] S-15a.1 10 takes update the rail live.
- [ ] S-15a.2 The rail matches `daily_stats`; the home is the page the Hub
      opens on; the greeting uses the macOS first name.

**Profile: an isolated portable debug build** (as for S-09, own identifier
`com.wisprbro.s15acheck`, so the owner's app and data are never touched).
In the worktree:

```sh
echo '{"identifier":"com.wisprbro.s15acheck","plugins":{"updater":{"active":false}}}' > /tmp/s15a.conf.json
bun run tauri build --debug --no-bundle --config src-tauri/tauri.fork.conf.json --config /tmp/s15a.conf.json
cd src-tauri/target/debug
printf "Handy Portable Mode\n" > portable
mkdir -p Data/models && cp ~/wb-bench/asr/whisper-base/whisper-base-Q8_0.gguf Data/models/s15a-whisper-base.gguf
./handy --start-hidden & sleep 15; kill %1       # creates Data/
```

In `Data/settings_store.json` › `settings` set `onboarding_completed: true`
(the Hub shows), `selected_model: "s15a-whisper-base"`,
`recording_retention_period: "never"`, `paste_method: "none"`,
`vad_enabled: false`, `selected_language: "en"`. Seed 30 neutral rows over
five days: `python3 -I ../../../scripts/wb-s15a-seed.py Data`.

**Procedure S-15a.2 (2 min).** Launch `./handy` (window visible). The Hub
opens on Dictation with "Hello, <macOS first name>". Compare the rail with
`sqlite3 Data/fork.db "SELECT SUM(words), SUM(timed_words), SUM(speech_ms),
COUNT(*) FROM daily_stats WHERE dictations > 0"`: total words = words; words
per minute = round(timed_words / (speech_ms / 60000)); streak = days in a
row up to today (5 with the seed). Click the pencil, enter a name, Save: the
greeting changes; "Use my Mac account name" restores it.

**Procedure S-15a.1 (unattended, ~2 min, needs speakers and the mic):**
with the app running, `scripts/wb-s15a-takes.sh <pid> <…>/Data <out dir> 10`.
Each take (holding `~/wb-bench/gui.lock`) plays a `say` sentence between two
SIGUSR2s, then screenshots the app's window and prints the `daily_stats`
totals. Pass: every screenshot shows the totals printed for that take
(words, rounded WPM, streak) without any reload or click; the voice profile
count equals total words. Quit, relaunch: the same numbers.

Last run: `docs/fork/test-runs/1.0.0-alpha.1-M-26-S-15a.md` (both pass).

### S-15b History and row actions

- [ ] S-15b.1 Play audio (CSP check), retry, undo AI edit, delete/undo.
- [ ] S-15b.2 Flag → the word appears in Dictionary.
- [ ] S-15b.3 Grouping by day across midnight, file tag, expired audio
      ("Audio removed after 14 days"), Save audio as…, Copy uses the shown
      text.

**Profile: an isolated portable release build** with its own identity
("Wispr Bro S15b Check", `com.wisprbro.s15bcheck`). Release, because Play
goes through the asset protocol under the §4.11 `csp` (a debug build uses
`devCsp`), and portable mode needs the run-time asset scope S-15b adds. The
history is synthetic: `seed.py` renders each made-up sentence with `say -o`
to a file (never the speakers) and converts it to Handy's 16 kHz WAV. In the
worktree:

```sh
S=<scratch>/s15b   # a folder whose path contains s15b
echo '{"identifier":"com.wisprbro.s15bcheck","productName":"Wispr Bro S15b Check","plugins":{"updater":{"active":false}}}' > $S/s15b.conf.json
bun run tauri build --bundles app --config src-tauri/tauri.fork.conf.json --config $S/s15b.conf.json
# the updater-signature error at the end is expected (no F-02 key)
swiftc -O scripts/wb-s15b/ax.swift -o $S/wbax       # AX driver; the terminal needs Accessibility
swiftc -O scripts/wb-s25/ui.swift -o $S/wbui        # key presses for the save panel
swiftc -O scripts/wb-window-id.swift -o $S/wbwin
swiftc -O scripts/wb-s17/clip.swift -o $S/wbclip    # clipboard snapshot/restore
```

Copy the `.app` from `target/release/bundle/macos/` to `$S/app/`, add
`Contents/MacOS/portable` (`Handy Portable Mode`), re-sign ad hoc
(`codesign --force --deep -s -`), run `Contents/MacOS/handy --start-hidden`
once and quit. Then (app closed):

```sh
D=$S/app/Wispr\ Bro\ S15b\ Check.app/Contents/MacOS/Data
mkdir -p "$D/models" && cp -c ~/wb-bench/asr/parakeet-tdt-0.6b-v3/parakeet-tdt-0.6b-v3-Q8_0.gguf "$D/models/s15b-parakeet-v3.gguf"
# Muesli for "Retry with Medium cleanup" (sha256-checked by the app; nothing downloads)
M="$D/huggingface/hub/models--Muesli-HQ--qwen35-2b-muesli-cleanup-v12-gguf/snapshots/e35af097be4f17d3673515fb321ce19aa300faf6"
mkdir -p "$M" && cp -c ~/wb-bench/llm/qwen35-2b-muesli-v12-Q4_K_M.gguf "$M/"
python3 -I scripts/wb-s15b/setup.py "$D" s15b-parakeet-v3
python3 -I scripts/wb-s15b/seed.py "$D"          # prints the seeded ids
$S/wbclip save $S/clip.bin                         # put back at the end
```

Run the bundle executable directly (it shares the terminal's grants) and
hold `~/wb-bench/gui.lock` for every GUI step (`lockf -k ~/wb-bench/gui.lock
$S/wbax <pid> '…'`). Set the output volume to 0 for the Play step and put
it back after (Play is checked by the player's clock, not by ear).

**Procedure (unattended, ~6 min).** Driver notes: Radix menus (⋮) open on
a key or pointer press, not on an AX press, so open ⋮ with `front; focus
More actions#n; key return` in one locked call, then `press <item>` or `key
return`; use `=Dictation` for the sidebar item (plain "Dictation" also
matches the Flow Bar's "Dictation bar" button, which starts a take); if
another test app's window sits at the same default position, `move 60 60`
first. Never take focus from an app a person may be using.

1. Launch, `wbax <pid> front`, screenshot (`screencapture -l $(wbwin <pid>
| cut -d' ' -f1)`). Pass (S-15b.3): "Today" holds the AI take, the plain
   take, the file row (tag "Weekly sync.m4a") and the 00:01 note;
   "Yesterday" holds 23:59 and the Sioban row; the old row's day is a date;
   its Play button is named "Audio removed after 14 days" (`wbax <pid> 'find
Audio removed'`).
2. `press Play recording#1`, wait 1.5 s, `find Pause`; screenshot. Pass
   (S-15b.1 Play): the player shows "Pause" and its clock passed 0:00; the
   log has no CSP or asset error.
3. On the AI row: `press More actions#1; press Undo AI edit`. Pass: the row
   reads "so um the meeting is at 2 actually 3" with "Before AI cleanup";
   `entry_meta.shown_variant = 'raw'`; `daily_stats` unchanged. `press Copy
text#1`: `pbpaste` is the raw text. Redo AI edit puts "The meeting is at
   3." back.
4. On the plain row: More actions → "Retry with Medium cleanup". Pass
   (S-15b.1 Retry): "Transcribing…" then the new text; the log says
   `retry queued (cleanup medium)` and `re-transcribed`; `previous_text`
   holds the old text when it changed.
5. Plain row: More actions → Delete transcript; within 9 s `press Undo`.
   Pass: the row comes back, `deleted_at` is NULL, `history.db` still has it.
   Delete again and wait 12 s: `history.db` no longer has the row, the WAV is
   gone, `daily_stats` unchanged.
6. Flag (S-15b.2): `press Wrong word? Fix it#…` on the Sioban row, `press
Sioban`, `focus Correct spelling; key …; type Siobhán`, `press Add to
dictionary`. Pass: toast "From now on “Sioban” becomes “Siobhán”."; the
   Dictionary page lists "Sioban → Siobhán"; `dictionary_entries.source =
'flag'`, one `corrections` row (source `flag`), `entry_meta.flagged = 1`.
7. Save audio as… on the 23:59 row; in the panel `wbui <pid> 'key
cmd-shift-g; key cmd-a; keys $S/out/; key return; key return'`. Pass:
   `$S/out/dictation-<date>-2359.wav` equals the recording byte for byte.
8. Quit; `wbclip restore $S/clip.bin`; volume back; delete `$S/app` and
   `~/Library/{WebKit,Caches}/com.wisprbro.s15bcheck`.

Last run: `docs/fork/test-runs/1.0.0-alpha.1-M-26-S-15b.md` (S-15b.1,
S-15b.2 and the list part of S-15b.3 pass; Save audio as… not run in the
real app).

### S-16 Dictionary

- [ ] S-16.1 Add "Siobhán" and "gonna → going to"; dictate both in Mail.
- [ ] S-16.2 A Handy user's custom words appear once in the Dictionary
      ("From your earlier custom words") and stay in Handy's list.
- [ ] S-16.3 After a take that used a replacement, its row shows "Used N
      times" without a reload.
- [ ] S-16.4 (person) Import: drag a `.csv` from Finder onto step 1 of the
      wizard: it is imported, never transcribed; Export opens the save panel
      and the file opens in Numbers with accents intact.

**Profile: an isolated portable debug build** (own identifier
`com.wisprbro.s16check`; the owner's app and data are never touched). In
the worktree:

```sh
echo '{"identifier":"com.wisprbro.s16check","plugins":{"updater":{"active":false}}}' > /tmp/s16.conf.json
bun run tauri build --debug --no-bundle --config src-tauri/tauri.fork.conf.json --config /tmp/s16.conf.json
cd src-tauri/target/debug
printf "Handy Portable Mode\n" > portable
mkdir -p Data/models && cp ~/wb-bench/asr/parakeet-tdt-0.6b-v3/parakeet-tdt-0.6b-v3-Q8_0.gguf Data/models/s16-parakeet-v3.gguf
./handy --start-hidden & sleep 15; kill %1      # creates Data/
python3 -I ../../../scripts/wb-s16/setup.py "$PWD/Data" s16-parakeet-v3   # Handy custom words, empty dictionary
WB_DEBUG_TEXT_TRACE=1 ./handy &                 # window visible
```

**Procedure (unattended, ~3 min, needs the speakers and the mic).**
`say -v Samantha --data-format=LEI16@16000 -o /tmp/s16.wav "I'm gonna call
Siobhan about the Kubernetes rollout tomorrow."`, then:

1. S-16.2: the log has `dictionary: synced with Handy's custom words (taken
in 2, …)`; Dictionary shows Siobhán and Kubernetes.
2. `scripts/wb-s16/take.sh <pid> /tmp/s16.wav out-before.txt`: "gonna"
   stays.
3. Add "gonna → going to" in Dictionary › Add word (a person, or
   `swift scripts/wb-s16/ui.swift <pid> 'front; click …; type gonna; …'`
   with coordinates from a window screenshot, under `~/wb-bench/gui.lock`).
4. Take again: "going to"; the row shows "Used 1 time" (S-16.3);
   `sqlite3 Data/fork.db "SELECT from_text, to_text, hit_count FROM
dictionary_entries WHERE kind='rule'"` has hit count 1.

With a person: the same in Mail with a throwaway account, plus S-16.4.
A word with accents (Siobhán) only changes the take on a model that takes
vocabulary; on Parakeet add the replacement `Siobhan → Siobhán`.

Last run: `docs/fork/test-runs/1.0.0-alpha.1-M-26-S-16.md`.

### S-17 Snippets

- [ ] S-17.1 A three-line address snippet in Mail, Slack, Notes (use a
      made-up address): said alone it becomes exactly the three lines; said
      inside a sentence only the trigger is replaced; in a chat box nothing
      is sent (Shift+Return between the lines).
- [ ] S-17.2 After a take that used the snippet, the Snippets page shows the
      new use count ("Used N times", "used N times" in the header) without
      a reload.
- [ ] S-17.3 (person) Import a `.csv` with a multi-line text (quoted cell)
      by dragging it from Finder onto step 1; Export opens the save panel and
      the file opens in Numbers with the line breaks inside the cell.

**Profile: an isolated portable debug build** (own identifier
`com.wisprbro.s17check`; the owner's app and data are never touched). In
the worktree:

```sh
echo '{"identifier":"com.wisprbro.s17check","plugins":{"updater":{"active":false}},"bundle":{"externalBin":[]}}' > /tmp/s17.conf.json
bun run tauri build --debug --no-bundle --config src-tauri/tauri.fork.conf.json --config /tmp/s17.conf.json
cd src-tauri/target/debug
printf "Handy Portable Mode\n" > portable
mkdir -p Data/models && cp ~/wb-bench/asr/parakeet-tdt-0.6b-v3/parakeet-tdt-0.6b-v3-Q8_0.gguf Data/models/s17-parakeet-v3.gguf
./handy --start-hidden & sleep 15; kill %1      # creates Data/
python3 -I ../../../scripts/wb-s17/setup.py "$PWD/Data" s17-parakeet-v3   # no snippets yet
WB_DEBUG_TEXT_TRACE=1 ./handy --start-hidden &
./handy                                         # second launch shows the window
```

**Procedure (unattended, ~6 min, needs the speakers and the mic).**

1. Snippets › Add snippet: `my address`, text on three lines (a person, or
   `swift scripts/wb-s16/ui.swift <pid> 'front; click …; type my address;
key tab; type …; key return; …'` with coordinates from a window
   screenshot, under `~/wb-bench/gui.lock`), then click Add snippet.
2. `scripts/wb-s17/run.sh` (one round): alone and inline into TextEdit,
   the Safari editor page and the Chrome chat page (S-11's `take.sh`;
   snapshots and restores the clipboard). Every line must say PASS. The
   audio goes through the speakers (the real mic path), so keep to one
   round at moderate volume when someone is at the Mac; repetition is
   covered by the matcher's unit tests (the acceptance table), not by more
   takes.
3. S-17.2: a window screenshot of the page shows the use count;
   `sqlite3 Data/fork.db "SELECT hit_count FROM snippets"` matches, and
   `SELECT text_stages, inserted_text FROM entry_meta` shows
   `{"stage":"snippets","changes":1}` and the expansion per take.

With a person: the same in Mail (throwaway account), Slack and Notes, plus
S-17.3.

Last run: `docs/fork/test-runs/1.0.0-alpha.1-M-26-S-17.md`.

### S-18 Notifications

- [ ] S-18.1 RS-10 notification items on the real slice: delivery, the click
      opens the right page, the action buttons run their action, and the
      permission prompt appears at the first real need and not before.
- [ ] S-18.2 The same on the **Developer ID signed, notarized** build (F-02)
      in `/Applications`, launched normally, on macOS 26, 15 and 13 (M-26,
      M-15, M-13), plus a click on a notification after the app has quit
      (relaunch → right page) and "Notification Settings" in System Settings
      (opens the panel).
- [ ] S-18.3 In-app: the bell, the panel (All / Unread, ⋯ › Mark all as
      read, the four toggles), a toast while the Hub is in front, Esc closes
      it, the version's feature popup shows once.

**Profile (S-18.1, S-18.3): an isolated portable release build** with its
own identity ("Wispr Bro S18 Check", `com.wisprbro.s18check`), so the
owner's app, data and notification settings are never touched. Why release
and `~/Applications`: a debug build panics under `open` (it writes TypeScript
bindings relative to `/`), and usernoted refuses a bundle outside a location
LaunchServices accepts (`/tmp`: "Notifications are not allowed for this
application"). In the worktree:

```sh
echo '{"identifier":"com.wisprbro.s18check","productName":"Wispr Bro S18 Check","plugins":{"updater":{"active":false}}}' > /tmp/s18.conf.json
bun run tauri build --bundles app --config src-tauri/tauri.fork.conf.json --config /tmp/s18.conf.json
# the updater-signature error at the end is expected (no F-02 key)
swiftc -O -o /tmp/wbnotif scripts/wb-s18-notifications.swift   # the terminal needs Accessibility
```

Copy the `.app` from `target/release/bundle/macos/` to `~/Applications/`, add
`Contents/MacOS/portable` (`Handy Portable Mode`), re-sign
(`codesign --force --deep --options runtime --entitlements src-tauri/Entitlements.plist -s -`),
`lsregister -f` it, run `Contents/MacOS/handy --start-hidden` once and quit,
then set `onboarding_completed: true`, `paste_method: "none"` and
`update_checks_enabled: false` in `Data/settings_store.json` and add one row
to `Data/history.db` › `transcription_history` (the text "Copy latest" must
copy). Run the bundle executable directly: it then shares the terminal's
Accessibility and Microphone grants, so Handy's permission screen does not
replace the Hub. Save the clipboard text first and put it back at the end.
macOS asks an app only once: a second Allow run needs another identifier
(`PlistBuddy -c "Set :CFBundleIdentifier …"`, re-sign, `lsregister -f`).

**Procedure S-18.1 + S-18.3 (unattended, ~5 min, hold
`~/wb-bench/gui.lock`).**

1. Start with the Hub in front: the feature popup shows; Got it; the bell's
   panel lists the announcement. Pass: no `asking macOS` line in the log.
2. `open -a Finder`, then start the demo (samples through the real path):
   `WISPRBRO_NOTIFY_DEMO="8000:model_ready,queued_takes_ready,queued_takes_ready,shortcut_problem,update_ready" <app>/Contents/MacOS/handy --debug --start-hidden`.
   Pass: the first post logs `asking macOS for notification permission
(first real need: model_ready)` and macOS asks;
   `wbnotif action "S18 Check" Allow`; every later post logs `delivered to
the center` and no second request. (Don't Allow instead: `permission not
granted`, later posts `stay in the inbox`, no second prompt.)
3. With a Focus on, notifications wait in Notification Center:
   `wbnotif nc-toggle`, `wbnotif expand "S18 Check"`. Without one, act on the
   banners directly. Then, bringing Finder back to the front each time:
   - `wbnotif click "Dictation is ready"` → the Hub in front on Dictation.
   - `wbnotif action "3 dictations ready" "Copy latest"` → the clipboard holds
     the seeded text; Finder stays in front.
   - `wbnotif action "3 dictations ready" Open` → Settings › History.
   - `wbnotif action "shortcut is not working" "Open Settings"` → System
     Settings, Privacy & Security › Accessibility (quit it if it was not
     running).
   - `wbnotif click "Update ready"` → Settings › About.
     Pass: each logs `activated via System` with that action; the entries are
     read in `Data/fork.db` › `notifications`.
4. Hub in front (click into its window), demo `7000:crash_report,model_ready`.
   Pass: both show as toasts in the Hub and the model-ready one logs no
   `delivered` line; `wbnotif key-escape` closes the toast.
5. Clean up: close the test notifications (`wbnotif action "S18 Check"
Close`), quit, `lsregister -u`, delete the bundle and its
   `~/Library/{WebKit,Caches}/<identifier>` folders, put the clipboard back.
   macOS keeps the app's row in System Settings › Notifications until it
   prunes it (no supported command removes it).

`wbnotif list <filter>` prints only matching notifications; never print
Notification Center unfiltered (`dump`), it holds the user's own
notifications.

Last run: `docs/fork/test-runs/1.0.0-alpha.1-M-26-S-18.md` (S-18.1 on an
ad-hoc build and S-18.3 pass; S-18.2 open).

### S-19a Styles: app groups and presets

- [ ] S-19a.1 Slack → Work, Messages → Personal, Mail → Email: the Style
      page lists each installed one in its group (Slack only if installed);
      a take into the app gets the group's preset (`style:` log line,
      `entry_meta.style_group`).
- [ ] S-19a.2 Each preset on a real take, through the whole chain (the app
      in front → the user's rule → the preset → cleanup → stage 10 → the
      field): Very relaxed (lower-case starts, no commas, no final period),
      Upbeat (ends with "!"), Formal (as cleaned), Relaxed; a Dutch take in
      a styled group comes out unstyled (English only).
- [ ] S-19a.3 "Press Enter after dictating": after a confirmed paste the
      app gets Return; the bar says "Sends automatically" at the take
      start; with the cleanup forced to fail (`WB_DEBUG_CLEANUP_DEADLINE_MS=1`)
      no Return.
- [ ] S-19a.4 The page in the real app: move an app with its menu, Undo,
      "+" lists the installed apps with their icons; the change is used by
      the next take without a restart.
- [ ] S-19a.5 RS-07 gates hold with every preset: the S-12a replay without
      a style (`scripts/wb-s19a/replay.sh <dir> none`; on a quiet machine,
      p50 near 300 ms), then each preset applied to that run
      (`scripts/wb-s19a/restyle.sh <dir>/run-none.jsonl <dir2>`; no preset
      changes the prompt, so this is exactly what a styled take inserts).

**Profile: an isolated portable debug build** (own identifier
`com.wisprbro.s19acheck`, which runs as a product build; never the owner's
app or data). In the worktree, as S-12a:

```sh
ci/llama-server/build.sh --if-missing
echo '{"identifier":"com.wisprbro.s19acheck","plugins":{"updater":{"active":false}}}' > /tmp/s19a.conf.json
bun run tauri build --debug --no-bundle --config src-tauri/tauri.fork.conf.json --config /tmp/s19a.conf.json
cd src-tauri/target/debug
printf "Handy Portable Mode\n" > portable
mkdir -p Data/models && cp ~/wb-bench/asr/parakeet-tdt-0.6b-v3/parakeet-tdt-0.6b-v3-Q8_0.gguf Data/models/
./handy --start-hidden & sleep 20; kill %1      # creates Data/
```

**Procedure (unattended, ~8 min, needs the speakers and the mic).**
`WB_S19A_MODEL_ID=parakeet-tdt-0.6b-v3 scripts/wb-s19a/run.sh personal work email other autosend fallback`:
per phase it stops the app (main process only), writes the phase's styles
and `app_rules` rows (`scripts/wb-s19a/setup.py`: TextEdit put in the
group under test, auto-send on for the last two), starts the app again
and dictates into new TextEdit documents through S-12a's `take.sh` (one
warm-up take per phase, under `~/wb-bench/gui.lock`, one round of TTS at
moderate volume). Check the printed document texts (a trailing `|` is the
Return), the log's `style:` and `insert:` lines (`submitted`) and
`fork.db` (`entry_meta.style_group`, `cleanup_outcome`). S-19a.1 and .4:
start the app once more, open the Hub (second launch), Style, and take
window screenshots (`scripts/wb-window-id.swift` + `screencapture -l`).
TextEdit stands in for Slack, Messages and Mail in S-19a.2/.3: those hold
the owner's real conversations and drafts and are never typed into.`

### S-19b Styles: web-app detection

- [ ] S-19b.1 RS-04 URL rows on the real slice.
- [ ] S-19b.2 Gmail in Chrome / Arc / Safari → Email.
- Procedure: TODO (S-19b).

### S-20 Context awareness

- [ ] S-20.1 Dictate a colleague's unusual surname visible in a Slack thread (a made-up name in a test workspace); verify the spelling and that the text never hits disk.
- [ ] S-20.2 The same take with context awareness off spells it as the recognizer heard it.
- [ ] S-20.3 No read in excluded places: a "Card number" field, Terminal (and by the unit tests: password fields, Secure Input, password managers, banking apps and sites, URL bars, numeric fields).
- [ ] S-20.4 The read stays within 100 ms (p95 of the `context: read` log lines) and the clipboard is byte-identical after every take.
- [ ] S-20.5 (person) Slack itself, a real colleague's name, the glyph seen on the bar, VoiceOver reading "Nearby text helped with spelling".

**Profile:** the S-12a portable debug build with a test identity
(`com.wisprbro.s20check`; never the owner's app, data or clipboard). In
the worktree:

```sh
ci/llama-server/build.sh --if-missing
echo '{"identifier":"com.wisprbro.s20check","plugins":{"updater":{"active":false}}}' > $SCRATCH/s20.conf.json
bun run tauri build --debug --no-bundle --config src-tauri/tauri.fork.conf.json --config $SCRATCH/s20.conf.json
cd src-tauri/target/debug && printf "Handy Portable Mode\n" > portable
mkdir -p Data/models && cp -c ~/wb-bench/asr/parakeet-tdt-0.6b-v3/parakeet-tdt-0.6b-v3-Q8_0.gguf Data/models/s20-parakeet-v3.gguf
cd ../../.. && scripts/wb-s05/app.sh init
scripts/wb-s20/setup.sh "$PWD/src-tauri/target/debug/Data" s20-parakeet-v3 ~/wb-bench/llm/qwen35-2b-muesli-v12-Q4_K_M.gguf on
swiftc -O -o $BIN/clip scripts/wb-s05/clip.swift
say -v Samantha -o $SCRATCH/s20.aiff "Thanks Siobhán Kowalczyk, I will send the checklist to Thibault today."
afconvert -f WAVE -d LEI16@16000 -c 1 $SCRATCH/s20.aiff $SCRATCH/s20.wav
scripts/wb-s05/app.sh start
```

**Procedure (unattended):** `WB_S20_BIN=$BIN scripts/wb-s20/take.sh <case>
<wav|none> <out dir>` for `textedit` and `chrome` with the WAV (through
the speakers into the mic, once each, moderate volume), then
`setup.sh … off`, restart, `textedit` with the WAV again (S-20.2), then
`card none` and `terminal none` (silent takes: the read still runs at
record start). Each case holds `~/wb-bench/gui.lock`, opens its own
window (a TextEdit document whose text names the people; the
`thread.html` stand-in for a Slack thread in a separate Chrome instance;
a Terminal window running `cat`), records only while that window is in
front, saves and checks the whole clipboard, and closes what it opened.
The log lines (`context: read <outcome> (<chars>, <hints>, <ms>)`,
`context: this take used its hints`) carry counts only. Then S-20.1's
disk check, with the app stopped: `grep -rl "Zebulonix\|Quarterbridge"
src-tauri/target/debug/Data` must find nothing (a name only the context
had, never said), nor the context's sentence; `entry_meta.context_used`
is 1 for the context-on takes.

The name eval (exit criterion): `bun scripts/wb-s20/names-audio.ts
$SCRATCH/names` (TTS written to files, never played), then
`WB_ASR_MODEL=<gguf> WB_NAMES_AUDIO=$SCRATCH/names WB_EVAL_OUT=<run.jsonl>
WB_LLAMA_SERVER=<sidecar> WB_LLM_MODEL=<muesli gguf> cargo test --lib
fork::context::names_eval -- --ignored --nocapture` per speech model.

### S-21 Transforms core

- [ ] S-21.1 RS-04 replace rows on the real slice.
- [ ] S-21.2 RS-11 ⌥ characters on the real slice.
- [ ] S-21.3 Polish in Gmail, Slack, VS Code, Notes.
- Procedure: TODO (S-21).

### S-22 Transforms page, editor, diff, auto-apply

- [ ] S-22.1 ⌥O after a transform in Slack.
- [ ] S-22.2 Auto-apply on a 3-sentence dictation.
- Procedure: TODO (S-22).

### S-23 Command Mode

- [ ] S-23.1 20 commands across Mail, Slack, Docs; ⌘Z restores.
- Procedure: TODO (S-23).

### S-24 Scratchpad

- [ ] S-24.1 Dictate into the note window.
- [ ] S-24.2 Open via ⌥S after binding.
- [ ] S-24.3 The note survives a force-quit.
- Procedure: TODO (S-24).

### S-25 Insights

- [ ] S-25.1 "Save as image" works in the release build under the CSP (§4.11).
- [ ] S-25.2 The Usage tab matches numbers computed by hand from the
      profile's tables; no percentile anywhere.
- [ ] S-25.3 Voice is locked below 1,700 words (blurred cards) and shows the
      four cards once the history crosses 1,700 words.

**Profile: an isolated portable release build** with its own identity
("Wispr Bro S25 Check", `com.wisprbro.s25check`), so the owner's app and
data are never touched. Release, because S-25.1 is about the release CSP
(a debug build uses `devCsp`). No audio is needed: the history is
synthetic and seeded. In the worktree:

```sh
echo '{"identifier":"com.wisprbro.s25check","productName":"Wispr Bro S25 Check","plugins":{"updater":{"active":false}}}' > <scratch>/s25.conf.json
bun run tauri build --bundles app --config src-tauri/tauri.fork.conf.json --config <scratch>/s25.conf.json
# the updater-signature error at the end is expected (no F-02 key)
swiftc -O scripts/wb-s25/ui.swift -o <scratch>/wbui        # the terminal needs Accessibility
swiftc -O scripts/wb-window-id.swift -o <scratch>/wbwin
```

Copy the `.app` from `target/release/bundle/macos/` to a scratch folder
whose path contains `s25`, add `Contents/MacOS/portable` (`Handy Portable
Mode`), re-sign ad hoc (`codesign --force --deep -s -`), run
`Contents/MacOS/handy --start-hidden` once and quit. In
`Contents/MacOS/Data/settings_store.json` › `settings` set
`onboarding_completed: true` and `update_checks_enabled: false`. Seed:
`python3 -I scripts/wb-s25/seed.py <…>/Data locked <scratch>/expected-locked.json`
(1,6xx made-up words over 21 local days, five apps; it prints the numbers
the page must show, computed in Python from the tables, not by the app).
Run the bundle executable directly so it shares the terminal's grants.

**Procedure (unattended, ~5 min; hold `~/wb-bench/gui.lock` for each GUI
step; every event goes only to the test app's own window).**

1. Launch, `wbui <pid> front`, click Insights. Screenshot the window
   (`screencapture -l $(wbwin <pid>)`). Pass (S-25.2): speed = `wpm`,
   "× the speed of an average typist (40 wpm)" = `times_typing_vs_40`,
   total words, dictionary and AI cleanup fixes, the category shares, current
   and longest streak and active days equal `expected-locked.json`; the
   heatmap's last cells have the expected bands; the page has no
   "percentile".
2. Voice tab. Pass (S-25.3): "unlocks at 1,700 words", the progress text
   equals total words, four blurred cards.
3. Usage tab, "Save as image"; in the save panel `wbui <pid> 'key
cmd-shift-g; key cmd-a; keys <scratch>/out/; key return; key return'`
   (the panel runs out of process and takes only real key presses, so
   `keys`, not `type`; never let it save into the user's Documents). Pass
   (S-25.1): a PNG at the path (2× the export area plus a 24 px margin), the
   log says `insights: saved an image`, no error in the log, the image shows
   the page's own fonts, the chart and the heatmap.
4. Quit, `seed.py <…>/Data unlock <scratch>/expected-unlock.json`, relaunch,
   Insights › Voice. Pass (S-25.3): unlocked; phrase "Sounds good to me",
   peak hour and app, most used word and most corrected word equal the
   JSON; "Save as image" on the Voice tab also writes a PNG.
5. Quit; delete the bundle copy and `~/Library/{WebKit,Caches}/com.wisprbro.s25check`.

Last run: `docs/fork/test-runs/1.0.0-alpha.1-M-26-S-25.md` (all three pass).

### S-26a App menus and tray

- [ ] S-26a.1 ⌘C / ⌘V / ⌘Z in Hub text fields.
- [ ] S-26a.2 The Help search box finds menu items.
- [ ] S-26a.3 The tray icon comes back after the menu bar process (ControlCenter) restarts (A-69).
- Procedure: TODO (S-26a).

### S-26b Help popover, support, Share, About

- [ ] S-26b.1 ⌘/ opens a prefilled issue in the browser.
- [ ] S-26b.2 The share sheet shows.
- Procedure: TODO (S-26b).

### S-27 Data and Privacy, export, delete all, diagnostics

- [ ] S-27.1 Export opens in Finder.
- [ ] S-27.2 Erase, then relaunch shows onboarding.
- [ ] S-27.3 `brew uninstall --zap` paths match (S-39).
- Procedure: TODO (S-27).

### S-28 Updates that never interrupt

- [ ] S-28.1 RS-12 on the real slice.
- [ ] S-28.2 An update during a long hands-free take waits.
- Procedure: TODO (S-28).

### S-29a Onboarding: welcome, permissions, the two tests

- [ ] S-29a.1 Clean Mac: permissions to shortcut test in ≤ 2 min.
- [ ] S-29a.2 Each diagnosis branch reachable.
- Procedure: TODO (S-29a).

### S-29b Onboarding: languages, practice, celebration, typing test, summary

- [ ] S-29b.1 10 people on clean Macs (8 GB and 16 GB): median ≤ 5 min, first dictation within 2 min of install (§5.3). (R5's "one Intel" is skipped under DEC-29.)
- Procedure: TODO (S-29b). Evidence: one row per tester, no names (tester ids only).

### S-30 First-use chooser, pill spotlight, setup checklist

- [ ] S-30.1 The spotlight panel is click-through and sits above full-screen apps.
- [ ] S-30.2 It dismisses on the first dictation in Mail.
- Procedure: TODO (S-30).

### S-31 Lesson engine and Transforms tutorial

- [ ] S-31.1 (Exit criterion; R5 has no separate real-Mac line) The Transforms lesson completes end-to-end on a real Mac in under 2 min.
- Procedure: TODO (S-31).

### S-32 Whisper tutorial and microphone quiz

- [ ] S-32.1 Whispered take on the starter and the full model at 30 cm.
- Procedure: TODO (S-32).

### S-33 ITN (gated by RS-13)

- R5 lists no real-Mac acceptance; RS-13 decides whether the slice ships.

### S-34 Dictionary auto-learn

- [ ] S-34.1 Correct "Kubernetees" → "Kubernetes" in Slack; the suggestion appears.
- Procedure: TODO (S-34).

### S-35 Vocabulary importers

- [ ] S-35.1 (Exit criterion) Import works on the owner's Mac with a real Wispr Flow install. Evidence: counts only, never the imported words.
- Procedure: TODO (S-35).

### S-36 Vibe coding

- [ ] S-36.1 Dictate "rename get user by id" in Cursor with the identifier visible.
- Procedure: TODO (S-36).

### S-37 Accessibility, localization, copy pass

- [ ] S-37.1 The VoiceOver checklist passes for onboarding, the Hub, settings and one dictation.
- [ ] S-37.2 Increase Contrast and Reduce Motion on.
- Procedure: TODO (S-37).

### S-38 Performance and reliability hardening

- [ ] S-38.1 (Exit criterion) Every §5 number met on each reference machine, or the model tier changed to meet it (recorded): latency benches p50/p95 per §5.1, 8 h idle run, 500-take soak, 50-take hardware run, memory-pressure test on 8 GB, startup timing.
- Procedure: TODO (S-38). Machines: all.

### S-39 1.0 release

- [ ] S-39.1 RS-12 re-run `1.0.0-rc → 1.0.0`.
- [ ] S-39.2 Install and `brew uninstall --zap`.
- [ ] S-39.3 Part A passes on all reference machines (R5 §9).
- Procedure: TODO (S-39).
