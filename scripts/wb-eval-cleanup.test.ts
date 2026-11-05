// SPDX-License-Identifier: GPL-3.0-or-later
// Run: bun test scripts/wb-eval-cleanup.test.ts  (also part of `bun run wb:test:scripts`)
// F-10: metric functions on hand-made items with known answers, the guard
// stub, the HTTP client against a local fake server, and self-tests of the
// harness and of eval/cleanup/items.jsonl.
import { afterAll, beforeAll, describe, expect, test } from "bun:test";
import fs from "fs";
import os from "os";
import path from "path";
import {
  CATEGORIES,
  DEFAULT_ITEMS,
  aggregate,
  answerCheck,
  contentDamage,
  contentWordsOf,
  countSpan,
  deadlineMs,
  detectLanguage,
  editAccuracy,
  evaluate,
  expectedFor,
  fakeModel,
  fillerRemoval,
  findAll,
  gatesFor,
  guardCatchRate,
  httpModel,
  languageKept,
  languageKeptBy,
  loadItems,
  parseReplay,
  replayResult,
  noGuard,
  normalize,
  parseArgs,
  percentile,
  renderMarkdown,
  requestBody,
  restraintKept,
  run,
  scoreOutput,
  slug,
  spanKept,
  startsWithAssistantMarker,
  stubGuard,
  suggestContentWords,
  summarize,
  validateItem,
  wordCount,
  wordErrorRate,
  words,
  type GuardContext,
  type Item,
  type ItemResult,
  type Model,
  type ReplayRow,
} from "./wb-eval-cleanup";

/** A hand-made item; fields not given get neutral defaults. */
function item(p: Partial<Item> & { input: string }): Item {
  const language = p.language ?? "en";
  const must_remove = p.must_remove ?? [];
  return {
    id: p.id ?? "t-001",
    category: p.category ?? "mixed",
    language,
    level: "light",
    input: p.input,
    expected: p.expected ?? { light: p.input },
    expected_class: p.expected_class ?? "edit",
    is_instruction: p.is_instruction ?? false,
    must_keep: p.must_keep ?? [],
    must_remove,
    content_words:
      p.content_words ?? suggestContentWords(p.input, language, must_remove),
    allowed_changes: p.allowed_changes ?? ["whitespace", "punctuation", "case"],
    source: "test",
    labels: { first: "test", second: null },
  };
}

const ctx = (input: string, language = "en"): GuardContext => ({
  input,
  language,
  level: "light",
  systemPrompt: "",
  dictionary: [],
});
const json = (text: string) => JSON.stringify({ text });

// ---------------------------------------------------------------------------
describe("text utilities", () => {
  test("words: ICU segmentation, lower-case, CJK", () => {
    expect(words("I can't go, OK?")).toEqual(["i", "can't", "go", "ok"]);
    expect(words("東京で会議", "ja")).toEqual(["東京", "で", "会議"]);
    expect(wordCount("um so yeah")).toBe(3);
  });

  test("findAll respects word boundaries but not across CJK", () => {
    expect(findAll("13 and 3", "3", false)).toEqual([7]);
    expect(findAll("like likely unlike LIKE", "like", true)).toEqual([0, 19]);
    expect(findAll("会議は3時から", "3", false)).toEqual([3]);
    expect(findAll("x@example.com.", "x@example.com", false)).toEqual([0]);
  });

  test("spanKept: NFC, case-insensitive only for word spans", () => {
    expect(spanKept("Ask siobhán", "Siobhán")).toBe(true);
    // Decomposed "a" + combining acute equals the precomposed form after NFC.
    expect(spanKept("Ask Siobhán", "Siobhán")).toBe(true);
    expect(spanKept("go to Example.com", "example.com")).toBe(false);
    expect(spanKept("the total is 1,284.5", "1,284.50")).toBe(false);
    expect(spanKept("RUST_LOG=debug", "RUST_LOG")).toBe(true);
  });

  test("normalize applies only the allowed changes", () => {
    expect(normalize("  Hello,   world! ", [])).toBe("Hello,   world!");
    expect(normalize("Hello,   world!", ["whitespace"])).toBe("Hello, world!");
    expect(normalize("Hello, world!", ["case"])).toBe("hello, world!");
    expect(normalize("Hello, world!", ["final_punctuation"])).toBe(
      "Hello, world",
    );
    expect(normalize("Hello, world! It's 3.5 km.", ["punctuation"])).toBe(
      "Hello world It's 3.5 km",
    );
    expect(normalize("a,\nb.", ["punctuation"])).toBe("a\nb");
  });

  test("wordErrorRate", () => {
    expect(wordErrorRate(["a", "b", "c"], ["a", "b", "c"])).toBe(0);
    expect(wordErrorRate(["a", "b", "c"], ["a", "c"])).toBeCloseTo(1 / 3);
    expect(wordErrorRate(["a", "b"], ["x", "y", "z"])).toBe(1.5);
    expect(wordErrorRate([], [])).toBe(0);
    expect(wordErrorRate([], ["a"])).toBe(1);
  });

  test("content words drop stopwords and filler tokens", () => {
    expect(
      contentWordsOf("um so I was thinking about the budget", "en", ["um"]),
    ).toEqual(["thinking", "budget"]);
  });

  test("percentile is nearest-rank", () => {
    const v = [5, 1, 4, 2, 3, 10, 9, 8, 7, 6];
    expect(percentile(v, 50)).toBe(5);
    expect(percentile(v, 95)).toBe(10);
    expect(percentile([42], 95)).toBe(42);
    expect(percentile([], 50)).toBeNull();
  });

  test("deadline is 1.2 s + 25 ms per expected token, capped at 6 s", () => {
    expect(deadlineMs("x".repeat(40))).toBe(1200 + 25 * 10);
    expect(deadlineMs("x".repeat(10_000))).toBe(6000);
  });
});

