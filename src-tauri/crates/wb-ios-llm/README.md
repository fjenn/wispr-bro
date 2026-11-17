# wb-ios-llm: AI cleanup inside the iPhone app

The Mac cleans a dictation through a `llama-server` sidecar (llama.cpp
b11624) on a Unix socket. An iPhone app cannot start a process, so this crate
links the same llama.cpp into the app and runs **llama-server's own request
code** in process: the same argument parser, chat template, JSON-schema
grammar, sampler, slot and prompt cache. Requests are the JSON bodies the Mac
sends; replies are the bytes the sidecar writes. Nothing of the server is
re-implemented, which is why the replies are identical (see "Evidence").

Status: Phase 1 (this crate, standalone, proven on the Mac host and built
for both iOS targets). Phase 2 (wiring into the app) is the plan at the end.

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
the same numbers on this engine. Phase 2 runs `eval_run.rs` itself.

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
back to the baseline right after unload. The iPhone number is a Phase 2
measurement.

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

Not verified yet: a real iPhone (Metal speed, footprint, Metal while the app
is in the background), the app's own path (gate, chunks, deadlines, pill)
on this engine, `hw.memsize` of an 8 GB iPhone.

## Integration (Phase 2), file by file

The supervisor already separates "start a server" (`Launcher`, `Server`)
from everything else, and it reaches the server only through
`transport.rs` + reqwest. So the iPhone swaps the launcher and nothing else:
supervisor, health polling, idle stop, crash handling, chat client, prompt,
guards, deadlines and `post_process` stay as they are.

1. **`src-tauri/Cargo.toml`**, iOS dependencies:
   `wb-ios-llm = { path = "crates/wb-ios-llm" }`. Also as an optional macOS
   dependency behind a feature `llm-inprocess`, so `eval_run.rs` and the
   supervisor tests can run on this engine on the Mac.
2. **New `src/fork/llm/inprocess.rs`** (`cfg(any(target_os = "ios", feature = "llm-inprocess"))`),
   declared in `fork/llm/mod.rs`:

   ```rust
   pub struct InProcessLauncher;
   struct InProcessServer { server: wb_ios_llm::serve::Server, tail: Arc<Mutex<OutputTail>> }

   impl Launcher for InProcessLauncher {
       fn launch(&self, spec: &SpawnSpec, on_exit: Box<dyn FnOnce() + Send>) -> io::Result<Box<dyn Server>> {
           let mut config = wb_ios_llm::Config::new(&spec.model);   // -c 4096, --cache-ram 256, threads: same constants
           config.device = match spec.device { Device::Gpu => wb_ios_llm::Device::default_for_target(), Device::Cpu => wb_ios_llm::Device::Cpu };
           let listen = match &spec.transport.endpoint { Endpoint::Unix(p) => Listen::Unix(p.clone()), Endpoint::Tcp(port) => Listen::Tcp(*port) };
           let server = serve::Server::start(listen, Some(spec.transport.api_key().into()), config)?;
           // a thread polls server.state(): on Failed(msg) → tail.push_line(msg); on_exit()
           Ok(Box::new(InProcessServer { server, tail }))
       }
   }
   impl Server for InProcessServer {
       fn pid(&self) -> u32 { std::process::id() }
       fn has_exited(&mut self) -> bool { matches!(self.server.state(), State::Failed(_) | State::Stopped) }
       fn stop(&mut self) { self.server.stop() }          // unloads the model
       fn tail(&self) -> OutputTail { … }
   }
   ```

   A failed Metal start falls back to `Device::Cpu` through the supervisor's
   existing retry.

3. **`src/fork/llm/runtime.rs` `register`**: on iOS pass
   `Some(PathBuf::from("in-process"))` as the "sidecar" and
   `Arc::new(InProcessLauncher)`; `on_ready`: skip `process::kill_stale` on
   iOS. With a model selected the state then goes `waiting_for_speech_model`
   → `downloading` → `verifying` → `ready` exactly as on the Mac.
4. **`src/fork/mod.rs`** (line ~611): run `llm::runtime::on_ready(app)` on iOS
   too (keep `llm::instruct::on_ready` Mac-only: transforms are not on the
   phone). `fork/ios/core.rs` already calls `post_process` after a take; with
   the runtime ready it cleans instead of reporting `not_ready`.
5. **`src/fork/llm/transport.rs` `choose`**: the app-data path on iPhone is
   too long for `sockaddr_un`, so today's code would pick loopback TCP (works,
   keyed, needs no permission). Prefer no port at all: on iOS try
   `std::env::temp_dir().join("wb-llm.sock")` (the container's `tmp/`, about
   95 bytes) before TCP.
6. **Model download**: nothing new. `runtime::start_download` →
   `download::ensure` (pinned revision and sha256, resume, `hub_dir()` under
   the app's data) runs as on the Mac once `on_ready` runs. On a simulator,
   copy `models--Muesli-HQ--qwen35-2b-muesli-cleanup-v12-gguf` into the
   container's `huggingface/hub` (IOS.md "Models on a simulator").
7. **Memory rule for iPhone** (`src/fork/llm/memory.rs`, `catalog.rs`):
   - `memory::read()` for iOS: `hw.memsize`, `available_bytes` from
     `os_proc_available_memory()` (the app's real headroom before jetsam),
     pressure from a latch set by the memory warning (below).
   - `catalog::select`: on iOS the model only for phones with 8 GB or more
     (`physical >= 7 GiB`: iPhones are 6, 8 or 12 GB and report a little
     under the nominal size); below that the state stays `unsupported`.
   - New `src/fork/ios/memory.rs`: observe
     `UIApplicationDidReceiveMemoryWarningNotification` (objc2, already a
     dependency) → set pressure to critical for 60 s and call
     `supervisor.stop()` at once when no request is in flight
     (`stop_if_idle`), which unloads the cleanup model. The speech model is
     not touched; takes get the deterministic text until pressure clears
     (`memory::allows`, already consulted per take).
   - `IDLE_TIMEOUT` (120 s) already unloads an unused model; the key-down
     prewarm (`runtime::prewarm`) loads it again in ~0.3 s.
8. **Status and frontend**: `fork_llm_status` needs no change (it reports the
   runtime's state). `src/fork/ios/IosSettings.tsx` `useCleanupAvailable`
   already makes Settings › Cleanup selectable for any state other than
   `unsupported` / `missing_runtime`; add: listen to the status event so the
   row updates live, show "Downloading… N %" (`downloading`/`verifying`),
   "Not available on this iPhone" for `unsupported` (under 8 GB) instead of
   "Coming to iPhone", and remove the `soon*` strings (26 locales).
9. **Xcode**: no project change expected. `libapp.a` carries the object and
   `build.rs` emits the frameworks (Metal, MetalKit, Accelerate, Foundation);
   check the app target links them. The build phase's `SDKROOT` is removed
   for cmake by `build.rs`.
10. **Docs**: `docs/fork/IOS.md` table row "`llama-server` sidecar" →
    "the same llama.cpp in process (`crates/wb-ios-llm`)"; check NOTICE
    names llama.cpp (MIT) for the iPhone app as it does for the sidecar.

Phase 2 measurements: `eval_run.rs` on this engine (Mac, feature
`llm-inprocess`); simulator take through the self-test WAVs; iPhone 17 Pro:
load time, 50-word latency, footprint with speech + cleanup, a memory
warning, and a take while the app is in the background (if Metal is refused
there, the supervisor's CPU fallback is the path to measure).
