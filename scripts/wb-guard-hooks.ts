// SPDX-License-Identifier: GPL-3.0-or-later
//
// Hook-size guard (R5 §4.2, §4.12; thin-layer rule §1).
//
// Compares the working tree with the merge base of HEAD and the upstream ref
// (default `upstream/main`, override with --base <ref> or $WB_UPSTREAM_REF):
//   - every changed file that exists upstream (modified, deleted or renamed
//     away) must be listed in `ci/hooks.txt`, and its changed-line count
//     (added + deleted, `git diff --numstat`) must be ≤ the listed budget;
//   - every added file must be in a fork path (wb-guard-lib FORK_PATHS) or be
//     listed in `ci/hooks.txt`.
//
// `ci/hooks.txt`: `<path> <budget>   # <slice>: <hook>`.

import {
  argValue,
  cliMain,
  git,
  isForkPath,
  newResult,
  readList,
  REPO_ROOT,
  tryGit,
  type GuardResult,
} from "./wb-guard-lib";

export interface HookBudget {
  path: string;
  budget: number;
  line: number;
  comment: string;
}

export function readHooks(root: string): HookBudget[] {
  return readList(root, "ci/hooks.txt").map((e) => {
    const [path, budget, ...rest] = e.value.split(/\s+/);
    if (!path || rest.length || !/^\d+$/.test(budget ?? "")) {
      throw new Error(
        `ci/hooks.txt:${e.line}: expected "<path> <budget>", got "${e.value}"`,
      );
    }
    return { path, budget: Number(budget), line: e.line, comment: e.comment };
  });
}

export interface HooksOptions {
  root?: string;
  base?: string;
}

export function runHooks(opts: HooksOptions = {}): GuardResult {
  const root = opts.root ?? REPO_ROOT;
  const ref = opts.base ?? process.env.WB_UPSTREAM_REF ?? "upstream/main";
  const r = newResult("hooks");

  let hooks: HookBudget[];
  try {
    hooks = readHooks(root);
  } catch (e) {
    r.violations.push({
      file: "ci/hooks.txt",
      rule: "hooks/list",
      message: (e as Error).message,
    });
    return r;
  }
  const byPath = new Map(hooks.map((h) => [h.path, h]));
  for (const h of hooks) {
    if (!h.comment) {
      r.violations.push({
        file: "ci/hooks.txt",
        line: h.line,
        rule: "hooks/list",
        message: `${h.path}: every entry needs a "# <slice>: <hook>" comment`,
      });
    }
  }

  if (!tryGit(root, ["rev-parse", "--verify", "--quiet", `${ref}^{commit}`])) {
    r.violations.push({
      file: "ci/hooks.txt",
      rule: "hooks/base",
      message: `upstream ref ${ref} not found; run \`git remote add upstream https://github.com/cjpais/Handy.git && git fetch upstream main\` (or pass --base)`,
    });
    return r;
  }
  const base = git(root, ["merge-base", "HEAD", ref]).trim();
  r.notes.push(`base ${ref} (merge base ${base.slice(0, 10)})`);

  // Working tree vs base; untracked files are not part of a PR and are ignored.
  const status = git(root, [
    "diff",
    "--no-renames",
    "--name-status",
    "-z",
    base,
  ]).split("\0");
  const kinds = new Map<string, string>();
  for (let i = 0; i + 1 < status.length; i += 2) {
    if (status[i]) kinds.set(status[i + 1], status[i][0]);
  }
  const numstat = git(root, ["diff", "--no-renames", "--numstat", "-z", base])
    .split("\0")
    .filter(Boolean);
  const counts = new Map<string, number | "binary">();
  for (const row of numstat) {
    const [a, d, ...p] = row.split("\t");
    const path = p.join("\t");
    counts.set(path, a === "-" || d === "-" ? "binary" : Number(a) + Number(d));
  }

  const used = new Set<string>();
  for (const [path, kind] of [...kinds].sort()) {
    const hook = byPath.get(path);
    if (hook) used.add(path);
    const n = counts.get(path) ?? 0;
    if (kind === "A") {
      if (!isForkPath(path) && !hook) {
        r.violations.push({
          file: path,
          rule: "hooks/new-file",
          message:
            "new file outside the fork namespace (R5 §4.1); move it under a fork path or list it in ci/hooks.txt",
        });
      }
      continue;
    }
    // Modified, deleted or type-changed upstream file.
    if (!hook) {
      r.violations.push({
        file: path,
        rule: "hooks/unlisted",
        message: `upstream file changed (${n} line(s), status ${kind}) but not listed in ci/hooks.txt (R5 §4.2)`,
      });
    } else if (n === "binary") {
      r.violations.push({
        file: path,
        rule: "hooks/binary",
        message: "binary upstream file changed; hooks must be text",
      });
    } else if (n > hook.budget) {
      r.violations.push({
        file: path,
        rule: "hooks/budget",
        message: `${n} changed lines vs upstream, budget is ${hook.budget} (ci/hooks.txt:${hook.line}, ${hook.comment})`,
      });
    }
  }
  for (const h of hooks) {
    if (!used.has(h.path)) {
      r.notes.push(`${h.path}: listed but unchanged vs upstream`);
    }
  }
  return r;
}

if (import.meta.main) {
  const argv = process.argv.slice(2);
  cliMain(() => runHooks({ base: argValue(argv, "--base") }));
}