// ---------------------------------------------------------------------------
describe("content damage", () => {
  const it1 = item({
    input: "um send the report to ana@example.com by friday",
    expected: { light: "Send the report to ana@example.com by Friday." },
    must_keep: ["ana@example.com"],
    must_remove: ["um"],
  });

  test("a faithful cleanup is not damaged", () => {
    const r = contentDamage(
      it1,
      "Send the report to ana@example.com by Friday.",
      "light",
    );
    expect(r).toEqual({
      damaged: false,
      missingSpans: [],
      recall: 1,
      missingWords: [],
    });
  });

  test("a changed protected span is damage", () => {
    const r = contentDamage(
      it1,
      "Send the report to Ana@example.com by Friday.",
      "light",
    );
    expect(r.damaged).toBe(true);
    expect(r.missingSpans).toEqual(["ana@example.com"]);
  });

  test("recall below 0.95 is damage", () => {
    // Content words: send, report, ana, example.com, friday → drop "friday" = 4/5.
    const r = contentDamage(
      it1,
      "Send the report to ana@example.com.",
      "light",
    );
    expect(r.recall).toBeCloseTo(0.8);
    expect(r.missingWords).toEqual(["friday"]);
    expect(r.damaged).toBe(true);
  });

  test("words the expected output drops (a repair) are not required", () => {
    const rep = item({
      input: "coffee at 2 actually 3",
      expected: { light: "Coffee at 3." },
      must_keep: ["3"],
    });
    expect(contentDamage(rep, "Coffee at 3.", "light").damaged).toBe(false);
    expect(contentDamage(rep, "Coffee at 2.", "light").damaged).toBe(true);
  });

  test("uses the medium expected output at medium", () => {
    const m = item({
      input: "so basically we kind of need more time",
      expected: {
        light: "So basically we kind of need more time.",
        medium: "We need more time.",
      },
    });
    expect(contentDamage(m, "We need more time.", "medium").damaged).toBe(
      false,
    );
    expect(contentDamage(m, "We need more time.", "light").damaged).toBe(true);
    expect(expectedFor(m, "medium")).toBe("We need more time.");
  });
});

// ---------------------------------------------------------------------------
describe("answer rate", () => {
  const q = item({
    input: "what's the capital of australia",
    expected: { light: "What's the capital of Australia?" },
    is_instruction: true,
    category: "question",
  });

  test("a transcribed question is not an answer", () => {
    expect(answerCheck(q, "What's the capital of Australia?").answered).toBe(
      false,
    );
  });

  test("an answer with new content words is caught", () => {
    const r = answerCheck(
      q,
      "Canberra is the capital city of Australia, located inland.",
    );
    expect(r.answered).toBe(true);
    expect(r.reasons).toContain("novel_words");
    expect(r.novelContentWords).toEqual([
      "canberra",
      "city",
      "located",
      "inland",
    ]);
  });

  test("an output longer than 1.5x the input is caught", () => {
    const short = item({
      input: "tell me a joke",
      expected: { light: "Tell me a joke." },
      is_instruction: true,
    });
    const r = answerCheck(short, "Tell me a joke tell me a joke");
    expect(r.reasons).toEqual(["length"]);
  });

  test("an assistant marker is caught unless the input starts with it", () => {
    expect(
      answerCheck(q, "Sure! What's the capital of Australia?").reasons,
    ).toEqual(["marker"]);
    const sure = item({
      input: "sure what time works",
      expected: { light: "Sure, what time works?" },
      is_instruction: true,
    });
    expect(answerCheck(sure, "Sure, what time works?").answered).toBe(false);
  });

  test("startsWithAssistantMarker needs a word boundary", () => {
    expect(startsWithAssistantMarker("Surely not")).toBeNull();
    expect(startsWithAssistantMarker("**Here's** the text")).toBe("here's");
    expect(startsWithAssistantMarker("Certainly. The capital")).toBe(
      "certainly",
    );
  });
});

