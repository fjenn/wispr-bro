// SPDX-License-Identifier: GPL-3.0-or-later
// Run: bun test scripts/wb-bench-cleanup.test.ts  (also part of `bun run wb:test:scripts`)
// RS-07 bench harness: prompt files and their provenance headers, request
// shapes per prompt/format/template, reply parsing, the stratified cut and
// the loopback-only spawn line. No server is started here.
import { describe, expect, test } from "bun:test";
import fs from "fs";
import os from "os";
import path from "path";
import {
  GENERAL_PROMPTS,
  MODELS,
  PROMPT_DIR,
  assertLocalOnly,
  buildRequest,
  parseCli,
  parsePromptFile,
  parseReply,
  promptFileFor,
  rescoreDir,
  rescoreReport,
  selectStratified,
  serverArgs,
  toScorerRaw,
  variantsFor,
  type PromptFile,
  type SavedReport,
  type Variant,
} from "./wb-bench-cleanup";
import {
  DEFAULT_ITEMS,
  REPO_ROOT,
  evaluate,
  fakeModel,
  loadItems,
  sha256,
  stubGuard,
  summarize,
  type Item,
} from "./wb-eval-cleanup";

const items = loadItems(DEFAULT_ITEMS);
const en = items.find((it) => it.language === "en")!;
const de = items.find((it) => it.language === "de")!;
const P: PromptFile = { header: [], body: "SYSTEM {{level}} {{language}}" };
const v = (prompt: Variant["prompt"], format: Variant["format"]): Variant => ({
  prompt,
  format,
  levels: ["light"],
});
type Msg = { role: string; content: string };
const msgs = (b: Record<string, unknown>) => b.messages as Msg[];

describe("prompt files", () => {
  const files = [
    ...GENERAL_PROMPTS.map((p) => promptFileFor(p)),
    promptFileFor("native", "muesli"),
    promptFileFor("native", "speakoflow-mini"),
  ];
  for (const f of files) {
    test(`${path.basename(f)} names source, commit and licence`, () => {
      const { header, body } = parsePromptFile(fs.readFileSync(f, "utf8"));
      const h = header.join("\n");
      expect(h).toMatch(/^Source: /m);
      expect(h).toMatch(/\b[0-9a-f]{40}\b/);
      expect(h).toMatch(/Licen[cs]e: (MIT|Apache-2\.0)/);
      expect(body.length).toBeGreaterThan(500);
    });
  }
  test("upstream licences are reproduced", () => {
    const lic = fs.readFileSync(
      path.join(PROMPT_DIR, "LICENSES-upstream.md"),
      "utf8",
    );
    for (const c of [
      "Copyright (c) 2024 OpenWhispr Team",
      "Copyright (c) 2026 Zach Latta",
      "Copyright (c) 2026 Abhishek Barali",
    ])
      expect(lic).toContain(c);
  });
  test("only the trimmed prompt is level-aware", () => {
    for (const p of GENERAL_PROMPTS) {
      const { body } = parsePromptFile(
        fs.readFileSync(promptFileFor(p), "utf8"),
      );
      expect(body.includes("{{level}}")).toBe(p === "trimmed");
    }
  });
  test("parsePromptFile splits at the first --- line", () => {
    expect(parsePromptFile("# a\n# b\n---\nx\n---\ny\n")).toEqual({
      header: ["a", "b"],
      body: "x\n---\ny",
    });
    expect(() => parsePromptFile("no separator")).toThrow();
  });
});

