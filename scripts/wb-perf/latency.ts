// SPDX-License-Identifier: GPL-3.0-or-later
/**
 * S-38: the §5.1 latency rows from a `fork::perf_bench` run, with PASS/FAIL
 * against R5's Apple Silicon budgets. Exit code 1 when a row fails.
 *
 *   bun scripts/wb-perf/latency.ts <run.jsonl> [--delivery-ms 41.7]
 *
 * Key-up → text per take = recognizer on the whole take + text pipeline
 * (+ cleanup) + delivery. Delivery is not in the headless run: S-04's soak
 * measured text in the field 41.7 ms p95 after the paste started
 * (`docs/fork/test-runs/1.0.0-alpha.1-M-26.md`), added to every take.
 * fn-down → capture (RS-01, S-03) and the AX read at key-up are GUI and not
 * here.
 */
import fs from "fs";

export type Budget = { p50: number; p95: number };
export type Row = {
  name: string;
  n: number;
  p50: number | null;
  p95: number | null;
  budget: Budget;
  pass: boolean | null;
  note?: string;
};

type Cleanup = { ms: number; outcome: string; why: string | null };
type Take = {
  id: string;
  audio_s: number;
  words: number;
  asr_ms: number;
  text_ms: number;
  light?: Cleanup;
  medium?: Cleanup;
  transform?: { ms: number; ok: boolean };
};
type Cold = { load_ms: number; first_run_ms: number; warm_run_ms: number };
type Meta = {
  asr: string;
  asr_backend?: string;
  cores?: number;
  llm: string | null;
  debug_build: boolean;
  cold: Cold[];
  llm_start_ms: number | null;
};

/** Nearest-rank percentile (the other bench scripts' method). */
export function percentile(values: number[], p: number): number | null {
  if (values.length === 0) return null;
  const v = [...values].sort((a, b) => a - b);
  return v[Math.min(v.length - 1, Math.round((v.length - 1) * p))];
}

export function row(
  name: string,
  values: number[],
  budget: Budget,
  note?: string,
): Row {
  const p50 = percentile(values, 0.5);
  const p95 = percentile(values, 0.95);
  return {
    name,
    n: values.length,
    p50,
    p95,
    budget,
    pass:
      p50 === null || p95 === null
        ? null
        : p50 <= budget.p50 && p95 <= budget.p95,
    note,
  };
}

export const BUDGETS = {
  plain: { p50: 500, p95: 1000 },
  light: { p50: 1300, p95: 2200 },
  medium: { p50: 1600, p95: 2600 },
  long: { p50: 2500, p95: 6000 },
  transform: { p50: 2000, p95: 3500 },
  asrCold: { p50: 800, p95: 1500 },
  llmCold: { p50: 1500, p95: 2500 },
} satisfies Record<string, Budget>;

export function rows(meta: Meta, takes: Take[], deliveryMs: number): Row[] {
  const base = (t: Take) => t.asr_ms + t.text_ms + deliveryMs;
  const short = takes.filter((t) => t.words <= 50);
  const long = takes.filter((t) => t.words > 50 && t.words <= 200);
  const out: Row[] = [
    row(
      "≤ 20 s take, cleanup None or deterministic",
      takes.filter((t) => t.audio_s <= 20).map(base),
      BUDGETS.plain,
    ),
  ];
  const withLlm = takes.some((t) => t.light);
  if (withLlm) {
    const lvl = (ts: Take[], k: "light" | "medium") =>
      ts.filter((t) => t[k]).map((t) => base(t) + t[k]!.ms);
    out.push(row("≤ 50 words, Light", lvl(short, "light"), BUDGETS.light));
    out.push(row("≤ 50 words, Medium", lvl(short, "medium"), BUDGETS.medium));
    out.push(
      row(
        "51-200 words, any cleanup",
        [...lvl(long, "light"), ...lvl(long, "medium")],
        BUDGETS.long,
        "p95 budget is the 6 s deadline, then fail-open",
      ),
    );
    out.push(
      row(
        "Transform on ≤ 150 words (generation + delivery)",
        takes
          .filter((t) => t.transform)
          .map((t) => t.transform!.ms + deliveryMs),
        BUDGETS.transform,
        "selection capture is GUI and not included",
      ),
    );
  }
  out.push(
    row(
      "First take after ASR unload (extra)",
      meta.cold.map((c) => c.load_ms + c.first_run_ms - c.warm_run_ms),
      BUDGETS.asrCold,
    ),
  );
  if (meta.llm_start_ms !== null) {
    out.push(
      row(
        "LLM cold start to ready (cold-cold row is M1 8 GB)",
        [meta.llm_start_ms],
        BUDGETS.llmCold,
        "one sample; the ordered load overlaps it with speaking",
      ),
    );
  }
  return out;
}