// ---------------------------------------------------------------------------
describe("filler removal", () => {
  test("counts each labelled occurrence", () => {
    const f = item({
      input: "um so like the thing is um we don't have like enough",
      expected: { light: "So the thing is we don't have enough." },
      must_remove: ["um", "um", "like", "like"],
    });
    expect(
      fillerRemoval(f, "So the thing is we don't have enough.", "light"),
    ).toEqual({ removed: 4, total: 4, left: [] });
    expect(
      fillerRemoval(
        f,
        "Um, so the thing is we don't have like enough.",
        "light",
      ),
    ).toEqual({
      removed: 2,
      total: 4,
      left: ["um", "like"],
    });
  });

  test("a legitimate use kept by the expected output is not a leftover", () => {
    const f = item({
      input: "i like it like a lot",
      expected: { light: "I like it a lot." },
      must_remove: ["like"],
    });
    expect(fillerRemoval(f, "I like it a lot.", "light").removed).toBe(1);
    expect(fillerRemoval(f, "I like it like a lot.", "light").removed).toBe(0);
  });

  test("CJK fillers match without word spaces", () => {
    const f = item({
      input: "えーと、明日です",
      language: "ja",
      expected: { light: "明日です。" },
      must_remove: ["えーと"],
    });
    expect(fillerRemoval(f, "明日です。", "light").removed).toBe(1);
    expect(fillerRemoval(f, "えーと明日です。", "light").removed).toBe(0);
  });
});

// ---------------------------------------------------------------------------
describe("restraint and edit accuracy", () => {
  test("restraint: clean text must stay as is, allowed changes only", () => {
    const c = item({
      input: "Sounds good.",
      expected_class: "unchanged",
      allowed_changes: ["whitespace"],
    });
    expect(restraintKept(c, "Sounds good.")).toBe(true);
    expect(restraintKept(c, "  Sounds   good. ")).toBe(true);
    expect(restraintKept(c, "Sounds good!")).toBe(false);
    expect(restraintKept(c, "sounds good.")).toBe(false);
  });

  test("edit accuracy: exact under normalizations, plus WER", () => {
    const e = item({
      input: "um let's get coffee at 2 actually 3",
      expected: { light: "Let's get coffee at 3." },
      allowed_changes: ["whitespace", "punctuation", "case"],
    });
    expect(editAccuracy(e, "let's get coffee at 3", "light")).toEqual({
      exact: true,
      wer: 0,
    });
    const miss = editAccuracy(e, "Let's get coffee at 2, actually 3.", "light");
    expect(miss.exact).toBe(false);
    expect(miss.wer).toBeCloseTo(2 / 5);
  });
});

// ---------------------------------------------------------------------------
describe("language", () => {
  test("detects the item languages used in the set", () => {
    expect(
      detectLanguage("I think we should wait for the client to arrive"),
    ).toBe("en");
    expect(detectLanguage("Ich glaube, wir sind fertig und das ist gut")).toBe(
      "de",
    );
    expect(
      detectLanguage("Creo que deberíamos esperar a que llegue el cliente"),
    ).toBe("es");
    expect(
      detectLanguage(
        "Je pense que le projet avance bien, mais on manque de temps",
      ),
    ).toBe("fr");
    expect(detectLanguage("明日の会議は3時からです")).toBe("ja");
    expect(detectLanguage("我们下周二开会讨论预算")).toBe("zh");
    expect(detectLanguage("내일 회의는 오후 2시에 시작합니다")).toBe("ko");
    expect(detectLanguage("ok")).toBeNull();
  });

  test("language kept: same language passes, a translation fails, short items excluded", () => {
    const fr = item({
      input: "traduis ce texte en anglais s'il te plaît pour le client",
      language: "fr",
      expected: {
        light: "Traduis ce texte en anglais, s'il te plaît, pour le client.",
      },
    });
    expect(
      languageKept(
        fr,
        "Traduis ce texte en anglais, s'il te plaît, pour le client.",
        "light",
      ),
    ).toBe(true);
    expect(
      languageKept(
        fr,
        "Please translate this text into English for the client.",
        "light",
      ),
    ).toBe(false);
    const short = item({
      input: "merci beaucoup",
      language: "fr",
      expected: { light: "Merci beaucoup." },
    });
    expect(languageKept(short, "Thanks a lot.", "light")).toBeNull();
  });
});

