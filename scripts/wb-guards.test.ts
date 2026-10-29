// SPDX-License-Identifier: GPL-3.0-or-later
//
// Self-tests for the CI guards (F-03): each guard runs on a fixture tree with
// a planted violation (must fail), then on the same tree with it removed
// (must pass). Last, the real tree passes every guard in PR mode and fails in
// release mode only on the F-02 placeholders.
//
// Hosts and package names that the guards reject are assembled at runtime
// (`H + "evil…"`, `"post" + "hog-js"`) so this file itself passes the guards.

import { afterAll, describe, expect, test } from "bun:test";
import { spawnSync } from "node:child_process";
import {
  copyFileSync,
  mkdirSync,
  mkdtempSync,
  readFileSync,
  rmSync,
  writeFileSync,
} from "node:fs";
import { tmpdir } from "node:os";
import { dirname, join } from "node:path";
import { REPO_ROOT, failed, type GuardResult } from "./wb-guard-lib";
import { mergePatch, runIdentity } from "./wb-guard-identity";
import { runPlumbing } from "./wb-guard-plumbing";
import { runHooks } from "./wb-guard-hooks";
import { runHosts } from "./wb-guard-hosts";
import {
  normalizePackedHost,
  printableStrings,
  runBundleHosts,
} from "./wb-guard-bundle-hosts";
import { packageOfSpecifier, runTelemetry } from "./wb-guard-telemetry";
import { runSpdx } from "./wb-guard-spdx";
import { runAll } from "./wb-guards";

const H = "https:" + "//";
const SPDX = "// SPDX-License-Identifier: GPL-3.0-or-later\n";
const dirs: string[] = [];

function fixture(files: Record<string, string> = {}): string {
  const root = mkdtempSync(join(tmpdir(), "wb-guard-"));
  dirs.push(root);
  // Real ci/ lists so the fixtures test the shipped configuration.
  for (const f of [
    "ci/allowed-hosts.txt",
    "ci/upstream-hosts-baseline.txt",
    "ci/inert-hosts.txt",
    "ci/telemetry-packages.txt",
    "ci/generated.txt",
    "ci/upstream-bound.txt",
  ]) {
    mkdirSync(dirname(join(root, f)), { recursive: true });
    copyFileSync(join(REPO_ROOT, f), join(root, f));
  }
  for (const [p, text] of Object.entries(files)) write(root, p, text);
  return root;
}

function write(root: string, rel: string, text: string): void {
  mkdirSync(dirname(join(root, rel)), { recursive: true });
  writeFileSync(join(root, rel), text);
}

function copyFromRepo(root: string, rel: string): void {
  mkdirSync(dirname(join(root, rel)), { recursive: true });
  copyFileSync(join(REPO_ROOT, rel), join(root, rel));
}

function rules(r: GuardResult): string[] {
  return r.violations.map((v) => v.rule);
}

function expectPass(r: GuardResult): void {
  expect(r.violations).toEqual([]);
  expect(failed(r)).toBe(false);
}

function sh(cwd: string, ...args: string[]): void {
  const res = spawnSync(
    "git",
    ["-c", "user.name=t", "-c", "user.email=t@t", ...args],
    {
      cwd,
      encoding: "utf8",
    },
  );
  if (res.status !== 0) throw new Error(`git ${args.join(" ")}: ${res.stderr}`);
}

afterAll(() => {
  for (const d of dirs) rmSync(d, { recursive: true, force: true });
});

