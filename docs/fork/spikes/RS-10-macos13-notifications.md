# RS-10: macOS 13 baseline and notifications

**Status:** 2026-10-30, **partly done**. The notification half is settled by
reading the plugin's source: **tauri-plugin-notification 2.4.0 fails the
criterion on macOS desktop**, so the row's fallback applies (S-18 uses
`objc2-user-notifications`, ~100 lines). The macOS 13 half **needs a signed
build and a macOS 13 Mac**; neither is available (F-02 is open, the owner's
Developer ID is not on this Mac, and the only Mac here runs macOS 26.6.2).

## What the row asks

Signed build on macOS 13.x (Apple Silicon): launch, permissions,
transcribe-cpp Metal, nspanel pill, `llama-server` Metal,
tauri-plugin-notification 2.4.0 delivery + click callback + action button
("Copy latest") + permission prompt timing (also on 15 and 26). Pass:
everything runs on 13; notifications deliver, the click opens the right
page and the action button runs its action.

## Notifications: tauri-plugin-notification 2.4.0 on macOS

Source read: `tauri-plugin-notification-2.4.0` from crates.io
(`src/desktop.rs`, `src/lib.rs`, `src/mobile.rs`, `Cargo.toml`), compared
with 2.3.3 (the version cached for the app's lockfile). The desktop code is
the same in both.

| Need (RS-10)                  | 2.4.0 on macOS desktop                                                                                                                                                                                                                                    |
| ----------------------------- | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Delivery                      | Yes: `notify_rust::Notification::show()` on a spawned task, after `notify_rust::set_application(identifier)` (in `tauri dev`: `com.apple.Terminal`). notify-rust uses mac-notification-sys, which posts through the deprecated `NSUserNotificationCenter` |
| Click callback                | **No.** Desktop `show()` drops the response; there is no desktop event for a click                                                                                                                                                                        |
| Action button ("Copy latest") | **No.** `register_action_types` and the action types are implemented only in `mobile.rs` (iOS, Android)                                                                                                                                                   |
| Permission prompt timing      | **Not controllable.** Desktop `request_permission()` and `permission_state()` return `Granted` without asking macOS; the system prompt appears whenever macOS decides (first post)                                                                        |

So the plugin cannot meet "the click opens the right page and the action
button runs its action" on macOS, on any macOS version. **Fallback applied:**
S-18 implements macOS notifications on `objc2-user-notifications` 0.3.2
(already in the app's lockfile): `UNUserNotificationCenter`
`requestAuthorizationWithOptions` at the first real need (S-18 exit criterion),
a `UNNotificationCategory` with a "Copy latest" `UNNotificationAction`, and a
`UNUserNotificationCenterDelegate` whose `didReceiveNotificationResponse`
routes the default action (click) to the page and the action identifier to
its action. tauri-plugin-notification is not adopted (no `Cargo.toml` or
capability change for it in S-18).

Not run on this Mac: an empirical delivery/click/action test of the fallback.
`UNUserNotificationCenter` needs a bundled, signed app with a stable bundle
id; an ad-hoc signed test app may work locally but would say little about the
notarized app. S-18's real-Mac acceptance runs it on the signed build (F-02)
on macOS 26 and 15, and on 13 when a macOS 13 Mac is available.

## macOS 13 baseline: needs a signed build and a macOS 13 Mac

Nothing of the macOS 13 half could run here. What it needs, exactly:

- **A signed, notarized build** from F-02 (Developer ID of team `446XT9839S`),
  because Gatekeeper, TCC prompts (Microphone, Accessibility) and
  `UNUserNotificationCenter` behave differently for ad-hoc signed apps.
- **A macOS 13.x Apple Silicon Mac** (or a macOS 13 VM on Apple Silicon,
  e.g. `tart` with a 13.x IPSW; about 15 GB and an hour to set up). Then, on
  13: launch, grant Microphone and Accessibility, one transcription with
  transcribe-cpp on Metal (fall back to CPU on 13 only if Metal fails, per the
  row), the nspanel pill (RS-03's checks: first click, hover), `llama-server`
  on Metal (RS-06), and the notification checks above with the fallback.
- **The same notification checks on macOS 15 and 26** with the signed build.

## What changes in R5

- RS-10 row: outcome recorded (plugin fails on desktop; fallback applied;
  macOS 13 part waits for F-02 and a macOS 13 Mac).
- S-18: R4 parts switch to `objc2-user-notifications` (the fallback); the
  plugin and `@tauri-apps/plugin-notification` are not added.

## S-18 follow-up (2026-10-30)

The fallback was built in S-18 and run on macOS 26.6.2 with an ad-hoc
release bundle in `~/Applications` (`docs/fork/test-runs/1.0.0-alpha.1-M-26-S-18.md`):
delivery, the permission request at the first real need, the click opening
the right Hub page, and the Copy latest, Open and Open Settings buttons all
pass. Still open, as above: the Developer ID build, macOS 15 and 13, and a
click on a notification after the app has quit (Mac test S-18.2).