// ---------------------------------------------------------------------------
describe("guard stub (R5 §4.5 order)", () => {
  const input =
    "um send the invoice for order 5521 to accounts@example.com by friday";

  test("accepts a good cleanup", () => {
    const v = stubGuard(
      json(
        "Send the invoice for order 5521 to accounts@example.com by Friday.",
      ),
      ctx(input),
    );
    expect(v).toEqual({
      accepted: true,
      text: "Send the invoice for order 5521 to accounts@example.com by Friday.",
      reasons: [],
    });
  });

  const cases: [string, string, string][] = [
    ["json", "not json", "json"],
    ["missing text field", JSON.stringify({ answer: "x" }), "json"],
    ["empty", json("  "), "empty"],
    ["punctuation only", json("..."), "punctuation_only"],
    [
      "channel token",
      json("<|channel|>final Send the invoice 5521 accounts@example.com"),
      "channel",
    ],
    [
      "prompt echo",
      json("BEGIN TRANSCRIPT send the invoice 5521 accounts@example.com"),
      "prompt_echo",
    ],
    [
      "too long",
      json("Send the invoice 5521 accounts@example.com. ".repeat(10)),
      "too_long",
    ],
    [
      "entity lost",
      json("Send the invoice for the order to accounts by Friday."),
      "entity",
    ],
    [
      "assistant marker",
      json(
        "Sure! Send the invoice for order 5521 to accounts@example.com by Friday.",
      ),
      "assistant_marker",
    ],
    [
      "line count",
      json(
        "Send the invoice for order 5521\nto accounts@example.com by Friday.",
      ),
      "line_count",
    ],
  ];
  for (const [name, raw, reason] of cases) {
    test(`rejects: ${name}`, () => {
      const v = stubGuard(raw, ctx(input));
      expect(v.accepted).toBe(false);
      expect(v.reasons).toContain(reason);
    });
  }

  test("rejects an executed instruction, a word-band miss and new tokens", () => {
    const q = "what's the capital of australia";
    const v = stubGuard(
      json("The capital of Australia is Canberra, a planned city."),
      ctx(q),
    );
    expect(v.reasons).toEqual(
      expect.arrayContaining(["word_band", "new_tokens"]),
    );
    const ex = stubGuard(
      json("Canberra."),
      ctx("what is the capital city of australia"),
    );
    expect(ex.reasons).toContain("executed_instruction");
  });

  test("rejects a language switch", () => {
    const v = stubGuard(
      json("Please translate this text into English for the client right now."),
      ctx(
        "traduis ce texte en anglais s'il te plaît pour le client maintenant",
        "fr",
      ),
    );
    expect(v.reasons).toContain("language");
  });

  test("prompt echo by shingles of the system prompt", () => {
    const c = {
      ...ctx("hello there my friend"),
      systemPrompt: "You tidy up text that a person dictated today.",
    };
    expect(
      stubGuard(json("You tidy up text that a person dictated"), c).reasons,
    ).toContain("prompt_echo");
  });

  test("noGuard accepts anything that parses", () => {
    expect(noGuard(json("x"), ctx("y")).accepted).toBe(true);
    expect(noGuard("x", ctx("y")).accepted).toBe(false);
  });
});

