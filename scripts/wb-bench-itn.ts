// SPDX-License-Identifier: GPL-3.0-or-later
/**
 * RS-13 ITN need (R5 §7 RS-13, RT-39, DEC-40). A bench harness, not product
 * code. It synthesises the number-heavy dictations of `eval/itn/corpus.json`
 * with macOS `say`, runs them through each language's default speech models
 * with the RS-09 bench binary (`src-tauri/crates/wb-bench-asr`, transcribe-cpp
 * the way the app's engine worker does), and scores every number expression
 * as written (digits), spoken (number words) or other.
 *
 *   bun scripts/wb-bench-itn.ts synth    # say → 16 kHz WAV per language + jobs
 *   bun scripts/wb-bench-itn.ts run      # every default model × its languages
 *   bun scripts/wb-bench-itn.ts report   # tables (markdown) to stdout + JSON
 *   bun scripts/wb-bench-itn.ts owner    # counts on the owner's RS-09 takes
 *
 * Options: --bin <wb-bench-asr>, --asr <model dir>, --out <results dir>.
 * Audio and raw results stay under ~/wb-bench/results/rs13 (never committed).
 */
import { spawnSync } from "child_process";
import fs from "fs";
import os from "os";
import path from "path";

const HOME = os.homedir();
const RATE = 16000;
const REPO_ROOT = path.resolve(import.meta.dir, "..");
const CORPUS = path.join(REPO_ROOT, "eval", "itn", "corpus.json");

export type Lang = "en" | "fr" | "nl";
export type Class = "written" | "spoken" | "other";

export type Expr = {
  words: string;
  cat: string;
  digits: string;
};

export type Item = { id: string; lang: Lang; say: string };

type Corpus = {
  voices: Record<Lang, string[]>;
  items: Item[];
  owner: { items: Record<string, string> };
};

/** The DEC-52 models per language: starter (if any) and the default. */
export const MODELS: {
  id: string;
  file: string;
  langs: Lang[];
  role: Partial<Record<Lang, string>>;
  stream?: boolean;
}[] = [
  {
    id: "parakeet-tdt_ctc-110m",
    file: "parakeet-tdt_ctc-110m/parakeet-tdt_ctc-110m-Q8_0.gguf",
    langs: ["en"],
    role: { en: "starter" },
  },
  {
    id: "parakeet-unified-en-0.6b",
    file: "parakeet-unified-en-0.6b/parakeet-unified-en-0.6b-Q8_0.gguf",
    langs: ["en"],
    role: { en: "default (streams)" },
    stream: true,
  },
  {
    id: "whisper-base",
    file: "whisper-base/whisper-base-Q8_0.gguf",
    langs: ["fr"],
    role: { fr: "stopgap starter" },
  },
  {
    id: "parakeet-tdt-0.6b-v3",
    file: "parakeet-tdt-0.6b-v3/parakeet-tdt-0.6b-v3-Q8_0.gguf",
    langs: ["nl", "fr"],
    role: { nl: "first download", fr: "reference" },
  },
  {
    id: "whisper-large-v3-turbo",
    file: "whisper-large-v3-turbo/whisper-large-v3-turbo-Q8_0.gguf",
    langs: ["en", "fr", "nl"],
    role: { en: "reference", fr: "default", nl: "default (upgrade)" },
  },
];

// ---------------------------------------------------------------------------
// Corpus

const MARK = /\[([^\]|]+)\|([a-z]+)\|([0-9]+)\]/g;

/** The text `say` speaks: markup removed. */
export function spokenText(say: string): string {
  return say.replace(MARK, (_m, words: string) => words);
}

/** The marked number expressions, in order. */
export function expressions(say: string): Expr[] {
  return [...say.matchAll(MARK)].map((m) => ({
    words: m[1],
    cat: m[2],
    digits: m[3],
  }));
}

// ---------------------------------------------------------------------------
// Scoring

/** Lowercase letters only (accents folded), so "tweeënveertig",
 * "twee en veertig" and "forty-two" compare without spaces or hyphens. */
export function letters(s: string): string {
  return s
    .normalize("NFD")
    .replace(/\p{M}/gu, "")
    .toLowerCase()
    .replace(/[^a-z]/g, "");
}

