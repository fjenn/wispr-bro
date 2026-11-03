# RS-02: tap health after sleep, revoke and re-sign (partial, scripted)

**Status:** the parts that need neither a person nor a signed build were run
2026-11-02 by the S-02 agent (MacBook Pro Mac17,6, M5 Max, macOS 26.6.2).
Everything R5 lists with a signed build, sleep, the Accessibility switch or a
re-signed install is **queued for the Mac wave** (procedure below; S-02
built the code it exercises). Not run: (a) sleep ×10, (b) revoke/re-add,
(c) re-signed install, (d) lid close with an external display.

## What S-02 built for it

- `fork/shortcut/health.rs`: watches **both** taps (the fork's own and
  Handy's handy-keys manager). No polling of the tap-enabled query
  (handy-keys #1827); a check is a **probe tap** (a filtering
  `CGEventTapCreate`, never scheduled, destroyed at once), run on: the two
  axwatch notifications (`com.apple.accessibility.api` distributed,
  `com.apple.tcc.access.changed` Darwin), system wake, display wake, session
  active, the main window becoming key, every take the bar shows, a missed
  release (stuck guard), a tap whose stream ended, and a 10 s heartbeat that
  only reads `AXIsProcessTrusted` while healthy (and probes while lost).
- After a failed probe that followed a notification it re-checks at
  +0.3/1/2.5/4.5 s (TCC can lag the switch). When the probe passes again both
  taps are **rebuilt** (fresh `KeyboardListener` / `HotkeyManager`, same
  hotkeys), so no relaunch is needed; after three failed rebuilds the Fix
  sheet offers "Relaunch to finish" (the RS-02 fallback). Wake, display wake
  and session-active also rebuild healthy taps (VoiceInk's triggers), except
  during a take.
- Loss is shown on the bar (Fix → Fix sheet), the tray badge/item, a Hub
  banner, and the S-18 notification (after onboarding only).

## Run 2026-11-02 (scripted, no GUI)

| Check                                                                                                                                                                                                                                                                                               | Result                                                                                             |
| --------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | -------------------------------------------------------------------------------------------------- |
| Does a privacy-record edit post `com.apple.tcc.access.changed` (the Darwin notification axwatch observes, which macOS 15+ does not withhold from unsigned builds)? `scripts/wb-s02/rs02-notify.sh 10` resets Chess's Accessibility record (never granted, so nothing changes) under `notifyutil -1` | **10/10**, notification seen ≤ 64–106 ms after `tccutil` returned (50 ms polling resolution)       |
| The probe tap returns and leaves nothing behind                                                                                                                                                                                                                                                     | `fork::shortcut::tests::health::the_probe_tap_answers_without_side_effects` passes in `cargo test` |
| No polling of the tap-enabled query anywhere in `src-tauri/src/fork/`                                                                                                                                                                                                                               | enforced by a test (`the_probe_is_a_create_and_destroy_never_a_poll_of_an_existing_tap`)           |
| Health state machine per tap (Healthy → Lost → Recovering → Healthy), re-check burst, rebuild retries, relaunch after 3 failures, silent wake refresh                                                                                                                                               | 12 unit tests (`fork/shortcut/tests/health.rs`)                                                    |

Interpretation: the Darwin notification is posted for a reset and arrives
fast, so (b) "loss detected within 2 s" should hold in any build; whether the
distributed `com.apple.accessibility.api` arrives in the **signed** build is
one of the queued checks (it only adds a second wake; either one triggers the
probe).

## Queued for the Mac wave (signed build, `MAC-TEST-SCRIPT.md` S-02)

1. Sleep 10 min, wake, press fn within 2 s; ×10. Log lines
   `fork hotkeys health: Wake → probe ok` and `tap rebuilt` (both taps).
2. Switch Wispr Bro off in Accessibility while running, press fn; loss within
   2 s (bar message, tray badge). Switch on: `running again` for both taps
   within 5 s, fn works, no relaunch. Repeat with **removing** the row (−)
   and re-adding it.
3. Install a re-signed build over the running one; loss within 2 s; Reset
   and re-grant from the Fix sheet; recovery within 5 s (or "Relaunch to
   finish").
4. Lid closed on an external display: wake, fn works.
5. Confirm both notifications arrive in the signed build
   (`fork hotkeys health: AccessibilityChanged` / `PrivacyChanged` at debug
   level).
