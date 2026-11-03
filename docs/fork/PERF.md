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

## Other §5 rows

Latency (§5.1), startup (§5.3), the 8 h idle run and the 8 GB memory-pressure
test are part 2. Settled evidence so far: RS-01 (`T_show`), RS-06/RS-08
(LLM runtime), S-03 (bar appear 163 ms p50, web view growth), S-12a (key-up →
text p50 230 ms on the M5 Max).
