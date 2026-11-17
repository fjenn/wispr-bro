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

| Mac                                                                                     | iPhone                                                                                                                                                                                                                                            |
| --------------------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `main.rs`                                                                               | `fork::ios::start` (`#[tauri::mobile_entry_point]`)                                                                                                                                                                                               |
| Speech engine in a worker process                                                       | The same engine and protocol on two threads of the app (`engine_supervisor`: `Worker::spawn` / `worker::spawn_in_process`); backends registered once for the process (`audio_toolkit/cpu_engine.rs`)                                              |
| Recorder (cpal) → capture sink                                                          | Swift session → `wb_take_feed` → the same capture sink (`fork/ios/core.rs`)                                                                                                                                                                       |
| Paste / AX insertion                                                                    | `fork::insert::deliver_to_keyboard`: the text is returned to Swift, the keyboard inserts it                                                                                                                                                       |
| `handy-keys`, `enigo`                                                                   | `crates/wb-ios-handy-keys`, `crates/wb-ios-enigo` under the same names (`extern crate … as` in `lib.rs`): compile, do nothing                                                                                                                     |
| Tray, app menu, global-shortcut plugin                                                  | `fork/ios/stubs/` (path-swapped modules)                                                                                                                                                                                                          |
| Window sizing, always-on-top, …                                                         | no-ops (`fork/ios/compat.rs`)                                                                                                                                                                                                                     |
| Updater, autostart, single instance, macOS permissions plugins                          | not loaded                                                                                                                                                                                                                                        |
| `llama-server` sidecar                                                                  | The same llama.cpp (b11624) and its server code linked into the app (`crates/wb-ios-llm`), started as a listener thread by `fork/llm/inprocess.rs`; supervisor, transport, client, prompt, guards and deadlines are the Mac's. Apple FM: not used |
| Hotkeys, Flow Bar, transforms, Command Mode, context awareness, auto-learn, Hub parking | not started (`fork::on_ready`)                                                                                                                                                                                                                    |
| Scratchpad: a floating note window, a shortcut, a bar button                            | The same notes (store, tables, `fork_scratchpad_*` commands) in a tab and a full-screen editor (`src/fork/ios/IosScratchpad.tsx`, `IosNote.tsx`); no window is ever built (`fork_scratchpad_new` only creates the note)                           |
| App-delegate life cycle (tao)                                                           | UIKit scenes, bridged in `fork/ios/scene.m` (required by the iOS 27 SDK)                                                                                                                                                                          |

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
`ios_get|set_session_timeout`, `ios_get|set_auto_return`, `ios_core_status`,
`ios_take_start|stop|cancel|state`.
The two session settings live in the App Group defaults suite
`group.com.wisprbro.ios` (`wb.session.timeoutSeconds`, `wb.session.autoReturn`).

## The tabs, My Voice and Scratchpad

Home · History · My Voice · Scratchpad · Settings (`src/fork/ios/nav.ts`).
My Voice (`#/hub/dictation/voice`) is three rows: Dictionary, Snippets and
Style, each with one line of what is there now. A row pushes its page,
which keeps its Hub location (`#/hub/dictionary`, …); Back, a swipe in from
the left edge or the tab again returns to the rows. Style is nowhere else.

Scratchpad is the Mac's feature on the same data: a list (pinned first,
search, a row menu to pin or delete) and a full-screen editor
(`#/hub/scratchpad/<id>`) with 500 ms autosave, text size, Copy, Share (the
web share sheet), a word count, Polish and Record. A note left empty is
removed when the editor closes.

- **Dictating into a note** is the Wispr Bro keyboard, as in any field.
- **Record** is a take the app starts itself: `ios_take_start` →
  `wb_ios_app_take_start` (`WBSessionCAPI.swift`) →
  `WBSessionController.startAppTake`, which starts the session when none is
  running and then the same take a keyboard command starts. Its text is
  kept for the app (`WBAppTake`) instead of being published to the
  keyboard; the page asks `ios_take_state` (phase, live level, text) every
  90 ms while the take runs and puts the text at the note's caret
  (`noteText.ts`). The session stays on afterwards, as after any take.
- **Polish** is the Mac's command (`fork_scratchpad_polish`, the Polish
  transform on the cleanup model). On the phone it first loads the model
  (up to 90 s, when memory allows): a transform's own start budget is
  2.5 s. Without the model on the phone the button says why (cleanup off,
  still downloading, download failed, not enough memory).

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

In short: on iPhone cleanup is off by default and, when a level is chosen,
runs on Apple's on-device model (the last subsection). The in-process
Muesli engine described first is built into the app but not used on a
phone; the measurements below say why.

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
  iOS refuses GPU work. The engine is started with `-ngl 0 --device none`.
  `-ngl 0` alone is not enough: llama.cpp still hands large matrix
  products to the GPU, which works while the app is in front and fails
  every request (a decode error, the take gets the rule-based text) once it
  is not.
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

