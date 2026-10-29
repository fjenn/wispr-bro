// SPDX-License-Identifier: GPL-3.0-or-later
/**
 * RS-09 speech-model bench runner (R5 §7 RS-09, DEC-51). A bench harness, not
 * product code. It drives `src-tauri/crates/wb-bench-asr` (transcribe-cpp the
 * way the app's engine worker does) over the owner's reading-script
 * recording, scores the output with `wb-bench-asr-text.ts` and prints the
 * report tables.
 *
 *   bun scripts/wb-bench-asr.ts split    # VAD + strongest model, cut sections
 *   bun scripts/wb-bench-asr.ts bench    # every model × applicable clip
 *   bun scripts/wb-bench-asr.ts chunk    # DEC-51 live chunking + streaming
 *   bun scripts/wb-bench-asr.ts report   # tables (markdown) to stdout
 *
 * Options: --bin <wb-bench-asr>, --wav <16 kHz mono WAV>, --asr <model dir>,
 * --vad <silero gguf>, --out <results dir>, --models a,b, --cpu.
 * Audio, transcripts and raw results stay under ~/wb-bench (personal data,
 * never committed).
 */
import { spawnSync } from "child_process";
import fs from "fs";
import os from "os";
import path from "path";
import {
  type Lang,
  type Range,
  type Section,
  type SectionId,
  ZERO,
  addCounts,
  align,
  charCounts,
  isLong,
  loadSections,
  mergeSegments,
  percentile,
  rate,
  seamReport,
  speechSegments,
  tokens,
  wordCounts,
} from "./wb-bench-asr-text";

const HOME = os.homedir();
const RATE = 16000;
export const REPO_ROOT = path.resolve(import.meta.dir, "..");

export type ModelSpec = {
  id: string;
  file: string;
  langs: Lang[];
  license: string;
  ccby: boolean;
  /** Catalog `capabilities.streaming`. */
  streaming: boolean;
  sizeBytes: number;
};

/** RS-09 candidates (R5 §7), Q8_0 files from Handy's catalog. */
export const MODELS: ModelSpec[] = [
  {
    id: "whisper-base",
    file: "whisper-base/whisper-base-Q8_0.gguf",
    langs: ["en", "nl", "fr"],
    license: "apache-2.0",
    ccby: false,
    streaming: false,
    sizeBytes: 84962880,
  },
  {
    id: "whisper-large-v3-turbo",
    file: "whisper-large-v3-turbo/whisper-large-v3-turbo-Q8_0.gguf",
    langs: ["en", "nl", "fr"],
    license: "apache-2.0",
    ccby: false,
    streaming: false,
    sizeBytes: 886381760,
  },
  {
    id: "moonshine-base",
    file: "moonshine-base/moonshine-base-Q8_0.gguf",
    langs: ["en"],
    license: "mit",
    ccby: false,
    streaming: false,
    sizeBytes: 77476480,
  },
  {
    id: "parakeet-tdt-0.6b-v3",
    file: "parakeet-tdt-0.6b-v3/parakeet-tdt-0.6b-v3-Q8_0.gguf",
    langs: ["en", "nl", "fr"],
    license: "cc-by-4.0",
    ccby: true,
    streaming: false,
    sizeBytes: 739508576,
  },
  {
    id: "parakeet-unified-en-0.6b",
    file: "parakeet-unified-en-0.6b/parakeet-unified-en-0.6b-Q8_0.gguf",
    langs: ["en"],
    license: "cc-by-4.0",
    ccby: true,
    streaming: true,
    sizeBytes: 731357568,
  },
  {
    id: "parakeet-tdt_ctc-110m",
    file: "parakeet-tdt_ctc-110m/parakeet-tdt_ctc-110m-Q8_0.gguf",
    langs: ["en"],
    license: "cc-by-4.0",
    ccby: true,
    streaming: false,
    sizeBytes: 135373280,
  },
];

/** The strongest multilingual model, used to split the recording. */
export const SPLIT_MODEL = "whisper-large-v3-turbo";
/** The multilingual model whose word timings place the sentence cuts. */
export const WORDS_MODEL = "parakeet-tdt-0.6b-v3";

export type Opts = {
  bin: string;
  wav: string;
  asr: string;
  vad: string;
  out: string;
  segmentsDir: string;
  models: string[] | null;
  cpu: boolean;
};

export function parseCli(argv: string[]): { cmd: string; opts: Opts } {
  const get = (k: string) => {
    const i = argv.indexOf(`--${k}`);
    return i >= 0 ? argv[i + 1] : undefined;
  };
  const opts: Opts = {
    bin:
      get("bin") ??
      path.join(REPO_ROOT, "src-tauri/target/release/wb-bench-asr"),
    wav:
      get("wav") ??
      path.join(HOME, "wb-bench/audio/reading-script-audio-16k.wav"),
    asr: get("asr") ?? path.join(HOME, "wb-bench/asr"),
    vad:
      get("vad") ??
      path.join(
        REPO_ROOT,
        "src-tauri/resources/models/silero-vad-v6.2-F32.gguf",
      ),
    out: get("out") ?? path.join(HOME, "wb-bench/results/rs09"),
    segmentsDir: get("segments") ?? path.join(HOME, "wb-bench/audio/segments"),
    models: get("models")?.split(",") ?? null,
    cpu: argv.includes("--cpu"),
  };
  return { cmd: argv[0] ?? "help", opts };
}

// ---------------------------------------------------------------------------
// Running the binary

function sh(bin: string, args: string[]): void {
  const r = spawnSync(bin, args, { stdio: ["ignore", "inherit", "inherit"] });
  if (r.status !== 0)
    throw new Error(`${bin} ${args.join(" ")} exited ${r.status}`);
}

function readJson<T>(file: string): T {
  return JSON.parse(fs.readFileSync(file, "utf8")) as T;
}

function writeJson(file: string, v: unknown): void {
  fs.mkdirSync(path.dirname(file), { recursive: true });
  fs.writeFileSync(file, JSON.stringify(v, null, 2));
}

export type Probs = {
  start: number;
  end: number;
  frame_samples: number;
  probs: number[];
};