describe("telemetry guard", () => {
  const banned = "post" + "hog-js";

  test("a planted analytics import fails, then passes after removal", () => {
    const root = fixture({
      "src/fork/track.ts": `${SPDX}import posthog from "${banned}";\nposthog.init();\n`,
    });
    const r = runTelemetry({ root });
    expect(rules(r)).toEqual(["telemetry/package"]);
    expect(r.violations[0].file).toBe("src/fork/track.ts");
    expect(r.violations[0].message).toContain(banned);

    write(root, "src/fork/track.ts", `${SPDX}export const x = 1;\n`);
    expectPass(runTelemetry({ root }));
  });

  test("unicode-segmentation passes; sentry-core and @segment/* fail", () => {
    const lock = (names: string[]) =>
      names
        .map((n) => `[[package]]\nname = "${n}"\nversion = "1.0.0"\n`)
        .join("\n");
    const root = fixture({
      "src-tauri/Cargo.lock": lock([
        "unicode-segmentation",
        "segment-tree",
        "serde",
      ]),
      "src/fork/a.ts": `${SPDX}import { x } from "@scope/segmentation";\n`,
    });
    expectPass(runTelemetry({ root }));

    write(
      root,
      "src-tauri/Cargo.lock",
      lock(["unicode-segmentation", "sentry" + "-core"]),
    );
    write(
      root,
      "package.json",
      JSON.stringify({
        dependencies: { ["@seg" + "ment/analytics-next"]: "1" },
      }),
    );
    const r = runTelemetry({ root });
    expect(
      r.violations.map((v) => v.message.match(/`([^`]+)`/)?.[1]).sort(),
    ).toEqual(["@seg" + "ment/analytics-next", "sentry" + "-core"]);
  });

  test("Rust use of a banned crate fails (underscore form)", () => {
    const root = fixture({
      "src-tauri/src/fork/x.rs": `${SPDX}use sentry_core::Hub;\n`,
    });
    expect(rules(runTelemetry({ root }))).toEqual(["telemetry/package"]);
  });

  test("specifier → package name", () => {
    expect(packageOfSpecifier("@tauri-apps/api/core")).toBe("@tauri-apps/api");
    expect(packageOfSpecifier("react-dom/client")).toBe("react-dom");
    expect(packageOfSpecifier("@/fork")).toBeNull();
    expect(packageOfSpecifier("./x")).toBeNull();
  });
});

describe("hosts guard", () => {
  test("a planted URL in a fork file fails, then passes after removal", () => {
    const root = fixture({
      "src/fork/net.ts": `${SPDX}export const U = "${H}tracker.evil-analytics.net/collect";\n`,
    });
    const r = runHosts({ root });
    expect(rules(r)).toEqual(["hosts/fork"]);
    expect(r.violations[0]).toMatchObject({ file: "src/fork/net.ts", line: 2 });

    write(
      root,
      "src/fork/net.ts",
      `${SPDX}export const U = "${H}github.com/o/wispr-bro/releases/latest/download/latest.json";\n` +
        `export const M = "${H}cas-bridge.xethub.hf.co/x";\nexport const L = "http://127.0.0.1:8080";\n`,
    );
    expectPass(runHosts({ root }));
  });

  test("fork scan covers the overlay, scripts/wb-* and fork locale files", () => {
    const root = fixture({
      "src-tauri/tauri.fork.conf.json": `{"$schema": "${H}schema.tauri.app/config/2", "x": "${H}cdn.fonts.example.org/a"}`,
      "scripts/wb-thing.ts": `${SPDX}fetch("${H}pastebin.example.org");\n`,
      "src/i18n/locales/en/fork.json": `{"a": "see ${H}help.example.org"}`,
    });
    const files = runHosts({ root })
      .violations.map((v) => v.file)
      .sort();
    expect(files).toEqual([
      "scripts/wb-thing.ts",
      "src-tauri/tauri.fork.conf.json",
      "src/i18n/locales/en/fork.json",
    ]);
  });

  test("[schema] hosts only as $schema; [link] hosts only inside openUrl", () => {
    const root = fixture({
      "src/fork/a.ts": `${SPDX}fetch("${H}schema.tauri.app/config/2");\n`,
    });
    expect(rules(runHosts({ root }))).toEqual(["hosts/schema-only"]);

    const allowed = readFileSync(join(root, "ci/allowed-hosts.txt"), "utf8");
    write(
      root,
      "ci/allowed-hosts.txt",
      allowed.replace("[link]\n", "[link]\nsupport.example.org\n"),
    );
    write(
      root,
      "src/fork/a.ts",
      `${SPDX}fetch("${H}support.example.org/x");\n`,
    );
    expect(rules(runHosts({ root }))).toEqual(["hosts/link-only"]);
    write(
      root,
      "src/fork/a.ts",
      `${SPDX}await openUrl(\n  "${H}support.example.org/x",\n);\n`,
    );
    expectPass(runHosts({ root }));
  });

  test("a new host in an upstream file fails, then passes after removal", () => {
    const root = fixture({
      "src/components/Foo.tsx": `const A = "${H}api.openai.com/v1"; // baseline\n`,
      "src-tauri/src/foo.rs": `const B: &str = "${H}user:pw@example.com/x";\n`,
    });
    expectPass(runHosts({ root }));

    write(
      root,
      "src-tauri/src/foo.rs",
      `const B: &str = "${H}telemetry.newvendor.io/v1";\n`,
    );
    const r = runHosts({ root });
    expect(rules(r)).toEqual(["hosts/upstream"]);
    expect(r.violations[0].file).toBe("src-tauri/src/foo.rs");
    expect(r.violations[0].message).toContain("telemetry.newvendor.io");

    write(
      root,
      "src-tauri/src/foo.rs",
      `const B: &str = "${H}github.com/x";\n`,
    );
    expectPass(runHosts({ root }));
  });

  test("inert entries need a proof comment", () => {
    const root = fixture();
    write(root, "ci/inert-hosts.txt", "[proven]\nblob.handy.computer\n");
    expect(rules(runHosts({ root }))).toEqual(["hosts/list"]);
  });
});

