# RS-09: speech-model bench (provisional run, one speaker, one Mac)

> **Provisional.** One speaker (the owner), one take of the reading script
> (`eval/asr/READING-SCRIPT.md`), read aloud rather than spoken freely, on
> **one Mac: an M5 Max with 128 GB**, which is far faster than the M1 8 GB
> reference machine of R5 §5. Every latency here is a lower bound for the
> slower machine classes. The network and first-run part of RS-09 (Network
> Link Conditioner at 10 and 50 Mbit/s, Welcome → starter ready → first
> take, §5.3) is **not covered**, and neither are the M 16 GB and Intel
> machines (DEC-29) or the 50-take, five-language set R5 asks for (this run
> has en, nl and fr only).

## Verdict

- **Every candidate meets the §5.1 short-take budget on this Mac**, on Metal
  and on the CPU: the slowest is whisper-large-v3-turbo at p95 426 ms for
  sentences of 4-12 s (budget p95 ≤ 1,000 ms). Turbo's cost is a fixed
  ~300 ms per take (the 30 s Whisper window), so it will be the first to
  miss on an M1. The Parakeet models take 20-60 ms.
- **The starter criterion passes in English.** whisper-base: 12.3 %
  normalised WER on the 12 English sentences (7.1 % on the 7 sentences read
  without a stumble), under the 15 % gate. parakeet-tdt_ctc-110m (135 MB,
  CC-BY) is better: 6.5 % / 3.6 %. Moonshine base is worse (17.4 % /
  10.7 %) and fails outright on any take it gets in one run longer than
  about a minute (see below).
- **whisper-base is not a usable starter in Dutch (63.6 % WER) and weak in
  French (28.3 %).** No model ≤ 150 MB in this set covers Dutch acceptably.
- **whisper-small (270 MB, Apache-2.0; follow-up run, same day) halves
  whisper-base's error in nl and fr**: short takes nl 29.5 % (fluent
  27.8 %), fr 15.2 % (13.8 %); long takes nl 6.9 %, fr 8.0 % (whisper-base
  25.9 % and 27.4 %), at 63-70 ms p50 and 670 MB peak footprint. It is a
  usable French starter and the mid option for slower Macs in nl and fr, but
  short Dutch dictations are still poor (about three words in ten wrong). In English it is no better than whisper-base on fluent
  sentences (6/84 errors each) and better on long takes (7.7 % vs 11.2 %).
  At 270 MB it cannot meet the §5.3 starter budget (see the policy).
