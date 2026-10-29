# RS-07: cleanup quality bake-off (provisional run)

> **PROVISIONAL.** The F-10 eval labels are single-labelled (`labels.second =
null` on all 229 items). The planned second human labeller pass and the
> inter-labeller agreement (R5 §11.1, F-10 exit criterion) have **not** been
> done. Every number below inherits that uncertainty; small differences
> between variants (a few items) are within label noise. The guard chain is
> the F-10 **TypeScript stub** of R5 §4.5, not S-12a's Rust guards, which do
> not exist yet. Re-run RS-07 after the second labelling pass and after
> S-12a.
>
> **One Mac only:** an Apple M5 Max with 128 GB. RS-06's 8 GB M1 numbers
> (latency budget, combined RSS ≤ 3.2 GB, memory pressure) are **not**
> covered; latency here is far below what an M1 will show.

## Verdict

- **No model × prompt × format passes every RS-07 Light gate** (provisional
  labels, stub guards). By R5 §7 RS-07 that triggers the fallback ("ship
  Light as deterministic + LLM only for punctuation/casing with a stricter
  guard; Medium and styles marked Beta"). Because the labels are unverified
  and the guards are a stub, treat this as **"fallback provisionally
  triggered"**, not as a decision. Re-run after the second labelling pass,
  after S-12a's Rust guards, and on an 8 GB M1.
- Closest to passing (user-facing Light gates only, i.e. ignoring the guard
  catch-rate gate):
  - **Gemma-4-E2B + trimmed + JSON:** damage 1.3 %, answer 0 %, language
    100 %, filler 89.7 % (one span short of 90 %).
  - **Qwen3-4B-Instruct-2507 + trimmed + JSON:** filler 92.6 %, damage 3.9 %.
  - **Muesli 2B v12 + OpenWhispr + JSON:** filler 92.6 %, damage 3.5 %.
- **The guard catch-rate gate (≥ 90 %) fails almost everywhere.** It passes
  in 6 of 60 rows, and none of the 6 is a real pass:
  - three rows rejected every reply (catch 100 % because nothing got
    through);
  - two Qwen3.5-0.8B rows had only 2-3 damaging outputs to catch;
  - Qwen3-1.7B + trimmed + plain caught 91/100, but accepted only 58 % of
    replies.

  Most damage the stub misses is a dropped word (a discourse marker, or the
  wrong half of a self-repair). An entity, length or language guard cannot
  see that by design.

- **JSON schema beats plain** for general models: it lowers pre-guard damage
  in most rows, often by half or more (Qwen3.5-2B + OpenWhispr 3.1 % vs
  10.5 %; Qwen3-1.7B + OpenWhispr 9.6 % vs 20.5 %). Exception: Gemma-4-E2B +
  FreeFlow + JSON is broken (see "Failures").
- **The trimmed prompt has the lowest (or tied-lowest) pre-guard damage for
  6 of 7 models**, excluding degenerate rows. The exception is Qwen3.5-0.8B,
  where OpenWhispr + JSON gives 0.9 % vs 1.3 %. The SpeakoFlow general
  prompt is the worst for every general model: 15-36 % pre-guard damage,
  and a 21-36 % pre-guard answer rate on Qwen.
  OpenWhispr's seed asks for number/date conversion and grammar fixes. Both
  are outside Light, so it damages more than the trimmed version on strong
  models (Qwen3-4B: 23.6 % vs 5.2 % pre-guard).
- **Small Qwen3.5 models barely remove fillers:** 0.8B removes 6-35 %, 2B
  removes 34-79 % post-guard. **Qwen3.5-0.8B is not a viable 8 GB fallback**
  on this evidence.

## Setup

| Item        | Value                                                                                                                                                                                                                                                                                                                                                                    |
| ----------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ |
| Machine     | Apple M5 Max (`Mac17,6`), 18 CPU cores (6 performance + 12 efficiency), 40-core GPU (Metal 4), 128 GB (137,438,953,472 B)                                                                                                                                                                                                                                                |
| macOS       | 26.6.2 (25G83)                                                                                                                                                                                                                                                                                                                                                           |
| llama.cpp   | official `ggml-org` release **b11624** (`llama-b11624-bin-macos-arm64.tar.gz`, sha256 `5bd8555c20e5d6d45831c038f484769245e5bcc1c265c4911c6ae15e4d9a35df`), `version: 0.6.0-dev (build 11624, commit af0579c78)`, AppleClang 21.0.0. Prebuilt binary (dynamic dylibs), **not** the CI static build R5 §4.5 specifies. Source tarball (sha256 `9a44be53…249f`) not needed. |
| Spawn line  | `llama-server -m <gguf> --host 127.0.0.1 --port <random free> -c 4096 -np 1 --cache-ram 256 --reasoning off --jinja --no-webui --offline`. Env scrubbed to `PATH`, `HOME`, random `LLAMA_API_KEY`. Never `-hf`; the runner refuses a non-loopback host or `-hf`.                                                                                                         |
| Items       | `eval/cleanup/items.jsonl`, all **229** items (no cut), sha256 `8af5f6fb199c45eebc6f004a54be94b44021ad8531019b8bba75196b619b84ce`                                                                                                                                                                                                                                        |
| Requests    | 60 runs, 16,946 requests, 76 min of request time (about 85 min wall clock including loads and hashing)                                                                                                                                                                                                                                                                   |
| Sampling    | temperature 0, seed 42, one request at a time; `chat_template_kwargs.enable_thinking=false` on chat requests; Muesli card template: `top_k 1`, `repeat_penalty 1`                                                                                                                                                                                                        |
| Scorer      | `scripts/wb-eval-cleanup.ts` (F-10) `evaluate()` / `summarize()` / `renderMarkdown()`, unchanged; R5 deadline enforced (a miss fails open)                                                                                                                                                                                                                               |
| Guards      | F-10 `stubGuard` (TypeScript stand-in for R5 §4.5; `fork/llm/guards.rs` does not exist yet). "Pre" = raw model output; "post" = after the stub guard, failing open to the input.                                                                                                                                                                                         |
| Harness     | `scripts/wb-bench-cleanup.ts` (this spike), tests in `scripts/wb-bench-cleanup.test.ts`                                                                                                                                                                                                                                                                                  |
| Raw outputs | `eval/cleanup/reports/rs07-2026-10-29/` (gitignored by `reports/.gitignore`, so **not committed**). Contents: one scorer `.md` + `.json` per model × prompt × format, `summary.json`, and a `*.server.log` per model.                                                                                                                                                    |

**Load.** The Mac was not idle. `fseventsd` was at about 100 % of one core
throughout. Load average rose from about 5 to about 13 during the Qwen3-4B
and Muesli runs, likely from other sessions on the machine. Latency figures
are therefore noisy. Quality numbers are not affected (temperature 0, fixed
seed).

### Prompts

All upstream licences were checked against the `LICENSE` file at the pinned
commit: all three repos are MIT, as R4 says. They are reproduced in
`eval/cleanup/prompts/LICENSES-upstream.md`. No Wispr Flow material was used.

| Variant                  | File                                              | Source (commit)                                                                                                                                                                                            | Licence                                         | Prompt chars | User-message framing                                                                                                 |
| ------------------------ | ------------------------------------------------- | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | ----------------------------------------------- | -----------: | -------------------------------------------------------------------------------------------------------------------- |
| openwhispr (seed)        | `eval/cleanup/prompts/openwhispr.txt`             | OpenWhispr `src/locales/en/prompts.json` `cleanupPrompt` (`b9e9ebefac4b961456c269164a93ddfdd53438b9`), `{{agentName}}` → `Assistant`                                                                       | MIT, © 2024 OpenWhispr Team                    |        2,223 | OpenWhispr `wrapCleanupTranscript`: `<transcript>…</transcript>` + "Output only the cleaned transcript."             |
| freeflow                 | `eval/cleanup/prompts/freeflow.txt`               | FreeFlow `Sources/PostProcessingService.swift` `defaultSystemPrompt` (`a647469d268d2265f036b4024c509e14eea7fa1c`)                                                                                          | MIT, © 2026 Zach Latta                         |        6,118 | FreeFlow `RAW_TRANSCRIPTION` block; FreeFlow's dictation-language line for non-English; app-context section left out |
| speakoflow               | `eval/cleanup/prompts/speakoflow.txt`             | SpeakoFlow `src-tauri/src/settings.rs` `IMPROVE_TRANSCRIPTIONS_PROMPT` (`9818a17fcd124e5f1ea6d34780ddea9b154322d8`)                                                                                        | MIT, © 2026 Abhishek Barali, © 2025 CJ Pais   |        7,577 | raw transcript (SpeakoFlow sends it as is)                                                                           |
| trimmed                  | `eval/cleanup/prompts/trimmed.txt`                | Our Light/Medium cut of the OpenWhispr seed: no ITN, no spoken punctuation, no formatting; level-aware                                                                                                     | MIT (derivative), modification notice in header |        1,742 | R5 §4.5 `BEGIN/END TRANSCRIPT` + JSON field (F-10 `userMessage`)                                                     |
| native (Muesli)          | `eval/cleanup/prompts/native-muesli.txt`          | model card `Muesli-HQ/qwen35-2b-muesli-cleanup-v12-gguf` rev `e35af097be4f17d3673515fb321ce19aa300faf6`                                                                                                    | Apache-2.0 (card)                               |          825 | card: one user turn, raw ChatML to `/completion`, empty `<think>` block, stop `<\|im_end\|>`, max 1,024 tokens       |
| native (SpeakoFlow Mini) | `eval/cleanup/prompts/native-speakoflow-mini.txt` | SpeakoFlow `SPEAKOFLOW_MINI_SYSTEM_PROMPT` (same commit). It is byte-identical to the prompt on the card (`SpeakoFlow/speakoflow-mini` rev `835431771f72820251fe6c6b4b07f12b000e2647`), which was checked. | MIT (text), Apache-2.0 (model)                  |        1,020 | card: system = trained prompt, user = raw transcript, `max_tokens` 2,048                                             |

FreeFlow's `commandModeSystemPrompt` (R4's pointer, line 117) is the
Transforms prompt. The dictation cleanup prompt beside it,
`defaultSystemPrompt`, is the one compared here.

