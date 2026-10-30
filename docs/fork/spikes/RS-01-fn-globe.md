# RS-01: fn / 🌐 key, round 1 results

**Run:** 2026-10-30, owner, `wb-spike-fn` 0.1.0 at `ae7242f7`. MacBook Pro Mac17,6
(M5 Max), macOS 26.6.2 (25G83), built-in keyboard and microphone, Terminal.
Raw results stay on the Mac (personal data, never committed):
`~/wb-bench/results/rs01/20261030-110952/` (`summary.txt`, `summary.json`,
`events.jsonl`, `takes/*.wav`, and `analysis.json` from
`wb-spike-fn --analyze`).

Setup under test (R5 §4.8): Handy's `HotkeyManager::new_with_blocking()` with
Handy's macOS default (`transcribe` = option+space), plus the fork's own
`KeyboardListener::new_with_blocking` with `{FN}` always blocked, `{FN, Space}`
while fn is down and `{Esc}`/`{FN, Esc}` while a take is active. The S-01
grammar ran with `T` = 500 ms and `T_show` = 150 ms, and the mic was opened
cold through handy-recorder at `T_show`.

## Short version

- **The Globe action leaks: DEC-44 is needed on macOS 26.** Blocking the
  fn-down event in our tap does not stop macOS's own 🌐 action. With "Show
  Emoji & Symbols" (this Mac's default: the key was unset) or "Change Input
  Source", a quick fn press triggers it. A 1 s hold never does. "Do Nothing"
  and "Start Dictation" showed nothing.
- **The grammar works.** Holds, double-taps, fn+Space, Esc and quick taps all
  behaved as specified, and both taps stayed alive at all 34 checks.
- **Chord timing is not measured yet.** The owner held fn down for the whole
  chord test (one 102 s hold with 94 keys), so round 1 has only one
  "fn → first chord key" time: 1,256 ms. The chord criteria failed for
  that reason, not because of a grammar bug.
- **Latency fails narrowly.** fn press → first audio was p50 262 ms and p95
  293 ms (§5.1 asks for p50 ≤ 250 ms and p95 ≤ 350 ms). That is `T_show`
  (about 154 ms in practice) plus about 110 ms of cold open.
- **Clipping is not shown by round 1.** No take was clipped by us: in all 10
  takes speech began after the mic was live (0.4-1.5 s after the press).
  "testing" is missing from 2 transcripts, but neither is a mic clip (see
  below). The ≤ 1-in-50 gate is therefore still untested.
- **Round 2** (`./scripts/wb-spike-fn.sh --round 2`, procedure in
  `RS-01-procedure.md`) tests the fixes: fn not blocked with the Globe
  setting at Do Nothing, `T_show` = 100 ms, and a prepared mic (DEC-53).

## Results against every RS-01 criterion

| RS-01 criterion (R5 §7)                                       | Round 1                                                | Notes                                                                                                                                                                                                                                                                                                                                                                                        |
| ------------------------------------------------------------- | ------------------------------------------------------ | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| No Globe action on tap, hold or double-tap, all four settings | **FAIL**                                               | See the table below. DEC-44 applies on macOS 26 for Emoji and Input Source.                                                                                                                                                                                                                                                                                                                  |
| 100/100 chords keep their native function                     | partly                                                 | fn+←/→ (Home/End), fn+↑/↓ (PgUp/PgDn) and fn+Delete (forward delete): yes. fn+F11 "did not do what F11 normally does": unresolved, see Chords. 94 chord keys, but all in one fn hold.                                                                                                                                                                                                        |
| 0 starts during chords                                        | **FAIL, but not a valid measurement**                  | The 94 keys came in a single 102 s fn hold whose first key (Home) arrived 1,256 ms after fn-down. By then the take had started and passed `T`, so later keys passed through and it kept recording (correct per rule 2). The block's other two "starts" were fn presses without a chord key: a 169 ms press, which opened the mic and was cancelled with no second press, and an 892 ms hold. |
| Orange indicator never shown during chords or quick taps      | quick taps PASS (40/40, mic never opened); chords FAIL | The dot during chords is explained by the 102 s hold, which was a real take.                                                                                                                                                                                                                                                                                                                 |
| F5 dictation key suppressed                                   | not run                                                |                                                                                                                                                                                                                                                                                                                                                                                              |
| Ctrl+Opt on an external keyboard                              | not run                                                | Built-in keyboard only.                                                                                                                                                                                                                                                                                                                                                                      |
| ≤ 1 clipped first word in 50 takes                            | **not shown**                                          | 10 takes, 0 clipped by us, because the speaker always started late. See Clipping.                                                                                                                                                                                                                                                                                                            |
| Both taps deliver after every wake                            | not run                                                | 34 health checks (`CGGetEventTapList`) during the session: both taps present and enabled every time.                                                                                                                                                                                                                                                                                         |
| §5.1 fn-down → capture running, cold: p50 ≤ 250, p95 ≤ 350 ms | **FAIL** (p50)                                         | p50 262 ms, p95 293 ms over 50 takes.                                                                                                                                                                                                                                                                                                                                                        |
| Grammar (S-01 rules 1-7, tool checks)                         | PASS                                                   | Hold 40/40, double-tap 40/40, fn+Space 5/5 (no space typed), Esc with fn held 5/5, quick tap 40/40 with the mic closed.                                                                                                                                                                                                                                                                      |