describe("requests", () => {
  test("json format adds the schema and our JSON line; plain adds neither", () => {
    const j = buildRequest(v("speakoflow", "json"), P, en, "light", 42);
    const p = buildRequest(v("speakoflow", "plain"), P, en, "light", 42);
    expect(j.body.response_format).toBeDefined();
    expect(p.body.response_format).toBeUndefined();
    expect(msgs(j.body)[0].content).toContain('{"text"');
    expect(msgs(p.body)[0].content).toBe(P.body);
    expect(msgs(p.body)[1].content).toBe(en.input);
    expect(j.body.temperature).toBe(0);
    expect(j.body.seed).toBe(42);
  });
  test("each upstream prompt keeps its own framing", () => {
    const ow = msgs(
      buildRequest(v("openwhispr", "plain"), P, en, "light", 1).body,
    );
    expect(ow[1].content).toBe(
      `<transcript>\n${en.input}\n</transcript>\n\nOutput only the cleaned transcript.`,
    );
    const ff = msgs(
      buildRequest(v("freeflow", "plain"), P, de, "light", 1).body,
    );
    expect(ff[0].content).toContain("primarily in German");
    expect(ff[1].content).toContain(`<<<RAW_TRANSCRIPTION\n${de.input}\n`);
    const ffEn = msgs(
      buildRequest(v("freeflow", "plain"), P, en, "light", 1).body,
    );
    expect(ffEn[0].content).toBe(P.body);
    const tr = msgs(
      buildRequest(v("trimmed", "plain"), P, de, "medium", 1).body,
    );
    expect(tr[0].content).toBe("SYSTEM medium de");
    expect(tr[1].content).toContain("BEGIN TRANSCRIPT");
  });
  test("Muesli card template goes to /completion as one user turn", () => {
    const r = buildRequest(v("native", "plain"), P, en, "light", 7, "muesli");
    expect(r.path).toBe("/completion");
    expect(r.body.prompt).toBe(
      `<|im_start|>user\n${P.body}\n\n${en.input}<|im_end|>\n<|im_start|>assistant\n<think>\n\n</think>\n\n`,
    );
    expect(r.body.top_k).toBe(1);
    expect(r.body.stop).toEqual(["<|im_end|>"]);
    expect(r.body.json_schema).toBeUndefined();
    const j = buildRequest(v("native", "json"), P, en, "light", 7, "muesli");
    expect(j.body.json_schema).toBeDefined();
  });
  test("SpeakoFlow Mini card template: trained prompt untouched, raw transcript", () => {
    const r = buildRequest(
      v("native", "json"),
      P,
      en,
      "light",
      7,
      "speakoflow-mini",
    );
    expect(msgs(r.body)[0].content).toBe(P.body);
    expect(msgs(r.body)[1].content).toBe(en.input);
    expect(r.body.max_tokens).toBe(2048);
  });
  test("native without a fine-tune is refused", () => {
    expect(() =>
      buildRequest(v("native", "plain"), P, en, "light", 1),
    ).toThrow();
  });
});

describe("replies", () => {
  test("parseReply reads chat and completion bodies", () => {
    const chat = JSON.stringify({
      choices: [{ message: { content: "Hi." } }],
      timings: {
        prompt_n: 3,
        prompt_per_second: 100,
        predicted_n: 2,
        predicted_per_second: 50,
      },
    });
    expect(parseReply("/v1/chat/completions", chat)).toEqual({
      content: "Hi.",
      timings: {
        promptN: 3,
        promptPerSecond: 100,
        predictedN: 2,
        predictedPerSecond: 50,
      },
    });
    expect(
      parseReply("/completion", JSON.stringify({ content: "x" })).content,
    ).toBe("x");
    expect(parseReply("/completion", "{}").content).toBeNull();
  });
  test("toScorerRaw wraps plain text and maps FreeFlow's EMPTY", () => {
    expect(toScorerRaw('{"text":"a"}', "json")).toBe('{"text":"a"}');
    expect(JSON.parse(toScorerRaw("  Hello.\n", "plain"))).toEqual({
      text: "Hello.",
    });
    expect(JSON.parse(toScorerRaw("EMPTY", "plain"))).toEqual({ text: "" });
  });
});

