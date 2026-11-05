# RS-13: ITN need (spoken numbers to digits)

**Status:** done 2026-10-30, unattended. **Result: the criterion fails for
every default model, so D-14 closes as not needed** (R5 §7 RS-13 fallback;
S-33 not built). The one model that writes numbers as words is the English
starter, which runs only until the English default has downloaded.

**Machine:** MacBook Pro Mac17,6 (M5 Max), macOS 26.6.2 (25G83). Engine and
harness as RS-09: `transcribe-cpp` 0.4.0 with Metal through the RS-09 bench
binary (`src-tauri/crates/wb-bench-asr`, unchanged), driven by
`scripts/wb-bench-itn.ts` (tests in `scripts/wb-bench-itn.test.ts`). Corpus:
`eval/itn/corpus.json` (synthetic, written for this project). Raw results
stay on the Mac (never committed): `~/wb-bench/results/rs13/20261030-175606/`.

## Short version

- **Default models write digits.** Over 56 number expressions per language
  (23 dictations × 2 synthetic voices), the spoken-form rate is **0 % for
  parakeet-unified-en (one pass), whisper-large-v3-turbo (en, fr, nl) and
  Parakeet TDT 0.6B v3 (fr), 3.6 % for parakeet-unified-en streamed (the way
  the app runs it) and 3.6 % for Parakeet v3 in Dutch.** The bar is 20 %.
  They write `$250`, `6:15 p.m.`, `July 4, 1992`, `18h15`, `4 juillet 1992`,
  `18.15 uur`, `3,5 kg`, `23e`, `555-213-4479` without any post-processing.
- **The English starter, parakeet-tdt_ctc-110m, writes words: 92.9 %**
  (52 of 56; the other 4 were misrecognised). Its output is lowercased
  normalised text ("two hundred and fifty dollars", "july fourth, nineteen
  ninety two"). Per DEC-52 it is used only for the first takes, until
  parakeet-unified-en is downloaded (minutes on a normal connection).
- **whisper-base (French stopgap starter) writes digits whenever it hears
  the number** (0 % spoken; 20 of 56 expressions were misrecognised, 17 of
  them with the second voice).
- **The owner's own speech agrees** (the RS-09 sentence takes, 7 number
  expressions per language, counts only): turbo 0/7 spoken in en and fr;
  parakeet-unified-en 2/7, turbo nl 2/7 and Parakeet v3 nl 3/7, but every
  one of those is a small count in running text ("twelve", "ten",
  "douze", "tien") or a Dutch clock idiom ("half vier", "kwart voor acht")
  that Dutch and English style guides write in words anyway. Only one
  expression in 21 default-model results is a real miss (Parakeet v3 nl
  "achttienhonderd"). The starter again wrote all 7 in words.
- **Shipping ITN would add risk for no gain on the defaults:** a NeMo-style
  cardinal tagger (text-processing-rs) also turns "twelve boxes" and "ten
  minutes" into digits, which the default models (correctly) leave as words.

## Results against the RS-13 criterion

Criterion (R5 §7): "If ≥ 20 % of number expressions come out in spoken form,
ITN is worth shipping."

Synthetic corpus, 23 dictations per language, each spoken by two `say`
voices (en: Samantha, Eddy; fr: Thomas, Eddy; nl: Xander (nl_NL), Ellen
(nl_BE)), 28 expressions per voice. "Written": the expression's digits
appear in the transcript (separators ignored). "Spoken": its number words
appear. "Other": neither (misrecognised).

| Language | Model                    | Role (DEC-52)      | Expressions | Written | Spoken | Other | Spoken rate | ≥ 20 %? |
| -------- | ------------------------ | ------------------ | ----------: | ------: | -----: | ----: | ----------: | ------- |
| en       | parakeet-tdt_ctc-110m    | starter            |          56 |       0 |     52 |     4 |  **92.9 %** | yes     |
| en       | parakeet-unified-en-0.6b | default (one pass) |          56 |      56 |      0 |     0 |       0.0 % | no      |
| en       | parakeet-unified-en-0.6b | default (streamed) |          56 |      54 |      2 |     0 |       3.6 % | no      |
| en       | whisper-large-v3-turbo   | reference          |          56 |      56 |      0 |     0 |       0.0 % | no      |
| fr       | whisper-base             | stopgap starter    |          56 |      36 |      0 |    20 |       0.0 % | no      |
| fr       | whisper-large-v3-turbo   | default            |          56 |      54 |      0 |     2 |       0.0 % | no      |
| fr       | parakeet-tdt-0.6b-v3     | reference          |          56 |      54 |      0 |     2 |       0.0 % | no      |
| nl       | parakeet-tdt-0.6b-v3     | first download     |          56 |      54 |      2 |     0 |       3.6 % | no      |
| nl       | whisper-large-v3-turbo   | default (upgrade)  |          56 |      56 |      0 |     0 |       0.0 % | no      |

Spoken-form expressions per category (spoken / total):

