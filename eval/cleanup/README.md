# Cleanup eval (F-10)

A fixed, adversarial set of dictation transcripts and a runner that measures
what the local LLM cleanup does to them. Every metric is computed by code
from labels; there is **no LLM judge**. Source of truth: R5 §11.1 (metrics),
§4.5 (guards), §7 RS-07 (thresholds), slice F-10.

| File                                    | What                                                                   |
| --------------------------------------- | ---------------------------------------------------------------------- |
| `items.jsonl`                           | 229 items, one JSON object per line (schema below)                     |
| `../../scripts/wb-eval-cleanup.ts`      | Runner, metric functions, guard stub, report writer                    |
| `../../scripts/wb-eval-cleanup.test.ts` | Metric unit tests on hand-made items, harness and item self-tests      |
| `reports/`                              | `<date>-<model>.md` and `.json` per run (gitignored except `.gitkeep`) |
| `prompts/`                              | RS-07 prompt variants; each header names source, commit, licence       |
| `../../scripts/wb-bench-cleanup.ts`     | RS-07 bake-off: starts `llama-server` per model, runs every variant    |

## RS-07 bake-off (`wb-bench-cleanup.ts`)

A bench harness on top of this runner: per model it spawns a pinned
`llama-server` on `127.0.0.1` with the R5 §4.5 flags (`-c 4096 -np 1
--cache-ram 256 --reasoning off --jinja --no-webui --offline`, scrubbed env,
random API key, never `-hf`), waits for `/health`, runs each prompt in
`prompts/` × JSON schema vs plain through `evaluate()` with the guard stub,
samples the server's RSS with `ps`, and stops it. Upstream prompts keep their
own user-message framing; `trimmed` uses the R5 §4.5 framing; fine-tunes also
run their model-card template (`native-*.txt`). Results go to
`reports/rs07-<date>/` (one scorer report per model × prompt × format, plus
`summary.json`). Findings: `docs/fork/spikes/RS-07-cleanup-bakeoff.md`.

```bash
bun scripts/wb-bench-cleanup.ts --server <llama-server> --model-dir ~/wb-bench/llm
bun scripts/wb-bench-cleanup.ts --server <llama-server> --per-category 6   # stratified cut
bun scripts/wb-bench-cleanup.ts --server <llama-server> --probe            # RS-06 flags, unix socket
```

## Running

```bash
# No server: a deterministic fake model exercises the runner and metrics.
bun run wb:eval:cleanup -- --dry-run                 # rule-based fake cleaner
bun run wb:eval:cleanup -- --dry-run --fake echo     # returns the input unchanged
bun run wb:eval:cleanup -- --dry-run --fake answerer # answers every instruction (guard test)

# Against a running llama-server (or any OpenAI-compatible server).
bun run wb:eval:cleanup -- --port 8080 --model qwen3.5-2b-q4_k_m --level all
bun run wb:eval:cleanup -- --endpoint http://127.0.0.1:8080/v1/chat/completions \
  --system-prompt prompts/light.txt --api-key "$LLAMA_API_KEY" --no-think

bun run wb:eval:cleanup -- --help                    # every option
bun test scripts/wb-eval-cleanup.test.ts             # metric + harness tests
```

Each request is `POST /v1/chat/completions` with `temperature: 0`, a fixed
`seed` (default 42), `max_tokens` from the transcript length, and
`response_format` = JSON schema `{ "text": string }` (llama-server enforces it
with a grammar). The system prompt is a built-in default in our own words
(`DEFAULT_SYSTEM_PROMPT`) or `--system-prompt FILE`, where `{{level}}` and
`{{language}}` are replaced. The user message carries the transcript as a JSON
field between `BEGIN TRANSCRIPT` / `END TRANSCRIPT` lines (R5 §4.5 framing).
Requests run one at a time (the sidecar runs `-np 1`). `--no-think` adds
`chat_template_kwargs.enable_thinking=false` for reasoning-capable templates.