### Globe action by "Press 🌐 key to" setting

The owner was asked after each block: "Did anything else pop up or change:
the emoji picker, the input source switching, or Apple's own Dictation?" The
fork tap blocked the fn-down event throughout.

| Setting (`AppleFnUsageType`)              | Hold 1 s ×10 | Quick tap ×10 | Double-tap ×10 | fn+Space ×5   | Chords        |
| ----------------------------------------- | ------------ | ------------- | -------------- | ------------- | ------------- |
| Show Emoji & Symbols (unset = default; 2) | nothing      | **popped up** | **popped up**  | **popped up** | **popped up** |
| Change Input Source (1)                   | nothing      | **popped up** | **popped up**  | –             | –             |
| Do Nothing (0)                            | nothing      | nothing       | nothing        | –             | –             |
| Start Dictation (3)                       | nothing      | nothing       | nothing        | –             | –             |

How to read it:

- The action fires on a **short fn press-release**, whatever else is
  happening. It never fires on a 1 s hold. The fn+Space and chord blocks also
  had short presses (the "press fn once to stop" press after a lock is
  110-310 ms in the fn+Space block, and the chord block had a 169 ms press), so their
  "popped up" may come from those presses rather than from fn+Space or the
  chord itself. Either way the fix is the same.
- The action is triggered below the session event tap: dropping the fn-down
  `FlagsChanged` event in our tap does not prevent it (modifier releases are
  never blocked by handy-keys).
- Start Dictation showed nothing. We did not check whether Dictation was
  enabled on this Mac, so this row says only that nothing appeared here.
- One read of the setting is unexplained. In the Do Nothing round,
  `defaults read com.apple.HIToolbox AppleFnUsageType` returned 2, while the
  owner confirmed System Settings showed Do Nothing, and nothing popped up.
  The other reads (1, 3, and 2 after restoring) matched. Possibly the value
  was set after the read. The onboarding check (DEC-44) must re-read before it
  acts, and round 2 re-checks the value.

**Outcome:** R5 DEC-44 now carries a dated note. Onboarding offers the
consent-and-set-Do-Nothing path when the value is 1, 2 or unset, and leaves 0
and 3 alone.

### Chords: fn-down → chord-key timing

Distribution of the time from fn-down to the first chord key of each press,
from `events.jsonl`:

| Block                                   | Presses with a chord key | fn-down → first key                                              |
| --------------------------------------- | ------------------------ | ---------------------------------------------------------------- |
| Chords (the 94 keys)                    | 1                        | Home at 1,256 ms (then 93 more keys over 101 s in the same hold) |
| fn+Space (Space is a lock, not a chord) | 7                        | 110, 171, 182, 200, 567, 1,108, 6,998 ms                         |
| Esc with fn held                        | 6                        | 480-1,252 ms (Esc and one Space)                                 |

So there is **no chord timing distribution yet**. The single chord press is
far beyond `T` (500 ms), which says the owner pressed fn first and looked for
the key afterwards. That is not how a fluent fn+arrow is typed. Round 2 asks
for one shortcut per press ("let go of both before the next one") and reports
min/p25/p50/p75/max plus counts under 100, 150 and 500 ms. S-01 still handles
the ambiguous case per rule 2: a chord key after `T_show` but before `T`
discards the take silently (no paste, pill gone at once). After `T`, keys
pass through and the take continues.

