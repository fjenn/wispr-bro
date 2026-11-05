// SPDX-License-Identifier: GPL-3.0-or-later
//
// Upstream sync helper (F-09, R5 §4.13). Merges upstream Handy into the fork
// with `--no-ff`, runs the cheap guards (`bun run wb:guards`) on the result
// and renders a Markdown report: upstream changes since the merge base (with
// the hotspots the checklist cares about), conflicts and guard results, plus
// the checklist from docs/fork/UPSTREAM-SYNC.md. Process: docs/fork/UPSTREAM-SYNC.md.
//
//   bun run wb:sync:dryrun                  dry run: merge into a temporary
//                                           worktree of HEAD (detached, never
//                                           pushed), print the report, remove it
//   … --fetch                               `git fetch upstream main` first
//   … --into <ref>                          merge into <ref> instead of HEAD
//   … --upstream <ref>                      default upstream/main
//   … --out <file>                          also write the report to <file>
//   … --base-branch <name>                 fork branch named in the conflict
//                                           commands (default wispr-bro)
//   … --keep                                keep the temporary worktree
//   … --in-place                            CI (wb-upstream-sync.yml): merge in
//                                           this checkout; on conflict abort
//                                           the merge (nothing is committed)
//
// Prints `status=<clean|conflict|uptodate>` last (and appends it to
// $GITHUB_OUTPUT when set). Exit 0 whatever the status (guard failures are
// reported, not fatal: the sync PR must still open); exit 2 on an error.

import { spawnSync } from "node:child_process";
import {
  existsSync,
  mkdtempSync,
  readFileSync,
  appendFileSync,
  rmSync,
  writeFileSync,
} from "node:fs";
import { tmpdir } from "node:os";
import { dirname, join } from "node:path";
import { argValue, git, REPO_ROOT, tryGit } from "./wb-guard-lib";

export type SyncStatus = "clean" | "conflict" | "uptodate";

export interface GuardRun {
  ok: boolean;
  /** One line per guard (`✓ hooks: pass`, `✗ hosts: 2 violation(s)`, …) plus the total. */
  summary: string[];
  output: string;
}

export interface SyncResult {
  status: SyncStatus;
  into: string;
  intoSha: string;
  upstream: string;
  upstreamSha: string;
  mergeBase: string;
  commits: number;
  /** `git diff --name-status <merge base> <upstream>` lines, e.g. `M\tsrc-tauri/src/lib.rs`. */
  changed: string[];
  conflicts: string[];
  /** Added upstream lines containing "Handy" in the English translation. */
  handyStrings: string[];
  guards?: GuardRun;
}