export function vadProbs(
  o: Opts,
  start: number,
  end: number,
  file: string,
): Probs {
  if (!fs.existsSync(file)) {
    fs.mkdirSync(path.dirname(file), { recursive: true });
    sh(o.bin, [
      "vad",
      "--wav",
      o.wav,
      "--vad",
      o.vad,
      "--start",
      String(start),
      "--end",
      String(end),
      "--out",
      file,
    ]);
  }
  return readJson<Probs>(file);
}

export type Job = {
  id: string;
  start: number;
  end: number;
  language: Lang | null;
};

export type RunResult = {
  id: string;
  audio_ms: number;
  ms: number;
  text: string;
  language_hint: string | null;
  detected_language: string | null;
  error: string | null;
  words?: { t0: number; t1: number; text: string }[];
};

export type Mem = {
  peak_rss: number;
  phys_footprint: number;
  peak_phys_footprint: number;
};

export type RunDoc = {
  model: string;
  cpu: boolean;
  info: {
    backend: string;
    device: string;
    languages: string[];
    supports_streaming: boolean;
    supports_long_form: boolean;
    max_audio_ms: number;
  };
  init_ms: number;
  load_ms: number;
  first_run: RunResult;
  results: RunResult[];
  reload_ms: number;
  after_reload: RunResult;
  after_reload_warm: RunResult;
  mem: Record<"start" | "loaded" | "after_runs" | "unloaded" | "end", Mem>;
};

export function runJobs(
  o: Opts,
  model: ModelSpec,
  jobs: Job[],
  file: string,
  reloadJob?: string,
  cpu = false,
  words = false,
): RunDoc {
  if (!fs.existsSync(file)) {
    const jobsFile = file.replace(/\.json$/, ".jobs.json");
    writeJson(jobsFile, { jobs, reload_job: reloadJob ?? null });
    const args = [
      "run",
      "--model",
      path.join(o.asr, model.file),
      "--wav",
      o.wav,
      "--jobs",
      jobsFile,
      "--out",
      file,
    ];
    if (cpu) args.push("--cpu");
    if (words) args.push("--words");
    sh(o.bin, args);
  }
  return readJson<RunDoc>(file);
}

// ---------------------------------------------------------------------------
// split: find the six sections and every sentence in the one-take recording

export type Utterance = Range & { text: string; lang: string | null };

export type Placed = {
  section: SectionId;
  /** Sentence index within the section (sections 1-3), or -1 for a long
   * section's utterances. */
  sentence: number;
  utterances: number[];
  start: number;
  end: number;
  /** Words the split model heard inside the sentence that are not in the
   * reference (fillers, stumbles, restarts, asides). */
  extraWords: number;
};

export type Split = {
  wav: string;
  samples: number;
  utterances: Utterance[];
  sections: {
    id: SectionId;
    start: number;
    end: number;
    utterances: number[];
  }[];
  sentences: Placed[];
  /** Synthetic shorter takes: en-long cut at the sentence end nearest 1 and
   * 3 minutes. `sentences` = how many reference sentences they hold. */
  prefixes: {
    id: string;
    section: SectionId;
    start: number;
    end: number;
    sentences: number;
  }[];
  /** Utterances that are not part of any reference text (repeats, asides). */
  extras: { utterance: number; reason: string }[];
  notes: string[];
};

/** The language a section's tokens are in, from its id. */
function langOf(id: SectionId): Lang {
  return id.slice(0, 2) as Lang;
}

/** How well `hyp` (an utterance) matches somewhere inside `ref`: the share of
 * its tokens that an alignment against the best-matching window of `ref`
 * gets right. */
export function containment(hyp: string[], ref: string[]): number {
  if (!hyp.length) return 0;
  const set = new Map<string, number>();
  for (const t of ref) set.set(t, (set.get(t) ?? 0) + 1);
  let hit = 0;
  for (const t of hyp) {
    const n = set.get(t) ?? 0;
    if (n > 0) {
      hit++;
      set.set(t, n - 1);
    }
  }
  return hit / hyp.length;
}

/**
 * Assigns each utterance to a section. Utterances that match no section well
 * (an aside, a cough the model turned into words, a fragment) get `null`;
 * the rest go through a Viterbi pass with a cost for switching sections, so
 * each section is one contiguous run (in any order).
 */
export function assignSections(
  utts: Utterance[],
  sections: Section[],
): (SectionId | null)[] {
  const score = utts.map((u) =>
    sections.map((s) =>
      containment(
        tokens(u.text, s.lang, "norm"),
        tokens(s.text, s.lang, "norm"),
      ),
    ),
  );
  const strong = utts
    .map((u, i) => i)
    .filter(
      (i) =>
        tokens(utts[i].text, "en", "raw").length >= 2 &&
        Math.max(...score[i]) >= 0.5,
    );
  const S = sections.length;
  const SWITCH = 1.5;
  const out: (SectionId | null)[] = utts.map(() => null);
  if (!strong.length) return out;
  let prev = sections.map((_, k) => 1 - score[strong[0]][k]);
  const back: number[][] = [];
  for (let x = 1; x < strong.length; x++) {
    const i = strong[x];
    const cur: number[] = [];
    const b: number[] = [];
    for (let k = 0; k < S; k++) {
      let best = Infinity;
      let arg = 0;
      for (let j = 0; j < S; j++) {
        const c = prev[j] + (j === k ? 0 : SWITCH);
        if (c < best) {
          best = c;
          arg = j;
        }
      }
      cur.push(best + 1 - score[i][k]);
      b.push(arg);
    }
    back.push(b);
    prev = cur;
  }
  let k = prev.indexOf(Math.min(...prev));
  const pathK = [k];
  for (let x = strong.length - 2; x >= 0; x--) {
    k = back[x][k];
    pathK.push(k);
  }
  pathK.reverse();
  strong.forEach((i, x) => (out[i] = sections[pathK[x]].id));
  return out;
}

/** A word of the split model's output, in samples of the whole file. */
export type TimedWord = {
  utt: number;
  start: number;
  end: number;
  text: string;
};

/**
 * Places a sentence section's words on its sentences: one monotonic
 * alignment of all their tokens to the section text; each word goes to the
 * sentence most of its aligned tokens fall in, or -1 when all of them are
 * insertions (a restart, an aside, a hallucination on a noise).
 */