**fn+F11.** The fork tap logged `F11` with the Fn modifier, not blocked.
`com.apple.keyboard.fnState` is unset, so the top row sends media keys and
fn+F11 sends a plain F11. `AppleSymbolicHotKeys` has no entry 36 (Show
Desktop). An absent entry means macOS uses its _default_, which for Show
Desktop is normally F11. So the absence does **not** show that F11 has no
action on this Mac. Open possibilities: (1) Show Desktop is off or remapped
in System Settings; (2) the dropped fn-down event left the system's modifier
state without fn, so the hotkey did not match; (3) the owner expected a
different action. Round 2 asks whether F11 / fn+F11 normally does anything,
and repeats fn+F11 with fn **not** blocked.

## Latency (cold mic)

Over the passing hold and clipping takes (fn press → first audio chunk):
**p50 262 ms, p95 293 ms** (n = 50). The breakdown over all 68 cold opens
after a held press:

| Step                                      | p50    | p95    |
| ----------------------------------------- | ------ | ------ |
| fn-down → mic requested (`T_show` + loop) | 154 ms | 155 ms |
| request → `Recorder::open` returned       | 44 ms  | 72 ms  |
| open → first audio chunk                  | 66 ms  | 68 ms  |

Cutting `T_show` to 100 ms should give about 210 ms (passes p50). A
prepared input unit removes most of the 110 ms. A bench on this Mac measured
first audio about 20 ms after the start request against about 112 ms for a
cold open (`cargo test --release -- --ignored prepared` in
`src-tauri/crates/wb-spike-fn`).

## First-word clipping, judged per take

The coordinator's rule: if speech is already present in the very first
captured frames, the take was **clipped by us** (speech began before the mic
was live). If there is leading silence, the speaker started after the mic was
live, so it is **not clipped**. `wb-spike-fn --analyze <run folder>` applies
this to every take. It finds the speech onset by energy (`src/onset.rs`) and
transcribes each take with the RS-09 bench binary (parakeet-unified-en 0.6B
Q8_0).

| Take | fn → first audio | Speech began after the press | Verdict     | Transcript               |
| ---- | ---------------- | ---------------------------- | ----------- | ------------------------ |
| 1    | 262 ms           | 1,542 ms                     | not clipped | Testing one.             |
| 2    | 278 ms           | 1,408 ms                     | not clipped | Testing one              |
| 3    | 264 ms           | 1,434 ms                     | not clipped | Destiny three.           |
| 4    | 293 ms           | 1,353 ms                     | not clipped | Testing one two          |
| 5    | 267 ms           | 837 ms                       | not clipped | Testing 123              |
| 6    | 265 ms           | 675 ms                       | not clipped | Testing 12               |
| 7    | 267 ms           | 617 ms                       | not clipped | Testing one, two, three. |
| 8    | 268 ms           | 1,328 ms                     | not clipped | Testing ain't five.      |
| 9    | 255 ms           | 395 ms                       | not clipped | One, two, three, ten.    |
| 10   | 297 ms           | 1,517 ms                     | not clipped | Testing when             |

**0 of 10 clipped by us; 10 of 10 had leading silence.** Speech began a
median 1.35 s after the press, so the mic had been live for about a second
before the first word. That is far later than "at the same moment". The two
transcripts without "testing" are not mic clips:

- **Take 3** ("Destiny three."): speech began 1.43 s after the press, so only
  about 0.6 s of it fit in the 2 s clip. The _end_ was cut by the save
  window, not the start. Round 2 saves 4 s.
- **Take 9** ("One, two, three, ten."): the earliest speech start (395 ms
  after the press, 130 ms after the mic was live). Transcribed alone, its
  first second reads "Yeah.", so something other than "testing" was said
  first.

So round 1 says nothing about the ≤ 1-in-50 gate: it never had a speaker
start inside the ~260 ms before the mic is live. The cause is plain timing,
not a measured clip. Without a pre-roll, any speech that starts less than
`T_show` + open time after the press is lost; that is about 260 ms today.
Round 2 gives a clear cue before each take and measures this directly.