/** Drop the joining words people and models add or leave out ("one hundred
 * and twenty", "vingt et un", "tweeduizend en vijf"), as separate tokens. */
function dropJoiners(s: string, lang: Lang): string {
  const joiner = { en: "and", fr: "et", nl: "en" }[lang];
  return s
    .split(/[\s,.;:!?-]+/)
    .filter((t) => t.toLowerCase() !== joiner)
    .join(" ");
}

/**
 * Classify each expression of one dictation against the transcript.
 * Written: its digits appear, in order, in the transcript's digit stream
 * (separators ignored: "$1,320", "3:30", "06 12 34" all count). Spoken: its
 * number words appear in the transcript's letter stream. Expressions are
 * matched left to right so two in one sentence cannot share a match.
 */
export function classify(hyp: string, exprs: Expr[], lang: Lang): Class[] {
  const digitStream = hyp.replace(/[^0-9]/g, "");
  // Two letter streams: as said, and with joiners dropped on both sides.
  const streams = [letters(hyp), letters(dropJoiners(hyp, lang))];
  const at = [0, 0];
  let dAt = 0;
  return exprs.map((e) => {
    const d = digitStream.indexOf(e.digits, dAt);
    if (d >= 0) {
      dAt = d + e.digits.length;
      return "written";
    }
    const words = [letters(e.words), letters(dropJoiners(e.words, lang))];
    for (const k of [0, 1]) {
      const l = streams[k].indexOf(words[k], at[k]);
      if (l >= 0) {
        at[k] = l + words[k].length;
        return "spoken";
      }
    }
    return "other";
  });
}

// ---------------------------------------------------------------------------
// I/O helpers

function opt(name: string, fallback: string): string {
  const i = process.argv.indexOf(`--${name}`);
  return i >= 0 && process.argv[i + 1] ? process.argv[i + 1] : fallback;
}

function loadCorpus(): Corpus {
  return JSON.parse(fs.readFileSync(CORPUS, "utf8")) as Corpus;
}

/** 16-bit PCM samples of a mono WAV (the `afconvert` output). */
function readPcm16(file: string): Int16Array {
  const b = fs.readFileSync(file);
  let at = 12;
  while (at + 8 <= b.length) {
    const id = b.toString("ascii", at, at + 4);
    const size = b.readUInt32LE(at + 4);
    if (id === "data") {
      const data = b.subarray(at + 8, at + 8 + size);
      return new Int16Array(
        data.buffer.slice(data.byteOffset, data.byteOffset + data.length),
      );
    }
    at += 8 + size + (size % 2);
  }
  throw new Error(`no data chunk in ${file}`);
}

function writePcm16(file: string, pcm: Int16Array) {
  const h = Buffer.alloc(44);
  h.write("RIFF", 0);
  h.writeUInt32LE(36 + pcm.length * 2, 4);
  h.write("WAVEfmt ", 8);
  h.writeUInt32LE(16, 16);
  h.writeUInt16LE(1, 20);
  h.writeUInt16LE(1, 22);
  h.writeUInt32LE(RATE, 24);
  h.writeUInt32LE(RATE * 2, 28);
  h.writeUInt16LE(2, 32);
  h.writeUInt16LE(16, 34);
  h.write("data", 36);
  h.writeUInt32LE(pcm.length * 2, 40);
  fs.writeFileSync(file, Buffer.concat([h, Buffer.from(pcm.buffer)]));
}

function run(cmd: string, args: string[]) {
  const r = spawnSync(cmd, args, { stdio: ["ignore", "inherit", "inherit"] });
  if (r.status !== 0) throw new Error(`${cmd} ${args.join(" ")} failed`);
}

type Job = {
  id: string;
  item: string;
  voice: string;
  start: number;
  end: number;
  language: Lang;
};

// ---------------------------------------------------------------------------
// Commands