/**
 * A run on a busy Mac (1-minute load above half the cores at the start or
 * the end) measured the other work: the report says so and is not a
 * verdict (exit 3).
 */
export function busyNote(
  meta: Meta,
  load: { start: number; end: number } | undefined,
): string | null {
  if (!load || !meta.cores) return null;
  const peak = Math.max(load.start, load.end);
  return peak > meta.cores / 2
    ? `NOT A MEASUREMENT: load average ${peak.toFixed(1)} on ${meta.cores} cores (limit ${meta.cores / 2}). Re-run on a quiet Mac.`
    : null;
}

function fmt(v: number | null): string {
  return v === null ? "–" : `${Math.round(v)}`;
}

export function report(meta: Meta, takes: Take[], deliveryMs: number): string {
  const lines = [
    `# §5.1 latency (S-38 headless bench)`,
    ``,
    `ASR ${meta.asr}${meta.asr_backend ? ` (${meta.asr_backend})` : ""}, LLM ${meta.llm ?? "none"}, ${takes.length} takes, delivery +${deliveryMs} ms, ${meta.debug_build ? "debug" : "release"} Rust build (native engines are always optimized).`,
    ``,
    `| Row | n | p50 ms | p95 ms | Budget p50/p95 | Result |`,
    `| --- | --- | --- | --- | --- | --- |`,
  ];
  for (const r of rows(meta, takes, deliveryMs)) {
    const result = r.pass === null ? "no data" : r.pass ? "PASS" : "FAIL";
    lines.push(
      `| ${r.name}${r.note ? ` (${r.note})` : ""} | ${r.n} | ${fmt(r.p50)} | ${fmt(r.p95)} | ${r.budget.p50}/${r.budget.p95} | ${result} |`,
    );
  }
  const fellOpen = takes.flatMap((t) =>
    (["light", "medium"] as const)
      .filter((k) => t[k] && t[k]!.outcome !== "applied")
      .map(
        (k) =>
          `${t.id}/${k}: ${t[k]!.outcome}${t[k]!.why ? ` (${t[k]!.why})` : ""}`,
      ),
  );
  if (fellOpen.length) {
    lines.push("", `Cleanups not applied: ${fellOpen.join(", ")}`);
  }
  return lines.join("\n");
}

if (import.meta.main) {
  const args = process.argv.slice(2);
  const file = args.find((a) => !a.startsWith("--"));
  const i = args.indexOf("--delivery-ms");
  const deliveryMs = i >= 0 ? Number(args[i + 1]) : 41.7;
  if (!file) {
    console.error(
      "usage: bun scripts/wb-perf/latency.ts <run.jsonl> [--delivery-ms N]",
    );
    process.exit(2);
  }
  const lines = fs
    .readFileSync(file, "utf8")
    .split("\n")
    .filter((l) => l.trim())
    .map((l) => JSON.parse(l));
  const meta = lines[0].meta as Meta;
  const load = lines.find((l) => l.load)?.load as
    | { start: number; end: number }
    | undefined;
  const takes = lines.slice(1).filter((l) => l.id) as Take[];
  console.log(report(meta, takes, deliveryMs));
  const busy = busyNote(meta, load);
  if (busy) {
    console.log(`\n${busy}`);
    process.exit(3);
  }
  if (rows(meta, takes, deliveryMs).some((r) => r.pass === false))
    process.exit(1);
}
