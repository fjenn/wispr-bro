# RS-08: Apple Foundation Models (provisional, one Mac)

**Outcome: Apple's model is a fallback only.** It misses the RS-08 pass
criteria against Muesli 2B v12 (the RS-07/S-12a winner) on latency and on
two quality metrics, so S-12b ships it with `AppleRole::Fallback`: it cleans
a take only when the local model cannot (still downloading, download
failed, or the memory rule says no). Refusals are not a problem (0 in 229
requests with either guardrail setting).

| Field     | Value                                                                                                                                                         |
| --------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Date      | 2026-10-31                                                                                                                                                    |
| Machine   | Mac17,6, M5 Max, 128 GB, shared with other agents (load average 2-58 during the runs)                                                                       |
| macOS     | 26.6.2 (25G83); Xcode with the macOS 27.0 SDK, Swift 6.4                                                                                                     |
| Apple AI  | **On** (`SystemLanguageModel.default.availability == .available`; read only, the setting was not touched). Context size **4096 tokens**.                     |
| Languages | `supportedLanguages`: da, de, en, es, fr, it, ja, ko, nb, nl, pt, sv, tr, vi, zh (Polish is not supported: the one `unsupportedLanguageOrLocale` in the eval) |
| Code      | the shipped path: `fork::llm::post_process::clean` with `fork::llm::apple::AppleEngine` over `crates/wb-apple-fm` (S-12b)                                     |
| Items     | `eval/cleanup/items.jsonl`, 271 items, sha256 `006789471e93…` (the S-12a gate set), language mode `owner`                                                     |
| Procedure | `scripts/wb-s12b/rs08.sh` (MAC-TEST-SCRIPT S-12b.1); reports in `eval/cleanup/reports/rs08/` (local)                                                          |

## Pass criteria (R5 §7 RS-08) and result

| Criterion                                                         | Result                                                                                                                                                         | Pass   |
| ----------------------------------------------------------------- | -------------------------------------------------------------------------------------------------------------------------------------------------------------- | ------ |
| p95 ≤ 1.2 s prewarmed, Light on 50 words                          | plain reply (shipped): p50 1113 / **p95 1317 ms** (16 takes); guided: p50 1326 / p95 1372 ms                                                                   | **no** |
| refusals ≤ 1 %                                                    | **0 %** (0/229 requests) with the default guardrails and with `permissiveContentTransformations`; 0 in a 60-request burst                                   | yes    |
| every RS-07 Light metric within 1 point of the winner (Muesli)    | best variant (plain reply): damage 0.7 % (Muesli 0.0), answer 0.0 % (0.0), **filler 91.7 % (95.0)**, language 100 % (100), **guard catch 92.0 % (100)**       | **no** |
| `rateLimited` as a background app                                 | 0 in the CLI benches (2 × 83 requests); the app itself in the background: see the S-12b test run                                                            | see run |
| real context size                                                  | 4096 tokens; the Light prompt (~700 tokens) plus a 120-word block fits with room                                                                             | yes    |

Fallback named in R5 ("Use Apple FM only while the GGUF downloads") is
applied, widened to the memory rule (S-12b "As built").

## Quality (Light, post-guard, no latency budget)

`WB_EVAL_NO_DEADLINE=1` and a 20 s request deadline: what the model writes,
before the R5 deadlines decide whether it arrives in time.

| Variant                                           | Damage        | Answer        | Filler            | Language kept | Guard catch     | RS-07 gates                     |
| ------------------------------------------------- | ------------- | ------------- | ----------------- | ------------- | --------------- | ------------------------------- |
| Muesli 2B v12 (S-12a, reference)                  | 0.0 %         | 0.0 %         | 95.0 % (114/120)  | 100 %         | 100 % (7/7)     | pass                            |
| Apple, guided `{text}`, permissive guardrails     | 1.8 % (5/271) | 0.0 % (0/117) | 93.3 % (112/120)  | 98.3 %        | 86.8 % (33/38)  | fail: language kept, guard catch |
| Apple, guided `{text}`, default guardrails        | identical outputs for all 271 items                                                                                                                        |
| **Apple, plain reply, permissive (shipped)**      | 0.7 % (2/271) | 0.0 % (0/117) | 91.7 % (110/120)  | 100 % (59/59) | 92.0 % (23/25)  | pass                            |

