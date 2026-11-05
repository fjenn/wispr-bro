# Pass 2 review, area E: Notetaker + Platform

**Reviewer stance.** I treated pass 1 (`pass1-E-notetaker-platform.md`) as suspect and re-opened its citations in the repo (`<repo>/`, branch `wispr-bro`, HEAD `7bfb2e2c`) and in the vendored crates under `~/.cargo/registry/src/index.crates.io-1949cf8c6b5b557f/` (written `~cargo/` below) and `~/.cargo/git/checkouts/hf-hub-1a18d389d3190fc4/9918f11/` (written `~hf-hub/`). I did not build or run anything, did not use the network, and did not open `docs/research/evidence/`.

**Tags.** `verified-in-code` = I read the cited line. `inferred` = reasoned from code I read, or from standard Tauri/macOS behavior I did not confirm in this repo. `unknown` = not determined.

---

## Summary

- **Rows checked:** 95 of 95 (E-01 to E-95), plus the network inventory (9 rows) and the 9 release steps. I opened every cited file. Exceptions: E-70 ("81 `pub` fields"), E-81 (component counts, only partly recounted) and E-89 (release-notes content) were checked only for existence and order of magnitude.
- **Evidence errors found:** 27. Most of them are wrong line targets or claims stated more strongly than the code supports. The ones that matter most:
  1. The process tap is **per output device, not global**.
  2. The permission plugin checks **Screen Recording**, not system-audio capture.
  3. Pass 1 says "transcript not in logs" but gives no proof. In fact transcripts are redacted in release builds, while a **settings dump with dictionary words and prompts** is logged at the default Debug level.
  4. `api.deepseek.com` is not an outbound call.
  5. `asset-prefix` has **no effect** on release asset names.
  6. Linux release jobs would **not** fail for lack of Azure secrets.
- **Class/effort changes:** 6. E-02 (EXTEND→BUILD), E-22 (M→L), E-44 (S→M), E-47 (M→L), E-84 (merged into E-10/E-48) and E-52 (HAVE, plus a small TUNE note). E-06 is also flagged because it conflicts with area B.
- **New rows:** 19 (E-201 to E-219).
- **Biggest correction (E-47):** dictation during a meeting is not just a scheduling problem. transcribe.cpp gives a live **stream an exclusive lease on the model**: other runs return `Busy` while it is held. Also, a dictation cancel (Esc) **kills the shared worker and cancels all queued work**, which would include a meeting's work. Running dictation and Notetaker at the same time needs a deliberate engine design (see E-47).

---

## Evidence corrections