- **whisper-large-v3-turbo is the most accurate model in all three
  languages** (en 9.4 %, nl 12.1 %, fr 4.8 % normalised, all sentences;
  2.0-4.4 % on the long takes). On the fluent English sentences the best
  four models are within one error of each other (2-3 errors in 84 words, two
  of which are the reader's own change of sentence 6), so in English the
  differences between turbo, parakeet-unified-en and parakeet-110m are within
  the noise of one speaker.
- **Parakeet TDT 0.6B v3 is second in nl and fr** (16.7 % and 6.2 %) at a
  fifth of turbo's latency, but in one French chunk it dropped a sentence and
  switched to English although it was told `fr` (31 % WER on that take with
  Handy's chunker).
- **DEC-51: chunking while the user speaks works.** On the synthetic 5-minute
  English take, key-up waits 357 ms with turbo (4,758 ms without chunking),
  47 ms with parakeet-unified-en streaming, 47 ms with Parakeet v3 eager
  chunks. Chunking costs the Whisper models about one point of WER in en and
  nl (French turbo lost 4-5 points at one seam, see below) and _improves_
  WER for the Parakeet models on takes over a minute
  (Parakeet v3 en-long: 11.8 % whole vs 5.5 % chunked). Few words are lost
  or doubled at seams (0-3 per take, most of them ordinary errors or the
  reader's own deviations that happen to sit next to a seam); the clearest
  seam effect is Whisper inventing "thank you" / "okay" in the silence at
  the end of a chunk.
- **Handy already does most of DEC-51.** For models without long-form
  support (Parakeet, Moonshine) Handy's live path cuts the recording at
  pauses into ~30 s chunks with its `Chunker` and decodes them while the user
  speaks (`LiveDecode::Chunked`, `long_form_chunking` on by default), and
  parakeet-unified-en streams (`LiveDecode::Stream`). Whisper models are
  decoded in one run after key-up (`supports_long_form`), which is where
  DEC-51 adds something: turbo needs chunking to meet the DEC-51 target on
  long takes.

## Setup

| Item         | Value                                                                                                                                                                                                                                                                                                                                                                                                                                                                                      |
| ------------ | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ |
| Machine      | Apple M5 Max (`Mac17,6`), 18 CPU cores (6 super + 12 performance), 40-core GPU (Metal 4), 128 GB (137,438,953,472 B). `sysctl machdep.cpu.brand_string hw.model hw.memsize`                                                                                                                                                                                                                                                                                                                |
| macOS        | 26.6.2 (25G83), `sw_vers`                                                                                                                                                                                                                                                                                                                                                                                                                                                                  |
| Load         | Not idle: load average about 5 from other sessions during the runs. Timings are noisy at the 10-20 % level; quality numbers are deterministic (greedy decoding).                                                                                                                                                                                                                                                                                                                           |
| Engine       | `transcribe-cpp` 0.4.0 / `transcribe-cpp-sys` 0.4.0 (as in `src-tauri/Cargo.lock`), features `metal` + `serde` (the app's Apple Silicon features), release profile identical to the app's (`lto`, `codegen-units = 1`)                                                                                                                                                                                                                                                                     |
| Harness      | `src-tauri/crates/wb-bench-asr` (Rust, its own workspace root; never part of the app build) and `scripts/wb-bench-asr.ts` + `scripts/wb-bench-asr-text.ts` (driver, splitter, scorer; tests in `scripts/wb-bench-asr.test.ts`)                                                                                                                                                                                                                                                             |
| How it calls | As the app's engine worker (`engine_supervisor/worker.rs`): `init_backends_default()` (CPU path: `init_backends_with(None, BackendMask::CPU)`), `Model::load_with(path, { backend: Auto \| Cpu, device: None })`, one `Session`, `session.run(pcm, RunOptions { language, ..Default::default() })` with the language passed only when the model advertises it (Handy's `transcribe_cpp_run_plan`); `TimestampKind::Auto` as in the app. No vocabulary, no translation, no post-processing. |
| Languages    | Each take is run with its true language as the hint (what the app does once its language detector or the user's languages pick it). Language detection itself is not measured.                                                                                                                                                                                                                                                                                                             |
| Chunking     | Handy's own `audio_toolkit::chunking::Chunker` and `join_transcripts`, fed Silero VAD v6.2 scores from Handy's `SileroVad` (the model Handy ships, `resources/models/silero-vad-v6.2-F32.gguf`, sha256 `0d871a80…88e6`, matches the catalog). Scores are computed in a separate CPU-only process and replayed, because a process can register only one backend mask (the app also scores VAD outside the GPU worker).                                                                      |
| Streaming    | `session.stream(run, StreamOptions::default())`, 480-sample feeds, `finalize()`, text = `text().full` (Handy's `live.rs`)                                                                                                                                                                                                                                                                                                                                                                  |
| Memory       | Per model one process; peak RSS from `getrusage`, peak physical footprint from `proc_pid_rusage(RUSAGE_INFO_V4)` (includes Metal buffers in unified memory). Peaks include a one-run decode of the 203 s English take.                                                                                                                                                                                                                                                                     |
| Raw results  | `~/wb-bench/results/rs09/` (outside the repo: transcripts of the owner's voice). Segments in `~/wb-bench/audio/segments/`.                                                                                                                                                                                                                                                                                                                                                                 |

### Models

All Q8_0 from Handy's catalog (`src-tauri/src/catalog/catalog.json`); every
file's sha256 matches the catalog. whisper-small was added in a follow-up the
same day: downloaded from the catalog's pinned repository and revision
(`handy-computer/whisper-small-gguf` @ `c0214bd3…`, Hugging Face only) and
run through the same bench (Metal only).

| Model                    | File MB | Licence    | CC-BY | Languages (catalog)          | Streaming (catalog) | sha256                                                             |
| ------------------------ | ------: | ---------- | ----- | ---------------------------- | ------------------- | ------------------------------------------------------------------ |
| whisper-base             |      85 | Apache-2.0 | no    | multilingual                 | no                  | `81c069428bc8a24551a8169cf31cf09bcfd9d4cf50389ae281323c9aa9648c81` |
| whisper-small            |     270 | Apache-2.0 | no    | multilingual                 | no                  | `9b9c8811bbcc82a7766f0fb0925614bdacb0923b2cc630daeac17108b655b860` |
| whisper-large-v3-turbo   |     886 | Apache-2.0 | no    | multilingual                 | no                  | `b2e30cc286bc9f3aba4db9099fc7403543497c05ce7100d0d83091ddfd25a183` |
| moonshine-base           |      77 | MIT        | no    | en                           | no                  | `7f0027dfd857d310b63a85ef57cadf183da712cc374f85a648f8bc18aaa2efc8` |
| parakeet-tdt-0.6b-v3     |     740 | CC-BY-4.0  | yes   | 25 European incl. en, nl, fr | no                  | `5859f77944efcd8eafa23a6350731960b2b55b2203df51f319665c807d802cc7` |
| parakeet-unified-en-0.6b |     731 | CC-BY-4.0  | yes   | en                           | **yes**             | `4b50b6dd862bf6e346929aaf4f5eaacec003bfa3f56462d6c874b41ef2f38795` |
| parakeet-tdt_ctc-110m    |     135 | CC-BY-4.0  | yes   | en                           | no                  | `7dd44c74a331d788a4e5f8b16913b3feb29ced22cf5613aad0e0f6cd30516296` |

What the engine reports: every model loaded on `MTL0` (Metal). Only
parakeet-unified-en reports `supports_streaming`; transcribe-cpp's
`Session::stream` / `Stream::feed` / `finalize` API is the one Handy already
uses and is worth using for that model (see DEC-51). The Whisper models
report long-form support; their finest timestamps are segments (word
timestamps are refused), the Parakeet models give word and token times.

## The recording and how it was split

One take, 651.0 s, 16 kHz mono (`reading-script-audio-16k.wav`). The owner
read all six sections in script order: en-sentences, nl-sentences,
fr-sentences, en-long, nl-long, fr-long. Nothing was skipped; every one of
the 36 sentences was found.

**Method** (`bun scripts/wb-bench-asr.ts split`):

1. Silero VAD over the whole file (Handy's `SileroVad`, Silero's
   `get_speech_timestamps` rules as in Handy's `chunking.rs`); speech segments
   closer than 300 ms joined into 119 utterances.
2. whisper-large-v3-turbo (the strongest multilingual model) transcribed each
   utterance with language detection on.
3. Each utterance was scored against every section's vocabulary; utterances
   matching no section (a cough or noise turned into words) were set aside;
   a Viterbi pass with a switch cost made each section one contiguous run.
   Sections were cut in the longest pause between two runs.
4. Whisper in transcribe.cpp reports segment times only, so for the sentence
   cuts Parakeet TDT 0.6B v3 (multilingual, word times) transcribed the
   utterances of the sentence sections again with each section's language.
   One monotonic alignment of its words to the section text placed each word
   on a sentence; sentences were cut in the longest silence between the last
   word of one sentence and the first of the next, with at most 0.5 s of
   silence kept on either side.
5. Two prefixes of en-long were cut at the sentence end nearest 1 and 3
   minutes; a synthetic 5-minute English take (291.5 s) was made by joining
   the en-sentences and en-long segments.

**Section cuts** (seconds in the take):

| Section                                   | Start s |  End s | Length s |
| ----------------------------------------- | ------: | -----: | -------: |
| en-sentences                              |    0.00 |  88.90 |     88.9 |
| nl-sentences                              |   88.90 | 181.68 |     92.8 |
| fr-sentences                              |  181.68 | 301.71 |    120.0 |
| en-long                                   |  301.71 | 504.32 |    202.6 |
| nl-long                                   |  504.32 | 564.72 |     60.4 |
| fr-long                                   |  564.72 | 651.01 |     86.3 |
| en-long-1m (first 10 sentences)           |  301.71 | 366.29 |     64.6 |
| en-long-3m (first 29 sentences)           |  301.71 | 481.03 |    179.3 |
| en-5m (synthetic: en-sentences + en-long) |       – |      – |    291.5 |

**Sentence cuts** (start-end in seconds; `*` = the split model heard words
that are not in the script inside the sentence):

| Section      | Cuts                                                                                                                                                                                                                            |
| ------------ | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| en-sentences | 1: 1.81-7.97\*, 2: 7.97-15.99\*, 3: 16.62-21.91, 4: 24.65-31.09, 5: 31.09-40.88\*, 6: 41.17-46.80, 7: 47.05-53.20, 8: 53.74-59.44, 9: 59.57-67.03, 10: 67.41-73.07, 11: 73.39-79.76\*, 12: 79.76-85.59\*                        |
| nl-sentences | 1: 92.21-98.74, 2: 99.60-108.94\*, 3: 108.94-114.16, 4: 115.21-119.86, 5: 120.43-124.59, 6: 124.81-128.67, 7: 128.67-135.12, 8: 135.50-140.91, 9: 143.53-147.15, 10: 147.15-151.73, 11: 153.49-163.25\*, 12: 168.75-174.64      |
| fr-sentences | 1: 188.72-197.78, 2: 198.03-210.77\*, 3: 211.63-220.66, 4: 222.00-232.27, 5: 233.77-239.47, 6: 240.72-247.15, 7: 248.24-255.35, 8: 257.61-266.03, 9: 267.02-274.48, 10: 275.47-286.55\*, 11: 287.09-291.70\*, 12: 291.98-298.35 |

**What the reader did differently from the script** (heard the same way by
every strong model, so it is the reader, not the models):

- Fillers and stumbles: en 1 ("um"), en 2 (a repeated "to" and "uh"), en 11
  ("uh"), en 12 (a "like"), fr 2 (a stumble over "comptabilité", with a pause
  in the middle), fr 10 (restarted "sept pour cent").
- An aside: en 5, after "devries" the surname was spelled out letter by
  letter.
- A restart with a paraphrase: nl 11 ("het weer is vandaag eindelijk lekker"
  then again in other words, then "zullen we buiten lunchen").
- Changed words without a stumble, not flagged by the split: en 6 ("no,
  actually Wednesday" for "no wait, on Wednesday"), nl 12 ("de hulp", "dat"
  for "je hulp", "het"), nl 1 probably "hey" for "hoi", nl 3 an added
  "alsjeblieft". Two sentences ran into the next without a pause (nl 6 → 7,
  nl 9 → 10); the word timings split them.
- The long sections have a few similar deviations (fr-long: "tout le monde
  est … était d'accord"). They count as errors for every model alike.

Because of this, the tables give WER over **all 12 sentences** and over the
**fluent** ones only (no `*`: 7 en, 10 nl, 9 fr). The fluent column is the
better measure of the model; the all-sentences column is closer to what a
real dictation contains. Even the fluent sets keep the unflagged deviations
above, so no model reaches 0 %.

**Scoring.** WER and CER against the script text (not against what the
reader said). **raw** = lowercase and punctuation stripped, nothing else;
**norm** = raw plus numbers, times, ordinals, currency, percent and e-mail
symbols written out in words in the take's language ("3:30" → "three thirty",
"€1,800" → "eighteen hundred euros", "7 %" → "zeven procent", "15h30" →
"quinze heures trente", "anna.devries@example.com" → "anna dot devries at
example dot com"). The script itself is all words, so norm only changes the
models' output. The gap between raw and norm is the model's number
formatting, which matters for dictation: Whisper and Parakeet v3/unified
write digits, parakeet-110m writes words. CER is on the norm text without
spaces.

## Results on Metal

### Short takes (dictation-sized), per language

12 sentences per language, 4-12 s each; each sentence run 3 times with the
model warm. Latency = from the end of the audio (the whole clip handed to
`run`) to the text, no cleanup; p50/p95 over the 36 runs. RTF = compute time
/ audio time.

**English**

| Model                    | WER norm | WER raw | CER | WER norm, fluent | p50 ms | p95 ms |   RTF |
| ------------------------ | -------: | ------: | --: | ---------------: | -----: | -----: | ----: |
| whisper-base             |     12.3 |    20.3 | 6.0 |              7.1 |     48 |     66 | 0.007 |
| whisper-small            |     14.5 |    23.9 | 5.5 |              7.1 |     63 |     89 | 0.010 |
| whisper-large-v3-turbo   |      9.4 |    18.8 | 3.6 |              2.4 |    318 |    426 | 0.051 |
| moonshine-base           |     17.4 |    26.8 | 7.8 |             10.7 |     62 |    115 | 0.010 |
| parakeet-tdt-0.6b-v3     |     13.8 |    15.9 | 5.1 |              4.8 |     56 |    101 | 0.009 |
| parakeet-unified-en-0.6b |     10.1 |    18.1 | 3.6 |              3.6 |     48 |     66 | 0.008 |
| parakeet-tdt_ctc-110m    |      6.5 |     6.5 | 3.8 |              3.6 |     21 |     27 | 0.004 |

Errors / reference words (norm): whisper-base 17/138 (fluent 6/84),
whisper-small 20/138 (6/84), turbo
13/138 (2/84), Moonshine 24/138 (9/84), Parakeet v3 19/138 (4/84), unified
14/138 (3/84), 110m 9/138 (3/84). Most of the spread in the all-sentences
column is en 5 (the spelled-out surname), where the models differ in how
they write the letters.

**Dutch**

| Model                  | WER norm | WER raw |  CER | WER norm, fluent | p50 ms | p95 ms |   RTF |
| ---------------------- | -------: | ------: | ---: | ---------------: | -----: | -----: | ----: |
| whisper-base           |     63.6 |    65.9 | 34.4 |             61.1 |     52 |    299 | 0.017 |
| whisper-small          |     29.5 |    32.6 | 17.8 |             27.8 |     70 |     86 | 0.012 |
| whisper-large-v3-turbo |     12.1 |    17.4 |  8.4 |             10.2 |    335 |    386 | 0.058 |
| parakeet-tdt-0.6b-v3   |     16.7 |    21.2 | 10.1 |             14.8 |     57 |     70 | 0.010 |

**French**

| Model                  | WER norm | WER raw |  CER | WER norm, fluent | p50 ms | p95 ms |   RTF |
| ---------------------- | -------: | ------: | ---: | ---------------: | -----: | -----: | ----: |
| whisper-base           |     28.3 |    38.6 | 12.9 |             30.3 |     49 |     63 | 0.006 |
| whisper-small          |     15.2 |    26.9 |  7.5 |             13.8 |     68 |     90 | 0.009 |
| whisper-large-v3-turbo |      4.8 |    16.6 |  3.0 |              1.8 |    325 |    402 | 0.041 |
| parakeet-tdt-0.6b-v3   |      6.2 |    14.5 |  4.4 |              3.7 |     64 |     86 | 0.008 |

### Long takes in one run

One `session.run` over the whole take, after key-up. This is the raw engine:
the app itself only does this for Whisper; for the other models Handy's batch
path cuts audio over 40 s into ~30 s chunks (see DEC-51 below for those
numbers).

| Model                    | en-long 203 s WER norm / raw |    ms | nl-long 60 s |    ms | fr-long 86 s |    ms | en-5m 292 s WER |     ms |
| ------------------------ | ---------------------------- | ----: | ------------ | ----: | ------------ | ----: | --------------- | -----: |
| whisper-base             | 11.2 / 11.4                  |   905 | 25.9 / 27.6  |   301 | 27.4 / 28.3  |   348 | 10.9            |  1,180 |
| whisper-small            | 7.7 / 7.9                    | 1,225 | 6.9 / 6.9    |   424 | 8.0 / 9.7    |   450 | 7.4             |  1,700 |
| whisper-large-v3-turbo   | 2.0 / 2.2                    | 3,467 | 2.6 / 2.6    | 1,191 | 4.4 / 6.2    | 1,857 | 4.0             |  4,758 |
| moonshine-base           | fails (repetition stop)      | 5,200 | –            |     – | –            |     – | fails           | 11,465 |
| parakeet-tdt-0.6b-v3     | 11.8 / 12.1                  | 1,913 | 6.0 / 7.8    |   327 | 12.4 / 15.0  |   619 | 11.8            |  3,335 |
| parakeet-unified-en-0.6b | 5.3 / 5.5                    | 1,807 | –            |     – | –            |     – | 6.6             |  3,177 |
| parakeet-tdt_ctc-110m    | 6.1 / 6.1                    |   878 | –            |     – | –            |     – | 7.4             |  1,787 |

Moonshine stops with "output repetition" (status 19) or "output truncated"
(status 18) on every take of 65 s or more; it is only usable chunked, as
Handy runs it. Parakeet v3 is markedly worse in one 3-minute run than in
30 s chunks (11.8 % vs 5.5 % on en-long).

### Load, unload/reload and memory

"Load" is the first `Model::load_with` + `session()` in the process; the
files were already in the page cache (downloaded the same day; a true cold
read from disk was not measured, no `purge` without sudo). "First run" is the
first `run` after a load (Metal pipeline and buffer set-up) on en sentence 1
(6.2 s); "reload" is a load after dropping the session and model, followed by
the "10 s take" (en sentence 5, 9.8 s) cold and then warm.

| Model                    | Load ms | First run ms | Same clip warm ms | Reload ms | Run after reload ms (cold → warm) | Peak RSS MB | Peak footprint MB | Footprint after unload MB |
| ------------------------ | ------: | -----------: | ----------------: | --------: | --------------------------------- | ----------: | ----------------: | ------------------------: |
| whisper-base             |      84 |          286 |                38 |        48 | 56 → 49                           |         339 |               428 |                       318 |
| whisper-small            |      99 |          237 |                64 |        72 | 102 → 90                          |         580 |               670 |                       326 |
| whisper-large-v3-turbo   |     316 |          199 |               385 |       283 | 252 → 431                         |       1,229 |             1,319 |                       389 |
| moonshine-base           |      78 |          263 |                52 |        45 | 117 → 101                         |         655 |               745 |                       276 |
| parakeet-tdt-0.6b-v3     |     264 |           54 |                45 |       274 | 65 → 57                           |       1,215 |             1,305 |                       435 |
| parakeet-unified-en-0.6b |     253 |           55 |                41 |       244 | 66 → 55                           |       1,161 |             1,250 |                       382 |
| parakeet-tdt_ctc-110m    |      80 |          132 |                23 |        64 | 53 → 34                           |         455 |               545 |                       343 |

The footprint after unload stays at 280-440 MB because the bench process
keeps the backend registry, the audio and the results; in the app the worker
process exits on unload, so its memory goes back to zero. Turbo's warm runs
vary between 200 and 430 ms (load on the machine); the extra cost of the
first take after an unload is reload + first run − warm run, at most about
300 ms here.

## Results on the CPU (whisper-base and Parakeet v3)

`Backend::Cpu` with the CPU-only backend mask, as the app's CPU fallback
worker. Sentences once each.

| Model                | Language | WER norm (fluent) | p50 ms | p95 ms |   RTF | en-long 203 s ms | en-5m 292 s ms | Peak footprint MB |
| -------------------- | -------- | ----------------- | -----: | -----: | ----: | ---------------: | -------------: | ----------------: |
| whisper-base         | en       | 13.0 (7.1)        |    252 |    261 | 0.038 |            2,671 |          3,730 |               297 |
| whisper-base         | nl       | 62.9 (61.1)       |    255 |    550 | 0.053 |                  |                |                   |
| whisper-base         | fr       | 28.3 (30.3)       |    254 |    287 | 0.031 |                  |                |                   |
| parakeet-tdt-0.6b-v3 | en       | 13.8 (4.8)        |    213 |    356 | 0.036 |           14,157 |         25,949 |             2,202 |
| parakeet-tdt-0.6b-v3 | nl       | 16.7 (14.8)       |    193 |    357 | 0.037 |                  |                |                   |
| parakeet-tdt-0.6b-v3 | fr       | 6.2 (3.7)         |    264 |    458 | 0.036 |                  |                |                   |

Accuracy on the CPU matches Metal (within one word). On this CPU both stay
inside the short-take budget; Parakeet v3 on a whole 3-5 minute take is
14-26 s and needs chunking. Load 47 ms (whisper-base) and 263 ms (Parakeet
v3).

## RS-09 criteria

| Criterion (R5 §7 RS-09, §5.1)                                                 | Result on this Mac                                                                                                                                                                                   |
| ----------------------------------------------------------------------------- | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Starter WER on clean English ≤ 15 %                                           | **Pass**: whisper-base 12.3 % (fluent 7.1 %); parakeet-110m 6.5 % (3.6 %). Moonshine base 17.4 % fails on all sentences, passes on fluent (10.7 %). (whisper-small 14.5 % / 7.1 %, but over 150 MB.) |
| ≤ 20 s take, cleanup none: p50 ≤ 500 ms, p95 ≤ 1,000 ms                       | **Pass for every model** on Metal (worst: turbo 318 / 426 ms) and for both CPU runs (worst: Parakeet v3 fr 264 / 458 ms; whisper-base nl p95 550 ms)                                                 |
| First take after ASR unload ≤ +800 ms p50, ≤ +1,500 ms p95                    | **Pass** (single measurement per model): reload + first-run penalty ≤ about 300 ms for every model; page cache warm                                                                                  |
| Starter ready within §5.3 (10 and 50 Mbit/s, clean Mac)                       | **Not measured** (no network part in this run)                                                                                                                                                       |
| Policy table meets §5.1 on each machine class, twice (with and without CC-BY) | **Only one machine class measured** (an M5 Max is faster than any class in R5); tables below are provisional                                                                                         |
| DEC-51: a 5-minute take inserts within the Light budget (≤ 50 words) + 1 s    | **ASR part passes with chunking**: key-up waits 32-357 ms on en-5m with chunks or streaming (turbo whole-take: 4,758 ms, fails). Cleanup paragraph by paragraph not measured.                        |
| §5.2: 8 GB Mac, ASR + LLM ≤ 3.2 GB                                            | **At risk with turbo**: 1.32 GB peak footprint here + Muesli 2B (2.0 GB in RS-07) ≈ 3.3 GB. Parakeet 0.6B: 1.25-1.31 GB; whisper-base 0.43 GB.                                                       |

## DEC-51: transcribing long takes while the user speaks

Three ways, all simulated on the recorded audio fed at the end of each step
as if live (the bench feeds faster than real time, then reconstructs the
timeline: a chunk can start decoding when it is ready and the engine is
free; key-up latency = when the last text is ready minus the end of the
audio):

- **Whole**: one run after key-up (what the app does for Whisper today).
- **A, Handy's live `Chunker`**: cuts at pauses near 30 s (5-40 s), a chunk
  final once 5 s past its window is scored; whatever is left at key-up is cut
  and decoded then. VAD = Silero v6.2 as Handy ships it. This is what the app
  does today for Parakeet and Moonshine. For the Whisper models it is
  applied here although the app does not.
- **B, eager pause chunks** (a DEC-51 variant written for this spike): cut in
  the middle of every pause ≥ 0.5 s once the chunk is ≥ 4 s (forced at the
  longest pause after 30 s); a chunk is ready once its pause has lasted
  0.5 s. Smaller last chunk, more seams.
- **Streaming**: parakeet-unified-en through `Session::stream`, the app's
  `LiveDecode::Stream`.

Lost / doubled = reference words deleted / words repeated within two words
of a seam. These are upper bounds: they also count ordinary errors and the
reader's own deviations that happen to fall next to a seam.

| Model                    | Take       | Audio s | Whole WER | Whole key-up ms | A chunks | A WER | A key-up ms | A lost / doubled | B chunks | B WER | B key-up ms | B lost / doubled |
| ------------------------ | ---------- | ------: | --------: | --------------: | -------: | ----: | ----------: | ---------------- | -------: | ----: | ----------: | ---------------- |
| whisper-base             | en-long-1m |      65 |       7.3 |             292 |        3 |  10.2 |         172 | 0 / 0            |        5 |   9.5 |          43 | 1 / 0            |
| whisper-base             | en-long-3m |     179 |      10.9 |             767 |        7 |   8.4 |         158 | 0 / 0            |       13 |   8.4 |          80 | 2 / 0            |
| whisper-base             | en-5m      |     292 |      10.9 |           1,180 |       11 |   9.4 |         170 | 0 / 0            |       27 |  10.8 |          32 | 3 / 0            |
| whisper-base             | nl-long    |      60 |      25.9 |             301 |        2 |  27.6 |         146 | 0 / 0            |        4 |  25.9 |          94 | 1 / 0            |
| whisper-base             | fr-long    |      86 |      27.4 |             348 |        3 |  27.4 |          99 | 0 / 1            |        7 |  31.9 |          84 | 0 / 0            |
| whisper-small            | en-long-1m |      65 |       7.3 |             396 |        3 |   8.0 |         325 | 0 / 0            |        5 |   7.3 |          58 | 1 / 0            |
| whisper-small            | en-long-3m |     179 |       8.4 |           1,080 |        7 |   6.7 |         216 | 0 / 0            |       13 |   6.9 |          99 | 2 / 0            |
| whisper-small            | en-5m      |     292 |       7.4 |           1,700 |       11 |   6.2 |         231 | 0 / 0            |       27 |   7.6 |          51 | 3 / 0            |
| whisper-small            | nl-long    |      60 |       6.9 |             424 |        2 |   7.8 |         246 | 0 / 0            |        4 |  11.2 |         107 | 0 / 0            |
| whisper-small            | fr-long    |      86 |       8.0 |             450 |        3 |  16.8 |         111 | 3 / 0            |        7 |  13.3 |          85 | 0 / 0            |
| whisper-large-v3-turbo   | en-long-1m |      65 |       2.9 |           1,174 |        3 |   2.9 |         795 | 0 / 0            |        5 |   3.6 |         310 | 1 / 0            |
| whisper-large-v3-turbo   | en-long-3m |     179 |       2.2 |           3,020 |        7 |   3.2 |         768 | 0 / 0            |       13 |   3.2 |         371 | 1 / 0            |
| whisper-large-v3-turbo   | en-5m      |     292 |       4.0 |           4,758 |       11 |   4.5 |         780 | 0 / 0            |       27 |   4.9 |         357 | 2 / 1            |
| whisper-large-v3-turbo   | nl-long    |      60 |       2.6 |           1,191 |        2 |   2.6 |         773 | 0 / 0            |        4 |   2.6 |         362 | 1 / 0            |
| whisper-large-v3-turbo   | fr-long    |      86 |       4.4 |           1,857 |        3 |   8.8 |         425 | 0 / 0            |        7 |   9.7 |         494 | 0 / 0            |
| moonshine-base           | en-long-1m |      65 |      fail |           1,974 |        3 |   7.3 |         671 | 0 / 0            |        5 |   6.6 |          89 | 1 / 0            |
| moonshine-base           | en-long-3m |     179 |      fail |           5,231 |        7 |   8.2 |         663 | 0 / 0            |       13 |   8.4 |         172 | 2 / 0            |
| moonshine-base           | en-5m      |     292 |      fail |          11,465 |       11 |   7.9 |         819 | 0 / 0            |       27 |  12.1 |          66 | 7 / 0            |
| parakeet-tdt-0.6b-v3     | en-long-1m |      65 |       5.1 |             359 |        3 |   3.6 |         204 | 0 / 1            |        5 |   4.4 |          64 | 1 / 1            |
| parakeet-tdt-0.6b-v3     | en-long-3m |     179 |      13.4 |           1,609 |        7 |   6.7 |         210 | 0 / 1            |       13 |   6.9 |          96 | 1 / 1            |
| parakeet-tdt-0.6b-v3     | en-5m      |     292 |      11.8 |           3,335 |       11 |   7.4 |         295 | 0 / 1            |       27 |   7.9 |          47 | 2 / 0            |
| parakeet-tdt-0.6b-v3     | nl-long    |      60 |       6.0 |             327 |        2 |   2.6 |         123 | 0 / 0            |        4 |   3.4 |         124 | 0 / 0            |
| parakeet-tdt-0.6b-v3     | fr-long    |      86 |      12.4 |             619 |        3 |  31.0 |         112 | 0 / 0            |        7 |   4.4 |          95 | 1 / 0            |
| parakeet-unified-en-0.6b | en-long-1m |      65 |       3.6 |             414 |        3 |   3.6 |         168 | 0 / 0            |        5 |   4.4 |          59 | 1 / 0            |
| parakeet-unified-en-0.6b | en-long-3m |     179 |       5.7 |           1,533 |        7 |   4.0 |         167 | 0 / 0            |       13 |   4.7 |          73 | 1 / 0            |
| parakeet-unified-en-0.6b | en-5m      |     292 |       6.6 |           3,177 |       11 |   5.1 |         190 | 0 / 0            |       27 |   5.7 |          73 | 2 / 0            |
| parakeet-tdt_ctc-110m    | en-long-1m |      65 |       5.8 |             178 |        3 |   3.6 |          89 | 0 / 0            |        5 |   5.1 |          20 | 1 / 0            |
| parakeet-tdt_ctc-110m    | en-long-3m |     179 |       6.7 |             721 |        7 |   5.9 |          72 | 0 / 0            |       13 |   6.4 |          35 | 1 / 0            |
| parakeet-tdt_ctc-110m    | en-5m      |     292 |       7.4 |           1,787 |       11 |   7.1 |          85 | 0 / 0            |       27 |   7.9 |          25 | 3 / 0            |

(The full en-long take, 203 s, behaves like en-long-3m; it is in the raw
results.)

**Streaming, parakeet-unified-en-0.6b** (480-sample feeds; compute per feed
stayed far below the audio clock, so there was never a backlog at key-up):

| Take            | Audio s | WER norm | WER raw | Feed compute / audio | Max feed ms | Finalize ms (= key-up) |
| --------------- | ------: | -------: | ------: | -------------------: | ----------: | ---------------------: |
| en-long-1m      |      65 |      3.6 |     3.6 |                0.037 |          70 |                     44 |
| en-long-3m      |     179 |      4.7 |     4.7 |                0.039 |          54 |                     45 |
| en-long         |     203 |      4.2 |     4.4 |                0.039 |         192 |                     43 |
| en-5m           |     292 |      6.7 |     9.1 |                0.039 |          53 |                     47 |
| 12 en sentences |  4-12 s |     10.1 |    19.6 |                    – |           – |         p50 32, p95 38 |

**Findings**

- **Latency.** Without chunking, key-up grows with the take (turbo 1.2 s at
  1 minute, 4.8 s at 5 minutes on this Mac; several times that on an M1).
  With chunking it stays flat: Handy's chunker leaves up to ~40 s for after
  key-up (turbo ~780 ms, Parakeet 170-300 ms here); eager chunks leave one
  sentence (turbo 310-494 ms, Parakeet 20-125 ms); streaming leaves only the
  finalize (43-47 ms). The DEC-51 target is met on this Mac by all three
  for the ASR part.
- **Accuracy.** Chunking costs the Whisper models 0-1 WER points in English
  and Dutch. French turbo goes from 4.4 % to 8.8-9.7 %: one chunk boundary
  falls where the reader corrected a word ("tout le monde est … était"), and
  turbo writes a different, worse version of that phrase when it sees only
  one side. For the Parakeet models chunking _helps_ on anything over a
  minute (Parakeet v3 en-5m 11.8 → 7.4 %, unified 6.6 → 5.1 %); Parakeet
  degrades in one long run. Streaming unified matches its chunked accuracy
  (4.2-6.7 %).
- **Seams.** Handy's ~30 s chunks show no lost words and at most one doubled
  word per take; eager chunks lose 1-3 words on long takes (7 for Moonshine
  on en-5m, which hallucinates at short chunk ends). The clearest seam
  artefact is Whisper's: turbo inserted "okay" and "thank you" in the silence
  at the end of two chunks of en-5m. A seam-repair pass should drop a
  chunk-final "thank you" / "okay" that has no audio behind it.
- **Parakeet v3 in French.** In Handy's first 28 s chunk of fr-long it left
  out a full sentence and switched to English mid-sentence ("Julien will
  raccourci … of the application"), although the run had `language = fr`.
  Eager chunks of the same audio scored 4.4 %. One occurrence, but it is the
  failure a user notices; it argues for turbo in French where it fits the
  budget.
- **whisper-small prefers long context.** Its Dutch WER goes from 6.9 %
  (whole 60 s take) to 7.8 % with Handy's ~30 s chunks and 11.2 % with
  sentence-sized chunks, the same weakness that gives it 29.5 % on short
  Dutch dictations. In French it hits the same self-correction seam as
  turbo (8.0 → 16.8 % with Handy's chunks).
- **Whisper chunking only adds a little over Handy.** DEC-51 needs the
  chunked live path for Whisper models (today `LiveDecode::None` because they
  report long form) and a smaller last chunk than Handy's 30-40 s if key-up
  on an M1 must stay inside +1 s: eager chunks keep turbo's key-up at one
  sentence's decode.

## Recommended per-language policy (DEC-02), provisional

Measured on one machine class only: an M5 Max ("fast Apple Silicon" below).
For slower Macs the right-hand column is what this run suggests, to be
confirmed by the RS-09 M1 8 GB and M 16 GB runs, which decide whether turbo
meets §5.1 there.

**With CC-BY models (DEC-26 option 1, the current default)**

| Language | Starter (≤ 150 MB)                                                                                                       | Main pick, fast Apple Silicon (measured)                                                                                | Main pick where turbo misses §5.1 (to confirm)                                                               |
| -------- | ------------------------------------------------------------------------------------------------------------------------ | ----------------------------------------------------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------ |
| en       | parakeet-tdt_ctc-110m (6.5 % / fluent 3.6 %, 21 ms)                                                                      | parakeet-unified-en-0.6b (10.1 % / 3.6 %, 48 ms, streams); turbo is the alternative with the best long-take WER (2.0 %) | parakeet-unified-en-0.6b                                                                                     |
| nl       | none ≤ 150 MB (whisper-base 63.6 %); whisper-small (270 MB, 29.5 % / long 6.9 %) if the cap is relaxed for nl, see below | whisper-large-v3-turbo (12.1 % / 10.2 %, long 2.6 %)                                                                    | parakeet-tdt-0.6b-v3 (16.7 % / 14.8 %, long 6.0 %, 57 ms); whisper-small (29.5 %, long 6.9 %) on the slowest |
| fr       | whisper-base as a stopgap (28.3 %); better whisper-small (270 MB, 15.2 % / long 8.0 %) if the cap is relaxed             | whisper-large-v3-turbo (4.8 % / 1.8 %, long 4.4 %)                                                                      | parakeet-tdt-0.6b-v3 (6.2 % / 3.7 %), chunked; whisper-small (15.2 %, long 8.0 %) on the slowest             |

**Without CC-BY models**

| Language | Starter (≤ 150 MB)                                                 | Main pick, fast Apple Silicon (measured) | Main pick where turbo misses §5.1 (to confirm)                                                                   |
| -------- | ------------------------------------------------------------------ | ---------------------------------------- | ---------------------------------------------------------------------------------------------------------------- |
| en       | whisper-base (12.3 % / 7.1 %, 48 ms; better than Moonshine base)   | whisper-large-v3-turbo (9.4 % / 2.4 %)   | whisper-small (14.5 % / 7.1 %, long 7.7 %, 63 ms): no gain over whisper-base on short takes, better on long ones |
| nl       | none ≤ 150 MB; whisper-small (270 MB) if the cap is relaxed for nl | whisper-large-v3-turbo                   | whisper-small (29.5 % short, 6.9 % long): weak on short takes; whisper-medium (831 MB) untested                  |
| fr       | whisper-base as a stopgap; whisper-small if the cap is relaxed     | whisper-large-v3-turbo                   | whisper-small (15.2 % short, 8.0 % long)                                                                         |

Why English differs from the DEC-02 rule taken literally: "the most accurate
model that meets the budget" is turbo in all three languages on this Mac.
In English the gap to parakeet-unified-en is one error in 84 fluent words
and 3 points on the long take, within what one speaker can show, while
unified is 6× faster per take, streams (live text, 45 ms key-up at any
length) and is the model R4/RS-09 named for English under DEC-26. If the
owner prefers the literal rule, turbo is the English pick on fast Macs in
both variants. For nl and fr turbo leads clearly and is the pick in both
variants. So on fast Macs CC-BY changes only the English starter and
(optionally) the English main model; it matters much more on slower Macs,
where Parakeet v3 and unified sit between whisper-base and turbo; without
CC-BY the only measured mid option is whisper-small, which is clearly worse
than Parakeet v3 on short Dutch and French takes (29.5 % vs 16.7 %, 15.2 % vs
6.2 %).

Policy notes for S-07a/S-07b:

- **The 150 MB starter cap and Dutch.** whisper-base's 64 % WER makes the
  RS-09 fallback ("keep whisper-base and show improving accuracy…") worse
  than waiting: most Dutch words come out wrong. whisper-small is the only
  measured model that would help, and it does not fit the cap: 270 MB is
  about 43 s at 50 Mbit/s and 3.6 min at 10 Mbit/s of transfer alone,
  against §5.3's 25 s and 90 s for "starter ready" (whisper-base: 14 s and
  68 s). The cap is really §5.3, so relaxing it means relaxing §5.3 for
  Dutch. **Recommendation:** relax it for nl only, to whisper-small, and
  accept a Dutch "starter ready" of ≈ 45 s at 50 Mbit/s / ≈ 4 min at
  10 Mbit/s (owner decision; it changes a §5.3 budget). Until it is ready,
  a Dutch take is recorded and queued (S-07a's "Setting up" card) instead of
  being transcribed by whisper-base. Expect short Dutch dictations from
  whisper-small to still need correcting (29.5 %); the main model (turbo or
  Parakeet v3) follows. For French whisper-base (28 %) is a weak but usable
  stopgap; whisper-small (15 %) is better, so the same exception could be
  applied to fr, but it is less pressing. canary-180m-flash (218 MB, CC-BY)
  does not cover Dutch and was not tested.
- Moonshine base must never get a take in one run longer than about a
  minute; Handy's chunked path already prevents that.
- Use Parakeet models only through the chunked or streaming path for takes
  over a minute (Handy's default), never one long run.
- If turbo is a main pick, DEC-51 needs the live chunked path for Whisper
  too, ideally with eager (sentence-sized) chunks, plus the seam filter for
  "thank you" / "okay".

## Error examples

Reference words (normalised) with the model's output where they differ:
`ref→out` substituted, `−ref` dropped, `+out` inserted. Snippets of our own
script, with only the error words from the model.

**whisper-large-v3-turbo**

- nl 7: **kun→nu** je de … bekijken en **−een** opmerking bij
- nl 4: kwart voor acht **−s −ochtends** (dropped "'s ochtends")
- fr 5: on publie la **mise→miss** à jour
- fr 1: la réunion **−de** demain à quinze heures trente
- nl 3: twee zwarte **inktpatronen→inkpatronen** bestellen

**parakeet-unified-en-0.6b**

- en 10: remind me to call **the→a** dentist
- en 1: hey sarah **can→can't** we move (Parakeet v3 and whisper-base heard
  the same; possibly the reader)
- en 5: anna **dot devries→d e v r i e s** (the spelled-out aside, written
  as letters)
- en 6: no **−wait on→actually** wednesday (the reader's change; every
  model)

**parakeet-tdt-0.6b-v3**

- nl 5: we zetten de update dinsdag live nee **wacht→echt** woensdag
- nl 1: kunnen we de afspraak van morgen **verzetten→overzetten naar→na**
  half vier
- fr 8: le train est **bloqué→bloc** avant bruxelles
- fr-long (chunked): "Julien va raccourcir l'introduction de l'application"
  came out with English words in it, and the sentence before it was missing
- en 3: paper and **two→true** black ink

**parakeet-tdt_ctc-110m** (English starter candidate)

- en 8: the pull **request→requests** and leave
- en 5: anna **dot devries→dvr is** at example **−dot** com
- en 6: no **−wait on→actually** wednesday (the reader)

**whisper-small** (nl/fr starter candidate)

- nl 7: **−kun −je −de −pull −request −bekijken** en **−een** opmerking bij
  (the first half of the sentence dropped)
- nl 3: wil je **−twaalf dozen→twaalfduizend** papier
- fr 1: **salut claire→s alluclère** est-ce qu'on … la **réunion→rignon**
- fr 9: le **dentiste→don de tiste** la semaine

**whisper-base** (current starter)

- en 9: sorry **the→to** train **got→god** stuck outside **utrecht→utecht**
- nl 4: de **−vlucht −vertrekt −op veertien→vluchtvertrekthopvertie** maart
- nl 5 and nl 7: most of the sentence lost, with Cyrillic (nl 5) and
  Chinese (nl 7) characters in the output
- fr 8: **désolé→oeuvres** le **train→trains est→et bloqué→blocs**

## Caveats

- **One speaker, one take, read text.** The owner read a script in one go,
  with fillers, stumbles and a few paraphrases; spontaneous speech, other
  voices, accents, noisy rooms and other microphones are not covered. With
  84-109 fluent reference words per language, one error moves WER by about
  one point: differences under 2-3 points are not meaningful.
- **One Mac, much faster than the reference.** The M5 Max (40-core GPU) is
  several times faster than an M1 (8-core GPU); the §5.1 verdicts here do
  not carry over. Turbo's fixed ~300 ms per take is the number to watch on
  the M1 run.
- **Not covered:** the first-run and network part of RS-09 (§5.3, Network
  Link Conditioner); M 16 GB and Intel; es, de, ja, zh; 50 sample takes per
  candidate; whisper-small on the CPU; language detection (each take ran
  with its true language);
  vocabulary prompting; cleanup after ASR; a true cold load from disk (files
  were in the page cache).
- **The split depends on two models.** Sections and sentences were placed
  with turbo and Parakeet v3, which are also benchmarked. Cuts are in pauses
  and include up to 0.5 s of silence, so a model never sees a cut word, but
  a mis-placed cut would hurt all models the same way. The two sentence
  pairs read without a pause (nl 6/7, nl 9/10) were cut at word timings.
- **The 5-minute take is synthetic** (en-sentences then en-long, with the
  silences of both); the longest real take is 203 s.
- **Latency is simulated** for chunking: decode times are measured, the
  live timeline is reconstructed. Real-time feeding on a loaded machine
  could add backlog on slower Macs.
- **The machine was not idle** (load average ~5); latency figures carry
  10-20 % noise.

## How to re-run

```sh
# Build the bench (its own workspace; uses a copy of the app's lockfile).
cd src-tauri/crates/wb-bench-asr && cp ../../Cargo.lock .
CMAKE_POLICY_VERSION_MINIMUM=3.5 CARGO_TARGET_DIR=../../target cargo build --release --offline
cd ../../..
# Split, bench (Metal, then CPU), chunking + streaming, report tables.
bun scripts/wb-bench-asr.ts split
bun scripts/wb-bench-asr.ts bench
bun scripts/wb-bench-asr.ts bench --cpu
bun scripts/wb-bench-asr.ts chunk
bun scripts/wb-bench-asr.ts report   # writes ~/wb-bench/results/rs09/report.md
```

Inputs: `~/wb-bench/audio/reading-script-audio-16k.wav`, models under
`~/wb-bench/asr/<id>/` (catalog Q8_0 files). Results are cached per file in
`~/wb-bench/results/rs09/`; delete a file to re-run that step. For the M1 run,
copy the models and the WAV and run the same commands.
