// SPDX-License-Identifier: GPL-3.0-or-later
// Run: bun test scripts/wb-bench-asr.test.ts  (also part of `bun run wb:test:scripts`)
// RS-09 bench: reference parsing, normalisation (numbers in en/nl/fr), WER
// alignment, seam accounting, VAD segments, the split's section and
// sentence placement, the DEC-51 eager plan and key-up simulation. No model
// or audio is touched here.
import { describe, expect, test } from "bun:test";
import {
  align,
  charCounts,
  counts,
  loadSections,
  mergeSegments,
  normaliseNumbers,
  normaliseRaw,
  numberWords,
  parseReadingScript,
  rate,
  seamReport,
  speechSegments,
  splitSentences,
  tokens,
  wordCounts,
  percentile,
} from "./wb-bench-asr-text";
import {
  MODELS,
  assignSections,
  containment,
  eagerPlan,
  errorSnippet,
  keyUpLatency,
  parseCli,
  placeWords,
  silenceCut,
  type Utterance,
} from "./wb-bench-asr";

describe("reading script", () => {
  const sections = loadSections();
  test("six sections in script order with 12 sentences in each short one", () => {
    expect(sections.map((s) => s.id)).toEqual([
      "en-sentences",
      "nl-sentences",
      "fr-sentences",
      "en-long",
      "nl-long",
      "fr-long",
    ]);
    for (const s of sections.slice(0, 3)) expect(s.sentences).toHaveLength(12);
    for (const s of sections.slice(3))
      expect(s.sentences.length).toBeGreaterThan(5);
    expect(sections[0].sentences[0]).toBe(
      "Hey Sarah, can we move tomorrow's meeting to three thirty?",
    );
    expect(sections[3].sentences[1]).toBe("First, the launch date.");
  });
  test("long text keeps every word of the quoted paragraphs", () => {
    const md =
      "## 4. `en-long.m4a` (x)\n\n> One two. Three\n> four.\n>\n> Five six.\n";
    const [s] = parseReadingScript(md);
    expect(s.sentences).toEqual(["One two.", "Three four.", "Five six."]);
  });
  test("sentence split needs a capital after the stop", () => {
    expect(splitSentences("At 3 p.m. we met. Then we left.")).toEqual([
      "At 3 p.m. we met.",
      "Then we left.",
    ]);
  });
});

describe("numbers in words", () => {
  test("English", () => {
    expect(numberWords(45, "en")).toBe("forty five");
    expect(numberWords(1800, "en")).toBe("eighteen hundred");
    expect(numberWords(2000, "en")).toBe("two thousand");
    expect(numberWords(115, "en")).toBe("one hundred fifteen");
  });
  test("Dutch writes one word", () => {
    expect(numberWords(45, "nl")).toBe("vijfenveertig");
    expect(numberWords(22, "nl")).toBe("tweeëntwintig");
    expect(numberWords(1800, "nl")).toBe("achttienhonderd");
    expect(numberWords(100, "nl")).toBe("honderd");
  });
  test("French", () => {
    expect(numberWords(21, "fr")).toBe("vingt et un");
    expect(numberWords(71, "fr")).toBe("soixante et onze");
    expect(numberWords(80, "fr")).toBe("quatre vingts");
    expect(numberWords(99, "fr")).toBe("quatre vingt dix neuf");
    expect(numberWords(1800, "fr")).toBe("mille huit cents");
    expect(numberWords(17, "fr")).toBe("dix sept");
  });
});

describe("normalisation", () => {
  test("raw only lowercases and strips punctuation", () => {
    expect(normaliseRaw("I'm at 3:30, OK?", "en")).toBe("im at 3 30 ok");
    expect(normaliseRaw("Est-ce qu'on peut ?", "fr")).toBe("est ce qu on peut");
    expect(normaliseRaw("'s ochtends", "nl")).toBe("s ochtends");
  });
  test("norm spells out times, ordinals, currency, percent and e-mail", () => {
    expect(normaliseNumbers("to 3:30?", "en")).toBe("to three thirty");
    expect(normaliseNumbers("the 14th at 7:45", "en")).toBe(
      "the fourteenth at seven forty five",
    );
    expect(normaliseNumbers("about €1,800 incl.", "en")).toBe(
      "about eighteen hundred euros incl",
    );
    expect(normaliseNumbers("by 7%", "en")).toBe("by seven percent");
    expect(normaliseNumbers("is anna.devries@example.com.", "en")).toBe(
      "is anna dot devries at example dot com",
    );
    expect(normaliseNumbers("om 10.00 uur, 7%", "nl")).toBe(
      "om tien uur zeven procent",
    );
    expect(normaliseNumbers("à 15h30, 7 %", "fr")).toBe(
      "à quinze heures trente sept pour cent",
    );
    expect(normaliseNumbers("1 800 euros", "fr")).toBe(
      "mille huit cents euros",
    );
  });
  test("the script's own words are unchanged by norm", () => {
    for (const s of loadSections())
      expect(normaliseNumbers(s.text, s.lang)).toBe(
        normaliseRaw(s.text, s.lang),
      );
  });
  test("digits and words score the same after norm, not raw", () => {
    const ref =
      "The flight leaves on the fourteenth of March at seven forty five.";
    const hyp = "The flight leaves on the 14th of March at 7:45.";
    expect(rate(wordCounts(ref, hyp, "en", "norm"))).toBe(0);
    expect(rate(wordCounts(ref, hyp, "en", "raw"))).toBeGreaterThan(0.2);
  });
});