| Row | Problem | Correct evidence (path:line) | Fix |
|---|---|---|---|
| E-01, headline 1 | Pass 1 calls the tap "global". It is created for **one output device's UID, stream 0**, with an empty process list marked exclusive (so it taps all processes on that device and excludes none). If audio is routed elsewhere, for example AirPods connected mid-call, the tap does not follow. | `~cargo/handy-cpal-0.18.2/src/host/coreaudio/macos/loopback.rs:93-109` (`initWithProcesses_andDeviceUID_withStream(&processes, device_uid, 0)`, `setExclusive(true)`), `device.rs:727-736` `verified-in-code` | Restate it as a device-bound tap. Add E-206 (device switching). Excluding our own process would need a non-empty process list, which means patching handy-cpal (E-205). |
| E-01 | The handy-recorder line numbers are off, and the stronger point is missing: `open_device` can only resolve **input** devices, so Handy's recorder cannot open the loopback path at all. | `~cargo/handy-recorder-0.1.0/src/backend/cpal/mod.rs:135-143` (`enumerate` → `host.input_devices()`), `:197-212` (`open_device`) `verified-in-code` | System audio needs our own small capture module that calls `handy-cpal` directly and resamples with `rubato` (already a Handy dependency, `src-tauri/Cargo.toml:63`). Do not route it through handy-recorder. |
| E-01 | The README says more than "14.6+": CoreAudio in handy-cpal has a baseline of **macOS 14.2**, and loopback needs 14.6+. Handy works around the baseline by weak-linking CoreAudio. | `~cargo/handy-cpal-0.18.2/README.md:60`; `src-tauri/build.rs:22-27` `verified-in-code` | Gate the feature at runtime with an OS-version check. Calling a 14.6-only symbol on an older OS crashes the app (`inferred`). |
| E-02 | The plugin commands exist, but they wrap `CGPreflightScreenCaptureAccess` and `CGRequestScreenCaptureAccess`, which cover **Screen Recording**. Process taps use the separate system-audio-capture consent (`inferred`; this is the "System Audio Recording Only" list in the R1 §11 pane). | `~cargo/tauri-plugin-macos-permissions-2.3.0/src/commands.rs:17-18,134-138,153-157` `verified-in-code` | The plugin cannot check or request tap permission. See the class change. |
| E-05 | `transcription_coordinator.rs:17-30` is the `RecordingPhase` enum, not the shortcut, tray or action plumbing. | `src-tauri/src/actions.rs:965` (`ACTION_MAP`); default bindings `src-tauri/src/settings.rs:953-990`; tray items `src-tauri/src/tray.rs:500-600`; tray handler `src-tauri/src/lib.rs:285-299` `verified-in-code` | Replace the citation. |
| E-12 | `overlay.rs:619` is `show_streaming_overlay`, not where the NSPanel is created. | `src-tauri/src/overlay.rs:18,27` (`tauri_panel!`), `:452-454` (`create_recording_overlay` via `PanelBuilder`) `verified-in-code` | Replace the citation. |
| E-15 | Pass 1 misses that **long-form non-streaming models produce no live text at all**. `live_decode` returns `None` when the model is long-form. There is also a single global stream route. | `src-tauri/src/managers/transcription/policy.rs:51-59`; `src-tauri/src/managers/transcription.rs:900-906` (one `router`, one `stream_active`) `verified-in-code` | A meeting live transcript needs chunking forced on for every non-streaming model, plus its own stream route. |
| E-17 | Cites the same `RecordingPhase` enum. | Cancel path: `src-tauri/src/utils.rs:102-126` `verified-in-code` | Replace the citation. |
| E-22 | The "worker gap" can be upgraded from `inferred` to verified. Also, the Rust diarize API is **batch-only over the whole recording**, even though the catalog says `"streaming": true`. It refuses to run while a stream holds the model, and the catalog marks it `timestamps: false`. | `src-tauri/src/engine_supervisor/protocol.rs:23-53` (no diarize request); `~cargo/transcribe-cpp-0.4.0/src/diarize.rs:114-127` (`run(&[f32])`, refuses while streaming), `:21-26` (`max_speakers`); `src-tauri/src/catalog/catalog.json:1520-1541` `verified-in-code` | See the class change (L). |
| E-25 | `transcription_jobs.rs:339-340` is the `crashed()` wrapper for panics. Recovery from worker crashes lives in the supervisor. | `src-tauri/src/managers/transcription_jobs.rs:250-253,339-341`; `src-tauri/src/engine_supervisor/mod.rs:8-11` `verified-in-code` | Split the two claims. |
| E-34 | FTS5 was tagged `inferred`. | `~cargo/libsqlite3-sys-0.35.0/build.rs:132` (`-DSQLITE_ENABLE_FTS5`) `verified-in-code` | Retag as verified. |
| E-39 | The import row misses a limit: **Opus, ogg/oga and webm are not decodable** (the comment says this). | `src/components/settings/history/transcribeFiles.ts:6-8` `verified-in-code` | Note it, and don't pick Opus for meeting storage (E-44). |
| E-44 | The R1 reference "§4 rows" describes dictation rows. R1 never shows whether meeting audio is kept. | R1 §4 vs §14 | Change the Wispr behavior cell to "`unknown` for meetings". |
| E-47 | The evidence is right but incomplete. It misses the decisive constraints listed in the next column. | `~cargo/transcribe-cpp-0.4.0/src/model.rs:87-101`, `src/session.rs:274-293`; `src-tauri/src/engine_supervisor/supervisor.rs:229-234,471-484`; `src-tauri/src/managers/transcription.rs:908-914`; `src-tauri/src/utils.rs:116-117` `verified-in-code` | Decisive constraints: (a) a stream holds the model's compute lease and other runs get `Busy`; (b) `cancel()` bumps a global epoch and kills the in-flight worker; (c) the Esc cancel calls it. See the class change. |
| E-51 | The version display was not cited. | `src/components/footer/Footer.tsx:13-26,42` (`v{version}`; on error it falls back to a hard-coded `"0.1.2"` at `:22`) `verified-in-code` | Add the citation. Replace the fallback with something honest. |
| E-52 | Pass 1 says HAVE without a caveat. If the user has turned update checks off, the tray item does **nothing** on click. It is removed from the menu only when updates are forced off by environment variable. The tray also shows a disabled version item. | `src-tauri/src/lib.rs:294-299`; `src-tauri/src/tray.rs:503,595-597` `verified-in-code` | Note it as a small TUNE. |
| E-53 | The check runs only once onboarding is done, because the Footer renders only in that state. | `src/App.tsx:453` `verified-in-code`; "runs on every launch in the hidden main window" `inferred` | Add the note. |
| E-58 | "Windows/Linux jobs would fail without Azure secrets" is half wrong. Linux jobs need no Azure secrets and would publish to our release. Only Windows fails, through `signCommand` and the trusted-signing-cli steps. | `src-tauri/tauri.conf.json:77`; `.github/workflows/build.yml:166-176,460-462` `verified-in-code` | Correct the reason. Trimming the matrix is still right. |
| E-58, step 4 | `asset-prefix` is used only by the "Patch asset name pattern" step. That step runs only when `asset-name-pattern` is non-empty, and `release.yml` never passes it. The other use, upload-artifact names, is off because `upload-artifacts: false`. | `.github/workflows/build.yml:23-26,245-256,479,520`; `.github/workflows/release.yml:77-78` `verified-in-code` | Renaming `asset-prefix` changes nothing in the release. Asset names come from `productName`, version and architecture through tauri-action's default pattern (`inferred`). |
| E-60 | The packaging paths are also hard-coded in the CI smoke tests. | `.github/workflows/build.yml:577-578,593-595,620-622,637-642,750-763` (`usr/bin/handy`, `/usr/lib/Handy`, `handy.exe`) `verified-in-code` | Change them together with `tauri.conf.json:53-55,65-67` and `src-tauri/build.rs:19` whenever Linux or Windows come back. |
| E-62 | `settings.rs:1526` `["Handy","cjpais"]` is a **test JSON fixture**. The real default for `custom_words` is empty. | `src-tauri/src/settings.rs:1020` `verified-in-code` | Remove it from the brand list. Add brand strings pass 1 missed: log file name `handy` (`src-tauri/src/lib.rs:841,845`), WAV names `handy-{ts}.wav` (`src-tauri/src/actions.rs:701`), LLM header `X-Title: Handy` (`src-tauri/src/llm_client.rs:196`). |
| E-64 | The hf-hub host and cache behavior were `inferred`. | `~hf-hub/src/api/tokio.rs:254,294` (`HF_ENDPOINT` override; default `https://huggingface.co`); `~hf-hub/src/lib.rs:40-43` (`HF_HOME`); `src-tauri/src/portable.rs:43-45` (sets `HF_HOME` only in portable mode) `verified-in-code` | Retag as verified. |
| E-78 | "Transcript text not found in log calls sampled (`inferred`)" understates both sides. Release builds **redact** transcripts. But a **full settings dump** is written at startup, and the default file level is Debug. That dump includes custom words, prompts and device names; API keys are redacted. | `src-tauri/src/utils.rs:17-23` (`redact_text`), used at `src-tauri/src/managers/transcription.rs:994-997`; `src-tauri/src/settings.rs:392,1102-1108`; `src-tauri/src/lib.rs:58` `verified-in-code` | Default the log level to Info, or drop the dump. Say this truthfully on the privacy page. |
| E-80 | Correct. Add that the runtime falls back to English, but CI fails when keys are missing. | `src/i18n/index.ts:93`; `scripts/check-translations.ts` (exits 1 on missing keys) `verified-in-code` | n/a |
| E-87, E-09, E-28, E-36 | "On/off by default in Wispr" overstates R1, which records the state of one account ("all on by default here", §5; "(on)", §11). Only the max recording length is stated as a default ("default 2 hours"). | R1 §5, §11 | Reword as "observed on/off". |
| Inventory #6 | `api.deepseek.com` is **not** an outbound call. It is a substring match that picks request fields, plus unit tests. | `src-tauri/src/llm_client.rs:62` (match); `:654` (`#[cfg(test)]`) before `:833-864` `verified-in-code` | Delete the row. |
| Inventory #1 | The tray trigger is conditional, and the startup trigger needs onboarding to be finished. | `src-tauri/src/lib.rs:294-299`; `src/App.tsx:453` `verified-in-code` | Amend the trigger column. |