Formats:

- **json:** `response_format` JSON schema `{text: string}`, which llama-server
  enforces as a grammar. General prompts also get one appended line: `Reply
only with JSON: {"text": "<the cleaned text>"}`. The native templates get
  the grammar only, so the trained prompt stays byte-exact.
- **plain:** no schema. The trimmed reply is wrapped as `{"text": …}` so that
  the same guard chain runs; FreeFlow's `EMPTY` sentinel maps to "".

Medium was run only for `trimmed`, the only level-aware prompt. The
upstream prompts have no level, so a "Medium" run of them would have repeated
the Light requests.

## Models

Peak RSS is that of the `llama-server` process alone, sampled with `ps -o
rss=` every 250 ms over the server's lifetime. No ASR model was loaded. RSS
from `ps` may not count all Metal (unified-memory) allocations; RS-06 should
also record `footprint` / `vmmap` numbers. The tok/s figures are the means
of llama-server's per-request `timings` across all runs.

| Model                | GGUF                                 |         Bytes | sha256                                                             | Cold start to /health | Peak RSS (llama-server) | Mean prefill tok/s | Mean gen tok/s |
| -------------------- | ------------------------------------ | ------------: | ------------------------------------------------------------------ | --------------------: | ----------------------: | -----------------: | -------------: |
| qwen3.5-0.8b         | `Qwen3.5-0.8B-Q4_K_M.gguf`           |   532,517,120 | `bd258782e35f7f458f8aced1adc053e6e92e89bc735ba3be89d38a06121dc517` |                608 ms |                 1260 MB |               1637 |            174 |
| qwen3.5-2b           | `Qwen3.5-2B-Q4_K_M.gguf`             | 1,280,835,840 | `aaf42c8b7c3cab2bf3d69c355048d4a0ee9973d48f16c731c0520ee914699223` |                706 ms |                 1918 MB |                879 |            107 |
| qwen3-1.7b           | `Qwen3-1.7B-Q4_K_M.gguf`             | 1,107,409,472 | `b139949c5bd74937ad8ed8c8cf3d9ffb1e99c866c823204dc42c0d91fa181897` |                605 ms |                 2116 MB |                711 |            119 |
| qwen3-4b-2507        | `Qwen3-4B-Instruct-2507-Q4_K_M.gguf` | 2,497,281,120 | `3605803b982cb64aead44f6c1b2ae36e3acdb41d8e46c8a94c6533bc4c67e597` |                609 ms |                 3664 MB |                252 |             54 |
| muesli-2b-v12        | `qwen35-2b-muesli-v12-Q4_K_M.gguf`   | 1,274,396,096 | `2d59d2118170e0170fa294d71eba0a701c4f26063f284f68a8bed880322c4143` |                709 ms |                 1985 MB |               1007 |             89 |
| gemma-4-e2b          | `gemma-4-E2B-it-Q4_K_M.gguf`         | 3,106,738,272 | `740185b21d22ceb83a11c3aa62ad5842ef32c70f6096d756bbee85a1e4ec34b8` |               1329 ms |                 3580 MB |                380 |             79 |
| speakoflow-mini-0.8b | `SpeakoFlow-Mini-0.8B-Q4_K_M.gguf`   |   541,903,616 | `530e0a5d4e9314e3bae2c92aecf3ab39ca48e8cfecac41372058f9fcb742329d` |                709 ms |                 1308 MB |               1164 |            163 |

