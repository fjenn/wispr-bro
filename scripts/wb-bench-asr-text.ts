// SPDX-License-Identifier: GPL-3.0-or-later
/**
 * RS-09 speech-model bench: text side (R5 §7 RS-09). Pure functions, no I/O
 * beyond reading the reference script:
 *
 * - the reference texts of `eval/asr/READING-SCRIPT.md`, by section;
 * - two normalisations for WER: **raw** (lowercase, punctuation stripped,
 *   nothing else) and **norm** (raw plus numbers, times, currency, percent
 *   and e-mail symbols spelled out in words, so "3:30", "€1,800" and "7 %"
 *   match "three thirty", "eighteen hundred euros" and "seven percent");
 * - word and character error rates with the alignment behind them;
 * - Silero-style speech segments from per-frame VAD probabilities (the same
 *   rules as Handy's `audio_toolkit/chunking.rs`).
 */
import fs from "fs";
import path from "path";

export type Lang = "en" | "nl" | "fr";

export type SectionId =
  | "en-sentences"
  | "nl-sentences"
  | "fr-sentences"
  | "en-long"
  | "nl-long"
  | "fr-long";

export const SECTIONS: SectionId[] = [
  "en-sentences",
  "nl-sentences",
  "fr-sentences",
  "en-long",
  "nl-long",
  "fr-long",
];

export function sectionLang(id: SectionId): Lang {
  return id.slice(0, 2) as Lang;
}

export function isLong(id: SectionId): boolean {
  return id.endsWith("-long");
}

export type Section = {
  id: SectionId;
  lang: Lang;
  /** Sentences in reading order (sections 1-3: the numbered lines; 4-6: the
   * paragraph text split at sentence ends). */
  sentences: string[];
  /** The whole text, sentences joined by spaces. */
  text: string;
};

/** Splits running text at `.`, `?`, `!` or `:` followed by a space and a
 * capital letter. */
export function splitSentences(text: string): string[] {
  return text
    .split(/(?<=[.?!])\s+(?=[A-ZÀ-Ý])/u)
    .map((s) => s.trim())
    .filter(Boolean);
}

/** Parses READING-SCRIPT.md: `## N. \`<id>.m4a\`` headings, numbered lines
 * for the sentence sections and `> ` quoted paragraphs for the long ones. */
