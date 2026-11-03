# RS-14: Secure Event Input, results (partial: round 1 dev build, round 2 scripted)

**Status:** partly run 2026-10-30 by the S-01 agent on an unsigned release
build of S-01 (Terminal as the responsible process for Accessibility and
the microphone). Not run: the signed build R5 asks for, iTerm2 (not
installed), the Safari password field, and everything that needs S-02 or
S-03 (Carbon Esc shadow, pill message). **Machine:** MacBook Pro Mac17,6
(M5 Max), macOS 26.6.2 (25G83). Driver: `scripts/wb-s01-keys.swift`
(synthetic CGEvents); holder check: `ioreg -l -w 0 | grep
kCGSSessionSecureInputPID`.

## What was run

Terminal > Secure Keyboard Entry turned on through its menu (UI
scripting), Terminal frontmost, `kCGSSessionSecureInputPID` = Terminal's
PID; only fn (flagsChanged) events were posted while Terminal was
frontmost, never keys. The setting was turned off again afterwards (it was
off before; `defaults read com.apple.Terminal SecureKeyboardEntry` = 0).

| Check (R5 RS-14)           | Result                                                                                                                                                                                                                                                                      |
| -------------------------- | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| fn hold → take             | **works**: 5 of 5 holds gave `Start, Stop` in the app log; in the one trial where the holder PID was confirmed both before and after, and in four where it was confirmed at the start (focus then moved: other agents shared the GUI)                                       |
| double-tap fn → lock       | **works** in 4 of 5 (`Start, Lock, Stop`); the fifth had foreign key events in it (another agent typing) and is not counted. No trial had the holder confirmed both before and after                                                                                        |
| fn+Space                   | not run under Secure Input (no keys may be posted into Terminal; the Safari case was not reached)                                                                                                                                                                           |
| Esc while recording (tap)  | not run (same reason). Expected: no key-down reaches the tap, so Esc does not cancel                                                                                                                                                                                        |
| Esc with the Carbon shadow | **not available in S-01**: the fork owns `cancel`, so Handy's own Carbon cancel shadow is no longer registered (`owns()` in `tauri_impl::register_shortcut`). The fork's shadow is S-02's hook (`secure_input_bindings`). Until then Esc does not cancel under Secure Input |
| Ctrl with fn held          | not run                                                                                                                                                                                                                                                                     |
| ⌃⌘V                        | no handler in S-01 (paste-last does not exist yet)                                                                                                                                                                                                                          |
| Globe still suppressed     | not testable with synthetic events (they never trigger the Globe action); RS-01 covers it with real keys                                                                                                                                                                    |
| Pill message               | S-02 / S-03                                                                                                                                                                                                                                                                 |

## Findings

- FlagsChanged events keep reaching the fork's tap while Terminal holds
  Secure Input, as Handy's `secure_input.rs` notes: the fn grammar (hold,
  double-tap) runs on flags alone.
- S-01 removes Handy's Esc fallback under Secure Input (its Carbon cancel
  shadow) because the fork owns the `cancel` binding. S-02 must add the
  fork's Esc shadow (R5 §4.2 `secure_input.rs` row) or the pill ✕.

## Still to run (signed build, S-02)

The full RS-14 table on the signed build: Safari password field, Terminal,
iTerm2; fn+Space and Esc (tap and Carbon shadow); Ctrl with fn (Command
Mode on/off); ⌃⌘V; Globe suppression with real keys; pill message; idle
Esc in the password field never swallowed.

## Round 2 (2026-11-02, S-02 agent, scripted)

**Driver:** `scripts/wb-s02/rs14-carbon.swift` (own window only, synthetic
`CGEventPost` at the HID level under `~/wb-bench/gui.lock`). A child copy of
the driver holds Secure Input with `EnableSecureEventInput`
(`IsSecureEventInputEnabled` true within 0.2 s); the parent owns a key
window, a listen-only session tap and Carbon `RegisterEventHotKey` hotkeys,
the same API Handy's Carbon shadow reaches through tauri-plugin-global-shortcut.
10 presses per row; machine as round 1.

| Phase | Secure Input | Registered | Tap key-downs | Carbon fired | Window got Esc | Tap fn flags |
| --- | --- | --- | --- | --- | --- | --- |
| A | off | Esc | 10/10 | 10/10 | 0/10 (Carbon consumed it) | – |
| A2 | off | nothing | 10/10 | – | 10/10 | – |
| B | **on** | Esc | **0/10** | **10/10** | 0/10 | – |
| C | **on** | nothing (idle) | 0/10 | – | **10/10** | – |
| D | **on** | ⌃⌘V | 0 | **10/10** | – | – |
| E | **on** | – | – | – | – | **10/10** |

**Findings**

- The Carbon shadow works under Secure Input: Esc and ⌃⌘V fire 10/10 while
  the tap sees no key-down (B, D). R5's fallback (✕ as the only cancel) is
  not needed; §9's Esc criterion holds through the shadow.
- An unregistered Esc reaches the focused window under Secure Input (C): as
  long as S-02 registers Esc only during a take, an idle Esc in a password
  field is never swallowed. While registered, Carbon consumes the key (A, B),
  so the take-only window matters.
- Without Secure Input both the tap and Carbon report a registered key (A):
  at the edges of an episode (Handy shadows only after 3 s sustained and
  drops them within a 1 s poll) one press can arrive twice. S-02's engine
  drops the second report within 300 ms (`SHADOW_DEDUPE_MS`, tested).
- fn flag changes keep reaching the tap (E), as in round 1.

**S-02 as built on this:** `fork::shortcut::secure_input_bindings` (the hook
in Handy's `reconcile_fallback`) shadows the fork's keyed bindings as
`fork:<action>` and Esc as `fork:cancel` only while a take records or is
processed; `on_shadow_event` (hook in `handle_shortcut_event`) routes them to
the engine. The bar names the holder ("Terminal has Secure Input on ·
fn+Space can't lock · Esc or ✕ cancels") for 5 s at the start of a take.

**Still queued for the Mac wave (signed build, real keys):** Safari password
field, Terminal Secure Keyboard Entry and iTerm2 as holders with the real app:
fn hold and double-tap, fn+Space ignored with the notice shown, Esc cancels
(tap and shadow), Ctrl with fn (Command Mode on/off), ⌃⌘V, Globe still
suppressed (real fn key), idle Esc in the password field not swallowed. See
`MAC-TEST-SCRIPT.md` S-02.