describe("bundle-hosts guard", () => {
  test("skips without a build", () => {
    const r = runBundleHosts({ root: fixture() });
    expect(r.skipped).toContain("no build");
    expect(failed(r)).toBe(false);
  });

  test("a disallowed host in dist JS or binary strings fails; allowed passes", () => {
    const root = fixture({
      "dist/assets/a.js": `fetch("${H}huggingface.co/x");var n="${"http:"}//www.w3.org/2000/svg";`,
    });
    expectPass(runBundleHosts({ root }));

    write(
      root,
      "dist/assets/a.js",
      `fetch("${H}api.mixpanel.example.com/track")`,
    );
    expect(rules(runBundleHosts({ root }))).toEqual(["hosts/bundle"]);
    rmSync(join(root, "dist"), { recursive: true });

    // Fake Mach-O: binary noise around packed string literals.
    const bin = Buffer.concat([
      Buffer.from([0xcf, 0xfa, 0xed, 0xfe, 0, 1, 2]),
      Buffer.from(
        `${H}github.com/o/wispr-bro/releases${H}huggingface.co/apiNextLiteral`,
      ),
      Buffer.from([0, 0, 0xff]),
      Buffer.from(`${H}crash.collector.example.net/upload`),
      Buffer.from([0]),
    ]);
    write(root, "app.bin", "");
    writeFileSync(join(root, "app.bin"), bin);
    const r = runBundleHosts({ root, binaries: ["app.bin"] });
    expect(r.violations.map((v) => v.message.split(" ")[1])).toEqual([
      "crash.collector.example.net",
    ]);
  });

  test("packed-string helpers", () => {
    expect(
      printableStrings(
        new Uint8Array([0, 104, 116, 116, 112, 58, 47, 47, 120, 0]),
        4,
      ),
    ).toEqual(["http:" + "//x"]);
    expect(
      normalizePackedHost("blob.handy.computertranslationauto", [
        "blob.handy.computer",
      ]),
    ).toBe("blob.handy.computer");
    expect(normalizePackedHost("www.years", [])).toBeUndefined();
    expect(normalizePackedHost("evil.example.de", [])).toBe("evil.example.de");
  });
});

