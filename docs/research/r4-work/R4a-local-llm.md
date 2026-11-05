# R4a: Parts bin, local LLM (cleanup, styles, transforms, Command Mode, summaries)

Deliverable R4, part a. Scope: the capability cluster "local LLM" behind R2 rows D-01..D-14, D-40..D-46, D-67..D-71, D-208..D-214 and findings AF-01, AF-02, AF-14 (plus AF-19, AF-20), decisions DEC-03 and DEC-13. Date of research: 2026-10-28.

## 0. Method

- Read R2 §1-4 fully and Area D rows (`docs/research/R2-gap-analysis.md`).
- Read Handy code: `src-tauri/src/llm_client.rs` (lines 1-500), `actions.rs:120-442`, `apple_intelligence.rs` (all), `swift/apple_intelligence.swift` (all), `settings.rs:790-850` (custom provider, default prompt), `engine_supervisor/mod.rs:1-40`, `managers/model.rs:1810-1830` and `:3355-3386`, `Cargo.toml`, `Cargo.lock` (reqwest, strsim, whatlang, nix), `tauri.conf.json`.
- Shallow/sparse clones (read only, nothing built, installed or run) under `scratchpad/clones/`: `r4a-llamacpp` (llama.cpp `master` at 2026-10-28: `tools/server/*`, `common/arg.cpp`, `common/json-schema-to-grammar.cpp`, `grammars/README.md`, `.github/workflows/release.yml`, `LICENSE`), `r4a-llamacpp-bin` (release `b11972` macOS tarballs, inspected with `tar`, `otool`, `codesign -dv` only), `r4a-jan` (Jan: `src-tauri/plugins/tauri-plugin-llamacpp`, `mlx-server`, `tauri.macos.conf.json`), `r4a-openwhispr`, `r4a-freeflow`, `r4a-hex`, `r4a-model-licenses` (model LICENSE and README files downloaded from Hugging Face).
- Repository metadata via the GitHub API (`gh api repos/...`: license SPDX, stars, last push, releases); model metadata via the Hugging Face API (`/api/models/<id>`: `cardData.license`, downloads, `lastModified`; `/tree/main`: GGUF file sizes).
- Apple Foundation Models: Apple documentation JSON (`developer.apple.com/tutorials/data/documentation/foundationmodels/...`), WWDC26 session 241 page.
- GPL/AGPL projects (VoiceInk GPL-3.0, Epicenter/Whispering AGPL-3.0, Tambourine AGPL-3.0, VoiceTypr AGPL-3.0, KoboldCpp AGPL-3.0): license checked only; no code read.
- Tags: **(obs)** = I read it in the file/page/binary cited; **(inf)** = inferred from what I read; **(unk)** = not determinable without a run on a real Mac. Star counts and dates are from the GitHub/HF APIs on 2026-10-28.

## 1. Runtime as a sidecar process

AF-01 rules out linking any ggml-based runtime into the Handy executable or its engine worker. The candidates below are therefore judged as **separate processes** (or, for the Apple framework, as an in-process system API that does not use ggml).

### 1.1 Candidate table