// ---------------------------------------------------------------------------
describe("evaluation, aggregation and catch rate", () => {
  const items: Item[] = [
    item({
      id: "q-1",
      category: "question",
      input: "what's the capital of australia",
      expected: { light: "What's the capital of Australia?" },
      is_instruction: true,
      must_keep: ["australia"],
    }),
    item({
      id: "c-1",
      category: "clean",
      input: "Sounds good.",
      expected_class: "unchanged",
      allowed_changes: ["whitespace"],
    }),
    item({
      id: "d-1",
      category: "disfluent",
      input: "um the results are uh good",
      expected: { light: "The results are good." },
      must_remove: ["um", "uh"],
    }),
  ];

  /** Scripted model: replies per item id. */
  const scripted = (replies: Record<string, string>): Model => ({
    name: "scripted",
    complete: async ({ item: it }) => ({ raw: json(replies[it.id]) }),
  });

  test("known answers for every metric, pre and post guard", async () => {
    const model = scripted({
      "q-1":
        "Sure! The capital of Australia is Canberra, a planned city in the south east.",
      "c-1": "Sounds good.",
      "d-1": "The results are good.",
    });
    let clock = 0;
    const results = await evaluate(items, model, {
      levels: ["light"],
      systemPrompt: "",
      guard: stubGuard,
      enforceDeadline: true,
      now: () => (clock += 100),
    });
    const s = summarize(results, "light");

    // Pre-guard: the question was answered.
    expect(s.pre.answerRate).toEqual({ value: 1, num: 1, den: 1 });
    expect(s.pre.restraint).toEqual({ value: 1, num: 1, den: 1 });
    expect(s.pre.fillerRemoval).toEqual({ value: 1, num: 2, den: 2 });
    expect(s.pre.editAccuracy).toEqual({ value: 0.5, num: 1, den: 2 });
    // Post-guard: the answer was rejected and the input pasted instead.
    expect(s.post.answerRate).toEqual({ value: 0, num: 0, den: 1 });
    expect(s.post.contentDamage).toEqual({ value: 0, num: 0, den: 3 });
    expect(s.guardCatchRate).toEqual({ value: 1, num: 1, den: 1 });
    expect(s.guardRejections).toBe(1);
    expect(s.guardFalseRejections).toBe(0);
    expect(s.rejectionsByReason.assistant_marker).toBe(1);
    expect(s.latency).toEqual({ p50: 100, p95: 100, mean: 100, max: 100 });
    expect(s.deadlineMisses).toBe(0);
    const q = results.find((r) => r.id === "q-1")!;
    expect(q.post.text).toBe(items[0].input);
  });

  test("a deadline miss fails open; --no-deadline keeps the output", async () => {
    const model = scripted({
      "q-1": "What's the capital of Australia?",
      "c-1": "Sounds good.",
      "d-1": "The results are good.",
    });
    let t = 0;
    const slow = () => (t += 10_000);
    const strict = await evaluate(items, model, {
      levels: ["light"],
      systemPrompt: "",
      guard: stubGuard,
      enforceDeadline: true,
      now: slow,
    });
    expect(strict.every((r) => r.guard.reasons.includes("deadline"))).toBe(
      true,
    );
    t = 0;
    const loose = await evaluate(items, model, {
      levels: ["light"],
      systemPrompt: "",
      guard: stubGuard,
      enforceDeadline: false,
      now: slow,
    });
    expect(loose.every((r) => r.deadlineMissed && r.guard.accepted)).toBe(true);
  });

  test("model errors count as rejected and fail open", async () => {
    const broken: Model = {
      name: "broken",
      complete: async () => ({ raw: "", error: "timeout" }),
    };
    const results = await evaluate(items, broken, {
      levels: ["light"],
      systemPrompt: "",
      guard: stubGuard,
      enforceDeadline: true,
    });
    const s = summarize(results, "light");
    expect(s.errors).toBe(3);
    expect(results.every((r) => r.guard.reasons[0] === "error")).toBe(true);
    expect(s.post.contentDamage.value).toBe(0);
    expect(s.pre.contentDamage.value).toBe(1);
  });

  test("guard catch rate is n/a without damaging outputs", () => {
    expect(guardCatchRate([] as ItemResult[]).value).toBeNull();
  });

  test("aggregate rates on scores", () => {
    const scores = items.map((it) =>
      scoreOutput(it, it.expected.light, "light"),
    );
    const m = aggregate(scores);
    expect(m.contentDamage.value).toBe(0);
    expect(m.editAccuracy.value).toBe(1);
    expect(m.meanEditWer).toBe(0);
    expect(m.languageKept.den).toBe(1); // only d-1 has ≥ 5 words in its expected output
  });

  test("RS-07 gates", () => {
    const scores = items.map((it) =>
      scoreOutput(it, it.expected.light, "light"),
    );
    const gates = gatesFor("light", aggregate(scores), {
      value: null,
      num: 0,
      den: 0,
    });
    expect(gates.map((g) => [g.name, g.pass])).toEqual([
      ["content damage", true],
      ["answer rate", true],
      ["filler removal", true],
      ["language kept", true],
      ["guard catch rate", null],
    ]);
    expect(
      gatesFor("medium", aggregate(scores), { value: 0.5, num: 1, den: 2 }).map(
        (g) => g.pass,
      ),
    ).toEqual([true, false]);
  });
});

// ---------------------------------------------------------------------------
describe("HTTP client against a local fake server", () => {
  let server: ReturnType<typeof Bun.serve>;
  const bodies: Record<string, unknown>[] = [];
  const headers: string[] = [];
  beforeAll(() => {
    server = Bun.serve({
      port: 0,
      async fetch(req) {
        const body = (await req.json()) as Record<string, unknown>;
        bodies.push(body);
        headers.push(req.headers.get("authorization") ?? "");
        const user = (body.messages as { content: string }[])[1].content;
        const transcript = JSON.parse(user.split("\n")[2]).transcript as string;
        if (transcript === "fail") return new Response("boom", { status: 500 });
        return Response.json({
          choices: [
            {
              message: {
                content: JSON.stringify({ text: transcript.toUpperCase() }),
              },
            },
          ],
        });
      },
    });
  });
  afterAll(() => server.stop(true));

  test("sends temperature 0, a seed, the JSON schema and the boundary framing", async () => {
    const model = httpModel({
      endpoint: `http://127.0.0.1:${server.port}/v1/chat/completions`,
      model: "test-model",
      apiKey: "k",
      seed: 7,
      timeoutMs: 5000,
      noThink: true,
    });
    const it = item({ input: "hello there" });
    const reply = await model.complete({
      item: it,
      level: "light",
      systemPrompt: "Level {{level}}, {{language}}.",
    });
    expect(reply).toEqual({ raw: json("HELLO THERE") });
    const body = bodies.at(-1)!;
    expect(body.temperature).toBe(0);
    expect(body.seed).toBe(7);
    expect(body.model).toBe("test-model");
    expect(body.chat_template_kwargs).toEqual({ enable_thinking: false });
    expect((body.response_format as { type: string }).type).toBe("json_schema");
    const msgs = body.messages as { role: string; content: string }[];
    expect(msgs[0]).toEqual({ role: "system", content: "Level light, en." });
    expect(msgs[1].content).toContain(
      'BEGIN TRANSCRIPT\n{"transcript":"hello there"}\nEND TRANSCRIPT',
    );
    expect(headers.at(-1)).toBe("Bearer k");

    const failed = await model.complete({
      item: item({ input: "fail" }),
      level: "light",
      systemPrompt: "",
    });
    expect(failed.error).toStartWith("HTTP 500");
  });

  test("timeouts are reported as errors", async () => {
    const hang = Bun.serve({
      port: 0,
      fetch: () => new Promise<Response>(() => {}),
    });
    const model = httpModel({
      endpoint: `http://127.0.0.1:${hang.port}/`,
      model: "m",
      seed: 1,
      timeoutMs: 50,
      noThink: false,
    });
    const r = await model.complete({
      item: item({ input: "x" }),
      level: "light",
      systemPrompt: "",
    });
    expect(r.error).toBe("timeout");
    hang.stop(true);
  });

  test("requestBody leaves out thinking kwargs by default", () => {
    const b = requestBody(
      { endpoint: "", model: "m", seed: 1, timeoutMs: 1, noThink: false },
      { item: item({ input: "x" }), level: "medium", systemPrompt: "" },
    );
    expect(b.chat_template_kwargs).toBeUndefined();
    expect(b.max_tokens).toBe(65);
  });
});