describe("selection and variants", () => {
  test("stratified cut keeps every category and is deterministic", () => {
    const cut = selectStratified(items, 3);
    const cats = new Set(items.map((it: Item) => it.category));
    for (const c of cats)
      expect(cut.filter((it) => it.category === c).length).toBe(
        Math.min(3, items.filter((it) => it.category === c).length),
      );
    expect(selectStratified(items, 3)).toEqual(cut);
    expect(selectStratified(items, null)).toBe(items);
  });
  test("native variants only for fine-tunes; medium only where asked", () => {
    const qwen = MODELS.find((m) => !m.native)!;
    const mini = MODELS.find((m) => m.native === "speakoflow-mini")!;
    const all = [...GENERAL_PROMPTS, "native" as const];
    const q = variantsFor(
      qwen,
      all,
      ["json", "plain"],
      ["light", "medium"],
      ["trimmed"],
    );
    expect(q.length).toBe(8);
    expect(
      q.filter((x) => x.levels.includes("medium")).map((x) => x.prompt),
    ).toEqual(["trimmed", "trimmed"]);
    expect(variantsFor(mini, all, ["plain"], ["light"], []).length).toBe(5);
  });
});

describe("server spawn line", () => {
  test("R5 §4.5 flags, loopback, offline, never -hf", () => {
    const a = serverArgs("/m.gguf", "127.0.0.1", 5000);
    for (const f of ["--offline", "--no-webui", "--jinja", "--cache-ram"])
      expect(a).toContain(f);
    expect(
      a.slice(a.indexOf("--reasoning"), a.indexOf("--reasoning") + 2),
    ).toEqual(["--reasoning", "off"]);
    expect(() => assertLocalOnly(a)).not.toThrow();
    expect(() =>
      assertLocalOnly(serverArgs("/m", "/tmp/x.sock")),
    ).not.toThrow();
    expect(() => assertLocalOnly(serverArgs("/m", "0.0.0.0", 1))).toThrow();
    expect(() => assertLocalOnly([...a, "-hf", "x/y"])).toThrow();
  });
  test("CLI requires --server and rejects unknown ids", () => {
    expect(() => parseCli([])).toThrow();
    expect(() => parseCli(["--server", "x", "--models", "nope"])).toThrow();
    expect(parseCli(["--help"])).toBeNull();
    expect(
      parseCli(["--server", "x", "--per-category", "4"])!.perCategory,
    ).toBe(4);
  });
});

