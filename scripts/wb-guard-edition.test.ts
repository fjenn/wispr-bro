// SPDX-License-Identifier: GPL-3.0-or-later
//
// Self-tests for the edition guard (docs/fork/EDITIONS.md): each planted leak
// of publik code into the normal build must fail, and the clean tree passes.
// Markers are assembled at runtime so this file holds none.

import { afterAll, describe, expect, test } from "bun:test";
import { mkdirSync, mkdtempSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { dirname, join } from "node:path";
import { failed } from "./wb-guard-lib";
import { runHosts } from "./wb-guard-hosts";
import {
  PUBLIK_MARKERS,
  PUBLIK_REQUIRED,
  runEdition,
} from "./wb-guard-edition";

const DOMAIN = PUBLIK_MARKERS[0] + ".com";
const dirs: string[] = [];

function tree(files: Record<string, string>): string {
  const root = mkdtempSync(join(tmpdir(), "wb-edition-"));
  dirs.push(root);
  const base: Record<string, string> = {
    "src-tauri/Cargo.toml": "[features]\npublik = []\n",
    "src-tauri/tauri.fork.conf.json": "{}",
    "src-tauri/src/fork/mod.rs":
      '#[cfg(feature = "publik")]\npub mod publik;\n',
    "src-tauri/src/fork/publik/mod.rs": `// ${DOMAIN}\n`,
    "src/fork/publik/Gate.tsx": `export const url = "${DOMAIN}";\n`,
    "src/fork/edition.ts":
      'export const load = () => import("./publik/Gate");\n',
    "docs/fork/EDITIONS.md": `${DOMAIN}\n`,
    ...files,
  };
  for (const [rel, body] of Object.entries(base)) {
    const p = join(root, rel);
    mkdirSync(dirname(p), { recursive: true });
    writeFileSync(p, body);
  }
  return root;
}

afterAll(() => {
  for (const d of dirs) rmSync(d, { recursive: true, force: true });
});

describe("edition guard: sources", () => {
  test("a clean tree passes", () => {
    expect(failed(runEdition({ root: tree({}) }))).toBe(false);
  });

  test("a marker outside the publik folders fails", () => {
    const r = runEdition({
      root: tree({
        "src-tauri/src/fork/net.rs": `const H: &str = "${DOMAIN}";\n`,
      }),
    });
    expect(r.violations.map((v) => v.rule)).toContain("edition/marker");
  });

  test("an ungated or negated Rust reference fails", () => {
    for (const body of [
      "pub mod publik;\n",
      '#[cfg(not(feature = "publik"))]\nuse crate::fork::publik::X;\n',
    ]) {
      const r = runEdition({
        root: tree({ "src-tauri/src/fork/mod.rs": body }),
      });
      expect(r.violations.map((v) => v.rule)).toContain("edition/rust-cfg");
    }
  });

  test("a frontend import outside the gateway fails", () => {
    const r = runEdition({
      root: tree({
        "src/App.tsx": 'import { Gate } from "./fork/publik/Gate";\n',
      }),
    });
    expect(r.violations.map((v) => v.rule)).toContain("edition/import");
  });

  test("turning the feature on by default or in a normal config fails", () => {
    const a = runEdition({
      root: tree({
        "src-tauri/Cargo.toml":
          '[features]\ndefault = ["publik"]\npublik = []\n',
      }),
    });
    expect(a.violations.map((v) => v.rule)).toContain("edition/default");
    const b = runEdition({
      root: tree({
        "src-tauri/tauri.fork.conf.json": '{"build":{"features":["publik"]}}',
      }),
    });
    expect(b.violations.map((v) => v.rule)).toContain("edition/config");
  });
});

describe("edition guard: what a normal user sees", () => {
  const WORD = "Pub" + "lik";
  test("the word in normal UI text or translations fails", () => {
    for (const [rel, body] of [
      ["src/i18n/locales/en/fork.json", `{"about": "Sign in with ${WORD}"}`],
      ["src/fork/settings/About.tsx", `<p>${WORD} edition</p>`],
    ]) {
      const r = runEdition({ root: tree({ [rel]: body }) });
      expect(r.violations.map((v) => v.rule)).toContain("edition/text");
    }
  });

  test("a Rust string mentioning it fails; the cfg itself does not", () => {
    const bad = runEdition({
      root: tree({
        "src-tauri/src/fork/tray.rs": `const LABEL: &str = "${WORD} account";\n`,
      }),
    });
    expect(bad.violations.map((v) => v.rule)).toContain("edition/text");
  });

  test("a normal build with the word anywhere fails", () => {
    const root = tree({});
    const dist = join(root, "dist-word");
    mkdirSync(dist);
    writeFileSync(join(dist, "index.js"), `t("${WORD} edition")`);
    expect(failed(runEdition({ edition: "standard", dist }))).toBe(true);
  });
});

describe("edition guard: artifacts", () => {
  test("a normal build with a marker fails, without one passes", () => {
    const root = tree({});
    const dirty = join(root, "dist-dirty");
    mkdirSync(dirty);
    writeFileSync(join(dirty, "index.js"), `fetch("https://${DOMAIN}/x")`);
    expect(failed(runEdition({ edition: "standard", dist: dirty }))).toBe(true);
    const clean = join(root, "dist-clean");
    mkdirSync(clean);
    writeFileSync(join(clean, "index.js"), 'console.log("local only")');
    expect(failed(runEdition({ edition: "standard", dist: clean }))).toBe(
      false,
    );
  });

  test("a publik binary must carry its own feed", () => {
    const root = tree({});
    const without = join(root, "app-a");
    writeFileSync(without, "\0\0releases/latest/download/latest.json\0\0");
    expect(failed(runEdition({ edition: "publik", binaries: [without] }))).toBe(
      true,
    );
    const withFeed = join(root, "app-b");
    writeFileSync(
      withFeed,
      `\0\0releases/latest/download/${PUBLIK_REQUIRED}\0\0`,
    );
    expect(
      failed(runEdition({ edition: "publik", binaries: [withFeed] })),
    ).toBe(false);
  });

  test("an unknown edition fails", () => {
    expect(failed(runEdition({ edition: "pro", dist: tree({}) }))).toBe(true);
  });
});

describe("hosts guard: publik's hosts", () => {
  test("only the publik folders may name them", () => {
    const allowed = `[publik]\n${DOMAIN}  # the publik edition\n`;
    const url = `const ORIGIN: &str = "https://${DOMAIN}";\n`;
    const inside = runHosts({
      root: tree({
        "ci/allowed-hosts.txt": allowed,
        "ci/inert-hosts.txt": "",
        "ci/upstream-hosts-baseline.txt": "",
        "src-tauri/src/fork/publik/client.rs": url,
      }),
    });
    expect(inside.violations).toEqual([]);
    const outside = runHosts({
      root: tree({
        "ci/allowed-hosts.txt": allowed,
        "ci/inert-hosts.txt": "",
        "ci/upstream-hosts-baseline.txt": "",
        "src-tauri/src/fork/net.rs": url,
      }),
    });
    expect(outside.violations.map((v) => v.rule)).toContain(
      "hosts/publik-only",
    );
  });
});
