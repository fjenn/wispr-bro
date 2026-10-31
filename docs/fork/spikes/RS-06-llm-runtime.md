# RS-06: local LLM runtime (partial run, one M5 Max, ad-hoc signed)

> **Partial.** Run on **one Mac, an M5 Max with 128 GB**, with the
> sidecar built by `ci/llama-server/build.sh` and signed **ad hoc** (the
> owner's Developer ID is not on this Mac, F-02 is open). The M1 8 GB rows,
> macOS 13, Gatekeeper on a notarized build, the cold-cold row and real
> sleep are **not covered**; they are listed at the end with what to run.
> Every latency below is a lower bound for an M1.

## Verdict

- **Transport (DEC-49): Unix socket, confirmed.** The pinned b11624 serves
  `/health` and chat completions on `--host <app data>/llm/llm.sock`, opens
  **no inet socket** (`lsof -i`: 0), socket mode `755` (only the user can
  connect). Loopback TCP with the key is kept as the fallback for paths that
  do not fit `sockaddr_un` (tested with the supervisor's fake server and the
  provider test).
- **The static, M1-baseline, macOS-13-target build works** and runs under
  the hardened runtime with **no entitlement** (Metal compiles its shaders
  fine). 18.5 MB, only system dylibs (`Accelerate`, `Foundation`, `Metal`,
  `MetalKit`, `CoreFoundation`, `libSystem`, `libc++`, `libobjc`),
  `minos 13.0`, no TLS.
- **Warm Light cleanup is fast here**: 189 English items ≤ 50 words with
  Muesli 2B v12, the OpenWhispr prompt and the JSON schema: **p50 150 ms,
  p95 306 ms** end to end through the app's client, 0 deadline misses, 0
  unparsable replies, 154 tok/s generation. Budget (§5.1, M1 8 GB) p95
  ≤ 2.2 s: met by 7× here; the M1 run decides.
- **Memory**: `llama-server` peak **phys_footprint 558 MiB, RSS 1,711 MiB**
  (RSS counts the memory-mapped 1.27 GB model file). With a speech model
  loaded and transcribing in the same run (Parakeet TDT 0.6B v3 Q8 in the
  bench process): **combined peak footprint 1,784 MiB, combined RSS
  2,873 MiB**, under the 3.2 GB ceiling of §5.2 (on 128 GB, so no memory
  pressure).
- **Abort ≤ 100 ms: met, after a fix.** With a non-streamed request the slot
  stayed busy for **+590 ms** after Esc: b11624 checks a non-streaming
  client for a hang-up once per second (`HTTP_POLLING_SECONDS = 1`,
  `tools/server/server-context.cpp`). The client now always streams; the
  server sees the closed socket at the next token: **cancel returns in
  0.1 ms, the slot is free again within +19 ms p95** (10 trials).
- **Crash and restart**: `kill -9` of the sidecar mid-request fails the
  request in **24 ms** (`Transport` error → the take fails open) and the
  supervisor has a new server ready in **1.3-2.0 s**. The parent-death
  watchdog kills the sidecar when the app is killed (unit test: < 500 ms;
  in-app run below).
- **Cold start to `/health`**: **p50 603 ms** (page cache warm), but the
  **first start of a newly built or copied binary takes ~3.5-4.1 s**: Metal
  compiles the embedded shader library and caches it per binary. Reproduced
  twice (fresh build, and a byte-identical copy at a new path: 4,100 ms, then
  527 ms). S-10 pays this once in the background right after the model is
  ready (and after each update), so no take sees it.
- **No network**: built without TLS (`-hf` fails with "HTTPS is not
  supported"), spawned with `--offline`, env cleared. The six hosts the
  binary's strings contain (download code for `-hf`/`-dr`, docs links) are
  in `ci/inert-hosts.txt` with that proof; `wb-guard-bundle-hosts.ts` passes
  on the sidecar.

## Setup

| Item       | Value                                                                                                                                                                                                                                                                                                                                      |
| ---------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ |
| Machine    | Apple M5 Max (`Mac17,6`), 128 GB, macOS 26.6.2 (25G83). Shared with other agents: load average 15-27 during the runs, so latencies are noisy upward                                                                                                                                                                                        |
| Sidecar    | llama.cpp **b11624** (`af0579c78a07`), source tarball sha256 `9a44be53…249f` from GitHub, built by `ci/llama-server/build.sh`: static, `GGML_METAL_EMBED_LIBRARY=ON`, `GGML_NATIVE=OFF`, `GGML_CPU_ARM_ARCH=armv8.4-a+fp16+dotprod`, `GGML_BLAS=OFF`, `LLAMA_OPENSSL=OFF`, no UI; binary sha256 `a7fc3216…9949`, ad hoc + hardened runtime |
| Spawn line | `-m <gguf> --alias local --host <dir>/llm.sock -c 4096 -np 1 --threads 8 --cache-ram 256 --reasoning off --jinja --no-ui --no-slots --offline`, env `PATH`, `HOME`, `LLAMA_API_KEY` only                                                                                                                                                   |
| Model      | Muesli 2B v12 Q4_K_M (`Muesli-HQ/qwen35-2b-muesli-cleanup-v12-gguf` @ `e35af097`, sha256 `2d59d211…4143`), the DEC-03 default                                                                                                                                                                                                              |
| Client     | The product code: `fork::llm::supervisor::Supervisor::complete` → `fork::llm::chat` (streamed, temperature 0, seed 42, JSON schema, `enable_thinking: false`), OpenWhispr prompt from `fork::llm::prompt`                                                                                                                                  |
| Items      | `eval/cleanup/items.jsonl`, English items with ≤ 50 words (189)                                                                                                                                                                                                                                                                            |
| ASR load   | Parakeet TDT 0.6B v3 Q8_0 through transcribe-cpp (Metal) in the bench process, transcribing a 20 s clip in a loop during 40 cleanups                                                                                                                                                                                                       |
| Memory     | `proc_pid_rusage` (`ri_phys_footprint` = Activity Monitor "Memory", incl. Metal; `ri_resident_size`), sampled every 100 ms                                                                                                                                                                                                                 |
| Harness    | `src-tauri/src/fork/llm/rs06.rs` (`rs06_bench`) and the ignored `real_*` tests; raw JSON in `~/wb-bench/results/rs06/` (not committed)                                                                                                                                                                                                     |

## Results

| Measure                                    |        p50 |              p95 |      max | n   |
| ------------------------------------------ | ---------: | ---------------: | -------: | --- |
| Spawn → `/health` OK (cache warm)          |     603 ms | 3,814 ms (first) | 3,814 ms | 5   |
| First request after start                  |     334 ms |           400 ms |   400 ms | 5   |
| Light ≤ 50 words, warm, end to end         | **150 ms** |       **306 ms** | 1,509 ms | 189 |
| Prompt eval (server timing)                |      53 ms |            64 ms |   139 ms | 189 |
| Light while ASR (Metal) is busy            |     179 ms |           269 ms |   556 ms | 40  |
| ASR 20 s clip while cleanups run           |      84 ms |           150 ms | 1,368 ms | 70  |
| Short request, baseline                    |      80 ms |           108 ms |   108 ms | 5   |
| Esc → short request answered (streamed)    |      92 ms |           100 ms |   100 ms | 10  |
| Slot freed after Esc (extra over baseline) |            |       **+19 ms** |          | 10  |
| Same, non-streamed client (before the fix) |            |          +676 ms |          | 10  |
| Crash mid-request → request fails          |      24 ms |                  |          | 1   |
| Crash → new server ready                   |  0.3-2.0 s |                  |          | 3   |

| Memory (MiB)                                   | phys_footprint |   RSS |
| ---------------------------------------------- | -------------: | ----: |
| `llama-server` alone, peak during 189 cleanups |            558 | 1,711 |
| Bench process with Parakeet 0.6B loaded        |            879 |       |
| Both, peak, ASR transcribing during cleanups   |      **1,784** | 2,873 |

Prefix cache: the median request reused **532 cached prompt tokens** (the
system prompt), so prompt eval is only the transcript (`--cache-ram 256`,
default `cache_prompt`). No `-cms` tuning was needed for this prompt size.

**Contention**: no crash, no error and no unparsable reply with ASR and the
LLM sharing the GPU; cleanup p95 went from 306 to 269 ms (noise; the busier
set was smaller) and ASR p95 stayed at 150 ms for a 20 s clip.

**Wake (simulated)**: `SIGSTOP` for 5 s then `SIGCONT` and `on_wake`: the
server is kept (`/health` in 0-1 ms). Killed while stopped: back in 321 ms
(the crash restart), and the next request works.

**Abort detail.** The first bench run used `stream: false` and measured
cancel-to-next-reply p50 677 ms against an 86 ms baseline. Reading the
server: `server_response_reader::next` waits on its queue with a 1 s
timeout and checks `should_stop` (connection closed) only between waits; a
streamed request writes every token and notices the hang-up at once. Fix in
`fork/llm/chat.rs` (`stream: true`, SSE assembly with timings); the second
run is the one in the table.

## In-app run (portable profile)

See "In-app check" below, filled from `~/wb-bench/results/rs06/app-run.txt`
(debug bundle built with `tauri build --debug --config
src-tauri/tauri.fork.conf.json --bundles app`, ad-hoc signed, launched with
`WISPRBRO_SELFCHECK=1 WISPRBRO_SELFCHECK_LLM=1` from a portable copy whose
only speech model is Parakeet TDT CTC 110M; nothing of the owner's app data
was used).

### In-app check (2026-10-30, 21:00-21:06)

| Step                                          | Result                                                                                                                                         |
| --------------------------------------------- | ---------------------------------------------------------------------------------------------------------------------------------------------- |
| Fresh profile, speech model on disk           | `fork_llm_status` over IPC at 6 s: `downloading` (the silent download started at launch)                                                       |
| Muesli download from Hugging Face (1.27 GB)   | Verified and ready after **196 s**; file in `Data/huggingface/hub/models--Muesli-HQ--…/snapshots/e35af097…/`                                   |
| First-launch warm-up                          | Server ready in **3,807 ms**, then stopped (the Metal shader compile, paid in the background)                                                  |
| Try it (selfcheck), server stopped            | Start 621 ms + cleanup 730 ms; "um so I was thinking we could uh meet on friday no wait thursday" → "I was thinking we could meet on Thursday" |
| Try it, warm                                  | **100 ms**                                                                                                                                     |
| Transport                                     | Unix socket (`unix` in the log), 0 inet sockets (`lsof -a -i`), 1 connection on `llm.sock`                                                     |
| `kill -9` of the app with the sidecar running | Sidecar gone within 1 s (kqueue watchdog)                                                                                                      |
| Relaunch                                      | Model ready in 0.2 ms (verification marker, no re-hash); start 612 ms, cleanup 193 ms, then 101 ms warm                                        |
| Idle                                          | `llm: idle for 120 s, stopping the server` 123 s after the last request; no `llama-server` process left                                        |
| Quit                                          | No `llama-server` left                                                                                                                         |

The F-05/F-08 IPC self-check reported FAIL in this run only because its CSP
media probe plays a file through the asset protocol, whose scope is
`$APPDATA/**`, and a portable profile keeps its data elsewhere ("audio
error 4"). That is a limit of the CSP probe in portable mode (F-08), not of
S-10; every other check, including `fork_llm_status`, passed.

## Signing and Developer ID (F-02)

- `tauri build` re-signs `Contents/MacOS/llama-server` with the app's
  identity, `--options runtime` and the app's `Entitlements.plist`
  (verified: `flags=0x10002(adhoc,runtime)`, entitlement
  `com.apple.security.device.microphone`). With `APPLE_SIGNING_IDENTITY`
  set, F-02 gets a Developer-ID-signed, timestamped sidecar for
  notarization with no S-10 change.
- Entitlements the sidecar needs: **none** (Metal works under the hardened
  runtime without `allow-jit` or `allow-unsigned-executable-memory`). It
  inherits the microphone one because Tauri 2.11 signs all binaries with
  one entitlements file; harmless (TCC attributes access to the app).
- `spctl --assess` rejects the ad-hoc binary, as expected. **Not verified:**
  Gatekeeper and notarization of a Developer-ID build on macOS 13 and 26.

## RS-06 checklist

| R5 RS-06 item                                    | Status                                                                                                             |
| ------------------------------------------------ | ------------------------------------------------------------------------------------------------------------------ |
| CI-style static build for 13.0                   | **Done** (`build.sh`; fails on any API newer than 13.0 used unguarded). Runs on 13: not verified (no macOS 13)     |
| Signed as externalBin, notarized, Gatekeeper     | Ad hoc + hardened runtime only. **Needs F-02 (Developer ID) and a clean Mac on 13 and 26**                         |
| `/health` on `--host <path>.sock`; `--offline`   | **Done** (both)                                                                                                    |
| Cold start, prefill/gen tok/s, peak RSS (Muesli) | **Done on M5 Max**. Other candidates: RS-07 table (same build family, dynamic binary). **M1 8 GB and 16 GB: open** |
| With the ASR model loaded                        | **Done on M5 Max** (Parakeet 0.6B v3)                                                                              |
| `-cms` prefix reuse                              | Prompt cache reuses the 532-token system prompt by default; `-cms` not needed at this size                         |
| Cleanup while ASR Metal is busy                  | **Done**, no crash, no error                                                                                       |
| Esc frees the slot                               | **Done**: +19 ms p95 with streaming (≤ 100 ms)                                                                     |
| Cold ASR + cold LLM, ordered loads, M1 8 GB      | **Open (needs M1 8 GB)**. The planner is unit-tested                                                               |
| 2B Light p95 ≤ 2.2 s on M1 8 GB, RSS ≤ 3.2 GB    | **Open (needs M1 8 GB)**. M5 Max: 306 ms, 2,873 MiB RSS combined                                                   |
| Intel                                            | Skipped (DEC-29)                                                                                                   |

## Needs the M1 8 GB or a signed build

1. **M1 8 GB (macOS 14)**: run the bench (Mac test script S-10.2-S-10.4),
   the cold-cold row (fresh boot, first take with ASR and LLM both cold),
   and count how often the memory rule (available ≥ 2,100 MiB + 1.5 GB)
   skips cleanup during a normal session. With 8 GB and a speech model
   loaded, "available" (free + inactive + purgeable) is often below 3.6 GB,
   so the rule may skip most takes. If it does, the rule should use the
   sidecar's phys_footprint (558 MiB here; the 1.27 GB of weights are
   clean, file-backed pages the kernel can drop) plus a margin rather than
   RSS. That is a one-constant change in `catalog.rs`, to decide on M1 data.
2. **M1 first-launch Metal compile**: 3.5 s here; likely longer on an M1.
   The background warm-up hides it unless the user dictates within that
   window right after setup.
3. **Signed build**: Developer ID + notarization, Gatekeeper on macOS 13
   and 26 from a quarantined download (S-10.1).
4. **macOS 13**: the binary is built for 13.0 and BLAS is off to avoid the
   only 13.3+ API the build used; it has not run on 13.
5. **Real sleep/lid close** mid-take (simulated here with SIGSTOP/SIGCONT;
   the wake observer is the S-04 one, which works).
6. **`wb-egress`** with the bundled sidecar (no egress workflow run yet).

## Reproduce

```sh
ci/llama-server/build.sh
cd src-tauri
WB_LLAMA_SERVER=$PWD/target/wb-sidecar/llama-server-aarch64-apple-darwin \
WB_LLM_MODEL=~/wb-bench/llm/qwen35-2b-muesli-v12-Q4_K_M.gguf \
WB_ASR_MODEL=~/wb-bench/asr/parakeet-tdt-0.6b-v3/parakeet-tdt-0.6b-v3-Q8_0.gguf \
WB_ASR_WAV=<16 kHz WAV> WB_RS06_OUT=<out.json> \
  cargo test --lib fork::llm -- --ignored --nocapture --test-threads 1 rs06 real_
```