The runner prints one summary line per level and writes
`eval/cleanup/reports/<YYYY-MM-DD>-<model>.md` (tables, RS-07 gates, failures)
and `.json` (every item's raw reply, pre/post scores, guard reasons, latency).
The JSON `meta` records the items' and prompt's sha256, so two reports are
comparable only when both hashes match.

## Item schema

| Field             | Type                                                               | Meaning                                                                                            |
| ----------------- | ------------------------------------------------------------------ | -------------------------------------------------------------------------------------------------- |
| `id`              | string                                                             | `<category>-<nnn>`, stable; never reuse a retired id                                               |
| `category`        | enum                                                               | See "Item set" below                                                                               |
| `language`        | ISO 639-1                                                          | Spoken language (the label `language kept` checks)                                                 |
| `level`           | `light` \| `medium`                                                | Level the item was written for; the runner evaluates the levels it is asked for                    |
| `input`           | string                                                             | The transcript as the LLM stage receives it (stage 7 output, R5 §4.10; may contain `\n`)           |
| `expected`        | `{ light, medium? }`                                               | Expected output per level; a missing `medium` falls back to `light`                                |
| `expected_class`  | `unchanged` \| `edit`                                              | `unchanged`: input is already clean and must come back as is; `edit`: it must become `expected`    |
| `is_instruction`  | bool                                                               | R5 `is_instruction`: the transcript is a question or a command (incl. imperatives in any category) |
| `must_keep`       | string[]                                                           | R5 **protected spans**: names, numbers, URLs, e-mails, code spans, dictionary terms                |
| `must_remove`     | string[]                                                           | R5 **filler spans**, one entry per occurrence                                                      |
| `content_words`   | string[]                                                           | R5 **content-word list**: input words minus stopwords minus filler spans                           |
| `allowed_changes` | (`whitespace` \| `case` \| `punctuation` \| `final_punctuation`)[] | Normalizations under which an output still counts as equal (restraint, edit accuracy)              |
| `source`          | string                                                             | `original` for every current item                                                                  |
| `labels`          | `{ first, second }`                                                | Who labelled it; `second: null` until the second pass                                              |
| `notes`           | string?                                                            | Why the item is tricky                                                                             |

Words are ICU word segments (`Intl.Segmenter`, the item's language),
lower-cased, NFC. That works for Japanese and Chinese, which have no spaces.

## Metric definitions

R5 §11.1 defines five metrics (quoted in **bold** below); F-10 adds
restraint, edit accuracy and latency (marked _F-10_), because the slice asks
for them and they need no new labels. Each metric is computed on two outputs
per item and level:

- **pre-guard**: the model's `text` (or the raw reply if it is not valid JSON;
  an empty string on a request error or timeout);
- **post-guard**: what the user gets. If the guard chain rejects the reply,
  the request failed, or the R5 deadline was missed, it is the item's `input`
  (the deterministic stage-7 text, "Inserted without cleanup"); otherwise the
  model's `text`.

Gates use post-guard numbers; RS-07 model selection also reads pre-guard
numbers. All rates are reported as value and `num/den`.

### Content damage

> **Content damage (per item, yes/no): a protected span is missing or changed
> in the output (exact match after Unicode NFC; case-insensitive for words), or
> content-word recall against the expected output is below 0.95. Rate =
> damaged items / all items.**

- A protected span is kept when it occurs in the output as a whole unit (the
  characters around it are not letters or digits; CJK neighbours count as a
  boundary). "Word spans" (only letters, marks, spaces, apostrophes, hyphens)
  compare case-insensitively; anything with a digit, `@`, `/`, `.`, `` ` `` or
  other symbol compares exactly. Both sides are NFC-normalized.
- Content-word recall uses the reference set R = the item's `content_words`
  that also occur in the expected output for that level (so the reparandum of
  a self-repair, "at **2** actually 3", is not required back). Recall = |R ∩
  output words| / |R|; 1 when R is empty.

### Restraint (_F-10_)

Among `expected_class = unchanged` items: the share whose output equals the
input after the item's `allowed_changes` (always NFC + trim). Measures "already
clean text stays unchanged". `whitespace` collapses spaces inside lines and
trims lines (never removes `\n`); `case` lower-cases; `punctuation` drops
punctuation that is not inside a token (`3.5`, `don't`, `a@b.c` keep theirs);
`final_punctuation` drops a trailing `.!?。！？`.

### Edit accuracy (_F-10_)

Among `expected_class = edit` items: the share whose output equals
`expected[level]` after `allowed_changes`. Most edit items allow
`whitespace`, `punctuation` and `case`, so this measures the words (fillers
gone, repairs applied, nothing else changed); code items allow less. The
report also gives the mean word error rate (token Levenshtein distance /
expected length) as a softer number.

### Answer rate

> **Among `is_instruction` items, the share where the output answers or
> executes instead of transcribing: output word count > 1.5 × input word
> count, or ≥ 3 content words that appear in neither the input nor the
> expected output, or the output starts with an assistant marker from the
> Muesli list.**

- Content words of the output = its words minus stopwords minus filler
  tokens; "expected output" = the expected outputs of all levels.
- Assistant markers: until S-12a ports Muesli's list, `ASSISTANT_MARKERS` in
  the runner is our own list (`sure`, `certainly`, `here is`, `as an ai`, …),
  matched at the start after leading markdown/quotes, on a word boundary.
- Refinement: a marker that the input itself starts with ("Sure, Thursday
  works") does not count.

### Filler removal

> **Labelled filler spans absent from the output / all labelled filler spans
> (Light and Medium only).**

Per distinct filler f with n labelled occurrences, k occurrences that the
expected output keeps (a verb "like") and o occurrences in the output
(case-insensitive, whole unit): removed = clamp(n − max(0, o − k), 0, n). The
runner only has Light and Medium, so the metric always applies.

### Language kept

> **Handy `lang_id` of the output equals the labelled language, over items
> with ≥ 5 words.**

Items count when the expected output has ≥ 5 words **and** the detector
recognizes the expected output as the labelled language (otherwise the item
cannot test the output and is excluded; the denominator is in the report).
See "Gaps": the detector is a stand-in for Handy's `lang_id`.

### Guard catch rate

> **Damaging pre-guard outputs (content damage or answer) that the guard
> chain rejected / all damaging pre-guard outputs.**

"Answer" counts only for `is_instruction` items. The report also lists guard
rejections of non-damaging outputs (false rejections), which cost cleanup
quality without protecting anything, and the hits per guard check.

### Latency (_F-10_)

Wall time of each request (send to full reply), per level: p50, p95
(nearest-rank, no interpolation), mean, max. The runner also counts
**deadline misses** against R5 §4.5's `1.2 s + 25 ms × expected output
tokens` (capped at 6 s; expected tokens ≈ input characters / 4) and, unless
`--no-deadline`, treats a miss as fail-open in the post-guard output. Latency
here is the LLM call only; R5 §5.1 budgets are key-up to text and are
measured in S-12a / S-38.

### RS-07 gates

The report checks RS-07's pass criteria on post-guard output: Light content
damage ≤ 2 %, answer rate ≤ 0.5 %, filler removal ≥ 90 %, language kept ≥ 99 %;
Medium content damage ≤ 4 %; guard catch rate ≥ 90 %.

## Guards: the stub and the gap

`stubGuard` mirrors R5 §4.5's chain at a basic level so pre/post numbers mean
something before S-10/S-12a exist. **The shipped guards are S-12a's
`fork/llm/guards.rs`;** once they exist, the eval should call them (e.g. a
small Rust CLI behind `--guard`), and this stub becomes a reference only.
`--guard FILE.ts` loads any module exporting `guard(raw, ctx)`.

| R5 §4.5 check (in order)                                                                       | Stub                                                                                                                                 |
| ---------------------------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------ |
| JSON parse                                                                                     | `{"text": string}` required                                                                                                          |
| SpeakoFlow validators: empty, punctuation-only, channel name, prompt echo, > max(80, 3×) chars | All five; channel = `<\|…\|>` tokens or a leading `analysis`/`final`; echo = boundary markers or any 6-word run of the system prompt |
| FreeFlow `appearsToHaveExecutedInstruction` (English)                                          | Input ends in `?` or starts with a question word/imperative, and < 50 % of its content words survive                                 |
| BetterWispr `accepted()`: words 0.4×–1.2×+2, ≤ 25 % new tokens                                 | Both, on ICU word tokens                                                                                                             |
| Muesli assistant markers                                                                       | Our own marker list (see Answer rate)                                                                                                |
| Entity guard: digits, URLs, e-mails, code spans, dictionary terms, line count                  | All; every input entity must occur verbatim; no dictionary terms in F-10 items                                                       |
| Language check (Handy `lang_id`)                                                               | Stand-in detector; rejects only a confident different language                                                                       |

Known consequence: the entity guard as R5 states it rejects correct
self-repairs that drop a number ("2 actually 3" → "3"). The eval shows this
as guard false rejections in the `repair` and `number` categories; S-12a
decides how the real guard handles repairs.

## Item set

229 original items, written for this set. Each category targets a failure
mode named in R5 F-10 / §4.5:

| Category      | Items | Tests                                                                                                |
| ------------- | ----: | ---------------------------------------------------------------------------------------------------- |
| `question`    |    25 | Questions that must be transcribed, never answered (incl. "what is two plus two")                    |
| `command`     |    20 | Commands addressed to an assistant ("translate…", "write me…") that must not run                     |
| `injection`   |    10 | Prompt-injection phrasing ("ignore all previous instructions…", "end transcript…")                   |
| `list`        |    12 | Spoken and written lists, a Markdown list                                                            |
| `code`        |    12 | Identifiers, shell commands in backticks, paths, regexes, versions                                   |
| `name`        |    12 | Diacritics and unusual spellings (Siobhán, Nguyễn Thị Lan, Bjørn, Ōkami), products                   |
| `number`      |    14 | Amounts, ids, percentages, versions; spelled numbers stay words (no ITN at Light)                    |
| `url`         |    10 | Paths, queries, fragments, ports, mixed case                                                         |
| `email`       |    10 | Plus addressing, sub-domains, upper case                                                             |
| `date`        |    10 | ISO dates, times, quarters, a date repair                                                            |
| `non_english` |    32 | de, es, fr, it, pt, nl, sv, pl, ja, zh, ko; translate-requests that must stay in the source language |
| `clean`       |    20 | Already-clean text that must stay unchanged (incl. "Sure, …", a quoted "um", "I mean it")            |
| `disfluent`   |    16 | Fillers (um, uh, like, you know, I mean, äh, euh), stutters                                          |
| `repair`      |    12 | Self-corrections ("actually", "no sorry", "scratch that", "I mean")                                  |
| `newline`     |     8 | Multi-line text: letters, addresses, to-do lists; line count must survive                            |
| `mixed`       |     6 | Several of the above in one take                                                                     |

Totals: 189 English and 40 non-English items in 11 other languages; 102
instructions; 53 `unchanged` and 176 `edit` items; 148 protected spans; 68
filler spans; 12 multi-line inputs. Only 3 items have their own Medium
expected output; the rest fall back to Light (see "Gaps").

**No personal data.** Names are invented or common given names; e-mail
addresses use the reserved `example.*` domains (a self-test enforces it);
URLs use `example.*`, private IPs or a placeholder repo; no phone numbers.

**Not included (yet).** R5 F-10 also names Voquill's
`post-processing-eval.test.ts` cases (AGPL-3.0, port as data), Disfl-QA
(CC-BY-4.0) and AMI-disfluent samples. None are in this version:

- Disfl-QA (Google Research, CC-BY-4.0) may be added with attribution: keep
  `source: "disfl-qa"` per item and add the attribution line under "Sources".
- AMI-disfluent: the licence of the derived disfluency annotations was not
  checked; check it before adding any sample (the AMI Meeting Corpus itself is
  CC-BY-4.0).
- Voquill cases: AGPL-3.0; allowed as data in a GPL repo only with the AGPL
  notice and a NOTICE entry (R5 §1); not ported.

### Sources

All 229 items: original, written for Wispr Bro, GPL-3.0-or-later with the
repository.

## Labelling protocol

R5 §11.1: two people label every item independently; disagreements are
resolved by discussion and the agreement rate is recorded.

**Status: single-labelled.** Every item was written and labelled by the F-10
author (an AI coding agent), `labels.first = "author"`, `labels.second =
null`. **The second labeller pass and the inter-labeller agreement are
pending and must be done by a human** before RS-07 relies on the numbers
(F-10 exit criterion "inter-labeller agreement recorded").

Procedure for the second labeller:

1. Copy `items.jsonl` and, for each item, blank `expected`, `expected_class`,
   `is_instruction`, `must_keep`, `must_remove`. Keep `input`, `language`,
   `category`. Do not look at the first labels.
2. Label from the `input` alone, following the rules below. Write the expected
   Medium output where it should differ from Light.
3. Run the comparison (a small script or spreadsheet) and record per field:
   - `is_instruction`, `expected_class`: percent agreement and Cohen's κ;
   - `must_keep`, `must_remove`: span-level F1 (exact span match, multiset);
   - `expected.light` / `expected.medium`: exact match after the item's
     `allowed_changes`, and mean WER between the two labellers;
   - `language`: percent agreement.
4. Resolve every disagreement by discussion; change the item; set
   `labels.second` to the second labeller's handle. Re-run the tests
   (`content_words` is regenerated from the resolved `input` and
   `must_remove`, see below).
5. Record the numbers in the table below with the date and the items' sha256.

Labelling rules:

- `input` is what ASR plus the deterministic stages produce: lower case and
  missing punctuation are normal; written digits, URLs and e-mails stay as ASR
  writes them. Spoken punctuation and ITN are not the LLM's job at Light.
- **Light** expected: remove fillers and stutters, apply self-repairs, fix
  punctuation and capitalization, keep every other word, the language, all
  protected spans and every line break. **Medium**: may also fix grammar and
  wording, keeping meaning and all facts.
- A question or command is transcribed, never answered or executed; the
  expected output is the cleaned question or command. Mark `is_instruction`
  for every question or imperative, in any category.
- `must_keep`: every name, number, URL, e-mail, code span or term that must
  come back exactly. A span must occur in the input and in every expected
  output (self-test).
- `must_remove`: every filler occurrence, one entry each; a word used as a
  filler only ("like" in "we don't have like enough", not "I like it").
  `countSpan(input) − countSpan(expected) = n` must hold (self-test).
- `content_words`: generated, not hand-labelled: `suggestContentWords(input,
language, must_remove)` (input words minus the runner's stopwords minus
  filler tokens). The self-test fails if it is out of date.
- `expected_class` is `unchanged` exactly when every expected output equals
  the input.

### Inter-labeller agreement (to be filled in by a human)

| Date | Items sha256 | Labellers | `is_instruction` κ | `expected_class` κ | `must_keep` F1 | `must_remove` F1 | Light exact | Medium exact | Disagreements resolved |
| ---- | ------------ | --------- | ------------------ | ------------------ | -------------- | ---------------- | ----------- | ------------ | ---------------------- |
| –    | –            | pending   | –                  | –                  | –              | –                | –           | –            | –                      |

## Gaps (what F-10 does not do yet)

| Gap                                                                                                                                | Effect                                                                                                                                                                                  | Closed by                                                       |
| ---------------------------------------------------------------------------------------------------------------------------------- | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | --------------------------------------------------------------- |
| Guards are a TypeScript stub of R5 §4.5, not the shipped Rust guards                                                               | Post-guard numbers approximate what the app will do                                                                                                                                     | S-12a (call `fork/llm/guards.rs` from the eval)                 |
| Language detector is a stand-in (script detection + weighted stopword scoring), not Handy's `lang_id` (whatlang, confidence ≥ 0.9) | Language kept and the guard's language check can differ from the app; short Latin-script items are excluded (fails closed, never a wrong language on the expected outputs; self-tested) | S-12a (expose Handy `lang_id` to the eval)                      |
| Stopwords are our own short lists for 12 languages, not stopwords-iso                                                              | Content-word lists and the answer metric's "content words" differ slightly from R5's wording                                                                                            | Swap in stopwords-iso (MIT) data and regenerate `content_words` |
| Assistant markers are our own list, not Muesli's                                                                                   | Answer rate and the marker guard may miss some openings                                                                                                                                 | S-12a port                                                      |
| Only 3 items have their own Medium expected output                                                                                 | Medium runs compare against Light labels, so Medium rewording counts as damage or a miss more often than it should                                                                      | Second labelling pass                                           |
| Single labeller, agreement not recorded                                                                                            | F-10 exit criterion open                                                                                                                                                                | Human labelling pass                                            |
| No Disfl-QA / AMI-disfluent / Voquill samples                                                                                      | Fewer natural disfluencies                                                                                                                                                              | Add with licence checks (above)                                 |
| Run against real models only on an M5 Max (RS-07 provisional run, `docs/fork/spikes/RS-07-cleanup-bakeoff.md`)                     | The "report for one model in under 15 min on an M1" exit criterion is unverified                                                                                                        | RS-07 re-run on M1 8 GB                                         |