Pass-1 citations I confirmed (no change needed):
- `tauri.conf.json:3,5,16-22,28,41-43,53-55,65-67,77,85-89`
- `release.yml:3,17-23,35,47-69,77,83`
- `build.yml:221-243,449-479`
- `catalog/mod.rs:36,84-86,316-320`
- `settings.rs:592-598,656-658,676-682,696-698,710-812,1376-1390`
- `UpdateChecker.tsx:50-77,85,108-109,143-162`
- `history.rs:22-40,104-105,163-185,435-450,557,691`
- `sink.rs:70-79`
- `lib.rs:817-821,876-897,965-973`
- `Info.plist:5-6`, `Entitlements.plist:5-8`
- `cli.rs:6-63`, `portable.rs:1-5,43-45`
- `Cargo.toml:2,3,19,51,61,76,207`
- `transcription_jobs.rs:1-2,31-43,136`
- `engine_supervisor/mod.rs:1-20`
- `~cargo/.../commands.rs:134,153`
- `~cargo/.../diarize.rs:1,53,117`
- `~cargo/handy-cpal-0.18.2/src/host/wasapi/mod.rs:24`
- `flake.nix:29-31,172`
- `code-quality.yml:50`
- 26 locales × 650 keys; 32 "Handy" occurrences in `en`.

---

## Class/effort changes