## Results

### Light, every model × prompt × format

Damage, answer, filler and language are post-guard (what the user gets) unless marked pre. `acc.` = share of replies the guard accepted (the rest fail open to the input). Gates: damage ≤ 2, answer ≤ 0.5, filler ≥ 90, language ≥ 99, guard catch ≥ 90 (all %).

| Model                | Prompt     | Format | Damage pre | **Damage** | Answer pre | **Answer** | **Filler** | Filler pre | **Language** | **Guard catch** | acc. | Edit acc. | Restraint | p50 ms | p95 ms | Deadline miss | Errors | RS-07 Light                            |
| -------------------- | ---------- | ------ | ---------: | ---------: | ---------: | ---------: | ---------: | ---------: | -----------: | --------------: | ---: | --------: | --------: | -----: | -----: | ------------: | -----: | -------------------------------------- |
| qwen3.5-0.8b         | openwhispr | json   |        0.9 |        0.0 |        1.0 |        0.0 |       20.6 |       20.6 |         99.5 |           100.0 |   98 |      67.0 |     100.0 |    131 |    186 |             0 |      0 | fail (filler)                          |
| qwen3.5-0.8b         | openwhispr | plain  |        4.4 |        1.3 |        2.0 |        0.0 |       35.3 |       35.3 |         99.5 |            70.0 |   96 |      68.8 |     100.0 |     95 |    145 |             0 |      0 | fail (filler, catch)                   |
| qwen3.5-0.8b         | freeflow   | json   |        1.3 |        0.0 |        1.0 |        0.0 |        5.9 |        5.9 |         99.5 |           100.0 |   98 |      62.5 |     100.0 |    137 |    221 |             0 |      0 | fail (filler)                          |
| qwen3.5-0.8b         | freeflow   | plain  |       11.8 |        5.7 |        0.0 |        0.0 |        7.4 |        8.8 |         94.1 |            51.9 |   93 |      54.0 |      98.1 |    105 |    172 |             0 |      0 | fail (damage, filler, language, catch) |
| qwen3.5-0.8b         | speakoflow | json   |       14.8 |        2.2 |       20.6 |        0.0 |       14.7 |       14.7 |         98.9 |            85.3 |   86 |      61.9 |      98.1 |    140 |    509 |             0 |      0 | fail (damage, filler, language, catch) |
| qwen3.5-0.8b         | speakoflow | plain  |       24.5 |        7.4 |       23.5 |        0.0 |       27.9 |       29.4 |         95.2 |            69.6 |   82 |      60.2 |      92.5 |    102 |    384 |             0 |      0 | fail (damage, filler, language, catch) |
| qwen3.5-0.8b         | trimmed    | json   |        1.3 |        0.4 |        1.0 |        0.0 |       33.8 |       33.8 |         99.5 |            66.7 |   99 |      67.6 |     100.0 |    156 |    215 |             0 |      0 | fail (filler, catch)                   |
| qwen3.5-0.8b         | trimmed    | plain  |        3.1 |        1.3 |        1.0 |        0.0 |       33.8 |       33.8 |         98.9 |            57.1 |   97 |      67.6 |     100.0 |     98 |    159 |             0 |      0 | fail (filler, language, catch)         |
| qwen3.5-2b           | openwhispr | json   |        3.1 |        0.9 |        1.0 |        0.0 |       66.2 |       72.1 |        100.0 |            71.4 |   96 |      77.3 |     100.0 |    215 |    290 |             0 |      0 | fail (filler, catch)                   |
| qwen3.5-2b           | openwhispr | plain  |       10.5 |        3.1 |        9.8 |        0.0 |       79.4 |       92.6 |         99.5 |            70.8 |   92 |      76.7 |      98.1 |    159 |    239 |             0 |      0 | fail (damage, filler, catch)           |
| qwen3.5-2b           | freeflow   | json   |        1.3 |        0.4 |        0.0 |        0.0 |       33.8 |       33.8 |         99.5 |            66.7 |   98 |      69.3 |      98.1 |    234 |    375 |             0 |      0 | fail (filler, catch)                   |
| qwen3.5-2b           | freeflow   | plain  |        5.7 |        1.7 |        1.0 |        0.0 |       41.2 |       47.1 |         97.8 |            69.2 |   95 |      69.9 |      96.2 |    177 |    305 |             0 |      0 | fail (filler, language, catch)         |
| qwen3.5-2b           | speakoflow | json   |       20.5 |        5.2 |       21.6 |        0.0 |       76.5 |       91.2 |         99.5 |            75.0 |   82 |      72.7 |     100.0 |    222 |    765 |             0 |      0 | fail (damage, filler, catch)           |
| qwen3.5-2b           | speakoflow | plain  |       24.9 |        5.7 |       27.5 |        0.0 |       77.9 |       95.6 |         98.9 |            77.6 |   79 |      72.2 |      96.2 |    171 |    696 |             0 |      0 | fail (damage, filler, language, catch) |
| qwen3.5-2b           | trimmed    | json   |        1.3 |        0.4 |        1.0 |        0.0 |       60.3 |       64.7 |        100.0 |            66.7 |   98 |      77.3 |     100.0 |    221 |    339 |             0 |      0 | fail (filler, catch)                   |
| qwen3.5-2b           | trimmed    | plain  |        1.7 |        0.9 |        1.0 |        0.0 |       57.4 |       60.3 |         99.5 |            50.0 |   99 |      72.7 |     100.0 |    167 |    286 |             0 |      0 | fail (filler, catch)                   |
| qwen3-1.7b           | openwhispr | json   |        9.6 |        1.7 |        6.9 |        0.0 |       61.8 |       75.0 |        100.0 |            81.8 |   89 |      75.0 |      98.1 |    172 |    250 |             0 |      0 | fail (filler, catch)                   |
| qwen3-1.7b           | openwhispr | plain  |       20.5 |        3.5 |        8.8 |        0.0 |       36.8 |       77.9 |         97.8 |            83.0 |   82 |      66.5 |      92.5 |    123 |    192 |             0 |      0 | fail (damage, filler, language, catch) |
| qwen3-1.7b           | freeflow   | json   |        7.0 |        2.6 |        0.0 |        0.0 |       44.1 |       48.5 |        100.0 |            62.5 |   93 |      66.5 |      98.1 |    184 |    262 |             0 |      0 | fail (damage, filler, catch)           |
| qwen3-1.7b           | freeflow   | plain  |       21.4 |        4.8 |        0.0 |        0.0 |       11.8 |       35.3 |         96.8 |            77.6 |   82 |      60.2 |      88.7 |    136 |    212 |             0 |      0 | fail (damage, filler, language, catch) |
| qwen3-1.7b           | speakoflow | json   |       27.5 |        5.7 |       29.4 |        0.0 |       58.8 |       72.1 |         98.9 |            79.4 |   76 |      68.2 |      94.3 |    194 |    631 |             0 |      0 | fail (damage, filler, language, catch) |
| qwen3-1.7b           | speakoflow | plain  |       36.2 |       12.2 |       23.5 |        0.0 |       41.2 |       55.9 |         94.1 |            66.3 |   75 |      58.5 |      86.8 |    145 |    494 |             0 |      0 | fail (damage, filler, language, catch) |
| qwen3-1.7b           | trimmed    | json   |        6.1 |        3.1 |        1.0 |        0.0 |       69.1 |       72.1 |         98.9 |            50.0 |   95 |      77.3 |      94.3 |    167 |    235 |             0 |      0 | fail (damage, filler, language, catch) |
| qwen3-1.7b           | trimmed    | plain  |       42.8 |        3.9 |       26.5 |        0.0 |       25.0 |       80.9 |         99.5 |            91.0 |   58 |      56.8 |      98.1 |    147 |    263 |             0 |      0 | fail (damage, filler)                  |
| qwen3-4b-2507        | openwhispr | json   |       23.6 |        4.8 |       12.7 |        0.0 |       66.2 |       98.5 |         98.9 |            79.6 |   69 |      76.1 |      96.2 |    452 |   1817 |            23 |      0 | fail (damage, filler, language, catch) |
| qwen3-4b-2507        | openwhispr | plain  |       21.4 |        3.9 |       12.7 |        0.0 |       79.4 |      100.0 |         99.5 |            81.6 |   80 |      80.1 |      96.2 |    298 |    507 |             0 |      0 | fail (damage, filler, catch)           |
| qwen3-4b-2507        | freeflow   | json   |        7.0 |        3.9 |        2.0 |        0.0 |       88.2 |       92.6 |         97.8 |            43.8 |   95 |      81.8 |      94.3 |    445 |    627 |             0 |      0 | fail (damage, filler, language, catch) |
| qwen3-4b-2507        | freeflow   | plain  |        9.6 |        4.8 |        1.0 |        0.0 |       80.9 |       89.7 |         98.9 |            50.0 |   93 |      80.1 |      92.5 |    317 |    496 |             0 |      0 | fail (damage, filler, language, catch) |
| qwen3-4b-2507        | speakoflow | json   |       33.6 |        4.4 |       28.4 |        0.0 |       63.2 |       97.1 |         99.5 |            87.5 |   67 |      73.9 |      86.8 |    447 |   1747 |            13 |      0 | fail (damage, filler, catch)           |
| qwen3-4b-2507        | speakoflow | plain  |       33.6 |        3.9 |       36.3 |        0.0 |       64.7 |       95.6 |         99.5 |            89.0 |   67 |      75.0 |      88.7 |    325 |   1865 |            17 |      0 | fail (damage, filler, catch)           |
| qwen3-4b-2507        | trimmed    | json   |        5.2 |        3.9 |        1.0 |        0.0 |       92.6 |       97.1 |         99.5 |            25.0 |   97 |      85.8 |      94.3 |    392 |    581 |             0 |      0 | fail (damage, catch)                   |
| qwen3-4b-2507        | trimmed    | plain  |        5.7 |        2.6 |        0.0 |        0.0 |       86.8 |       94.1 |         98.9 |            53.8 |   93 |      56.3 |      41.5 |    395 |    611 |             0 |      0 | fail (damage, filler, language, catch) |
| muesli-2b-v12        | openwhispr | json   |        6.1 |        3.5 |        1.0 |        0.0 |       92.6 |       97.1 |         99.5 |            42.9 |   93 |      86.4 |      98.1 |    274 |    451 |             0 |      0 | fail (damage, catch)                   |
| muesli-2b-v12        | openwhispr | plain  |        7.4 |        3.5 |        1.0 |        0.0 |       92.6 |       98.5 |         99.5 |            52.9 |   92 |      86.4 |      98.1 |    198 |    333 |             0 |      0 | fail (damage, catch)                   |
| muesli-2b-v12        | freeflow   | json   |        7.9 |        3.1 |        2.9 |        0.0 |       70.6 |       95.6 |         99.5 |            61.1 |   75 |      81.3 |      98.1 |    626 |   2153 |            36 |      0 | fail (damage, filler, catch)           |
| muesli-2b-v12        | freeflow   | plain  |       10.9 |        5.2 |        3.9 |        0.0 |       88.2 |       98.5 |         98.9 |            52.0 |   91 |      84.7 |     100.0 |    228 |    395 |             0 |      0 | fail (damage, filler, language, catch) |
| muesli-2b-v12        | speakoflow | json   |        9.6 |        4.4 |        2.9 |        0.0 |       92.6 |       98.5 |         99.5 |            54.5 |   91 |      85.8 |      98.1 |    263 |    381 |             0 |      0 | fail (damage, catch)                   |
| muesli-2b-v12        | speakoflow | plain  |       12.2 |        5.7 |        5.9 |        0.0 |       88.2 |       98.5 |         98.9 |            53.6 |   90 |      82.4 |      96.2 |    192 |    323 |             0 |      0 | fail (damage, filler, language, catch) |
| muesli-2b-v12        | trimmed    | json   |        3.9 |        3.1 |        0.0 |        0.0 |       83.8 |       91.2 |         99.5 |            22.2 |   97 |      84.7 |      98.1 |    244 |    380 |             0 |      0 | fail (damage, filler, catch)           |
| muesli-2b-v12        | trimmed    | plain  |        6.1 |        3.9 |        1.0 |        0.0 |       86.8 |       92.6 |         99.5 |            35.7 |   94 |      83.0 |      98.1 |    187 |    312 |             0 |      0 | fail (damage, filler, catch)           |
| muesli-2b-v12        | native     | json   |        7.9 |        5.2 |        1.0 |        0.0 |       91.2 |       97.1 |         99.5 |            33.3 |   93 |      83.5 |      98.1 |    358 |    473 |             0 |      0 | fail (damage, catch)                   |
| muesli-2b-v12        | native     | plain  |       14.8 |        7.4 |        3.9 |        0.0 |       91.2 |       98.5 |         98.4 |            50.0 |   90 |      80.7 |      94.3 |    197 |    288 |             0 |      0 | fail (damage, language, catch)         |
| gemma-4-e2b          | openwhispr | json   |       11.8 |        4.4 |        2.9 |        0.0 |       80.9 |       98.5 |         99.5 |            63.0 |   89 |      83.5 |      88.7 |    470 |    648 |             1 |      0 | fail (damage, filler, catch)           |
| gemma-4-e2b          | openwhispr | plain  |       15.3 |        5.7 |        4.9 |        0.0 |       79.4 |       98.5 |         99.5 |            62.9 |   87 |      79.5 |      83.0 |    235 |    373 |             0 |      0 | fail (damage, filler, catch)           |
| gemma-4-e2b          | freeflow   | json   |      100.0 |        0.0 |        0.0 |        0.0 |        0.0 |      100.0 |         99.5 |           100.0 |    0 |      60.8 |     100.0 |   1231 |   1378 |             1 |      0 | fail (filler)                          |
| gemma-4-e2b          | freeflow   | plain  |        3.5 |        0.4 |        2.0 |        0.0 |       75.0 |       79.4 |         99.5 |            87.5 |   94 |      80.7 |      90.6 |    230 |    324 |             0 |      0 | fail (filler, catch)                   |
| gemma-4-e2b          | speakoflow | json   |       16.2 |        5.7 |       10.8 |        0.0 |       85.3 |       95.6 |        100.0 |            65.8 |   86 |      79.0 |      79.2 |    406 |    630 |             0 |      0 | fail (damage, filler, catch)           |
| gemma-4-e2b          | speakoflow | plain  |       22.3 |        5.7 |       17.6 |        0.0 |       80.9 |       98.5 |         99.5 |            76.8 |   78 |      77.3 |      81.1 |    201 |    379 |             0 |      0 | fail (damage, filler, catch)           |
| gemma-4-e2b          | trimmed    | json   |        3.5 |        1.3 |        0.0 |        0.0 |       89.7 |       98.5 |        100.0 |            62.5 |   95 |      86.4 |      94.3 |    380 |    531 |             0 |      0 | fail (filler, catch)                   |
| gemma-4-e2b          | trimmed    | plain  |        2.6 |        1.3 |        0.0 |        0.0 |       86.8 |       95.6 |        100.0 |            50.0 |   95 |      85.2 |      90.6 |    245 |    404 |             0 |      0 | fail (filler, catch)                   |
| speakoflow-mini-0.8b | openwhispr | json   |        4.4 |        1.7 |        2.9 |        0.0 |       52.9 |       54.4 |         98.9 |            66.7 |   88 |      76.1 |     100.0 |    174 |    431 |             0 |      0 | fail (filler, language, catch)         |
| speakoflow-mini-0.8b | openwhispr | plain  |        3.5 |        1.3 |        8.8 |        0.0 |       23.5 |       41.2 |         99.5 |            78.6 |   64 |      68.8 |     100.0 |    111 |    188 |             0 |      0 | fail (filler, catch)                   |
| speakoflow-mini-0.8b | freeflow   | json   |       14.0 |        2.2 |       11.8 |        0.0 |       14.7 |       23.5 |         98.4 |            86.1 |   79 |      65.3 |      98.1 |    209 |    536 |             0 |      0 | fail (damage, filler, language, catch) |
| speakoflow-mini-0.8b | freeflow   | plain  |        1.7 |        0.0 |      100.0 |        0.0 |        0.0 |        0.0 |         99.5 |           100.0 |    0 |      60.8 |     100.0 |    436 |    524 |             0 |      0 | fail (filler)                          |
| speakoflow-mini-0.8b | speakoflow | json   |        7.4 |        4.4 |        0.0 |        0.0 |       55.9 |       57.4 |         98.9 |            41.2 |   94 |      74.4 |      96.2 |    209 |    505 |             0 |      0 | fail (damage, filler, language, catch) |
| speakoflow-mini-0.8b | speakoflow | plain  |        5.2 |        3.5 |        1.0 |        0.0 |       41.2 |       44.1 |         98.4 |            33.3 |   95 |      71.6 |      98.1 |    103 |    162 |             0 |      0 | fail (damage, filler, language, catch) |
| speakoflow-mini-0.8b | trimmed    | json   |        2.6 |        0.9 |        3.9 |        0.0 |       41.2 |       45.6 |         98.9 |            77.8 |   94 |      73.3 |     100.0 |    180 |    264 |             0 |      0 | fail (filler, language, catch)         |
| speakoflow-mini-0.8b | trimmed    | plain  |        3.9 |        0.0 |      100.0 |        0.0 |        0.0 |       20.6 |         99.5 |           100.0 |    0 |      60.8 |     100.0 |    231 |    299 |             0 |      0 | fail (filler)                          |
| speakoflow-mini-0.8b | native     | json   |       14.0 |        4.4 |        0.0 |        0.0 |       50.0 |       55.9 |         98.4 |            68.8 |   87 |      70.5 |      96.2 |    226 |    758 |             0 |      0 | fail (damage, filler, language, catch) |
| speakoflow-mini-0.8b | native     | plain  |        3.1 |        2.6 |        1.0 |        0.0 |       42.6 |       44.1 |         99.5 |            14.3 |   97 |      73.3 |      96.2 |     93 |    142 |             0 |      0 | fail (damage, filler, catch)           |

