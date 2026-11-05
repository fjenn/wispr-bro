// SPDX-License-Identifier: GPL-3.0-or-later
//
// Tests for scripts/wb-upstream-sync.ts (F-09) on throwaway git repos.

import { afterAll, describe, expect, test } from "bun:test";
import { spawnSync } from "node:child_process";
import {
  mkdtempSync,
  readFileSync,
  rmSync,
  writeFileSync,
  mkdirSync,
} from "node:fs";
import { tmpdir } from "node:os";
import { dirname, join } from "node:path";
import {
  extractChecklist,
  hookPaths,
  hotspots,
  renderReport,
  syncIn,
} from "./wb-upstream-sync";
import { REPO_ROOT } from "./wb-guard-lib";

const dirs: string[] = [];
afterAll(() =>
  dirs.forEach((d) => rmSync(d, { recursive: true, force: true })),
);

function sh(cwd: string, ...args: string[]): string {
  const r = spawnSync(
    "git",
    ["-c", "user.name=t", "-c", "user.email=t@t", ...args],
    {
      cwd,
      encoding: "utf8",
    },
  );
  if (r.status !== 0) throw new Error(`git ${args.join(" ")}: ${r.stderr}`);
  return r.stdout.trim();
}

function write(root: string, rel: string, text: string): void {
  mkdirSync(dirname(join(root, rel)), { recursive: true });
  writeFileSync(join(root, rel), text);
}

const LIB = (first: string, handler: string) =>
  `${first}\nfn run() {\n    ${handler}\n}\n`;
/** Upstream changes only the first line (merges cleanly with the fork hook). */
const CLEAN = LIB("fn a() { /* upstream */ }", "builder.invoke_handler(h);");
/** Upstream rewrites the hook line itself (conflicts). */
const CONFLICT = LIB("fn a() {}", "builder.invoke_handler(h).build();");

/** `main` = upstream, `wispr-bro` = fork with one hook; upstream then commits `upstreamLib`. */
function repo(upstreamLib: string): string {
  const root = mkdtempSync(join(tmpdir(), "wb-sync-test-"));
  dirs.push(root);
  sh(root, "init", "-q", "-b", "main");
  write(
    root,
    "src-tauri/src/lib.rs",
    "fn a() {}\nfn run() {\n    builder.invoke_handler(h);\n}\n",
  );
  write(root, "src/i18n/locales/en/translation.json", '{\n  "a": "x"\n}\n');
  sh(root, "add", "-A");
  sh(root, "commit", "-qm", "upstream 1");
  sh(root, "switch", "-qc", "wispr-bro");
  write(
    root,
    "src-tauri/src/lib.rs",
    "fn a() {}\nfn run() {\n    builder.invoke_handler(fork::compose_invoke(h));\n}\n",
  );
  sh(root, "commit", "-qam", "fork hook");
  sh(root, "switch", "-q", "main");
  write(root, "src-tauri/src/lib.rs", upstreamLib);
  write(
    root,
    "src/i18n/locales/en/translation.json",
    '{\n  "a": "x",\n  "b": "Open Handy"\n}\n',
  );
  sh(root, "commit", "-qam", "upstream 2");
  sh(root, "switch", "-q", "wispr-bro");
  return root;
}

describe("syncIn", () => {
  test("clean merge creates a --no-ff merge commit and lists upstream changes", () => {
    const root = repo(CLEAN);
    const r = syncIn({ root, upstream: "main", guards: false });
    expect(r.status).toBe("clean");
    expect(r.commits).toBe(1);
    expect(r.changed).toContain("M\tsrc-tauri/src/lib.rs");
    expect(r.handyStrings).toEqual(['"b": "Open Handy"']);
    expect(
      sh(root, "rev-list", "--parents", "-n1", "HEAD").split(" "),
    ).toHaveLength(3);
    expect(readFileSync(join(root, "src-tauri/src/lib.rs"), "utf8")).toContain(
      "fork::compose_invoke",
    );
  });

  test("conflict aborts the merge and commits nothing", () => {
    const root = repo(CONFLICT);
    const before = sh(root, "rev-parse", "HEAD");
    const r = syncIn({ root, upstream: "main", guards: false });
    expect(r.status).toBe("conflict");
    expect(r.conflicts).toEqual(["src-tauri/src/lib.rs"]);
    expect(sh(root, "rev-parse", "HEAD")).toBe(before);
    expect(sh(root, "status", "--porcelain")).toBe("");
    const body = renderReport(
      r,
      "- [ ] item",
      ["src-tauri/src/lib.rs"],
      "2026-01-05",
      "test-base",
    );
    expect(body).toContain("- `src-tauri/src/lib.rs`");
    expect(body).toContain(
      "git switch -c sync/upstream-2026-01-05 origin/test-base",
    );
    expect(body).toContain("git merge --no-ff upstream/main");
  });

  test("up to date when upstream is already merged", () => {
    const root = repo(CLEAN);
    syncIn({ root, upstream: "main", guards: false });
    expect(syncIn({ root, upstream: "main", guards: false }).status).toBe(
      "uptodate",
    );
  });

  test("refuses a dirty checkout", () => {
    const root = repo(CLEAN);
    write(root, "src-tauri/src/lib.rs", "dirty\n");
    expect(() => syncIn({ root, upstream: "main", guards: false })).toThrow(
      /uncommitted/,
    );
  });
});

describe("report helpers", () => {
  test("the checklist in docs/fork/UPSTREAM-SYNC.md covers every §4.13 item", () => {
    const list = extractChecklist(
      readFileSync(join(REPO_ROOT, "docs/fork/UPSTREAM-SYNC.md"), "utf8"),
    );
    for (const item of [
      "bindings.ts",
      "version",
      "ci/hooks.txt",
      "release.yml",
      "apply_product_settings",
      "wb:brand",
      "upstream-hosts-baseline",
      "protocol.rs",
      "licence register",
      "wb:guards",
    ]) {
      expect(list).toContain(item);
    }
  });

  test("hotspots flag hook files and watched paths only", () => {
    const hooks = hookPaths(
      readFileSync(join(REPO_ROOT, "ci/hooks.txt"), "utf8"),
    );
    expect(hooks).toContain("src-tauri/src/lib.rs");
    const hs = hotspots(
      [
        "M\tsrc-tauri/src/lib.rs",
        "M\t.github/workflows/release.yml",
        "A\tsrc/components/New.tsx",
        "M\tsrc/bindings.ts",
      ],
      hooks,
    );
    expect(hs).toHaveLength(3);
    expect(hs[0]).toContain("hook file");
    expect(hs[1]).toContain("port fixes into wb-*");
  });
});
