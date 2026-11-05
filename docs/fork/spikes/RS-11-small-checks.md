# RS-11: small checks (mic loss, permission revoke, mouse buttons, ⌥ characters)

**Status:** done 2026-10-30, unattended (no owner session). **Result:** mic
loss, mouse buttons and "Transforms off swallows nothing" **pass**. Two
criteria fail as written and their fallbacks or a new decision apply:
**a permission revoke cannot be detected mid-take** (macOS does not cut a
running take; DEC-55 changes S-08), and **⌥ bindings swallow ¡ ™ ø ß µ when
Transforms are on** (row fallback: warn in the Transforms toggle and suggest
⌃⌥ alternatives).

**Machine:** MacBook Pro Mac17,6 (M5 Max), macOS 26.6.2 (25G83), US layout,
built-in mic only (no USB mic, no AirPods; an iPhone Continuity mic is listed).
Tool: `src-tauri/crates/wb-spike-small` 0.1.0 (its own workspace and pruned
lockfile; handy-recorder 0.1.0 and handy-keys 0.3.4, the app's versions).
Raw results stay on the Mac (never committed): `~/wb-bench/results/rs11/`.

Other agents were driving apps on this Mac at the same time (S-04 soak takes
in Mail, Safari, ChatGPT and a running Wispr Bro build). The tool types only
into its own TextEdit document, and only after checking that TextEdit is
frontmost with that document focused.

## Short version

| RS-11 criterion (R5 §7)                  | Result                                                                                                                                                                                                                          |
| ---------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Mid-take mic loss keeps the audio so far | **PASS** 10/10. Chosen mic removed: audio kept up to the loss (3.03-3.04 s of 3.00 s), `DeviceLost` 32-44 ms later. Default mic removed: kept 3.03-3.14 s, `DeviceLost` 1.3-3.4 s later. 0 dropped frames                       |
| Permission revoke detected, audio kept   | **Audio kept: PASS** (the take keeps recording real audio). **Detected: FAIL** — no recorder error, no status change; macOS lets the app keep the mic "until it is quit". DEC-55                                                |
| Mouse buttons captured                   | **PASS** 45/45: middle, back and forward, alone and with ⌥, recorded as `mousemiddle`, `mousex1`, `mousex2`; a binding made from the recording fires 15/15                                                                      |
| Transforms off: nothing swallowed        | **PASS** 50/50 (two runs): ⌥1 ⌥2 ⌥O ⌥S ⌥M type ¡ ™ ø ß µ with the fork's tap and Handy's manager running                                                                                                                       |
| (row) Transforms on: ⌥ characters        | **Swallowed** 0/25 with ⌥ keys bound in the fork's blocking set, 0/25 through Handy's manager. Fallback applied (S-21/S-22): warn in the toggle, offer ⌃⌥1 ⌃⌥2 ⌃⌥O; with those bound, ⌥ characters land 25/25 and ⌃⌥1 is caught |

## Mic loss (RC-06)

There is no USB mic or AirPods on this Mac, so a CoreAudio aggregate device
that wraps the built-in mic stands in for one. Destroying it mid-take removes
the device from the HAL the way an unplug or a Bluetooth drop does (device
gone, `kAudioDevicePropertyDeviceIsAlive` false). Each take: open
handy-recorder at 16 kHz mono (`RecorderConfig::speech()`), record 3 s, destroy
the device, wait 2 s, `stop()`. Five takes per case.

| Case                                                     | Audio kept        | End reason                   | Failure reported after removal | Dropped |
| -------------------------------------------------------- | ----------------- | ---------------------------- | ------------------------------ | ------- |
| The user's chosen mic disappears (private aggregate)     | 3.03-3.04 s (5/5) | `RecorderFailed(DeviceLost)` | 32-44 ms                       | 0       |
| The system default mic disappears (aggregate as default) | 3.03-3.14 s (5/5) | `RecorderFailed(DeviceLost)` | 1,265-3,420 ms                 | 0       |

The audio before the loss is real (RMS 0.001-0.02 per 250 ms window, no
digital silence) and nothing after it is invented. When the default device
went away, macOS switched the default back to the built-in mic, but the open
stream did not follow it: the recorder fails and the take ends with what it
had. The tool put the user's default input back after every take
(`default_input_restored: true`).

What this does not cover: a real USB unplug and a real AirPods drop (no
hardware), and Bluetooth-specific effects (HFP/A2DP switch, the route change
while AirPods stay connected). handy-recorder's own failure tests cover those
paths with fakes.

For S-08: the failure handler fires within ~40 ms for the chosen device and
within ~1-3.5 s for the default device; the pill message ("Mic disconnected,
kept what we heard") should key off `DeviceLost` from the failure handler,
and the take's samples come from `Stopped::sink`.

## Permission revoke (RC-06)

The recorder runs inside a small ad-hoc signed test app
(`WBSpikeSmall.app`, bundle id `com.wisprbro.spike.small`, so it has its own
Microphone permission). The tool answers the real macOS prompt with AX
("Allow" in `UserNotificationCenter`), records, and 3 s into the take turns
the app's switch off in System Settings › Privacy & Security › Microphone by
AX (`WBSpikeSmall_Toggle`). A second variant revokes with
`tccutil reset Microphone`. Afterwards the tool removes the app's entry
(`tccutil reset`) and quits System Settings.

What happened, in both variants:

- macOS showed: "“WBSpikeSmall” will have access to your microphone until it
  is quit. You can choose to quit “WBSpikeSmall” now, or do it on your own
  later." with **Quit & Reopen** and **Later**.
- The running take was not touched: 18.1 s of real audio, no
  digital-silence window, `EndReason::StopCalled`, no recorder failure.
- `AVCaptureDevice.authorizationStatus` (handy-recorder's
  `permission_status()`) read **NotDetermined** for the whole run, before and
  after the grant and after the revoke. In a process that got its access
  through the CoreAudio open (not `requestAccess`), the in-process status is
  stale. (Not checked: whether a fresh process of the same app reads it
  correctly.)
- The `open` call blocked until the prompt was answered (0.8-5.4 s here).

So "audio so far kept" holds trivially, but "revoke is detected" cannot be
met: there is nothing to detect while the app runs. **DEC-55** (new) records
what S-08 does instead: keep the take running; persist the take's audio on
quit (the user may press Quit & Reopen mid-take), so the reopened app can
offer it; the next `open` after a relaunch gets `PermissionDenied` and the
pill shows the error with Fix. S-29a must not poll the in-process
`AVCaptureDevice` status to see a grant made through the prompt (unverified
for a `requestAccess` grant; check in S-29a).

Not run: pressing **Quit & Reopen**. The test app is a plain Rust binary
without an `NSApplication`, so how it is quit says nothing about how Tauri
handles the quit; S-08 tests that on the real app.

## Mouse buttons (RC-07)

Handy's shortcut recorder (`shortcut/handy_keys.rs` `start_recording`) is a
plain handy-keys `KeyboardListener`, and the frontend commits the combo on
key-up. The tool runs that listener and posts other-mouse-button events at the
HID tap over its TextEdit window (button 2 = middle, 3 = back, 4 = forward),
10 times alone and 5 times with ⌥ held.

| Button  | Captured alone | Captured with ⌥ | Recorded as                              | Binding fires |
| ------- | -------------- | --------------- | ---------------------------------------- | ------------- |
| Middle  | 10/10          | 5/5             | `mousemiddle`, `option_left+mousemiddle` | 5/5           |
| Back    | 10/10          | 5/5             | `mousex1`, `option_left+mousex1`         | 5/5           |
| Forward | 10/10          | 5/5             | `mousex2`, `option_left+mousex2`         | 5/5           |

"Binding fires": the recorded string parses back (`Hotkey::from_str`),
registers in a `HotkeyManager`, and the next press of that button produces a
`Pressed` event. Synthetic HID events go through the same event tap as a real
mouse; a real mouse with side buttons was not available.

Note for S-06: handy-keys never blocks mouse events (the `OtherMouseDown` arm
in `platform/macos/listener.rs` does not consult the blocking set), so a
"Press Enter" bound to a mouse button also reaches the app under the pointer
(a middle click in a browser still opens a link in a new tab). S-06 should
say so next to the binding, or suggest a side button.

## ⌥ characters (RC-13)

The tool types ⌥1 ⌥2 ⌥O ⌥S ⌥M (key down for ⌥, the key, ⌥ up, as a keyboard
sends them) into its TextEdit document, 5 times each, in four set-ups. The
fork's own tap runs in every set-up the way R5 §4.8 runs it (handy-keys
`KeyboardListener::new_with_blocking` with `{FN}` always in the set), plus
Handy's `HotkeyManager` (blocking variant) as shipped.

| Set-up                                                              | Characters typed | Fork tap saw the key |
| ------------------------------------------------------------------- | ---------------- | -------------------- |
| Transforms off (no ⌥ bindings)                                      | 25/25, 25/25     | 25/25                |
| Transforms on, ⌥ keys in the fork's blocking set                    | 0/25, 0/25       | 25/25                |
| Transforms on, ⌥ keys registered in Handy's manager                 | 0/25, 0/25       | 0/25, 1/25           |
| ⌃⌥1 ⌃⌥2 ⌃⌥O ⌃⌥S ⌃⌥M bound in the fork's set (suggested alternative) | 25/25, 25/25 \*  | 25/25                |

\* The tool scored 23/25 in both runs because a Tab from another process
landed in the document between two reads; all 25 characters are in the
document (5 of each). With the ⌃⌥ set bound, ⌃⌥1 itself was caught (document
unchanged).

So with Transforms off nothing is swallowed (criterion passes, as DEC-31
registers Transforms hotkeys only when Transforms are on). With them on, every
bound ⌥ key is swallowed by design; users who type ¡ ™ ø ß µ lose those
characters. Fallback applied: the Transforms toggle warns that ⌥1, ⌥2 and ⌥O
replace the characters ¡, ™ and ø while Transforms are on, and offers ⌃⌥
alternatives in one click. The same holds for any other bare-⌥ shortcut (the
⌥S that S-24's dialog proposes replaces ß), so S-24 proposes ⌃⌥S instead.
Not tested: the Carbon shadow path used under Secure Input (R5 §4.8); a
registered Carbon hotkey also consumes its key, so the same warning covers it.

Side finding: when both taps block the same key, the tap created last (here
Handy's manager) sees and swallows it first, so the earlier tap never sees it
(0-1/25). Fork bindings must therefore never also be registered in Handy's
manager (already the plan: the `owns()` hook in R5 §4.8).

## Reproduce

```sh
cd src-tauri/crates/wb-spike-small
cargo build --release
./target/release/wb-spike-small mic-loss    --out ~/wb-bench/results/rs11/<ts>
./target/release/wb-spike-small mic-default --out ~/wb-bench/results/rs11/<ts>
./target/release/wb-spike-small mouse       --out ~/wb-bench/results/rs11/<ts>
./target/release/wb-spike-small option      --out ~/wb-bench/results/rs11/<ts>
./target/release/wb-spike-small perm        --out ~/wb-bench/results/rs11/<ts> [--revoke tccutil]
```

The process needs Accessibility (to post events and read TextEdit).
`mic-default` briefly makes the test aggregate the system default input and
puts the previous default back. `perm` answers the Microphone prompt, flips the
test app's switch in System Settings, and removes the app's entry afterwards.