export function placeWords(words: TimedWord[], section: Section): number[] {
  const lang = section.lang;
  const ref: string[] = [];
  const refSentence: number[] = [];
  section.sentences.forEach((s, k) => {
    for (const w of tokens(s, lang, "norm")) {
      ref.push(w);
      refSentence.push(k);
    }
  });
  const hyp: string[] = [];
  const hypWord: number[] = [];
  words.forEach((w, i) => {
    for (const t of tokens(w.text, lang, "norm")) {
      hyp.push(t);
      hypWord.push(i);
    }
  });
  const votes = words.map(() => new Map<number, number>());
  for (const s of align(ref, hyp)) {
    if (s.op === "del" || s.op === "ins") continue;
    const m = votes[hypWord[s.hypIndex]];
    const k = refSentence[s.refIndex];
    m.set(k, (m.get(k) ?? 0) + (s.op === "ok" ? 1 : 0.5));
  }
  let max = -1;
  return votes.map((m) => {
    if (!m.size) return -1;
    const k = [...m.entries()].sort((a, b) => b[1] - a[1] || a[0] - b[0])[0][0];
    if (k < max) return -1;
    max = k;
    return k;
  });
}

/** The middle of the longest silence (outside `speech`) in `[a, b]`, else
 * the middle of `[a, b]`. */
export function silenceCut(speech: Range[], a: number, b: number): number {
  if (b <= a) return Math.round((a + b) / 2);
  let best: Range | null = null;
  let cursor = a;
  const consider = (s: number, e: number) => {
    if (e > s && (!best || e - s > best.end - best.start))
      best = { start: s, end: e };
  };
  for (const r of speech) {
    if (r.end <= a) continue;
    if (r.start >= b) break;
    consider(cursor, Math.min(r.start, b));
    cursor = Math.max(cursor, r.end);
  }
  consider(cursor, b);
  const g = best as Range | null;
  return g ? Math.round((g.start + g.end) / 2) : Math.round((a + b) / 2);
}

/** The middle of the longest pause between utterances `a` and `b` (a < b). */
function cutBetween(
  utts: Utterance[],
  a: number,
  b: number,
): { at: number; after: number } {
  let best = a;
  for (let i = a; i < b; i++) {
    if (utts[i + 1].start - utts[i].end > utts[best + 1].start - utts[best].end)
      best = i;
  }
  return {
    at: Math.round((utts[best].end + utts[best + 1].start) / 2),
    after: best,
  };
}

/** Utterance merge gap: pauses shorter than this stay inside one utterance. */
export const UTTERANCE_GAP_MS = 300;