### Reading the table

- **Guard catch** is the share of damaging pre-guard outputs the stub
  rejected. **Guard rejections** (the complement of `acc.`) include false
  rejections, which cost quality.
- **Rows that pass by doing nothing are not wins.**
  - Gemma-4-E2B + FreeFlow + JSON: 0 % accepted (failure 1 below).
  - SpeakoFlow Mini + FreeFlow plain and + trimmed plain: 0 % accepted. The
    fine-tune "cleans" the whole user turn, including the
    `RAW_TRANSCRIPTION` / `BEGIN TRANSCRIPT` framing, so every reply is
    rejected and fails open to the raw input.
- **Post-guard filler removal is capped by false rejections.** Every fail-open
  reinserts the input, fillers included.
  - Gemma + trimmed + JSON removes 98.5 % pre-guard but 89.7 % post-guard.
  - The rejections that cost it fillers were correct catches: lines
    collapsed (`newline-003`, `-005`, `-008`, `mixed-006`), and "4" spelled
    as "Four" (`number-009`).
  - A deterministic filler pass ahead of the LLM (R5 §4.10 stage 7) would
    keep those fillers out even on fail-open. This eval feeds the LLM raw
    fillers, so the stage-7 benefit is not measured.

### Medium

| Model                | Format | Damage pre | **Damage post** | Answer post | Filler post | Guard catch | p95 ms | RS-07 Medium (damage ≤ 4 %, catch ≥ 90 %) |
| -------------------- | ------ | ---------: | --------------: | ----------: | ----------: | ----------: | -----: | ----------------------------------------- |
| qwen3.5-0.8b         | json   |        1.3 |             0.4 |         0.0 |        36.8 |        66.7 |    217 | fail (guard catch rate)                   |
| qwen3.5-0.8b         | plain  |        3.5 |             1.7 |         0.0 |        39.7 |        50.0 |    160 | fail (guard catch rate)                   |
| qwen3.5-2b           | json   |        0.4 |             0.4 |         0.0 |        67.6 |         0.0 |    349 | fail (guard catch rate)                   |
| qwen3.5-2b           | plain  |        2.2 |             1.3 |         0.0 |        61.8 |        40.0 |    283 | fail (guard catch rate)                   |
| qwen3-1.7b           | json   |        5.7 |             2.6 |         0.0 |        72.1 |        53.8 |    286 | fail (guard catch rate)                   |
| qwen3-1.7b           | plain  |       28.4 |             2.2 |         0.0 |        23.5 |        93.0 |    271 | **pass**                                  |
| qwen3-4b-2507        | json   |        6.1 |             4.8 |         0.0 |        94.1 |        21.4 |    589 | fail (content damage, guard catch rate)   |
| qwen3-4b-2507        | plain  |        7.0 |             4.4 |         0.0 |        86.8 |        37.5 |    545 | fail (content damage, guard catch rate)   |
| muesli-2b-v12        | json   |        4.8 |             3.5 |         0.0 |        89.7 |        27.3 |    373 | fail (guard catch rate)                   |
| muesli-2b-v12        | plain  |        7.0 |             4.8 |         0.0 |        89.7 |        35.3 |    323 | fail (content damage, guard catch rate)   |
| gemma-4-e2b          | json   |        5.2 |             2.6 |         0.0 |        89.7 |        50.0 |    565 | fail (guard catch rate)                   |
| gemma-4-e2b          | plain  |        2.6 |             1.3 |         0.0 |        88.2 |        50.0 |    406 | fail (guard catch rate)                   |
| speakoflow-mini-0.8b | json   |        1.7 |             0.4 |         0.0 |        38.2 |        93.3 |    296 | **pass**                                  |
| speakoflow-mini-0.8b | plain  |        3.9 |             0.0 |         0.0 |         0.0 |       100.0 |    299 | **pass**                                  |