| Row | Pass-1 | New | Reason with evidence |
|---|---|---|---|
| E-02 System-audio permission | EXTEND S | **BUILD S-M** | The plugin's checks are for Screen Recording (`commands.rs:17-18,137,156`, `verified-in-code`). There is no public preflight API for the audio-capture consent (`inferred`). The usual approach is to start a short trial tap and treat the absence of an error, or of silence, as "granted". The "app restarts after grant" step from R1 §11 also has to be designed. Needs a `NSAudioCaptureUsageDescription` key, which is missing today (`Info.plist:4-7`). |
| E-22 Diarization | EXTEND M | **EXTEND L** | The work spans several places, all verified in code. **Engine:** batch-only `run` over the whole recording (`diarize.rs:114-117`); refuses while a stream is live (`diarize.rs:124-126`); new worker request type (`protocol.rs:23-53`); the supervisor holds one model per worker, so ASR and diarizer would swap or need a second worker (`engine_supervisor/mod.rs:12-15`). **Catalog:** filter must be lifted so the model can be downloaded (`catalog/mod.rs:316-320`). **Speakers and audio:** speaker IDs must be mapped onto the "Them" track only; English-only training (`catalog.json:1528-1530`); at most 4 speakers. **Rename:** a UI to rename speakers. |
| E-44 Meeting audio kept | EXTEND S | **EXTEND M** | Two tracks of about 2 h each. There is no audio encoder in the dependency tree: symphonia and rodio only decode (`Cargo.toml:59,61`; `inferred` from features). Opus would not be re-importable (`transcribeFiles.ts:6-8`), so FLAC needs a new encoder crate. Playback must stay synced with the transcript. |
| E-47 Dictation during a meeting | EXTEND M | **EXTEND L** (architecture decision first) | All verified in code: one worker, one model (`engine_supervisor/mod.rs:12-13`); a live stream takes the model's exclusive lease and runs get `Busy` (`transcribe-cpp model.rs:87-101`, `session.rs:277-287`); one global stream route (`transcription.rs:900-906`); Esc cancel → `engine.cancel()` bumps a global epoch and kills the worker, which also cancels queued meeting work (`utils.rs:116-117`, `transcription.rs:912-914`, `supervisor.rs:471-484`). Two viable designs (`inferred`): (1) the meeting uses **chunked, non-streaming** decode through the same queue, with cancel scoped per owner. This is cheaper, but live text stalls while the user dictates. (2) A **second `EngineSupervisor`** loads its own copy of the model, which doubles RAM/VRAM. Either way it is a core edit in a merge hotspot. |
| E-84 Memory for long recordings | EXTEND S | **merged** into E-10 / E-48 | Duplicate (same evidence: `sink.rs:73`). |
| E-52 Tray "Check for updates" | HAVE | **HAVE (+TUNE S)** | The click is a silent no-op when the user has turned updates off (`lib.rs:294-299`). Either hide the item or open settings. |
| E-06 Auto-detect any call | BUILD M | **BUILD M (keep); flag conflict** | Area B has `B-15` ("recognize meetings") as OUT and area C has `C-97` (Notetaker settings) as OUT (v1). Call detection is fully local (Core Audio "device running somewhere", or a process list; `inferred`), so it should not be OUT. Reconcile in R2 merge. |
| E-01 System audio capture | EXTEND M | **EXTEND M (keep, rescoped)** | M still holds. The work is our own capture module on `handy-cpal` plus `rubato` resampling, gated to 14.6+. The tap being device-bound (E-206) and capturing our own sounds (E-205) add S items. There is no reason to use ScreenCaptureKit: it needs full Screen Recording consent and buys nothing above macOS 14.6 (`inferred`). Supporting macOS 13 to 14.5 would be a separate M (`inferred`). |

---

## Release pipeline: verified change list

What I read: all of `release.yml` (83 lines) and `build.yml` (lines 1-820), plus the triggers of every other workflow. Nothing was executed.

### A. One-time setup (repo and secrets)

1. **Enable GitHub Actions** on our GitHub repo. GitHub disables Actions on forks by default (`inferred`). The repo must also be **public**, or `releases/latest/download/latest.json` will not resolve for unauthenticated clients (`inferred`).
2. **Repo secrets.** `release.yml:83` passes them with `secrets: inherit`; `build.yml` consumes them:
   - `APPLE_CERTIFICATE`: base64 of the Developer ID Application `.p12` (`build.yml:224,457`).
   - `APPLE_CERTIFICATE_PASSWORD` (`:225,458`).
   - `KEYCHAIN_PASSWORD`: any random string (`:226`).
   - `APPLE_ID` and `APPLE_PASSWORD` (an app-specific password), plus `APPLE_TEAM_ID` (`:453,455,456`). `APPLE_ID_PASSWORD` (`:454`) is a legacy name that Tauri v2 does not read (`inferred`), so it can be left unset.
   - `TAURI_SIGNING_PRIVATE_KEY`, `TAURI_SIGNING_PRIVATE_KEY_PASSWORD` (`:463-464`).
   - `GITHUB_TOKEN` is automatic (`:452`).
   - Not needed for macOS only: `AZURE_*` (`:460-462`).
