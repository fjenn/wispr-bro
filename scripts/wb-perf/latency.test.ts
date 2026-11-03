// SPDX-License-Identifier: GPL-3.0-or-later
// Run: bun test scripts/wb-perf/latency.test.ts  (also part of `bun run wb:test:scripts`)
// S-38 latency report: percentiles, the §5.1 rows and their verdicts, the
// busy-Mac refusal.
import { describe, expect, test } from "bun:test";
import { BUDGETS, busyNote, percentile, row, rows } from "./latency";

const meta = {
  asr: "a.gguf",
  llm: "m.gguf",
  debug_build: false,
  cores: 16,
  cold: [{ load_ms: 300, first_run_ms: 150, warm_run_ms: 50 }],
  llm_start_ms: 900,
};

function take(id: string, words: number, asr: number, light: number) {
  return {
    id,
    audio_s: words / 3,
    words,
    asr_ms: asr,
    text_ms: 2,
    light: { ms: light, outcome: "applied", why: null },
    medium: { ms: light + 100, outcome: "applied", why: null },
    transform: { ms: 900, ok: true },
  };
}

describe("latency report", () => {
  test("nearest-rank percentiles", () => {
    expect(percentile([], 0.5)).toBeNull();
    expect(percentile([3, 1, 2], 0.5)).toBe(2);
    expect(percentile([1, 2, 3, 4, 5, 6, 7, 8, 9, 10], 0.95)).toBe(10);
  });

  test("a row passes only when p50 and p95 are both inside", () => {
    expect(row("x", [400, 450, 900], BUDGETS.plain).pass).toBe(true);
    expect(row("x", [400, 450, 1200], BUDGETS.plain).pass).toBe(false);
    expect(row("x", [], BUDGETS.plain).pass).toBeNull();
  });

  test("the rows add the pipeline, cleanup and delivery per take", () => {
    const takes = [take("s1", 10, 100, 500), take("m1", 120, 300, 1500)];
    const r = rows(meta, takes, 40);
    const light = r.find((x) => x.name === "≤ 50 words, Light")!;
    expect(light.p50).toBe(100 + 2 + 40 + 500);
    const long = r.find((x) => x.name.startsWith("51-200"))!;
    expect(long.n).toBe(2);
    const cold = r.find((x) => x.name.startsWith("First take"))!;
    expect(cold.p50).toBe(400);
    expect(r.every((x) => x.pass !== false)).toBe(true);
  });

  test("a busy Mac is not a measurement", () => {
    expect(busyNote(meta, { start: 4, end: 6 })).toBeNull();
    expect(busyNote(meta, { start: 4, end: 9 })).toContain("NOT A MEASUREMENT");
    expect(busyNote(meta, undefined)).toBeNull();
  });
});