**On the iPhone 17 Pro (A19 Pro, 12 GB, iOS 26.6, debug build, WAV
self-test at speaking speed, 2026-11-17):**

| Configuration                               | In front                                                                                                           | In the background (every keyboard take)                                            |
| ------------------------------------------- | ------------------------------------------------------------------------------------------------------------------ | ---------------------------------------------------------------------------------- |
| `-ngl 0`, GPU still used for large products | model ready 1.6 s, instructions cached 1.9 s, 15-word Light 1.3 s                                                  | every request fails (decode error); the take gets the rule-based text after 3-6 ms |
| `--device none` (shipped), 2 threads        | model ready 0.5 s, instructions cached 12.5 s; 15 words and 50 words both run into their deadlines (1.75 s, 2.9 s) | the same                                                                           |
| `--device none`, 4 threads                  | model ready 0.3 s, instructions cached 8.5 s; both run into their deadlines                                        | the same                                                                           |

Memory with the speech model and the cleanup model loaded (weights
mapped): footprint 1.45-1.59 GB, 1.8-1.9 GB left before the app's limit,
22 self-test takes without the app being ended. With the weights copied
(no `--no-repack`): 2.63 GB, 0.74 GB left, ended during the second take.

So on this phone Muesli is fast enough only on the GPU, and the GPU is not
available where dictation happens. With 4 or 6 threads a 15-word take was
cleaned in 1.5-1.6 s in the background (3 of 3); a 50-word take never made
its 2.9 s deadline. **The iPhone build therefore has no local model**
(`catalog::select` returns none there; a debug build with
`WB_IOS_LOCAL_LLM=1` still runs it for measurements).

### Apple's on-device model is the iPhone's engine (opt-in)

`crates/wb-apple-fm` builds for iOS (FoundationModels, weak-linked; iOS 26
and Apple Intelligence are checked at run time) and `apple::ROLE` is
`Default` on iPhone: the same prompt, guards, deadlines and fail-open as
the Mac's fallback engine. Settings › Cleanup follows
`fork_llm_apple_status`: not available on this iPhone / Apple Intelligence
is off / Apple's model is still downloading / ready.

The Mac's eval through the shipped path on Apple's model (macOS 26.6,
2026-11-17, 272 items, language mode `owner`), next to Muesli:

|                                         | Damage raw → after guards | Answers raw → after | Filler removal | Guard catch          | p50 / p95     |
| --------------------------------------- | ------------------------- | ------------------- | -------------- | -------------------- | ------------- |
| Muesli, Light (the Mac)                 | → 0.0 %                   | → 0.0 %             | 95.1 %         | 100 %                | 104 / 160 ms  |
| Apple, Light, plain reply               | 8.8 % → 0.7 %             | 5.1 % → 0.0 %       | 91.1 %         | 92.0 %               | 494 / 598 ms  |
| Apple, Light, guided `{text}`           | 14.0 % → 1.8 %            | 6.8 % → 0.0 %       | 92.7 %         | 86.8 %               | 660 / 783 ms  |
| Apple, Light, plus rules written for it | 9.9 % → 0.7 %             | 5.1 % → 0.0 %       | 91.9 %         | 92.6 %               | 597 / 705 ms  |
| Apple, Medium                           | → 1.5 %                   | → 0.0 %             | 92.7 %         | 86.7 % (gate failed) | 649 / 1243 ms |

Apple's model answers dictated questions and drops content far more often
than Muesli; the guards catch most of it, not all (2 of 272 takes inserted
with a word lost). Guided generation and an extra block of rules for this
model did not change that, so neither is shipped. It does not meet the
Mac's bar (damage 0.0 %, guard catch 100 %, filler about 95 %), so
**cleanup stays None by default on iPhone**; Light and Medium are an
opt-in and the page says what to expect.

On the iPhone 17 Pro, app in front: Apple's model available (context 4096),
15 words in 0.8-1.0 s, 50 words in 1.4 s, no rate limit, footprint
unchanged (1.17-1.24 GB, the speech model). **Not measured: the same with
the app in the background (every keyboard take), and a 50-take run for
rate limits**; the phone locked before the app could be put behind another.

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

## Automatic sync

Pair once, then the Mac and the iPhone stay in step by themselves whenever
both apps run on the same local network. The file route above stays for
everything else. Shared Rust in `src-tauri/src/fork/sync/`; the merge is
the same one (`sync::merge::merge`).

