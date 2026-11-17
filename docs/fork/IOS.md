# Wispr Bro for iPhone: how the same app builds for iOS

The iPhone app is this crate and this React UI. Only what iOS forces is
different, behind `#[cfg(target_os = "ios")]`; the iOS-only code is in
`src-tauri/src/fork/ios/`. Desktop builds are unchanged.

## Build and run

```bash
bun run wb:ios:sim       # debug build for the simulator → src-tauri/gen/apple/build/arm64-sim/Wispr Bro.app
bun run wb:ios:device    # debug build for an iPhone (automatic signing, team 446XT9839S)
xcrun simctl install <sim> "src-tauri/gen/apple/build/arm64-sim/Wispr Bro.app"
xcrun simctl launch <sim> com.wisprbro.ios
# plain cargo:
. src-tauri/ios/env.sh && SDKROOT=$(xcrun --sdk iphonesimulator --show-sdk-path) \
  cargo check --target aarch64-apple-ios-sim     # in src-tauri
```

- Config: `tauri.conf.json` ← `tauri.fork.conf.json` ← `tauri.fork.ios.conf.json`
  (identifier `com.wisprbro.ios`, version, team, iOS 17.0, no sidecar).
  `Info.ios.plist` is merged into the app's Info.plist on every build
  (microphone text, background audio, Live Activities, `wisprbro://`, the
  scene manifest). Entitlements: `gen/apple/handy_iOS/handy_iOS.entitlements`
  (App Group `group.com.wisprbro.ios`).
- The Xcode project is generated: edit `src-tauri/gen/apple/project.yml`, then
  `cd src-tauri/gen/apple && xcodegen generate`. The app target compiles the
  Swift session code from `src-tauri/ios-native/` (`WBShared`, `WBSession`,
  `WBActivity`) and embeds the `WBKeyboard` and `WBWidgets` extensions; the
  Rust library (`libapp.a`) implements `ios-native/WBCore/wb_core.h`.
- `src-tauri/ios/bin/` holds two shims that go first on `PATH` for iOS builds
  (`env.sh`, and the Xcode "Build Rust Code" phase): `swift` (swift-rs needs
  SwiftPM's native build engine on Swift 6.4) and `xcrun` (the tauri CLI wants
  a simulator runtime with exactly the SDK's version). Each explains itself.
- Icon: `icons-fork/src/app-icon-ios.svg` (the Mac artwork, full-bleed).

## What is the same

Speech engine and models (catalog, starter, policy, pinned downloads, VAD,
chunking), the text pipeline, History, stats, dictionary, snippets, style,
settings, i18n, and every command the Hub pages use.

## What iOS swaps

| Mac | iPhone |
| --- | --- |
| `main.rs` | `fork::ios::start` (`#[tauri::mobile_entry_point]`) |
| Speech engine in a worker process | The same engine and protocol on two threads of the app (`engine_supervisor`: `Worker::spawn` / `worker::spawn_in_process`); backends registered once for the process (`audio_toolkit/cpu_engine.rs`) |
| Recorder (cpal) → capture sink | Swift session → `wb_take_feed` → the same capture sink (`fork/ios/core.rs`) |
| Paste / AX insertion | `fork::insert::deliver_to_keyboard`: the text is returned to Swift, the keyboard inserts it |
| `handy-keys`, `enigo` | `crates/wb-ios-handy-keys`, `crates/wb-ios-enigo` under the same names (`extern crate … as` in `lib.rs`): compile, do nothing |
| Tray, app menu, global-shortcut plugin | `fork/ios/stubs/` (path-swapped modules) |
| Window sizing, always-on-top, … | no-ops (`fork/ios/compat.rs`) |
| Updater, autostart, single instance, macOS permissions plugins | not loaded |
| `llama-server` sidecar, Apple FM | not started: `fork_llm_status` is `unsupported`, cleanup reports `not_ready`, takes get the deterministic text and style |
| Hotkeys, Flow Bar, transforms, Command Mode, Scratchpad, context awareness, auto-learn, Hub parking | not started (`fork::on_ready`) |
| App-delegate life cycle (tao) | UIKit scenes, bridged in `fork/ios/scene.m` (required by the iOS 27 SDK) |

## The take API and the Swift side

`fork/ios/core.rs` exports `wb_core_init`, `wb_core_status_json`,
`wb_take_start|feed|finish|cancel`, `wb_string_free`. `wb_take_finish` runs
what `TranscribeAction::stop` runs after a recording stops.

`fork/ios/host.rs` calls the Swift session's C functions
(`ios-native/WBSession/wb_ios.h`). `fork/ios/swift_stubs.c` defines the same
functions weak, so the library also links into an app without the Swift
sources; with them, the Swift definitions win. Nothing is switched.

Commands for the page (`fork/ios/commands.rs`, called through `invoke`):
`ios_session_state`, `ios_start_session`, `ios_end_session`,
`ios_setup_state`, `ios_request_mic`, `ios_open_settings`,
`ios_get|set_session_timeout`, `ios_get|set_auto_return`, `ios_core_status`.
The two session settings live in the App Group defaults suite
`group.com.wisprbro.ios` (`wb.session.timeoutSeconds`, `wb.session.autoReturn`).

## Check without a microphone

```bash
SIMCTL_CHILD_WB_IOS_SELFTEST_WAV=/abs/en.wav,/abs/nl.wav \
SIMCTL_CHILD_WB_IOS_SELFTEST_REPEAT=2 xcrun simctl launch <sim> com.wisprbro.ios
```

Each 16 kHz mono WAV goes through the C API as a take; results are logged
(`ios selftest:`) and written to `<app data>/ios-selftest.json`.

## Models on a simulator or a phone

The app downloads its models itself (pinned revision and sha256) into
`Library/Application Support/com.wisprbro.ios/huggingface` in its container.
To avoid a download on a simulator, copy a model's
`huggingface/hub/models--…` folder from the Mac app's data
(`~/Library/Application Support/com.wisprbro.desktop/`) into that folder
before the first launch (`xcrun simctl get_app_container <sim> com.wisprbro.ios data`;
`cp -c` clones without using disk space).