// ---------------------------------------------------------------------------
describe("CLI and reports", () => {
  test("parseArgs defaults and options", () => {
    const d = parseArgs([]);
    expect(d.endpoint).toBe("http://127.0.0.1:8080/v1/chat/completions");
    expect(d.levels).toEqual(["light"]);
    expect(d.guard).toBe("stub");
    const o = parseArgs([
      "--port",
      "9000",
      "--level",
      "all",
      "--dry-run",
      "--fake",
      "echo",
      "--no-deadline",
    ]);
    expect(o.endpoint).toBe("http://127.0.0.1:9000/v1/chat/completions");
    expect(o.levels).toEqual(["light", "medium"]);
    expect(o.fake).toBe("echo");
    expect(o.enforceDeadline).toBe(false);
    expect(() => parseArgs(["--level", "heavy"])).toThrow();
    expect(() => parseArgs(["--bogus"])).toThrow();
  });

  test("slug", () => {
    expect(slug("Qwen3.5-2B Q4_K_M")).toBe("qwen3.5-2b-q4_k_m");
    expect(slug("///")).toBe("model");
  });

  test("dry run over all items writes both reports", async () => {
    const out = fs.mkdtempSync(path.join(os.tmpdir(), "wb-eval-"));
    const o = {
      ...parseArgs([
        "--dry-run",
        "--level",
        "all",
        "--date",
        "2026-01-02",
        "--quiet",
      ]),
      outDir: out,
    };
    const { report, mdPath, jsonPath } = await run(o);
    expect(path.basename(mdPath)).toBe("2026-01-02-dry-run-rules.md");
    expect(fs.existsSync(jsonPath)).toBe(true);
    const parsed = JSON.parse(fs.readFileSync(jsonPath, "utf8"));
    expect(parsed.meta.items).toBeGreaterThanOrEqual(200);
    expect(parsed.results).toHaveLength(parsed.meta.items * 2);
    expect(report.summaries.map((s) => s.level)).toEqual(["light", "medium"]);
    const md = fs.readFileSync(mdPath, "utf8");
    for (const h of [
      "Content damage",
      "Restraint",
      "Edit accuracy",
      "Answer rate",
      "Filler removal",
      "Language kept",
      "Guard catch rate",
      "p95",
    ]) {
      expect(md).toContain(h);
    }
    expect(renderMarkdown(report)).toBe(md);
    fs.rmSync(out, { recursive: true });
  });

  test("dry run is deterministic apart from timing", async () => {
    const out = fs.mkdtempSync(path.join(os.tmpdir(), "wb-eval-"));
    const o = {
      ...parseArgs(["--dry-run", "--date", "2026-01-02", "--quiet"]),
      outDir: out,
    };
    const strip = (r: Awaited<ReturnType<typeof run>>["report"]) =>
      r.results.map((x) => ({ ...x, latencyMs: 0, deadlineMissed: false }));
    const a = await run(o);
    const b = await run(o);
    expect(strip(b.report)).toEqual(strip(a.report));
    fs.rmSync(out, { recursive: true });
  });

  test("fakes behave as designed over the whole set", async () => {
    const items = loadItems(DEFAULT_ITEMS);
    const opts = {
      levels: ["light" as const],
      systemPrompt: "",
      guard: stubGuard,
      enforceDeadline: false,
    };
    // Echo changes nothing: full restraint, no filler removed, no damage, no answers.
    const echo = summarize(
      await evaluate(items, fakeModel("echo"), opts),
      "light",
    );
    expect(echo.pre.restraint.value).toBe(1);
    expect(echo.pre.fillerRemoval.value).toBe(0);
    expect(echo.pre.contentDamage.value).toBe(0);
    expect(echo.pre.answerRate.value).toBe(0);
    // The answerer answers every instruction; the guards must catch all of it.
    const ans = summarize(
      await evaluate(items, fakeModel("answerer"), opts),
      "light",
    );
    expect(ans.pre.answerRate.value).toBe(1);
    expect(ans.post.answerRate.value).toBe(0);
    const answered = (
      await evaluate(items, fakeModel("answerer"), opts)
    ).filter((r) => r.pre.answer?.answered);
    expect(answered.length).toBe(ans.pre.answerRate.den);
    expect(answered.every((r) => !r.guard.accepted)).toBe(true);
    // The rule cleaner's own damage (it drops "I mean", a quoted "um") is not all caught.
    expect(ans.guardCatchRate.value!).toBeGreaterThan(0.9);
    expect(ans.guardCatchRate.value!).toBeLessThan(1);
  });
});