/** Paths the §4.13 checklist reviews by hand when upstream touches them. */
export const WATCHED: [RegExp, string][] = [
  [
    /^\.github\/workflows\/(release|build)\.yml$/,
    "release/build workflow: port fixes into wb-*",
  ],
  [
    /^\.github\/workflows\/(?!wb-)/,
    "upstream workflow: check §4.1 enabled/disabled list",
  ],
  [/^src\/bindings\.ts$/, "regenerate, never hand-merge"],
  [
    /^src-tauri\/src\/settings\.rs$/,
    "defaults vs fork::settings::apply_product_settings",
  ],
  [
    /^src-tauri\/src\/engine_supervisor\/protocol\.rs$/,
    "protocol.rs diff (M2, N-01)",
  ],
  [/^src-tauri\/src\/catalog\//, "catalog: licence register"],
  [
    /^src\/i18n\/locales\/[^/]+\/translation\.json$/,
    "strings: run bun run wb:brand",
  ],
  [/^(src-tauri\/Cargo\.toml|package\.json)$/, "re-apply our version line"],
];

/** Paths listed in ci/hooks.txt (first token of every non-comment line). */
export function hookPaths(hooksTxt: string): string[] {
  return hooksTxt
    .split("\n")
    .map((l) => l.replace(/#.*/, "").trim())
    .filter(Boolean)
    .map((l) => l.split(/\s+/)[0]);
}

/** The checklist between the markers in docs/fork/UPSTREAM-SYNC.md. */
export function extractChecklist(doc: string): string {
  const m = doc.match(
    /<!-- checklist:begin -->\n([\s\S]*?)<!-- checklist:end -->/,
  );
  if (!m)
    throw new Error("docs/fork/UPSTREAM-SYNC.md: checklist markers not found");
  return m[1].trim();
}

/** Annotate upstream changes: hook files from ci/hooks.txt and watched paths. */
export function hotspots(changed: string[], hooks: string[]): string[] {
  const out: string[] = [];
  for (const line of changed) {
    const path = line.split("\t").at(-1)!;
    const notes: string[] = [];
    if (hooks.includes(path))
      notes.push(
        "hook file (ci/hooks.txt): re-check the hook and its line numbers",
      );
    for (const [re, note] of WATCHED) if (re.test(path)) notes.push(note);
    if (notes.length) out.push(`\`${path}\`: ${notes.join("; ")}`);
  }
  return out;
}

function summarizeGuards(output: string, ok: boolean): GuardRun {
  const summary = output
    .split("\n")
    .filter(
      (l) =>
        /^[✓✗-] \S+: /.test(l) || /guard\(s\) failed|all guards passed/.test(l),
    );
  return { ok, summary, output };
}

export function runGuards(root: string, upstream = "upstream/main"): GuardRun {
  if (!existsSync(join(root, "scripts/wb-guards.ts"))) {
    return {
      ok: false,
      summary: ["scripts/wb-guards.ts not found"],
      output: "",
    };
  }
  const r = spawnSync("bun", ["run", "wb:guards"], {
    cwd: root,
    encoding: "utf8",
    env: { ...process.env, WB_UPSTREAM_REF: upstream },
  });
  return summarizeGuards(`${r.stdout ?? ""}${r.stderr ?? ""}`, r.status === 0);
}

const IDENTITY = [
  "-c",
  "user.name=Wispr Bro upstream sync",
  "-c",
  "user.email=upstream-sync@users.noreply.github.com",
];

export interface SyncOptions {
  /** Checkout to merge in; HEAD of it is the merge target. */
  root: string;
  upstream?: string;
  /** Name of what HEAD is, for the report (default: the branch, else "HEAD"). */
  label?: string;
  /** Run the guards after a clean merge (default true). */
  guards?: boolean;
}

/** Merge `upstream` into HEAD of `root` with --no-ff; abort on conflict. */
export function syncIn(opts: SyncOptions): SyncResult {
  const { root } = opts;
  const upstream = opts.upstream ?? "upstream/main";
  const upstreamSha = git(root, [
    "rev-parse",
    "--verify",
    `${upstream}^{commit}`,
  ]).trim();
  const intoSha = git(root, ["rev-parse", "HEAD"]).trim();
  const into =
    opts.label ??
    (tryGit(root, ["symbolic-ref", "--short", "-q", "HEAD"])?.trim() || "HEAD");
  const mergeBase = git(root, ["merge-base", "HEAD", upstreamSha]).trim();
  const changed = git(root, ["diff", "--name-status", mergeBase, upstreamSha])
    .split("\n")
    .filter(Boolean);
  const commits = Number(
    git(root, ["rev-list", "--count", `${mergeBase}..${upstreamSha}`]).trim(),
  );
  const en = "src/i18n/locales/en/translation.json";
  const handyStrings = git(root, [
    "diff",
    "-U0",
    mergeBase,
    upstreamSha,
    "--",
    en,
  ])
    .split("\n")
    .filter((l) => l.startsWith("+") && !l.startsWith("+++") && /Handy/.test(l))
    .map((l) => l.slice(1).trim());
  const base = {
    into,
    intoSha,
    upstream,
    upstreamSha,
    mergeBase,
    commits,
    changed,
    handyStrings,
  };

  const withGuards = opts.guards !== false;
  if (mergeBase === upstreamSha) {
    const guards = withGuards ? runGuards(root, upstream) : undefined;
    return { ...base, status: "uptodate", conflicts: [], guards };
  }
  if (git(root, ["status", "--porcelain", "--untracked-files=no"]).trim()) {
    throw new Error(
      `${root} has uncommitted changes; commit or stash them first`,
    );
  }
  const hasIdentity = tryGit(root, ["config", "user.email"])?.trim();
  const msg = `Merge ${upstream} (${upstreamSha.slice(0, 10)}) into ${into}`;
  const merge = spawnSync(
    "git",
    [
      ...(hasIdentity ? [] : IDENTITY),
      "-c",
      "commit.gpgSign=false",
      "merge",
      "--no-ff",
      "--no-edit",
      "-m",
      msg,
      upstreamSha,
    ],
    { cwd: root, encoding: "utf8" },
  );
  if (merge.status !== 0) {
    const conflicts = git(root, ["diff", "--name-only", "--diff-filter=U"])
      .split("\n")
      .filter(Boolean);
    tryGit(root, ["merge", "--abort"]);
    if (!conflicts.length)
      throw new Error(
        `git merge failed: ${merge.stderr.trim() || merge.stdout.trim()}`,
      );
    return { ...base, status: "conflict", conflicts };
  }
  const guards = withGuards ? runGuards(root, upstream) : undefined;
  return { ...base, status: "clean", conflicts: [], guards };
}

const short = (s: string) => s.slice(0, 10);

function guardSection(L: string[], g: GuardRun | undefined, where: string) {
  if (!g) return;
  L.push(
    "",
    `## Guards (\`bun run wb:guards\` ${where}): ${g.ok ? "pass" : "FAIL"}`,
  );
  L.push("", "```", ...g.summary, "```");
  if (g.ok) return;
  const detail = g.output
    .split("\n")
    .filter((l) => /^\s{2,}\S/.test(l))
    .slice(0, 40);
  L.push("", "<details><summary>Violations</summary>", "", "```");
  L.push(...detail, "```", "", "</details>");
}

export function renderReport(
  r: SyncResult,
  checklist: string,
  hooks: string[],
  date: string,
  baseBranch = "wispr-bro",
): string {
  const L: string[] = [];
  const branch = `sync/upstream-${date}`;
  L.push(
    `Upstream sync (F-09): \`${r.upstream}\` at \`${short(r.upstreamSha)}\` into \`${r.into}\` at \`${short(r.intoSha)}\`.`,
  );
  L.push(
    `Merge base \`${short(r.mergeBase)}\`; ${r.commits} upstream commit(s), ${r.changed.length} file(s) changed upstream. Process: \`docs/fork/UPSTREAM-SYNC.md\`.`,
  );
  if (r.status === "uptodate") {
    L.push("", "Nothing to merge: upstream is already contained in the fork.");
    guardSection(L, r.guards, "on the current tree");
    return L.join("\n");
  }
  if (r.status === "conflict") {
    L.push(
      "",
      `## Merge conflicts (${r.conflicts.length}), nothing was committed`,
      "",
    );
    for (const f of r.conflicts) L.push(`- \`${f}\``);
    L.push(
      "",
      'Resolve locally (hotspot advice: `docs/fork/UPSTREAM-SYNC.md`, "Resolving conflicts"):',
      "",
      "```sh",
      "git remote add upstream https://github.com/cjpais/Handy.git 2>/dev/null || true",
      "git fetch origin && git fetch upstream main",
      `git switch -c ${branch} origin/${baseBranch}`,
      "git merge --no-ff upstream/main",
      "# fix each file above, then: git add <file> && git commit --no-edit",
      "bun run wb:guards",
      `git push -u origin ${branch}`,
      `gh pr create --base ${baseBranch} --head ${branch} --title "chore(sync): merge upstream Handy ${date}"`,
      "```",
      "",
      "Then work through the checklist below in that PR.",
    );
  }
  const hs = hotspots(r.changed, hooks);
  L.push("", "## Upstream changes that need a look", "");
  L.push(
    ...(hs.length
      ? hs.map((h) => `- ${h}`)
      : ["- none of the hook files or watched paths changed"]),
  );
  if (r.handyStrings.length) {
    L.push(
      "",
      'New English strings containing "Handy" (override them with `bun run wb:brand`):',
      "",
    );
    for (const s of r.handyStrings) L.push(`- \`${s.replace(/`/g, "'")}\``);
  }
  guardSection(L, r.guards, "on the merge");
  L.push(
    "",
    "<details><summary>All upstream file changes</summary>",
    "",
    "```",
  );
  L.push(
    ...r.changed.slice(0, 300),
    ...(r.changed.length > 300 ? [`… ${r.changed.length - 300} more`] : []),
  );
  L.push("```", "", "</details>", "", "## Checklist", "", checklist);
  return L.join("\n");
}

function main(argv: string[]): void {
  const upstream = argValue(argv, "--upstream") ?? "upstream/main";
  const out = argValue(argv, "--out");
  const baseBranch = argValue(argv, "--base-branch") ?? "wispr-bro";
  const date = new Date().toISOString().slice(0, 10);
  if (argv.includes("--fetch")) {
    tryGit(REPO_ROOT, [
      "remote",
      "add",
      "upstream",
      "https://github.com/cjpais/Handy.git",
    ]);
    git(REPO_ROOT, ["fetch", "--no-tags", "upstream", "main"]);
  }
  const checklist = extractChecklist(
    readFileSync(join(REPO_ROOT, "docs/fork/UPSTREAM-SYNC.md"), "utf8"),
  );
  const hooks = hookPaths(
    readFileSync(join(REPO_ROOT, "ci/hooks.txt"), "utf8"),
  );

  let root = REPO_ROOT;
  let temp: string | undefined;
  // In CI, HEAD is sync/upstream-<date>, freshly created from the base branch.
  let label: string | undefined = baseBranch;
  if (!argv.includes("--in-place")) {
    const into = argValue(argv, "--into") ?? "HEAD";
    temp = join(mkdtempSync(join(tmpdir(), "wb-sync-")), "tree");
    git(REPO_ROOT, ["worktree", "add", "--detach", temp, into]);
    root = temp;
    label = into;
    console.log(
      `dry run in temporary worktree ${temp} (detached at ${into}; nothing is pushed)`,
    );
  }
  try {
    const result = syncIn({ root, upstream, label });
    const report = renderReport(result, checklist, hooks, date, baseBranch);
    if (out) writeFileSync(out, `${report}\n`);
    console.log(
      `\n${report.replace(/\n## Checklist[\s\S]*/, "\n(checklist: docs/fork/UPSTREAM-SYNC.md)")}`,
    );
    console.log(`\nstatus=${result.status}`);
    if (process.env.GITHUB_OUTPUT)
      appendFileSync(process.env.GITHUB_OUTPUT, `status=${result.status}\n`);
  } finally {
    if (temp && !argv.includes("--keep")) {
      git(REPO_ROOT, ["worktree", "remove", "--force", temp]);
      rmSync(dirname(temp), { recursive: true, force: true });
      console.log(`removed temporary worktree ${temp}`);
    }
  }
}

if (import.meta.main) {
  try {
    main(process.argv.slice(2));
  } catch (e) {
    console.error(`wb-upstream-sync: ${(e as Error).message}`);
    process.exit(2);
  }
}