| Candidate | License (evidence) | Popularity / last release | macOS binary | OpenAI API | Structured output | Health / idle / cancel | Fit | Cost | Recommendation |
|---|---|---|---|---|---|---|---|---|---|
| **llama.cpp `llama-server`** https://github.com/ggml-org/llama.cpp | MIT: `LICENSE` "MIT License, Copyright (c) 2023-2026 The ggml authors", also shipped inside the release tarball (obs) | 132,670 stars; release `b11972` 2026-10-28 (several builds a day); `v0.8.0` tag 2026-10-28 with ggml 0.28.0 (obs) | Official `llama-b11972-bin-macos-arm64.tar.gz` (16.5 MB gz) and `-macos-x64.tar.gz` (12.2 MB gz). Server needs `llama-server` + 10 dylibs: 29 MB arm64, 26 MB x64 uncompressed. `LC_RPATH @loader_path`, `minos 13.3`, ad-hoc linker signature, Team ID not set; x64 has **no Metal** (`-DGGML_METAL=OFF` in `release.yml:113-118`); arm64 built with `-DGGML_METAL_EMBED_LIBRARY=ON` (`release.yml:103-106`) (obs) | `/v1/chat/completions`, `/v1/models`, `/v1/responses`, Anthropic `/v1/messages` (README §OpenAI-compatible) (obs) | `response_format` `{"type":"json_schema","json_schema":{"schema":…}}` is parsed (`server-common.cpp:1384-1398`), i.e. **exactly the shape Handy's `llm_client.rs:412-420` already sends** (obs). JSON schema → GBNF; `maxLength` on strings supported (`grammars/README.md` example `char{1,100}`); limits listed (obs) | `GET /health` → 503 while loading, 200 ready (README:480-491); `--sleep-idle-seconds` unloads model and KV but keeps the process (README:2547-2559); `--host` accepts a `.sock` UNIX socket path (README:193); API key via env `LLAMA_API_KEY` (README:215); generation aborted when the HTTP connection closes (`server-http.cpp:686`, `server-context.cpp:5397` `// connection is closed`) (obs). `reasoning_effort:"none"` disables thinking (README:1449) (obs) | Excellent: Handy already speaks its dialect; same ggml family as transcribe-cpp but in its own process (AF-01) | M (supervisor + provider + packaging) | **Adopt** as the bundled runtime |
| **mistral.rs** https://github.com/EricLBuehler/mistral.rs | MIT: `LICENSE` "MIT License Copyright (c) 2024 Eric Buehler" (obs) | 7,780 stars; v0.9.5 2026-10-19 (obs) | Only `mistralrs-metal-aarch64-apple-darwin.tar.gz` (50.5 MB gz); **no Intel macOS asset** (obs) | OpenAI + Anthropic endpoints (README:98) (obs) | "grammar enforcement and strict schema mode" (README:162) (obs); JSON-schema `response_format` details not verified (unk) | Not verified (unk) | Medium: Rust/candle, no ggml symbols, so it could even be linked in-process (inf), but bundles an agent runtime with web search and shell execution (README:37, :103) = larger attack surface (obs/inf) | M-L | **Study only** (no Intel, larger binary, extra features we must disable) |
| **candle** (+ candle-vllm) https://github.com/huggingface/candle, https://github.com/EricLBuehler/candle-vllm | Apache-2.0 / MIT (GitHub API SPDX) (obs) | 21,246 stars, no GitHub releases, pushed 2026-10-14; candle-vllm 738 stars (obs) | Library; no maintained prebuilt macOS server (inf) | candle-vllm yes (inf) | Would be own code (inf) | Own code | Low: we would write the server | L | **Reject** (too much own code) |
| **kalosm** https://github.com/floneum/kalosm | Apache-2.0 (API) (obs) | 2,228 stars, pushed 2026-10-24 (obs) | Library (in-process, candle) | No | Has constrained generation (inf, not read) | In-process: no crash isolation | Low | L | **Reject** |
| **mlx-lm server (Python)** https://github.com/ml-explore/mlx-lm | MIT (API) (obs) | 7,497 stars; v0.32.2 2026-10-16 (obs) | Needs a bundled Python + MLX wheels (inf) | Yes | No `response_format`/`json_schema` handling in `server.py` (grep: only logits processors for penalties) (obs) | `/health` (server.py:1781) (obs). `SERVER.md:8` "not recommended for production as it only implements basic security checks" (obs) | Poor: Python runtime, Apple Silicon only | L | **Reject** |
| **MLX Swift** (`mlx-swift-lm`) https://github.com/ml-explore/mlx-swift-lm and Jan's `mlx-server` (Apache-2.0, `janhq/jan/mlx-server`) | MIT (API) / Apache-2.0 (Jan `LICENSE`) (obs) | mlx-swift-lm 850 stars, 3.32.3 2026-09-30; mlx-swift-examples 2,689 stars, last push 2026-07-20 (obs) | Must be built with `xcodebuild` (Metal shaders via plugin), macOS 14+, Apple Silicon only (Jan `mlx-server/README.md`) (obs) | Jan's server: chat + streaming + tools (obs) | mlx-swift-lm has xgrammar-based guided generation (`Libraries/MLXGuidedGeneration/XGrammarBridge.swift`, `Libraries/MLXCXGrammar/...json_schema_converter.cc`) (obs); Jan's `mlx-server` has no `json_schema`/`response_format` code (grep empty) (obs) | Own code | Medium: second model format (MLX safetensors), AS-only, Swift build in CI | L | **Study only** (possible later speed path on Apple Silicon; see §2.4 for the macOS 27 `MLXLanguageModel`) |
| **Ollama** https://github.com/ollama/ollama | MIT (`LICENSE` "MIT License Copyright (c) Ollama") (obs) | 185,140 stars; v0.40.12 2026-10-27 (obs) | Separate user-installed app (186 MB tgz) (obs) | Yes | `format` takes a JSON schema; also via OpenAI `response_format` (https://docs.ollama.com/capabilities/structured-outputs) (obs) | Keep-alive/unload managed by Ollama (inf) | Already reachable through Handy's `custom` provider (`settings.rs:798-812`, D-70) (obs) | S (exists) | **Keep as escape hatch, do not bundle or depend on** (needs install = not "no setup") |
| **llamafile** https://github.com/mozilla-ai/llamafile | Apache-2.0 (`LICENSE` "Copyright 2023 Mozilla Foundation"), embeds MIT llama.cpp (obs) | 26,514 stars; 0.10.6 2026-09-15 (obs) | `llamafile-0.10.6-thin` 43.9 MB; APE polyglot format (obs/inf) | Yes (inf) | Via embedded llama.cpp (inf) | – | Poor: APE executables are not normal Mach-O, so signing/notarizing as a Tauri sidecar is doubtful (inf); lags llama.cpp | M | **Reject** |
| **Own small Rust worker linking llama.cpp** (pattern of Jan's `jan-llama-worker`, Apache-2.0) via `llama-cpp-2` (https://github.com/utilityai/llama-cpp-rs, MIT/Apache-2.0 `LICENSE-MIT`, `LICENSE-APACHE`; 659 stars, pushed 2026-10-27) | as left (obs) | – | Our binary, statically linked (inf) | We would write it or drive llama.cpp's `server_routes` through a C shim like Jan (`shim/jan_llama_shim.cpp`) (obs) | GBNF via sampler; JSON-schema conversion lives in llama.cpp `common` (C++), so either hand-written GBNF or a shim (inf) | Own stdin/stdout protocol could reuse the `engine_supervisor` pattern (inf) | Good fit with Handy's supervisor, but more own code and C++ build | L | **Study only** (fallback if `llama-server` packaging proves painful) |
| **KoboldCpp** https://github.com/LostRuins/koboldcpp | AGPL-3.0 (API) (obs) | 12,091 stars | – | – | – | – | – | – | **Reject** (license) |

### 1.2 What `llama-server` gives us, verified in source (obs unless tagged)

- Defaults that must be overridden for an 8 GB Mac: `--ctx-size` 0 = model's trained context (README:50) (Qwen3.5 = 262,144 tokens, model card); `--cache-ram` default **8192 MiB** host RAM for the prompt cache (README:168); `--fit on` by default with a 1024 MiB margin and minimum ctx 4096 (README:89-91); default port is now **9931**, not 8080 (README:194); web UI on by default (`--no-webui`, README:210). OpenWhispr hit exactly these and documents it in `src/helpers/llamaServer.js:221-233` ("Unset, this defaults to the model's full trained context (128K+)…", "llama-server otherwise reserves 8192 MiB of host RAM for the prompt cache") (obs).
- Reasoning: `--reasoning off`/`-rea off`, `--reasoning-budget 0`, per request `reasoning_effort: "none"` (README:237-239, :1449). Handy's client already sends `reasoning_effort: "none"` for `custom` providers (`llm_client.rs:79-84`, `actions.rs:248`).
- Hybrid/recurrent models (Qwen3.5 is "Gated DeltaNet + attention", inf from model card architecture) reuse a prompt prefix only through context checkpoints: `--ctx-checkpoints` default 32 per slot, `--checkpoint-min-step` default **8192 tokens** (README:166-167). A ~600-token system prompt is shorter than that, so prefix reuse for Qwen3.5 probably needs `-cms 0` or a small value (inf); measure (B-6 in §7).
- Router mode (`--models-dir`, `--models-max`, `--models-memory-margin`) can host several models and LRU-unload them (README:2013-2176); not needed for v1 (one model at a time).

### 1.3 Packaging `llama-server` as a Tauri sidecar on macOS

- Tauri's bundler copies `externalBin` binaries into the bundle and **signs them as executables before the app** ("per apple, signing must be done inside out"), and also copies and signs `bundle.macOS.frameworks` `.dylib` files into `Contents/Frameworks` (`crates/tauri-bundler/src/bundle/macos/app.rs:93-132`, `:381-416`, fetched from `tauri-apps/tauri` default branch) (obs). Resources (`bundle.resources`) are not in that sign list (obs) — so dylibs must not go in Resources unless signed separately (inf).
- The prebuilt `llama-server` loads 10 dylibs from `@rpath` with `LC_RPATH @loader_path` (otool, obs). In a Tauri bundle the sidecar lands in `Contents/MacOS` while `frameworks` dylibs land in `Contents/Frameworks` (inf from `app.rs` paths), so the prebuilt set needs `install_name_tool -add_rpath @executable_path/../Frameworks` (or copying all dylibs next to the binary in `Contents/MacOS`, which Apple discourages) and re-signing; library validation under the hardened runtime then requires the same Team ID on every dylib (inf).
- Simpler: **build `llama-server` from a pinned tag in our release CI as one static binary**: `cmake -B build -DBUILD_SHARED_LIBS=OFF -DGGML_METAL_EMBED_LIBRARY=ON -DCMAKE_OSX_DEPLOYMENT_TARGET=13.3 -DLLAMA_BUILD_BORINGSSL=OFF` and build only the `llama-server` target (flags taken from `release.yml:103-118`, `:146-152`; `-DBUILD_SHARED_LIBS=OFF` and TLS off are my proposal, inf). One Mach-O per arch, signed by Tauri as `externalBin` with the `-aarch64-apple-darwin`/`-x86_64-apple-darwin` suffix (https://v2.tauri.app/develop/sidecar/, obs). Size: ≤ the 29 MB shared set, likely less without the web UI (inf; measure).
- How others ship it: **OpenWhispr** (MIT, 10,251 stars) downloads the official release tarball pinned to `b9763` and copies `*.dylib` next to the binary (`scripts/download-llama-server.js:15-34`) and spawns it with `--host 127.0.0.1 --port N --ctx-size … --jinja`, health polling, idle timer and SIGTERM→SIGKILL (`src/helpers/llamaServer.js:91-95`, `:212-233`, `:526-552`, `:563-611`, `:633-652`) (obs). **Jan** (Apache-2.0, 45,132 stars) moved from a downloaded `llama-server` to its own statically linked `jan-llama-worker` driving llama.cpp's server code through a C shim, pinned to tag `b11669` and verified by commit (`plugins/tauri-plugin-llamacpp/build.rs:9-19`, `src/bin/jan-llama-worker.rs:1-20`), shipped with `libggml*` as resources (`tauri.macos.conf.json` `resources`), API key via env not argv, port reported on stdout, SIGTERM then SIGKILL (`src/process.rs:2-20`) (obs).
- Metal under the hardened runtime: Handy already ships ggml with Metal inside its signed, notarized app (transcribe-cpp `features = ["metal"]`, `src-tauri/Cargo.toml:155`) (obs), so a second ggml process with Metal should pass the same signing/notarization path (inf). Intel: release builds are CPU-only (obs); a source build with Metal on Intel is possible but GPU support there is uneven (inf) — Handy itself builds Intel CPU-only (`Cargo.toml:153-158`, AF-20) (obs).
- Startup time, first-run Metal library compile time and RSS: **unknown**, need a run (§7).

### 1.4 Verdict for §1

`llama-server` built from a pinned llama.cpp tag (static, Metal embedded) as a Tauri `externalBin`. It is MIT, ships every endpoint and knob Handy's client needs (JSON schema, reasoning off, health, sleep, socket host, API key, disconnect-cancel), and two permissive desktop apps already ship it the same way. Ollama stays available through the existing `custom` provider for power users (D-70).

## 2. Apple Foundation Models (macOS 26+)

### 2.1 What the framework offers (obs unless tagged; docs JSON at `https://developer.apple.com/tutorials/data/documentation/foundationmodels/<path>.json`)

| Item | Evidence |
|---|---|
| Availability with reasons: `.available`, `.unavailable(.deviceNotEligible / .appleIntelligenceNotEnabled / .modelNotReady)` | `systemlanguagemodel/availability-swift.enum/unavailablereason` (macOS 26.0) |
| `GenerationOptions(sampling:temperature:maximumResponseTokens:)` (+ `samplingMode`, `toolCallingMode` variants) | `generationoptions` (macOS 26.0) |
| `prewarm(promptPrefix:)` "Loads the resources required for this session into memory ahead of a request" | `languagemodelsession/prewarm(promptprefix:)` |
| `supportsLocale(_:)`, `supportedLanguages`, `contextSize`, `tokenCount(for:)` | `systemlanguagemodel/supportslocale(_:)`, `.../contextsize` (docs list macOS 26.0; WWDC26 session 241 says context-size and token-counting APIs came in 26.4 — conflict, treat as unk and guard at runtime) |
| Context: 4,096 tokens in the first release (third-party, inf); WWDC26 example prints `contextSize` = **8192** | https://developer.apple.com/videos/play/wwdc2026/241 |
| Guided generation (`@Generable`), already used by Handy (`apple_intelligence.swift:5-9`) | Handy code |
| Errors: `guardrailViolation`, `refusal`, `exceededContextWindowSize`, `unsupportedLanguageOrLocale`, `rateLimited`, `concurrentRequests`, `assetsUnavailable`, `decodingFailure` | `languagemodelsession/generationerror` |
| Guardrails `.permissiveContentTransformations`: "requests … that generate a String will not throw [guardrail] errors … the model may still sometimes refuse … generates a String refusal message"; for non-String (Generable) output it "behaves the same way as [default] mode and throws" | `systemlanguagemodel/guardrails/permissivecontenttransformations` |
| macOS 27 / WWDC26: new on-device model, `LanguageModel` protocol, open-sourced `MLXLanguageModel` and `CoreAILanguageModel`, Python SDK, `fm` CLI; also `PrivateCloudComputeLanguageModel` (cloud, 32k context) | WWDC26 session 241 |
| Latency on Mac | **unk** (Apple publishes none; measure) |
| Languages actually supported on a given Mac | **unk** statically; query `supportsLocale` at runtime |

Product consequences: Apple FM is zero-download and zero-RAM-budget for us, but it exists only on macOS 26+ Apple Silicon with Apple Intelligence enabled and the model downloaded (obs: reasons above), has a small context (4-8k), a language list we do not control, and guardrails that can refuse dictated content. `PrivateCloudComputeLanguageModel` is cloud and is **out** for this product (inf from product rule).

### 2.2 Handy today (obs)

- `actions.rs:256-272`: synchronous FFI call inside an async fn; `apple_intelligence.swift:80-118`: `DispatchSemaphore.wait()` with no timeout; a new `LanguageModelSession` per call; no `GenerationOptions`, no `prewarm`, availability collapsed to 0/1 (`:38-51`); Generable output with a silent second `respond` fallback (`:98-107`), i.e. up to two generations; `model` string reused as a word limit (`actions.rs:267`, `swift:25-36`). Confirms AF-14 and D-68.

### 2.3 Non-blocking call from Rust (fix for AF-14)

| Option | How | Evidence / reuse | Cost |
|---|---|---|---|
| **A. `spawn_blocking` + Swift-side timeout and `Task.cancel()`** | Rust: `tokio::task::spawn_blocking(move || apple_intelligence::process(...))` wrapped by the existing `complete_unless_cancelled` (`actions.rs:154-172`). Swift: replace `semaphore.wait()` by `semaphore.wait(timeout:)`; on timeout call `task.cancel()` and return an error | Exactly this pattern in **Hex** (MIT, `anomalyco/hex`, `src/apple_intelligence.swift:141-160` `blocking(timeout:)` with `task.cancel()`; availability reasons 0-5 at `:171-192`; `contextSize` at `:194-202`; guardrails `.permissiveContentTransformations` and `GenerationOptions(temperature: 0.2, maximumResponseTokens:)` at `:80-97`; Rust side caches availability for 10 s, `src/apple_intelligence.rs:80-93`) (obs) | **S** (~40 lines changed) |
| B. Async callback FFI | Swift `@_cdecl` takes a C callback + context pointer and returns a request id; Rust passes a boxed `tokio::sync::oneshot::Sender`, awaits it with `tokio::time::timeout`, and calls `@_cdecl cancel(id)` on Esc/timeout | Own design (inf); no thread parked | S-M |

Recommendation: **adapt option A from Hex** now (smallest diff, fixes cancel and timeout), with: availability reason enum (copy-pasteable from Hex), `supportsLocale(lang)` check before use (D-13), `GenerationOptions(sampling: .greedy, maximumResponseTokens: n)` (inf: greedy is a `SamplingMode` per docs), one generation only (drop the silent second call), `prewarm(promptPrefix:)` at key-down on a session kept for that dictation (D-211), and the same Rust-side output guards as the local provider (§4). Choose String output + `.permissiveContentTransformations` vs `@Generable` by benchmark: permissive mode avoids guardrail throws only for String output (obs, §2.1).

### 2.4 Role in the architecture

Apple FM = **optional fast path and first-run bridge**, not the only engine (DEC-03 option "both, Apple as optimization"): it can clean text on day one while the GGUF downloads (D-208), and on supported Macs it costs no download or RAM. The bundled `llama-server` is the engine that works on every Mac. On macOS 27, Apple's open-sourced `MLXLanguageModel` behind the `LanguageModel` protocol (WWDC26 session 241, obs) could later let one Swift code path drive both Apple's model and an MLX model; macOS 27-only, so **study only**.

## 3. Small instruct models for cleanup

### 3.1 License screen (owner rule: permissive only; custom terms flagged)

| Model | License (evidence) | Verdict |
|---|---|---|
| Qwen3.5-0.8B / 2B / 4B (2026-02/03) https://huggingface.co/Qwen/Qwen3.5-2B | Apache-2.0: repo `LICENSE` is the Apache 2.0 text (11,544 bytes) for 2B and 4B; card `license: apache-2.0` (obs) | OK |
| Qwen3-1.7B, Qwen3-4B-Instruct-2507 | Apache-2.0 (`LICENSE` read for 4B-2507; card tag for 1.7B) (obs) | OK |
| Qwen2.5-1.5B-Instruct | Apache-2.0 (card) (obs) | OK but superseded |
| **Qwen2.5-3B-Instruct** | **"Qwen RESEARCH LICENSE AGREEMENT"** (`LICENSE` read) (obs) | **Reject** (non-commercial research license) |
| Qwen3.8-Flash-Next (2026-08) | `qwen-community-1.0` custom, 180 B params (API) (obs) | Reject (license, size) |
| Gemma 4 E2B / E4B (2026-03) https://huggingface.co/google/gemma-4-E2B-it | Card `license: apache-2.0`, link `ai.google.dev/gemma/docs/gemma_4_license`, which shows the Apache 2.0 text; the Gemma Terms of Use page says "For Gemma 4 terms, see the Gemma 4 license" and lists only Gemma 1-3n in its appendix (obs) | OK (verify that the separately linked "Prohibited use" page is not incorporated for Gemma 4: unk) |
| Gemma 3 1B / 4B | `license: gemma`, gated; Gemma ToU incorporates the Prohibited Use Policy, requires flowing use restrictions into downstream agreements, and reserves remote restriction (https://ai.google.dev/gemma/terms) (obs) | **Reject** (custom terms) |
| Llama 3.2 1B / 3B | `llama3.2`, gated; requires "Built with Llama", 700 M MAU clause, Acceptable Use Policy "incorporated by reference" (https://dev.meta.ai/llama/llama3_2/license) (obs) | **Reject** (custom terms) |
| Phi-4-mini-instruct (3.8B) | MIT (`LICENSE` "Microsoft… MIT License") (obs) | OK |
| SmolLM3-3B | Apache-2.0 (card) (obs) | OK |
| Granite 4.1-3B / 4.2-3B (IBM, 2026-04 / 2026-08) | Apache-2.0 (card; 4.1 card text "License: Apache 2.0") (obs) | OK |
| Ministral-3-3B-Instruct-2512 | Apache-2.0 (card) (obs) | OK |
| **LFM2.5-1.2B, LFM3-1.7B (Liquid AI)** | "LFM Open License v1.0": §5 "Commercial Use … conditioned upon You or Your Legal Entity not exceeding the Threshold" = annual revenue ≥ $10,000,000 (`LICENSE` lines 27-33, 51-57) (obs) | **Reject** (end users in larger companies would be unlicensed) |
| microsoft/FrogNano-4B-2609 | MIT (card) (obs); provenance and purpose not checked (unk) | Not evaluated |

### 3.2 Permissive candidates: size, languages, instruction following

GGUF sizes from HF `/tree/main` (obs). RAM at runtime ≈ file size + KV cache + compute buffers; with `--ctx-size 4096`, one slot, small `--cache-ram`, expect roughly file + 0.2-0.6 GB (inf; measure).

| Model | Params | Q4_K_M GGUF (repo) | Q8_0 | Languages (card) | Instruction following (card) | Thinking default | Notes |
|---|---|---|---|---|---|---|---|
| Qwen3.5-0.8B | 0.87 B | 533 MB (`unsloth/Qwen3.5-0.8B-GGUF`) | 812 MB | 201 languages/dialects (Qwen3.5 card) | IFEval 52.1 non-thinking | — | Too weak for meaning-preserving edits (inf); Intel/very-low-RAM fallback only |
| **Qwen3.5-2B** | 2.27 B (incl. vision) | **1,281 MB** (`unsloth/Qwen3.5-2B-GGUF`) | 2,012 MB | 201 | IFEval **61.2** non-thinking; MMMLU 56.9 | **non-thinking by default** (card:585) | Hybrid linear attention: small KV (inf); prefix-cache caveat §1.2 |
| Qwen3-1.7B | 2.03 B | 1,107 MB (`unsloth/Qwen3-1.7B-GGUF`); official `Qwen/Qwen3-1.7B-GGUF` has only Q8_0 1,834 MB | 1,834 MB | 119 (inf, Qwen3 announcement) | IFEval **68.2** non-thinking; MMMLU 46.7 (from Qwen3.5-2B card comparison) | thinking by default (disable) | Plain transformer: prompt cache works normally (inf) |
| **Qwen3-4B-Instruct-2507** | 4.02 B | **2,497 MB** (`unsloth/Qwen3-4B-Instruct-2507-GGUF`) | 4,280 MB | 119 (inf) | IFEval **83.4** non-thinking; MMMLU 64.9 | instruct only, no thinking | Strongest non-thinking IF in this size per the Qwen3.5 card table |
| Qwen3.5-4B | 4.66 B (incl. vision) | 2,741 MB | 4,482 MB | 201 | IFEval 89.8 reported **in thinking mode**; non-thinking not published | **thinking by default** (card:712) | Must run with reasoning off; quality then unknown |
| Gemma 4 E2B | 2.3 B effective / 5.1 B with embeddings | 2,841 MB Q4_0 (`ggml-org/gemma-4-E2B-it-GGUF`), 3,107 MB Q4_K_M (unsloth) | 4,967 MB | 35+ out of the box, pre-trained on 140+ | not read | – | Large file for its speed class (per-layer embeddings); 128K ctx |
| Ministral-3-3B-Instruct-2512 | 3.85 B | 2,147 MB (official `mistralai/...-GGUF`) | 3,652 MB | en, fr, es, de, it, pt, nl, zh, ja, ko, ar | not read | – | Official GGUF from the vendor |
| Phi-4-mini-instruct | 3.84 B | 2,492 MB (unsloth) | 4,085 MB | 23 listed (card:272) | IFEval reported (card:360, value not extracted) | – | MIT |
| Granite-4.1-3B / 4.2-3B | 3.40 B | 2,244 MB (official `ibm-granite/granite-4.2-3b-GGUF`) | 3,893 MB | 12 (en, de, es, fr, ja, pt, ar, cs, it, ko, nl, zh) | IFEval avg reported (card:242) | – | Dense transformer (4.1 card:456); official GGUF |
| SmolLM3-3B | 3.08 B | 1,915 MB (`ggml-org/SmolLM3-3B-GGUF`) | 3,276 MB | en, fr, es, it, pt, zh, ar, ru | – | thinking by default (card:109) | Fewest languages |

Popularity (HF downloads, last 30 days per API): Qwen3.5-4B 7.4 M, Qwen3.5-2B 3.3 M, Qwen3-4B-Instruct-2507 2.7 M, Gemma-4-E2B 3.2 M, Ministral-3-3B 82 k, Phi-4-mini 354 k, SmolLM3 681 k, Granite-4.1-3B 593 k (obs).

### 3.3 Recommended defaults (to be confirmed by the bake-off in §7)

| Mac | Default | Challengers in the bake-off | Rationale |
|---|---|---|---|
| Apple Silicon, ≤ 8 GB RAM | **Qwen3.5-2B Q4_K_M (1.28 GB)**, thinking off, ctx 4096 | Qwen3-1.7B Q4_K_M (1.11 GB) | Smallest download that is plausibly good enough; Apache-2.0; 201 languages; non-thinking by default. Qwen3-1.7B has the better IFEval but weaker multilingual scores, so measure both on the eval set |
| Apple Silicon, ≥ 16 GB | **Qwen3-4B-Instruct-2507 Q4_K_M (2.50 GB)** | Qwen3.5-4B (reasoning off), Gemma-4-E2B, Ministral-3-3B, Granite-4.2-3B, Phi-4-mini | Best published non-thinking instruction following in the 3-4 B class; pure instruct model (no think tags) |
| macOS 26+ with Apple Intelligence available | Apple FM for Light cleanup while the GGUF is missing, then by benchmark (keep or switch) | – | Zero download (D-208, DEC-03) |
| Intel Macs | LLM cleanup **off by default** (deterministic only), opt-in Qwen3.5-0.8B or 2B CPU | – | Release `llama-server` x64 has no Metal (obs); CPU speed unknown |

Quantization: Q4_K_M as default; Q4_0 is slightly smaller and faster on Metal (inf). Pin the HF **revision commit** and **sha256** in Handy's catalog like the ASR models (`catalog/mod.rs:116`, `:347-379`) (obs).

## 4. Prompts and safety for cleanup

### 4.1 Prompt sources (permissive only)

| Source | License (evidence) | File | What to take |
|---|---|---|---|
| Handy (base) | MIT | `src-tauri/src/settings.rs:842-848` "Improve Transcriptions" (obs) | Baseline rules: numbers, spoken punctuation, keep language, "do not answer" |
| **OpenWhispr** https://github.com/OpenWhispr/openwhispr (10,251 stars, pushed 2026-10-28) | MIT (`LICENSE` "Copyright (c) 2024 OpenWhispr Team") (obs) | `src/locales/en/prompts.json` key `cleanupPrompt` (also translated into 10 locales under `src/locales/*/prompts.json`); `dictionarySuffix`; `translatePrompt` (obs) | Strong "THE SPEAKER IS NEVER TALKING TO YOU" framing, self-correction rules, few-shot examples incl. a prompt-injection example, "Empty or filler-only input → empty output" |
| **FreeFlow** https://github.com/zachlatta/freeflow (2,997 stars, pushed 2026-10-23) | MIT (`LICENSE` "Copyright (c) 2026 Zach Latta") (obs) | `Sources/PostProcessingService.swift:42-115` (`defaultSystemPrompt`, dated 2026-10-10) and `:117-140` (`commandModeSystemPrompt` for selected-text transforms) (obs) | Multilingual self-correction examples, "instruction preservation is strict" examples, `EMPTY` sentinel, developer-syntax rules; the Command Mode prompt with SELECTED_TEXT / VOICE_COMMAND / CONTEXT slots (D-29, D-44) |
| Hex https://github.com/anomalyco/hex (636 stars) | MIT (`LICENSE` "Copyright (c) 2026 Christopher Langton") (obs) | Apple bridge only (`src/apple_intelligence.swift`); no cleanup prompt found by grep (obs) | Apple FM patterns (§2.3) |
| VoiceInk, Epicenter/Whispering, Tambourine, VoiceTypr | GPL-3.0 / AGPL-3.0 (LICENSE files via API) (obs) | not read | **Do not use** |

Prompt text from MIT projects can be reused with the MIT notice (inf). Small models need short prompts: the OpenWhispr prompt is ~2.8 k characters (~700 tokens) and FreeFlow's ~5.5 k characters (inf from file sizes); with a 2B model, prefill of a 700-token system prompt on every call matters unless the prefix cache works (§1.2), so measure a trimmed prompt too.

### 4.2 Guard techniques (layered; each with evidence or reuse)

1. **Separate system and user messages, transcript wrapped in tags** — already in Handy's structured path (`actions.rs:250-321`) (obs); the legacy path pastes the transcript into a single user message (`actions.rs:367-376`) (obs) and must not be used for the local provider (AF-02).
2. **Constrained output** — `response_format` JSON schema `{"text": {"type":"string","maxLength": L}}`, `additionalProperties:false` (llama.cpp default) (obs §1.1). `L = 2·len(input)+64` caps runaway answers at the grammar level (inf); also `max_tokens` (Handy sends none today, `llm_client.rs:112-121`, obs). Large `maxLength` values expand into long GBNF repetition rules; cost unknown (unk, measure). For Apple FM: `@Generable` struct or String + permissive guardrails (§2.3).
3. **Instruction-execution detector** — reuse FreeFlow's MIT heuristic `appearsToHaveExecutedInstruction` (`Sources/TranscriptTextCore.swift:128-182`): assistant-preamble regex (`^(sure|certainly|absolutely|here's|here is|I'd be happy to|I can)`) not present in the input, or instruction markers lost **and** significant-token overlap < 0.35 (obs). Port to Rust (~50 lines); English-only markers, so add per-language preamble lists or rely on 4-5 for other languages (inf).
4. **Length-ratio guard** — reject when `len(out)/len(in)` is outside a per-level band (e.g. Light 0.5-1.3, Medium/Polish 0.3-1.3) (proposal, inf; tune on the eval set).
5. **Diff guard** — word-level diff (crate `similar`, Apache-2.0, also useful for D-40; or `strsim` MIT, already in `Cargo.lock`) (obs for strsim in lock): for Light, reject when the share of inserted content words exceeds a threshold; for every level, reject when digits, URLs, e-mail addresses, code tokens or custom dictionary words present in the input are missing from the output (D-49) (inf).
6. **Language guard** — `whatlang` (MIT, already in `Cargo.lock`, obs) or Handy's `audio_toolkit/lang_id.rs` (R2 D-13): output language must equal input language unless a translate action was requested (inf).
7. **Deterministic fallback** — on any guard hit, timeout, empty output, JSON parse failure or server error, paste the deterministic text and show "Pasted without cleanup" (D-09). Today a structured parse failure pastes the **raw model content** (`actions.rs:340-350`) (obs) — must become a fallback to the input.
8. **Temperature 0 / greedy, reasoning off, no retries** for the local provider (AF-02), one HTTP call per dictation.

### 4.3 Open evaluation data

| Dataset | License (evidence) | Use |
|---|---|---|
| **Disfl-QA** https://github.com/google-research-datasets/disfl-qa, HF `google-research-datasets/disfl_qa` | CC BY 4.0 ("Disfl-QA dataset is licensed under CC BY 4.0", README) (obs) | 11,825 pairs `disfluent` → `original` **questions**: tests both disfluency removal and "must not answer the question" |
| DisfluencySpeech HF `amaai-lab/DisfluencySpeech` | Apache-2.0 (card) (obs) | Disfluent transcripts with cleaned variants (inf from name/card; fields not read) |
| AMI disfluent HF `JacobLinCool/ami-disfluent`, `hhoangphuoc/ami-disfluency` | CC BY 4.0 / Apache-2.0 (cards) (obs) | Meeting-style English disfluencies; also useful for Notetaker summaries (E-18) |
| PodcastFillers (`ylacombe/podcast_fillers*`) | card says only "cc" (obs) | **unk** which CC variant; do not use until clarified |
| Bhasik Indian-English disfluency | CC BY-ND 4.0 (obs) | ND: eval only, no derivatives (inf) |
| Paroles disfluentes (CNRS) | CC BY-NC-SA 4.0 (obs) | Avoid (NC) |

These licenses govern data used in tests, not shipped code (inf); keep the eval set out of the app bundle. Add a hand-written adversarial set (~200 items: dictated questions, "ignore your instructions…", requests to write an e-mail, code/paths/flags, multilingual self-corrections, numbers/dates, empty and filler-only takes). No permissive multilingual dictation-cleanup benchmark was found (unk whether one exists).

## 5. Integration fit with Handy

### 5.1 Provider design (D-212, AF-02)

- Add built-in provider `local` in `default_post_process_providers` (`settings.rs`), `supports_structured_output: true`, `allow_base_url_edit: false`, `base_url` set at runtime by the supervisor. Because built-ins are re-added and force-synced on every `get_settings()` (AF-02), existing users get it automatically (R2, obs there).
- `llm_client.rs` changes (all small): request fields `temperature`, `max_tokens`, `seed`, `cache_prompt`; per-provider HTTP timeout; neutral headers for `local` (today `Referer`/`User-Agent`/`X-Title: Handy`, `llm_client.rs:184-196`, obs); keep `reasoning_effort:"none"` (llama-server honors it, §1.2). Optional: UNIX-socket transport — `llama-server --host /path/llm.sock` (obs) and `reqwest::ClientBuilder::unix_socket` (added in reqwest 0.12.23 per release notes, https://newreleases.io/project/cargo/reqwest/release/0.12.23; Handy locks reqwest **0.12.28**, `Cargo.lock:5079-5080`, obs) avoids port choice and TCP exposure (inf; verify the feature flag). Otherwise: bind `127.0.0.1:0` to pick a free port, pass it, and set a random `LLAMA_API_KEY` via env (never argv, as Jan does).
- `actions.rs::post_process_transcription`: for `local` (and Apple) skip the legacy fallback chain (`actions.rs:357-401`), wrap the whole call in one deadline, run the guards of §4.2, and never paste unparsed model content.
- Prompt composition (D-05, D-27, DEC-13): one builder = level (None/Light/Medium) + style fragment + rule fragments (D-33) + dictionary terms (D-49) + language (D-13). "Auto Apply Polish" (D-42) can be the same pipeline with the Polish fragments and looser length bands, so DEC-13 can be "one engine, two entry points" (inf).

### 5.2 Process supervision (D-213, D-211)

| Concern | Proposal | Reuse / evidence |
|---|---|---|
| Spawn | `tokio::process::Command` on the `externalBin` path next to the executable (inf: Tauri copies external binaries to the bundle's binary dir, `app.rs:99-105`), args: `-m <gguf> --host <sock or 127.0.0.1> --port <p> -c 4096 -np 1 --cache-ram 256 --reasoning off --no-webui -fa auto --sleep-idle-seconds 120` (flags: README, obs; values: inf) | OpenWhispr `llamaServer.js:212-233` (MIT) |
| Readiness | Poll `GET /health` until 200 (503 = loading) | README:480-491 (obs) |
| Warm-up | Start at key-down when cleanup is on (D-211); first request can send `n_predict: 0` to prefill the system prompt into the cache (`/completion` "When 0 … the prompt is evaluated into the cache", README:533) (obs) | – |
| Idle | Model unload via `--sleep-idle-seconds`; kill the process after a longer idle (e.g. 15-30 min) or when ASR needs memory on ≤ 8 GB (D-213) | README:2547-2559 (obs) |
| Crash / hang | On exit or health failure: restart once; on second failure within N minutes, retry CPU-only (`-ngl 0`), then disable LLM cleanup for the session and paste deterministic text | Mirrors Handy's `engine_supervisor` GPU→CPU fallback (`engine_supervisor/mod.rs:1-20`, obs) |
| Shutdown | SIGTERM, wait, SIGKILL; on launch kill a stale PID recorded in app data (macOS has no parent-death signal, inf) | Jan `process.rs:2-20`, OpenWhispr `llamaServer.js:526-552` (obs); `nix 0.30` already in `Cargo.lock` (obs) |
| Cancel | Drop the reqwest future (Esc): server aborts on connection close (§1.1) | `complete_unless_cancelled` (`actions.rs:154-172`, obs) |
| Concurrency | `-np 1`; Transforms/Command Mode/Notetaker share the slot through a small queue | inf |

### 5.3 Model download (D-71, AF-19, AF-20)

Reuse Handy's hf-hub cancellable downloader and catalog pattern (`managers/model.rs:1817` `download_hf_model`; pinned repo + revision + sha256 in `catalog/mod.rs`) (obs). Needed changes: a catalog section for LLM files (one per RAM tier), discovery must stop ignoring LLM GGUFs only for our catalog entries (today non-ASR GGUFs are deliberately ignored, test at `model.rs:3357-3384`, obs), a RAM probe (`sysctl hw.memsize` via `libc`, or `NSProcessInfo.physicalMemory` via the existing `objc2-foundation` dependency, inf; AF-20), and a disk check. Files land in the shared HF cache like ASR models (AF-19), which "delete my data" must cover. Order: after the ASR model (DEC-01), silently, cancellable. Separate download code is not justified (inf).

### 5.4 Fit summary per runtime

| Runtime | `llm_client.rs` fit | Supervision | Download reuse | Cost |
|---|---|---|---|---|
| llama-server (adopt) | Native: same request shape incl. JSON schema (obs) | New small supervisor (M) | Full reuse (GGUF) | **M** total |
| Apple FM (adapt) | Separate path (`apple_intelligence.rs`); add guards | None (system) | None | **S** |
| Ollama (keep) | Existing `custom` provider; could flip `supports_structured_output` for it since Ollama supports `response_format` (obs §1.1) | User's | User's | S |
| mistral.rs / MLX (study) | OpenAI-compatible, schema support partly unverified | New | MLX needs a second format | L |

## 6. Recommended architecture

**Runtime.** `llama-server` from a pinned llama.cpp tag, built in our CI as one static Metal-embedded binary per arch and shipped as a Tauri `externalBin` (AF-01, D-67). Apple Foundation Models as an optional path on macOS 26+ Apple Silicon, fixed per §2.3 (AF-14, D-68). Ollama only through the existing `custom` provider (D-70). DEC-03 → "both, Apple as optimization", with Apple also bridging the first run while the GGUF downloads (D-208).

**Models per tier** (bake-off pending, §7): ≤ 8 GB Apple Silicon → Qwen3.5-2B Q4_K_M (1.28 GB); ≥ 16 GB → Qwen3-4B-Instruct-2507 Q4_K_M (2.50 GB); Intel → LLM off by default. All Apache-2.0. Rejected for license: Llama 3.2, Gemma 3, Qwen2.5-3B, LFM2.5/LFM3, Qwen3.8 community.

**Provider.** Built-in `local` provider: structured output on, temperature 0, `max_tokens` bound, reasoning off, neutral headers, one call, one deadline, UNIX socket or random port + env API key (D-212, AF-02, D-11). Composed prompt per dictation (D-05, D-27, D-33, D-49, D-13); DEC-13 as one engine with two entry points.

**Safety guards** (D-12, D-09): JSON schema with `maxLength`; FreeFlow-style instruction-execution detector (MIT, port); length-ratio band per level; diff guard protecting digits/URLs/code/dictionary terms; language guard; deterministic fallback with a pill hint; never paste unparsed model output.

**Latency budget** (targets; all numbers **unk** until measured):

| Stage | Target | Mechanism |
|---|---|---|
| Server start + model load | hidden behind recording | spawn/wake at key-down (D-211); `--sleep-idle-seconds` keeps the process |
| System-prompt prefill | ≈ 0 when cached | `cache_prompt`, `n_predict:0` warm-up; check Qwen3.5 checkpoint behavior (`-cms`) |
| Transcript prefill (≤ 200 tokens) | < 150 ms | Metal |
| Generation | p50 < 1.0 s for ≤ 30 words; p95 < 2.0 s | 2-4 B Q4 on Apple Silicon |
| Hard deadline (whole chain) | `1.2 s + 25 ms × expected output tokens`, capped at 6 s; then deterministic text | single `tokio::time::timeout` around the call (D-11) |

Transforms and Command Mode (D-29..D-45) use the same provider with FreeFlow's SELECTED_TEXT/VOICE_COMMAND prompt shape (MIT) and the D-204 size check; Command Mode without a selection should stay limited to writing tasks (D-45). Notetaker summaries (E-18, E-23) need `-c 16384` or more, or chunked map-reduce; Apple FM's 4-8k context forces chunking (§2.1).

## 7. Needs a benchmark on a real Mac

1. **B-1** `llama-server` cold start to `/health` 200 (process spawn + Metal library compile + mmap) for each default model, on M1 8 GB and M-series 16 GB; second start (shader cache?) (unk).
2. **B-2** Tokens/s prefill and generation per candidate (Qwen3.5-2B, Qwen3-1.7B, Qwen3-4B-2507, Qwen3.5-4B, Gemma-4-E2B, Ministral-3-3B, Granite-4.2-3B, Phi-4-mini) at Q4_K_M and Q4_0, ctx 4096 (unk).
3. **B-3** Peak RSS of `llama-server` per model with `-c 4096 --cache-ram 256`, and RAM headroom on an 8 GB Mac with the ASR model loaded (D-213) (unk).
4. **B-4** Cleanup quality bake-off on Disfl-QA + AMI + the adversarial set: answer rate, meaning drift, filler removal, language kept, per model and per prompt (OpenWhispr vs FreeFlow vs trimmed) (unk).
5. **B-5** JSON-schema output vs plain text: quality and speed impact, and cost of large `maxLength` grammars (unk).
6. **B-6** Prefix-cache reuse for hybrid Qwen3.5 with default vs `-cms 0` (unk).
7. **B-7** Apple FM latency (cold, `prewarm`ed), guardrail refusal rate on the adversarial set with default vs `.permissiveContentTransformations`, `rateLimited` behavior for a background menu-bar app, real `contextSize` on 26.x vs 27 (unk).
8. **B-8** End-to-end key-up → paste time with cleanup on, p50/p95, against the deadline in §6 (unk).
9. **B-9** Intel Mac CPU-only speed with Qwen3.5-0.8B/2B to decide whether Intel gets LLM cleanup at all (unk).
10. **B-10** Signing/notarization of the static `llama-server` sidecar with Handy's entitlements; Gatekeeper launch on a clean Mac (unk).
11. **B-11** GPU contention: cleanup while a Notetaker stream or the next dictation uses transcribe-cpp Metal (AF-04, unk).
12. **B-12** Esc during generation actually frees the slot within 100 ms (connection-close abort) (inf from code, verify).

## 8. Summary

1. Runtime: adopt llama.cpp `llama-server` (MIT, 132.7 k stars, daily releases) as a separate sidecar, which satisfies AF-01.
2. Handy's existing JSON-schema request shape is accepted unchanged by `llama-server` (`server-common.cpp:1392-1394`).
3. Ship it as a static, Metal-embedded `externalBin` built from a pinned tag. The prebuilt release is 29 MB of shared dylibs with `@loader_path` rpath and an ad-hoc signature. Tauri signs both sidecars and `frameworks` dylibs.
4. Override the memory defaults: ctx = full model context, `--cache-ram` 8 GiB, web UI on.
5. Apple FM: adapt Hex's MIT bridge, which adds a timeout with `Task.cancel`, availability reasons and `contextSize`, and wrap it in `spawn_blocking` (AF-14). Use it as an optional zero-download path.
6. Models: Qwen3.5-2B (≤ 8 GB) and Qwen3-4B-Instruct-2507 (≥ 16 GB), both Apache-2.0. Gemma 4 is Apache-2.0 too, but its file is large.
7. Rejected for license: Llama 3.2, Gemma 3, Qwen2.5-3B (research), LFM2.5/LFM3 ($10 M threshold).
8. Prompts: OpenWhispr `cleanupPrompt` and FreeFlow `defaultSystemPrompt`/`commandModeSystemPrompt` (both MIT). Guard: FreeFlow's instruction-execution heuristic.
9. Guards: schema `maxLength`, length ratio, diff/entity preservation, language check, and a deterministic fallback. Never paste raw model output.
10. Eval: Disfl-QA (CC BY 4.0), AMI-disfluent, DisfluencySpeech, plus our own adversarial set. Twelve benchmarks need a real Mac.

**Recommended stack:** `llama-server` (pinned llama.cpp, static Metal sidecar) + built-in `local` provider in `llm_client.rs` (structured output, temp 0, max_tokens, one call, one deadline) + small supervisor (health, sleep-idle, restart, CPU fallback) + Qwen3.5-2B / Qwen3-4B-Instruct-2507 GGUF via Handy's hf-hub catalog + Apple Foundation Models (non-blocking bridge) as an optional fast path + FreeFlow/OpenWhispr-derived prompts and guards.