## What round 1 does not cover

- macOS 14 and 15 (only a macOS 26 Mac is available).
- An external keyboard, including Ctrl+Opt on a non-Apple one.
- The F5 dictation key.
- Sleep/wake (10 cycles with both taps delivering).
- Finder (TextEdit only), fn+F1..F12 in full (only F11, F1 and one F10).
- 100 separate chords (one long hold instead), and 50 clipping takes (10).
- Secure Input (RS-14).
- Handy's tap reports only its own hotkeys, so `events.jsonl` has every raw
  event from the fork's tap but only hotkey events from Handy's.

## Round 2

`./scripts/wb-spike-fn.sh --round 2`, about 5-8 minutes. Results go to
`~/wb-bench/results/rs01/<timestamp>-r2/`. It runs `T_show` = 100 ms
throughout (`Timings::with_show_ms`) and the fork tap without `{FN}` in the
blocking set (`Grammar::with_fn_block(_, false)`; fn+Space and Esc are still
blocked).

- **Part A, fn not blocked:** the Globe setting must read Do Nothing (the
  tool checks, and tells the owner to set it). Then quick taps ×5,
  double-taps ×5 and holds ×5 (did anything pop up?), and fn+F11 ×5 plus
  fn+←/→ ×5, one per press (usual function? orange dot?). It asks once
  whether F11 / fn+F11 normally does anything.
- **Part B, first words:** 10 cued takes per variant. (a) `T_show` = 100 ms,
  cold open. (b) `T_show` = 100 ms, prepared input unit; the owner is asked
  whether the orange dot shows while the unit is only prepared, before and
  after the takes. (c) pre-roll was dropped, because it needs the input
  running while idle (DEC-53). Every take is judged as above and transcribed
  automatically.

DEC-53 in R5 records the choice once round 2 is in.

### Round 2 results (2026-10-30)

Run `20261030-131006-r2` (an earlier aborted start, `-130802-r2`, is
ignored). macOS 26.6.2, M5 Max, built-in keyboard and mic, `T_show` = 100 ms,
bare fn press not blocked.

- **Part A, Globe = Do Nothing (macOS read 0), fn not blocked:** quick taps,
  double-taps and holds all behaved (5/5 each; the rest were invalid, mostly
  "quick taps" of 100-164 ms, which count as presses at 100 ms). Nothing
  popped up in any block. fn+F11 showed the desktop as usual and fn+←/→
  jumped to top and bottom. So with Do Nothing, blocking the fn press buys
  nothing (DEC-44 update).
- **Chords and the orange dot:** failed, but not because of blocking. As in
  round 1, the owner held fn first and pressed every chord key during one
  long hold, the first key 1.09 s after fn-down (fn+click, then PageUp/Down,
  Home/End, F11). By then the take had started, so the dot showed; and under
  the old rule 2 (keys after `T` pass through) the take would have pasted.
  Holding fn before the key is how this owner types fn chords, so it is a
  real pattern, not a test artefact. Fix: DEC-54, a chord at any time in a
  push-to-talk press drops the take; changed in `grammar.rs` (rule 2) with
  the scenario "key after T is still a chord". A brief dot for late chords
  remains; fluent chords (key within `T_show`) stay dark.
- **First words** (10 cued takes each, all transcribed with "testing"):

  | variant           | fn → first audio p50 / p95 | clipped by us | speech began after fn (p50) |
  | ----------------- | -------------------------- | ------------- | --------------------------- |
  | (a) cold open     | 211 / 216 ms               | 0 / 10        | 1162 ms                     |
  | (b) prepared unit | 129 / 140 ms               | 0 / 10        | 808 ms                      |

  The owner saw no dot while the unit was only prepared, the dot during each
  take, and it gone after. Decision (DEC-53): prepared unit, `T_show` stays
  150 ms (≈ 180 ms expected). Speech still began ≥ 635 ms after the press,
  so the ≤ 1-in-50 clipping gate is still not really stressed; S-01's
  acceptance run keeps it.

- Both taps healthy at all 14 checks.

Still open for S-01 and later: macOS 14/15, Finder, an external keyboard,
the F5 key, sleep/wake, Bluetooth/USB mics, 50-take clipping, and "Start
Dictation" with fn unblocked.
