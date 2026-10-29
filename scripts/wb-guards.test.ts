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
import {
  REPO_ROOT,
  failed,
  lexRust,
  listFiles,
  readList,
  rustCfgTestMods,
  type GuardResult,
} from "./wb-guard-lib";
import { mergePatch, runIdentity } from "./wb-guard-identity";
import { runPlumbing } from "./wb-guard-plumbing";
import { runHooks } from "./wb-guard-hooks";
import {
  isExampleName,
  isLoopbackAddress,
  isPrivateAddress,
  runHosts,
} from "./wb-guard-hosts";
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
    write(
      root,
      "ci/inert-hosts.txt",
      "[proven]\nblob.handy.computer # it is fine\n",
    );
    expect(rules(runHosts({ root }))).toEqual(["hosts/list"]);
    write(root, "ci/inert-hosts.txt", "blob.handy.computer # proof: x::y\n");
    expect(rules(runHosts({ root }))).toEqual(["hosts/list"]);
  });
});

describe("hosts guard: [test] hosts", () => {
  const cloud = `${H}api.openai.com/v1`;
  const forkMod = `${SPDX}#[cfg(test)]\nmod tests;\n#[cfg(test)]\n#[allow(dead_code)]\npub(crate) mod testutil;\n`;

  test("allowed in #[cfg(test)]-declared tests.rs / testutil.rs and scripts/wb-*.test.ts", () => {
    const root = fixture({
      "src-tauri/src/fork/mod.rs": forkMod,
      "src-tauri/src/fork/tests.rs": `${SPDX}const U: &str = "${cloud}";\n`,
      "src-tauri/src/fork/testutil.rs": `${SPDX}const U: &str = "${H}mail.example.com/";\n`,
      "src-tauri/src/fork/llm/mod.rs": `${SPDX}#[cfg(test)]\nmod tests;\n`,
      "src-tauri/src/fork/llm/tests.rs": `${SPDX}const U: &str = "http://192.168.1.10:11434/v1";\n`,
      "scripts/wb-thing.test.ts": `${SPDX}const U = "${cloud}";\n`,
    });
    expectPass(runHosts({ root }));
  });

  test("allowed inside a #[cfg(test)] mod block and in comments, not after the block", () => {
    const body =
      `${SPDX}/// Refuses \`${H}blob.handy.computer\` (doc comment).\n` +
      `pub fn f() -> &'static str { "}" }\n` +
      `#[cfg(test)]\nmod tests {\n    const A: &str = "}{";\n    const C: char = '}';\n` +
      `    const R: &str = r#"}" }"#;\n    fn g<'a>(x: &'a str) -> &'a str { x }\n` +
      `    const U: &str = "${cloud}";\n}\n`;
    const root = fixture({ "src-tauri/src/fork/net.rs": body });
    expectPass(runHosts({ root }));

    write(
      root,
      "src-tauri/src/fork/net.rs",
      body + `pub const LEAK: &str = "${cloud}";\n`,
    );
    const r = runHosts({ root });
    expect(rules(r)).toEqual(["hosts/test-only"]);
    expect(r.violations[0].line).toBe(body.split("\n").length);
  });

  test("a [test] host in production fork code fails", () => {
    const root = fixture({
      "src/fork/llm.ts": `${SPDX}fetch("${cloud}");\n`,
      "src-tauri/src/fork/llm/mod.rs": `${SPDX}pub const U: &str = "${cloud}";\n`,
      // Named tests.rs but not declared #[cfg(test)]: production code.
      "src-tauri/src/fork/models/mod.rs": `${SPDX}pub mod tests;\n`,
      "src-tauri/src/fork/models/tests.rs": `${SPDX}pub const U: &str = "${cloud}";\n`,
      // cfg(any(test, …)) is not recognized: fails closed.
      "src-tauri/src/fork/a.rs": `${SPDX}#[cfg(any(test, feature = "x"))]\nmod t {\n    const U: &str = "${cloud}";\n}\n`,
      // A string that only looks like a comment.
      "src-tauri/src/fork/b.rs": `${SPDX}pub const U: &str = "// ${cloud}";\n`,
    });
    const r = runHosts({ root });
    expect(r.violations.map((v) => `${v.file} ${v.rule}`).sort()).toEqual([
      "src-tauri/src/fork/a.rs hosts/test-only",
      "src-tauri/src/fork/b.rs hosts/test-only",
      "src-tauri/src/fork/llm/mod.rs hosts/test-only",
      "src-tauri/src/fork/models/tests.rs hosts/test-only",
      "src/fork/llm.ts hosts/test-only",
    ]);
  });

  test("a [test] host is never accepted upstream or in the bundle", () => {
    const root = fixture({
      "src-tauri/src/foo.rs": `const B: &str = "${H}127.0.0.1.nip.io/v1";\n`,
      "dist/assets/a.js": `fetch("${H}127.0.0.1.nip.io/v1")`,
    });
    expect(rules(runHosts({ root }))).toEqual(["hosts/upstream"]);
    expect(rules(runBundleHosts({ root }))).toEqual(["hosts/bundle"]);
  });

  test("[test] entries need a checked category comment", () => {
    const root = fixture();
    const allowed = readFileSync(join(root, "ci/allowed-hosts.txt"), "utf8");
    for (const bad of [
      "tracker.evil-analytics.net",
      "tracker.evil-analytics.net # just a test",
      "tracker.evil-analytics.net # example: not really",
      "8.8.8.8 # private: public DNS",
      "10.0.0.9 # loopback: no",
      "*.evil-analytics.net # refusal: wildcard",
    ]) {
      write(root, "ci/allowed-hosts.txt", `${allowed}\n${bad}\n`);
      expect(rules(runHosts({ root }))).toEqual(["hosts/list"]);
    }
    write(
      root,
      "ci/allowed-hosts.txt",
      `${allowed}\nfoo.test # example: RFC 6761\n172.20.1.1 # private: LAN\n0x7f000002 # loopback: hex\n`,
    );
    expectPass(runHosts({ root }));
  });

  test("address and example-name helpers", () => {
    expect(isExampleName("my-llm.example.com")).toBe(true);
    expect(isExampleName("*.example.com")).toBe(true);
    expect(isExampleName("x.invalid")).toBe(true);
    expect(isExampleName("example.com.evil.net")).toBe(false);
    expect(isExampleName("notexample.com")).toBe(false);
    expect(isLoopbackAddress("0x7f000001")).toBe(true);
    expect(isLoopbackAddress("2130706433")).toBe(true);
    expect(isLoopbackAddress("127.1.2.3")).toBe(true);
    expect(isLoopbackAddress("128.0.0.1")).toBe(false);
    expect(isPrivateAddress("192.168.1.10")).toBe(true);
    expect(isPrivateAddress("172.32.0.1")).toBe(false);
  });

  test("Rust lexer: cfg(test) modules and comment spans", () => {
    const src =
      `/* outer /* nested } */ */\nfn a() { let s = b"{"; let c = b'}'; }\n` +
      `#[cfg(test)]\nmod t { fn x() { let _ = r##"}"#"##; } }\nfn after() {}\n`;
    const lex = lexRust(src);
    expect(lex.ok).toBe(true);
    expect(lex.comments.length).toBe(1);
    const mods = rustCfgTestMods(src);
    expect(mods.map((m) => [m.name, m.kind])).toEqual([["t", "inline"]]);
    expect(src.slice(...mods[0].span).endsWith("} }")).toBe(true);
    expect(lexRust(`const S: &str = "never closed;`).ok).toBe(false);
    expect(rustCfgTestMods(`#[cfg(test)]\nmod t { "unterminated }`)).toEqual(
      [],
    );
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

  test("lock mode: a lockfile change passes with its manifest in budget, fails otherwise", () => {
    const root = fixture({
      "src-tauri/Cargo.toml":
        '[package]\nname = "handy"\nversion = "0.9.8"\n\n[dependencies]\nrdev = "1"\nserde = "1"\n',
      "src-tauri/Cargo.lock": Array.from(
        { length: 40 },
        (_, i) => `[[package]]\nname = "p${i}"\n`,
      ).join("\n"),
    });
    sh(root, "init", "-q", "-b", "main");
    sh(root, "add", "-A");
    sh(root, "commit", "-q", "-m", "upstream");
    sh(root, "branch", "upstream-main");
    sh(root, "checkout", "-q", "-b", "wispr-bro");
    write(
      root,
      "ci/hooks.txt",
      `src-tauri/Cargo.toml 1 # F-08: removed rdev\nsrc-tauri/Cargo.lock lock # F-08: rdev removal\n`,
    );
    // Lockfile churn alone (manifest untouched) fails.
    const lock = readFileSync(join(root, "src-tauri/Cargo.lock"), "utf8");
    write(
      root,
      "src-tauri/Cargo.lock",
      lock.split("\n").slice(0, 20).join("\n"),
    );
    expect(rules(runHooks({ root, base: "upstream-main" }))).toEqual([
      "hooks/lock",
    ]);

    // With the manifest change (1 line, budget 1) any lockfile change passes.
    const toml = readFileSync(join(root, "src-tauri/Cargo.toml"), "utf8");
    write(root, "src-tauri/Cargo.toml", toml.replace('rdev = "1"\n', ""));
    expectPass(runHooks({ root, base: "upstream-main" }));

    // An over-budget manifest still fails, and the lockfile with it.
    write(
      root,
      "src-tauri/Cargo.toml",
      toml.replace('rdev = "1"\n', 'sneaky = "1"\n'),
    );
    expect(rules(runHooks({ root, base: "upstream-main" })).sort()).toEqual([
      "hooks/budget",
      "hooks/lock",
    ]);

    // A manifest without a numeric budget does not license its lockfile.
    write(root, "src-tauri/Cargo.toml", toml.replace('rdev = "1"\n', ""));
    write(
      root,
      "ci/hooks.txt",
      `src-tauri/Cargo.lock lock # F-08: rdev removal\n`,
    );
    expect(rules(runHooks({ root, base: "upstream-main" })).sort()).toEqual([
      "hooks/lock",
      "hooks/unlisted",
    ]);
  });

  test("lock mode is only for known generated lockfiles", () => {
    const root = hooksFixture();
    write(
      root,
      "ci/hooks.txt",
      "src-tauri/src/lib.rs lock # F-05: not a lockfile\n",
    );
    expect(rules(runHooks({ root, base: "upstream-main" }))).toEqual([
      "hooks/list",
    ]);
  });

  test("the real ci/hooks.txt parses and every entry has a slice comment", () => {
    const r = runHooks({ base: "upstream/main" });
    expect(r.violations.filter((v) => v.rule === "hooks/list")).toEqual([]);
  });
});

describe("the current tree", () => {
  test("every [proven] inert host names Rust tests that exist", () => {
    const rs = listFiles(REPO_ROOT)
      .filter((f) => f.startsWith("src-tauri/src/") && f.endsWith(".rs"))
      .map((f) => readFileSync(join(REPO_ROOT, f), "utf8"))
      .join("\n");
    const proven = readList(REPO_ROOT, "ci/inert-hosts.txt").filter(
      (e) => e.section === "proven",
    );
    expect(proven.length).toBeGreaterThan(0);
    for (const e of proven) {
      const paths = e.comment.match(/\bfork(?:::[a-z_][a-z0-9_]*)+/g) ?? [];
      expect({ host: e.value, paths: paths.length > 0 }).toEqual({
        host: e.value,
        paths: true,
      });
      for (const p of paths) {
        const fn = p.split("::").pop()!;
        const re = new RegExp(
          `#\\[(?:tokio::)?test\\][\\s\\S]{0,200}?fn ${fn}\\b`,
        );
        expect({ host: e.value, test: p, found: re.test(rs) }).toEqual({
          host: e.value,
          test: p,
          found: true,
        });
      }
    }
  });

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