describe("spdx guard", () => {
  test("a missing SPDX header fails, then passes after adding it", () => {
    const root = fixture({ "src-tauri/src/fork/new.rs": "pub fn f() {}\n" });
    const r = runSpdx({ root });
    expect(rules(r)).toEqual(["spdx/missing"]);
    expect(r.violations[0].file).toBe("src-tauri/src/fork/new.rs");

    write(root, "src-tauri/src/fork/new.rs", `${SPDX}pub fn f() {}\n`);
    expectPass(runSpdx({ root }));
  });

  test("generated files and JSON are exempt", () => {
    const root = fixture({
      "src/fork/bindings.ts": "// generated\n",
      "src/fork/x.gen.json": "{}",
      "src/fork/data.json": "{}",
    });
    expectPass(runSpdx({ root }));
  });

  test("no GPL header outside fork paths", () => {
    const root = fixture({
      "src-tauri/src/clipboard.rs":
        "// SPDX-License-Identifier: AGPL-3.0-only\nfn x() {}\n",
    });
    expect(rules(runSpdx({ root }))).toEqual(["spdx/upstream-gpl"]);
    write(root, "src-tauri/src/clipboard.rs", "fn x() {}\n");
    expectPass(runSpdx({ root }));
  });

  test("AGPL files must be listed in NOTICE once it exists", () => {
    const agpl = "// SPDX-License-Identifier: AGPL-3.0-only\n";
    const root = fixture({
      "src-tauri/src/fork/diagnostics/sanitizer.rs": `${agpl}fn s() {}\n`,
    });
    const skipped = runSpdx({ root });
    expectPass(skipped);
    expect(skipped.notes.join()).toContain("NOTICE not found");

    write(root, "NOTICE", "Wispr Bro\n");
    expect(rules(runSpdx({ root }))).toEqual(["spdx/agpl-notice"]);
    write(
      root,
      "NOTICE",
      "Voquill: src-tauri/src/fork/diagnostics/sanitizer.rs (AGPL-3.0-only)\n",
    );
    expectPass(runSpdx({ root }));
  });
});

describe("identity guard", () => {
  const identityFixture = () => {
    const root = fixture();
    for (const f of [
      "src-tauri/tauri.conf.json",
      "src-tauri/tauri.fork.conf.json",
      "src-tauri/Cargo.toml",
      "package.json",
    ])
      copyFromRepo(root, f);
    return root;
  };
  const overlayPath = (root: string) =>
    join(root, "src-tauri/tauri.fork.conf.json");
  const editOverlay = (
    root: string,
    f: (o: Record<string, unknown>) => void,
  ) => {
    const o = JSON.parse(readFileSync(overlayPath(root), "utf8"));
    f(o);
    writeFileSync(overlayPath(root), JSON.stringify(o, null, 2));
  };

  test("merge patch follows RFC 7396", () => {
    expect(
      mergePatch(
        { a: { b: 1, c: 2 }, d: [1, 2] },
        { a: { c: null, e: 3 }, d: [9] },
      ),
    ).toEqual({
      a: { b: 1, e: 3 },
      d: [9],
    });
  });

  test("the upstream bundle id in the overlay fails, then passes after removal", () => {
    const root = identityFixture();
    expectPass(runIdentity({ root }));

    editOverlay(root, (o) => (o.identifier = "com.pais.handy"));
    const r = runIdentity({ root });
    expect(rules(r).sort()).toEqual([
      "identity/identifier",
      "identity/upstream",
    ]);

    editOverlay(root, (o) => (o.identifier = "com.wisprbro.desktop"));
    expectPass(runIdentity({ root }));
  });

  test("dropping the overlay's updater block exposes Handy's key and feed", () => {
    const root = identityFixture();
    editOverlay(root, (o) => delete o.plugins);
    const msgs = runIdentity({ root })
      .violations.map((v) => v.message)
      .join("\n");
    expect(msgs).toContain("Handy's updater public key");
    expect(msgs).toContain("cjpais/Handy");
  });

  test("productName and version mismatches fail; tag must match", () => {
    const root = identityFixture();
    editOverlay(root, (o) => {
      o.productName = "Handy";
      o.version = "9.9.9";
    });
    expect(rules(runIdentity({ root })).sort()).toEqual([
      "identity/productName",
      "identity/version",
      "identity/version",
    ]);
    const ok = identityFixture();
    expect(rules(runIdentity({ root: ok, tag: "v0.0.1" }))).toEqual([
      "identity/tag",
    ]);
  });

  test("F-02 placeholders pass on PRs and fail in release mode", () => {
    const root = identityFixture();
    expectPass(runIdentity({ root }));
    const rel = runIdentity({ root, release: true });
    expect(rel.violations.length).toBeGreaterThan(0);
    expect(new Set(rules(rel))).toEqual(new Set(["identity/placeholder"]));

    editOverlay(root, (o) => {
      o.plugins = {
        updater: {
          pubkey: "RWQexample-real-key",
          endpoints: [
            "https://github.com/someone/wispr-bro/releases/latest/download/latest.json",
          ],
        },
      };
    });
    expectPass(runIdentity({ root, release: true }));
  });
});

