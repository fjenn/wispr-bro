# Intel Macs

Wispr Bro runs on Intel Macs with the same features as on Apple Silicon:
local speech recognition, local cleanup (Muesli 2B), transforms and Command
Mode (Qwen3.5 2B), on every Mac with 8 GB of memory or more. The one thing
Intel cannot have is Apple Intelligence, which Apple only ships on Apple
Silicon; Wispr Bro's own cleanup model covers it there as it does on Apple
Silicon Macs without Apple Intelligence.

## What differs, and where

Only these chip specific pieces differ; everything else is the same code.

| Piece                  | Apple Silicon                         | Intel                                    | Where                                     |
| ---------------------- | ------------------------------------- | ---------------------------------------- | ----------------------------------------- |
| Speech engine          | transcribe-cpp with Metal             | transcribe-cpp on the CPU                | `src-tauri/Cargo.toml`                    |
| `llama-server` sidecar | Metal, M1 CPU baseline                | no Metal, AVX2 + FMA + F16C CPU baseline | `ci/llama-server/build.sh`                |
| Where the LLM runs     | Metal first, CPU fallback             | CPU (`-ngl 0`) from the first start      | `fork::llm::process::Device::initial`     |
| LLM threads            | all logical cores but two, at most 8  | physical cores, at most 8                | `fork::llm::process::recommended_threads` |
| Cleanup deadlines      | as R5 §5.1                            | 2.5 times longer (`CPU_FACTOR`)          | `fork::llm::deadline`                     |
| Speech model choice    | Metal latencies, Apple Silicon budget | CPU latencies, Intel budget              | `fork::models::policy`                    |
| Release asset          | `Wispr-Bro_<v>_aarch64.dmg`           | `Wispr-Bro_<v>_x86_64.dmg`               | `scripts/wb-release/macos.sh --arch`      |
| Update feed            | `darwin-aarch64` in `latest.json`     | `darwin-x86_64` in the same file         | `wb-release.yml` publish                  |

Why no Metal on Intel: on Intel Macs with AMD GPUs, ggml's Metal backend can
hang compiling its shaders at startup (Handy #2340), which is why Handy
already builds the speech engine for Intel on the CPU. Metal on Intel and AMD
GPUs is also rarely faster than the CPU for models this size, so the CPU is
the reliable choice for both engines.

The Intel build is cross compiled on the same Apple Silicon CI runner
(`tauri build --target x86_64-apple-darwin`), with its checks and smoke test
under Rosetta 2. A separate DMG keeps the Apple Silicon download and its
update entry exactly as they were; a universal app would double the download
for everyone.

## Test checklist (a real Intel Mac)

Run on an Intel Mac with macOS 13 or later and 8 GB or more.

1. Install the `x86_64` DMG; the app opens without a Gatekeeper warning.
2. Onboarding completes (microphone, accessibility, shortcut test).
3. Settings › Advanced shows "Intel processor" and a speech model downloads.
4. A short dictation lands as text in another app.
5. The cleanup model downloads, and a dictation with "um" and a self
   correction comes out cleaned (not "Inserted without cleanup").
6. A long dictation (a minute or more) is cleaned without timing out.
7. A transform and a Command Mode request work.
8. Activity Monitor shows `llama-server` as an Intel process using the CPU
   only, and it stops after about two minutes idle.
9. Note how long a short take takes from key-up to text, for the first and a
   later take; if cleanups often time out, `CPU_FACTOR` needs raising.

## First release with Intel

Updates on Intel work like on Apple Silicon: the app looks up
`darwin-x86_64` in the same `latest.json`. `wb-release.yml` writes both
entries. A release built on the owner's Mac instead runs `macos.sh` once per
`--arch`, and each run writes a `latest.json` with its own entry only, so
merge them before uploading:

    bun scripts/wb-release/manifest.ts --version <v> --release-tag <tag> \
      --asset "darwin-aarch64=<aarch64 .app.tar.gz>" \
      --asset "darwin-x86_64=<x86_64 .app.tar.gz>" --out latest.json

`scripts/wb-release/release-local.sh` does all of this for a new release
(RELEASING.md, "Both chips"). Then check the uploaded file lists both
platforms, and that an installed
Intel copy of the previous release offers the update and relaunches on it.

To add Intel to a release that is already out with only the Apple Silicon
build (1.0.0-alpha.6), run on the owner's Mac, from a pushed checkout whose
version is that release's:

    WB_NOTARY_PROFILE=wispr-bro-notary scripts/wb-release/add-intel.sh v1.0.0-alpha.6

It builds, signs, notarizes and staples the Intel app and DMG, uploads the
DMG, the updater archive and its `.sig`, then a `latest.json` with
`darwin-x86_64` added (`add-platform.ts` keeps every published entry,
`version`, `notes` and `pub_date` as they were) and a `SHA256SUMS` with the
Intel lines added, and reads the release back. Nothing of the Apple Silicon
build is rebuilt or replaced. It needs `gh` signed in to an account that can
edit `fjenn/wispr-bro`.