export function cmdSplit(o: Opts): Split {
  const sections = loadSections();
  // The binary clamps `end` to the file; its reply says how long it is.
  const probs = vadProbs(
    o,
    0,
    Number.MAX_SAFE_INTEGER,
    path.join(o.out, "split/vad-full.json"),
  );
  const samples = probs.end;
  const segs = speechSegments(probs.probs, probs.frame_samples, samples);
  const utt = mergeSegments(segs, UTTERANCE_GAP_MS);
  const turbo = MODELS.find((m) => m.id === SPLIT_MODEL)!;
  const jobs: Job[] = utt.map((r, i) => ({
    id: `u${i}`,
    start: r.start,
    end: r.end,
    language: null,
  }));
  const doc = runJobs(
    o,
    turbo,
    jobs,
    path.join(o.out, `split/turbo-utterances-${UTTERANCE_GAP_MS}.json`),
  );
  const utts: Utterance[] = utt.map((r, i) => ({
    ...r,
    text: doc.results[i].text,
    lang: doc.results[i].detected_language,
  }));
  const assigned = assignSections(utts, sections);
  const notes: string[] = [];
  const out: Split = {
    wav: o.wav,
    samples,
    utterances: utts,
    sections: [],
    sentences: [],
    prefixes: [],
    extras: [],
    notes,
  };

  // Runs of strongly assigned utterances, in order.
  const runs: { id: SectionId; first: number; last: number }[] = [];
  assigned.forEach((id, i) => {
    if (id === null) return;
    const last = runs[runs.length - 1];
    if (last && last.id === id) last.last = i;
    else runs.push({ id, first: i, last: i });
  });
  const seen = new Set<SectionId>();
  for (const r of runs) {
    if (seen.has(r.id))
      notes.push(`section ${r.id} appears in more than one run`);
    seen.add(r.id);
  }
  for (const s of sections)
    if (!seen.has(s.id)) notes.push(`section ${s.id} not found`);
  notes.push(`reading order: ${runs.map((r) => r.id).join(", ")}`);

  // Section cuts: the longest pause between two runs.
  let start = 0;
  let firstUtt = 0;
  runs.forEach((r, k) => {
    let end = samples;
    let lastUtt = utts.length - 1;
    if (k + 1 < runs.length) {
      const c = cutBetween(utts, r.last, runs[k + 1].first);
      end = c.at;
      lastUtt = c.after;
    }
    const idx: number[] = [];
    for (let i = firstUtt; i <= lastUtt; i++) idx.push(i);
    out.sections.push({ id: r.id, start, end, utterances: idx });
    start = end;
    firstUtt = lastUtt + 1;
  });

  // Word timings for the sentence cuts: whisper in transcribe.cpp reports
  // segments only, so the strongest multilingual model with word timings
  // (Parakeet TDT 0.6B v3) transcribes the sentence sections' utterances
  // again, told each section's language.
  const words = MODELS.find((m) => m.id === WORDS_MODEL)!;
  const wjobs: Job[] = out.sections
    .filter((sec) => !isLong(sec.id) || sec.id === "en-long")
    .flatMap((sec) =>
      sec.utterances.map((i) => ({
        id: `u${i}`,
        start: utts[i].start,
        end: utts[i].end,
        language: langOf(sec.id),
      })),
    );
  const wdoc = runJobs(
    o,
    words,
    wjobs,
    path.join(o.out, `split/${WORDS_MODEL}-words-${UTTERANCE_GAP_MS}.json`),
    undefined,
    false,
    true,
  );
  const byId = new Map(wdoc.results.map((r) => [r.id, r]));
  const timed = (i: number): TimedWord[] =>
    (byId.get(`u${i}`)?.words ?? []).map((w) => ({
      utt: i,
      start: utts[i].start + Math.round((w.t0 * RATE) / 1000),
      end: utts[i].start + Math.round((w.t1 * RATE) / 1000),
      text: w.text,
    }));

  // Sentences: from the first to the last word placed on each, cut in the
  // longest silence between neighbours, with whatever the reader said in
  // between (stumbles, restarts, asides) left in.
  const PAD = RATE / 2;
  for (const sec of out.sections) {
    const section = sections.find((s) => s.id === sec.id)!;
    if (isLong(sec.id)) continue;
    const words = sec.utterances.flatMap(timed);
    const place = placeWords(words, section);
    const spans: { k: number; first: number; last: number }[] = [];
    for (let k = 0; k < section.sentences.length; k++) {
      const ws = place.map((p, i) => (p === k ? i : -1)).filter((i) => i >= 0);
      if (!ws.length) {
        notes.push(`${sec.id} #${k + 1}: not found`);
        continue;
      }
      spans.push({ k, first: ws[0], last: ws[ws.length - 1] });
    }
    spans.forEach((sp, x) => {
      const prev = spans[x - 1];
      const next = spans[x + 1];
      const lo = prev
        ? silenceCut(segs, words[prev.last].end, words[sp.first].start)
        : sec.start;
      const hi = next
        ? silenceCut(segs, words[sp.last].end, words[next.first].start)
        : sec.end;
      // At most half a second of silence on either side of the speech.
      const speechStart =
        segs.find((r) => r.end > words[sp.first].start)?.start ??
        words[sp.first].start;
      const speechEnd =
        [...segs].reverse().find((r) => r.start < words[sp.last].end)?.end ??
        words[sp.last].end;
      const start = Math.max(
        lo,
        Math.min(words[sp.first].start, speechStart) - PAD,
      );
      const end = Math.min(hi, Math.max(words[sp.last].end, speechEnd) + PAD);
      const inside = words
        .slice(sp.first, sp.last + 1)
        .filter((_, j) => place[sp.first + j] !== sp.k);
      if (inside.length)
        notes.push(
          `${sec.id} #${sp.k + 1}: ${inside.length} word(s) not in the reference inside the sentence`,
        );
      const us = [
        ...new Set(words.slice(sp.first, sp.last + 1).map((w) => w.utt)),
      ];
      out.sentences.push({
        section: sec.id,
        sentence: sp.k,
        utterances: us,
        start,
        end,
        extraWords: inside.length,
      });
    });
    const unplacedOutside = words.filter(
      (w, i) =>
        place[i] < 0 && !spans.some((sp) => i >= sp.first && i <= sp.last),
    );
    for (const w of unplacedOutside)
      out.extras.push({
        utterance: w.utt,
        reason: `word between sentences of ${sec.id}: ${w.text.trim()}`,
      });
  }

  // Prefixes of en-long: cut in the silence after the sentence that ends
  // nearest 1 and 3 minutes in.
  const enLong = out.sections.find((sec) => sec.id === "en-long");
  if (enLong) {
    const section = sections.find((s) => s.id === "en-long")!;
    const words = enLong.utterances.flatMap(timed);
    const place = placeWords(words, section);
    const lastOf = (k: number) => place.lastIndexOf(k);
    const firstOf = (k: number) => place.indexOf(k);
    for (const [id, secs] of [
      ["en-long-1m", 60],
      ["en-long-3m", 180],
    ] as const) {
      let best: { k: number; cut: number } | null = null;
      for (let k = 1; k < section.sentences.length; k++) {
        const a = lastOf(k - 1);
        const b = firstOf(k);
        if (a < 0 || b < 0 || b < a) continue;
        const cut = silenceCut(segs, words[a].end, words[b].start);
        const off = Math.abs(cut - enLong.start - secs * RATE);
        if (!best || off < Math.abs(best.cut - enLong.start - secs * RATE))
          best = { k, cut };
      }
      if (best)
        out.prefixes.push({
          id,
          section: "en-long",
          start: enLong.start,
          end: best.cut,
          sentences: best.k,
        });
      else notes.push(`no sentence boundary for ${id}`);
    }
  }
  writeJson(path.join(o.out, "split/split.json"), out);
  writeSegments(o, out);
  return out;
}

/** Writes each section (and each sentence) as its own 16 kHz WAV. */
function writeSegments(o: Opts, s: Split): void {
  fs.mkdirSync(o.segmentsDir, { recursive: true });
  const cut = (start: number, end: number, name: string) =>
    sh("ffmpeg", [
      "-y",
      "-loglevel",
      "error",
      "-i",
      o.wav,
      "-af",
      `atrim=start_sample=${start}:end_sample=${end}`,
      "-ar",
      "16000",
      "-ac",
      "1",
      "-c:a",
      "pcm_s16le",
      path.join(o.segmentsDir, name),
    ]);
  for (const sec of s.sections) cut(sec.start, sec.end, `${sec.id}.wav`);
  for (const p of s.sentences)
    cut(
      p.start,
      p.end,
      `${p.section}-${String(p.sentence + 1).padStart(2, "0")}.wav`,
    );
  for (const p of s.prefixes) cut(p.start, p.end, `${p.id}.wav`);
  // A synthetic ~5-minute English take (DEC-51): en-sentences then en-long.
  const a = s.sections.find((x) => x.id === "en-sentences");
  const b = s.sections.find((x) => x.id === "en-long");
  if (a && b) {
    sh("ffmpeg", [
      "-y",
      "-loglevel",
      "error",
      "-i",
      path.join(o.segmentsDir, "en-sentences.wav"),
      "-i",
      path.join(o.segmentsDir, "en-long.wav"),
      "-filter_complex",
      "[0:a][1:a]concat=n=2:v=0:a=1",
      "-ar",
      "16000",
      "-ac",
      "1",
      "-c:a",
      "pcm_s16le",
      path.join(o.segmentsDir, SYNTH_5M),
    ]);
  }
}

/** The synthetic 5-minute take's file name in the segments directory. */
export const SYNTH_5M = "en-5m-synthetic.wav";

// ---------------------------------------------------------------------------

// ---------------------------------------------------------------------------
// Clips

export type Clip = {
  id: string;
  wav: string;
  start: number;
  end: number;
  lang: Lang;
  section: SectionId;
  kind: "sentence" | "long";
  /** Reference text (the script's words, not what the reader said). */
  ref: string;
  /** Sentence clips where the split model heard words outside the script. */
  disfluent: boolean;
};