describe("rescore (saved outputs, current labels)", () => {
  const trimmedFile = "eval/cleanup/prompts/trimmed.txt";
  const body = parsePromptFile(
    fs.readFileSync(path.join(REPO_ROOT, trimmedFile), "utf8"),
  ).body;
  const pick = ["disfluent-002", "disfluent-009", "question-001", "repair-006"];
  const subset = items.filter((it) => pick.includes(it.id));
  // disfluent-009 as first labelled: "so" and "basically" not yet fillers.
  const FILLERS = ["so", "basically"];
  const oldLabels = subset.map((it) =>
    it.id === "disfluent-009"
      ? {
          ...it,
          must_remove: it.must_remove.filter((f) => !FILLERS.includes(f)),
        }
      : it,
  );

  async function savedRun(on: Item[] = subset): Promise<SavedReport> {
    const model = fakeModel("rules");
    const results = await evaluate(on, model, {
      levels: ["light"],
      systemPrompt: body,
      guard: stubGuard,
      enforceDeadline: true,
    });
    return {
      meta: {
        date: "2026-10-29",
        model: "fake__trimmed__json",
        endpoint: null,
        dryRun: true,
        guard: "stub",
        levels: ["light"],
        items: subset.length,
        itemsFile: "x",
        itemsSha256: "old",
        systemPromptSha256: sha256(body),
        systemPromptFile: trimmedFile,
        temperature: 0,
        seed: 42,
        enforceDeadline: true,
        durationMs: 1,
        runner: "test",
      },
      summaries: [summarize(results, "light")],
      results,
    };
  }

  test("same labels reproduce the saved scores exactly", async () => {
    const saved = await savedRun();
    const r = await rescoreReport(saved, subset, {
      systemPrompt: body,
      itemsFile: "x",
      itemsSha256: "old",
    });
    expect(r.missing).toEqual([]);
    expect(r.unknown).toEqual([]);
    expect(r.guardChanged).toEqual([]);
    expect(r.report.results).toEqual(saved.results);
    expect(r.report.summaries).toEqual(saved.summaries);
  });

  test("new labels move the scores; missing and unknown outputs are reported", async () => {
    const saved = await savedRun(oldLabels);
    saved.results = saved.results.filter((x) => x.id !== "repair-006");
    saved.results.push({ ...saved.results[0], id: "retired-001" });
    const r = await rescoreReport(saved, subset, {
      systemPrompt: body,
      itemsFile: "x",
      itemsSha256: "new",
    });
    expect(r.missing).toEqual(["light/repair-006"]);
    expect(r.unknown).toEqual(["light/retired-001"]);
    expect(r.report.meta.items).toBe(3);
    expect(r.report.meta.itemsSha256).toBe("new");
    // The two new filler spans join the denominator (repair-006 has none);
    // the rule-based fake leaves both in, so the removed count stays.
    const before = saved.summaries[0].post.fillerRemoval;
    const after = r.report.summaries[0].post.fillerRemoval;
    expect(after.den).toBe(before.den + FILLERS.length);
    expect(after.num).toBe(before.num);
    // Latency and guard outcome are replayed, not re-measured.
    const d = r.report.results.find((x) => x.id === "disfluent-009")!;
    const o = saved.results.find((x) => x.id === "disfluent-009")!;
    expect(d.latencyMs).toBe(o.latencyMs);
    expect(d.guard).toEqual(o.guard);
  });

  test("rescoreDir reads only the source dir and writes reports + summary", async () => {
    const tmp = fs.mkdtempSync(path.join(os.tmpdir(), "wb-rescore-"));
    const src = path.join(tmp, "rs07-x");
    const out = path.join(tmp, "rs07-x-rescored");
    fs.mkdirSync(src);
    const saved = await savedRun();
    saved.results = saved.results.filter((x) => x.id !== "question-001");
    fs.writeFileSync(
      path.join(src, "fake__trimmed__json.json"),
      JSON.stringify(saved),
    );
    const itemsFile = path.join(tmp, "items.jsonl");
    fs.writeFileSync(
      itemsFile,
      subset.map((it) => JSON.stringify(it)).join("\n") + "\n",
    );
    const before = fs.readFileSync(path.join(src, "fake__trimmed__json.json"));
    const r = await rescoreDir(src, out, itemsFile, () => {});
    expect(r.rows.length).toBe(1);
    expect(r.missing).toEqual({ fake__trimmed__json: ["light/question-001"] });
    expect(fs.readdirSync(src)).toEqual(["fake__trimmed__json.json"]);
    expect(fs.readFileSync(path.join(src, "fake__trimmed__json.json"))).toEqual(
      before,
    );
    for (const f of [
      "fake__trimmed__json.json",
      "fake__trimmed__json.md",
      "summary.json",
    ])
      expect(fs.existsSync(path.join(out, f))).toBe(true);
    const summary = JSON.parse(
      fs.readFileSync(path.join(out, "summary.json"), "utf8"),
    );
    expect(summary.meta.missing).toEqual(r.missing);
    expect(summary.rows[0].prompt).toBe("trimmed");
    await expect(rescoreDir(src, src, itemsFile, () => {})).rejects.toThrow();
    // A prompt that changed since the run is refused (prompt-echo guard).
    saved.meta.systemPromptSha256 = "0".repeat(64);
    fs.writeFileSync(
      path.join(src, "fake__trimmed__json.json"),
      JSON.stringify(saved),
    );
    await expect(rescoreDir(src, out, itemsFile, () => {})).rejects.toThrow(
      /changed since the run/,
    );
    fs.rmSync(tmp, { recursive: true, force: true });
  });

  test("CLI: --rescore needs no --server", () => {
    const o = parseCli(["--rescore", "eval/cleanup/reports/rs07-x"])!;
    expect(o.rescore).toBe(path.resolve("eval/cleanup/reports/rs07-x"));
    expect(o.server).toBe("");
  });
});