- **Pre-guard** Apple's model answers or follows dictated instructions far
  more often than Muesli: answer rate 7.7 % guided / 5.1 % plain, damage
  14.0 % / 8.9 %; injections 60 % damaged pre-guard (guided). The guard
  chain catches nearly all of it; what gets through is mostly non-English:
  Spanish and French repairs and rewrites (es 2/7, fr 3/26 damaged, guided).
- **Guided generation is worse than a plain reply** on every metric and
  slower (short items back to back p50 1109 vs 834 ms; 50 words prewarmed
  p95 1372 vs 1317 ms): the model, held to the `text` field, rewrites more. The prompt already asks for `{"text": …}` JSON,
  which the plain reply follows; S-12b asks for a plain reply.
- **Guardrails:** default and permissive gave byte-identical outputs and no
  refusal on this set (which has injections, commands and questions but no
  sensitive content). S-12b uses permissive content transformations
  (TypeWhisper, hex), the setting meant for rewriting user text.
- Medium: plain reply damage 1.8 % (≤ 4 % passes), guard catch 87.5 %
  (35/40); guided damage 3.0 %, catch 84.0 %. Muesli: 0.0 %, 100 %.
- Default guardrails with the plain reply: again byte-identical to
  permissive, 0 refusals.

## Latency

`fork::llm::rs08` (ignored test), our own ten 50-word dictations
(`fork::llm::apple::SAMPLES_50`), Light, English, the shipped `clean()`.
Cold = 25 s without a request first (the system unloads the model); prewarmed
= the same gap, then `prewarm` with the take's instructions and 1.5 s of
"speech"; warm = back to back.

| Reply                | n  | p50     | p95     | max     | Outcome                                    |
| -------------------- | -- | ------- | ------- | ------- | ------------------------------------------ |
| plain, cold          | 8  | 1908 ms | 2805 ms | 2805 ms | 1 of 8 over the R5 deadline (fail-open)    |
| **plain, prewarmed** | 16 | 1113 ms | 1317 ms | 1323 ms | 15 applied, 1 guard                        |
| plain, warm (burst)  | 59 | 1355 ms | 1517 ms | 1519 ms | no errors, no `rateLimited` in 60 requests |
| guided, cold         | 8  | 2203 ms | 2812 ms | 2812 ms | 2 of 8 over the R5 deadline                |
| guided, prewarmed    | 16 | 1326 ms | 1372 ms | 1409 ms | 15 applied, 1 guard                        |
| guided, warm (burst) | 59 | 1576 ms | 1656 ms | 1701 ms | no errors, no `rateLimited` in 60 requests |

For comparison, Muesli on the sidecar: LLM time per eval item p50 115 ms,
p95 183 ms (S-12a); key-up → text p50 230 ms in the real app.

**With the app's deadlines on the eval set** (R5's request deadline, the
prewarm 1 s before each item; items of 1-15 words): plain reply p50 379 ms,
p95 707 ms, **0 deadline misses**, the same post-guard numbers as without a
deadline. Guided: 63 of the 229 items sent (28 %) missed the deadline and
went in as spoken.

**Prewarm** helps (cold 1.9 s → 1.1 s on 50 words), but a 50-word Light
cleanup still takes 1.0-1.3 s prewarmed, just above the 1.2 s p95 bar and
5-10 times Muesli. Back-to-back requests without a prewarm are slower than
prewarmed ones (each request is a new session that prefills the ~700-token
instructions again).

## Decision

- `fork::llm::apple::ROLE = Fallback`, `GUARDRAILS =
  permissiveContentTransformations`, `GUIDED = false`.
- Apple's model cleans a take only when the sidecar cannot: model not ready
  (downloading, failed, missing runtime) or the R5 memory rule says no
  (Apple's model runs in a system process outside our memory budget). A
  language it does not support (`supportedLanguages`) is not sent; an
  unknown language is, and an `unsupportedLanguageOrLocale` reply fails
  open.
- Same guards, deadlines and fail-open as the sidecar; key-down prewarm.

## Not measured / open

- Other Macs: M1 8 GB (where the memory rule often says no and Apple's model
  would matter most), macOS 26.0-26.3, macOS 27.
- Apple Intelligence off or not eligible on a real Mac (only the mapping is
  unit-tested; the setting was not changed here).
- A person's own text with sensitive content (refusal rate on such text).
- The machine was shared with other agents; latencies are pessimistic, but
  Muesli was measured under similar load.