export function loadSplit(o: Opts): Split {
  return readJson<Split>(path.join(o.out, "split/split.json"));
}

function wavSamples(file: string): number {
  // 16-bit mono PCM written by ffmpeg: find the `data` chunk.
  const b = fs.readFileSync(file);
  let at = 12;
  while (at + 8 <= b.length) {
    const id = b.toString("ascii", at, at + 4);
    const size = b.readUInt32LE(at + 4);
    if (id === "data") return Math.floor(Math.min(size, b.length - at - 8) / 2);
    at += 8 + size + (size % 2);
  }
  throw new Error(`no data chunk in ${file}`);
}

export function clips(o: Opts, s: Split): Clip[] {
  const sections = loadSections();
  const ref = (id: SectionId) => sections.find((x) => x.id === id)!;
  const out: Clip[] = [];
  for (const p of s.sentences) {
    const sec = ref(p.section);
    out.push({
      id: `${p.section}-${String(p.sentence + 1).padStart(2, "0")}`,
      wav: o.wav,
      start: p.start,
      end: p.end,
      lang: sec.lang,
      section: p.section,
      kind: "sentence",
      ref: sec.sentences[p.sentence],
      disfluent: p.extraWords > 0,
    });
  }
  for (const sec of s.sections) {
    if (!isLong(sec.id)) continue;
    out.push({
      id: sec.id,
      wav: o.wav,
      start: sec.start,
      end: sec.end,
      lang: ref(sec.id).lang,
      section: sec.id,
      kind: "long",
      ref: ref(sec.id).text,
      disfluent: false,
    });
  }
  for (const p of s.prefixes) {
    out.push({
      id: p.id,
      wav: o.wav,
      start: p.start,
      end: p.end,
      lang: "en",
      section: p.section,
      kind: "long",
      ref: ref(p.section).sentences.slice(0, p.sentences).join(" "),
      disfluent: false,
    });
  }
  const synth = path.join(o.segmentsDir, SYNTH_5M);
  if (fs.existsSync(synth)) {
    out.push({
      id: "en-5m",
      wav: synth,
      start: 0,
      end: wavSamples(synth),
      lang: "en",
      section: "en-long",
      kind: "long",
      ref: ref("en-sentences").text + " " + ref("en-long").text,
      disfluent: false,
    });
  }
  return out;
}

const REPS = 3;

function modelsFor(o: Opts, cpu: boolean): ModelSpec[] {
  const ids =
    o.models ??
    (cpu ? ["whisper-base", "parakeet-tdt-0.6b-v3"] : MODELS.map((m) => m.id));
  return MODELS.filter((m) => ids.includes(m.id));
}

/** The sentence clip closest to 10 s in a language the model has: the
 * "10 s take" re-run after unload and reload. */
function tenSecondJob(model: ModelSpec, cs: Clip[]): string {
  const sentences = cs.filter(
    (c) => c.kind === "sentence" && model.langs.includes(c.lang),
  );
  sentences.sort(
    (a, b) =>
      Math.abs(a.end - a.start - 10 * RATE) -
      Math.abs(b.end - b.start - 10 * RATE),
  );
  return `${sentences[0].id}#0`;
}

export function cmdBench(o: Opts): void {
  const s = loadSplit(o);
  const cs = clips(o, s);
  const tag = o.cpu ? "-cpu" : "";
  for (const m of modelsFor(o, o.cpu)) {
    const mine = cs.filter((c) => m.langs.includes(c.lang));
    const reps = o.cpu ? 1 : REPS;
    const jobs: Job[] = [];
    for (let r = 0; r < reps; r++)
      for (const c of mine.filter((c) => c.kind === "sentence"))
        jobs.push({
          id: `${c.id}#${r}`,
          start: c.start,
          end: c.end,
          language: c.lang,
        });
    for (const c of mine.filter((c) => c.kind === "long" && c.wav === o.wav))
      jobs.push({
        id: `${c.id}#0`,
        start: c.start,
        end: c.end,
        language: c.lang,
      });
    console.log(`== ${m.id}${tag}: ${jobs.length} runs`);
    runJobs(
      o,
      m,
      jobs,
      path.join(o.out, `bench/${m.id}${tag}.json`),
      tenSecondJob(m, cs),
      o.cpu,
    );
    const synth = mine.find((c) => c.id === "en-5m");
    if (synth) {
      const so = { ...o, wav: synth.wav };
      runJobs(
        so,
        m,
        [{ id: "en-5m#0", start: synth.start, end: synth.end, language: "en" }],
        path.join(o.out, `bench/${m.id}${tag}-5m.json`),
        undefined,
        o.cpu,
      );
    }
  }
}

// ---------------------------------------------------------------------------
// DEC-51: chunked while speaking, and streaming

export type ChunkDoc = {
  info: RunDoc["info"];
  warm_ms: number;
  samples: number;
  chunks: {
    index: number;
    emitted_at: number | null;
    samples: number;
    run: RunResult;
  }[];
  after_key_up_ms: number;
  text: string;
};

export type StreamDoc = {
  audio_ms: number;
  feeds: number;
  feed_ms_total: number;
  feed_ms_max: number;
  backlog_at_key_up_ms: number;
  finalize_ms: number;
  text: string;
};

/**
 * DEC-51 "eager" plan: cut in the middle of every pause of at least
 * `pauseMs` once the chunk is at least `minS` long; past `maxS` without such
 * a pause, cut at the longest pause in the chunk. A chunk is ready (can be
 * decoded while the user keeps talking) once its pause has lasted `pauseMs`.
 */
