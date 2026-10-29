// SPDX-License-Identifier: GPL-3.0-or-later
//
// `cargo clippy -D warnings`, limited to fork code (R5 §4.12 Tests row).
//
// Why not plain `cargo clippy -- -D warnings`: upstream Handy is not
// clippy-clean (11 warnings in lib.rs, managers/model.rs, portable.rs, … at
// the F-03 merge base), and the thin-layer rule forbids fixing them in
// upstream files. So we run clippy on the whole crate with JSON output and
// fail on any warning or error whose primary span is in fork code
// (`src-tauri/src/fork/`, `src-tauri/crates/wb-*`), plus any hard compile
// error anywhere. Upstream warnings are counted and printed, never fatal.
//
// Usage: bun scripts/wb-clippy-fork.ts [extra cargo args…]

import { spawnSync } from "node:child_process";
import { join } from "node:path";
import { REPO_ROOT } from "./wb-guard-lib";

interface Span {
  file_name: string;
  line_start: number;
  is_primary: boolean;
}
interface Message {
  reason: string;
  message?: {
    level: string;
    message: string;
    code?: { code: string } | null;
    spans: Span[];
    rendered?: string;
  };
}

export function isForkSpan(file: string): boolean {
  return /^(src\/fork\/|crates\/wb-)/.test(file.replace(/^\.\//, ""));
}

const extra = process.argv.slice(2);
const res = spawnSync(
  "cargo",
  ["clippy", "--all-targets", "--message-format=json", ...extra],
  {
    cwd: join(REPO_ROOT, "src-tauri"),
    encoding: "utf8",
    maxBuffer: 512 * 1024 * 1024,
    stdio: ["ignore", "pipe", "inherit"],
  },
);

const seen = new Set<string>();
let fork = 0;
let upstream = 0;
let hardErrors = 0;
for (const line of (res.stdout ?? "").split("\n")) {
  if (!line.startsWith("{")) continue;
  let msg: Message;
  try {
    msg = JSON.parse(line) as Message;
  } catch {
    continue;
  }
  const m = msg.message;
  if (msg.reason !== "compiler-message" || !m) continue;
  if (m.level !== "warning" && m.level !== "error") continue;
  const primary = m.spans.find((s) => s.is_primary);
  const key = `${primary?.file_name}:${primary?.line_start}:${m.message}`;
  if (seen.has(key)) continue; // lib and lib-test report the same lint
  seen.add(key);
  if (primary && isForkSpan(primary.file_name)) {
    fork++;
    process.stdout.write(m.rendered ?? `${key}\n`);
  } else if (m.level === "error" && !m.code?.code.startsWith("clippy::")) {
    hardErrors++;
    process.stdout.write(m.rendered ?? `${key}\n`);
  } else if (primary) {
    upstream++;
  }
}

console.log(
  `clippy: ${fork} fork finding(s), ${hardErrors} compile error(s), ${upstream} upstream warning(s) ignored`,
);
if (res.status !== 0 && hardErrors === 0) {
  console.log(`cargo clippy exited with ${res.status}`);
  process.exit(1);
}
process.exit(fork > 0 || hardErrors > 0 ? 1 : 0);