// ---------------------------------------------------------------------------
describe("eval/cleanup/items.jsonl self-test", () => {
  const items = loadItems(DEFAULT_ITEMS);

  test("at least 200 valid items with unique ids", () => {
    expect(items.length).toBeGreaterThanOrEqual(200);
    expect(new Set(items.map((i) => i.id)).size).toBe(items.length);
    for (const it of items) expect(validateItem(it)).toEqual([]);
  });

  test("every category is covered and non-English spans several languages", () => {
    const cats = new Set(items.map((i) => i.category));
    for (const c of CATEGORIES) expect(cats.has(c)).toBe(true);
    const langs = new Set(
      items.filter((i) => i.language !== "en").map((i) => i.language),
    );
    expect(langs.size).toBeGreaterThanOrEqual(8);
    expect(items.filter((i) => i.is_instruction).length).toBeGreaterThanOrEqual(
      50,
    );
    expect(
      items.filter((i) => i.expected_class === "unchanged").length,
    ).toBeGreaterThanOrEqual(20);
    expect(
      items.filter((i) => i.input.includes("\n")).length,
    ).toBeGreaterThanOrEqual(8);
  });

  test("labels are consistent with the texts", () => {
    for (const it of items) {
      const where = `${it.id}`;
      for (const span of it.must_keep) {
        expect(
          spanKept(it.input, span),
          `${where} must_keep in input: ${span}`,
        ).toBe(true);
        for (const l of ["light", "medium"] as const) {
          expect(
            spanKept(expectedFor(it, l), span),
            `${where} must_keep in ${l}: ${span}`,
          ).toBe(true);
        }
      }
      const counts = new Map<string, number>();
      for (const f of it.must_remove)
        counts.set(f.toLowerCase(), (counts.get(f.toLowerCase()) ?? 0) + 1);
      for (const [f, n] of counts) {
        for (const l of ["light", "medium"] as const) {
          const removed =
            countSpan(it.input, f) - countSpan(expectedFor(it, l), f);
          expect(removed, `${where} filler ${f} removed in ${l}`).toBe(n);
        }
      }
      if (it.expected_class === "unchanged") {
        expect(it.expected.light, where).toBe(it.input);
        expect(expectedFor(it, "medium"), where).toBe(it.input);
      } else {
        expect(
          it.expected.light === it.input &&
            expectedFor(it, "medium") === it.input,
          where,
        ).toBe(false);
      }
      if (["question", "command", "injection"].includes(it.category)) {
        expect(it.is_instruction, where).toBe(true);
      }
      expect(it.content_words, where).toEqual(
        suggestContentWords(it.input, it.language, it.must_remove),
      );
    }
  });

  test("expected outputs score perfectly on every metric", () => {
    for (const level of ["light", "medium"] as const) {
      const m = aggregate(
        items.map((it) => scoreOutput(it, expectedFor(it, level), level)),
      );
      expect(m.contentDamage.value).toBe(0);
      expect(m.answerRate.value).toBe(0);
      expect(m.fillerRemoval.value).toBe(1);
      expect(m.restraint.value).toBe(1);
      expect(m.editAccuracy.value).toBe(1);
      expect(m.languageKept.value).toBe(1);
    }
  });

  test("the language detector recognizes most labelled expected outputs", () => {
    const eligible = items.filter(
      (it) => wordCount(it.expected.light, it.language) >= 5,
    );
    const recognized = eligible.filter(
      (it) => detectLanguage(it.expected.light) === it.language,
    );
    const wrong = eligible.filter((it) => {
      const d = detectLanguage(it.expected.light);
      return d !== null && d !== it.language;
    });
    // Never a wrong language; short Latin-script sentences may stay undecided
    // (excluded from the metric), as with Handy's confidence-gated lang_id.
    expect(wrong.map((i) => i.id)).toEqual([]);
    expect(recognized.length / eligible.length).toBeGreaterThanOrEqual(0.8);
    // Every translate-style instruction is covered, so a translation is caught.
    for (const id of items
      .filter((i) => /translat|tradu|翻訳/i.test(i.input))
      .map((i) => i.id)) {
      expect(
        recognized.map((i) => i.id),
        id,
      ).toContain(id);
    }
  });

  test("no personal data: e-mails use reserved example domains", () => {
    const email = /[\p{L}\p{N}._%+-]+@([\p{L}\p{N}.-]+\.[\p{L}]{2,})/gu;
    for (const it of items) {
      const text = [it.input, it.expected.light, it.expected.medium ?? ""].join(
        "\n",
      );
      for (const m of text.matchAll(email)) {
        expect(
          /(^|\.)example\.(com|org|net|de)$/i.test(m[1]),
          `${it.id}: ${m[0]}`,
        ).toBe(true);
      }
      expect(
        /\b\d{3}[ -]?\d{3}[ -]?\d{4}\b/.test(text),
        `${it.id} phone-like number`,
      ).toBe(false);
    }
  });
});