All three Medium "passes" are artefacts:

- **SpeakoFlow Mini plain:** every reply rejected (see above).
- **SpeakoFlow Mini JSON and Qwen3-1.7B plain:** filler removal 38 % and
  24 %, so the output is mostly the input.

No model that actually edits passes Medium. Only 3 items have their own
Medium label (F-10 gap), so Medium damage is measured against Light labels
and is overstated.

## RS-07 pass/fail

| Gate (Light, post-guard) | Threshold | Best value seen (variant)                                                                                                               | Any variant passing all gates? |
| ------------------------ | --------- | --------------------------------------------------------------------------------------------------------------------------------------- | ------------------------------ |
| Content damage           | ≤ 2 %     | 0.4 % (Qwen3.5-2B trimmed json; Gemma FreeFlow plain), 1.3 % (Gemma trimmed json and plain)                                             | **No**                         |
| Answer rate              | ≤ 0.5 %   | 0.0 % post-guard in all 60 rows. Pre-guard: 0-36 %, worst with the SpeakoFlow prompt                                                    | –                              |
| Filler removal           | ≥ 90 %    | 92.6 % (Qwen3-4B trimmed json; Muesli OpenWhispr and SpeakoFlow prompts, json)                                                          | –                              |
| Language kept            | ≥ 99 %    | 100 % (several)                                                                                                                         | –                              |
| Guard catch rate         | ≥ 90 %    | 91 % (Qwen3-1.7B trimmed plain, 58 % accepted); 100 % on 2-3 damaging outputs (Qwen3.5-0.8B OpenWhispr/FreeFlow json). Otherwise ≤ 89 % | –                              |
| Medium content damage    | ≤ 4 %     | 0.4-2.6 % for Qwen3.5 / Gemma; 3.5 % Muesli json; 4.4-4.8 % Qwen3-4B                                                                    | see Medium caveat              |

