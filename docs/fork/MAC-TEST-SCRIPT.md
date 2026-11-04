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

**Binary name.** Since S-38 the executable is `wispr-bro` (`mainBinaryName`):
a bundle's `Contents/MacOS/wispr-bro` and a `tauri build --no-bundle`'s
`target/<profile>/wispr-bro`; a plain `cargo build` still writes `handy`.
Where a procedure below says `./handy`, run whichever of the two is there
(newest). The drivers do this through `scripts/wb-lib/app.sh` (`wb_bin_name`
reads `CFBundleExecutable` for a bundle; `wb_cancel` sends `--cancel` only to
a running instance and kills a call that does not return in 15 s: with no
instance, `--cancel` starts a whole app, the 9-minute hang of Mac wave 1).
Never symlink one name to the other (Tauri's resource lookup panics).

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
- [ ] A1.4 Onboarding timing: first dictation ≤ 2 min after install and ≤ 60 s after permissions (§5.3). Procedure: S-29a.1 and S-29b.1 (`scripts/wb-s29b/run.sh timing`).

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
- [ ] A9.2 Beta channel; a stable install ignores beta. Automated in F-02 (`fork::update::tests`), loopback run F-02.3a; real feeds F-02.4.
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

- [x] F-02.1 **Mac wave 4: PASS (rc1: app `4c58627d-0d93-4415-ac5e-45abfd0fa630` and DMG `39caa9cd-24be-41fa-ab8e-05b10e99ebea` Accepted, both stapled, updater archive signed by `89E085217ACC46CF`, `verify.sh: all checks passed` in the script and again standalone; no dialog, no prompt)** ([run](test-runs/1.0.0-alpha.1-M-26-macwave-4.md)) Release pipeline on this Mac (queued for the Mac wave; the one
      release-build run): first let codesign use the Developer ID key without
      a dialog (RELEASING.md "Locally"), then
      `WB_NOTARY_PROFILE=wispr-bro-notary scripts/wb-release/macos.sh --out ~/wb-release/1.0.0-alpha.1`.
      Expect: nested code check passes (Team ID, runtime, timestamp),
      notarization `Accepted` for the app and the DMG, both stapled, the
      updater archive verified against key `89E085217ACC46CF`, and
      `verify.sh: all checks passed` (codesign deep/strict, `spctl -t exec`
      Notarized Developer ID, `spctl -t open` on the DMG, `stapler validate`
      ×2, entitlements, `--list-devices`). If notarization fails, fix and
      resume with `--reuse-app` (no rebuild). Keep `release.log` and the
      notary logs as evidence.
- [ ] F-02.2 Clean-Mac install from the DMG (queued for the Mac wave, then
      every release): download the DMG through a browser (quarantine set),
      open it, drag to Applications, first launch shows no Gatekeeper warning
      beyond "downloaded from the Internet"; macOS 26 (M-26) and 13 (M-13).
- [x] F-02.3 **Mac wave 1: PASS 7/7** ([run](test-runs/1.0.0-alpha.1-M-26-macwave-1.md)) Update end to end over a loopback GitHub stand-in, no clicks
      (queued for the Mac wave): `scripts/wb-f02/update-e2e.sh <out>` builds
      A (overlay version) and B (one prerelease step up) as Developer-ID
      signed debug bundles (`com.wisprbro.f02check`), signs B's archive with
      the real updater key, serves a GitHub-shaped tree on 127.0.0.1:18092 and
      runs A with a throwaway HOME. Expect PASS for F-02.3a (stable install
      ignores a newer beta), .3b (A installs B), .3c (relaunched as B), .3d
      (B up to date), .3e (bundle on disk is B), .3f (codesign deep/strict),
      .3g (designated requirement unchanged, so privacy grants carry over).
      Stop: the script stops the app and the server itself; delete `<out>`.
- [ ] F-02.4 RS-12 on real GitHub releases (release wave, needs the push):
      install alpha.N from the DMG, grant Microphone + Accessibility, publish
      alpha.N+1, update from the debug panel (Updates › Check now / Install),
      permissions still granted; publish a `-beta.N` and confirm a stable
      install reports "Up to date" while a beta-channel install updates.
      Procedure: `docs/fork/spikes/RS-12.md` once written. Machines: M-13, M-26.

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
   (`"…/Wispr Bro.app/Contents/MacOS/wispr-bro" --debug`), so the log has the
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

**Queued for the Mac wave** (signed build; the backend run that shares the
hotkey harness). Done already, scripted, without a person (2026-11-02):
`scripts/wb-s02/rs02-notify.sh` (privacy-record edits post
`com.apple.tcc.access.changed` 10/10, ≤ ~100 ms) and
`scripts/wb-s02/rs14-carbon.swift` (Carbon Esc/⌃⌘V fire 10/10 under Secure
Input while the tap sees no key-down; an unregistered Esc reaches the
window). Reports: `docs/fork/spikes/RS-02-tap-health.md`,
`RS-14-secure-input.md` round 2. Log lines are at debug level
(`--debug`), prefix `fork hotkeys health:`.

- [ ] S-02.1 **Mac wave 2: NOT RUN (sleep/wake of the owner's Mac; owner)** ([run](test-runs/1.0.0-alpha.1-M-26-macwave-2.md)) RS-02 (a): sleep 10 min, wake, press fn within 2 s → a take
      starts; ×10. Log: `Wake`/`ScreensWake` probe ok, `tap rebuilt`, Handy's
      `handy-keys manager rebuilt`.
- [ ] S-02.2 **Mac wave 2: NOT RUN (TCC switches; owner)** ([run](test-runs/1.0.0-alpha.1-M-26-macwave-2.md)) RS-02 (b): System Settings › Privacy & Security ›
      Accessibility, switch Wispr Bro **off** while it runs: within 2 s the
      bar shows "Your shortcut stopped working" with Fix, the tray shows the
      badge and "Shortcut stopped working", the Hub banner appears; a
      notification only if the app is not in front. Switch **on**: within
      5 s "Your shortcut works again", fn starts a take, no relaunch. Repeat
      by **removing** the row (−) and adding it back (+).
- [ ] S-02.3 **Mac wave 2: NOT RUN (re-signed install + TCC re-grant; owner)** ([run](test-runs/1.0.0-alpha.1-M-26-macwave-2.md)) RS-02 (c): install a re-signed build over the running app;
      loss within 2 s; Fix sheet › Reset and re-grant → the macOS prompt →
      allow → recovery within 5 s, or "Relaunch to finish" works in one click.
- [ ] S-02.4 **Mac wave 2: NOT RUN (lid and external display; owner)** ([run](test-runs/1.0.0-alpha.1-M-26-macwave-2.md)) RS-02 (d): lid closed on an external display, wake, fn works.
- [ ] S-02.5 **Mac wave 2: NOT RUN (needs a TCC switch; owner)** ([run](test-runs/1.0.0-alpha.1-M-26-macwave-2.md)) RS-02: both notifications (`AccessibilityChanged`,
      `PrivacyChanged`) appear in the signed build's debug log on a switch.
- [ ] S-02.6 **Mac wave 2: NOT RUN (real fn holds; the owner's app holds fn)** ([run](test-runs/1.0.0-alpha.1-M-26-macwave-2.md)) RS-14 for each holder — Safari password field, Terminal Secure
      Keyboard Entry, iTerm2 secure input (if installed): fn hold → take;
      double-tap fn → hands-free; fn+Space does not lock and the bar shows
      "<App> has Secure Input on · fn+Space can't lock · Esc or ✕ cancels"
      for 5 s; Esc cancels a held take, a hands-free take and a
      transcription (log `Secure Input shadow fired (cancel)` once Secure
      Input has lasted 3 s); ⌃⌘V pastes the last dictation; Ctrl with fn held
      starts a Command Mode take when it is on and plain dictation when off;
      the Globe action stays suppressed (real fn key); **idle** Esc in the
      password field reaches the field (not swallowed).
- [ ] S-02.7 **Mac wave 2: NOT RUN (the ⌃⌘V/⌃⌘C Carbon shadows would collide with the owner's app)** ([run](test-runs/1.0.0-alpha.1-M-26-macwave-2.md)) A-205: Settings › Debug › Secure Input status while Terminal
      holds it lists `fork:paste_last`, `fork:copy_last` (covered) and any
      fn+key fork binding (uncovered); Handy's banner and tray describe them.
- [ ] S-02.8 **Mac wave 2: NOT RUN (first launch without Accessibility; owner TCC)** ([run](test-runs/1.0.0-alpha.1-M-26-macwave-2.md)) First launch without Accessibility, onboarding grants it: the
      shortcut works right after the grant without a relaunch, and nothing
      warned before onboarding finished.

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
   `Data/settings_store.json`. Start `…/Contents/MacOS/wispr-bro --start-hidden
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
- [ ] S-05.3 Card appears on the desktop and in Finder. (OT2-A: now "Copied to
      clipboard" and the text on the clipboard; OT2-A.6.)
- [ ] S-05.4 Start a take in Slack, release in Notes → text in Notes.
- [ ] S-05.5 Switch apps during processing → card. (OT2-A: now the clipboard
      and "Copied to clipboard"; OT2-A.6.)
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

`app.sh start` (since the Mac wave 2 fixes) unbinds every global shortcut
of the profile except those named on its command line, clones a model cache
into `Data/huggingface` when there is none (`scripts/wb-lib/profile.py`),
and starts the app with its test microphone
(`WISPRBRO_TEST_MIC_WAV=Data/test-mic.wav`): the take drivers load each WAV
into it (`wb_mic_load`/`wb_mic_play`, `scripts/wb-lib/app.sh`) and refuse to
play through the speakers unless `WB_ALLOW_SPEAKERS=1`.

Helpers (compiled once): `for h in clip drive; do swiftc -O -o $BIN/$h
scripts/wb-s05/$h.swift; done; swiftc -O -o $BIN/pill scripts/wb-s03-pill.swift`.

**Procedure (S-05.2 to S-05.7, unattended, ~1 min per take, no speakers):** `WB_S05_BIN=$BIN scripts/wb-s05/run.sh <out dir> [rounds]` (or `python3 -I scripts/wb-s05/run.py …` when executing scripts made on this Mac hangs: syspolicyd down). Before the stop each take checks again that its own window has focus and cancels the take otherwise (`handy --cancel`), so nothing goes to a window the run did not open. Each
take holds `~/wb-bench/gui.lock`, opens its own window, checks the
frontmost app before recording, records through Handy's toggle (SIGUSR2)
while the app's test microphone plays `say`'s WAV, and closes only what it
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
scripts/wb-s05/app.sh start paste_last,copy_last press_enter=mousex1 undo_cancel=ctrl+option+z
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
the WAV (the app's test microphone, `app.sh start`; never the speakers;
every shortcut but those four unbound). Every case holds `~/wb-bench/gui.lock`, opens its own window
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
SIGUSR2 and a `say` WAV played by the app's test microphone
(`WISPRBRO_TEST_MIC_WAV` exported for the app and `scripts/wb-s07a/take.sh`;
never the speakers), so no
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
on a real link (or Network Link Conditioner by a person). The take of
S-07a.2 uses the test microphone, not the speakers.

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
- [x] S-07b.8 **Mac wave 2: PASS** ([run](test-runs/1.0.0-alpha.1-M-26-macwave-2.md)) No model loop (Mac wave 1, finding 12): with slow takes on
      record the policy steps down once and stays there over two more
      launches; no second download, the deleted model does not come back.
      _(queued for the Mac wave: `scripts/wb-s07b/loop.sh`, no audio)_
- [x] S-07b.9 **Mac wave 2: PASS (load 28.5: speed not recorded, paste in the target's second, no model work)** ([run](test-runs/1.0.0-alpha.1-M-26-macwave-2.md)) Under load a take's paste is not delayed by model work and its
      time is not recorded. _(queued for the Mac wave; one take through the
      speakers)_

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
6. S-07b.8 (after step 1, app quit, mirror running):
   `scripts/wb-s07b/loop.sh <app> <out>`. It seeds 10 warm takes at 18×
   RS-09's p50 for unified-en spread over 90 min and launches three times.
   Pass: it prints `PASS S-07b.8` (launch 1 logs `measured slower than its
class` and `swapped`; launches 2-3 log `→ Stay` and no `downloading a
better speech model`; `measured_factor` is in `model_policy_state`;
   unified-en stays deleted).
7. S-07b.9: relaunch with `run.sh … relaunch en`, start CPU load
   (`for i in $(seq $(sysctl -n hw.ncpu)); do yes >/dev/null & done`), wait
   60 s, one `scripts/wb-s07a/take.sh <pid> <aiff>` into TextEdit, then
   `killall yes`. Pass: the log has `take speed not recorded (load …)`; the
   take's `insert: take N … pasted` line is within 2 s of its `insert: take N
target` line; no `models:` download, checksum or swap line falls between
   the take's start and its paste.

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
`kill -USR2`; every launch unbinds the profile's shortcuts and clones a
model cache (`scripts/wb-lib/profile.py`). `run.sh take` and `relaunch`
start the app with its test microphone (`open --env
WISPRBRO_TEST_MIC_WAV=$OUT/test-mic.wav`) and play a `say -o` WAV through
it, never the speakers. The device cases need the real microphone:
`relaunch <mute> real` before `revoke`/`denied` (silent; the driver refuses
otherwise). The mic loss is the one check the test microphone cannot give
(it opens no device, so there is nothing to cut): `run.sh loss` refuses
unless `WB_ALLOW_SPEAKERS=1`, then plays one short WAV at
`WB_SPEAKER_VOLUME` (default 0.6). Hold `~/wb-bench/gui.lock`
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
3. Revoke + quit (`grant.sh`, `run.sh relaunch 0 real`, `run.sh revoke`): `tccutil reset Microphone
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

**Procedure S-09.1 (unattended, ~13 min, no speakers):**
`scripts/wb-s05/app.sh start` (test microphone, shortcuts unbound), then
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
- [x] S-10.9 **Mac wave 4: PASS (fresh copy: warm-up 7.7 s kept its server for the take, "a take wants the server, it stays up"; text in our TextEdit 0.9 s after the stop; no second cold start); the driver needed four fixes on its first run** ([run](test-runs/1.0.0-alpha.1-M-26-macwave-4.md)) **Queued for the Mac wave** (Mac wave 3 fixes, finding 3:
      the first take after a fresh start took 19 s). The first take after a
      fresh start, during the launch warm-up: run
      `scripts/wb-s10/first-take.sh` with `APP` = a debug test bundle
      (onboarded, models in place) and `OUT` = a folder. It removes
      `Data/llm/warm-*` so the warm-up runs, and starts one SIGUSR2 take of
      a `say` WAV through the test microphone the moment the cleanup model
      is ready, into its own TextEdit; no speakers. PASS = S-10.9a text
      inserted ≤ 8 s after the take stops; .9b no `llama-server stopped`
      between the warm-up's start and the take's cleanup (the log may say
      "warm-up done; a take wants the server, it stays up"); .9c no second
      `llama-server ready in` before the take's cleanup. A start still cold
      at cleanup time pastes the raw text within the 2.5 s start budget
      (`cleanup: timeout`), never waits longer.

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
cd ../../.. && WB_DEBUG_TEXT_TRACE=1 scripts/wb-s05/app.sh start  # debug builds log each take's texts
```

**Procedure (unattended, ~2 min per round, no speakers: the app plays each take's WAV as its test microphone (`WISPRBRO_TEST_MIC_WAV`, set by `scripts/wb-s05/app.sh start`), never through the speakers):** `scripts/wb-s11/run.sh 3`. Each round dictates six takes with
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
cd ../../.. && WB_DEBUG_TEXT_TRACE=1 scripts/wb-s05/app.sh start
```

`setup.py` sets Parakeet, VAD off, spoken languages en/nl/fr, puts the
pinned Muesli GGUF in the profile's HF cache (its sha256 is checked, nothing
is downloaded), sets the level and two dictionary replacements. Stop the
app by its main process only (the other `handy --start-hidden` is S-10's
watchdog, which stops the sidecar).

**Procedure (unattended, no speakers: the app plays each take's WAV as its test microphone (`WISPRBRO_TEST_MIC_WAV`, set by `scripts/wb-s05/app.sh start`), never through the speakers):**
`scripts/wb-s12a/run.sh light 2` (seven takes per round: en fillers +
repair, an en question, an en dictionary spelling, nl lead-ins, an nl
repair, fr lead-ins, an fr repair), then restart the app with
`WB_DEBUG_CLEANUP_DEADLINE_MS=1 scripts/wb-s05/app.sh start` and run `run.sh timeout`, then
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
3. Restart with `WB_DEBUG_LLM_HOLD_MODEL=1 scripts/wb-s05/app.sh start` only
   and run `scripts/wb-s12b/takes.sh <out dir>` (three takes through the
   app's test microphone, never the speakers; the clipboard is saved and restored by
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
3. **Onboarding keeps the overlay title bar** (Design Phase 0 changed this:
   it used to switch back to the standard bar). Settings › Debug (turn on
   debug mode with ⌘⇧D) › Run onboarding: the traffic lights stay centred in
   the 46 pt strip above the stepper, and the strip drags the window.
4. **Theme (S-13.4).** Click the moon icon: the whole window turns dark
   (sidebar, page, Handy's settings tabs) and the icon becomes a sun.
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
   and main (no footer since Design Phase 0). ⌘F5 to stop.
7. **File drops (S-13.5).** Drag any `.csv` from Finder onto the Dictation
   page and drop it: nothing happens (no toast, no History entry). Open
   Settings › History and drop an `.m4a` or `.wav` on it: the dashed "drop"
   frame shows while over the list, a "Reading…" toast appears and the file
   is added to History. Drop the same file on the sidebar: nothing happens.
8. **Contrast.** System Settings › Accessibility › Display › Increase
   contrast on: every label stays readable in light and dark.

Record window sizes, screenshots (mocked or empty profile only, no personal
data) and the log lines in the run file.

### Design Phase 0 (foundations) — queued for the Mac wave

Built 2026-11-03 on `design/foundations` (docs/fork/design/SYSTEM-PLAN.md §7
Phase 0). A debug build is enough (`bun run wb:dev`); one pass, by eye,
mocked or empty profile only.

- [x] DP0.1 **Mac wave 6: FIX → PASS (lights were 2.25 pt high; centre now 23.0 pt)** ([run](test-runs/1.0.0-alpha.1-M-26-macwave-6.md)) Title strip: the traffic lights are vertically centred in the
      46 pt strip, close button's centre about 21 pt from the left edge, in
      the Hub and on every onboarding step; they stay put after resizing,
      full screen and back, and after the Hub is parked and rebuilt (S-38).
      If they sit too high or low, adjust `TRAFFIC_LIGHTS` in
      `src-tauri/src/fork/hub/mod.rs` (tao's inset; one number) and note it.
- [ ] DP0.2 **Mac wave 6: NOT RUN (no drags with the owner at the Mac; spec-covered)** ([run](test-runs/1.0.0-alpha.1-M-26-macwave-6.md)) The strip drags the window (Hub: the empty middle of the strip;
      onboarding: either side of the stepper); double-click zooms.
- [x] DP0.3 **Mac wave 6: PASS** ([run](test-runs/1.0.0-alpha.1-M-26-macwave-6.md)) Inset content panel: 22 pt corners, 10 pt from the right and
      bottom window edges, hairline edge, no footer bar; Settings › About
      shows the version.
- [x] DP0.4 **Mac wave 6: PASS (fonts by eye; Network panel not opened)** ([run](test-runs/1.0.0-alpha.1-M-26-macwave-6.md)) Fonts in WebKit: UI text is Figtree (double-storey `a`), page
      titles sans semibold, banner headlines EB Garamond, Russian or Greek UI
      falls back to Inter, CJK/Arabic to the system fonts. Web Inspector ›
      Network shows no font request leaving the app (only `asset`/`tauri`
      URLs).
- [ ] DP0.5 **Mac wave 6: PARTIAL (appearance only; Timelines and system Reduce Motion not run)** ([run](test-runs/1.0.0-alpha.1-M-26-macwave-6.md)) Motion in WebKit: a dialog (Dictionary › Add word) springs in and
      fades out; a page change crossfades; button press shows a small scale.
      With Reduce Motion on, both become a plain fade. Web Inspector ›
      Timelines: no dropped frames on the dialog open on M-26.
- [ ] DP0.6 **Mac wave 6: PARTIAL (light/dark PASS after a dark heatmap fix; Increase Contrast not run)** ([run](test-runs/1.0.0-alpha.1-M-26-macwave-6.md)) Light, dark and Increase Contrast: the hero banner, sidebar and
      panel read clearly in each.
- [x] DP0.7 **Mac wave 6: PASS (debug build; release gate not launched)** ([run](test-runs/1.0.0-alpha.1-M-26-macwave-6.md)) `#/fork/gallery` opens in the debug build (⌘⇧D panel works as
      before) and not in a release build.

### Design Phase 0b (shared pieces) — queued for the Mac wave

Built 2026-11-03 on `design/foundations-2`. Same debug build and pass as
Design Phase 0, by eye.

- [x] DP0b.1 **Mac wave 6: PASS** ([run](test-runs/1.0.0-alpha.1-M-26-macwave-6.md)) Sidebar: the wordmark (bars + "Wispr Bro") heads the sidebar,
      about 28 pt below the title strip; collapsed, the bars alone, centred;
      the strip above and the wordmark row still drag the window.
- [ ] DP0b.2 **Mac wave 6: PARTIAL (gallery opens; scrolling and VoiceOver not run)** ([run](test-runs/1.0.0-alpha.1-M-26-macwave-6.md)) `#/fork/gallery` in WebKit: chips, radios, option cards (lilac
      fill + purple edge + tick when chosen), setting rows and toast cards
      look as in the Playwright shots in light and dark; Tab/arrows/Space
      work on chips and radios; VoiceOver reads "toggle button, selected"
      on a chip and "radio button, 1 of 3" on an option card.
- [x] DP0b.3 **Mac wave 6: PASS (real toast; Reduce Motion not run)** ([run](test-runs/1.0.0-alpha.1-M-26-macwave-6.md)) A real toast (Dictionary › Export, or Show a toast on the
      gallery) is the dark card with a cream action, bottom right, and
      fades in and out (plain fade with Reduce Motion).
- [ ] DP0b.4 **Mac wave 6: NOT RUN** ([run](test-runs/1.0.0-alpha.1-M-26-macwave-6.md)) In the gallery's dark board under a light window, a menu,
      select and popover open dark.

### Design W5 (lessons and first use) — queued for the Mac wave

Built 2026-11-03 on `design/w5-lessons`. A debug build with an empty profile
(`bun run wb:dev`), onboarding finished; one pass, by eye, light and dark.

- [x] W5.1 **Mac wave 6: PASS** ([run](test-runs/1.0.0-alpha.1-M-26-macwave-6.md)) First-use chooser (opens after onboarding): a white sheet over the
      Hub, the serif question with "try" in italic, seven outlined chips with
      icons, the scene on the colourful wallpaper with the dark caption pill
      under the bar; arrows/Home/End move the choice, Try it now still starts
      the spotlight, Maybe later closes it for good.
- [ ] W5.2 **Mac wave 6: PARTIAL (look PASS; practice ⌥1/⌥O not driven)** ([run](test-runs/1.0.0-alpha.1-M-26-macwave-6.md)) Transforms lesson (Transforms › Try it): white sheet, dashes
      centred at the top, the heading's italic word in purple, three raised
      example cards (the Polish one shows struck and purple words), the
      ticker of ticks scrolls and pauses on hover and with its round button.
      Practice: ⌥1 on the selected draft rewrites it, the bar below shows
      Done · View changes inside a round cut-out; ⌥O shows the dark changes
      card; ⌘Z puts the draft back. How it works: the dark stage plays and
      pauses from its corner button.
- [ ] W5.3 **Mac wave 6: PARTIAL (look PASS; take and quiz not driven)** ([run](test-runs/1.0.0-alpha.1-M-26-macwave-6.md)) Whisper lesson (home banner › Dictate quietly): warm sheet, the
      photo with the 30 cm drawing under it, the quoted sentence in italic
      serif, a real take fills the mail composer (practice mode unchanged);
      the four questions as option cards and mic tiles; the table has a
      column per mic with its drawing and a green Best match band.
- [x] W5.4 **Mac wave 6: FIX → PASS ("Show me →"; footer pinned under the card)** ([run](test-runs/1.0.0-alpha.1-M-26-macwave-6.md)) Sidebar checklist: compact card (no clipping at 720 pt tall),
      ink progress bar, hollow circles that fill with a tick, "Show me →"
      on the next item.
- [ ] W5.5 **Mac wave 6: NOT RUN (system Reduce Motion / Increase Contrast)** ([run](test-runs/1.0.0-alpha.1-M-26-macwave-6.md)) Reduce Motion: sheets and steps only fade, every scene is still,
      no pause buttons; Increase Contrast: chips, option cards and the
      table's edges stay visible. WebKit timeline: no dropped frames when a
      lesson sheet opens on M-26.

### Design W7 (art) — queued for the Mac wave

Built 2026-11-03 on `design/w7-assets` (SYSTEM-PLAN §6, W7). A debug build
with an empty profile is enough (`bun run wb:dev`); one pass, by eye.

- [x] W7.1 **Mac wave 6: PASS** ([run](test-runs/1.0.0-alpha.1-M-26-macwave-6.md)) Banner photos in WebKit: Home, Dictionary, Snippets and Transforms
      show their photo under the dark grade (mic, thesaurus spines, card
      catalogue drawers, typewriter keys); the headline and body stay
      readable; no banding or visible JPEG blocks at 2x on the built-in
      display; light and dark use the same photo.
- [x] W7.2 **Mac wave 6: PASS** ([run](test-runs/1.0.0-alpha.1-M-26-macwave-6.md)) Empty-state characters: an empty History, Dictionary (All and
      Suggested), Snippets, a search with no match and the notifications
      panel each show their character; the ink is near-black in light and
      near-white in dark, and switches with the title-strip theme toggle
      without a reload.
- [x] W7.3 **Mac wave 6: PASS (bundled art; Web Inspector not opened)** ([run](test-runs/1.0.0-alpha.1-M-26-macwave-6.md)) Web Inspector › Network: every picture loads from the app
      (`tauri://`/`asset`), none from the network; no CSP error in the
      console (the characters are inlined SVGs with a `<style>` element;
      `style-src 'unsafe-inline'` allows it).

### Design W4 (the bar and the spotlight) — queued for the Mac wave

Built 2026-11-03 on `design/w4-bar` (docs/fork/design/SYSTEM-PLAN.md §7,
W4). Test identity `com.wisprbro.w4check`, debug build, shortcuts unbound,
takes by SIGUSR2 through the WAV microphone (`WISPRBRO_TEST_MIC_WAV`, WAVs
from `scripts/wb-meter/make-wav.py`); never the owner's app, speakers or
real microphone. Window sizes and timings through
`swift scripts/wb-s03-pill.swift` (`WB_PID=<pid>`); stills with its `shot`.
Reference: Wispr Flow's bar crops (local evidence `02-bar-click-*`,
`03-bar-hover-*`; by eye only). Before/after renders of every web state:
the builder's scratch folder (not in the repo).

- [x] W4.1 **Mac wave 6: PASS (hands-free path; push-to-talk and top position not run)** ([run](test-runs/1.0.0-alpha.1-M-26-macwave-6.md)) Sizes. `-e 'rest'`: the resting window is 54 × 18 pt (the 38 × 6
      line plus its hit area), the line's bottom 8 pt above the Dock. A held
      take (`appear 1 1500`, or SIGUSR2 + `windows` mid-take): the active
      window is 72 × 30 (push to talk) or 100 × 30 (hands-free: `clickbar`),
      300 × 72 while the mic label shows (label 34 tall, 8 pt above the
      capsule), back to 72/100 × 30 after about 2 s. Bar at the top of the
      display (Settings › bar position): the label hangs below the capsule.
- [x] W4.2 **Mac wave 6: PASS** ([run](test-runs/1.0.0-alpha.1-M-26-macwave-6.md)) Look, by eye beside the reference crops: an opaque black capsule
      with no white edge on a light window; in dark mode a faint hairline
      only. Hands-free: ✕ in a dark 18 pt disc on the left, ✓ in a white disc
      with a black check on the right (mirrored in Arabic). Text on every
      state is 15 px. The resting line is the same black, a little
      translucent, with no edge.
- [ ] W4.3 **Mac wave 6: NOT RUN (Timelines)** ([run](test-runs/1.0.0-alpha.1-M-26-macwave-6.md)) Morphs (Web Inspector › Timelines on the `fork-pill` page for
      frame drops; M-26, 60 Hz and 120 Hz): fn down grows the resting line
      into the capsule in one spring with no jump at the hand-over from the
      native bridge to the web view; the mic label blurs in, holds, leaves;
      Transcribing grows out of the take's capsule; the take's end shrinks
      the capsule back into the resting line. A click on the bar and a hover
      grow on the same spring. With Reduce Motion on: sizes change at once,
      the label only fades, the spinner is still.
- [x] W4.4 **Mac wave 6: FIX → PASS (café hum lay at 0.55; loud 0.92 vs quiet 0.60)** ([run](test-runs/1.0.0-alpha.1-M-26-macwave-6.md)) Waveform: a dotted line from fn down until the first audio frame,
      then bars, tallest in the middle, moving with the words. Café WAV
      (`make-wav.py`): flat on the noise, lively on the speech (OT1.2). Peaks:
      two takes, `SPEECH_PEAK=0.15` and `SPEECH_PEAK=0.9`, with `--debug`:
      the `pill: level frames …, loudest bar …` lines of the loud take reach
      higher (≥ 0.95) than the ordinary one (≤ 0.9), and the ordinary take's
      bars no longer all sit at the top.
- [ ] W4.5 **Mac wave 6: NOT RUN (no clicks with the owner at the Mac)** ([run](test-runs/1.0.0-alpha.1-M-26-macwave-6.md)) Clicks: during the ~2 s label the window is wider than the
      capsule; a click beside the capsule (inside that window, outside the
      capsule) is absorbed by the bar's panel, as designed (not passed to the
      app below), and the frontmost app keeps focus (DEC-28); ✕ and ✓ still cancel and finish without activating
      Wispr Bro (S-03's `watchfront` check). A take with the app hidden (⌘H)
      leaves it hidden (S-28.8 re-check).
- [ ] W4.6 **Mac wave 6: NOT RUN (needs a Polish in a target app)** ([run](test-runs/1.0.0-alpha.1-M-26-macwave-6.md)) Card and diff: a card (take into the desktop) and the View
      changes diff (⌥O after a Polish) look like the renders: dark card,
      15 px text, teal inserted chips, struck grey deleted chips, four
      outline actions.
- [x] W4.7 **Mac wave 6: PASS** ([run](test-runs/1.0.0-alpha.1-M-26-macwave-6.md)) Spotlight (S-30 chooser › any card › Try it): the screen dims with
      a round cut-out of about 224 pt centred on the bar, the hint in a black
      card above it; dictating there keeps the label inside the circle.
- [ ] W4.8 **Mac wave 6: NOT RUN** ([run](test-runs/1.0.0-alpha.1-M-26-macwave-6.md)) Scratchpad group on the bar (S-24 on): the two buttons sit on the
      black capsule (not 6 pt below it), hover help still per button.

### Design W6 (Scratchpad and dialogs) — queued for the Mac wave

Built 2026-11-03 on `design/w6-scratch-dialogs`. Debug build, by eye unless
noted; the shots in Playwright are the reference.

- [x] DW6.1 **Mac wave 6: FIX → PASS (tab chip 2.5 pt from the lights; Arabic room moved right; now 90 pt physical)** ([run](test-runs/1.0.0-alpha.1-M-26-macwave-6.md)) Open the note (Hub › Scratchpad › New note, or the shortcut):
      no system title text; the traffic lights sit at the start of the
      44 pt strip, vertically centred, clear of the first tab chip; empty
      strip space drags the window; expand and close work. Repeat with the
      app language set to Arabic (strip mirrored, lights still clear).
- [x] DW6.2 **Mac wave 6: PASS** ([run](test-runs/1.0.0-alpha.1-M-26-macwave-6.md)) Re-run S-24.1 (`scripts/wb-s24/`, WAV microphone): a take started
      from the note keeps focus and the text lands in it with the new chrome.
- [x] DW6.3 **Mac wave 6: PASS (hover toolbar not driven)** ([run](test-runs/1.0.0-alpha.1-M-26-macwave-6.md)) Scratchpad page in light and dark: hero photo, note peek, hover
      toolbar on a Recents card (red delete), Add to the bar switch.
- [ ] DW6.4 **Mac wave 6: PARTIAL (Add word PASS; other dialogs only in Playwright WebKit)** ([run](test-runs/1.0.0-alpha.1-M-26-macwave-6.md)) Dialogs in WebKit: Add word (with a conflict), Snippet, the
      import wizard's column pickers (listbox opens, keyboard works), Report
      a problem, the feature popup, What's new as a sheet; Reduce Motion
      crossfades them.

### Design polish — queued for the Mac wave

Built 2026-11-03 on `design/polish`. Debug build, test identity, takes by
SIGUSR2 through the WAV microphone (as Design W4); by eye unless noted.

- [ ] DP.1 **Mac wave 6: NOT RUN (needs a Polish in TextEdit)** ([run](test-runs/1.0.0-alpha.1-M-26-macwave-6.md)) Diff card height: Polish a one-sentence selection in TextEdit,
      then ⌥O: the card holds its one or two lines with no empty band under
      them; a ten-sentence selection gives the tallest card with the text
      scrolling; a Japanese sentence wraps inside the card (no clipped
      line). `swift scripts/wb-s03-pill.swift windows` shows the window
      height = 104 + 24 × lines.
- [ ] DP.2 **Mac wave 6: NOT RUN (same)** ([run](test-runs/1.0.0-alpha.1-M-26-macwave-6.md)) Chips: deleted and inserted words never touch ("perhaps" and
      "We could" have a gap) on the bar and in the Hub's View changes.
- [x] DP.3 **Mac wave 6: PASS** ([run](test-runs/1.0.0-alpha.1-M-26-macwave-6.md)) Drawn bars: the first-use chooser's scene, the Transforms lesson
      practice and How it works, and the whisper lesson show the same bar as
      the real one (black capsule, 11 bars, the split "Transcribing…"
      capsule, the white View changes pill).
- [ ] DP.4 **Mac wave 6: NOT RUN** ([run](test-runs/1.0.0-alpha.1-M-26-macwave-6.md)) The Transforms editor (Transforms › a card › Edit) opens like the
      Settings modal (scrim, rise) and closes with Esc.

### Design W2 (Hub pages) — queued for the Mac wave

Built 2026-11-03 on `design/w2-hub` (SYSTEM-PLAN §7, W2). A debug build with
a profile that has some history (the S-09/S-15b seeders, or a few WAV
takes through `WISPRBRO_TEST_MIC_WAV`), one pass by eye, light then dark
(title-strip toggle), at the default window size and once at the minimum.

- [x] W2.1 **Mac wave 6: FIX → PASS (sidebar footer pinned)** ([run](test-runs/1.0.0-alpha.1-M-26-macwave-6.md)) Home: sans 24 pt greeting with the pencil appearing on hover;
      photo banner (no example chips); the rail is one tinted card (serif
      numbers, then the voice profile under a hairline with the thin purple
      bar); History is one bordered list per day under uppercase day labels,
      rows hover to the warm tint and show play/copy/flag/⋯ over the row's
      end; Play still opens the player and the menu still opens.
- [x] W2.2 **Mac wave 6: FIX → PASS (dark heatmap contrast; Save as image not run)** ([run](test-runs/1.0.0-alpha.1-M-26-macwave-6.md)) Insights: three tinted stat cards (number, micro label, hairline,
      detail), the teal category bars with the share inside and the name
      after, the teal heatmap and legend; Voice locked shows the Insights
      photo hero with the white progress bar; "Save as image" still writes a
      PNG that looks like the page.
- [x] W2.3 **Mac wave 6: PASS** ([run](test-runs/1.0.0-alpha.1-M-26-macwave-6.md)) Dictionary and Snippets: ghost Import/Export, black Add; the
      search field and the "Sort" dropdown (a menu, not a native popup) work
      by mouse and keyboard; rows in one bordered list with edit/delete
      revealed on hover and on Tab; Suggested shows the auto-learn switch
      row and Accept/Reject always visible.
- [x] W2.4 **Mac wave 6: PASS** ([run](test-runs/1.0.0-alpha.1-M-26-macwave-6.md)) Style: the page is a centred column; each group tab opens with
      the Style photo hero carrying the group's real app icons in frosted
      circles and a "+" (each icon's menu and the "+" picker work; VoiceOver
      reads the app name); three preset cards with the sample bubble, the
      chosen one with a purple edge, arrows move the choice; Cleanup shows
      three option cards (also in Settings › Advanced).
- [x] W2.5 **Mac wave 6: PASS** ([run](test-runs/1.0.0-alpha.1-M-26-macwave-6.md)) Transforms: Beta tag; "Enable Transforms" switch and the ⌥ O
      "View changes" chip in the header; the auto-apply row (dropdown +
      switch) and the shortcut note; Command Mode as a tinted row with its
      switch; serif "My Transforms" with tutorial/reset/create, cards with
      their key caps; the editor and View changes still open.
- [x] W2.6 **Mac wave 6: PASS** ([run](test-runs/1.0.0-alpha.1-M-26-macwave-6.md)) Share: one card with the serif headline, the link field + black
      Copy, Share… (the share sheet still anchors to the button) and the
      character on the right.
- [ ] W2.7 **Mac wave 6: NOT RUN (system settings)** ([run](test-runs/1.0.0-alpha.1-M-26-macwave-6.md)) Increase Contrast and Reduce Motion on: hairlines darken, focus
      rings show, no hover-only control is unreachable by keyboard.

### Design W1 (onboarding) — queued for the Mac wave

Built 2026-11-03 on `design/w1-onboarding` (SYSTEM-PLAN §7 W1). A debug
build with an empty profile shows onboarding (`bun run wb:dev`); one pass
through every step, by eye, light and dark (title-strip theme or System
Settings), once with Reduce Motion on. No take is needed except the
practice (use the WAV microphone, `WISPRBRO_TEST_MIC_WAV`).

- [x] W1.1 **Mac wave 6: PASS** ([run](test-runs/1.0.0-alpha.1-M-26-macwave-6.md)) Shell: the stepper is centred in the title strip between the
      traffic lights and Help, the purple line grows step by step, the
      window drags from the strip, "← Back" is top-left of each stage.
- [ ] W1.2 **Mac wave 6: NOT RUN (clicks through the flow)** ([run](test-runs/1.0.0-alpha.1-M-26-macwave-6.md)) Steps slide in the direction of travel (Back slides the other
      way) without a flash; with Reduce Motion they crossfade.
- [ ] W1.3 **Mac wave 6: PARTIAL (look PASS; live fn/mic not driven)** ([run](test-runs/1.0.0-alpha.1-M-26-macwave-6.md)) Welcome's demo loops and pauses; Permissions shows the pane and
      prompt for the permission being asked; the mic card's bars move
      with the WAV microphone and Yes goes on; the shortcut key turns
      purple while fn is held and Yes goes on.
- [ ] W1.4 **Mac wave 6: PARTIAL (look PASS; typing not driven)** ([run](test-runs/1.0.0-alpha.1-M-26-macwave-6.md)) Typing test: click the paragraph, type; the caret walks the
      paragraph (no visible second copy of the text), wrong letters turn
      red; the language menu opens on the dark stage in the window's
      theme.
- [ ] W1.5 **Mac wave 6: PARTIAL (look PASS; timing and Reduce Motion not run)** ([run](test-runs/1.0.0-alpha.1-M-26-macwave-6.md)) Celebration after a practice take: headline, bars, arrow
      drawing, number rolling to 3.3×, rocket, caption, Continue, one
      confetti burst; smooth at 60 Hz and 120 Hz (Web Inspector timeline:
      no long frames); with Reduce Motion all at once, no confetti.
- [ ] W1.6 **Mac wave 6: PARTIAL (look PASS; slider not driven)** ([run](test-runs/1.0.0-alpha.1-M-26-macwave-6.md)) Time saved: the number rolls; dragging the slider rolls it
      again; Summary's switch and Start dictating finish setup.

### Design W3 (Settings) — queued for the Mac wave

Built 2026-11-03 on `design/w3-settings`. Same debug build as Design Phase
0; by eye in light, dark, Increase Contrast and Reduce Motion, with the
reference frames u-033 and o-0178 open beside it (local only).

- [x] DW3.1 **Mac wave 6: PASS (look; timing not measured)** ([run](test-runs/1.0.0-alpha.1-M-26-macwave-6.md)) Settings (⌘, or the sidebar) opens a 960 × 640 modal that
      springs in (no bounce), over a lighter scrim; with Reduce Motion it
      only fades. Section changes crossfade; the pane starts at the top.
- [x] DW3.2 **Mac wave 6: PASS** ([run](test-runs/1.0.0-alpha.1-M-26-macwave-6.md)) Nav: glyphs, "SETTINGS" and "APP" micro labels, the active
      fill, the version at the bottom; Advanced's chevron turns. RTL
      (Arabic app language): the nav sits on the right, chevrons mirror.
- [x] DW3.3 **Mac wave 6: FIX → PASS (drawn radios)** ([run](test-runs/1.0.0-alpha.1-M-26-macwave-6.md)) Every section: serif title, one intro line, tinted row cards
      with switches/selects at the end; Handy's rows (Launch on Startup,
      Advanced) look like the fork's rows: black switch with a white knob,
      field-style dropdowns, no pink anywhere; History Limit has no spin
      buttons and ↑/↓ still step it.
- [ ] DW3.4 **Mac wave 6: NOT RUN** ([run](test-runs/1.0.0-alpha.1-M-26-macwave-6.md)) Selects open as popovers under the field, Esc closes only the
      list (the modal stays), VoiceOver reads "pop-up button" with the row
      title and the description.
- [ ] DW3.5 **Mac wave 6: PARTIAL (look PASS; recording not driven)** ([run](test-runs/1.0.0-alpha.1-M-26-macwave-6.md)) Shortcuts panel: tinted cards, key caps in white fields,
      pencil/trash, "+" squares; record a new shortcut (field turns purple,
      Esc cancels); Reset to defaults and Done.

### S-14 Settings modal and hotkeys panel

- [x] S-14.1 **Mac wave 1: PASS** ([run](test-runs/1.0.0-alpha.1-M-26-macwave-1.md)) Dock toggle on macOS 26 (activation policy flips).
- [ ] S-14.2 **Mac wave 1: BLOCKED (needs a reboot; owner)** ([run](test-runs/1.0.0-alpha.1-M-26-macwave-1.md)) Launch at login after reboot.
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

**Procedure S-15a.1 (unattended, ~2 min, no speakers):** start the app
with `scripts/wb-s05/app.sh start` (test microphone, shortcuts unbound) and
show the Hub with a second `./handy`, then
`scripts/wb-s15a-takes.sh <pid> <…>/Data <out dir> 10`.
Each take (holding `~/wb-bench/gui.lock`) loads a `say -o` sentence into the
test microphone between two SIGUSR2s, then screenshots the app's window and prints the `daily_stats`
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
(`codesign --force --deep -s -`), run `Contents/MacOS/wispr-bro --start-hidden`
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
cd ../../.. && WB_DEBUG_TEXT_TRACE=1 scripts/wb-s05/app.sh start
(cd src-tauri/target/debug && ./handy)          # second launch shows the window
```

**Procedure (unattended, ~3 min, no speakers: the app plays each take's WAV as its test microphone (`WISPRBRO_TEST_MIC_WAV`, set by `scripts/wb-s05/app.sh start`), never through the speakers).**
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
cd ../../.. && WB_DEBUG_TEXT_TRACE=1 scripts/wb-s05/app.sh start
(cd src-tauri/target/debug && ./handy)          # second launch shows the window
```

**Procedure (unattended, ~6 min, no speakers: the app plays each take's WAV as its test microphone (`WISPRBRO_TEST_MIC_WAV`, set by `scripts/wb-s05/app.sh start`), never through the speakers).**

1. Snippets › Add snippet: `my address`, text on three lines (a person, or
   `swift scripts/wb-s16/ui.swift <pid> 'front; click …; type my address;
key tab; type …; key return; …'` with coordinates from a window
   screenshot, under `~/wb-bench/gui.lock`), then click Add snippet.
2. `scripts/wb-s17/run.sh` (one round): alone and inline into TextEdit,
   the Safari editor page and the Chrome chat page (S-11's `take.sh`;
   snapshots and restores the clipboard). Every line must say PASS. The
   audio is the app's test microphone, never the speakers; repetition is
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
`lsregister -f` it, run `Contents/MacOS/wispr-bro --start-hidden` once and quit,
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
   `WISPRBRO_NOTIFY_DEMO="8000:model_ready,queued_takes_ready,queued_takes_ready,shortcut_problem,update_ready" <app>/Contents/MacOS/wispr-bro --debug --start-hidden`.
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

**Procedure (unattended, ~8 min, no speakers: each phase start sets the
test microphone, `WISPRBRO_TEST_MIC_WAV=Data/test-mic.wav`, unbinds the
profile's shortcuts and clones a model cache when there is none).**
`WB_S19A_MODEL_ID=parakeet-tdt-0.6b-v3 scripts/wb-s19a/run.sh personal work email other autosend fallback`:
per phase it stops the app (main process only), writes the phase's styles
and `app_rules` rows (`scripts/wb-s19a/setup.py`: TextEdit put in the
group under test, auto-send on for the last two), starts the app again
and dictates into new TextEdit documents through S-12a's `take.sh` (one
warm-up take per phase, under `~/wb-bench/gui.lock`, one round of TTS
WAVs through the test microphone). Check the printed document texts (a trailing `|` is the
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
<wav|none> <out dir>` for `textedit` and `chrome` with the WAV (the app's
test microphone, `app.sh start`; never the speakers), then
`setup.sh … off`, restart, `textedit` with the WAV again (S-20.2), then
`card none` and `terminal none` (silent takes: a silent WAV; the read
still runs at record start). Each case holds `~/wb-bench/gui.lock`, opens its own
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

- [ ] S-21.1 **Mac wave 1: BLOCKED (RS-04 apps not installed)** ([run](test-runs/1.0.0-alpha.1-M-26-macwave-1.md)) RS-04 replace rows on the real slice.
- [x] S-21.2 **Mac wave 1: PASS, partial: ⌥1 swallowed and ran; ⌃⌥ alternatives not run** ([run](test-runs/1.0.0-alpha.1-M-26-macwave-1.md)) RS-11 ⌥ characters on the real slice.
- [ ] S-21.3 **Mac wave 1: PASS in TextEdit only (1.1-1.3 s, one ⌘Z); Gmail/Slack/VS Code/Notes not run** ([run](test-runs/1.0.0-alpha.1-M-26-macwave-1.md)) Polish in Gmail, Slack, VS Code, Notes.
- Procedure: TODO (S-21).

### S-22 Transforms page, editor, diff, auto-apply

- [ ] S-22.1 **Mac wave 1: PASS in TextEdit only (Slack not installed)** ([run](test-runs/1.0.0-alpha.1-M-26-macwave-1.md)) ⌥O after a transform in Slack.
- [ ] S-22.2 **Mac wave 1: NOT VERIFIED (fail-open seen; no auto-apply run logged under load)** ([run](test-runs/1.0.0-alpha.1-M-26-macwave-1.md)) Auto-apply on a 3-sentence dictation.
- [x] S-22.3 **Mac wave 3: PASS "never while a take runs" (Hub opened mid-take: no popup for 8 s, also after a page change; it came 1.7 s after the stop)** ([run](test-runs/1.0.0-alpha.1-M-26-macwave-3.md)) **Mac wave 2: PASS (no popup over View changes; it appears on the next page); "never while a take runs" not re-checked** ([run](test-runs/1.0.0-alpha.1-M-26-macwave-2.md)) **Queued for the Mac wave (fix on `fix/wave1-b`, Finding 9):** on a
      fresh test identity (feature popup not yet dismissed), ⌥O after a Polish
      opens the Hub on View changes with no "Your notifications live here"
      popup over it; the popup appears once you leave the diff (another Hub
      page), and never while a take runs.
- Procedure: TODO (S-22).

### S-23 Command Mode

- [ ] S-23.1 **Mac wave 1: BLOCKED (fn+Ctrl only; owner's app holds fn; accounts)** ([run](test-runs/1.0.0-alpha.1-M-26-macwave-1.md)) 20 commands across Mail, Slack, Docs; ⌘Z restores.
- Procedure: TODO (S-23).

### S-24 Scratchpad

- [x] S-24.1 **Mac wave 5: PASS regression after the wave 4 activation guard (note from the Hub, front pid = app at +0.2/+1/+3 s, `own_app`, no `focus_moved`, text in the note)** ([run](test-runs/1.0.0-alpha.1-M-26-macwave-5.md)) **Mac wave 2: PASS bundled and --no-bundle (focus kept at +0.2/+1/+3 s, text in the note); step 3 corrected (S-05 key-up rule)** ([run](test-runs/1.0.0-alpha.1-M-26-macwave-2.md)) **Mac wave 1: FAIL (app loses focus when a take starts from the note)** ([run](test-runs/1.0.0-alpha.1-M-26-macwave-1.md)) Dictate into the note window.
- [x] S-24.2 **Mac wave 2: PASS bundled and --no-bundle (⌥S front in 100-200 ms, typed keys in the note)** ([run](test-runs/1.0.0-alpha.1-M-26-macwave-2.md)) **Mac wave 1: FAIL, fixed on `fix/macwave-s24` (window showed the Hub); note does not take focus** ([run](test-runs/1.0.0-alpha.1-M-26-macwave-1.md)) Open via ⌥S after binding.
- [x] S-24.3 **Mac wave 1: PASS with the `fix/macwave-s24` fix** ([run](test-runs/1.0.0-alpha.1-M-26-macwave-1.md)) The note survives a force-quit.
- **Fixed on `fix/wave1-b` (2026-11-03), queued for the Mac wave:** S-24.1
  (the bar's web view no longer flips the app to the Prohibited activation
  policy; no AX call into our own process) and S-24.2's focus (every open
  path calls `set_focus`). Rerun S-24.1 and S-24.2 on a bundled test
  identity and a `--no-bundle` build:
  1. ⌥S (bound to `scratchpad.open`) with TextEdit in front:
     `lsappinfo front` is the test app within 1 s and typed keys land in the
     note (the note's text changes; TextEdit's does not).
  2. With the note in front and its caret in the text, start a take
     (`kill -USR2 <pid>` or `--toggle-transcription`), play the WAV, stop:
     `lsappinfo front` stays the test app at +0.2, +1 and +3 s after the
     start; the log has `context: read own_app` and no `focus_moved`; the
     note gets the text; the app does not crash.
  3. Start a take in TextEdit, click into the note, stop: the text goes into
     the note (the target is the field focused at key-up, S-05 rule 2:
     "started in Slack and released in Notes is fine"); no `focus_moved`
     card. (Mac wave 2: this step used to say "the card", which contradicts
     S-05; a card is only for focus moving after the stop.)
     Driver: `WBUI=… WBKEYS=… scripts/wb-s24/recheck.sh <exe> <Data> <wav> <out>`
     (steps 1-3, own TextEdit instance, SIGUSR2 takes).
     Builder check (2026-11-03, `--no-bundle` debug build, portable `com.wisprbro.fixbcheck`, no speakers, under `gui.lock`): ⌥S with a TextEdit document in front made the test app frontmost and typed keys landed in the note (notes row 1 `Zz`, TextEdit unchanged); a take started with SIGUSR2 from the note kept the app frontmost at +0.2, +1.0 and +3.0 s (`context: read own_app`, `pill: web view built`), and the app stayed up after the stop. Not checked: the take's text arriving in the note (no speech model was loaded and no WAV path exists), the bundled identity.
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
`Contents/MacOS/wispr-bro --start-hidden` once and quit. In
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

- [x] S-26a.1 **Mac wave 1: PASS** ([run](test-runs/1.0.0-alpha.1-M-26-macwave-1.md)) ⌘C / ⌘V / ⌘Z in Hub text fields.
- [x] S-26a.2 **Mac wave 1: PASS** ([run](test-runs/1.0.0-alpha.1-M-26-macwave-1.md)) The Help search box finds menu items.
- [x] S-26a.3 **Mac wave 1: PASS** ([run](test-runs/1.0.0-alpha.1-M-26-macwave-1.md)) The tray icon comes back after the menu bar process (ControlCenter) restarts (A-69).
- [x] S-26a.0 **Mac wave 1: PASS** ([run](test-runs/1.0.0-alpha.1-M-26-macwave-1.md)) The menu bar has Dictation, Vocabulary and Help with their items; Edit keeps ⌘Z ⌘X ⌘C ⌘V ⌘A.
- [x] S-26a.4 **Mac wave 1: PASS** ([run](test-runs/1.0.0-alpha.1-M-26-macwave-1.md)) Vocabulary's dictionary count follows a word added from Vocabulary › Add word… without reopening the app.

**Status: queued for the Mac wave** (UI wave; not run by the slice agent).

**Profile:** a portable debug build with the slice identity
(`com.wisprbro.s26acheck`, product runtime), `onboarding_completed: true`,
`update_checks_enabled: false`. Build as in S-25 with that identifier
(debug is enough: nothing here is CSP-sensitive), then:

```sh
swiftc -O scripts/wb-s25/ui.swift -o <scratch>/wbui        # Accessibility for the terminal
swiftc -O scripts/wb-s26a/menus.swift -o <scratch>/wbmenus
export WBUI=<scratch>/wbui WBMENUS=<scratch>/wbmenus
```

**Procedure (unattended, ~3 min; each step holds `~/wb-bench/gui.lock`;
keys go only to the test app while it is in front; the clipboard is saved
and restored around S-26a.1).** Launch the bundle executable, note its pid.

1. `scripts/wb-s26a/run.sh menubar <pid> <out>`. Pass (S-26a.0): every
   line PASS; `menubar.txt` shows Dictation ("N words dictated", Copy last
   transcript, Microphone ▸, Languages ▸), Vocabulary (two counts, Add word…,
   New snippet…), Help (Documentation, Shortcuts, Microphone, Languages).
2. Open the Hub's Dictionary page and the search field's window point
   (`wbui <pid> front`; screenshot), then
   `scripts/wb-s26a/run.sh edit <pid> <x> <y> <out>`. Pass (S-26a.1): ⌘X,
   ⌘V twice, ⌘Z (one paste undone) and ⌘C all PASS; the user's clipboard is
   restored.
3. `scripts/wb-s26a/run.sh help <pid> snippet <out>`. Pass (S-26a.2): the
   Help menu opens with its search box, `help-results.txt` lists more than
   the search field, and `help-search.png` shows "New snippet…" found
   (Vocabulary highlighted).
4. `scripts/wb-s26a/run.sh counts <pid> <out>`. Pass (S-26a.4): the
   dictionary count is one higher right after Save.
5. `scripts/wb-s26a/run.sh tray <pid> <out>` (restarts ControlCenter: the
   menu bar blinks for a second). Pass (S-26a.3): the status item is back
   within 30 s, `menu-bar.png` shows the glyph.
6. By eye in the same run: click the tray glyph: Open Wispr Bro, Check for
   updates…, Paste last transcript (greyed on an empty profile), Shortcuts,
   Microphone ▸, Languages ▸, Documentation, Quit; no Model ▸ or Unload.
7. Quit; delete the bundle copy and
   `~/Library/{WebKit,Caches}/com.wisprbro.s26acheck`.

### S-26b Help popover, support, Share, About

- [x] S-26b.0 **Mac wave 2: PASS (Help menu; tray by eye not checked)** ([run](test-runs/1.0.0-alpha.1-M-26-macwave-2.md)) Help has Report a problem… [⌘/] and Send feedback…; the tray
      has both after Documentation.
- [ ] S-26b.1 **Mac wave 2: NOT RUN (opens the owner's default browser; owner)** ([run](test-runs/1.0.0-alpha.1-M-26-macwave-2.md)) ⌘/ opens a prefilled issue in the browser.
- [x] S-26b.2 **Mac wave 2: PASS (screenshot; the AX probe cannot see the macOS 26 picker)** ([run](test-runs/1.0.0-alpha.1-M-26-macwave-2.md)) The share sheet shows.

**Status: queued for the Mac wave** (UI wave, with S-26a's run; not run by
the slice agent).

**Profile:** as S-26a, with the slice identity `com.wisprbro.s26bcheck`
(debug build, product runtime, `onboarding_completed: true`,
`update_checks_enabled: false`). Nothing is sent to GitHub: the issue form
is only opened, never submitted. Compile the drivers once:

```sh
swiftc -O scripts/wb-s25/ui.swift -o <scratch>/wbui        # Accessibility for the terminal
swiftc -O scripts/wb-s26a/menus.swift -o <scratch>/wbmenus
swiftc -O scripts/wb-s26b/share.swift -o <scratch>/wbshare
export WBUI=<scratch>/wbui WBMENUS=<scratch>/wbmenus WBSHARE=<scratch>/wbshare
```

**Procedure (unattended, ~1 min; each step holds `~/wb-bench/gui.lock`;
keys go only to the test app while it is in front).** Launch the bundle
executable, note its pid and its log file
(`<portable Data>/logs/*.log` or `~/Library/Logs/com.wisprbro.s26bcheck/`).

1. `scripts/wb-s26b/run.sh menus <pid> <out>`. Pass (S-26b.0): both lines
   PASS. By eye: the tray shows Documentation, Report a problem… (⌘/),
   Send feedback…, then Quit.
2. `scripts/wb-s26b/run.sh issue <pid> <log> <out>`. Pass (S-26b.1): a
   browser comes to the front, the log has "help: opened the Problem form
   on GitHub", and the page (URL in `browser.txt` for Safari, else
   `issue.png`) is `github.com/fjenn/wispr-bro/issues/new` with the title
   "Problem: " and an "About this Mac" block of versions only (no
   dictation, no path). Close the tab without submitting. While the repo is
   still empty (CONTEXT "GitHub"), GitHub shows its sign-in or empty-repo
   page instead: then only the URL in the address bar is checked.
3. Open the Hub's Share with a friend page, note the Share… button's window
   point (`wbui <pid> front`; screenshot), then
   `scripts/wb-s26b/run.sh share <pid> <x> <y> <out>`. Pass (S-26b.2): the
   picker lists services (e.g. Mail, Messages, AirDrop) in `picker.txt` and
   `share-sheet.png` shows it next to the button; Escape closes it and
   nothing is sent.
4. Quit; delete the bundle copy and
   `~/Library/{WebKit,Caches}/com.wisprbro.s26bcheck`.

### S-27 Data and Privacy, export, delete all, diagnostics

- [x] S-27.1 **Mac wave 1: PASS** ([run](test-runs/1.0.0-alpha.1-M-26-macwave-1.md)) Export opens in Finder. _(queued for the Mac wave)_
- [x] S-27.2 **Mac wave 2: PASS re-check (window shown on onboarding after --start-hidden)** ([run](test-runs/1.0.0-alpha.1-M-26-macwave-2.md)) **Mac wave 1: PASS** ([run](test-runs/1.0.0-alpha.1-M-26-macwave-1.md)) Erase, then relaunch shows onboarding. _(queued for the Mac wave)_
- [x] S-27.3 **Mac wave 1: paths recorded** ([run](test-runs/1.0.0-alpha.1-M-26-macwave-1.md)) `brew uninstall --zap` paths match (S-39). _(queued for the Mac
      wave: this slice records the path list; S-39 compares it with the
      cask's `zap` stanza)_
- [x] S-27.4 **Mac wave 1: PASS** ([run](test-runs/1.0.0-alpha.1-M-26-macwave-1.md)) Copy and Save diagnostics are sanitized (seeded marker phrase,
      home path) and hold no databases, audio or settings. _(queued)_
- [x] S-27.5 **Mac wave 2: PASS re-check (marker gone from both copies; structured copy keeps bundleID, exception, frames)** ([run](test-runs/1.0.0-alpha.1-M-26-macwave-2.md)) **Mac wave 1: FAIL (`.ips` copy keeps a marker line)** ([run](test-runs/1.0.0-alpha.1-M-26-macwave-1.md)) Every "Show in Finder" opens its folder; a recent macOS `.ips`
      for the test identity is picked up as a crash file and shown once in
      the inbox. **Fixed on `fix/wave1-a`** (the copy keeps only allow-listed
      crash fields, `fork/diagnostics/ips.rs`); re-check _(queued for the
      next Mac wave: step 4 with a body line and a JSON free-text field both
      holding the marker)_

**Profile: an isolated debug build** with its own identity ("Wispr Bro S27
Check", `com.wisprbro.s27check`; the product runtime, its own app data,
WebKit storage and TCC rows), so the owner's app and data are never
touched. Debug is enough (no CSP-sensitive check). No audio: History is
seeded. Two layouts, both under the test identity:

- **Portable** (S-27.1, .2, .4, .5): copy the `.app` to a scratch folder
  whose path contains `s27`, add `Contents/MacOS/portable` containing `Handy Portable Mode` (an empty marker is ignored on a first launch), re-sign ad hoc,
  as in S-25. `Data` = `Contents/MacOS/Data`.
- **Installed** (S-27.3): the same `.app` without the `portable` marker, so
  it uses `~/Library/Application Support/com.wisprbro.s27check` and
  `~/Library/Logs/com.wisprbro.s27check`.

```sh
echo '{"identifier":"com.wisprbro.s27check","productName":"Wispr Bro S27 Check","plugins":{"updater":{"active":false}}}' > <scratch>/s27.conf.json
bun run tauri build --debug --bundles app --config src-tauri/tauri.fork.conf.json --config <scratch>/s27.conf.json
swiftc -O scripts/wb-s25/ui.swift -o <scratch>/wbui        # S-25's driver; the terminal needs Accessibility
swiftc -O scripts/wb-window-id.swift -o <scratch>/wbwin
```

Run `Contents/MacOS/wispr-bro --start-hidden` once and quit, set
`onboarding_completed: true` and `update_checks_enabled: false` as in S-25,
then `python3 -I scripts/wb-s27/seed.py <…>/Data <scratch>/s27-seed.json`
(three neutral rows with silent WAVs, two fake model files, a log line and a
crash file carrying the marker phrase).

**Procedure (unattended, ~5 min; hold `~/wb-bench/gui.lock` for each GUI
step; events go only to the test app's window via `wbui`).**

1. Launch, open Settings › Data and Privacy, screenshot. Pass: the page
   names huggingface.co and github.com, the four paths are inside `Data/`
   (portable), each "Show in Finder" opens a Finder window on that folder
   (S-27.5; close them).
2. "Export my data"; in the save panel `wbui <pid> 'key cmd-shift-g; key
cmd-a; keys <scratch>/out/; key return; key return'`. Pass (S-27.1): a
   Finder window selects the zip; `python3 -I scripts/wb-s27/verify.py
export <zip> <scratch>/s27-seed.json` passes.
3. "Save diagnostics…" into `<scratch>/out/`, then "Copy diagnostics" and
   `pbpaste > <scratch>/out/copy.txt`. Pass (S-27.4): `verify.py
diagnostics <zip> <seed>` passes; `copy.txt` contains neither the marker
   nor `$HOME/`.
4. Quit. Copy a real `.ips` header for the identity into
   `~/Library/Logs/DiagnosticReports/Wispr Bro S27 Check-s27.ips`
   (`{"bundleID":"com.wisprbro.s27check"}` then any body line containing the
   marker; for the re-check also a real report's body with the marker in
   `asi` and a thread name, e.g. from `kill -ABRT` of the test app);
   relaunch. Pass (S-27.5): the inbox shows one crash notice; the log
   folder has a `crash-…-ips-…txt` without the marker that still names
   `bundleID`, the exception type and the crashed thread's frames. Delete
   the `.ips`.
5. "Delete all my data", leave "Also delete downloaded models" off, type
   `DELETE`, confirm. Pass (S-27.2): the app quits and relaunches into
   onboarding with its window shown even when it was started with
   `--start-hidden` (screenshot; Mac wave 1 finding 8, fixed on
   `fix/wave1-a`, re-check queued for the next Mac wave); then `verify.py erased <Data> <seed> kept` (seeded
   rows, WAVs, log and crash marker gone, fake models kept) and `verify.py
onboarding <Data>` pass. ("Nothing but models left" is the `fork::erase`
   unit test; the relaunched app recreates empty stores at once.)
6. Repeat step 5 with the models box checked: `verify.py erased <Data>
<seed> deleted` passes (re-seed first).
7. Installed layout: launch once without `portable`, quit, then
   `sh scripts/wb-s27/zap-paths.sh com.wisprbro.s27check` and paste the list
   into the run report (S-27.3 input for S-39).
8. Clean up: delete the bundle copy, `~/Library/{Application Support,Logs,WebKit,Caches}/com.wisprbro.s27check`
   and any `~/Library/Preferences/com.wisprbro.s27check.plist`.

#### Mac wave 2 fixes (`fix/wave2`): crash relaunch

- [x] S-27.6 **Mac wave 3: PASS (3 relaunches after `kill -ABRT`: ready in 1 s, no window, no restorer/alert frames, no savedState with the Hub open)** ([run](test-runs/1.0.0-alpha.1-M-26-macwave-3.md)) **Queued for the Mac wave** (finding 1: a relaunch after a crash hung behind AppKit's "reopen windows?" alert). Debug test bundle (any `com.wisprbro.<x>check`, keys unbound with `wb_unbind_keys`): launch with `--start-hidden`, wait for `fork ready`, `kill -ABRT` it ≈ 15 s after launch (as S-27.5 did), then launch it again with `--start-hidden` three times in a row. Pass: each launch logs `fork ready` and `startup: ready` within 20 s, no window of the app is on screen (`scripts/wb-window-id.swift <pid>` finds none), `sample <pid> 1` never shows `NSPersistentUIRestorer`/`NSAlert runModal`, and the log has no `window restoration is still on`. Also: no `~/Library/Saved Application State/<id>.savedState` is written while the Hub is open (the Hub is not restorable). Clean up the `.ips` reports as S-27.5 does.

### S-28 Updates that never interrupt

- [ ] S-28.1 RS-12 on the real slice.
- [ ] S-28.2 An update during a long hands-free take waits.
- Procedure: TODO (S-28).

### S-28 Updates that never interrupt

All queued for the Mac wave (backend, update harness; runs right after
F-02.3 on the same two builds, so F-02.1's one-time codesign Always Allow is
needed first).

- [ ] S-28.0 **Mac wave 2: not needed: the drivers launch from the terminal** ([run](test-runs/1.0.0-alpha.1-M-26-macwave-2.md)) Once: grant Microphone to "Wispr Bro F02 Check"
      (`com.wisprbro.f02check`) and Automation for the terminal → System
      Events (the script hides the app). No speakers: the take records
      silence.
- [ ] S-28.1-.4 **Mac wave 2: 14/17 PASS; FAIL S-28.1d-f: relaunched shown although the app was hidden** ([run](test-runs/1.0.0-alpha.1-M-26-macwave-2.md)) `scripts/wb-f02/update-e2e.sh <out>` (or
      `F02_ONLY_S28=1 scripts/wb-f02/update-e2e.sh <out>` to skip F-02.3).
      Expect PASS for:
      S-28.1a B downloads and verifies in the background (no selftest);
      .1b nothing installs during a 55 s hands-free take that outlasts the
      40 s quiet window ("update during a long hands-free take waits");
      .1c B installs after the take and the quiet window; .1d/.1e the app
      relaunches hidden because the Hub was hidden, and the new process logs
      it; .1f no Hub window after the relaunch; .1g the bundle is B.
      S-28.2a-.2d B ready, quit: installs at quit, no relaunch.
      S-28.3a-.3c a tampered archive fails three times, `update-state.json`
      records the 24 h pause, the bundle stays A.
      S-28.4a-.4c with A's folder read-only nothing installs and no password
      prompt appears ("waiting for Restart now" in the log).
      Evidence: `<out>/s28-*.log`, `<out>/update-lines.log`.
- [ ] S-28.5 **Mac wave 2: NOT RUN (by eye; owner)** ([run](test-runs/1.0.0-alpha.1-M-26-macwave-2.md)) Footer and tray by eye in the same run (consolidated UI wave):
      while S-28.2's app has B ready, open the Hub: the footer reads
      "Update ready · Restart now" next to the version; the tray item reads
      "Restart to update to B"; turn update checks off in Settings › Data and
      Privacy: the footer reads "Update checks off · Turn on" and the tray
      "Update checks are off…" (opens Data and Privacy).
- [ ] S-28.7 **Mac wave 3: FAIL S-28.1d-f again (14/17); cause found: the take itself unhides the hidden app and shows its Hub, so the relaunch is correctly "shown"; without a take the relaunch is hidden (PASS)** ([run](test-runs/1.0.0-alpha.1-M-26-macwave-3.md)) **Queued for the Mac wave** (Mac wave 2 fixes, finding 2): re-run `F02_ONLY_S28=1 scripts/wb-f02/update-e2e.sh <out>`. The relaunch decision now reads AppKit (`NSApp.isHidden`, the Hub's occlusion state) as well as Tauri's window state and the parked Hub (`fork::update::scheduler::relaunch_hidden`). Expect S-28.1d-.1f PASS: `update: relaunching (hidden)`, `relaunched after an update (hidden)`, no Hub window. The profile now starts with every shortcut unbound and a cloned model cache (no download), and the take plays a silent WAV through the test microphone (no Microphone grant needed). Extra by hand if time allows: the Hub minimised (⌘M) or behind another app's window when B installs → also relaunches hidden; the Hub in front → relaunches shown.
- [x] S-28.8 **Mac wave 5: PASS 19/19 (1h/1i hidden with no Hub during the take, then 1d-f `relaunching (hidden)`, no Hub after); extra: take, 16 s (web view destroyed), ⌘H, take → hidden, no Hub at +1/+3/+5/+8 s and after the stop** ([run](test-runs/1.0.0-alpha.1-M-26-macwave-5.md)) **Mac wave 4: FAIL 14/19 (1d-1i): the take still unhides the hidden app; isolated: a take while the per-take web view already exists stays hidden (the `canHide` fix works), the unhide comes with building the `fork-pill` web view at take start** ([run](test-runs/1.0.0-alpha.1-M-26-macwave-4.md)) **Queued for the Mac wave** (Mac wave 4 fixes, finding 1):
      re-run `F02_ONLY_S28=1 scripts/wb-f02/update-e2e.sh <out>`. Cause of
      the wave 4 FAIL: wry calls `NSApp.activate()` after every web view
      build, which unhides a hidden app ~0.5 s later
      (`scripts/wb-f02/unhide-repro.swift` shows it off screen). The
      `fork-pill` and `fork-spotlight` builds now run with app activation
      switched off (`fork/pill/activation.rs`); `canHide` stays off on all
      three panels. Expect 19/19 PASS: S-28.1h (5 s into the take System
      Events still reports the app not visible), .1i (no Hub window during
      the take), then .1d-.1f (`relaunching (hidden)`, no Hub window). The
      take must be the first one after ⌘H with no `fork-pill` web view
      alive (the driver's take is; in the log
      `pill: web view built` follows `Received SIGUSR2`). Extra: wait > 10 s after a take (the
      web view is destroyed), ⌘H, SIGUSR2 again → still hidden, no Hub. By
      hand if time allows: ⌘H the app, dictate into TextEdit: the bar shows
      and pastes, the Hub stays hidden.
- [x] S-28.9 **Mac wave 5: PASS edge re-check (updated profile, Hub opened 2 s and 25 s into a SIGUSR2 take: no What's New in 10 s of screenshots/AX; it opens 2.2 s after the stop, and 8.3 s in the 25 s case: the gate also waits for `update.busy`, sampled by the 10 s tick)** ([run](test-runs/1.0.0-alpha.1-M-26-macwave-5.md)) **Mac wave 4: PASS fresh install (chooser alone, no What's New, also after Maybe later; key = running version, also from an empty key) and updated profile (once, then never again); FAIL edge: the Hub opened within ~10 s of a take's start shows What's New during the take (the gate's `busy` is sampled every 10 s); "panel open first" not verified (the driver could not open the panel)** ([run](test-runs/1.0.0-alpha.1-M-26-macwave-4.md)) **Queued for the Mac wave** (Mac wave 3 fixes, finding 4):
      What's New on a fresh install. A fresh test profile (debug bundle,
      `wb_unbind_keys` + `wb_clone_models`), onboarding finished: on the
      first Hub open the first-use chooser shows alone (no "What's new in
      …" dialog over or after it, also after "Maybe later"), and Handy's
      `settings_store.json` then has `whats_new_last_seen_version` = the
      running version. Then an updated profile (that key set to `0.9.7`, the
      chooser already answered): the Hub opens with What's New once; with
      the notifications panel open first it waits until the panel closes.
      `scripts/wb-s37/run.sh hub` needs no `press-if Got it` for What's New
      on a fresh copy any more (it stays harmless).
      **Wave 4 re-check (Mac wave 4 fixes, finding 2):** the gate now reads
      the take itself (`recording-state-event` plus Handy's `is_recording`
      when the Hub mounts), not only `busy`. On the updated profile (key
      `0.9.7`), Hub hidden: start a take (SIGUSR2, the silent test
      microphone; the log must say `test microphone plays`), open the Hub 2 s
      later (second launch). Expect no "What's new in …" in screenshots
      every second for the next 10 s of the take; after the stop it opens
      within ~2 s. Same with the Hub opened 25 s into the take (wave 4 PASS
      case). Wait until no copy of the identity runs before each launch.
- [ ] S-28.6 RS-12 on real GitHub releases (release wave, with F-02.4):
      alpha.N → alpha.N+1 installs by itself after 20 idle minutes, the app
      comes back to the bar without a window, Microphone and Accessibility
      still granted; What's New for N+1 shows once when the Hub is next
      opened and not while dictating. Exit criterion (R5): no install
      mid-take during the 2-day beta test.

### S-29a Onboarding: welcome, permissions, the two tests

All items **queued for the Mac wave** (consolidated UI run; nothing here has
run on the Mac yet). Playwright covers every screen, branch, resume, preview,
axe light/dark, RTL and keyboard-only (`tests/fork/onboarding.spec.ts`).

- [ ] S-29a.1 **Mac wave 1: BLOCKED (TCC prompts; owner)** ([run](test-runs/1.0.0-alpha.1-M-26-macwave-1.md)) Clean account (no TCC rows for the test identity): permissions to shortcut test in ≤ 2 min (`run.sh timing` from the log's `onboarding: step` lines).
- [ ] S-29a.2 **Mac wave 1: BLOCKED (owner)** ([run](test-runs/1.0.0-alpha.1-M-26-macwave-1.md)) Each diagnosis branch reachable on the real Mac: tap not live (launch before granting Accessibility; after the grant, Try again lights the key **without a relaunch**), Secure Input (a password field in Safari focused), no fn key (switch → ⌃⌥ lights), Globe on Emoji (consent → `AppleFnUsageType` 0 and the marker written; switching to ⌃⌥ restores the old value and drops the marker).
- [ ] S-29a.3 **Mac wave 1: BLOCKED (owner's app holds fn)** ([run](test-runs/1.0.0-alpha.1-M-26-macwave-1.md)) Shortcut test: a synthetic fn press lights the key and records no take (`run.sh keytest`).
- [x] S-29a.4 **Mac wave 1: PASS** ([run](test-runs/1.0.0-alpha.1-M-26-macwave-1.md)) Resume after relaunch at each step (`run.sh resume`), no step written on resume.
- [x] S-29a.5 **Mac wave 2: PASS on release (rename from ~/Downloads, copy from a disk image, both continue at Languages); name clash not run** ([run](test-runs/1.0.0-alpha.1-M-26-macwave-2.md)) **Mac wave 1: BLOCKED on a debug build (relaunch panics in the debug bindings export); rerun on release** ([run](test-runs/1.0.0-alpha.1-M-26-macwave-1.md)) Move to Applications from `~/Downloads` (rename) and from a mounted disk image (copy): the app reopens from Applications and continues at Languages; a name clash shows the Finder message.
- [ ] S-29a.6 **Mac wave 1: partial; by eye: owner** ([run](test-runs/1.0.0-alpha.1-M-26-macwave-1.md)) By eye: the permission pictures match macOS 26 (and 15 if a second Mac is available), light and dark; no screen names a model; the speech download never blocks a step.
- [x] S-29a.7 **Mac wave 3: PASS (no login item at first launch or after quitting at Welcome; checked → registered at completion; unchecked → none)** ([run](test-runs/1.0.0-alpha.1-M-26-macwave-3.md)) **Queued for the Mac wave** (Mac wave 2 fixes, finding 3): a fresh, non-portable **release** install (as S-29a.5: from a disk image to `/Applications`, empty `~/Library/Application Support/<id>`). Pass: no `Registered login item` line and no "Login Item Added" notification during the first launch and the whole of onboarding up to the Summary; quitting at Welcome leaves no login item (System Settings › General › Login Items); finishing with "Open Wispr Bro when you log in" checked logs `Registered login item via SMAppService` right after `onboarding: completed`; unchecked, none. Remove the item through Settings › System afterwards.

**Setup.** Debug build with the test identity (CONTEXT, execution
protocol): `echo '{"identifier":"com.wisprbro.s29acheck","productName":"Wispr Bro S29a Check","plugins":{"updater":{"active":false}}}' > <scratch>/s29a.conf.json`,
`bun run tauri build --debug --bundles app --config src-tauri/tauri.fork.conf.json --config <scratch>/s29a.conf.json`;
copy the `.app` to a scratch folder whose path contains `s29a`, add
`Contents/MacOS/portable` containing `Handy Portable Mode`, sign ad hoc, launch once and quit.
`swiftc -O scripts/wb-s01-keys.swift -o <scratch>/wbkeys` (the terminal
needs Accessibility for posting events). Record `run.sh globe-state` first
and put the owner's Globe setting back at the end if any case changed it.

**Procedure** (`APP=<bundle> OUT=<dir> KEYS=<scratch>/wbkeys scripts/wb-s29a/run.sh <case>`; holds `~/wb-bench/gui.lock`; no audio):

1. `resume` (S-29a.4), `keytest` (S-29a.3).
2. `python3 -I scripts/wb-s29a/profile.py <…>/Data fresh`, `tccutil reset All com.wisprbro.s29acheck`, launch, go through Welcome → Languages → Permissions (grant both in the real prompts) → mic test → shortcut test → Finish; then `timing` (S-29a.1).
3. Diagnoses (S-29a.2): `profile.py … shortcut_test`, `tccutil reset Accessibility com.wisprbro.s29acheck`, launch, No → "cannot see key presses", grant, Try again, press fn. Then Globe: set "Press 🌐 key to" Show Emoji, `globe-state`, No → Globe → Set to Do Nothing, `globe-state`; Edit shortcut → Ctrl+Option (or the no-fn branch), `globe-state` (restored). Secure Input with a focused password field.
4. `move` (S-29a.5) twice: from `~/Downloads`, and from a `hdiutil create`/`attach` image holding the bundle; clean up `/Applications/Wispr Bro S29a Check.app`.
5. Quit; delete the bundle copies and `~/Library/{WebKit,Caches}/com.wisprbro.s29acheck`.

### S-29b Onboarding: languages, practice, celebration, typing test, summary

Items S-29b.2-.7 are **queued for the Mac wave** (consolidated UI run;
nothing here has run on the Mac yet). Playwright covers every screen and
branch: practice text, a slow starter (records, progress, fills late,
moves on at 90 s), nothing heard, cancel, the ⌃⌥ copy, the celebration with
and without reduced motion, the counter from 0, wpm math, early finish,
paste blocked, languages, Try again, Recalculate, the slider, every skip,
the summary and launch at login, Back, no model names, keyboard-only, RTL,
axe light/dark (`tests/fork/onboarding-practice.spec.ts`).

- [ ] S-29b.1 10 people on clean Macs (8 GB and 16 GB): median ≤ 5 min, first dictation within 2 min of install (§5.3), 0 testers stuck (> 15 min or outside help). (R5's "one Intel" is skipped under DEC-29.) Evidence: one row per tester, no names (tester ids only); `run.sh timing` per tester.
- [x] S-29b.2 **Mac wave 2: PASS (SIGUSR2 instead of fn: the owner's app holds fn)** ([run](test-runs/1.0.0-alpha.1-M-26-macwave-2.md)) Practice with a ready model: fn+Space, the WAV, fn → the composer fills, nothing is pasted, the clipboard is unchanged, History has the take with source `practice` and the Home stats do not count it (`run.sh practice`).
- [x] S-29b.3 **Mac wave 2: PASS (queued, moved on at 90 s, take in History)** ([run](test-runs/1.0.0-alpha.1-M-26-macwave-2.md)) Practice during the first download (stand-in at 1 Mbit/s): "Setting up… N%" climbs, then the composer fills or the screen moves on after 90 s; the take is in History either way (`run.sh queued`).
- [x] S-29b.4 **Mac wave 2: PASS** ([run](test-runs/1.0.0-alpha.1-M-26-macwave-2.md)) A take with another app in front during Practice is an ordinary dictation (`run.sh outside`).
- [ ] S-29b.5 **Mac wave 2: NOT RUN (by eye; owner)** ([run](test-runs/1.0.0-alpha.1-M-26-macwave-2.md)) By eye: the celebration is dark in light mode, confetti once (none with Reduce motion on in System Settings › Accessibility › Display); the counter counts up from 0.
- [ ] S-29b.6 **Mac wave 2: NOT RUN (by hand; owner)** ([run](test-runs/1.0.0-alpha.1-M-26-macwave-2.md)) Typing test by hand in WebKit: typing works (the window is `select-none`; the fields opt back in), paste is ignored, the number is plausible against a known typist, Recalculate updates the estimate; the slider by keyboard.
- [x] S-29b.7 **Mac wave 2: PASS (SMAppService login item registered; unchecking removes it)** ([run](test-runs/1.0.0-alpha.1-M-26-macwave-2.md)) Start dictating with "Open Wispr Bro when you log in" checked → the login item exists (`run.sh login`), the resting bar appears (DEC-38); unchecked → none.

**Setup.** As S-29a with `s29b` in place of `s29a` (identifier
`com.wisprbro.s29bcheck`, product name "Wispr Bro S29b Check", a scratch
path containing `s29b`), plus `say -o <scratch>/practice.wav --data-format=LEI16@16000 "Hi Sam, thanks for the notes. Can we meet on Thursday at ten?"`
and, for `queued`, `python3 -I scripts/wb-s07a/hf-mirror.py --root ~/wb-bench/asr --port 18077 --rate 125000`
with `HF_ENDPOINT=http://127.0.0.1:18077` exported for the app.

**Procedure** (`APP=<bundle> OUT=<dir> KEYS=<scratch>/wbkeys WAV=<scratch>/practice.wav scripts/wb-s29b/run.sh <case>`; holds `~/wb-bench/gui.lock`; since the Mac wave 2 fixes the app plays the WAV as its test microphone (`WISPRBRO_TEST_MIC_WAV`, set by the script), never through the speakers, and the profile's shortcuts are unbound unless a fn take needs them):

1. `practice` (S-29b.2), `outside` (S-29b.4), then `queued` (S-29b.3).
2. `python3 -I scripts/wb-s29b/profile.py <…>/Data fresh`, launch, go through every screen to Start dictating (S-29b.5-.7), then `timing` and `login` (the login item is an SMAppService item, read from the app log's `Registered login item`, not a `~/Library/LaunchAgents` plist). With another Wispr Bro running that holds fn, set `TAKE=signal` and `WBUI=<compiled scripts/wb-s25/ui.swift>` (activates the app before a practice take; takes start only with the expected app in front).
3. Quit; remove the login item if it was left on; delete the bundle copies and `~/Library/{WebKit,Caches}/com.wisprbro.s29bcheck`.

### S-30 First-use chooser, pill spotlight, setup checklist

Items S-30.1-.6 are **queued for the Mac wave** (consolidated UI run;
nothing here has run on the Mac yet). Playwright covers the chooser (seven
options, a scene and a hint each, keyboard-only, Esc/Maybe later, reduced
motion, RTL), the own-apps banner (spotlight on/off, again, other, no bar,
fallback wording), the checklist (state and event ticks, the backend's
`try_in_app`, coach marks, chaining, Open at login, hide, all done) and the
`fork-spotlight` page (hint above/below the cut-out, every choice, inactive,
RTL), with axe light/dark (`tests/fork/first-use.spec.ts`); Rust covers the
dismissal rules and the cut-out layout (`fork::pill::spotlight`).

- [x] S-30.1 **Mac wave 2: PASS** ([run](test-runs/1.0.0-alpha.1-M-26-macwave-2.md)) The spotlight is one window of ours at layer 24 covering the bar's display, with the resting bar (layer 25) lit in its cut-out (`run.sh panel`).
- [x] S-30.2 **Mac wave 2: PASS** ([run](test-runs/1.0.0-alpha.1-M-26-macwave-2.md)) Click-through: with the dim on, a click and typed keys reach TextEdit; Wispr Bro never becomes frontmost (`run.sh through`, check `through.png`).
- [x] S-30.3 **Mac wave 2: PASS** ([run](test-runs/1.0.0-alpha.1-M-26-macwave-2.md)) Over a full-screen app: the dim stays on screen over TextEdit in full screen (`run.sh fullscreen`).
- [x] S-30.4 **Mac wave 2: PASS** ([run](test-runs/1.0.0-alpha.1-M-26-macwave-2.md)) Esc ends it and destroys the window (`run.sh esc`); a take into our own Scratchpad does not (`run.sh own`).
- [x] S-30.5 **Mac wave 2: PASS with TextEdit for Mail (owner's Mail never used)** ([run](test-runs/1.0.0-alpha.1-M-26-macwave-2.md)) The first dictation in Mail ends it and ticks "Try it in another app" (`run.sh mail`; a new message, never sent, no screenshot of Mail).
- [ ] S-30.6 **Mac wave 3: PARTIAL scripted: chooser over the Hub, scene frames change, Try it now → layer-24 dim with the AI chat hint, Esc ends it, Add a word ticks (1 of 6); Reduce Motion, coach marks and chaining by eye (owner)** ([run](test-runs/1.0.0-alpha.1-M-26-macwave-3.md)) **Mac wave 2: PARTIAL: chooser opens with the Hub; the rest by eye (owner)** ([run](test-runs/1.0.0-alpha.1-M-26-macwave-2.md)) By eye, from a fresh `profile.py` without a choice: the chooser opens with the Hub, the scenes play (and stand still with Reduce motion), Try it now dims the screen with the matching hint, the checklist card ticks Add a word after adding one on the Dictionary page, its coach mark points at the button, and the next step is shown by itself (Snippets with its coach mark; since S-32 chaining only shows the next item and never runs it).

**Setup.** As S-29a with `s30` in place of `s29a` (identifier
`com.wisprbro.s30check`, product name "Wispr Bro S30 Check", a scratch path
containing `s30`), launched once so the profile exists; `swiftc -O
scripts/wb-s03-pill.swift -o <scratch>/wbpill`, `swiftc -O
scripts/wb-s01-keys.swift -o <scratch>/wbkeys`, and `say -o
<scratch>/s30.wav --data-format=LEI16@16000 "Hi Priya, thanks for the
notes."`.

**Procedure** (`APP=<bundle> OUT=<dir> PILL=<scratch>/wbpill KEYS=<scratch>/wbkeys WAV=<scratch>/s30.wav scripts/wb-s30/run.sh <case>`; holds `~/wb-bench/gui.lock`; each case relaunches with `WISPRBRO_S30_SPOTLIGHT`, which only test identities honour):

1. `panel`, `through`, `fullscreen`, `esc` (S-30.1-.4).
2. `own` (the driver binds `scratchpad.open` to ⌥S for this case only, every other shortcut unbound; it opens the note from its own TextEdit), then `mail` (S-30.4-.5; the app plays the WAV as its test microphone, `WISPRBRO_TEST_MIC_WAV`, never the speakers). `textedit` is the same check in a TextEdit document of the driver's own instance, for a Mac whose Mail holds a real account. With another Wispr Bro running that holds fn (the owner's), set `TAKE=signal` (SIGUSR2 takes) and `WBWIN=<compiled scripts/wb-window-id.swift>` (for `through`).
3. `python3 -I scripts/wb-s30/profile.py <…>/Data`, launch normally, S-30.6 by eye.
4. Quit; delete the bundle copies and `~/Library/{WebKit,Caches}/com.wisprbro.s30check`.

### S-31 Lesson engine and Transforms tutorial

Items S-31.1-.4 are **queued for the Mac wave** (consolidated UI run;
`scripts/wb-s31/run.sh`, AX driver `scripts/wb-s31/lesson.swift`). Test
bundle: "Wispr Bro S31 Check" (`com.wisprbro.s31check`, portable profile,
onboarding completed, a ready local AI engine so the practice rewrite is
real). No audio: the lesson has no take.

- [x] S-31.1 **Mac wave 2: PASS (7 s end to end)** ([run](test-runs/1.0.0-alpha.1-M-26-macwave-2.md)) (Exit criterion) `run.sh off`: with Transforms off, Transforms
      › Start tutorial › Try it out › select all › ⌥1 › "Text rewritten" › ⌥O
      shows the changes › ⌘Z restores the draft › ⌥1 › Turn on Transforms,
      **under 120 s** end to end; `transforms_enabled` is on afterwards, the
      clipboard is unchanged, no `transforms: run` log line (nothing pasted).
- [x] S-31.2 **Mac wave 2: PASS** ([run](test-runs/1.0.0-alpha.1-M-26-macwave-2.md)) `run.sh on`: with Transforms on, the same practice; the fork tap
      swallows ⌥1 and hands it over (log `tutorials: transform.polish went to the
practice step`), no real run; the last button says Done.
- [x] S-31.3 **Mac wave 2: PASS after a driver fix** ([run](test-runs/1.0.0-alpha.1-M-26-macwave-2.md)) `run.sh outside`: the practice step open, TextEdit in front with
      a selection: ⌥1 runs a real Polish there (log `transforms: run … pasted`),
      nothing is handed to the lesson.
- [ ] S-31.4 **Mac wave 3: PARTIAL scripted: Help › Essentials › Learn Transforms opens the lesson, its example ticker moves; How it works scenes and Reduce Motion by eye (owner)** ([run](test-runs/1.0.0-alpha.1-M-26-macwave-3.md)) **Mac wave 2: PARTIAL: switch on and checklist tick seen; scenes, Reduce motion, Help by eye (owner)** ([run](test-runs/1.0.0-alpha.1-M-26-macwave-2.md)) By eye (screenshot `off.png` and the session): the drawn bar's
      spotlight and "Done · View changes", the three How it works scenes moving
      (and still with Reduce motion on), Help › Essentials › Learn Transforms,
      the checklist's "Try a Transform" ticking after the lesson closes.

### S-32 Whisper tutorial and microphone quiz

Items S-32.1-.4 are **queued for the Mac wave** (nothing here has run on
the Mac yet). Playwright covers the whole lesson in a browser
(`tests/fork/whisper.spec.ts`: banner, Help and checklist entries, the
scene, the practice take passing at ≥ 70 % and the tips below it, the
queued take's example, the four questions and the table, Copy results,
keyboard-only, RTL, reduced motion, axe light/dark), the S-30 chaining
change (`tests/fork/first-use.spec.ts`); bun covers the threshold
(`whisperMatch.test.ts`) and every quiz combination (`micTable.test.ts`).

- [x] S-32.1 **Mac wave 3: PASS 4/4 (whisper at −24 dB: starter 0.913, full 1.000 of the words)** ([run](test-runs/1.0.0-alpha.1-M-26-macwave-3.md)) (R5 real-Mac acceptance) A whispered take at 30 cm passes the
      lesson's own rule (≥ 70 % of the sentence's words) on the **starter**
      and on the **full model**: `run.sh all` (no microphone, no speakers:
      a synthetic whisper made with `say -o`, the Whisper voice when installed,
      scaled to −24 dB with a faint noise floor by `quiet_wav.py`, fed through
      the app's own `--transcribe-file` path). PASS = four `PASS` lines (the
      loud reference and the 30 cm whisper on both models). If the whisper
      FAILs while the reference passes, rerun with `GAIN_DB=-18` and record
      both numbers; the file path skips Silero VAD, so S-32.2 confirms it on a
      live take.
- [ ] S-32.2 By a person (needs the live mic, so the owner, not the driver):
      Help › Essentials › Learn to dictate quietly › the practice step, whisper
      the email at about 30 cm from the built-in mic on the starter and again
      after the full model is in. PASS = the composer fills and Continue
      enables; the newest `entry_meta` row has source `practice`; the
      clipboard is unchanged and nothing was pasted; "Try it in another app"
      does not tick.
- [ ] S-32.3 By a person, the same with the Hub moved behind TextEdit: the
      take pastes into TextEdit as an ordinary dictation (practice takes only
      start with Wispr Bro in front).
- [ ] S-32.4 By eye: the home banner's "Dictate quietly: show me how", the
      scene and the drawings in light and dark, "Open Sound settings" opens
      Sound › Input, "More about microphones" opens the GitHub page in the
      browser, and finishing an item from the checklist card only points at the
      next one (Open at login is never turned on by itself).

**Setup.** As S-29a with `s32` in place of `s29a` (identifier
`com.wisprbro.s32check`, product name "Wispr Bro S32 Check", portable),
with the English starter and the full English model installed in its
`Data/models` (let the app fetch them once from Hugging Face; never copy
anything from `~/wb-bench`). `APP=<bundle> OUT=<dir> scripts/wb-s32/run.sh
models` lists the ids; pass others with `STARTER=` / `FULL=`.

**Procedure.** `APP=<bundle> OUT=<dir> scripts/wb-s32/run.sh all` (headless,
opens no window, no gui.lock); keep `run.log` and `results.jsonl`. Then
S-32.2-.4 by hand on the same bundle. Quit; delete the bundle copies and
`~/Library/{WebKit,Caches}/com.wisprbro.s32check`.

### S-33 ITN (gated by RS-13)

- R5 lists no real-Mac acceptance; RS-13 decides whether the slice ships.

### S-34 Dictionary auto-learn

Queued for the Mac wave (the S-19b + S-36 browser/AX run); not run by the
slice (2026-11-02). Slack is not installed on the test Mac: a local chat
composer (`scripts/wb-s34/composer.html`, Return sends and empties as in
Slack) in a separate Chrome profile stands in, as S-20's thread page does.

- [x] S-34.1 **Mac wave 2: PASS** ([run](test-runs/1.0.0-alpha.1-M-26-macwave-2.md)) (R5 acceptance) `fix`: the take pastes "… Kubernetees …"; the driver retypes it as "Kubernetes" and presses Return; within a second Dictionary › Suggested holds the word "Kubernetes" (`learned`: `word|Kubernetes|||suggested`), `corrections` holds `Kubernetees|Kubernetes`, and the log has `learn: watching the field` then `learn: committed (key|emptied) … 1 correction(s), 1 suggestion(s)`.
- [x] S-34.2 **Mac wave 2: PASS** ([run](test-runs/1.0.0-alpha.1-M-26-macwave-2.md)) (exit criterion) After S-34.1: no field text persisted: `leaks none` (the composer's "door code 4471" is in neither `fork.db`, its WAL nor `wispr-bro.log`); the `learn:` log lines carry no text.
- [x] S-34.3 **Mac wave 2: PASS** ([run](test-runs/1.0.0-alpha.1-M-26-macwave-2.md)) `tab`: the same fix committed by Tab gives the same suggestion (run after rejecting S-34.1's in the app, or after `setup.sh … on`, which clears learned rows).
- [x] S-34.4 **Mac wave 2: PASS** ([run](test-runs/1.0.0-alpha.1-M-26-macwave-2.md)) Rejected never returns: in the app, Dictionary › Suggested › ✕ on "Kubernetes"; then `fix` again: `learned` shows it `rejected`, no new suggestion, the log says `0 suggestion(s)`.
- [x] S-34.5 **Mac wave 2: PASS** ([run](test-runs/1.0.0-alpha.1-M-26-macwave-2.md)) `wait`: the fix without a commit: `learn: nothing learned (no_commit) after ~30000 ms`, no row.
- [x] S-34.6 **Mac wave 2: PASS** ([run](test-runs/1.0.0-alpha.1-M-26-macwave-2.md)) `card`: the "Card number" field: `learn: not watched (sensitive)`; `setup.sh … off` then `fix`: no `learn:` line at all.
- [x] S-34.7 **Mac wave 2: PASS** ([run](test-runs/1.0.0-alpha.1-M-26-macwave-2.md)) Clipboard byte-identical after every case (`clip same`; the driver types, never pastes).
- [ ] S-34.8 **Mac wave 3: PARTIAL scripted: the Suggested tab and Settings › Data and Privacy toggles follow one setting (`auto_learn_enabled`) both ways; light/dark by eye (owner)** ([run](test-runs/1.0.0-alpha.1-M-26-macwave-3.md)) **Mac wave 2: PARTIAL: Suggested tab, Accept/Reject seen by AX; toggle, light/dark by eye (owner)** ([run](test-runs/1.0.0-alpha.1-M-26-macwave-2.md)) (person, UI wave) Dictionary › Suggested: the toggle, Accept/Reject, the row leaves the tab; Settings › Data and Privacy › "Learn from my corrections" follows the same setting; light and dark.
- Not covered on this Mac: Slack itself (not installed); Electron apps are covered by the S-05 lease path only.

**Profile:** the S-20 portable debug build (`com.wisprbro.s20check` or an
`s34check` identity, never the owner's app, data or clipboard), set up as
under S-20, then with the app stopped:

```sh
scripts/wb-s34/setup.sh "$PWD/src-tauri/target/debug/Data" on
say -v Samantha -o $SCRATCH/s34-deploy.aiff "Deploy it to zebra today."
afconvert -f WAVE -d LEI16@16000 -c 1 $SCRATCH/s34-deploy.aiff $SCRATCH/s34-deploy.wav
scripts/wb-s05/app.sh start
```

**Procedure (unattended):** `WB_S34_BIN=$BIN scripts/wb-s34/take.sh <case>
$SCRATCH/s34-deploy.wav <out dir>` for `fix` (S-34.1/.2/.7), `card`,
`wait`, then reject in the app and `fix` again (S-34.4), `setup.sh … on`
(restart) and `tab` (S-34.3), `setup.sh … off` (restart) and `fix`
(S-34.6). Each case holds `~/wb-bench/gui.lock`, records only while Chrome
is in front, plays the WAV once as the app's test microphone (never the
speakers), and prints the
learned rows, the last `learn:` line and the leak check.

### S-35 Vocabulary importers

- [ ] S-35.1 (Exit criterion) Import works on the owner's Mac with a real Wispr Flow install. Evidence: counts only, never the imported words.
- Procedure: TODO (S-35).

### S-36 Vibe coding

Queued for the Mac wave (the S-19b + S-36 browser/AX run); not run by the
slice (2026-11-02).

- [ ] S-36.1 **Mac wave 2: NOT RUN (this Cursor version shows Log In inside the workspace window; needs a signed-in throwaway profile from the owner)** ([run](test-runs/1.0.0-alpha.1-M-26-macwave-2.md)) **Mac wave 1: FAIL / not conclusive (identifiers=0; fresh Cursor profile shows Log In)** ([run](test-runs/1.0.0-alpha.1-M-26-macwave-1.md)) (Exit criterion) Dictate "rename get user by id" in Cursor with `getUserById` visible (screen-reader mode on): the buffer gets `getUserById`; the log shows `context: read ok (…, cursor: N code name(s), 0 file name(s), …)` with N ≥ 1 and `text: … identifiers=1`.
- [ ] S-36.2 **Mac wave 1: BLOCKED (Cursor sign-in)** ([run](test-runs/1.0.0-alpha.1-M-26-macwave-1.md)) In Cursor's chat input with `index.ts` open in a tab, dictate "open index dot ts": the chat gets `@index.ts`; the log shows `cursor chat: … M file name(s)` with M ≥ 1 and `file_tags=1`.
- [ ] S-36.3 **Mac wave 1: BLOCKED (Cursor sign-in)** ([run](test-runs/1.0.0-alpha.1-M-26-macwave-1.md)) The same chat take with "Tag files in chat" off (`setup.sh … off`): the text keeps "index dot ts" (or `index.ts`), no `@`.
- [ ] S-36.4 **Mac wave 1: not run** ([run](test-runs/1.0.0-alpha.1-M-26-macwave-1.md)) Not set up (`setup.sh … unset`): the editor take keeps "get user by id" and the log line has no `cursor:` part (DEC-41: off until set up).
- [x] S-36.5 **Mac wave 1: PASS** ([run](test-runs/1.0.0-alpha.1-M-26-macwave-1.md)) TextEdit (not a listed editor) with `index.ts` in the document: "open index dot ts" is not tagged.
- [ ] S-36.6 **Mac wave 1: BLOCKED (sudo)** ([run](test-runs/1.0.0-alpha.1-M-26-macwave-1.md)) No filesystem scanning (exit criterion): `sudo fs_usage -w -f filesys <app pid>` during S-36.1/.2 shows no open or stat of the workspace folder (`$TMPDIR/wb-s36-workspace`); the names come from AX only. Read p95 of the editor takes stays within S-20's 100 ms.
- [x] S-36.7 **Mac wave 1: PASS (TextEdit and editor takes)** ([run](test-runs/1.0.0-alpha.1-M-26-macwave-1.md)) Clipboard byte-identical after the `editor` and `textedit` takes (the `chat` case reads its text back with ⌘A ⌘C, so its own check reports "changed (restored)" by design).
- [ ] S-36.8 **Mac wave 1: BLOCKED (person)** ([run](test-runs/1.0.0-alpha.1-M-26-macwave-1.md)) (person, UI wave) Settings › Vibe coding: Set up, the three steps, Finish, Change editors, Tag files in chat enabled only with Cursor or Windsurf; light and dark.
- Not covered on this Mac: VS Code and Windsurf (not installed); their bundle ids and chat detection are unit-tested only.
- **Driver fix on `fix/wave1-b` (2026-11-03), queued:** Mac wave 1's `identifiers=0` came from a restored Cursor session (the driver never removed the profile, so the editor take went into `index.ts`). `take.sh` now removes the profile every take (or copies `WB_S36_PROFILE`, a prepared signed-out profile past the Log In window) and sets `window.restoreWindows: none`. Rerun S-36.1: check that the evidence screenshot shows `user_service.ts` focused before reading the verdict. Keep the throwaway `--user-data-dir` short (≤ ~60 chars): Cursor's IPC socket path must fit 103 bytes, so a deep `$TMPDIR` path fails at start.

**Profile:** the S-20 portable debug build (`com.wisprbro.s20check` or a
`s36check` identity, never the owner's app, data or clipboard), set up as
under S-20 with context awareness on, then with the app stopped:

```sh
scripts/wb-s36/setup.sh "$PWD/src-tauri/target/debug/Data" on
say -v Samantha -o $SCRATCH/s36-rename.aiff "rename get user by id"
say -v Samantha -o $SCRATCH/s36-tag.aiff "open index dot ts"
for f in rename tag; do afconvert -f WAVE -d LEI16@16000 -c 1 $SCRATCH/s36-$f.aiff $SCRATCH/s36-$f.wav; done
scripts/wb-s05/app.sh start
```

**Procedure (unattended):** `WB_S36_BIN=$BIN scripts/wb-s36/take.sh <case>
<wav> <out dir>`: `editor s36-rename.wav` (S-36.1), `chat s36-tag.wav`
(S-36.2), `textedit s36-tag.wav` (S-36.5); then `setup.sh … off`, restart,
`chat s36-tag.wav` (S-36.3); `setup.sh … unset`, restart, `editor
s36-rename.wav` (S-36.4). Each case holds `~/wb-bench/gui.lock`, opens
Cursor in a throwaway profile (`$TMPDIR/wb-s36-cursor-profile`,
`editor.accessibilitySupport: on`) on a throwaway workspace, records only
while Cursor (or TextEdit) is in front, plays the WAV once as the app's
test microphone (never the speakers), reads the result back (saved file, chat text, document), and
closes what it opened. If Cursor's chat input needs a sign-in in a fresh
profile, S-36.2/.3 are recorded as not verified (the owner's Cursor
profile is never used).

### S-37 Accessibility, localization, copy pass

Items S-37.1-.3 are **queued for the Mac wave** (S-37a built 2026-11-03;
nothing here has run on the Mac). Playwright covers axe on every Hub page,
Settings section, Hub dialog and onboarding step in light, dark and
`prefers-contrast: more`, Reduce Motion stills and keyboard-only Tab walks
(`tests/fork/a11y-sweep.spec.ts`); `cargo test` covers the announcement
table and texts (`fork/pill/tests.rs`); `bun run wb:copy-check` the copy.

- [ ] S-37.1 (R5 real-Mac acceptance) By a person, by ear:
      `docs/fork/VOICEOVER-CHECKLIST.md` sections 1-6 (onboarding on a fresh
      profile, every Hub page, Settings, the Scratchpad, lessons, the bar and
      one dictation into TextEdit: "Listening", "Transcribing", "Text
      inserted"; a card reads its reason and text; a password field's card
      never reads the text). PASS = every line ticked; record the words heard
      for any FAIL.
- [ ] S-37.2 By a person: the ★ lines of the same checklist again with
      **Increase Contrast** and **Reduce Motion** on (System Settings ›
      Accessibility › Display; `run.sh prefs` prints both). PASS = rings,
      lines and selected rows clearly visible in light and dark, no sliding,
      spinning or confetti, the take and its announcements unchanged.
- [ ] S-37.3 **Mac wave 3: FAIL 1 control (Settings › Advanced: History Limit's number stepper, an unnamed AXIncrementor); 20 other screens PASS** ([run](test-runs/1.0.0-alpha.1-M-26-macwave-3.md)) Scripted AX-tree check, no audio: `run.sh hub`, then
      `run.sh lessons`, then `run.sh onboarding` on a fresh-profile copy.
      PASS = every screen's line in `run.log` is `PASS` (0 unnamed controls
      in `axtree.tsv`); `placeholder-only` rows are listed for review, not
      failures.
- [x] S-37.4 **Mac wave 4: PASS (18/18 screens 0 unnamed, no `AXIncrementor`, `AXTextField "History Limit"` kept; ↑ on the focused field 5 → 6)** ([run](test-runs/1.0.0-alpha.1-M-26-macwave-4.md)) **Queued for the Mac wave** (Mac wave 3 fixes, finding 2):
      `run.sh hub` again. The fork's Handy a11y layer marks Handy's number
      fields and hides WebKit's spin buttons (`settings/handyA11y.ts`,
      `tokens.css`). PASS = `settings Advanced` has 0 unnamed (no
      `AXIncrementor` row; the `AXTextField "History Limit"` stays), every
      other screen as in wave 3; by hand: focus History Limit and press ↑:
      the value goes up by one.

**Setup.** As S-29a with `s37` in place of `s29a` (identifier
`com.wisprbro.s37check`, product name "Wispr Bro S37 Check", portable,
English). For `hub`/`lessons` a finished onboarding (as S-31's bundle); for
`onboarding` a second copy with a fresh profile. The terminal needs
Accessibility. Compile the drivers once:
`swiftc -O scripts/wb-s31/lesson.swift -o $T/wblesson` and
`swiftc -O scripts/wb-s37/axtree.swift -o $T/wbaxtree`.

**Procedure.** `APP=<bundle> OUT=<dir> LESSON=$T/wblesson AXTREE=$T/wbaxtree
scripts/wb-s37/run.sh hub` (then `lessons`; `onboarding` with the fresh
bundle); keep `run.log`, `axtree.tsv` and `unnamed.txt`. Then S-37.1/.2 by a
person with VoiceOver on the same bundle. Quit; delete the bundle copies and
`~/Library/{WebKit,Caches}/com.wisprbro.s37check`.

**S-37b (localization preparation) — queued for the Mac wave.**

- [ ] S-37b.1 The app menu's counts use the app language's plural form: with
      the app language Russian and a profile with a word count whose form is
      `_few` or `_many` (for example 2-4 or 5-20 words dictated), Dictation's
      "N words dictated" and Vocabulary's dictionary and snippet counts read
      as `rust.menu.*_<category>` for their numbers. Repeat once with Arabic
      (six forms) or Polish. A test identity with a seeded profile is enough.
- [ ] S-37b.2 A macOS notification in Russian uses the CLDR form: a
      milestone or queued-takes notification (e.g. the S-07c queue with 2 and
      5 takes) shows the `_few`/`_many` title, and the Hub inbox shows the
      same text.

Procedure (unattended except S-37b.2's banner by eye; ~5 min). Build the
drivers as for S-26a (`WBMENUS`). Launch the S-26a test identity with the
app language set to `ru`, note its pid, then
`scripts/wb-s37b/plurals.sh <pid> ru <out>`: every line PASS. Change the
language to `ar` (or `pl`), relaunch, run it again. For S-37b.2, run the
S-07c queued-takes driver with the language `ru` (two takes, then five) and
compare the banner and inbox titles with `rust.notify.queuedTakesReady.title_few`
and `_many`.

### S-38 Performance and reliability hardening

- [ ] S-38.1 (Exit criterion) Every §5 number met on each reference machine, or the model tier changed to meet it (recorded): latency benches p50/p95 per §5.1, 8 h idle run, 500-take soak, 50-take hardware run, memory-pressure test on 8 GB, startup timing. Part 2 (after S-25..S-37).
- [x] S-38.2 **Mac wave 1: PASS: 157.4 MB median RSS, 0.41 % CPU over 10 min (release, test identity)** ([run](test-runs/1.0.0-alpha.1-M-26-macwave-1.md)) Idle memory and CPU (§5.2) on the release build, models unloaded, bar visible, Hub closed for > 60 s: total RSS (app + its WebKit processes) ≤ 250 MB and app CPU ≤ 0.5 % over 10 min (`python3 -I scripts/wb-soak/idle.py --pid <pid> --minutes 10 --footprint`). Part 1 measured on a debug build (PERF.md). **Queued for the Mac wave** on the release build.
- [x] S-38.3 **Mac wave 2: PASS re-check (Insights › Voice and Home kept across parking, same frame)** ([run](test-runs/1.0.0-alpha.1-M-26-macwave-2.md)) **Mac wave 1: PARTIAL, FAIL: page not kept across parking** ([run](test-runs/1.0.0-alpha.1-M-26-macwave-1.md)) Parked Hub by hand (queued for the Mac wave, one pass, debug
      build with `WB_DEBUG_PARK_AFTER_S=10`): close the Hub, wait 15 s, the
      log says `hub park: main hidden, web view released` and `idle.py`
      shows one WebContent process fewer; then reopen it from (a) the tray's
      Settings, (b) the Dock icon with "Show in Dock" on, (c) a second launch
      of the app, (d) a macOS notification's Open (S-18 queued-takes
      notification: lands on History), (e) ⌥O after a transform (S-22 View
      changes page). Each opens the Hub at its previous position and size,
      in the chosen appearance, on the right page; hotkeys keep working
      while it is parked; the tray glyph matches the menu bar in light and
      dark while parked; ⌘Q / tray Quit still quit at once. Close the
      Scratchpad window, wait 15 s, open it from the bar: the last note is
      there. Minimize the Hub, wait 15 s: it stays in the Dock.
      **Fixed on `fix/wave1-b` (2026-11-03), queued:** the rebuilt Hub opens
      on the page and tab it showed when parked (e.g. close on Insights ›
      Voice, wait 15 s, reopen from the tray: Insights › Voice; close on
      Home: Home).
- [x] S-38.4 **Mac wave 3: PASS 500 takes through the WAV microphone (release test identity, two TextEdit instances): 496 confirmed, 0 lost, 0 duplicate, 0 wrong_target, 4 skipped (System Settings came forward; cancelled by the pid guard), clipboard restored 500/500; app RSS 146→152 MB, bar web view 192→212 MB, no crash** ([run](test-runs/1.0.0-alpha.1-M-26-macwave-3.md)) **Mac wave 2: NOT RUN (BlackHole needs the owner's reboot)** ([run](test-runs/1.0.0-alpha.1-M-26-macwave-2.md)) Exactly-once soak (§5.4, 100 takes for A12.1, 500 before 1.0):
      `scripts/wb-soak/start.sh <test bundle> <out>` (shortcuts unbound,
      model cache cloned, the app started with its test microphone
      `WISPRBRO_TEST_MIC_WAV=<out>/test-mic.wav`; prints `pid` and `mic`),
      then `scripts/wb-soak/soak.py … --pid <pid> --mic <out>/test-mic.wav`
      (header lists the setup: an empty field focused in each app, helpers
      compiled into `--bin`). Since the Mac wave 2 fixes the takes need no
      BlackHole, no reboot and no speakers: soak.py copies each WAV into the
      test microphone before the take (`--device <loopback>` remains for a
      Mac that has one). Pass: 0 lost, 0 duplicate, 0 wrong_target;
      clipboard_restored true on every take. **Queued for the Mac wave**
      (Slack and Notion are not installed on the builder Mac).
- [ ] S-38.5 **Mac wave 1: ps and crash report name wispr-bro (PASS); grants from a handy-named build: owner** ([run](test-runs/1.0.0-alpha.1-M-26-macwave-1.md)) The release bundle's executable is `Contents/MacOS/wispr-bro`: `ps`, Activity Monitor and a test crash's `.ips` name it; Microphone and Accessibility grants from an earlier `handy`-named build still apply (same bundle id). **Queued for the Mac wave** (with F-02.1). **Mac wave 4:** rc1's bundle executable is `Contents/MacOS/wispr-bro` (notary ticket and `verify.sh`); rc1 itself was not launched (it carries the shipping identity, whose single instance and data are the owner's running app), so `ps`/`.ips` stay as Mac wave 1 found them; grants: owner.
- [ ] S-38.6 **Mac wave 2 (M5 Max, load 7.4-7.8): 6/7 rows PASS, LLM cold start 5,447 ms FAIL (one sample)** ([run](test-runs/1.0.0-alpha.1-M-26-macwave-2.md)) §5.1 latency on each reference machine (part 2; headless, no
      GUI, nothing played): `scripts/wb-perf/latency.sh` (TTS takes written to
      files, `fork::perf_bench` on the shipped recognizer, pipeline, cleanup
      and Polish transform with the real sidecar; PASS/FAIL per row from
      `scripts/wb-perf/latency.ts`). Run it with each tier's speech model
      (`WB_ASR_MODEL`) on M1 8 GB, M-series 16 GB and, if DEC-29 keeps it,
      Intel. M5 Max numbers in `docs/fork/PERF.md`. **Queued for the
      reference-machine wave** (M1 8 GB / M 16 GB not on hand).
- [ ] S-38.7 8 GB memory rule (part 2). Simulated on any Mac with a debug
      build: `WB_DEBUG_MEMORY=physical_gb=8` (8 GB tier: speech model capped
      at 1,400 MB footprint, Muesli 2B), then
      `WB_DEBUG_MEMORY=physical_gb=8,available_mb=2000` → a Light take skips
      the local LLM (Apple's model stands in with Apple Intelligence on, else
      the bar reads "Inserted without cleanup"), then
      `…,pressure=critical` with the server running → the take skips the LLM
      and within 5 s the log says `stopped the idle server`; `pressure=warn`
      with the server running → cleaned as usual. On a real M1 8 GB: 30 takes
      with Light while Safari holds ~15 tabs: combined RSS (app +
      `llama-server`) ≤ 3.2 GB during a take, `sysctl vm.swapusage` grows
      ≤ 200 MB over the session, no stall beyond the 6 s deadline, no crash.
      **Queued for the Mac wave** (simulation) and the reference-machine
      wave (M1 8 GB).
- [x] S-38.8 **Mac wave 2: PASS on the release build (warm p50 1,015 ms, p95 1,415 ms; cold launch not measured)** ([run](test-runs/1.0.0-alpha.1-M-26-macwave-2.md)) Startup (§5.3, part 2): `/usr/bin/lockf -k ~/wb-bench/gui.lock
python3 -I scripts/wb-perf/startup.py --app <portable dir or .app>
--runs 10` on a finished-onboarding portable profile with Accessibility
      granted: launch → hotkey ready and bar visible p50 ≤ 2 s, p95 ≤ 3 s,
      plus the cold first launch reported. Release build on each reference
      machine. **Queued for the Mac wave.**
- [ ] S-38.9 50-take hardware run (§9, before 1.0): `scripts/wb-soak/start.sh
--hardware <test bundle> <out>` (real microphone, only fn kept), then `python3 -I
scripts/wb-soak/soak.py --hardware --plan plan.json --takes 50 --app
unused --pid 0 --bin <helpers> --out <dir>`; a person holds fn and reads
      each printed sentence into the focused app. Pass: 0 lost, 0 duplicate,
      0 wrong_target, clipboard restored every take. **Queued for the Mac
      wave** (needs the owner).
- [ ] S-38.10 8 h idle (§5.2, RT-17, before 1.0): release build, bar
      visible, 100 takes spread over the first hour (`soak.py --takes 100
--gap 30`), then `python3 -I scripts/wb-soak/idle.py --pid <pid>
--minutes 480 --every 60 --footprint`: idle RSS ≤ 250 MB and CPU ≤ 0.5 %
      at the end, the pill's web view growth ≤ 50 MB, no crash. **Queued for
      the Mac wave** (overnight; the takes use the test microphone,
      `start.sh`, no BlackHole).
- [x] S-38.11 **Mac wave 3: PASS (two takes, the second after `wb_mic_load`; no Microphone TCC request, volume untouched)** ([run](test-runs/1.0.0-alpha.1-M-26-macwave-3.md)) **Queued for the Mac wave** (Mac wave 2 fixes: takes without speakers). A debug test bundle started with `WISPRBRO_TEST_MIC_WAV=<scratch>/test-mic.wav` and keys unbound (`. scripts/wb-lib/app.sh; wb_unbind_keys <Data>; wb_test_mic <scratch> <say WAV>`), our own TextEdit in front (`wb_textedit_open`), one SIGUSR2 take of the WAV's length (`wb_mic_play`). Pass: the log says `test microphone plays …`; the bar shows levels while the WAV plays; the TextEdit file gets the sentence; the orange microphone indicator never lights and no Microphone prompt appears (no real device is opened); the output volume is untouched. Then a second take after `wb_mic_load <another WAV>` transcribes the new sentence (the file is re-read per take). Drivers switched to it: `wb-s07a/take.sh`, `wb-s07b/take.sh`, `wb-s24/recheck.sh`, `wb-s29b/run.sh`, `wb-s30/run.sh`, `wb-s34/take.sh`, `wb-s36/take.sh`, then every other Mac driver: `wb-s05` (`app.sh start` sets the test microphone for every debug-build driver, unbinds the shortcuts and clones the model cache), `wb-s06`, `wb-s08` (`take`; `loss` needs real mic audio and refuses without `WB_ALLOW_SPEAKERS=1`), `wb-s09-soak.sh`, `wb-s11`, `wb-s12a`, `wb-s12b`, `wb-s15a-takes.sh`, `wb-s16`, `wb-s17`, `wb-s19a`, `wb-s19b`, `wb-s20` and the soak (`wb-soak/start.sh`, `soak.py --mic`) (`wb_mic_load`/`wb_mic_play`; they refuse to play through the speakers unless `WB_ALLOW_SPEAKERS=1`). `wb-s20/names-audio.ts` only writes files.
- [x] S-38.6b **Mac wave 3: PASS on a release bench (load 4.1-6.7): LLM cold start 421 / 403 ms (two samples, sidecar started once before); first start of a fresh sidecar copy 4,607 ms (the one-time Metal compile, its own row); every other row PASS except sample 1's first ASR load (4.6 s, the new bench binary's own Metal compile; sample 2: 285 ms p95)** ([run](test-runs/1.0.0-alpha.1-M-26-macwave-3.md)) **Queued for the Mac wave** (Mac wave 2 finding 4, see `PERF.md` "LLM cold start"): `latency.sh` on a **release** build, two samples, with the sidecar already started once on this Mac, then one deliberate first start of a freshly built sidecar recorded on its own row (the one-time Metal library compile). The spawn line now has `--fit off` (no dry-run second model load).
- Procedure: S-38.2-S-38.10 above. Machines: all for S-38.1.

### Owner test 1 fixes (`fix/owner-test-1`)

Test identity `com.wisprbro.metercheck` (portable debug build in the
worktree's `src-tauri/target/debug`, started from that folder; shortcuts
unbound, models cloned, update checks off). Never the owner's app or data,
never the speakers or the real microphone.

- [x] OT1.1 **Builder run (2026-11-03): PASS.** Mic meter with café noise:
      `EXE=<…/target/debug/wispr-bro> OUT=<scratch> scripts/wb-meter/run.sh`
      (the WAV comes from `scripts/wb-meter/make-wav.py`: 4 s of pink noise +
      120 Hz hum at -30 dBFS, `say` speech mixed over the noise, 3 s of noise,
      2 s of silence; onboarding resumed at the mic test, the meter starts by
      itself; the levels come from the `fork audio: meter at …` trace lines).
      318 readings: noise mean 0.00 (max 1 bar), speech mean 0.43 / max 0.80
      (up to 10 of 12 bars, moving), noise after mean 0.01, silence 0. The
      old fixed -60..0 dBFS mapping on the same readings: 6-7 bars on the
      noise, 6-10 during speech (the owner's report).
- [x] OT1.2 **Mac wave 6: FIX → PASS (hum held the bars at 0.55; now flat)** ([run](test-runs/1.0.0-alpha.1-M-26-macwave-6.md)) **Queued for the Mac wave.** The Flow Bar's waveform, by eye:
      same build and WAV (`WISPRBRO_TEST_MIC_WAV`), a take by SIGUSR2 with
      the café WAV: the bars lie flat on the noise and move with the words.
- [x] OT1.3 **Mac wave 6: PASS** ([run](test-runs/1.0.0-alpha.1-M-26-macwave-6.md)) **Queued for the Mac wave.** A stuck alpha profile repairs itself
      on the release build: a portable release test bundle whose
      `Contents/MacOS/Data/fork.db` is a v5 file with v6's columns (make it
      with `ForkDb::with_migrations(test_migrations(5))` + 0006's SQL, as
      `migrate_tests.rs` does; never the owner's file), first launch: the log
      has `fork.db repair:` and `fork.db migrated from version 5 to 9`, one
      `fork.backup-v5-*.db`; Dictionary › Add word saves "Siobhán" and it
      survives a relaunch.
- [x] OT1.4 **Mac wave 6: PASS** ([run](test-runs/1.0.0-alpha.1-M-26-macwave-6.md)) **Queued for the Mac wave.** A failed save is explained: with
      the same bundle's `fork.db` replaced by a file that cannot migrate
      (`user_version` 99 makes it "too new"), Dictionary › Add word shows
      "Not saved: Wispr Bro can't open its data file. …" in the dialog and
      the page shows "Your dictionary can't be opened".

### Owner test 2 fixes (`fix/hold`, OT2-A)

The S-05 portable debug build (`com.wisprbro.s05check`, set up as in S-05
above), started by the driver with only push-to-talk and the double press
bound, on **Ctrl+Option** by default so the owner's Wispr Bro (on fn) never
answers the synthetic hold (`WB_OT2A_TRIGGER=fn` uses fn, and the driver
refuses it while another Wispr Bro runs). Takes go through the hotkey
engine: a synthetic trigger hold (`scripts/wb-s01-keys.swift`:
`flagsChanged` with the held flags; `click`, `activate`, `front` added for
OT2-A) and the test microphone (a `say` WAV, never the speakers). Each take
saves and restores the whole clipboard. Helpers:
`swiftc -O -o $WB_S05_BIN/keys scripts/wb-s01-keys.swift`, plus `clip` and
`drive` from `scripts/wb-s05/`. Run:
`WB_S05_BIN=<bin> scripts/wb-ot2a/run.sh <out> [rounds]`.

- [ ] OT2-A.1 **Queued for the Mac wave.** `hold:cmdtab`: our Finder window
      in front, the trigger held; 1 s in, ⌘-Tab with the trigger still held
      and TextEdit activated; released after the speech. Pass: no
      `Cancelled`/`ChordIgnored`/`StuckReleased` in the log, the text in
      the TextEdit document (route `paste`), History has it.
- [ ] OT2-A.2 **Queued for the Mac wave.** `hold:click`: two TextEdit
      documents, a click (trigger flags held) from A into B mid-take. Pass:
      the text in B, A empty, the take never cancelled.
- [ ] OT2-A.3 **Queued for the Mac wave.** `hold:finder` and
      `hold:desktop`: start in TextEdit, click into our Finder window (or an
      empty desktop spot) mid-take, release there. Pass: nothing pasted
      (TextEdit empty), route `card (no_text_field)`, the log line
      "copied to the clipboard", the clipboard holds the text as plain
      unmarked text equal to History's, the bar shows "Copied to clipboard" (not the card; check
      once by eye or `screencapture`), and the driver's restore is
      byte-identical.
- [ ] OT2-A.4 **Queued for the Mac wave.** `free:cmdtab`: the same switch in
      hands-free (double press, ⌘-Tab + TextEdit, a press stops). Pass: the
      text in TextEdit; both modes decide the target the same way.
- [ ] OT2-A.5 **Queued for the Mac wave.** With fn (`WB_OT2A_TRIGGER=fn`,
      only when the owner's app is quit, or a person): the four cases above
      on the real Globe key, plus a real ⌘-Tab through the switcher and a
      trackpad swipe to another Space while holding fn (synthetic events do
      not drive the system's own gestures).
- [ ] OT2-A.6 **Queued for the Mac wave.** S-05's driver after OT2-A:
      `scripts/wb-s05/run.sh <out> 1 card:finder card:desktop moved:40`
      now expects "Copied to clipboard" and the plain text on the clipboard
      instead of the card (`summarize.py` updated); `card:password` still
      expects the refused card.

### Owner test 2 fixes, part E (`fix/polish2`, OT2-E)

Owner feedback on rc3: the DMG window, About's GitHub links, Support in the
sidebar, and four design calls from Mac wave 6. Never the owner's Wispr Bro,
`/Applications/Wispr Bro.app` or its data; no sudo. UI checks run on a debug
test bundle (`com.wisprbro.polish2check`, shortcuts unbound,
`WISPRBRO_HUB_ROUTE` for deep links) with `screencapture -l` on the app's own
windows, light and dark, English and Arabic.

- [x] OT2.1 **Builder run (2026-11-04): FIX → PASS.** The first dry run built
      the app and the DMG with its window (every install-window check
      passed) but `codesign --verify --deep --strict` rejected the app in
      the image: hiding the `.app` extension (`SetFile -a E`) had put Finder
      info on the bundle. The extension is no longer hidden (Finder hides
      `.app` by default anyway); the DMG rebuilt by `dmg/build.sh` from the
      same app then passed `verify.sh --unsigned` in full (window checks,
      ad-hoc signature deep/strict, `--list-devices` smoke). Unsigned dry run
      of the release pipeline: `macos.sh --out <scratch> --unsigned` runs the
      tauri build, then `dmg/build.sh`, then `verify.sh --unsigned`, which reads
      the mounted image's `.DS_Store`: background 1x + 2x, toolbar, sidebar and
      status bar hidden, 660 × 400, icons 128, Wispr Bro.app at (180, 228),
      Applications at (480, 228). No Finder window was opened (the image is
      attached `-nobrowse`). A second full `macos.sh --unsigned` run after the
      fix built the DMG through the pipeline; its `verify.sh` stalled in a
      subshell `source` of `dmg/build.sh` and then in a plain `hdiutil detach`
      while the Mac was loaded, so the venv lookup became `build.sh
--print-python` and the detach gets 60 s before `-force`; `verify.sh
--unsigned` on that DMG then passed every check in 3.4 s.
- [ ] OT2.2 **Queued for the Mac wave.** The DMG window by eye: open the
      dry-run (or next rc's) DMG in Finder, light and dark appearance.
      Pass: the window opens at about 660 × 400 with no toolbar or sidebar,
      our background whole (heading "Drag to install", the help line, the
      tray, the arrow; nothing cut off at the bottom), the app icon left
      and Applications right on the tray at 128 pt, both labels readable
      (dark in light mode, light in dark mode), the volume icon is ours;
      dragging the app onto Applications copies it (to a scratch
      Applications folder alias if the owner's must stay untouched: drag
      onto a Finder window of `<scratch>/Applications` instead). Then the
      signed path: the next rc's `verify.sh` lists the install-window
      checks and passes, and the stapled DMG still opens with no Gatekeeper
      prompt.
- [ ] OT2.3 **Queued for the Mac wave.** Sidebar Support: the item sits
      between Share with a friend and Settings, stays at the bottom with the
      setup card open at the default window size; a click opens "Support
      Wispr Bro" (Star on GitHub with GitHub's mark, Contribute, Send
      feedback; no Sponsor item); Help › Get in touch › Support Wispr Bro
      opens the same menu. Light/dark, Arabic. Links are not clicked (they
      open the owner's browser).
- [ ] OT2.4 **Queued for the Mac wave.** Settings › About: View source code,
      View license and Handy on GitHub show the GitHub mark in the button's
      ink, crisp at 2x, light and dark.
- [ ] OT2.5 **Queued for the Mac wave.** Handy's hosted rows in sentence
      case: Settings › System ("Launch on startup", "Show tray icon", "Start
      hidden"), Advanced and the debug rows; the Turkish and Italian Hub
      show their rows in sentence case; Japanese and German unchanged.
- [ ] OT2.6 **Queued for the Mac wave.** Home banner: at the default window
      and at full screen, English and Arabic, the heading, body and button
      stay clear of the microphone in the photo.
- [ ] OT2.7 **Queued for the Mac wave.** Onboarding Summary on the unbound
      test profile: "Not set" rows show a grey dash, set rows the green tick.
- [ ] OT2.8 **Queued for the Mac wave.** No focus ring after a deep link:
      `WISPRBRO_HUB_ROUTE=#/hub/settings/microphone` at launch, Help ›
      Microphone by mouse, the tray's Shortcuts…, and a tab deep link
      (Insights › Voice): no ring on the close button or the tab panel; one
      Tab press then draws the ring on the next control.

### S-39 1.0 release

- [ ] S-39.1 RS-12 re-run `1.0.0-rc → 1.0.0`.
- [ ] S-39.2 Install and `brew uninstall --zap`.
- [ ] S-39.3 Part A passes on all reference machines (R5 §9).
- Procedure: TODO (S-39).