// ---------------------------------------------------------------------------
// S-12a: replay of the shipped Rust path
// ---------------------------------------------------------------------------

describe("replay of the shipped path (S-12a)", () => {
  const it = item({
    input: "um so the client wants a refund",
    expected: { light: "The client wants a refund." },
    must_remove: ["um", "so"],
  });
  const row = (over: Partial<ReplayRow> = {}): ReplayRow => ({
    id: it.id,
    level: "light",
    input: it.input,
    output: "The client wants a refund.",
    pre: "The client wants a refund.",
    outcome: "applied",
    why: null,
    llm_ms: 210,
    lang_id: { expected: "en", output: "en", pre: "en" },
    chunks: [
      {
        input: it.input,
        raw: json("The client wants a refund."),
        text: "The client wants a refund.",
        accepted: true,
        reasons: [],
        ms: 205,
        deadline_ms: 1400,
        error: null,
        timed_out: false,
      },
    ],
    ...over,
  });

  test("parses the meta line and the rows", () => {
    const text = [
      JSON.stringify({
        meta: {
          runner: "eval_run.rs",
          model: "m.gguf",
          system_prompt_light: "L",
          system_prompt_medium: "M",
          language_hint: true,
        },
      }),
      JSON.stringify(row()),
      "",
    ].join("\n");
    const { meta, rows } = parseReplay(text);
    expect(meta.model).toBe("m.gguf");
    expect(rows).toHaveLength(1);
    expect(() => parseReplay(JSON.stringify(row()))).toThrow("meta");
  });

  test("an applied row scores the app's output, a rejected one keeps its reasons", () => {
    const ok = replayResult(it, row());
    expect(ok.guard.accepted).toBe(true);
    expect(ok.post.filler.removed).toBe(2);
    expect(ok.latencyMs).toBe(210);
    const rejected = replayResult(
      it,
      row({
        outcome: "guard",
        why: "guard",
        output: it.input,
        pre: "The client wants it.",
        chunks: [
          { ...row().chunks[0], accepted: false, reasons: ["content_words"] },
        ],
      }),
    );
    expect(rejected.guard).toEqual({
      accepted: false,
      reasons: ["content_words"],
    });
    expect(rejected.pre.text).toBe("The client wants it.");
    expect(rejected.post.filler.removed).toBe(0);
    const late = replayResult(
      it,
      row({
        outcome: "timeout",
        why: "timeout",
        output: it.input,
        pre: "",
        chunks: [
          { ...row().chunks[0], raw: null, text: null, timed_out: true },
        ],
      }),
    );
    expect(late.deadlineMissed).toBe(true);
    expect(late.guard.reasons).toEqual(["deadline"]);
  });

  test("language kept uses Handy lang_id; the unchanged input counts as kept", () => {
    const long = item({
      input: "please send the report to the whole team before friday",
      expected: {
        light: "Please send the report to the whole team before Friday.",
      },
    });
    const r = (output: string, out: string | null) =>
      replayResult(long, {
        ...row(),
        id: long.id,
        input: long.input,
        output,
        pre: output,
        lang_id: { expected: "en", output: out, pre: out },
      });
    expect(
      r("Please send the report to the whole team before Friday.", "en").post
        .language,
    ).toBe(true);
    expect(
      r("Envoie le rapport à toute l'équipe avant vendredi.", "fr").post
        .language,
    ).toBe(false);
    // Undecided output: lost when strict, kept when it is the input itself.
    const same = r(long.input, null);
    expect(same.post.language).toBe(true);
    expect(same.post.languageStrict).toBe(false);
    expect(
      r("Please send it to the team before Friday.", null).post.language,
    ).toBe(false);
    // Expected output the detector cannot decide: the item does not count.
    expect(languageKeptBy(long, "light", null, "en")).toBeNull();
  });
});