| Piece         | What it does                                                                                                                                                                                            |
| ------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `net.m`/`.rs` | Network.framework, one file for both platforms: the Mac advertises `_wisprbro-sync._tcp` (Bonjour, `includePeerToPeer`), the phone browses and connects; whole messages on plain TCP. It knows no keys. |
| `secure.rs`   | Pairing (SPAKE2 on the Mac's 6-digit code → the pair's 256-bit key), session start (fresh nonces, proofs, HKDF), messages sealed with ChaCha20-Poly1305 under a counter.                                |
| `session.rs`  | The exchange: every 2 s each side looks whether its state (`store::collect`) differs from what it last sent, sends it, and merges what arrives.                                                         |
| `auto.rs`     | The engine: one manager thread; the Mac listens while sync is on, the phone connects to its paired Mac whenever it is found, again after a drop.                                                        |
| `peers.rs`    | Pairings and keys in `sync-peers.json` (user-only file in the app's data folder; never in `fork.db`, which "Export my data" copies).                                                                    |

- **Pairing:** the Mac's switch (Settings › Data and Privacy › Sync with
  your iPhone) makes it findable; on the phone Settings › Sync with your Mac
  › Find my Mac lists it; the Mac shows a code (on any Hub page), the phone
  takes it. A wrong code pairs nobody and the code is spent. The code never
  crosses the wire and a listener learns nothing from the pairing.
- **Afterwards, no taps:** on connect and within a few seconds of any
  change on either side (a word, a snippet, a setting, a take's stats),
  both ways. "Changed" is read from the database, so nothing has to
  announce it. A session that breaks loses nothing: the next one starts
  from both full states and merging twice changes nothing.
- **The iPhone syncs only while its process runs:** the app open, or a
  dictation session on. The page says so. On return to the foreground the
  browser is restarted.
- **Why not TLS-PSK in Network.framework:** a listener takes one
  pre-shared key, and the Mac serves pairing requests and several paired
  phones on one service. The keys and the sealing are therefore in Rust,
  above a plain connection, where they are also tested (wrong code,
  changed, repeated, reordered and replayed messages, a cut session).
- **Not forward secret:** a session's keys come from the pair's long-term
  key and both nonces. Someone who later steals `sync-peers.json` and has
  recorded the local traffic could read it. Unpair and pair again makes a
  new key.
- **Permissions:** `NSLocalNetworkUsageDescription` and `NSBonjourServices`
  in `Info.ios.plist` and the Mac's `Info.plist`. iOS asks at the first
  Find my Mac; macOS 15 and later ask when the switch is first turned on.
- **Privacy promise:** R5 §1 #3; `ci/allowed-hosts.txt` `[peer]`; the
  hosts guard allows local-link APIs only in `fork/sync/` and only the
  listed Bonjour service.
- **Check on one Mac:** `scripts/wb-sync/e2e.sh` (a Mac test-identity build
  of this clone, `com.wisprbro.synccheck`, and the simulator app; Bonjour
  and the connection are the real ones). Debug builds take a fixed pairing
  code from `WB_SYNC_TEST_CODE`.
- **Found on the first real iPhone (2026-11-17):** the Mac paired and the
  phone said "Pairing did not work". The Mac closed the connection right
  after its last pairing message and the close overtook it. A close now
  goes out behind everything already sent (`net.m`), the unpair notice
  waits for the other side to hang up, and a failure names its cause
  (code, time-out, never reached, dropped midway, different versions).
- **Failures name the side and the fix:** the pairing dialog says which
  device refused and what to do: the code did not match, timed out,
  "Allow Local Network for Wispr Bro in Settings", "Update Wispr Bro on
  your Mac" / "on this iPhone" (a version refusal carries the refusing
  side's protocol version, and a first message's `v` is read before the
  rest, so a later version can change everything else and still be told
  apart). The Mac words its own failures (`sync.pairing.macProblems`).
- **The first Local Network question:** a connection the system holds back
  for want of local network access (`waiting`, `kDNSServiceErr_PolicyDenied`)
  is kept for up to 60 s instead of 10: Network.framework goes on by itself
  once the user taps Allow, so the pairing continues. Cancel stops the wait.
- **Logs to diagnose a failed attempt:** both sides log every step under
  `sync:` (Bonjour state, found, a phone asked to pair, why a pairing or a
  first message was refused, why a connection waits or failed as
  `<state> <domain> <code>`). The Mac test build writes them to
  `stdout.log` and `Data/logs/`; the phone's to `Library/Application Support/
com.wisprbro.ios/logs/wispr-bro.log` in its container (`xcrun devicectl
device copy from --domain-type appDataContainer --domain-identifier
com.wisprbro.ios …`).
- **Ships in** the Mac app from 1.0.0-alpha.9 on (installed Macs get it
  through the updater); earlier Mac versions have none of this.
- **Not checked:** a real iPhone with a real Mac, the two permission
  prompts, and the peer-to-peer link without a shared Wi-Fi network.