3. **Signing identity.** It is picked by `grep "Developer ID Application"` over the temporary keychain (`build.yml:240-242`). Keep only one such certificate in the `.p12`, or `CERT_ID` may come out as several lines (`inferred`).
4. **Updater key.** Generate it with `tauri signer generate`. Put the **public key file content, base64** (same format as today's value at `tauri.conf.json:86`) into `plugins.updater.pubkey`. **Back up the private key and its password offline.** If they are lost, no installed copy can ever be updated again (`inferred`; the pubkey is compiled into the app). Rotating the key requires an intermediate release that is signed with the old key and ships the new pubkey.

### B. Config edits

Put these in a fork-only overlay where possible (see E-211).

5. `tauri.conf.json:3` `productName` and `:5` `identifier`. Choose **once, before the first public build**, together with the Apple Team. Changing the identifier or the team after release resets macOS permissions, because the designated requirement changes. That breaks dictation (Accessibility, Microphone) for every updated user (`inferred`). It also moves app data, settings and logs (`inferred`).
6. `tauri.conf.json:86` pubkey and `:88` endpoint → `https://github.com/<our-org>/<repo>/releases/latest/download/latest.json`.
7. `tauri.conf.json:77` Windows `signCommand`. Irrelevant for macOS-only CI once the matrix is trimmed, but it breaks **any** local Windows build. Remove it or override it in the fork overlay before Windows work starts.
8. `tauri.conf.json:4` `version`. `release.yml:21` and `build.yml:85` read it with a grep for the first `"version": "…"`, so it must stay in the base `tauri.conf.json`. Keep `Cargo.toml:3` in step (`flake.nix:29-31` reads it). Use **our own version line**, for example starting at 1.0.0 (see E-213). Otherwise upstream bumps cause conflicts on every merge, and the tags `v0.9.x` collide with upstream tags that may exist in our repo. GitHub's `createRelease` (`release.yml:30-38`) reuses an existing tag that points at an upstream commit, while the build checks out our branch (`build.yml:78`) (`inferred`).
9. Optional: `minimumSystemVersion` (`tauri.conf.json:42`). Keep 10.15 for dictation and gate Notetaker at runtime (E-92).

### C. Workflow edits

10. `release.yml:47-69`: trim to the two macOS entries:
    - `macos-26` / `aarch64-apple-darwin`. Keep it: needed for the Apple Intelligence SDK, `:49`.
    - `macos-latest` / `x86_64-apple-darwin`. Intel builds now have Metal off (HEAD commit `7bfb2e2c`), so Intel performance is CPU-only (`inferred` from the commit title).
11. `release.yml:77` `asset-prefix`: renaming it is optional and has **no effect** on release assets (see the corrections). Asset names follow `productName`.
12. `tagName` and `releaseName` (`build.yml:476-477`) are derived from the version and match the draft tag (`release.yml:33`). No change needed.
13. Leave `build.yml` untouched. Its Windows steps (`:66-72,159-176,265-381`), Linux steps (`:121-207,383-447,485-514,526-708`) and smoke tests (`usr/bin/handy`, `/usr/lib/Handy`, `handy.exe`: `:577-595,620-642,750-763`) only run on platforms we have dropped. They **will break when Linux/Windows return after a productName change** (Tauri derives the bundle binary name from it, `inferred`). Fix them then, together with `tauri.conf.json:53-55,65-67` and `build.rs:19`.
14. **Add a macOS verification step.** There is none today: `build.yml` audits only Linux and Windows. After tauri-action, run `codesign --verify --deep --strict`, `spctl -a -vv` on the `.app`, `xcrun stapler validate`, and a launch smoke test of the bundled binary with `--list-devices` (`cli.rs` headless flag). Whether the DMG itself is notarized and stapled is `unknown`; check it with `spctl -a -t open --context context:primary-signature`.
15. Other workflows:
    - `main-build.yml:8-9` runs on push to `main` with `sign-binaries: true` (`:51`). In our repo it would spend signing secrets every time we sync upstream into `main`. Point it at `wispr-bro` or disable it.
    - `code-quality.yml`, `test.yml` and `nix-check.yml` run on `main` and on PRs.
    - `build-test.yml` and `pr-test-build.yml` are manual and use `handy-test` / `handy-pr-*` prefixes (cosmetic).

### D. Release run and publish (pass-1 steps 6-7 confirmed, with additions)

16. Run Release (`workflow_dispatch`, `release.yml:3`). It creates a draft (`:35`). Each macOS job builds clean (`no-cache: true`, `:82`), signs, notarizes and uploads `.dmg`, `.app.tar.gz` and `.sig`. tauri-action merges both architectures into one `latest.json` (`darwin-aarch64` and `darwin-x86_64`) (`inferred`).
17. **Publish the draft.** Until then `releases/latest/download/latest.json` serves the previous release (`inferred`). Never publish a release that lacks the darwin entries: Mac clients then hit a "platform not found" error, and the footer shows nothing, because errors only go to the console (`UpdateChecker.tsx:108-109`).
18. Test the update path: install N-1 → footer "update available" → install → relaunch (`UpdateChecker.tsx:122-162`). Also test N-1 from the tray.

### Things that would break the updater

- Changing the identifier or the signing team after release (TCC reset; see 5).
- Losing the minisign private key or its password (see 4).
- An upstream merge "taking theirs" on `tauri.conf.json:86-88`. Our builds would then trust upstream's key, or poll upstream's feed. Add a CI grep guard (E-212).
- A version number that is not strictly greater than the installed one (semver comparison, `inferred`), or that collides with an upstream tag (see 8).
- An install location the app cannot write to, or a translocated app run from the DMG (E-216).
- Installing in the middle of a recording: `downloadAndInstall` is followed by an immediate `relaunch()` (`UpdateChecker.tsx:143-162`) (E-215).

---

## Network inventory: verified

Method: `rg "https?://"` over `src-tauri/src`, `src`, `tauri.conf.json`, `Cargo.toml`, `build.rs` and `index.html`; then I traced each hit to its trigger. I also checked the defaults of `hf-hub`, `@handy-computer/models` (no URLs and no `fetch` in `node_modules/@handy-computer`), and the frontend for remote fonts and CDNs (none).

| # | Endpoint | Triggered when | Without user action? | Evidence |
|---|---|---|---|---|
| 1 | `github.com/cjpais/Handy/releases/latest/download/latest.json`, plus the asset download on install | Footer mounts after onboarding (each launch, `inferred`); footer click; tray item (only if checks are enabled) | **Yes**: the check runs at launch when `update_checks_enabled` (default `true`). The download needs a click. | `tauri.conf.json:88`; `settings.rs:596-598`; `UpdateChecker.tsx:64,85,136,143`; `App.tsx:453`; `lib.rs:294-299` `verified-in-code` |
| 2 | `https://huggingface.co` (or `HF_ENDPOINT`) | Model or language-detector download | No: onboarding, the Models page or model migration, each started by a click. The detector is fetched together with a model download. | `~hf-hub/src/api/tokio.rs:254,294`; `model.rs:1876,2032-2042`; `ModelMigration.tsx:179-190`; `useLanguageDetector.ts:43` `verified-in-code` |
| 3 | `https://blob.handy.computer/{repo}/{rev}/{file}` | HF download failed | No (fallback of #2) | `catalog/mod.rs:36`; `model.rs:2175-2215` `verified-in-code` |
| 4 | `https://blob.handy.computer/*.bin` (legacy) | Legacy predefined models | No | `model.rs:916,947,977,1007,1038` `verified-in-code` (whether they are still listed: `unknown`) |
| 5 | LLM providers (OpenAI, Z.AI, OpenRouter, Anthropic, Groq, Cerebras, Bedrock; Custom defaults to `localhost:11434`) | Post-processing; model list fetched after the user selects a configured provider or clicks refresh | No (`post_process_enabled = false`) | `settings.rs:696-698,719-804`; `usePostProcessProviderState.ts:107-115,184-187`; headers `llm_client.rs:184-196` `verified-in-code` |
| ~~6~~ | ~~`api.deepseek.com`~~ | **Removed**: string match and tests only | n/a | `llm_client.rs:62,654` `verified-in-code` |
| 7 | Apple Intelligence | On-device | n/a | `swift/apple_intelligence.swift:5,64-75` `verified-in-code` |
| 8 | Links opened in the browser (donate, repo, docs ×2, portable releases, release-notes issue link) | Click | No | `AboutSettings.tsx:34,74`; `SecureInputWarning.tsx:10`; `LinuxGuide.tsx:7`; `portableInstaller.ts:12`; `content/release-notes/0.9.0.md:43` `verified-in-code` |
| 9 | `localhost:1420` | Dev only | No | `tauri.conf.json:8` `verified-in-code` |
| 10 (new) | OS-level traffic outside our control: Gatekeeper/notarization online check on first launch; WebKit | First launch | Yes, but OS-level | `inferred`. Mention it in the privacy copy rather than claiming "zero network". |

Conclusion: pass 1 is confirmed except for row 6, and two trigger details were refined. **The only call made without user action is the update check.** No telemetry was found (`verified-in-code` by search).

---

## New rows

| ID | Feature | R1 ref | Wispr behavior | Handy today (evidence) | Class | Effort | Notes / risks | Reuse hint |
|---|---|---|---|---|---|---|---|---|
| E-201 | Notetaker settings page (container) | §5, §11 | One settings page with these sections: Meeting detection, While Notetaker is active, Notepad, Keyboard shortcut, Transcript, Consent, Sharing, Import, plus a "Start tutorial" button. | Absent; no settings section for meetings (`src/components/Sidebar.tsx:34-60`). `verified-in-code` | BUILD | S | Individual options are E-05 to E-10, E-28, E-35 to E-38. Conflicts with `C-97` (OUT v1): reconcile. Put it in a fork-only settings store (pass-1 hotspot 3). | Existing `SettingsGroup` pattern |
| E-202 | Docked-panel header controls and the "…" note menu | §11, §14 | Traffic lights, back arrow, "…" menu (contents `unknown`; rename, delete and export likely, `inferred`), side-by-side toggle. | Absent. `verified-in-code` (no meeting window) | BUILD | S | Ships with E-12. | n/a |
| E-203 | Meeting store, lifecycle and retention (reopen, rename, delete; keep separately from dictation history) | §4, §5 Plans ("more meetings kept longer"), §14 | Meetings persist under Today / Past notes. | Dictation history keeps 5 by default and `PreserveLimit` deletes older entries (`settings.rs:676-682`; `history.rs:435-450`). Reusing it would purge meetings. `verified-in-code` | BUILD | S-M | Separate `meetings.db` (pass-1 hotspot 6) with its own retention and disk-usage display. | SQLite FTS5 (`libsqlite3-sys build.rs:132`) |
| E-204 | Search inside one transcript | §11, §14 (search icon in the Transcript header) | Find words in the current transcript. | Absent. `verified-in-code` | BUILD | S | Separate from E-34 (search across notes). | n/a |
| E-205 | Own-sound and mute contamination of the system track | inferred from §14 "You/Them" | Clean "Them" track. | The tap includes all processes, so Handy's own feedback sounds are captured (`loopback.rs:93,108`). `mute_while_recording` mutes the **system output** via AppleScript during dictation (`managers/audio.rs:24-40,743-771`; default off, `settings.rs:1036`). `verified-in-code`. Whether a tap sees audio after an output mute is `unknown`. | BUILD | S | During a meeting: skip "mute while dictating" and the feedback sounds, or exclude our PID from the tap. Excluding the PID needs a handy-cpal patch (an upstream contribution). Otherwise the user goes deaf mid-call. | handy-cpal PR adding a process-exclusion list |
| E-206 | Output-device changes during a meeting | inferred | Capture follows headphones or AirPods. | The tap is created for a fixed device UID (`loopback.rs:94-101`). `verified-in-code`. No default-output listener for loopback. | BUILD | S-M | Re-create the tap when the default output changes, and stitch the timeline. Bluetooth HFP also drops output quality while the AirPods mic is in use (handy-recorder holds the headset open, `~cargo/handy-recorder-0.1.0/src/backend/cpal/mod.rs:321-351`). | Core Audio default-device property listener |
| E-207 | Prevent sleep and App Nap while recording a meeting | inferred (2 h meetings) | Recording survives an idle display. | No power assertion anywhere (`rg IOPMAssertion\|NSProcessInfo\|caffeinate` returns nothing). `verified-in-code` | BUILD | S | Take the assertion only while a meeting records. Closing the lid still sleeps the Mac. | `objc2-foundation` `beginActivity`, or IOKit |
| E-208 | "Join meeting": open the event's meeting link and start Notetaker | §3 (Opt+M "Join meeting / start Notetaker"), §11 | One shortcut joins the call and starts recording. | Absent. `verified-in-code` | BUILD | S (after E-07) | Needs the EventKit URL/notes field. Opt+M types "µ" on US layouts; whether Handy's key hook swallows it is `unknown`. | `tauri-plugin-opener` (already used) |
| E-209 | Notetaker menu in the macOS app menu bar | §6 | Top-level "Notetaker" app menu. | No custom app menu (`rg set_menu` returns nothing in `lib.rs`, so Tauri's default menu, `inferred`). `verified-in-code` (absence) | BUILD | S | Coordinate with `A-63`, `C-105` and `C-106` (Dictation and My Voice menus). Build one fork-owned menu module. | Tauri `Menu` API |
| E-210 | System audio on Windows and Linux (later) | n/a | n/a | WASAPI loopback exists in handy-cpal (`~cargo/handy-cpal-0.18.2/src/host/wasapi/mod.rs:24`). On Linux, handy-recorder already lists PulseAudio **monitor** sources (`device.rs:28-30`, `backend/cpal/mod.rs:114-121`) but Handy filters them out (`src-tauri/src/commands/audio.rs:220`). `verified-in-code` | EXTEND | M | Linux is the cheapest port: just stop filtering for the meeting source. | n/a |
| E-211 | Fork-only Tauri config overlay for identity and updater | n/a | n/a | Upstream already uses platform overlays (`src-tauri/tauri.windows.conf.json`). Tauri merges `tauri.macos.conf.json` and `--config <file>` (`inferred`, standard Tauri v2). `verified-in-code` (overlay precedent) | TUNE | S | Holds `productName`, `identifier`, updater pubkey and endpoint. Keep `version` in the base file, because the workflows grep it there (`release.yml:21`, `build.yml:85`). Cuts conflicts on `tauri.conf.json`. | Tauri config merge |
| E-212 | Updater key custody plus a CI guard against identity regression | n/a | n/a | The pubkey is compiled in (`tauri.conf.json:86`); there is no guard. `verified-in-code` | BUILD | S | A CI step fails the build if the upstream pubkey, endpoint or identifier strings reappear after a merge. Document key backup and rotation. | Shell grep step |
| E-213 | Our own version scheme and tag hygiene | n/a | n/a | Version is shared with upstream (0.9.8, `tauri.conf.json:4`, `Cargo.toml:3`); tags are `v<version>` (`release.yml:33`). `verified-in-code` | TUNE | S | Avoid tag collisions and non-monotonic versions (see the release list, item 8). | n/a |
| E-214 | macOS release verification job | n/a | n/a | `build.yml` audits Linux (`:526-697`) and Windows (`:709-811`) only. `verified-in-code` | BUILD | S | codesign, spctl, stapler and a `--list-devices` launch smoke test. | n/a |
| E-215 | Defer update install while recording | §11 (Update button, restart) | Restart is user-initiated. | `downloadAndInstall` is followed at once by `relaunch()` (`UpdateChecker.tsx:143-162`). Nothing checks for an active recording or meeting. `verified-in-code` | EXTEND | S | Block or queue the update while dictating or recording a meeting. Optionally install on quit. | Tauri updater Rust API (`download` / `install` split) |
| E-216 | App location, translocation and non-admin updates | n/a | n/a | No install-location check (`rg /Applications\|translocat` returns nothing). `verified-in-code` | BUILD | S | Offer to move to /Applications on first run. The updater cannot replace a translocated or read-only bundle (`inferred`). | "Move to Applications" OSS snippets |
| E-217 | Distribution channels (open-source "app stores") | n/a | n/a | Only GitHub Releases. The Nix path disables the updater (`flake.nix:172`). `verified-in-code` | BUILD | S | Homebrew cask in our own tap first (`auto_updates true`, keep the in-app updater). The Mac App Store is not viable: its sandbox conflicts with Accessibility paste and global key hooks (`inferred`). | Homebrew cask template |
| E-218 | Licenses and attributions | n/a | n/a | MIT is declared (`tauri.conf.json:31`). No acknowledgements UI was found. The catalog lists model licenses, for example the Sortformer license is `"other"` (`catalog.json:1525`). `verified-in-code` | BUILD | S | Keep Handy's MIT notice. Show each downloaded model's license and check redistribution terms before adding the diarizer. | `cargo-about` / `license-checker` |
| E-219 | CI triggers for our branch | n/a | n/a | Quality, tests and nix run on `main` and PRs (`code-quality.yml:4-5,18`; `test.yml:4-8`; `nix-check.yml:12-13,22`); `main-build.yml` signs on push to `main` (`:8-9,51`). `verified-in-code` | TUNE | S | Retarget to `wispr-bro`. Avoid spending signing secrets on upstream syncs into `main`. | n/a |

Cross-area duplicates to drop or merge in R2:
- E-51/E-52 ≈ `C-27` / `A-60`
- E-66/E-67 ≈ `C-94`
- E-68 ≈ `C-93`
- E-89 ≈ `C-99`
- E-90 ≈ `C-102` / `C-103`
- E-87 ≈ `A-64` / `C-85`
- E-80 ≈ `C-83`
- E-30 ≈ `C-03`
- E-43 checklist ≈ `C-10`
- E-02 ≈ `B-14`
- E-50 ≈ `B-07` / `B-08`

---

## Open questions resolved

1. **Can one loaded ASR model serve a live meeting and dictation at once?** (pass-1 Q4) **No, not as is.** A live stream takes the model's exclusive compute lease (`transcribe-cpp model.rs:87-101`, `session.rs:277-287`). The supervisor has one worker and one model (`engine_supervisor/mod.rs:12-13`). Dictation's cancel kills that worker and cancels queued work (`supervisor.rs:471-484`, `utils.rs:116-117`). `verified-in-code`. Options are in E-47.
2. **Can `check_screen_recording_permission` detect tap permission?** (pass-1 Q1, partly) It checks Screen Recording (`commands.rs:137`, `verified-in-code`), not the audio-capture consent that process taps use (`inferred`). A real-device test is still needed to choose the detection method.
3. **Is diarization ready to plug in?** It is batch-only, English-only and capped at 4 speakers, and it cannot run while a stream is active (`diarize.rs:21-26,114-127`; `catalog.json:1528-1530`). `verified-in-code`
4. **Do transcripts leak into logs?** Not in release builds, which redact them (`utils.rs:17-23`). Settings, including the dictionary and prompts, do leak at the default Debug level (`settings.rs:1105-1108`). `verified-in-code`
5. **Does `asset-prefix` rename release assets?** No (`build.yml:248`; `release.yml` passes no pattern). `verified-in-code`
6. **Is FTS5 available for search?** Yes (`libsqlite3-sys-0.35.0/build.rs:132`). `verified-in-code`
7. **Where does a Linux system-audio path exist?** Monitor sources are already listed and only filtered out by Handy (`commands/audio.rs:220`). `verified-in-code`

## Still open

1. Real-device test on macOS 14.6+ and 15: which TCC prompt and Settings pane a process tap triggers, how to detect "granted", whether a restart is needed, and whether a tap still captures audio while the output is muted (E-02, E-205).
2. Engine design for E-47: chunked meeting decode on a shared queue with per-owner cancel, or a second supervisor (RAM budget on 8 GB Macs). Needs a decision before Notetaker work starts.
3. Is the DMG itself notarized and stapled by tauri-bundler v2, or only the `.app`? (`unknown`; check with E-214.)
4. Binary name inside Linux and Windows bundles after a `productName` change (`inferred`: it follows productName). This affects the CI smoke tests when those platforms return.
5. Does a global Opt+M binding swallow "µ" for users who type it? (`unknown`)
6. Sortformer license terms for redistribution (`catalog.json` says `"other"`), and quality on non-English audio (`unknown`).
7. Product decisions carried over from pass 1: keep or replace the `blob.handy.computer` mirror; hide or remove cloud LLM providers; local LLM choice (E-49); minimum macOS for Notetaker (14.6 recommended).
8. Startup time and idle memory: still unmeasured (no running allowed).
