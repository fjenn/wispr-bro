# Performance measurements (S-38)

R5 §5 budgets, measured with `scripts/wb-soak/` and the earlier slice
harnesses. Each row names the build, machine and method. Part 1 (2026-11-02)
covers the slices merged up to S-24 and F-02; part 2 adds the later slices
and the reference machines (M1 8 GB, M-series 16 GB, Intel if DEC-29 keeps
it).

## §5.2 Idle memory (app + its WebKit processes, RSS)

Method: `python3 -I scripts/wb-soak/idle.py --pid <pid> --minutes 4 --every 20
--footprint` (the S-03 `procs` method: the app process plus the WebKit
processes it is responsible for). Portable test profile, test identity
`com.wisprbro.s38check`, started `--start-hidden` (onboarding done, so the
resting bar shows and the Hub starts hidden), models unloaded, 60 s settle.
M5 Max, macOS 26.6.2. One run each.

| Build                      | Hub parking                 | Total RSS median (MB) | App     | Hub WebContent | GPU  | Networking |
| -------------------------- | --------------------------- | --------------------- | ------- | -------------- | ---- | ---------- |
| Release, S-03 (2026-10-30) | none                        | 287-293               | 130-136 | ≈ 106          | yes  | yes        |
| Debug, S-38 part 1         | `WB_DEBUG_PARK_AFTER_S=off` | 383.2 (max 491.0)     | 192.2   | 144.2          | 33.7 | 13.2       |
| Debug, S-38 part 1         | parked after 10 s           | **216.1** (max 216.2) | 203.0   | gone           | gone | 13.1       |

- Parking the hidden Hub removed its WebContent process and, with no web view
  left, WebKit's GPU process: **−167 MB RSS** on the same build. Physical
  footprint went from 143 MB (55 + 68 + 16 + 4) to 80 MB (76 + 4).
- The debug app process is ~65 MB larger than the release one (192-203 vs
  130-136 MB), so the release total with the Hub parked is expected near
  150 MB, under the 250 MB ceiling. **Not yet measured on the release
  build**: Mac test S-38.2, queued for the Mac wave.
- Reopening through Handy's show path (a second launch) rebuilt the Hub
  (`hub park: main window rebuilt`), back to 386 MB with the window open.
- App CPU at rest was 14.8 % and 25.8 % in these runs: the profile had no
  upgraded speech model, so S-07b downloaded whisper-large-v3-turbo and ran
  its speed probe during both windows (log), on a debug build. Not an idle
  number; S-03's release run measured 0.24-0.33 %. The 10-minute release
  check is part of S-38.2.

## §5.4 Exactly-once insertion

Not run in part 1. Harness: `scripts/wb-soak/soak.py` (Mac test S-38.4,
queued; needs BlackHole 2ch). Earlier evidence: S-04's 100-take soak in 10
apps, 0 lost / 0 duplicated, 100/100 clipboards byte-identical
(`docs/fork/test-runs/1.0.0-alpha.1-M-26.md`).

## §5.1 Latency (part 2)

Harness: `scripts/wb-perf/latency.sh` (headless; `fork::perf_bench` on the
shipped recognizer, text pipeline, cleanup and Polish transform with the
real sidecar; 20 TTS takes of 8-185 words written to WAVs, never played;
the paste is added from S-04's 41.7 ms p95). The report refuses a run when
the 1-minute load average was above half the cores (exit 3).

Two runs on the M5 Max builder (2026-11-02, debug Rust build, Parakeet
unified-en 0.6B Q8_0 on Metal `MTL0`, Muesli 2B v12): both were **not a
measurement**. The Mac had a load average of 55-83 on 18 cores and another
agent's `llama-server` and Chrome test runs on the GPU; a warm 2 s take took
1.1-1.3 s in the recognizer (RS-09 on the same Mac, quiet: 48 ms p50), the
sidecar 14.6 s to start, and most Medium and long cleanups hit their
deadline and failed open (as designed). The one row that passed under that
load was the Polish transform (p50 1,947 ms, p95 2,845 ms vs 2,000/3,500).
The text pipeline took 20-34 ms per take in the debug build (one 172 ms
first call with lazy tables), under the same load.

**Queued:** a quiet M5 Max run, and the reference machines (Mac test
S-38.6). Settled evidence until then: S-12a's real-app key-up → text p50
230 ms on the M5 Max; RS-06 Light p95 306 ms; RS-09's recognizer latencies.

## §5.2 Memory pressure (part 2)

`fork::llm::memory::allows` (tests `eight_gb_memory_rule`,
`pressure_levels_map`, `the_debug_override_simulates_another_mac`;
`pressure_stop_skips_the_timeout_but_never_a_request`). With Muesli 2B
(RS-06 resident 2,100 MiB) a stopped server starts on an 8 GB Mac only with
≥ 3,636 MiB available; critical pressure skips the LLM and stops an idle
server within one 5 s idle tick. The 8 GB tier is unchanged: RS-06's
combined RSS with Parakeet 0.6B loaded was 2,873 MiB (≤ 3.2 GB). The real
M1 8 GB run (30 takes, swap growth, combined RSS) is Mac test S-38.7;
`WB_DEBUG_MEMORY` simulates its rule on any Mac.

## §5.3 Startup (part 2)

`fork/startup.rs` logs `startup: ready N ms after launch (hotkeys …, bar
…)` from the kernel's process start; `scripts/wb-perf/startup.py` launches
a portable profile N times. **Not run** (it puts the bar on screen and
needs a profile with Accessibility granted): Mac test S-38.8.

## Other §5 rows

The 8 h idle run (S-38.10), the 500-take soak (S-38.4, BlackHole) and the
50-take hardware run (S-38.9, `soak.py --hardware`) are queued. Settled
evidence so far: RS-01 (`T_show`), RS-06/RS-08 (LLM runtime), S-03 (bar
appear 163 ms p50, web view growth), S-12a (key-up → text p50 230 ms on the
M5 Max).