No variant passes all Light gates, so per R5 RS-07 **"No model passes
Light"** applies, provisionally.

## Recommendation for DEC-03 tiers (provisional)

1. **Do not change the DEC-03 tier models on this run.** The decisive
   failures (filler 89.7 vs 90 %, damage 3.5-3.9 vs 2 %, guard catch) are
   within the noise of single-labelled data. The guards being measured are
   a stub.
2. **≥ 16 GB:**
   - Keep **Qwen3-4B-Instruct-2507**, with the **trimmed prompt + JSON
     schema**. It is the best general model on filler (92.6 %) with
     moderate damage (3.9 %), and it never answered post-guard.
   - Watch its latency: with the long OpenWhispr and SpeakoFlow prompts it
     missed the R5 deadline 13-23 times (p95 about 1.8 s), even on an M5
     Max.
   - **Gemma-4-E2B** (trimmed + JSON) had the lowest damage among models
     that actually edit (1.3 %) and is the strongest challenger. It is the
     same RAM class as the 4B model (3.1 GB file, 3.6 GB RSS), so it is not
     an 8 GB candidate.
   - Before Gemma could be promoted: run RS-06 on it, verify its licence
     (not verified in this run), and fix its reasoning leak (failure 1).
3. **≤ 8 GB:**
   - **Qwen3.5-2B** (the DEC-03 pick) is safe but lazy: damage 0.4 % and
     restraint 100 % with trimmed + JSON, but only 60-66 % of fillers
     removed.
   - **Muesli 2B v12** has the same base and size (2.0 GB RSS) and removes
     88-93 % of fillers, but damages 3-5 %. R4 also bars it from default
     until its training data is documented.
   - Recommendation:
     - Keep Qwen3.5-2B as the 8 GB model.
     - Ship Light on 8 GB per the RS-07 fallback: a deterministic filler
       and repair pass, with the LLM limited to punctuation and casing under
       the stricter guard.
     - Re-test Muesli after asking its authors for provenance (R4 ship
       checklist item 6).
   - **Drop Qwen3.5-0.8B as an 8 GB fallback.** It removes 6-35 % of
     fillers. **SpeakoFlow Mini** is English-only, removes about 42 % of
     fillers with its own template, and breaks with any framing.
