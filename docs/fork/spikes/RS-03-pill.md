# RS-03: pill panel (first click, hover, Spaces, capture, displays, idle memory)

**Status:** done 2026-10-30, unattended, on one Mac. **Result:** first click,
hover, full-screen Spaces, capture hiding and display changes **pass** with
the plain nspanel 2.1.1 + Tolkovo setup. **`_setPreventsActivation:` is not
needed** (DEC-28 updated: not used). Not covered: desktop-to-desktop Space
switches (this Mac has one desktop), system sleep, a real external display,
and the 8 h idle run (see "Idle memory and CPU": a 10 min rest run and a
compressed 100-take run instead).

**Machine:** MacBook Pro Mac17,6 (M5 Max), macOS 26.6.2 (25G83), built-in
Liquid Retina XDR (1728 × 1117 pt at 2×), on battery. Tool:
`src-tauri/crates/wb-spike-pill` 0.1.0, a minimal Tauri 2.11.5 app with one
`fork-pill` panel built per R5 §4.9 on **tauri-nspanel 2.1.1** (crates.io;
the app still pins the git v2.1 branch at 2.1.0): `PanelLevel::Status`,
borderless non-activating style mask, `can_become_key_window: false`,
`hides_on_deactivate(false)`, `accept_first_mouse(true)`, all Spaces +
full-screen auxiliary, a native tracking area (`active_always`,
`mouse_entered_and_exited`), bottom-centre placement on the display under the
cursor, a screen-parameters observer and a 1 Hz cursor-display check. There
is no Handy `recording_overlay` and no Hub window. The app runs as an
Accessory app, like Wispr Bro at rest. Helpers: `tools/sck-grab.swift` (a
ScreenCaptureKit `SCStream` recorder) and `tools/vdisplay.m` (a
`CGVirtualDisplay` 1× display that is "unplugged" when the helper exits).
Raw results stay on the Mac (never committed): `~/wb-bench/results/rs03/`.

Other agents were driving apps on this Mac at the same time (S-04 soak takes
in Mail, Safari and ChatGPT, and a running Wispr Bro build with its own
bottom-centre overlay). The click check therefore logs every app activation
(`NSWorkspaceDidActivateApplicationNotification`), waits for 1.5 s without
one before each click, and types only into its own TextEdit document.

## Results against every RS-03 criterion

| RS-03 criterion (R5 §7)                                           | Result                                                                                                                                                                      |
| ----------------------------------------------------------------- | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| First click acts in 20/20 without focus loss (✕ and ✓, TextEdit)  | **PASS without the private API:** 77/77 clean trials over two runs (✕ and ✓), click handled 54-78 ms after the click, TextEdit kept focus, the next key landed in TextEdit  |
| … with and without `_setPreventsActivation:`                      | With it: 36/36 clean trials pass too. **Toggling it off at runtime breaks the panel** (39/40 clicks then activate the app). DEC-28: not used                                |
| Tooltip within 200 ms in 20/20 hovers, TextEdit frontmost         | **PASS** 20/20 via the native tracking area: tooltip painted p50 54 ms, p95 59 ms, max 62 ms. CSS `mouseenter` fired 0/20 while another app was active                      |
| Re-shows on every Space                                           | **Full screen: PASS** 10/10. **Desktop switch: not run** (one desktop on this Mac)                                                                                          |
| … after wake and display changes, placed correctly on both scales | **PASS**: 1× virtual display plugged ×3 and unplugged ×3, display sleep/wake ×3; follows the cursor within 0.57-0.99 s, back on the 2× display within 0.19-0.24 s of unplug |
| Capture hiding in ≥ 2 of 3 recorders                              | **PASS 3/3**: `screencapture` still, `screencapture -v` video, ScreenCaptureKit `SCStream` all show 0 pill pixels with `set_content_protected(true)` (control: ~9,900)      |
| Pill RSS growth ≤ 50 MB; combined idle RSS ≤ 250 MB; CPU ≤ 0.5 %  | See "Idle memory and CPU"                                                                                                                                                   |

## First click (RT-13, RC-01)

TextEdit holds a document of the tool's own; the tool clicks the pill's ✕
or ✓ (synthetic HID mouse events at the button centre), waits 300 ms, checks
which app is frontmost and whether the pill's app became active or its panel
key, then types "x" and reads TextEdit's text through AX.