export function eagerPlan(
  probs: Probs,
  pauseMs = 500,
  minS = 4,
  maxS = 30,
): { start: number; end: number; readyAt: number }[] {
  const n = probs.end - probs.start;
  const segs = speechSegments(probs.probs, probs.frame_samples, n);
  const pauses: { start: number; end: number }[] = [];
  for (let i = 0; i + 1 < segs.length; i++)
    pauses.push({ start: segs[i].end, end: segs[i + 1].start });
  const out: { start: number; end: number; readyAt: number }[] = [];
  let from = 0;
  const pause = (pauseMs * RATE) / 1000;
  for (let i = 0; i < pauses.length; i++) {
    const p = pauses[i];
    const len = p.start - from;
    if (len < minS * RATE) continue;
    if (p.end - p.start >= pause) {
      const cut = Math.round((p.start + p.end) / 2);
      out.push({
        start: from,
        end: cut,
        readyAt: Math.min(n, p.start + pause),
      });
      from = cut;
    } else if (len > maxS * RATE) {
      // Longest pause since `from`.
      const cands = pauses
        .slice(0, i + 1)
        .filter((q) => q.start > from + minS * RATE);
      const best =
        cands.sort((a, b) => b.end - b.start - (a.end - a.start))[0] ?? p;
      const cut = Math.round((best.start + best.end) / 2);
      out.push({ start: from, end: cut, readyAt: p.end });
      from = cut;
    }
  }
  out.push({ start: from, end: n, readyAt: n });
  return out;
}

/** When the last chunk's text is ready after key-up, given when each chunk
 * could start (samples from the clip start) and how long each took (ms):
 * one engine, chunks decoded in order. */
export function keyUpLatency(
  readyAt: number[],
  ms: number[],
  endSamples: number,
): number {
  let done = 0; // ms from clip start
  for (let i = 0; i < ms.length; i++) {
    const ready = (readyAt[i] / RATE) * 1000;
    done = Math.max(done, ready) + ms[i];
  }
  return done - (endSamples / RATE) * 1000;
}

export function cmdChunk(o: Opts): void {
  const s = loadSplit(o);
  const cs = clips(o, s).filter((c) => c.kind === "long");
  for (const c of cs) {
    const probsFile = path.join(o.out, `vad/${c.id}.json`);
    const probs = vadProbs({ ...o, wav: c.wav }, c.start, c.end, probsFile);
    for (const m of modelsFor(o, false)) {
      if (!m.langs.includes(c.lang)) continue;
      // (A) Handy's live Chunker.
      const fa = path.join(o.out, `chunk/${m.id}/${c.id}.handy.json`);
      if (!fs.existsSync(fa)) {
        fs.mkdirSync(path.dirname(fa), { recursive: true });
        console.log(`== chunk A ${m.id} ${c.id}`);
        sh(o.bin, [
          "chunk",
          "--model",
          path.join(o.asr, m.file),
          "--wav",
          c.wav,
          "--start",
          String(c.start),
          "--end",
          String(c.end),
          "--language",
          c.lang,
          "--probs",
          probsFile,
          "--out",
          fa,
        ]);
      }
      // (B) Eager pause chunks.
      const plan = eagerPlan(probs);
      const jobs: Job[] = plan.map((p, i) => ({
        id: `b${i}`,
        start: c.start + p.start,
        end: c.start + p.end,
        language: c.lang,
      }));
      console.log(`== chunk B ${m.id} ${c.id}: ${jobs.length} chunks`);
      runJobs(
        { ...o, wav: c.wav },
        m,
        jobs,
        path.join(o.out, `chunk/${m.id}/${c.id}.eager.json`),
      );
      // (C) Streaming, where the model streams.
      if (m.streaming) {
        const fc = path.join(o.out, `stream/${m.id}/${c.id}.json`);
        if (!fs.existsSync(fc)) {
          fs.mkdirSync(path.dirname(fc), { recursive: true });
          sh(o.bin, [
            "stream",
            "--model",
            path.join(o.asr, m.file),
            "--wav",
            c.wav,
            "--start",
            String(c.start),
            "--end",
            String(c.end),
            "--language",
            c.lang,
            "--out",
            fc,
          ]);
        }
      }
    }
  }
  // Streaming on the short English takes too.
  for (const m of modelsFor(o, false).filter((m) => m.streaming)) {
    for (const c of clips(o, s).filter(
      (c) => c.kind === "sentence" && m.langs.includes(c.lang),
    )) {
      const fc = path.join(o.out, `stream/${m.id}/${c.id}.json`);
      if (fs.existsSync(fc)) continue;
      fs.mkdirSync(path.dirname(fc), { recursive: true });
      sh(o.bin, [
        "stream",
        "--model",
        path.join(o.asr, m.file),
        "--wav",
        c.wav,
        "--start",
        String(c.start),
        "--end",
        String(c.end),
        "--language",
        c.lang,
        "--out",
        fc,
      ]);
    }
  }
}

// ---------------------------------------------------------------------------
// Report

const pct = (x: number) => (100 * x).toFixed(1);
const ms0 = (x: number | null) =>
  x === null ? "–" : Math.round(x).toLocaleString("en-US");
const mb = (b: number) => Math.round(b / 1e6).toLocaleString("en-US");

export type Scored = {
  clip: Clip;
  text: string;
  ms: number;
  audioMs: number;
  error: string | null;
};

function score(items: Scored[]) {
  let norm = ZERO;
  let raw = ZERO;
  let chars = ZERO;
  let ms = 0;
  let audio = 0;
  for (const it of items) {
    norm = addCounts(
      norm,
      wordCounts(it.clip.ref, it.text, it.clip.lang, "norm"),
    );
    raw = addCounts(raw, wordCounts(it.clip.ref, it.text, it.clip.lang, "raw"));
    chars = addCounts(chars, charCounts(it.clip.ref, it.text, it.clip.lang));
    ms += it.ms;
    audio += it.audioMs;
  }
  return { norm, raw, chars, ms, audio, rtf: audio ? ms / audio : 0 };
}

/** Marked-up alignment of one sentence, only around its errors. */
export function errorSnippet(
  ref: string,
  hyp: string,
  lang: Lang,
  context = 2,
): string {
  const steps = align(tokens(ref, lang, "norm"), tokens(hyp, lang, "norm"));
  const keep = new Set<number>();
  steps.forEach((st, i) => {
    if (st.op !== "ok")
      for (let j = i - context; j <= i + context; j++) keep.add(j);
  });
  const parts: string[] = [];
  let gap = false;
  steps.forEach((st, i) => {
    if (!keep.has(i)) {
      gap = true;
      return;
    }
    if (gap && parts.length) parts.push("…");
    gap = false;
    parts.push(
      st.op === "ok"
        ? st.ref!
        : st.op === "sub"
          ? `**${st.ref}→${st.hyp}**`
          : st.op === "del"
            ? `**−${st.ref}**`
            : `**+${st.hyp}**`,
    );
  });
  return parts.join(" ");
}