4. **Prompt for S-12a:**
   - Start from the **trimmed** prompt with **JSON schema on**.
   - Drop the SpeakoFlow general prompt. Its "always rebuild" rules cause
     both damage and answering on small models.
   - Remove or neutralize the in-prompt examples. Qwen3-1.7B plain copied
     the first example's output ("Can you send me the report by Friday?")
     verbatim for most inputs (42.8 % pre-guard damage). Smaller models do
     this when the transcript is short.
5. **Guards for S-12a:**
   - The stub catches at most 89 % of damage in any row with a real number
     of damaging outputs (91 % only with 58 % of replies accepted), mostly
     because dropped words are invisible to its checks.
   - S-12a should add a **content-word recall check against the input**
     (allowing the labelled filler list and repair markers). That is the
     check that would catch dropped discourse words and wrong-side repairs.
   - Then re-measure the catch-rate gate. Until the second labelling pass
     settles whether "hmm", "yeah", "so", "okay" and "basically" must be
     kept, part of this gate is a labelling question.

## Failures and things not verified

1. **Gemma-4-E2B ignores `--reasoning off` under FreeFlow + JSON schema.**
   - Reproduced by hand: `content` is empty and `reasoning_content` holds a
     "Thinking Process: 1. Analyze the Request…" that runs until
     `max_tokens` (`finish_reason: length`). This happens with or without
     `chat_template_kwargs.enable_thinking=false`.
   - It occurs in 229/229 FreeFlow-JSON requests. Elsewhere it was rare:
     empty content in 1-3 of 229 requests in Gemma OpenWhispr and SpeakoFlow
     runs, and once each for Qwen3-4B and Qwen3-1.7B.
   - The guard handled it correctly (JSON parse fails → fail open).
   - For RS-06: `--reasoning off` is accepted by b11624 but is **not**
     sufficient for Gemma 4 with every prompt. A reasoning budget of 0
     (`--reasoning-budget 0`) or an output check is needed.
