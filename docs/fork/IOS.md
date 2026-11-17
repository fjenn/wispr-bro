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
  (App Group `group.com.wisprbro.ios`), written by xcodegen from the
  `entitlements.properties` in `project.yml`: change them there. Without
  the group the keyboard and the app cannot see each other.
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

| Mac                                                                                                 | iPhone                                                                                                                                                                                                                                            |
| --------------------------------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `main.rs`                                                                                           | `fork::ios::start` (`#[tauri::mobile_entry_point]`)                                                                                                                                                                                               |
| Speech engine in a worker process                                                                   | The same engine and protocol on two threads of the app (`engine_supervisor`: `Worker::spawn` / `worker::spawn_in_process`); backends registered once for the process (`audio_toolkit/cpu_engine.rs`)                                              |
| Recorder (cpal) → capture sink                                                                      | Swift session → `wb_take_feed` → the same capture sink (`fork/ios/core.rs`)                                                                                                                                                                       |
| Paste / AX insertion                                                                                | `fork::insert::deliver_to_keyboard`: the text is returned to Swift, the keyboard inserts it                                                                                                                                                       |
| `handy-keys`, `enigo`                                                                               | `crates/wb-ios-handy-keys`, `crates/wb-ios-enigo` under the same names (`extern crate … as` in `lib.rs`): compile, do nothing                                                                                                                     |
| Tray, app menu, global-shortcut plugin                                                              | `fork/ios/stubs/` (path-swapped modules)                                                                                                                                                                                                          |
| Window sizing, always-on-top, …                                                                     | no-ops (`fork/ios/compat.rs`)                                                                                                                                                                                                                     |
| Updater, autostart, single instance, macOS permissions plugins                                      | not loaded                                                                                                                                                                                                                                        |
| `llama-server` sidecar                                                                              | The same llama.cpp (b11624) and its server code linked into the app (`crates/wb-ios-llm`), started as a listener thread by `fork/llm/inprocess.rs`; supervisor, transport, client, prompt, guards and deadlines are the Mac's. Apple FM: not used |
| Hotkeys, Flow Bar, transforms, Command Mode, Scratchpad, context awareness, auto-learn, Hub parking | not started (`fork::on_ready`)                                                                                                                                                                                                                    |
| App-delegate life cycle (tao)                                                                       | UIKit scenes, bridged in `fork/ios/scene.m` (required by the iOS 27 SDK)                                                                                                                                                                          |

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

## Speech runs on the CPU

iOS refuses GPU work from an app in the background (Metal:
`kIOGPUCommandBufferCallbackErrorBackgroundExecutionNotPermitted`), and a
take dictated from the keyboard always runs there: the Metal decode came
back empty, without an error. `transcribe_gpu_disabled_for_host()` is
therefore true on iOS. iPhone 17 Pro (A19 Pro), parakeet-tdt-0.6b-v3 Q8_0,
debug build, WAV self-test: a 10 s take in 0.9-1.2 s (1.5-1.7 s with the
model load), a 56 s take in 6.5-8.7 s, a 7.5 s Dutch take in 0.8-1.0 s,
peak memory 1.35-1.49 GB; the same with the app in the background behind a
live session. (Metal, in front only: 0.25-0.33 s, 1.7-2.5 s, 1.04 GB.)

## Cleanup runs in the app

An app cannot start `llama-server`, so the cleanup model runs in the app's
own process: `crates/wb-ios-llm` is the sidecar's llama.cpp with its server
request code, and `fork/llm/inprocess.rs` (`InProcessLauncher`) answers the
supervisor's "start a server" with a listener thread on a Unix socket in the
container's `tmp/`. Everything above it is unchanged, so the model, prompt,
JSON schema, guards and deadlines are the Mac's (the crate's README has the
byte-for-byte comparison with the sidecar). llama.cpp's ggml and
transcribe-cpp's ggml share the binary because the crate hides all of its
symbols but seven.

- **CPU only**, like speech: keyboard takes run in the background, where
  iOS refuses GPU work. `--device`-independent: the engine is started with
  `-ngl 0`.
- **`--no-repack`**: on the CPU llama.cpp otherwise copies the weights into
  another layout, 1.3 GB of app memory on top of the speech model. With it
  the weights stay a mapping of the model file, which does not count
  against the app. (With the copy the iPhone 17 Pro ended the app during
  the second take: its limit for this app is about 3.4 GB, speech alone
  uses 1.2-1.5 GB.)
- **Threads**: the performance cores (`hw.perflevel0.physicalcpu`).
- **The model follows the session**: it is loaded, and its instructions
  (about 530 tokens, seconds of CPU once) are put into the prompt cache,
  when a session starts and at the start of a take; it is unloaded when the
  session ends, after 15 minutes without a take, and at once on a memory
  warning (`fork/ios/memory.rs`; the speech model stays, takes get the
  rule-based text for a minute). With cleanup set to None it is never
  loaded.
- **Which phones**: 8 GB and more (`catalog::select`); others report
  `unsupported` and Settings says so. Before a load the app must have
  1.4 GB left before iOS's limit (`memory.rs`, `os_proc_available_memory`).