export function parseReadingScript(md: string): Section[] {
  const out: Section[] = [];
  const parts = md.split(/^## \d+\. `([a-z]{2}-(?:sentences|long))\.m4a`.*$/m);
  for (let i = 1; i < parts.length; i += 2) {
    const id = parts[i] as SectionId;
    const body = parts[i + 1];
    let sentences: string[];
    if (isLong(id)) {
      const quoted = body
        .split("\n")
        .filter((l) => l.startsWith(">"))
        .map((l) => l.replace(/^>\s?/, ""));
      const paragraphs = quoted
        .join("\n")
        .split(/\n\s*\n/)
        .map((p) => p.replace(/\s+/g, " ").trim())
        .filter(Boolean);
      sentences = paragraphs.flatMap(splitSentences);
    } else {
      sentences = body
        .split("\n")
        .map((l) => l.match(/^\d+\.\s+(.*)$/)?.[1])
        .filter((s): s is string => !!s)
        .map((s) => s.trim());
    }
    out.push({
      id,
      lang: sectionLang(id),
      sentences,
      text: sentences.join(" "),
    });
  }
  return out;
}

export const READING_SCRIPT = path.resolve(
  import.meta.dir,
  "../eval/asr/READING-SCRIPT.md",
);

export function loadSections(file = READING_SCRIPT): Section[] {
  return parseReadingScript(fs.readFileSync(file, "utf8"));
}

// ---------------------------------------------------------------------------
// Numbers in words

const EN_ONES = [
  "zero",
  "one",
  "two",
  "three",
  "four",
  "five",
  "six",
  "seven",
  "eight",
  "nine",
  "ten",
  "eleven",
  "twelve",
  "thirteen",
  "fourteen",
  "fifteen",
  "sixteen",
  "seventeen",
  "eighteen",
  "nineteen",
];
const EN_TENS = [
  "",
  "",
  "twenty",
  "thirty",
  "forty",
  "fifty",
  "sixty",
  "seventy",
  "eighty",
  "ninety",
];

function en(n: number): string {
  if (n < 20) return EN_ONES[n];
  if (n < 100)
    return EN_TENS[Math.floor(n / 10)] + (n % 10 ? " " + EN_ONES[n % 10] : "");
  if (n < 1000)
    return (
      EN_ONES[Math.floor(n / 100)] +
      " hundred" +
      (n % 100 ? " " + en(n % 100) : "")
    );
  // 1100-9999 in whole hundreds are said "eighteen hundred".
  if (n < 10000 && n % 100 === 0 && n % 1000 !== 0)
    return en(n / 100) + " hundred";
  if (n < 1_000_000)
    return (
      en(Math.floor(n / 1000)) +
      " thousand" +
      (n % 1000 ? " " + en(n % 1000) : "")
    );
  return String(n);
}

const EN_ORD: Record<string, string> = {
  one: "first",
  two: "second",
  three: "third",
  five: "fifth",
  eight: "eighth",
  nine: "ninth",
  twelve: "twelfth",
};

function enOrdinal(n: number): string {
  const words = en(n).split(" ");
  const last = words.pop()!;
  const ord =
    EN_ORD[last] ??
    (last.endsWith("y") ? last.slice(0, -1) + "ieth" : last + "th");
  return [...words, ord].join(" ");
}

const NL_ONES = [
  "nul",
  "een",
  "twee",
  "drie",
  "vier",
  "vijf",
  "zes",
  "zeven",
  "acht",
  "negen",
  "tien",
  "elf",
  "twaalf",
  "dertien",
  "veertien",
  "vijftien",
  "zestien",
  "zeventien",
  "achttien",
  "negentien",
];
const NL_TENS = [
  "",
  "",
  "twintig",
  "dertig",
  "veertig",
  "vijftig",
  "zestig",
  "zeventig",
  "tachtig",
  "negentig",
];

/** Dutch writes numbers as one word ("vijfenveertig", "achttienhonderd"). */
function nl(n: number): string {
  if (n < 20) return NL_ONES[n];
  if (n < 100) {
    const o = n % 10;
    const t = NL_TENS[Math.floor(n / 10)];
    if (!o) return t;
    const one = NL_ONES[o];
    return one + (one.endsWith("e") ? "ën" : "en") + t;
  }
  if (n < 1000) {
    const h = Math.floor(n / 100);
    return (h > 1 ? NL_ONES[h] : "") + "honderd" + (n % 100 ? nl(n % 100) : "");
  }
  if (n < 10000 && n % 100 === 0 && n % 1000 !== 0)
    return nl(n / 100) + "honderd";
  if (n < 1_000_000) {
    const k = Math.floor(n / 1000);
    return (
      (k > 1 ? nl(k) : "") + "duizend" + (n % 1000 ? " " + nl(n % 1000) : "")
    );
  }
  return String(n);
}

const FR_ONES = [
  "zéro",
  "un",
  "deux",
  "trois",
  "quatre",
  "cinq",
  "six",
  "sept",
  "huit",
  "neuf",
  "dix",
  "onze",
  "douze",
  "treize",
  "quatorze",
  "quinze",
  "seize",
];
const FR_TENS = [
  "",
  "",
  "vingt",
  "trente",
  "quarante",
  "cinquante",
  "soixante",
];

/** French, words separated by spaces (hyphens are dropped by the
 * normaliser anyway). */
function fr(n: number): string {
  if (n <= 16) return FR_ONES[n];
  if (n < 20) return "dix " + FR_ONES[n - 10];
  if (n < 70) {
    const t = FR_TENS[Math.floor(n / 10)];
    const o = n % 10;
    if (!o) return t;
    return t + (o === 1 ? " et un" : " " + FR_ONES[o]);
  }
  if (n < 80) return "soixante" + (n === 71 ? " et " : " ") + fr(n - 60);
  if (n < 100) {
    if (n === 80) return "quatre vingts";
    return "quatre vingt " + fr(n - 80);
  }
  if (n < 1000) {
    const h = Math.floor(n / 100);
    const r = n % 100;
    const head = h === 1 ? "cent" : FR_ONES[h] + (r ? " cent" : " cents");
    return head + (r ? " " + fr(r) : "");
  }
  if (n < 1_000_000) {
    const k = Math.floor(n / 1000);
    const r = n % 1000;
    return (k === 1 ? "mille" : fr(k) + " mille") + (r ? " " + fr(r) : "");
  }
  return String(n);
}

export function numberWords(n: number, lang: Lang): string {
  return lang === "en" ? en(n) : lang === "nl" ? nl(n) : fr(n);
}

// ---------------------------------------------------------------------------
// Normalisation

/** Lowercase, apostrophes, hyphens and punctuation, nothing else. */
export function normaliseRaw(text: string, lang: Lang): string {
  let t = text
    .normalize("NFC")
    .toLowerCase()
    .replace(/[’‘`´]/g, "'");
  // English drops the apostrophe inside a word (i'm → im); French and Dutch
  // split there (d'encre → d encre, 's ochtends → s ochtends).
  t = lang === "en" ? t.replace(/(\p{L})'(\p{L})/gu, "$1$2") : t;
  t = t.replace(/[^\p{L}\p{N}\s]/gu, " ");
  return t.replace(/\s+/g, " ").trim();
}

const PERCENT: Record<Lang, string> = {
  en: "percent",
  nl: "procent",
  fr: "pour cent",
};
const EURO: Record<Lang, string> = { en: "euros", nl: "euro", fr: "euros" };

/** Raw plus symbols and numbers spelled out the way the script says them. */
export function normaliseNumbers(text: string, lang: Lang): string {
  let t = text
    .normalize("NFC")
    .toLowerCase()
    .replace(/[’‘`´]/g, "'");
  // Thousands separators: en "1,800"; nl/fr "1.800", "1 800" (also narrow
  // no-break spaces).
  t =
    lang === "en"
      ? t.replace(/(\d),(\d{3})\b/g, "$1$2")
      : t.replace(/(\d)[.   ](\d{3})\b/g, "$1$2");
  // E-mail addresses and dotted names (en only: the script spells one out).
  if (lang === "en") {
    t = t.replace(/\S+@\S+|\S+\.(?:com|org|net|nl|fr|be|io)\b/g, (addr) =>
      addr
        .replace(/[.,;:!?]+$/, "")
        .replace(/@/g, " at ")
        .replace(/\./g, " dot "),
    );
  }
  // Currency and percent.
  t = t.replace(/€\s?(\d+)/g, `$1 ${EURO[lang]}`);
  t = t.replace(/(\d+)\s?€/g, `$1 ${EURO[lang]}`);
  t = t.replace(/\b(\d+)\s?(euro|eur)\b/g, `$1 ${EURO[lang]}`);
  t = t.replace(/(\d+)\s?%/g, `$1 ${PERCENT[lang]}`);
  // Times.
  if (lang === "fr") {
    t = t.replace(/\b(\d{1,2})\s?h\s?(\d{2})\b/g, (_, h, m) =>
      `${h} ${h === "1" ? "heure" : "heures"} ${m === "00" ? "" : m}`.trim(),
    );
    t = t.replace(
      /\b(\d{1,2})\s?h\b/g,
      (_, h) => `${h} ${h === "1" ? "heure" : "heures"}`,
    );
  }
  t = t.replace(/\b(\d{1,2})[:.](\d{2})\b/g, (_, h, m) =>
    m === "00" ? h : `${h} ${m}`,
  );
  // English ordinals.
  if (lang === "en") {
    t = t.replace(/\b(\d+)(st|nd|rd|th)\b/g, (_, n) => enOrdinal(Number(n)));
  }
  // Any remaining digits.
  t = t.replace(/\d+/g, (d) => " " + numberWords(Number(d), lang) + " ");
  return normaliseRaw(t, lang);
}

export type Normaliser = "raw" | "norm";

export function normalise(text: string, lang: Lang, how: Normaliser): string {
  return how === "raw"
    ? normaliseRaw(text, lang)
    : normaliseNumbers(text, lang);
}

export function tokens(text: string, lang: Lang, how: Normaliser): string[] {
  const t = normalise(text, lang, how);
  return t ? t.split(" ") : [];
}

// ---------------------------------------------------------------------------
// Error rates

export type Op = "ok" | "sub" | "del" | "ins";
export type Step = {
  op: Op;
  ref?: string;
  hyp?: string;
  refIndex: number;
  hypIndex: number;
};

/** Levenshtein alignment of `ref` to `hyp`; ties prefer ok/sub, then del. */
export function align<T>(ref: T[], hyp: T[]): Step[] {
  const n = ref.length;
  const m = hyp.length;
  const d: Uint32Array[] = [];
  for (let i = 0; i <= n; i++) {
    d.push(new Uint32Array(m + 1));
    d[i][0] = i;
  }
  for (let j = 0; j <= m; j++) d[0][j] = j;
  for (let i = 1; i <= n; i++) {
    for (let j = 1; j <= m; j++) {
      const c = ref[i - 1] === hyp[j - 1] ? 0 : 1;
      d[i][j] = Math.min(d[i - 1][j - 1] + c, d[i - 1][j] + 1, d[i][j - 1] + 1);
    }
  }
  const steps: Step[] = [];
  let i = n;
  let j = m;
  while (i > 0 || j > 0) {
    if (i > 0 && j > 0) {
      const c = ref[i - 1] === hyp[j - 1] ? 0 : 1;
      if (d[i][j] === d[i - 1][j - 1] + c) {
        steps.push({
          op: c ? "sub" : "ok",
          ref: String(ref[i - 1]),
          hyp: String(hyp[j - 1]),
          refIndex: i - 1,
          hypIndex: j - 1,
        });
        i--;
        j--;
        continue;
      }
    }
    if (i > 0 && d[i][j] === d[i - 1][j] + 1) {
      steps.push({
        op: "del",
        ref: String(ref[i - 1]),
        refIndex: i - 1,
        hypIndex: j,
      });
      i--;
    } else {
      steps.push({
        op: "ins",
        hyp: String(hyp[j - 1]),
        refIndex: i,
        hypIndex: j - 1,
      });
      j--;
    }
  }
  return steps.reverse();
}

export type Counts = { ref: number; sub: number; del: number; ins: number };

export function counts(steps: Step[]): Counts {
  const c: Counts = { ref: 0, sub: 0, del: 0, ins: 0 };
  for (const s of steps) {
    if (s.op !== "ins") c.ref++;
    if (s.op === "sub") c.sub++;
    if (s.op === "del") c.del++;
    if (s.op === "ins") c.ins++;
  }
  return c;
}

export function addCounts(a: Counts, b: Counts): Counts {
  return {
    ref: a.ref + b.ref,
    sub: a.sub + b.sub,
    del: a.del + b.del,
    ins: a.ins + b.ins,
  };
}

export const ZERO: Counts = { ref: 0, sub: 0, del: 0, ins: 0 };

export function rate(c: Counts): number {
  return c.ref ? (c.sub + c.del + c.ins) / c.ref : 0;
}

export function wordCounts(
  ref: string,
  hyp: string,
  lang: Lang,
  how: Normaliser,
): Counts {
  return counts(align(tokens(ref, lang, how), tokens(hyp, lang, how)));
}

/** Characters of the normalised text, spaces removed. */
export function charCounts(ref: string, hyp: string, lang: Lang): Counts {
  const chars = (s: string) => [...normaliseNumbers(s, lang).replace(/ /g, "")];
  return counts(align(chars(ref), chars(hyp)));
}

// ---------------------------------------------------------------------------
// Seams of a chunked transcript

export type SeamReport = {
  seams: number;
  /** Reference words deleted within `window` reference words of a seam. */
  lost: number;
  /** Inserted words within the window that repeat a neighbouring word. */
  doubled: number;
  /** All errors (sub+del+ins) within the window. */
  errorsNearSeams: number;
  examples: string[];
};

/**
 * Where chunk texts meet in the joined hypothesis, which reference words
 * were lost and which words were doubled there. `chunkTokenCounts` are the
 * normalised token counts of each chunk's text, in order.
 */
export function seamReport(
  refTokens: string[],
  chunkTokens: string[][],
  window = 2,
): SeamReport {
  const hyp = chunkTokens.flat();
  const steps = align(refTokens, hyp);
  const seamsAt: number[] = [];
  let acc = 0;
  for (const c of chunkTokens.slice(0, -1)) {
    acc += c.length;
    seamsAt.push(acc);
  }
  const report: SeamReport = {
    seams: seamsAt.length,
    lost: 0,
    doubled: 0,
    errorsNearSeams: 0,
    examples: [],
  };
  for (const at of seamsAt) {
    // The step index where hypothesis token `at` starts.
    let k = steps.findIndex((s) => s.op !== "del" && s.hypIndex >= at);
    if (k < 0) k = steps.length;
    // Back up over deletions that sit right at the seam.
    while (k > 0 && steps[k - 1].op === "del") k--;
    const lo = Math.max(0, k - window - 2);
    const hi = Math.min(steps.length, k + window + 2);
    const near = steps.slice(lo, hi);
    let errs = 0;
    for (let x = lo; x < hi; x++) {
      const s = steps[x];
      if (s.op === "ok") continue;
      errs++;
      if (s.op === "del") report.lost++;
      if (s.op === "ins") {
        const neighbours = [
          hyp[s.hypIndex - 1],
          hyp[s.hypIndex + 1],
          hyp[s.hypIndex - 2],
          hyp[s.hypIndex + 2],
        ];
        if (neighbours.includes(s.hyp!)) report.doubled++;
      }
    }
    report.errorsNearSeams += errs;
    if (errs && report.examples.length < 6) {
      report.examples.push(
        near
          .map((s) =>
            s.op === "ok"
              ? s.ref
              : s.op === "sub"
                ? `[${s.ref}→${s.hyp}]`
                : s.op === "del"
                  ? `[-${s.ref}]`
                  : `[+${s.hyp}]`,
          )
          .join(" ") +
          " | seam before hyp word " +
          at,
      );
    }
  }
  return report;
}

// ---------------------------------------------------------------------------
// VAD segments (Handy's chunking.rs `speech_segments`, Silero's defaults)

const RATE = 16000;
const THRESHOLD = 0.5;
const NEG_THRESHOLD = THRESHOLD - 0.15;
const MIN_SPEECH = (250 * RATE) / 1000;
const SPEECH_PAD = (30 * RATE) / 1000;

export type Range = { start: number; end: number };

export function speechSegments(
  probs: number[],
  frameSamples: number,
  samples: number,
  minSilenceMs = 100,
): Range[] {
  const minSilence = (minSilenceMs * RATE) / 1000;
  const segs: Range[] = [];
  let triggered = false;
  let start = 0;
  let tempEnd: number | null = null;
  probs.forEach((p, i) => {
    const at = i * frameSamples;
    if (p >= THRESHOLD) {
      tempEnd = null;
      if (!triggered) {
        triggered = true;
        start = at;
      }
      return;
    }
    if (p < NEG_THRESHOLD && triggered) {
      if (tempEnd === null) tempEnd = at;
      if (at - tempEnd < minSilence) return;
      if (tempEnd - start > MIN_SPEECH) segs.push({ start, end: tempEnd });
      triggered = false;
      tempEnd = null;
    }
  });
  if (triggered && samples - start > MIN_SPEECH)
    segs.push({ start, end: samples });
  for (let i = 0; i < segs.length; i++) {
    if (i === 0) segs[0].start = Math.max(0, segs[0].start - SPEECH_PAD);
    if (i + 1 === segs.length) {
      segs[i].end = Math.min(samples, segs[i].end + SPEECH_PAD);
      continue;
    }
    const silence = segs[i + 1].start - segs[i].end;
    const pad = silence < 2 * SPEECH_PAD ? Math.floor(silence / 2) : SPEECH_PAD;
    segs[i].end = Math.min(samples, segs[i].end + pad);
    segs[i + 1].start = Math.max(0, segs[i + 1].start - pad);
  }
  return segs;
}

/** Joins segments separated by less than `gapMs` of silence. */
export function mergeSegments(segs: Range[], gapMs: number): Range[] {
  const gap = (gapMs * RATE) / 1000;
  const out: Range[] = [];
  for (const s of segs) {
    const last = out[out.length - 1];
    if (last && s.start - last.end < gap) last.end = s.end;
    else out.push({ ...s });
  }
  return out;
}

export function percentile(values: number[], p: number): number | null {
  if (!values.length) return null;
  const sorted = [...values].sort((a, b) => a - b);
  const rank = Math.max(1, Math.ceil((p / 100) * sorted.length));
  return sorted[rank - 1];
}