| Run (`~/wb-bench/results/rs03/`)           | Setting                                   | Trials | Click acted | TextEdit kept focus | Key landed | Focus moved by the click   |
| ------------------------------------------ | ----------------------------------------- | ------ | ----------- | ------------------- | ---------- | -------------------------- |
| `20261030-1807` (no activation log)        | plain (private API never called)          | 40     | 40          | 40                  | 40         | 0                          |
| `20261030-1830`                            | plain                                     | 37 \*  | 37          | 37                  | 37         | 0                          |
| `20261030-1830`                            | `_setPreventsActivation: YES` at creation | 40     | 37          | 36                  | 36         | 0 by the click †           |
| `20261030-1820` (one process, alternating) | `YES` for the trial                       | 40     | 40          | 40                  | 40         | 0                          |
| `20261030-1820` (one process, alternating) | set back to `NO` after a `YES` trial      | 40     | 38          | 1                   | 1          | **39** (our app, 55-97 ms) |

\* 3 trials skipped because another app was activated within the 1.5 s quiet
window. † 4 trials were disturbed by other agents: 3 times a window of the
other agent's Wispr Bro build covered the pill's spot (the click never reached
the pill and activated that app 62-68 ms later) and once Mail was activated
139 ms after the click. The first `--prevent` run at `20261030-1807` had no
activation log and other agents switched apps during it (the tool then typed
"x" into whichever app was in front); it is discarded.

Click to the webview's `click` handler: 54-78 ms (p50 62 ms) in every
setting, including the synthetic mouse-down/up pair (30 + 20 ms of sleeps).

So on macOS 26.6.2 the non-activating, never-key NSPanel from nspanel 2.1.1
with `accept_first_mouse(true)` already keeps the frontmost app's focus. The
private `_setPreventsActivation:` adds nothing, and turning it off again at
runtime leaves the window in a state where every click activates the app.
**DEC-28 updated:** the fork does not call it. If a later macOS shows focus
loss, set it once at creation and never toggle it.

## Hover (RT-14, RC-03)

The cursor glides from 250 pt above the pill onto its centre (5 moves over
~40 ms), 20 times, with another app frontmost. The native tracking area's
`mouseEntered:` emits `pill-hover`; the webview shows the tooltip and reports
after two animation frames ("painted").