describe("plumbing guard", () => {
  const plumbingFixture = () => {
    const root = fixture();
    for (const f of [
      "src-tauri/build.rs",
      "src-tauri/src/lib.rs",
      "src/bindings.ts",
    ])
      copyFromRepo(root, f);
    return root;
  };
  const buildRs = (root: string) => join(root, "src-tauri/build.rs");

  test("an AppManifest in build.rs fails, then passes after removal", () => {
    const root = plumbingFixture();
    expectPass(runPlumbing({ root }));
    const original = readFileSync(buildRs(root), "utf8");

    writeFileSync(
      buildRs(root),
      original.replace(
        "    tauri_build::build()\n}",
        '    tauri_build::try_build(\n        tauri_build::Attributes::new()\n            .app_manifest(tauri_build::AppManifest::new().commands(&["fork_ping"])),\n    )\n    .unwrap()\n}',
      ),
    );
    const r = runPlumbing({ root });
    expect(rules(r)).toContain("plumbing/acl-manifest");
    expect(rules(r)).toContain("plumbing/build-rs");

    writeFileSync(buildRs(root), original);
    expectPass(runPlumbing({ root }));
  });

  test("a Handy command named fork_* fails", () => {
    const root = plumbingFixture();
    const lib = join(root, "src-tauri/src/lib.rs");
    const src = readFileSync(lib, "utf8");
    writeFileSync(
      lib,
      src.replace(
        "collect_commands![",
        "collect_commands![\n        commands::fork_oops,",
      ),
    );
    expect(rules(runPlumbing({ root }))).toEqual(["plumbing/fork-prefix"]);
  });

  test("a second or bypassing invoke_handler fails", () => {
    const root = plumbingFixture();
    const lib = join(root, "src-tauri/src/lib.rs");
    const src = readFileSync(lib, "utf8");
    writeFileSync(
      lib,
      src.replace(
        ".invoke_handler(fork::compose_invoke(invoke_handler))",
        ".invoke_handler(invoke_handler)",
      ),
    );
    expect(rules(runPlumbing({ root }))).toEqual(["plumbing/invoke-handler"]);
    write(
      root,
      "src-tauri/src/extra.rs",
      "fn x() { b.invoke_handler(other) }\n",
    );
    writeFileSync(lib, src);
    expect(rules(runPlumbing({ root }))).toEqual([
      "plumbing/invoke-handler",
      "plumbing/invoke-handler",
    ]);
  });

  test("upstream-bound files may not use the fork namespace", () => {
    const root = plumbingFixture();
    write(
      root,
      "ci/upstream-bound.txt",
      "src-tauri/src/shortcut/handy_keys.rs\n",
    );
    write(
      root,
      "src-tauri/src/shortcut/handy_keys.rs",
      "use crate::fork::x;\n",
    );
    expect(rules(runPlumbing({ root }))).toEqual(["plumbing/upstream-bound"]);
    write(root, "src-tauri/src/shortcut/handy_keys.rs", "fn fixed() {}\n");
    expectPass(runPlumbing({ root }));
  });
});