export function cmdReport(o: Opts): string {
  const s = loadSplit(o);
  const cs = clips(o, s);
  const byId = new Map(cs.map((c) => [c.id, c]));
  const lines: string[] = [];
  const out = (l = "") => lines.push(l);
  const langs: Lang[] = ["en", "nl", "fr"];

  type Loaded = { m: ModelSpec; doc: RunDoc; doc5?: RunDoc; cpu: boolean };
  const loaded: Loaded[] = [];
  for (const cpu of [false, true])
    for (const m of MODELS) {
      const f = path.join(o.out, `bench/${m.id}${cpu ? "-cpu" : ""}.json`);
      if (!fs.existsSync(f)) continue;
      const f5 = f.replace(/\.json$/, "-5m.json");
      loaded.push({
        m,
        doc: readJson(f),
        doc5: fs.existsSync(f5) ? readJson(f5) : undefined,
        cpu,
      });
    }
  const results = (l: Loaded): Scored[] =>
    [...l.doc.results, ...(l.doc5?.results ?? [])].map((r) => ({
      clip: byId.get(r.id.split("#")[0])!,
      text: r.text,
      ms: r.ms,
      audioMs: r.audio_ms,
      error: r.error,
    }));
  const name = (l: Loaded) => `${l.m.id}${l.cpu ? " (CPU)" : ""}`;

  // Short takes, per language.
  for (const lang of langs) {
    out(`#### ${lang}: short takes (12 sentences, ${REPS} runs each on Metal)`);
    out();
    out(
      "| Model | WER norm | WER raw | CER | WER norm, fluent only | p50 ms | p95 ms | max ms | RTF |",
    );
    out("| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: |");
    for (const l of loaded) {
      if (!l.m.langs.includes(lang)) continue;
      const all = l.doc.results
        .map((r) => ({ r, c: byId.get(r.id.split("#")[0])! }))
        .filter((x) => x.c && x.c.kind === "sentence" && x.c.lang === lang);
      const first = all.filter((x) => x.r.id.endsWith("#0"));
      const sc = score(
        first.map((x) => ({
          clip: x.c,
          text: x.r.text,
          ms: x.r.ms,
          audioMs: x.r.audio_ms,
          error: x.r.error,
        })),
      );
      const fluent = score(
        first
          .filter((x) => !x.c.disfluent)
          .map((x) => ({
            clip: x.c,
            text: x.r.text,
            ms: x.r.ms,
            audioMs: x.r.audio_ms,
            error: x.r.error,
          })),
      );
      const lat = all.map((x) => x.r.ms);
      const rtf =
        all.reduce((a, x) => a + x.r.ms, 0) /
        all.reduce((a, x) => a + x.r.audio_ms, 0);
      out(
        `| ${name(l)} | ${pct(rate(sc.norm))} | ${pct(rate(sc.raw))} | ${pct(rate(sc.chars))} | ${pct(rate(fluent.norm))} | ${ms0(percentile(lat, 50))} | ${ms0(percentile(lat, 95))} | ${ms0(Math.max(...lat))} | ${rtf.toFixed(3)} |`,
      );
    }
    out();
  }

  // Long takes, whole.
  out("#### Long takes, one run over the whole take");
  out();
  out(
    "| Model | Take | Audio s | WER norm | WER raw | CER | Time ms | RTF | Error |",
  );
  out("| --- | --- | ---: | ---: | ---: | ---: | ---: | ---: | --- |");
  for (const l of loaded) {
    for (const x of results(l).filter(
      (x) => x.clip && x.clip.kind === "long",
    )) {
      const sc = score([x]);
      out(
        `| ${name(l)} | ${x.clip.id} | ${(x.audioMs / 1000).toFixed(0)} | ${pct(rate(sc.norm))} | ${pct(rate(sc.raw))} | ${pct(rate(sc.chars))} | ${ms0(x.ms)} | ${sc.rtf.toFixed(3)} | ${x.error ?? ""} |`,
      );
    }
  }
  out();

  // Load, reload, memory.
  out("#### Load, first run, reload and memory");
  out();
  out(
    "| Model | Backend | Load ms (first) | First run ms (clip) | Same clip warm ms | Reload ms | Run after reload ms | Peak RSS MB | Peak footprint MB | Footprint after unload MB |",
  );
  out("| --- | --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: |");
  for (const l of loaded) {
    const d = l.doc;
    const warm = d.results.find((r) => r.id === d.first_run.id)?.ms ?? null;
    out(
      `| ${name(l)} | ${d.info.backend} | ${ms0(d.load_ms)} | ${ms0(d.first_run.ms)} (${d.first_run.id.split("#")[0]}, ${(d.first_run.audio_ms / 1000).toFixed(1)} s) | ${ms0(warm)} | ${ms0(d.reload_ms)} | ${ms0(d.after_reload.ms)} → ${ms0(d.after_reload_warm.ms)} warm (${d.after_reload.id.split("#")[0]}, ${(d.after_reload.audio_ms / 1000).toFixed(1)} s) | ${mb(d.mem.end.peak_rss)} | ${mb(d.mem.end.peak_phys_footprint)} | ${mb(d.mem.unloaded.phys_footprint)} |`,
    );
  }
  out();

  // Chunking.
  out("#### DEC-51: whole take vs chunked while speaking");
  out();
  out(
    "Key-up = time from the end of the audio to the joined text. Whole = one run after key-up (what a non-chunking app waits). A = Handy's live `Chunker` (≈30 s chunks at pauses, last chunk decoded after key-up). B = eager chunks at every ≥ 0.5 s pause once ≥ 4 s long. Lost / doubled = reference words deleted / words repeated within 2 words of a seam.",
  );
  out();
  out(
    "| Model | Take | Audio s | Whole WER | Whole key-up ms | A chunks | A WER | A key-up ms | A lost/doubled | B chunks | B WER | B key-up ms | B lost/doubled |",
  );
  out(
    "| --- | --- | ---: | ---: | ---: | ---: | ---: | ---: | --- | ---: | ---: | ---: | --- |",
  );
  const seamExamples: string[] = [];
  for (const l of loaded.filter((l) => !l.cpu)) {
    for (const c of cs.filter(
      (c) => c.kind === "long" && l.m.langs.includes(c.lang),
    )) {
      const whole = results(l).find((x) => x.clip?.id === c.id);
      const fa = path.join(o.out, `chunk/${l.m.id}/${c.id}.handy.json`);
      const fb = path.join(o.out, `chunk/${l.m.id}/${c.id}.eager.json`);
      if (!whole || !fs.existsSync(fa) || !fs.existsSync(fb)) continue;
      const a = readJson<ChunkDoc>(fa);
      const b = readJson<RunDoc>(fb);
      const refT = tokens(c.ref, c.lang, "norm");
      const wWer = rate(wordCounts(c.ref, whole.text, c.lang, "norm"));
      const aText = a.text;
      const aSeam = seamReport(
        refT,
        a.chunks.map((k) => tokens(k.run.text, c.lang, "norm")),
      );
      const aKey = keyUpLatency(
        a.chunks.map((k) => k.emitted_at ?? a.samples),
        a.chunks.map((k) => k.run.ms),
        a.samples,
      );
      const probs = readJson<Probs>(path.join(o.out, `vad/${c.id}.json`));
      const plan = eagerPlan(probs);
      const bText = b.results
        .map((r) => r.text.trim())
        .filter(Boolean)
        .join(" ");
      const bSeam = seamReport(
        refT,
        b.results.map((r) => tokens(r.text, c.lang, "norm")),
      );
      const bKey = keyUpLatency(
        plan.map((p) => p.readyAt),
        b.results.map((r) => r.ms),
        c.end - c.start,
      );
      out(
        `| ${l.m.id} | ${c.id} | ${((c.end - c.start) / RATE).toFixed(0)} | ${pct(wWer)} | ${ms0(whole.ms)} | ${a.chunks.length} | ${pct(rate(wordCounts(c.ref, aText, c.lang, "norm")))} | ${ms0(aKey)} | ${aSeam.lost}/${aSeam.doubled} | ${b.results.length} | ${pct(rate(wordCounts(c.ref, bText, c.lang, "norm")))} | ${ms0(bKey)} | ${bSeam.lost}/${bSeam.doubled} |`,
      );
      for (const e of [
        ...aSeam.examples.map((x) => `A ${x}`),
        ...bSeam.examples.map((x) => `B ${x}`),
      ])
        seamExamples.push(`${l.m.id} ${c.id}: ${e}`);
    }
  }
  out();

  // Streaming.
  const streamers = MODELS.filter((m) => m.streaming);
  for (const m of streamers) {
    const dir = path.join(o.out, `stream/${m.id}`);
    if (!fs.existsSync(dir)) continue;
    out(`#### Streaming: ${m.id} (StreamOptions::default(), 480-sample feeds)`);
    out();
    out(
      "| Take | Audio s | WER norm | WER raw | Feed compute / audio | Max feed ms | Backlog at key-up ms | Finalize ms |",
    );
    out("| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: |");
    const sentenceFin: number[] = [];
    let sNorm = ZERO;
    let sRaw = ZERO;
    for (const c of cs.filter((c) => m.langs.includes(c.lang))) {
      const f = path.join(dir, `${c.id}.json`);
      if (!fs.existsSync(f)) continue;
      const d = readJson<StreamDoc>(f);
      if (c.kind === "sentence") {
        sentenceFin.push(d.finalize_ms + d.backlog_at_key_up_ms);
        sNorm = addCounts(sNorm, wordCounts(c.ref, d.text, c.lang, "norm"));
        sRaw = addCounts(sRaw, wordCounts(c.ref, d.text, c.lang, "raw"));
        continue;
      }
      out(
        `| ${c.id} | ${(d.audio_ms / 1000).toFixed(0)} | ${pct(rate(wordCounts(c.ref, d.text, c.lang, "norm")))} | ${pct(rate(wordCounts(c.ref, d.text, c.lang, "raw")))} | ${(d.feed_ms_total / d.audio_ms).toFixed(3)} | ${ms0(d.feed_ms_max)} | ${ms0(d.backlog_at_key_up_ms)} | ${ms0(d.finalize_ms)} |`,
      );
    }
    if (sentenceFin.length)
      out(
        `| 12 en sentences | – | ${pct(rate(sNorm))} | ${pct(rate(sRaw))} | – | – | – | p50 ${ms0(percentile(sentenceFin, 50))}, p95 ${ms0(percentile(sentenceFin, 95))} |`,
      );
    out();
  }

  // Error examples (local review; pick a few for the report).
  out("#### Sentence errors (norm), per model");
  out();
  for (const l of loaded.filter((l) => !l.cpu)) {
    out(`- **${l.m.id}**`);
    for (const r of l.doc.results.filter((r) => r.id.endsWith("#0"))) {
      const c = byId.get(r.id.split("#")[0]);
      if (!c || c.kind !== "sentence") continue;
      const w = wordCounts(c.ref, r.text, c.lang, "norm");
      if (rate(w) === 0) continue;
      out(
        `  - ${c.id}${c.disfluent ? " (disfluent)" : ""}: ${errorSnippet(c.ref, r.text, c.lang)}`,
      );
    }
  }
  out();
  out("#### Seam examples");
  out();
  for (const e of seamExamples.slice(0, 60)) out(`- ${e}`);
  return lines.join("\n");
}

if (import.meta.main) {
  const { cmd, opts } = parseCli(process.argv.slice(2));
  if (cmd === "split") {
    const s = cmdSplit(opts);
    for (const n of s.notes) console.log(n);
    for (const sec of s.sections)
      console.log(
        sec.id,
        (sec.start / RATE).toFixed(2),
        (sec.end / RATE).toFixed(2),
        sec.utterances.length,
      );
    console.log("extras", s.extras.length);
  } else if (cmd === "bench") {
    cmdBench(opts);
  } else if (cmd === "chunk") {
    cmdChunk(opts);
  } else if (cmd === "report") {
    const md = cmdReport(opts);
    fs.writeFileSync(path.join(opts.out, "report.md"), md);
    console.log(md);
  } else {
    console.log(
      "usage: bun scripts/wb-bench-asr.ts split|bench|chunk|report [--cpu] [--models a,b] [options]",
    );
  }
}