| Measure                               | Result                                                                            |
| ------------------------------------- | --------------------------------------------------------------------------------- |
| Native `mouseEntered:` received       | 20/20, p50 41 ms after the first move                                             |
| Tooltip painted                       | 20/20 ≤ 200 ms; p50 54, p95 59, max 62 ms                                         |
| CSS / DOM `mouseenter` in the webview | **0/20**                                                                          |
| Frontmost app during the hovers       | TextEdit 2, Safari 11, Mail 7 (other agents' apps; the pill app was never active) |

The frontmost app was not always TextEdit because other agents brought
their apps forward; what matters for RC-03 is that the pill's app is inactive,
which held in 20/20. The screenshots `hover-0.png` and `hover-19.png` show the
tooltip drawn. CSS `:hover` cannot be the hover source: WKWebView gets no
mouse-moved events while its app is inactive. R5 §4.9 already says "hover
comes from a native tracking area"; no CSS fallback is needed.

## Spaces and full screen (A-19)

| Case                                            | Without re-show                                      | With re-show on Space change |
| ----------------------------------------------- | ---------------------------------------------------- | ---------------------------- |
| TextEdit enters full screen (its own Space), ×5 | 5/5 on screen, inside the display, Space change seen | 5/5                          |
| Desktop switch ⌃→ / ⌃←, ×5 each                 | not exercised: one desktop, no Space change          | same                         |

With `can_join_all_spaces` + `full_screen_auxiliary` the pill stays on screen
over the full-screen app without any re-show (screenshot
`fullscreen-plain.png`). The tool did not add desktops to the owner's Mission
Control set-up. The re-show on `NSWorkspaceActiveSpaceDidChangeNotification`
stays in S-03 as a cheap safety net; the desktop switch moves to S-03's
real-Mac acceptance.

## Screen capture (RT-16)

The pill's waveform area is pure magenta; each recorder's frame is searched
for magenta pixels inside the pill's bounds.

| Recorder                                                      | Control (not protected) | `set_content_protected(true)` |
| ------------------------------------------------------------- | ----------------------- | ----------------------------- |
| `screencapture` still (Screenshot app engine)                 | 9,984 px                | **0**                         |
| `screencapture -v` video, frame at 1.5 s (QuickTime's engine) | 9,984 px                | **0**                         |
| ScreenCaptureKit `SCStream`, 15th frame (Zoom, OBS, Teams)    | 9,788 px                | **0**                         |

3 of 3 recorders hide the pill. Zoom, OBS and QuickTime themselves are not
installed; the SCK stream uses the same API they use on macOS 14+.

## Displays, sleep and scale

A `CGVirtualDisplay` (1920 × 1080 at 1×, the private CoreGraphics class
DeskPad and BetterDisplay use) stands in for an external monitor; killing its
helper process is the "unplug". The built-in display is 2×. Three rounds:

| Step                                                  | Result (3 rounds)                                                                                        |
| ----------------------------------------------------- | -------------------------------------------------------------------------------------------------------- |
| Plug, cursor moved onto the 1× display                | pill centred at its bottom, backing scale 1.0, after 568 / 747 / 801 ms                                  |
| Cursor back on the built-in 2× display                | placed correctly, scale 2.0, after 353 / 950 / 988 ms                                                    |
| Unplug while the pill sits on the 1× display          | re-placed on the built-in display after 232 / 240 / 191 ms; one screen-parameters notification each time |
| Display sleep (`pmset displaysleepnow`, 6 s) and wake | 3/3 visible and correctly placed                                                                         |

The 0.6-1.0 s follow time is the 1 Hz cursor-display check, inside S-03's
"follows the cursor's display within 1 s". Not run: system sleep (waking
needs root `pmset schedule` or a person; this Mac was on battery), a real
external display, and a 1× mode on the built-in panel (it offers none between
1280 and 1800 pt).

## Idle memory and CPU (RT-17, §5.2)

**Interim (the 100-take run is still going; this section is updated when
it ends).** The 8 h run was not possible: the Mac was on battery (36 % →
13 % during this session) and would have slept. Two shorter runs instead:

- **Rest, 10 min, no takes** (`20261030-1900-idle-rest`): the pill process
  alone used 0.21 s of CPU in 540 s (**0.04 %**) and held 107 MB RSS, flat.
  The tool's in-app sampler missed the WebKit processes in this run (a bare
  binary has no bundle, so macOS makes the terminal responsible for them);
  the sampler is fixed and an external sampler covers the next run.
- **100 takes in 30 min** (`20261030-1900-idle-takes`; each take: 5 s of
  30 fps level events, 1 s processing, back to resting): pill process +
  three WebKit processes, sampled every 30 s by `ps`. At start 203 MB in
  all; after 23 takes 221 MB.

## What changes in R5

- **DEC-28:** `_setPreventsActivation:` not needed on macOS 26.6.2; the fork
  does not call it. If ever needed, set once at creation, never toggle.
- **§4.9 / S-03:** drop "`_setPreventsActivation:` only if RS-03 shows it is
  still needed"; hover only from the native tracking area (CSS hover fires
  0/20 while another app is active); `set_content_protected(true)` hides the
  pill from all three capture paths, so S-03 can offer "Hide the bar in screen
  recordings and shares"; the 1 Hz check meets the 1 s follow criterion.
- **S-03 real-Mac acceptance** keeps what RS-03 could not do here: a desktop
  switch between two desktops, system sleep and wake, a real external
  display, and the 8 h idle run on the real slice.

## Reproduce

```sh
cd src-tauri/crates/wb-spike-pill
cargo build --release
mkdir -p target/tools
clang -fobjc-arc -O2 -framework Foundation -framework CoreGraphics tools/vdisplay.m -o target/tools/vdisplay
xcrun swiftc -O tools/sck-grab.swift -o target/tools/sck-grab
OUT=~/wb-bench/results/rs03/$(date +%Y%m%d-%H%M)
./target/release/wb-spike-pill click    --out $OUT --n 20            # then with --prevent
./target/release/wb-spike-pill hover    --out $OUT --n 20
./target/release/wb-spike-pill spaces   --out $OUT --n 5
./target/release/wb-spike-pill capture  --out $OUT                   # then with --protected
./target/release/wb-spike-pill displays --out $OUT --n 3
./target/release/wb-spike-pill idle     --out $OUT --hours 8 --takes 100
```

The process needs Accessibility and Screen Recording. `displays` adds and
removes a virtual display and sleeps the display for 6 s per round.
