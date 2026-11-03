// SPDX-License-Identifier: GPL-3.0-or-later
//
// Runs every Wispr Bro CI guard (R5 §4.12) and exits 1 if any fails.
//
//   bun run wb:guards                 PR mode (F-02 placeholders allowed)
//   bun run wb:guards -- --release    release mode (placeholders fail)
//   … --base <ref>                    upstream ref for the hook guard
//   … --binary <path>                 also scan a built Mach-O / .app
//
// Each guard also runs alone: `bun run wb:guard:<name>`.

import {
  argValue,
  failed,
  printResult,
  type GuardResult,
} from "./wb-guard-lib";
import { runIdentity } from "./wb-guard-identity";
import { runPlumbing } from "./wb-guard-plumbing";
import { runHooks } from "./wb-guard-hooks";
import { runHosts } from "./wb-guard-hosts";
import { runBundleHosts } from "./wb-guard-bundle-hosts";
import { runTelemetry } from "./wb-guard-telemetry";
import { runSpdx } from "./wb-guard-spdx";
import { runIcons } from "./wb-guard-icons";

export interface AllOptions {
  root?: string;
  release?: boolean;
  base?: string;
  binaries?: string[];
}

export function runAll(opts: AllOptions = {}): GuardResult[] {
  const { root } = opts;
  const guards: [string, () => GuardResult][] = [
    ["identity", () => runIdentity({ root, release: opts.release })],
    ["plumbing", () => runPlumbing({ root })],
    ["hooks", () => runHooks({ root, base: opts.base })],
    ["hosts", () => runHosts({ root })],
    ["bundle-hosts", () => runBundleHosts({ root, binaries: opts.binaries })],
    ["telemetry", () => runTelemetry({ root })],
    ["spdx", () => runSpdx({ root })],
    ["icons", () => runIcons({ root })],
  ];
  return guards.map(([name, run]) => {
    try {
      return run();
    } catch (e) {
      return {
        guard: name,
        violations: [
          {
            file: "-",
            rule: `${name}/crash`,
            message: String((e as Error).stack ?? e),
          },
        ],
        notes: [],
      };
    }
  });
}

if (import.meta.main) {
  const argv = process.argv.slice(2);
  const binaries: string[] = [];
  argv.forEach((a, i) => {
    if (a === "--binary" && argv[i + 1]) binaries.push(argv[i + 1]);
  });
  const started = performance.now();
  const results = runAll({
    release: argv.includes("--release"),
    base: argValue(argv, "--base"),
    binaries,
  });
  for (const r of results) printResult(r);
  const bad = results.filter(failed);
  const secs = ((performance.now() - started) / 1000).toFixed(1);
  if (bad.length) {
    console.log(
      `\n${bad.length} guard(s) failed: ${bad.map((r) => r.guard).join(", ")} (${secs}s)`,
    );
    process.exit(1);
  }
  console.log(`\nall guards passed (${secs}s)`);
}