describe("error rates", () => {
  test("alignment counts substitutions, deletions and insertions", () => {
    const c = counts(align("a b c d".split(" "), "a x c d e".split(" ")));
    expect(c).toEqual({ ref: 4, sub: 1, del: 0, ins: 1 });
    expect(rate(c)).toBe(0.5);
    expect(counts(align(["a", "b"], []))).toEqual({
      ref: 2,
      sub: 0,
      del: 2,
      ins: 0,
    });
  });
  test("CER ignores spaces", () => {
    expect(rate(charCounts("pull request", "pullrequest", "en"))).toBe(0);
  });
  test("tokens follow the normaliser", () => {
    expect(tokens("Hi, 2 you", "en", "norm")).toEqual(["hi", "two", "you"]);
    expect(tokens("", "en", "raw")).toEqual([]);
  });
  test("percentile is nearest-rank", () => {
    expect(percentile([5, 1, 3, 2, 4], 50)).toBe(3);
    expect(percentile([1, 2, 3, 4, 5, 6, 7, 8, 9, 10], 95)).toBe(10);
    expect(percentile([], 50)).toBeNull();
  });
});

describe("seams", () => {
  const ref = "one two three four five six".split(" ");
  test("a clean join has no seam errors", () => {
    const r = seamReport(ref, [
      ["one", "two", "three"],
      ["four", "five", "six"],
    ]);
    expect(r).toMatchObject({
      seams: 1,
      lost: 0,
      doubled: 0,
      errorsNearSeams: 0,
    });
  });
  test("a word lost at the seam", () => {
    const r = seamReport(ref, [
      ["one", "two"],
      ["four", "five", "six"],
    ]);
    expect(r.lost).toBe(1);
  });
  test("a word doubled at the seam", () => {
    const r = seamReport(ref, [
      ["one", "two", "three"],
      ["three", "four", "five", "six"],
    ]);
    expect(r.doubled).toBe(1);
    expect(r.lost).toBe(0);
  });
  test("errors far from the seam do not count", () => {
    const r = seamReport(ref, [
      ["one", "two", "three", "four", "five"],
      ["six"],
    ]);
    expect(r.errorsNearSeams).toBe(0);
    const far = seamReport("a b c d e f g h i j".split(" "), [
      ["x", "b", "c", "d", "e"],
      ["f", "g", "h", "i", "j"],
    ]);
    expect(far.errorsNearSeams).toBe(0);
  });
});

describe("VAD segments", () => {
  const frame = 512;
  const probs = (pattern: number[]) =>
    pattern.flatMap((p) => Array(16).fill(p)); // 16 frames ≈ 0.51 s
  test("speech, a half-second pause, speech", () => {
    const p = probs([0, 1, 1, 0, 1, 1, 0]);
    const segs = speechSegments(p, frame, p.length * frame);
    expect(segs).toHaveLength(2);
    expect(segs[0].end).toBeLessThan(segs[1].start);
  });
  test("short gaps merge", () => {
    expect(
      mergeSegments(
        [
          { start: 0, end: 16000 },
          { start: 18000, end: 32000 },
          { start: 60000, end: 70000 },
        ],
        300,
      ),
    ).toEqual([
      { start: 0, end: 32000 },
      { start: 60000, end: 70000 },
    ]);
  });
  test("cut in the longest silence of a gap", () => {
    const speech = [
      { start: 0, end: 100 },
      { start: 150, end: 200 },
      { start: 400, end: 500 },
    ];
    expect(silenceCut(speech, 90, 410)).toBe(300);
    expect(silenceCut([], 10, 20)).toBe(15);
  });
});