function synth(out: string) {
  const corpus = loadCorpus();
  const tmp = path.join(out, "tts");
  fs.mkdirSync(tmp, { recursive: true });
  const gap = new Int16Array(RATE / 2); // 0.5 s of silence around each take
  for (const lang of ["en", "fr", "nl"] as Lang[]) {
    const parts: Int16Array[] = [];
    const jobs: Job[] = [];
    let at = 0;
    for (const [vi, voice] of corpus.voices[lang].entries()) {
      for (const item of corpus.items.filter((i) => i.lang === lang)) {
        const base = path.join(tmp, `${item.id}-v${vi}`);
        if (!fs.existsSync(`${base}.wav`)) {
          run("say", ["-v", voice, "-o", `${base}.aiff`, spokenText(item.say)]);
          run("afconvert", [
            "-f",
            "WAVE",
            "-d",
            "LEI16@16000",
            "-c",
            "1",
            `${base}.aiff`,
            `${base}.wav`,
          ]);
          fs.rmSync(`${base}.aiff`);
        }
        const pcm = readPcm16(`${base}.wav`);
        parts.push(gap, pcm, gap);
        jobs.push({
          id: `${item.id}-v${vi}`,
          item: item.id,
          voice,
          start: at,
          end: at + gap.length * 2 + pcm.length,
          language: lang,
        });
        at += gap.length * 2 + pcm.length;
      }
    }
    const all = new Int16Array(at);
    let o = 0;
    for (const p of parts) {
      all.set(p, o);
      o += p.length;
    }
    writePcm16(path.join(out, `${lang}.wav`), all);
    fs.writeFileSync(
      path.join(out, `${lang}-jobs.json`),
      JSON.stringify({ jobs }, null, 2),
    );
    console.log(`${lang}: ${jobs.length} takes, ${(at / RATE).toFixed(0)} s`);
  }
}

function runModels(out: string, bin: string, asr: string) {
  for (const m of MODELS) {
    for (const lang of m.langs) {
      const wav = path.join(out, `${lang}.wav`);
      const jobsFile = path.join(out, `${lang}-jobs.json`);
      const dest = path.join(out, `${m.id}-${lang}.json`);
      if (!fs.existsSync(dest)) {
        console.log(`run ${m.id} ${lang}`);
        run(bin, [
          "run",
          "--model",
          path.join(asr, m.file),
          "--wav",
          wav,
          "--jobs",
          jobsFile,
          "--out",
          dest,
        ]);
      }
      if (!m.stream) continue;
      // The app streams this model (Handy's `LiveDecode::Stream`): one
      // stream per take, the text the user gets is `text().full`.
      const sdest = path.join(out, `${m.id}-stream-${lang}.json`);
      if (fs.existsSync(sdest)) continue;
      const { jobs } = JSON.parse(fs.readFileSync(jobsFile, "utf8")) as {
        jobs: Job[];
      };
      const results: { id: string; text: string }[] = [];
      const one = path.join(out, "stream-one.json");
      for (const j of jobs) {
        run(bin, [
          "stream",
          "--model",
          path.join(asr, m.file),
          "--wav",
          wav,
          "--start",
          String(j.start),
          "--end",
          String(j.end),
          "--language",
          lang,
          "--out",
          one,
        ]);
        const r = JSON.parse(fs.readFileSync(one, "utf8")) as {
          text: string;
        };
        results.push({ id: j.id, text: r.text });
      }
      fs.rmSync(one);
      fs.writeFileSync(sdest, JSON.stringify({ results }, null, 2));
    }
  }
}

type Tally = Record<Class, number>;
const zero = (): Tally => ({ written: 0, spoken: 0, other: 0 });
const pct = (n: number, d: number) =>
  d ? `${((100 * n) / d).toFixed(1)} %` : "-";