2. **SpeakoFlow Mini needs its exact template.** Any wrapper (FreeFlow,
   trimmed) makes it return the wrapper. That is expected from the card, but
   it means the R5 §4.5 boundary framing cannot be used with it.
3. **The Muesli card template is not better** than general prompts on this
   set: 14.8 % pre-guard damage plain, 7.9 % JSON, against 6.1 % with the
   OpenWhispr prompt + JSON.
4. **Not covered:**
   - 8 GB M1 (RS-06 budgets).
   - ASR loaded at the same time.
   - The static, signed CI build.
   - Prefix reuse with `-cms`.
   - Abort.
   - Intel.
   - Apple Foundation Models (RS-08).
   - Disfl-QA / AMI samples (not in F-10 yet).
   - Shipped Rust guards and Handy `lang_id` (the eval uses the F-10 stand-ins).
5. **Labels are single-labelled** (see the top of this report). Many "damage"
   hits are on discourse markers a second labeller may treat as fillers:
   `disfluent-002/-004/-008/-009/-012/-013`.
6. **The Gemma-4-E2B licence and the GGUF's HF revision were not checked**
   in this run. All seven GGUFs were downloaded beforehand and are
   identified here by sha256 only.

## RS-06 answers from this run (b11624)

| Question                                           | Answer                                                                                                                                                                                                                                                                                                                                           | Evidence                                          |
| -------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ | ------------------------------------------------- |
| `--offline` accepted?                              | **Yes.** The server starts and serves with `--offline` alongside `-m` (help: "Offline mode: forces use of cache, prevents network access").                                                                                                                                                                                                      | `--probe`, `probe.json`; all 7 model runs used it |
| `/health` on a Unix socket (`--host <path>.sock`)? | **Yes.** Help: "IP addresses to listen on, comma-separated, or UNIX socket paths ending in .sock". `/health` → `{"status":"ok"}` about 1.0 s after spawn (Qwen3.5-2B). `lsof -i` shows **no inet sockets** while on the socket. Chat completions also work over the socket (tested with curl). The socket file mode is `755` (owner-write only). | `--probe`                                         |
| Reasoning flag                                     | **`--reasoning off`** exists (`-rea, --reasoning [on\|off\|auto]`, default `auto`), plus `--reasoning-budget N` (0 = end at once), `--reasoning-format`, `--reasoning-effort`. Accepted by every model. It does **not** suppress Gemma-4-E2B thinking in one prompt/format (failure 1).                                                          | `--help`, failure 1                               |
| Other flags                                        | `--no-webui` is accepted but deprecated in favour of `--no-ui`. `--cache-ram`, `-cms/--checkpoint-min-step`, `--sleep-idle-seconds`, `--api-key` (env `LLAMA_API_KEY`) and `--jinja` (default on) exist. Default port is 9931, matching R4.                                                                                                      | `--help`                                          |
| Cold start to `/health` (M5 Max, warm disk cache)  | 0.6-0.7 s for the Qwen models and the fine-tunes; 1.3 s for Gemma-4-E2B                                                                                                                                                                                                                                                                          | Models table                                      |

## Reproduce

```bash
tar xzf llama-b11624-bin-macos-arm64.tar.gz -C <empty dir>
bun scripts/wb-bench-cleanup.ts --server <dir>/llama-b11624/llama-server --model-dir ~/wb-bench/llm --date 2026-10-29
bun scripts/wb-bench-cleanup.ts --server <dir>/llama-b11624/llama-server --probe --models qwen3.5-2b
```

`--per-category N` gives a stratified cut (N evenly spaced items per
category) if a full run is too slow on a smaller Mac.