describe("hooks guard", () => {
  const hooksFixture = () => {
    const root = fixture();
    write(
      root,
      "src-tauri/src/lib.rs",
      Array.from({ length: 20 }, (_, i) => `// line ${i}`).join("\n") + "\n",
    );
    write(root, "src-tauri/src/other.rs", "fn other() {}\n");
    sh(root, "init", "-q", "-b", "main");
    sh(root, "add", "-A");
    sh(root, "commit", "-q", "-m", "upstream");
    sh(root, "branch", "upstream-main");
    sh(root, "checkout", "-q", "-b", "wispr-bro");
    // One hook line (1 deleted + 1 added = 2 changed lines).
    const lib = readFileSync(join(root, "src-tauri/src/lib.rs"), "utf8");
    write(
      root,
      "src-tauri/src/lib.rs",
      lib.replace("// line 3", ".plugin(fork::init())"),
    );
    write(root, "ci/hooks.txt", "src-tauri/src/lib.rs 2 # F-05: plugin hook\n");
    write(root, "src-tauri/src/fork/mod.rs", `${SPDX}pub fn init() {}\n`);
    sh(root, "add", "-A");
    sh(root, "commit", "-q", "-m", "fork");
    return root;
  };

  test("within budget passes; an extra changed line fails; removal passes", () => {
    const root = hooksFixture();
    expectPass(runHooks({ root, base: "upstream-main" }));

    const libPath = join(root, "src-tauri/src/lib.rs");
    const lib = readFileSync(libPath, "utf8");
    writeFileSync(
      libPath,
      lib.replace("// line 9", "// line 9\nfork::sneaky();"),
    );
    const r = runHooks({ root, base: "upstream-main" });
    expect(rules(r)).toEqual(["hooks/budget"]);
    expect(r.violations[0].message).toContain("3 changed lines");

    writeFileSync(libPath, lib);
    expectPass(runHooks({ root, base: "upstream-main" }));
  });

  test("an unlisted upstream change, a deletion or a new non-fork file fails", () => {
    const root = hooksFixture();
    write(root, "src-tauri/src/other.rs", "fn other() { changed() }\n");
    write(root, "src-tauri/src/newmod.rs", "fn n() {}\n");
    sh(root, "add", "-A");
    expect(rules(runHooks({ root, base: "upstream-main" })).sort()).toEqual([
      "hooks/new-file",
      "hooks/unlisted",
    ]);

    sh(root, "reset", "-q", "--hard");
    rmSync(join(root, "src-tauri/src/other.rs"));
    expect(rules(runHooks({ root, base: "upstream-main" }))).toEqual([
      "hooks/unlisted",
    ]);
  });

  test("a missing upstream ref fails with the fetch command", () => {
    const root = hooksFixture();
    const r = runHooks({ root, base: "nope/main" });
    expect(rules(r)).toEqual(["hooks/base"]);
    expect(r.violations[0].message).toContain("git fetch upstream");
  });

  test("the real ci/hooks.txt parses and every entry has a slice comment", () => {
    const r = runHooks({ base: "upstream/main" });
    expect(r.violations.filter((v) => v.rule === "hooks/list")).toEqual([]);
  });
});

describe("the current tree", () => {
  test("passes every guard in PR mode", () => {
    const results = runAll({
      base: process.env.WB_UPSTREAM_REF ?? "upstream/main",
    });
    for (const r of results)
      expect({ guard: r.guard, violations: r.violations }).toEqual({
        guard: r.guard,
        violations: [],
      });
  });

  test("fails in release mode only on the F-02 placeholders", () => {
    const results = runAll({
      release: true,
      base: process.env.WB_UPSTREAM_REF ?? "upstream/main",
    });
    const bad = results.filter(failed);
    expect(bad.map((r) => r.guard)).toEqual(["identity (release)"]);
    expect(new Set(bad[0].violations.map((v) => v.rule))).toEqual(
      new Set(["identity/placeholder"]),
    );
  });
});
