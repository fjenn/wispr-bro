# RS-01: fn / 🌐 key test, how to run it

This test checks what happens when Wispr Bro listens to the fn key (the 🌐
globe key, bottom left of the keyboard). It takes **about 15-20 minutes**.
Nothing leaves your Mac.

## Start it (3 steps)

1. Open **Terminal** (press ⌘ Space, type `Terminal`, press Enter).
2. Copy this line into Terminal and press Enter:

   ```sh
   ~/Projects/wispr-bro/scripts/wb-spike-fn.sh
   ```

   The first time, it spends about a minute getting ready.

3. Follow what it says on screen. It asks short yes/no questions: type
   `y` or `n` (or `u` if you're not sure) and press Enter.

To stop at any time, press **Control + C**. What you did up to then is kept.

## What will happen

- **Permissions.** The test checks what Terminal may do. If something is
  off it tells you exactly where to switch it on in System Settings, offers
  to open that page, and waits for you:
  - **Accessibility** (Privacy & Security → Accessibility → Terminal): needed.
  - **Microphone** (macOS asks "Terminal would like to access the
    microphone": click Allow): needed for the orange-dot checks.
  - **Input Monitoring** (Privacy & Security → Input Monitoring → Terminal):
    only if the test cannot see the fn key; it checks with one test press.
  - If macOS says Terminal must quit and reopen, do that and run the same
    command again.
- **Your 🌐 setting.** It reads "Press 🌐 key to" (System Settings →
  Keyboard) and asks you to confirm what it says. It never changes it.
- **TextEdit.** It opens a TextEdit window with practice text. Put it next to
  the Terminal window.
- **Seven short tests**, each explained before it starts:
  1. hold fn for a second, 10 times;
  2. quick taps, 10 times;
  3. double-taps, 10 times;
  4. fn + Space in TextEdit, 5 times;
  5. Esc while holding fn, 5 times;
  6. about 30 fn shortcuts in TextEdit (fn + arrows, fn + delete, fn + F11);
  7. 10 takes saying "testing one two three" as you press fn.

  After each test you answer a few questions about what you saw: did the
  emoji picker, a language switcher or Apple's Dictation pop up? Did the
  orange microphone dot appear (top right of the screen)? Did the shortcut
  still do its normal thing?

- **Optional extra rounds** (about 2 minutes each): tests 1-3 again with each
  of the other three "Press 🌐 key to" settings. It tells you exactly what to
  pick in System Settings each time, and reminds you to set it back at the
  end.
- **Results.** At the end it prints a short summary and the folder it saved
  everything in (`~/wb-bench/results/rs01/<date-time>/`). Tell Claude you're
  done; it reads the results from there. You can close TextEdit without
  saving.

What is saved: the key presses during the test (ordinary typing in other
apps is recorded only as "(typing)", never as letters), the test's decisions,
your answers, and ten 4-second sound clips of "testing one two three". All
of it stays on this Mac.

## Round 2 (about 5-8 minutes)

Round 1 showed two things to fix. Round 2 tries the fixes. Start it the same
way, with one extra word:

```sh
~/Projects/wispr-bro/scripts/wb-spike-fn.sh --round 2
```

What will happen:

- **Permissions**, as in round 1 (usually nothing to do this time).
- **Your 🌐 setting must be "Do Nothing".** The test reads it. If it says
  something else, it tells you to change it in System Settings → Keyboard →
  "Press 🌐 key to", offers to open that page, and checks again. At the end
  it reminds you to set it back.
- **One question:** does F11 or fn+F11 normally do anything on your Mac
  (for example, show the desktop)?
- **Part A** (Wispr Bro does *not* hold back the fn key this time):
  1. quick taps, 5 times;
  2. double-taps, 5 times;
  3. holds of about a second, 5 times;
  4. in TextEdit: fn + F11 five times, then fn + ← and fn + → five times.
     **One shortcut at a time:** press fn, press the key, let go of both.
     Don't keep fn held between shortcuts.

  After each one: did anything pop up? Did the orange dot appear? Did the
  shortcut do its normal thing?
- **Part B:** 20 takes of "testing one two three", 10 one way, then 10
  another way. Before each take you see "get ready...", then a line starting
  with **>>> NOW**. Then press fn and **start talking at once**: don't wait
  for the orange dot. Let go after "three". The next cue comes by itself.
  Before the second set, the test asks whether the orange dot is showing even
  though you're not pressing anything. Please look carefully: that answer
  decides which fix we use.
- **At the end** it checks every take by itself (did the mic catch the start
  of "testing"?), writes it down, and prints a summary. Results go to
  `~/wb-bench/results/rs01/<date-time>-r2/`. Tell Claude when you're done.

What is saved: the same kind of key log as round 1, your answers, and the
20 takes (4 seconds each), plus their transcripts. All of it stays on this
Mac.

## For the team: what this run covers and what it doesn't

The tool is `src-tauri/crates/wb-spike-fn` (its own workspace and lockfile;
`--dry-run` prints the intro and permission check and exits, `--self-test`
runs the fn grammar against its scripted presses). It runs the RS-01 two-tap
setup: Handy's `HotkeyManager::new_with_blocking()` registering Handy's macOS
defaults the way `init_shortcuts` does (`transcribe` = option+space; `cancel`
skipped; post-processing off), plus the fork's own
`KeyboardListener::new_with_blocking` with the §4.8 blocking set (`{FN}`
always; `{FN, Space}` while fn is down; `{Esc}` and `{FN, Esc}` while a take
is active). The fn grammar is `src/grammar.rs` (S-01 rules 1-7, `T` = 500 ms,
`T_show` = 150 ms). The mic is opened cold through handy-recorder at `T_show`
(at once on a double-tap or fn+Space) and closed on release, cancel or a
chord. Output: `events.jsonl` (raw events from both taps, every decision, mic
open/first-audio/close times, blocking-set changes, tap health from
`CGGetEventTapList` at every block boundary), `summary.json`, `summary.txt`,
`takes/*.wav`.

Compared with the RS-01 row in R5 §7, this run is smaller:

- **macOS 26 only.** RS-01 also asks for macOS 14 and 15; only a macOS 26 Mac
  is available. Re-run on 14 and 15 when there are machines.
- **About 30 chords instead of 100**, in TextEdit only (not Finder), and only
  fn + F11 / F1 instead of fn + F1..F12.
- **10 clipping takes instead of 50.** The WAVs are saved; listening to or
  transcribing them is a separate step.
- **Not covered:** the F5 dictation key bound as fn+dictation, Ctrl+Opt on an
  external keyboard, the 10 sleep/wake cycles, and Secure Input (RS-14).
- Handy's tap only reports its own registered hotkeys, so `events.jsonl` has
  every raw event from the fork's tap but only hotkey events from Handy's.

Round 2 (`--round 2`, `src/round2.rs`; results in `report: RS-01-fn-globe.md`):

- Grammar unchanged except two options: `Timings::with_show_ms(100)` for the
  whole round, and `Grammar::with_fn_block(_, false)`, which leaves `{FN}`
  out of the blocking set (`{FN, Space}` and `{Esc}`/`{FN, Esc}` as before).
- Clipping variants, 10 takes each: (a) cold open through handy-recorder;
  (b) a prepared input unit. That is cpal `build_input_stream`, which on
  macOS ends in `AudioUnitInitialize` with the unit stopped, built before the
  takes; each take then only calls `play`/`pause`
  (`AudioOutputUnitStart`/`Stop`). handy-recorder cannot do this today:
  `Recorder::open` builds and plays the stream. A pre-roll (c) was dropped:
  it needs the input running while idle, which lights the indicator.
- Each take is judged from the data: fn-down, first captured sample, speech
  onset (energy, `src/onset.rs`). Speech within 50 ms of the first captured
  sample counts as "clipped by us"; leading silence counts as "not clipped".
  Transcripts come from `src-tauri/target/release/wb-bench-asr run` with
  `~/wb-bench/asr/parakeet-unified-en-0.6b/parakeet-unified-en-0.6b-Q8_0.gguf`
  (one model load for all takes; ffmpeg only if a WAV is not 16 kHz mono).
  If the binary is missing, the summary says how to build it:
  `cd src-tauri/crates/wb-bench-asr && CMAKE_POLICY_VERSION_MINIMUM=3.5
  CARGO_TARGET_DIR=../../target cargo build --release --offline`.
- `wb-spike-fn --analyze <run folder>` applies the same per-take analysis to
  any finished run (round 1 included) and writes `analysis.json` there.
- `--round 2 --dry-run` prints the plan, the Globe setting and whether
  transcripts will work, then exits.
