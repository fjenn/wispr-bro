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
// `ci/hooks.txt`: `<path> <budget>   # <slice>: <hook>`, or
// `<path> lock   # <slice>: <reason>` for a generated lockfile (LOCKFILES):
// any change to it is accepted (lockfile churn from a dependency change is
// not a hook), but only when its manifest (`Cargo.toml` / `package.json`) is
// itself listed with a numeric budget, changed vs upstream and within that
// budget. So every lockfile change must follow a reviewed manifest change;
// a lockfile change with an untouched manifest (e.g. a bare `cargo update`)
// fails. Lock mode applies only to the files in LOCKFILES.

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

/**
 * Generated lockfiles that may use `lock` mode, and the manifest each one is
 * generated from. Adding to this table is a guard change (reviewed).
 */
export function lockManifest(path: string): string | undefined {
  const dir = path.includes("/")
    ? path.slice(0, path.lastIndexOf("/") + 1)
    : "";
  const base = path.slice(dir.length);
  if (base === "Cargo.lock") return `${dir}Cargo.toml`;
  if (base === "bun.lock") return `${dir}package.json`;
  // bun2nix output regenerated from bun.lock by `scripts/check-nix-deps.ts`
  // (the `postinstall` script): derived from package.json via bun.lock.
  if (path === ".nix/bun.nix" || path === ".nix/bun-lock-hash")
    return "package.json";
  return undefined;
}

export interface HookBudget {
  path: string;
  /** Changed-line budget, or "lock" for a generated lockfile. */
  budget: number | "lock";
  line: number;
  comment: string;
}

export function readHooks(root: string): HookBudget[] {
  return readList(root, "ci/hooks.txt").map((e) => {
    const [path, budget, ...rest] = e.value.split(/\s+/);
    if (!path || rest.length || !/^(\d+|lock)$/.test(budget ?? "")) {
      throw new Error(
        `ci/hooks.txt:${e.line}: expected "<path> <budget>" or "<path> lock", got "${e.value}"`,
      );
    }
    if (budget === "lock" && !lockManifest(path)) {
      throw new Error(
        `ci/hooks.txt:${e.line}: ${path} is not a known generated lockfile; "lock" applies only to Cargo.lock, bun.lock and .nix/bun.nix / .nix/bun-lock-hash`,
      );
    }
    return {
      path,
      budget: budget === "lock" ? "lock" : Number(budget),
      line: e.line,
      comment: e.comment,
    };
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
    } else if (hook.budget === "lock") {
      const manifest = lockManifest(path)!;
      const m = byPath.get(manifest);
      const mn = counts.get(manifest);
      const problem =
        !m || m.budget === "lock"
          ? `its manifest ${manifest} has no numeric budget in ci/hooks.txt`
          : !kinds.has(manifest) || mn === undefined
            ? `its manifest ${manifest} is unchanged vs upstream; a lockfile may change only with a reviewed manifest change`
            : mn === "binary" || mn > m.budget
              ? `its manifest ${manifest} is over budget (${mn} changed lines, budget ${m.budget})`
              : undefined;
      if (problem) {
        r.violations.push({
          file: path,
          rule: "hooks/lock",
          message: `lockfile changed (${n} line(s)) but ${problem} (ci/hooks.txt:${hook.line})`,
        });
      }
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