describe("split placement", () => {
  const sections = loadSections();
  const utt = (text: string, i: number): Utterance => ({
    start: i * 10,
    end: i * 10 + 5,
    text,
    lang: null,
  });
  test("containment is the share of words found", () => {
    expect(containment(["a", "b"], ["a", "c"])).toBe(0.5);
    expect(containment([], ["a"])).toBe(0);
  });
  test("sections are contiguous and asides are left out", () => {
    const utts = [
      utt("Hey Sarah, can we move tomorrow's meeting to 3:30?", 0),
      utt("Hard riches.", 1),
      utt("Please order 12 boxes of paper and 2 black ink cartridges.", 2),
      utt(
        "Hoi Thomas, kunnen we de afspraak van morgen verzetten naar half vier?",
        3,
      ),
      utt(
        "Wil je twaalf dozen papier en twee zwarte inktpatronen bestellen?",
        4,
      ),
    ];
    expect(assignSections(utts, sections)).toEqual([
      "en-sentences",
      null,
      "en-sentences",
      "nl-sentences",
      "nl-sentences",
    ]);
  });
  test("words go to sentences in order; a restart is not placed", () => {
    // Two sentences only, so the hypothesis covers the whole reference.
    const two = sections[0].sentences.slice(0, 2);
    const sec = { ...sections[0], sentences: two, text: two.join(" ") };
    const words = [
      "Hey",
      "Sarah",
      "can",
      "we",
      "move",
      "tomorrow's",
      "meeting",
      "to",
      "3:30",
      "I",
      "just",
      "sent",
      "Hey",
      "the",
      "invoice",
      "to",
      "accounting",
      "it",
      "should",
      "arrive",
      "before",
      "Friday",
    ].map((text, i) => ({ utt: 0, start: i * 100, end: i * 100 + 50, text }));
    const place = placeWords(words, sec);
    expect(place.slice(0, 9).every((k) => k === 0)).toBe(true);
    expect(place[9]).toBe(1);
    expect(place[12]).toBe(-1);
    expect(place[14]).toBe(1);
  });
});

describe("DEC-51 plan and key-up", () => {
  test("eager plan cuts at long pauses once the chunk is long enough", () => {
    // 1 frame = 512 samples; build 6 s speech, 0.6 s pause, 6 s speech,
    // 0.2 s pause, 3 s speech.
    const f = (s: number) => Math.round((s * 16000) / 512);
    const p = [
      ...Array(f(6)).fill(1),
      ...Array(f(0.6)).fill(0),
      ...Array(f(6)).fill(1),
      ...Array(f(0.2)).fill(0),
      ...Array(f(3)).fill(1),
    ];
    const plan = eagerPlan({
      start: 0,
      end: p.length * 512,
      frame_samples: 512,
      probs: p,
    });
    expect(plan).toHaveLength(2);
    expect(plan[0].end / 16000).toBeCloseTo(6.3, 0);
    expect(plan[0].readyAt).toBeLessThan(plan[1].start + 16000);
    expect(plan[1].end).toBe(p.length * 512);
  });
  test("key-up waits for the backlog and the last chunk", () => {
    // Chunks ready at 10 s and 20 s (end), 1 s and 0.5 s to decode.
    expect(keyUpLatency([160000, 320000], [1000, 500], 320000)).toBe(500);
    // A slow engine still busy with chunk 1 at key-up.
    expect(keyUpLatency([160000, 320000], [12000, 500], 320000)).toBe(2500);
  });
});

describe("report helpers and CLI", () => {
  test("error snippet marks only the errors and their context", () => {
    expect(errorSnippet("a b c d e f g h", "a b c x e f g h", "en", 1)).toBe(
      "c **d→x** e",
    );
  });
  test("defaults point outside the repo for audio and results", () => {
    const { cmd, opts } = parseCli([
      "bench",
      "--models",
      "whisper-base",
      "--cpu",
    ]);
    expect(cmd).toBe("bench");
    expect(opts.models).toEqual(["whisper-base"]);
    expect(opts.cpu).toBe(true);
    expect(opts.out).toContain("wb-bench/results");
    expect(opts.wav).toContain("wb-bench/audio");
  });
  test("candidates match R5 RS-09 and the catalog's languages", () => {
    expect(MODELS.map((m) => m.id).sort()).toEqual(
      [
        "moonshine-base",
        "parakeet-tdt-0.6b-v3",
        "parakeet-tdt_ctc-110m",
        "parakeet-unified-en-0.6b",
        "whisper-base",
        "whisper-large-v3-turbo",
      ].sort(),
    );
    for (const m of MODELS) expect(m.ccby).toBe(m.license === "cc-by-4.0");
    expect(MODELS.filter((m) => m.streaming).map((m) => m.id)).toEqual([
      "parakeet-unified-en-0.6b",
    ]);
  });
});