| Category | en 110m | en unified | en unified (stream) | en turbo | fr base | fr turbo | fr v3 | nl v3 | nl turbo |
| -------- | ------: | ---------: | ------------------: | -------: | ------: | -------: | ----: | ----: | -------: |
| cardinal |     8/8 |        0/8 |                 0/8 |      0/8 |     0/8 |      0/8 |   0/8 |   0/8 |      0/8 |
| money    |     8/8 |        0/8 |                 0/8 |      0/8 |     0/8 |      0/8 |   0/8 |   0/8 |      0/8 |
| time     |     7/8 |        0/8 |                 0/8 |      0/8 |     0/8 |      0/8 |   0/8 |   1/8 |      0/8 |
| date     |     6/6 |        0/6 |                 0/6 |      0/6 |     0/6 |      0/6 |   0/6 |   0/6 |      0/6 |
| measure  |     8/8 |        0/8 |                 1/8 |      0/8 |     0/8 |      0/8 |   0/8 |   1/8 |      0/8 |
| percent  |     3/4 |        0/4 |                 0/4 |      0/4 |     0/4 |      0/4 |   0/4 |   0/4 |      0/4 |
| ordinal  |     4/4 |        0/4 |                 1/4 |      0/4 |     0/4 |      0/4 |   0/4 |   0/4 |      0/4 |
| digits   |     2/4 |        0/4 |                 0/4 |      0/4 |     0/4 |      0/4 |   0/4 |   0/4 |      0/4 |
| year     |     6/6 |        0/6 |                 0/6 |      0/6 |     0/6 |      0/6 |   0/6 |   0/6 |      0/6 |

The four spoken-form results from default models: streamed unified-en
"twenty-third floor" and "nine hundred square feet" (the one-pass run of the
same model wrote `23rd` and `900`); Parakeet v3 nl "negen uur dertig" and
"twaalf kilometer".

Owner's RS-09 takes (real speech, one take per sentence, counts only; the
transcripts are personal data and stay on the Mac):

| Language | Model                    | Expressions | Written | Spoken | Other | Spoken rate |   Of which ITN should change |
| -------- | ------------------------ | ----------: | ------: | -----: | ----: | ----------: | ---------------------------: |
| en       | parakeet-tdt_ctc-110m    |           7 |       0 |      7 |     0 |     100.0 % |                            5 |
| en       | parakeet-unified-en-0.6b |           7 |       5 |      2 |     0 |      28.6 % |             0 (small counts) |
| en       | whisper-large-v3-turbo   |           7 |       7 |      0 |     0 |       0.0 % |                            0 |
| fr       | whisper-base             |           7 |       5 |      1 |     1 |      14.3 % |              0 (small count) |
| fr       | whisper-large-v3-turbo   |           7 |       7 |      0 |     0 |       0.0 % |                            0 |
| nl       | parakeet-tdt-0.6b-v3     |           7 |       4 |      3 |     0 |      42.9 % |        1 ("achttienhonderd") |
| nl       | whisper-large-v3-turbo   |           7 |       4 |      2 |     1 |      28.6 % | 0 (small count, clock idiom) |

The reading script has only seven number expressions per language, and three
of them are small counts or Dutch clock idioms where words are the expected
written form, so these rows are a sanity check, not a measurement. They show
the same pattern: the defaults write digits where a writer would.

## Verdict

- **D-14 closes as not needed (RS-13 fallback).** No default model reaches
  the 20 % bar (0-3.6 %); S-33 is not built and text-processing-rs is not
  adopted. Pipeline stage 6 (R5 §4.10) stays empty.
- **The English starter is the exception, and it is accepted.** For the
  minutes until parakeet-unified-en is ready, numbers arrive as words. This
  is not worth an ITN stage: the window is short, ITN would need its own
  English-only path for one transient model (its other output, such as
  lowercased months, shows it was trained on normalised text). Whether Light
  cleanup turns the starter's number words into digits is not measured; if
  S-12a wants it, a prompt rule is cheaper than an ITN stage.
- **Re-run when a default changes.** Any new default model (a new language,
  DEC-52 change, a model upgrade) re-runs `bun scripts/wb-bench-itn.ts
synth|run|report` with its language added to the corpus; if it crosses
  20 %, S-33 reopens for that language.

## Caveats

- **Synthetic speech.** `say` voices are clean, evenly paced and read the
  spoken form exactly as written. That biases _recognition_ upwards (fewer
  "other" results than real speech), but not obviously the _form_: the
  written-vs-spoken choice comes from the model's training text, and the
  owner's real takes gave the same pattern. Real speech with hesitations
  ("two hundred, uh, fifty") may come out in words more often; that is not
  measured.
- **Coverage.** One run per model (greedy decoding, deterministic), 28
  expressions per voice, 2 voices per language; en, fr and nl only (the
  languages DEC-52 names defaults for). Other languages' defaults are not
  yet chosen.
- **Scoring is lenient on format.** "Written" means the digits are present;
  `6.15pm` vs `6:15 p.m.` or `18 uur 15` vs `18.15 uur` count the same.
  Those are style questions for the cleanup prompt, not ITN.
- **No LLM cleanup in the loop.** The numbers are raw ASR output; Light
  cleanup (DEC-03) may change them either way and is measured in RS-07, not
  here.
- **Machine was shared** with another session's GUI spikes; this affects
  timing only, which RS-13 does not measure.

## How to re-run

```sh
# Bench binary: build as in RS-09 (src-tauri/crates/wb-bench-asr).
bun scripts/wb-bench-itn.ts synth  --out ~/wb-bench/results/rs13/<ts>
bun scripts/wb-bench-itn.ts run    --out ~/wb-bench/results/rs13/<ts> \
  --bin src-tauri/target/release/wb-bench-asr
bun scripts/wb-bench-itn.ts report --out ~/wb-bench/results/rs13/<ts>
bun scripts/wb-bench-itn.ts owner  # counts on the owner's RS-09 takes
```

`synth` speaks every corpus sentence with both voices of its language
(`say -o`, then `afconvert` to 16 kHz mono 16-bit WAV) and joins them into
one WAV per language with 0.5 s of silence around each take, plus a jobs
file of sample ranges. `run` passes each language's WAV through its models
with the RS-09 bench (`run` subcommand, one take per job, language hint
set), and streams each take separately for parakeet-unified-en. `report`
writes `detail.json` (every transcript and class) next to the results.