- **Download**: the Mac's pinned downloader, after the speech model. A
  model file copied into the container is verified by its sha256 and used.
- A take that cannot be cleaned in time (model still loading, deadline,
  guard) gets the rule-based text, as on the Mac.

To run the Mac's cleanup eval on this engine:
`WB_LLM_INPROCESS=1 [WB_LLM_DEVICE=cpu] WB_LLAMA_SERVER=in-process … cargo test --features llm-inprocess --lib fork::llm::eval_run -- --ignored`.

## What the page gets from the app

- `ios-session-state` / `ios-setup-state` events whenever the Swift session's
  state or the setup checks change (`commands.rs::watch`, compared every
  250 ms); the page also asks on mount and when it comes to the front.
- `window.__wbKeyboardInset(points)` from UIKit's keyboard notifications
  (`scene.m`), before the keyboard moves: sheets and pages lay themselves
  out above the keyboard in step with it (`useKeyboardInset.ts`).
- `scene.m` also removes the web form bar over the keyboard and paints the
  window and the web view in the launch screen's colour
  (`Assets.xcassets/LaunchBackground`, light and dark).

## Back to the app the user was typing in

After the keyboard opened Wispr Bro (`WBSessionController.returnToHost`):

1. the keyboard named its host (iOS before 26.4), or UIKit's remote keyboard
   state gave it (`WBHostAppObserver`): the host is opened by its link when
   it is in `WBHostAppCatalog`, else by bundle id (`LSApplicationWorkspace`,
   private, on a background queue: on the main thread it blocks the app);
2. otherwise the swipe-back hint stays up.

On the iOS 27.0 simulator the keyboard state arrives without a source or
host bundle, so a cold start from the real keyboard ends on the hint; both
"named host" routes work (`WBReturnTests`). Not yet checked on a phone.

## End-to-end checks in a simulator

`src-tauri/ios-native/e2e/` (xcodegen project, `run.sh`): a tiny host app
(`com.wisprbro.e2e.host`) and XCUITests that drive the host, the real
keyboard and the installed app, with speech from WAV files
(`WB_FAKE_MIC_WAV`, simulator only; the file is read again at every take).

```bash
bun run wb:ios:sim && xcrun simctl install <sim> "src-tauri/gen/apple/build/arm64-sim/Wispr Bro.app"
# once per simulator: finish onboarding and add the keyboard in Settings
# (General › Keyboard › Keyboards › Add New Keyboard › Wispr Bro).
export WB_WAV_DIR=/abs/wavs            # en.wav, nl.wav: `say -o x.aiff …; afconvert -f WAVE -d LEI16@16000 -c 1 x.aiff x.wav`
src-tauri/ios-native/e2e/run.sh <sim> /abs/shots light -only-testing:WBE2ETests/WBKeyboardFlowTests
src-tauri/ios-native/e2e/run.sh <sim> /abs/shots dark  -only-testing:WBE2ETests/WBReturnTests
WB_SCRIPT=/abs/walk.txt src-tauri/ios-native/e2e/run.sh <sim> /abs/shots light -only-testing:WBE2ETests/WBScriptTests
```

The keyboard is a full typing keyboard (letters, numbers and symbols,
`ios-native/WBShared/FullKeyboard`, keys named `wb.key.<character>`) with
the dictation strip above the keys.

`WBKeyboardFlowTests`: cold start from the keyboard, an English take
inserted once, a warm Dutch take with one space before it, cancel, a
correction typed with the letter keys after the dictated text, and
"End session after: Immediately". `WBScriptTests` runs a small script
(tap, type, shot, …; the commands are listed in the file) for walking
screens. `src-tauri/ios/sim-fresh.sh <sim> <folder with huggingface/>`
gives a first launch without a model download.

## Check without a microphone

```bash
SIMCTL_CHILD_WB_IOS_SELFTEST_WAV=/abs/en.wav,/abs/nl.wav \
SIMCTL_CHILD_WB_IOS_SELFTEST_REPEAT=2 xcrun simctl launch <sim> com.wisprbro.ios
```

Each 16 kHz mono WAV goes through the C API as a take; results are logged
(`ios selftest:`) and written to `<app data>/ios-selftest.json` (rewritten
after every take). Self-test takes leave no History row, no stats and no
recording. More switches (all in `fork/ios/selftest.rs`): `…_REALTIME=1`
feeds at speaking speed, `…_LEVEL=light` cleans the self-test's takes at
that level without changing the setting, `…_DELAY`, `…_WAIT_LLM`,
`…_GAP_MS`, `…_THREADS` (debug builds: compare model configurations),
`…_MEMORY_WARNING_AT`. A relative WAV path is read from the app's data
folder; each take reports the cleanup step, the app's footprint and
whether the app was in front.

On a phone:

```bash
xcrun devicectl device copy to --device <udid> --domain-type appDataContainer \
  --domain-identifier com.wisprbro.ios --source /abs/selftest \
  --destination "Library/Application Support/com.wisprbro.ios/selftest"
xcrun devicectl device process launch --device <udid> --terminate-existing \
  --environment-variables '{"WB_IOS_SELFTEST_WAV":"selftest/a.wav","WB_IOS_SELFTEST_REALTIME":"1","WB_IOS_SELFTEST_LEVEL":"light","WB_IOS_SELFTEST_OPEN_URL":"wisprbro://session/start"}' \
  com.wisprbro.ios
xcrun devicectl device copy from … --source "Library/Application Support/com.wisprbro.ios/ios-selftest.json" --destination out.json
```

## Models on a simulator or a phone

The app downloads its models itself (pinned revision and sha256) into
`Library/Application Support/com.wisprbro.ios/huggingface` in its container.
To avoid a download on a simulator, copy a model's
`huggingface/hub/models--…` folder from the Mac app's data
(`~/Library/Application Support/com.wisprbro.desktop/`) into that folder
before the first launch (`xcrun simctl get_app_container <sim> com.wisprbro.ios data`;
`cp -c` clones without using disk space).

## Sync with the Mac

No account, no server: one file, carried by the user, merged on the other
side. The engine is shared Rust (`src-tauri/src/fork/sync/`, both
platforms); its module doc has the rules.

- **What syncs:** dictionary (words, replacements, deletions), snippets, the
  cross-device settings (languages, styles, sounds, retention, greeting
  name, typing speed) and stats. The cleanup level stays per device (it
  decides whether a 1.3 GB model is downloaded and loaded). History text and audio
  never do.
- **Files the importer reads:** a `.wisprbro` sync file (a zip of
  `manifest.json` and `sync.json`); the Mac app's "Export my data" zip, of
  which only `databases/fork.db` and the two settings files are read (so
  the shipped Mac, 1.0.0-alpha.6, syncs to the phone unchanged); a
  Dictionary or Snippets CSV.
- **Merge:** by key, newest `updated_at` wins, deletions are tombstones
  (`sync_tombstones`, written by triggers, migration 0010). Settings are
  last-writer-wins per key (`sync_settings`: last seen value and time; a
  value never changed from the default is not sent). Each other device's
  daily stats are stored as that device's snapshot (`sync_device_stats`)
  and replaced whole on the next import; Home and Insights read
  `all_daily_stats` (this device plus the snapshots) and say so.
- **iPhone:** Settings › Sync with your Mac (`src/fork/ios/IosSync.tsx`).
  "Choose a file" is the document picker (`tauri-plugin-dialog`, which
  copies the file into the app's `tmp`); AirDrop / Files "Open in Wispr
  Bro" arrives through `scene.m` → `wb_sync_file_opened` (`sync/ios.rs`),
  the document types are in `Info.ios.plist`. "Send to Mac" opens the share
  sheet (`sync/ios.m`) with the phone's `.wisprbro` file, plus the two CSVs
  the shipped Mac's import wizards read. The app deletes its copy of an
  imported file (a Mac export holds history and recordings).
- **Mac (this branch):** Settings › Data and Privacy › Sync with your
  iPhone: Import… and Export… of the same file, the same review.
- **Checked in the simulator (2026-11-17):** a Mac export opened through
  the file-URL path (`scene.m`), the review sheet, Merge (dictionary,
  snippets, name and stats arrive; Home says "Includes 1 other device"),
  "Send to Mac" (the share sheet shows a "Wispr Bro Sync File"), and a
  `.wisprbro` opened by the system (`xcrun simctl openurl <sim>
file:///…/x.wisprbro` reaches the app through the document types). A
  synthetic Mac export for this:
  `WB_SYNC_EXPORT_OUT=/abs/data.zip cargo test --lib fork::sync::tests::write_a_mac_export -- --ignored`,
  copied to the container's `Documents/Inbox` and opened with
  `WB_IOS_SELFTEST_OPEN_URL=file://…`.
- **Not checked:** the document picker ("Choose a file"), and AirDrop /
  "Open in" and the share sheet on a real phone.

## Next: automatic sync over the local network (not built)

The merge takes a manifest and a payload, whatever carried them
(`sync::merge::merge`), so a second transport needs no new merge code:

1. **Find:** both apps open; the Mac advertises `_wisprbro-sync._tcp` with
   Bonjour, the phone browses for it (`Network.framework` on both sides).
2. **Pair once:** the Mac shows a six-digit code, typed on the phone; a
   PAKE (SPAKE2) over it yields a long-term key each side keeps in its
   Keychain next to the peer's device id. No code, no connection.
3. **Talk:** a Noise-style encrypted channel on that key; each side sends
   `store::collect`'s payload and merges the other's. Stats and tombstones
   already make repeats harmless.
4. **When:** on app foreground and after a change, while both are open.
   Nothing runs in the background, and never beyond the local network.

It needs: a Mac release with this branch merged; on iOS
`NSLocalNetworkUsageDescription` and `NSBonjourServices` (the system asks
the user for local-network access); a new `[local]` section in
`ci/allowed-hosts.txt` with a `wb-guard-hosts` rule that admits link-local
peers only for the sync module (today the app may contact GitHub and
Hugging Face and nothing else); and a line in the privacy text.