function report(out: string) {
  const corpus = loadCorpus();
  const byId = new Map(corpus.items.map((i) => [i.id, i]));
  const rows: string[] = [];
  const cats: Record<string, Record<string, Tally>> = {};
  const detail: unknown[] = [];
  rows.push(
    "| Language | Model | Role | Expressions | Written | Spoken | Other | Spoken rate | ≥ 20 %? |",
    "| --- | --- | --- | --: | --: | --: | --: | --: | --- |",
  );
  const variants = MODELS.flatMap((m) =>
    m.langs.flatMap((lang) => {
      const v = [{ m, lang, file: `${m.id}-${lang}.json`, label: m.id }];
      if (m.stream)
        v.push({
          m,
          lang,
          file: `${m.id}-stream-${lang}.json`,
          label: `${m.id} (stream)`,
        });
      return v;
    }),
  );
  for (const lang of ["en", "fr", "nl"] as Lang[]) {
    for (const v of variants.filter((x) => x.lang === lang)) {
      const f = path.join(out, v.file);
      if (!fs.existsSync(f)) continue;
      const doc = JSON.parse(fs.readFileSync(f, "utf8")) as {
        results: { id: string; text: string }[];
      };
      const t = zero();
      for (const r of doc.results) {
        const item = byId.get(r.id.replace(/-v\d+$/, ""));
        if (!item) continue;
        const exprs = expressions(item.say);
        const cls = classify(r.text, exprs, lang);
        cls.forEach((c, i) => {
          t[c]++;
          const key = `${lang} ${v.label}`;
          cats[exprs[i].cat] ??= {};
          cats[exprs[i].cat][key] ??= zero();
          cats[exprs[i].cat][key][c]++;
        });
        detail.push({
          model: v.label,
          lang,
          id: r.id,
          text: r.text,
          classes: cls,
        });
      }
      const n = t.written + t.spoken + t.other;
      rows.push(
        `| ${lang} | ${v.label} | ${v.m.role[lang]} | ${n} | ${t.written} | ${t.spoken} | ${t.other} | ${pct(t.spoken, n)} | ${t.spoken / n >= 0.2 ? "yes" : "no"} |`,
      );
    }
  }
  const keys = [...new Set(Object.values(cats).flatMap((c) => Object.keys(c)))];
  const catRows = [
    `| Category | ${keys.join(" | ")} |`,
    `| --- | ${keys.map(() => "--:").join(" | ")} |`,
    ...Object.entries(cats).map(
      ([cat, byKey]) =>
        `| ${cat} | ${keys
          .map((k) => {
            const t = byKey[k] ?? zero();
            return `${t.spoken}/${t.written + t.spoken + t.other}`;
          })
          .join(" | ")} |`,
    ),
  ];
  fs.writeFileSync(
    path.join(out, "detail.json"),
    JSON.stringify(detail, null, 2),
  );
  console.log(rows.join("\n"));
  console.log("\nSpoken-form expressions / total, per category:\n");
  console.log(catRows.join("\n"));
}

/**
 * The same scoring on the owner's own RS-09 sentence takes (real speech,
 * read from the reading script), using the RS-09 Metal runs (`#0`). Prints
 * counts only: the owner's transcripts are personal data and stay local.
 */
function owner(rs09: string) {
  const corpus = loadCorpus();
  const rows = [
    "| Language | Model | Expressions | Written | Spoken | Other | Spoken rate |",
    "| --- | --- | --: | --: | --: | --: | --: |",
  ];
  for (const lang of ["en", "fr", "nl"] as Lang[]) {
    for (const m of MODELS.filter((x) => x.langs.includes(lang))) {
      const f = path.join(rs09, "bench", `${m.id}.json`);
      if (!fs.existsSync(f)) continue;
      const doc = JSON.parse(fs.readFileSync(f, "utf8")) as {
        results: { id: string; text: string }[];
      };
      const t = zero();
      for (const [id, say] of Object.entries(corpus.owner.items)) {
        if (!id.startsWith(lang)) continue;
        const r = doc.results.find((x) => x.id === `${id}#0`);
        if (!r) continue;
        for (const c of classify(r.text, expressions(say), lang)) t[c]++;
      }
      const n = t.written + t.spoken + t.other;
      rows.push(
        `| ${lang} | ${m.id} | ${n} | ${t.written} | ${t.spoken} | ${t.other} | ${pct(t.spoken, n)} |`,
      );
    }
  }
  console.log(rows.join("\n"));
}

if (import.meta.main) {
  const cmd = process.argv[2];
  const out = opt(
    "out",
    path.join(HOME, "wb-bench", "results", "rs13", "latest"),
  );
  fs.mkdirSync(out, { recursive: true });
  const bin = opt(
    "bin",
    path.join(REPO_ROOT, "src-tauri", "target", "release", "wb-bench-asr"),
  );
  const asr = opt("asr", path.join(HOME, "wb-bench", "asr"));
  if (cmd === "synth") synth(out);
  else if (cmd === "run") runModels(out, bin, asr);
  else if (cmd === "report") report(out);
  else if (cmd === "owner")
    owner(opt("rs09", path.join(HOME, "wb-bench", "results", "rs09")));
  else {
    console.error("usage: bun scripts/wb-bench-itn.ts synth|run|report|owner");
    process.exit(1);
  }
}
