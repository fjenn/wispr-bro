# wb-ios-llm: AI cleanup inside the iPhone app

The Mac cleans a dictation through a `llama-server` sidecar (llama.cpp
b11624) on a Unix socket. An iPhone app cannot start a process, so this crate
links the same llama.cpp into the app and runs **llama-server's own request
code** in process: the same argument parser, chat template, JSON-schema
grammar, sampler, slot and prompt cache. Requests are the JSON bodies the Mac
sends; replies are the bytes the sidecar writes. Nothing of the server is
re-implemented, which is why the replies are identical (see "Evidence").

The app side is `src-tauri/src/fork/llm/inprocess.rs`; how the iPhone app
uses it (CPU only, mapped weights, the model follows the session, memory
rule, numbers on a phone) is in `docs/fork/IOS.md`, "Cleanup runs in the app".

## Why our own build and not the `llama-cpp-2` crate

`llama-cpp-2` binds libllama only. Identical behaviour needs the server
layer (template, grammar, slot cache), which only llama.cpp's
`tools/server` has, pinned to exactly b11624, and its ggml symbols would
still clash with transcribe-cpp's.

## What is in here

| Path                    | What                                                                                                                                                                                                        |
| ----------------------- | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `native/build.sh`       | Builds llama.cpp b11624 (tag, commit and source sha256 of `ci/llama-server/build.sh`) for one target into `libwb_ios_llm.a`; cached per target triple, not per cargo profile                                |
| `native/CMakeLists.txt` | llama.cpp as a subproject (the sidecar's options, no executables) plus the ten `tools/server` files of its `server-context` library                                                                         |
| `native/wb_llm.{h,cpp}` | The C API (7 functions): `llama_server()`'s single-model start without the HTTP listener, then the route handlers called directly                                                                           |
| `native/exports.txt`    | The only symbols the library exports                                                                                                                                                                        |
| `src/lib.rs`            | `Engine` (load, request, unload on drop), `Config` (the sidecar's arguments), `Stop` (cancel flag, deadline, probe)                                                                                         |
| `src/serve.rs`          | `Server`: the engine behind the sidecar's own front door (Unix socket or loopback port, bearer key, `/health`, chunked SSE), so the app's supervisor, transport, health check and chat client run unchanged |
| `src/memory.rs`         | `physical_memory_bytes()`, `footprint_bytes()` (`phys_footprint`, what jetsam counts)                                                                                                                       |
| `examples/eval.rs`      | The equivalence run: eval items through the engine, its listener and a real sidecar, with the app's prompt builder and guard chain compiled from `src-tauri/src` unchanged                                  |
| `examples/coexist.rs`   | transcribe-cpp and this crate in one binary, speech and cleanup at the same time, unload/reload with the footprint; also the iOS link test                                                                  |
| `tests/`                | `real_model.rs` (stream, prompt cache, cancel, deadline, reload), `listener.rs` (reqwest on a Unix socket, as the app's client)                                                                             |

The crate is in no workspace and has its own `target/` (ignored). Its
`Cargo.lock` is a subset of `src-tauri/Cargo.lock`.

## Two ggml copies in one binary

transcribe-cpp carries its own ggml and exports it under the plain names
(`nm -gU libggml-base.a`: 985 global symbols, `_ggml_abort`, `_ggml_add`, …).
llama.cpp's ggml has the same names at a different revision, so a plain
static link is a duplicate-symbol error at best and two libraries calling
into each other's structs at worst.

`native/build.sh` therefore merges our objects and every llama.cpp archive
member they need into **one relocatable object** (`ld -r` with
`-exported_symbols_list native/exports.txt`). Every other global symbol
(`ggml_*`, `gguf_*`, `llama_*`, the C++ ones, the embedded Metal library)
becomes local to that object and can no longer meet transcribe-cpp's at the
final link. Three details:

- `-fno-common`: ggml's lookup tables are tentative definitions, and `ld -r`
  cannot hide a common symbol.
- The API functions carry `visibility("default")`; everything else is built
  hidden.
- `GGMLMetalClass` (ggml-metal's one Objective-C class; class names are
  process-wide) is renamed. With the embedded Metal library neither copy
  defines it today; the rename keeps that true.

The script fails unless the visible symbols are exactly `exports.txt`, no
llama.cpp/ggml symbol is left undefined (it would bind to transcribe-cpp's),
and the class is renamed. Each copy registers its own Metal backend; they
share nothing.

Alternative considered: build llama.cpp against transcribe-cpp's ggml (one
copy). Rejected: the two ggml revisions differ, and "identical to the Mac"
means llama.cpp's own ggml.

## Build

```bash
cd src-tauri/crates/wb-ios-llm
native/build.sh aarch64-apple-darwin      # Metal, macOS 13.0
native/build.sh aarch64-apple-ios         # Metal, iOS 17.0
native/build.sh aarch64-apple-ios-sim     # Metal compiled in; runs on the CPU (below)
cargo test --release                      # host
. ../../ios/env.sh
cargo build --release --example coexist --target aarch64-apple-ios
cargo build --release --example coexist --target aarch64-apple-ios-sim
```

`build.rs` calls `native/build.sh` for the target. The source tarball comes
from `WB_LLAMA_SRC_TARBALL`, `src-tauri/target/wb-llama-build/`,
`~/wb-bench/llama.cpp/` or GitHub, and is sha256-checked. llama.cpp needs no
patch for iOS (`LLAMA_SUBPROCESS` is off there; router, HTTP listener, web UI
and MCP are not built). CPU code uses the sidecar's M1 baseline
(`armv8.4-a+fp16+dotprod`); on iPhone it only runs where the model is loaded
(8 GB and more: A17 Pro and later).

**Simulator:** its Metal device computes wrong results with ggml (measured:
garbage text, 44 s per request, then an abort), so `Device::default_for_target()`
is the CPU there, as for the speech engine.

## API

```rust
let engine = Engine::load(&Config::new(model_path))?;      // mmap, -c 4096, all layers on Metal
let stop = Stop::with_flag(cancel_flag, Some(deadline));
let status = engine.request("/v1/chat/completions", &body_json, &stop, |sse_bytes| …)?;
drop(engine);                                               // unload

let server = serve::Server::start(Listen::Unix(sock), Some(key), config)?;  // what the app uses
```

- `Config::server_args()` is the Mac's `SpawnSpec::args()` without `--host`:
  `-m <gguf> --alias local -c 4096 -np 1 --threads <n> --cache-ram 256
--reasoning off --jinja --no-ui --no-slots --offline --fit off` (`-ngl 0` for
  the CPU), parsed by llama-server's parser.
- Temperature, seed, `max_tokens`, `response_format` (JSON schema → grammar),
  `chat_template_kwargs` and `stream` are fields of the body, as on the Mac.
- Prompt cache: the server's slot keeps the previous prompt, so the system
  prompt is evaluated once (measured: 532 of ~576 prompt tokens from the
  cache on every request) and `--cache-ram` is the sidecar's.
- Stop: looked at between tokens and once a second while the first token is
  awaited, the way the sidecar notices a closed socket; the task is cancelled
  and the slot freed. A caller with a deadline should not wait for `request`
  to return (it may take until the next token): the app's client already
  returns at its timeout and drops the connection.
- One engine per process (`Error::Busy`); load and unload any number of times.
- Every entry point runs in its own autorelease pool (Metal objects created
  on worker threads would otherwise live until the thread ends).

## Evidence (2026-11-17, M5 Max, macOS 26, load average 4-12)

**Same replies.** 302 requests (the 272 items of `eval/cleanup/items.jsonl`
plus the ten 50-word dictations of `fork/llm/apple.rs` `SAMPLES_50`, three
times), Light, built by the app's `prompt.rs`, judged by the app's
`guards.rs`, against the sidecar of the installed Mac app
(`/Applications/Wispr Bro.app/Contents/MacOS/llama-server`, build 11624
`af0579c78`):

| Path                         | Identical raw replies | Identical guard verdicts | Guard accepted          |
| ---------------------------- | --------------------- | ------------------------ | ----------------------- |
| `Engine::request` vs sidecar | **302 / 302**         | 302 / 302                | 285 / 302 (94.4 %) both |
| `serve::Server` vs sidecar   | **302 / 302**         | 302 / 302                | 285 / 302 (94.4 %) both |

Scored by `scripts/wb-eval-cleanup.ts --replay` (272 items): damage 0.0 %,
answer 0.0 %, filler removal 78.0 % (English 87.3 %), language 100 %, guard
catch 100 %, the same line for both engines. This run sends every item as one
request with no language (the scorer's `none` mode; S-12a reports 81.7 % for
it) and without the app's "already clean" gate and chunking, which live in
`post_process.rs` (needs Tauri). The app's 95.0 % is its shipped path with the
owner's languages; because the replies are byte-identical, that path gives
the same numbers on this engine ("In the app" below has that run).

**Latency**, wall time per request, warm:

|                        | All 302, p50 / p95 | 50-word takes (n = 30), p50 / p95 | Tokens/s |
| ---------------------- | ------------------ | --------------------------------- | -------- |
| `Engine::request`      | 110 / 299 ms       | **299 / 337 ms**                  | 242      |
| `serve::Server` (HTTP) | 114 / 308 ms       | **308 / 351 ms**                  | 237      |
| Sidecar (Mac app's)    | 115 / 309 ms       | **309 / 351 ms**                  | 234      |

For reference, S-12a: 115 / 183 ms per item on its set; RS-06: 150 / 306 ms
for ≤ 50 words under load. Load: 0.25-0.3 s warm (first load of a new binary
2.4-3.2 s, the one-time Metal pipeline compile); unload 25 ms.

**Stop.** Cancel during a reply returns in 25 ms; a request dropped by
reqwest frees the slot (next reply in 78 ms); a deadline that passes during
prompt evaluation returns at the first token (34 ms).

**Memory** (`phys_footprint`). Mac, Metal: the model adds about 285 MB (the
weights are mapped, not copied) and the footprint is back within 40 MB of the
speech-only figure 3 s after an unload. Simulator, CPU: 1.39 GB while loaded,
back to the baseline right after unload (weights repacked into a copy; see
"In the app").

**Both libraries in one binary.** `examples/coexist.rs` (transcribe-cpp
Parakeet 110m on Metal + Muesli): 6 transcriptions and 6 cleanups at the same
time, results unchanged; three unload/reload rounds; the speech model answers
afterwards. Links for `aarch64-apple-ios` and `aarch64-apple-ios-sim` with no
duplicate symbol, and runs in the iOS 27 simulator
(`xcrun simctl spawn <sim> target/aarch64-apple-ios-sim/release/examples/coexist …`):
same cleaned text as the Mac, load 0.85 s, 26-word cleanup 0.5 s on the CPU.

Reproduce:

```bash
cargo run --release --example eval -- --model ~/wb-bench/llm/qwen35-2b-muesli-v12-Q4_K_M.gguf \
  --items ../../../eval/cleanup/items.jsonl --out <dir> --listener \
  --sidecar "/Applications/Wispr Bro.app/Contents/MacOS/llama-server"
WB_LLM_MODEL=~/wb-bench/llm/qwen35-2b-muesli-v12-Q4_K_M.gguf \
  cargo test --release -- --ignored --nocapture
```

Numbers on an iPhone: `docs/fork/IOS.md`.

## In the app

`src-tauri/src/fork/llm/inprocess.rs` (`InProcessLauncher`) implements the
supervisor's `Launcher`: it starts `serve::Server` on the transport the
supervisor chose and reports a failed load as the server's exit. Nothing
else of the cleanup stack knows the difference: supervisor, health polling,
idle stop, chat client, prompt, guards and deadlines are the Mac's.

- iPhone: always (`fork/llm/runtime.rs` `server_launcher`), on the CPU with
  `--no-repack` (see `docs/fork/IOS.md` for why).
- Mac: with the `llm-inprocess` feature and `WB_LLM_INPROCESS=1`, for the
  eval and tests (`WB_LLM_DEVICE=cpu` for the iPhone's CPU path):

  ```bash
  WB_LLM_INPROCESS=1 WB_LLM_DEVICE=cpu WB_LLAMA_SERVER=in-process \
    WB_LLM_MODEL=~/wb-bench/llm/qwen35-2b-muesli-v12-Q4_K_M.gguf \
    WB_EVAL_LANGUAGE=owner WB_EVAL_OUT=run.jsonl \
    cargo test --features llm-inprocess --lib fork::llm::eval_run::eval_run_through -- --ignored
  bun scripts/wb-eval-cleanup.ts --replay run.jsonl --level light
  ```

  The app's own path on this engine (2026-11-17, 271 items, Light, language
  mode `owner`), on the CPU and on Metal alike: damage 0.0 %, answer 0.0 %,
  filler removal 95.1 %, language kept 100 %, guard catch 100 %: every
  S-12a gate passes (the sidecar's documented run: 95.0 %).

Memory on the CPU: llama.cpp repacks the weights into a second copy unless
`--no-repack` is given (Mac, 6 threads: 1.58 GB of footprint with the copy,
0.55 GB without; 50-word take 0.89 s against 0.94 s).
