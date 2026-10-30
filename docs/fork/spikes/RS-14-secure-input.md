# RS-14: Secure Event Input, results (partial, dev build)

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
