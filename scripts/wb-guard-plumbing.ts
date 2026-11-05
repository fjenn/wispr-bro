// SPDX-License-Identifier: GPL-3.0-or-later
//
// Fork plumbing guard (R5 §4.1 Commands row, §4.12).
//   - `src-tauri/build.rs`: `fn main()` ends in plain `tauri_build::build()`
//     and nothing defines an `AppManifest` (or an `InlinedPlugin`), so fork app
//     commands keep needing no ACL permission (DEC-46).
//   - No Handy command in `collect_commands![…]` (lib.rs) or in the generated
//     `src/bindings.ts` starts with `fork_` (compose_invoke routes by prefix).
//   - Exactly one `.invoke_handler(…)` call in upstream Rust code, and it is
//     `.invoke_handler(fork::compose_invoke(invoke_handler))`.
//   - Upstream-clean: files in `ci/upstream-bound.txt` never mention
//     `crate::fork`/`fork::` and carry no GPL/AGPL header.
//
// These mirror the Rust tests in `src-tauri/src/fork/testutil.rs` so they
// fail fast without a cargo build.

import {
  cliMain,
  isForkCodePath,
  listFiles,
  newResult,
  readIfExists,
  readList,
  REPO_ROOT,
  stripRustComments,
  type GuardResult,
} from "./wb-guard-lib";

export const GPL_HEADER_RE =
  /SPDX-License-Identifier:\s*\(?\s*A?GPL|GNU (Affero )?General Public License/i;

/** Body of `fn main() { … }`, comments stripped, or undefined. */
export function buildMainBody(buildRs: string): string | undefined {
  const src = stripRustComments(buildRs);
  const m = src.match(/\bfn\s+main\s*\(\s*\)\s*\{/);
  if (!m || m.index === undefined) return undefined;
  let depth = 0;
  const start = m.index + m[0].length - 1;
  for (let i = start; i < src.length; i++) {
    if (src[i] === "{") depth++;
    else if (src[i] === "}") {
      depth--;
      if (depth === 0) return src.slice(start + 1, i);
    }
  }
  return undefined;
}

export function handyCommandNames(libRs: string): string[] | undefined {
  const start = libRs.indexOf("collect_commands![");
  if (start < 0) return undefined;
  const from = start + "collect_commands![".length;
  const end = libRs.indexOf("]", from);
  if (end < 0) return undefined;
  return stripRustComments(libRs.slice(from, end))
    .split(",")
    .map((s) => s.trim())
    .filter(Boolean)
    .map((p) => p.split("::").pop()!.replace(/<.*$/, "").trim());
}

export function runPlumbing(opts: { root?: string } = {}): GuardResult {
  const root = opts.root ?? REPO_ROOT;
  const r = newResult("plumbing");
  const v = (file: string, rule: string, message: string, line?: number) =>
    r.violations.push({ file, rule, message, line });

  // build.rs
  const buildRs = readIfExists(root, "src-tauri/build.rs");
  if (buildRs === undefined) {
    v("src-tauri/build.rs", "plumbing/build-rs", "file is missing");
  } else {
    const body = buildMainBody(buildRs);
    if (body === undefined) {
      v("src-tauri/build.rs", "plumbing/build-rs", "no `fn main() { … }`");
    } else if (!/(^|[;}\s])tauri_build::build\(\)\s*;?\s*$/.test(body)) {
      v(
        "src-tauri/build.rs",
        "plumbing/build-rs",
        "fn main() must end in plain `tauri_build::build()` (no try_build / Attributes)",
      );
    }
    const code = stripRustComments(buildRs);
    for (const word of ["AppManifest", "app_manifest", "InlinedPlugin"]) {
      const idx = code.indexOf(word);
      if (idx >= 0) {
        v(
          "src-tauri/build.rs",
          "plumbing/acl-manifest",
          `defines \`${word}\`: an app ACL manifest makes Tauri check permissions for every fork_* command (R5 §4.1)`,
          code.slice(0, idx).split("\n").length,
        );
      }
    }
  }

  // Handy command names.
  const libRs = readIfExists(root, "src-tauri/src/lib.rs");
  if (libRs === undefined) {
    v("src-tauri/src/lib.rs", "plumbing/commands", "file is missing");
  } else {
    const names = handyCommandNames(libRs);
    if (!names) {
      v(
        "src-tauri/src/lib.rs",
        "plumbing/commands",
        "collect_commands![…] not found",
      );
    } else {
      for (const n of names.filter((n) => n.startsWith("fork_"))) {
        v(
          "src-tauri/src/lib.rs",
          "plumbing/fork-prefix",
          `Handy command \`${n}\` starts with fork_ (reserved for fork commands)`,
        );
      }
    }
  }
  const bindings = readIfExists(root, "src/bindings.ts");
  if (bindings) {
    for (const m of bindings.matchAll(/TAURI_INVOKE\("(fork_[^"]*)"/g)) {
      v(
        "src/bindings.ts",
        "plumbing/fork-prefix",
        `Handy bindings contain fork command \`${m[1]}\``,
      );
    }
  }

  // invoke_handler calls in upstream Rust code.
  const calls: { file: string; line: number; text: string }[] = [];
  for (const f of listFiles(root)) {
    if (!f.startsWith("src-tauri/src/") || !f.endsWith(".rs")) continue;
    if (isForkCodePath(f)) continue;
    const src = stripRustComments(readIfExists(root, f) ?? "");
    // `.invoke_handler(` with an argument; specta's `.invoke_handler()`
    // only creates Handy's handler.
    for (const m of src.matchAll(/\.invoke_handler\(\s*([^)\s][^\n]*)/g)) {
      calls.push({
        file: f,
        line: src.slice(0, m.index).split("\n").length,
        text: m[1].trim(),
      });
    }
  }
  if (calls.length !== 1) {
    for (const c of calls)
      v(
        c.file,
        "plumbing/invoke-handler",
        `one .invoke_handler(…) call expected, found ${calls.length}`,
        c.line,
      );
    if (calls.length === 0)
      v(
        "src-tauri/src/lib.rs",
        "plumbing/invoke-handler",
        "no .invoke_handler(…) call found",
      );
  } else if (
    !/^fork::compose_invoke\(\s*invoke_handler\s*\)\s*\)/.test(calls[0].text)
  ) {
    v(
      calls[0].file,
      "plumbing/invoke-handler",
      `the invoke_handler must be \`fork::compose_invoke(invoke_handler)\`, found \`${calls[0].text}\``,
      calls[0].line,
    );
  }

  // Upstream-bound files.
  let bound: string[] = [];
  try {
    bound = readList(root, "ci/upstream-bound.txt").map((e) => e.value);
  } catch {
    v(
      "ci/upstream-bound.txt",
      "plumbing/upstream-bound",
      "list file is missing",
    );
  }
  for (const f of bound) {
    const text = readIfExists(root, f);
    if (text === undefined) {
      v(
        f,
        "plumbing/upstream-bound",
        "listed in ci/upstream-bound.txt but missing",
      );
      continue;
    }
    const code = f.endsWith(".rs") ? stripRustComments(text) : text;
    if (/\bcrate::fork\b|\bfork::|from\s+["']@\/fork/.test(code)) {
      v(
        f,
        "plumbing/upstream-bound",
        "upstream-bound file must not use the fork namespace",
      );
    }
    if (GPL_HEADER_RE.test(text.split("\n").slice(0, 30).join("\n"))) {
      v(
        f,
        "plumbing/upstream-bound",
        "upstream-bound file must not carry a GPL/AGPL header",
      );
    }
  }
  return r;
}

if (import.meta.main) cliMain(() => runPlumbing());
