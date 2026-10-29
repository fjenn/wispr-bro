// SPDX-License-Identifier: GPL-3.0-or-later
//
// Tests for the third-party notices generator (F-04). Fixture trees stand in
// for node_modules, cargo-about's JSON and crate sources; the last tests run
// the npm, manual and NOTICE parts on the real tree.

import { afterAll, describe, expect, test } from "bun:test";
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
import { REPO_ROOT } from "./wb-guard-lib";
import {
  type AboutJson,
  collectNpm,
  generate,
  licenceProblem,
  missingModelAttributions,
  NATIVE,
  noticeApacheNames,
  noticeSection,
  parseAccepted,
  render,
} from "./wb-notices";

const dirs: string[] = [];
afterAll(() => {
  for (const d of dirs) rmSync(d, { recursive: true, force: true });
});

function write(root: string, rel: string, text: string): void {
  mkdirSync(dirname(join(root, rel)), { recursive: true });
  writeFileSync(join(root, rel), text);
}

const accepted = new Set(
  parseAccepted(readFileSync(join(REPO_ROOT, "about.toml"), "utf8")),
);

describe("licence expressions", () => {
  test("accepted list comes from about.toml", () => {
    expect(accepted.has("MIT")).toBe(true);
    expect(accepted.has("Apache-2.0 WITH LLVM-exception")).toBe(true);
    expect(accepted.has("GPL-2.0-only")).toBe(false);
  });

  test("free, GPL-3.0-compatible expressions pass", () => {
    for (const l of [
      "MIT",
      "MIT OR Apache-2.0",
      "(MIT OR Apache-2.0) AND Unicode-3.0",
      "Apache-2.0 WITH LLVM-exception OR Apache-2.0 OR MIT",
      "GPL-2.0-only OR MIT",
      "MPL-2.0",
      "LGPL-2.1+",
    ])
      expect({ l, p: licenceProblem(l, accepted) }).toEqual({
        l,
        p: undefined,
      });
  });

  test("GPL-2.0-only, unlicensed, non-free and unknown licences fail", () => {
    for (const l of [
      "GPL-2.0-only",
      "GPL-2.0",
      "GPL-2.0-only AND MIT",
      "UNLICENSED",
      "SEE LICENSE IN LICENSE.md",
      "CC-BY-NC-4.0",
      "BUSL-1.1",
      "Foo-1.0",
      "",
      undefined,
      "(MIT",
      "MIT OR",
    ])
      expect(licenceProblem(l, accepted)).toBeString();
  });
});

describe("npm closure", () => {
  const npmFixture = () => {
    const root = mkdtempSync(join(tmpdir(), "wb-notices-"));
    dirs.push(root);
    write(
      root,
      "package.json",
      JSON.stringify({
        dependencies: { a: "1", "@s/b": "1", "@tailwindcss/vite": "4" },
        devDependencies: { devonly: "1" },
      }),
    );
    write(
      root,
      "node_modules/a/package.json",
      JSON.stringify({
        name: "a",
        version: "1.0.0",
        license: "MIT",
        dependencies: { c: "1" },
        optionalDependencies: { "not-installed": "1" },
      }),
    );
    write(root, "node_modules/a/LICENSE", "MIT text a\n");
    // Nested copy resolved from a, and a hoisted copy of the same version.
    write(
      root,
      "node_modules/a/node_modules/c/package.json",
      JSON.stringify({ name: "c", version: "2.0.0", license: "ISC" }),
    );
    write(
      root,
      "node_modules/@s/b/package.json",
      JSON.stringify({
        name: "@s/b",
        version: "0.1.0",
        licenses: [{ type: "MIT" }, { type: "Apache-2.0" }],
        dependencies: { c: "2" },
      }),
    );
    write(
      root,
      "node_modules/c/package.json",
      JSON.stringify({ name: "c", version: "2.0.0", license: "ISC" }),
    );
    write(root, "node_modules/c/NOTICE", "c notice\n");
    write(
      root,
      "node_modules/@tailwindcss/vite/package.json",
      JSON.stringify({
        name: "@tailwindcss/vite",
        version: "4.0.0",
        license: "MIT",
        dependencies: { vite: "6" },
      }),
    );
    write(
      root,
      "node_modules/devonly/package.json",
      JSON.stringify({
        name: "devonly",
        version: "1.0.0",
        license: "GPL-2.0-only",
      }),
    );
    return root;
  };

  test("walks runtime deps, skips dev and build-only subtrees, dedupes copies", () => {
    const { pkgs, problems } = collectNpm(npmFixture());
    expect(problems).toEqual([]);
    expect(pkgs.map((p) => `${p.name}@${p.version} ${p.license}`)).toEqual([
      "@s/b@0.1.0 MIT OR Apache-2.0",
      "a@1.0.0 MIT",
      "c@2.0.0 ISC",
    ]);
    expect(pkgs.find((p) => p.name === "a")!.texts[0].text).toBe(
      "MIT text a\n",
    );
  });

  test("a missing runtime dependency is a problem", () => {
    const root = npmFixture();
    rmSync(join(root, "node_modules/@s"), { recursive: true });
    expect(collectNpm(root).problems.map((p) => p.where)).toEqual(["npm:@s/b"]);
  });
});

describe("generate on a fixture tree", () => {
  const catalog = {
    models: [
      { id: "p1", repository: "handy-computer/p1-gguf", license: "cc-by-4.0" },
      {
        id: "nc",
        repository: "handy-computer/nc-gguf",
        license: "cc-by-nc-4.0",
      },
      { id: "w", repository: "handy-computer/w-gguf", license: "mit" },
    ],
  };
  const notice = (models: string, apache = "(none)") =>
    [
      "Wispr Bro",
      "",
      "=====",
      "5. Apache-2.0 NOTICE files",
      "=====",
      "",
      apache,
      "",
      "=====",
      "6. Model attributions",
      "=====",
      "",
      models,
      "",
      "=====",
      "7. Fonts",
      "=====",
      "",
      "- Some font, OFL-1.1",
      "",
    ].join("\n");

  const treeFixture = () => {
    const root = mkdtempSync(join(tmpdir(), "wb-notices-"));
    dirs.push(root);
    copyFileSync(join(REPO_ROOT, "about.toml"), join(root, "about.toml"));
    write(
      root,
      "package.json",
      JSON.stringify({ dependencies: { "@tailwindcss/vite": "4" } }),
    );
    write(root, "src-tauri/src/catalog/catalog.json", JSON.stringify(catalog));
    write(root, "NOTICE", notice("- P1: handy-computer/p1-gguf, CC-BY-4.0"));
    const crates: AboutJson["crates"] = [];
    for (const n of NATIVE) {
      const dir = join(root, "crates", n.crate);
      for (const c of n.components)
        if (c.file) write(dir, c.file, `${n.crate} ${c.file} text\n`);
      write(dir, "Cargo.toml", "");
      crates.push({
        license: "MIT",
        package: {
          name: n.crate,
          version: "1.0.0",
          manifest_path: join(dir, "Cargo.toml"),
          dependencies: [{ name: "cc", kind: "build" }],
        },
      });
    }
    const about: AboutJson = {
      crates,
      licenses: [
        {
          id: "MIT",
          name: "MIT License",
          text: "MIT text\n",
          used_by: crates.map((c) => ({
            crate: { name: c.package.name, version: c.package.version },
          })),
        },
      ],
    };
    return { root, about };
  };

  test("a complete tree passes and renders every section", () => {
    const { root, about } = treeFixture();
    const r = generate({ root, about });
    expect(r.problems).toEqual([]);
    for (const h of [
      "## Speech models",
      "## Fonts",
      "## C/C++ compiled into Rust crates",
      "### transcribe-cpp-sys 1.0.0",
      "ggml/LICENSE",
      "## Rust crates",
      "## npm packages",
    ])
      expect(r.markdown).toContain(h);
    expect(r.markdown).toContain("handy-computer/p1-gguf");
  });

  test("a forbidden crate licence, an unlisted native crate, a stale entry fail", () => {
    const { root, about } = treeFixture();
    const dir = join(root, "crates", "gplsys");
    write(dir, "Cargo.toml", "");
    about.crates.push({
      license: "GPL-2.0-only",
      package: {
        name: "gplsys",
        version: "0.1.0",
        manifest_path: join(dir, "Cargo.toml"),
        dependencies: [{ name: "cc", kind: "build" }],
      },
    });
    about.crates = about.crates.filter((c) => c.package.name !== "ring");
    const r = generate({ root, about });
    expect(r.problems.map((p) => p.where).sort()).toEqual([
      "native:gplsys",
      "native:ring",
      "rust:gplsys 0.1.0",
    ]);
  });

  test("a CC-BY model without attribution and an unlisted NOTICE file fail", () => {
    const { root, about } = treeFixture();
    write(root, "NOTICE", notice("(none yet)"));
    write(join(root, "crates", "ring"), "NOTICE", "Apache NOTICE of ring\n");
    const r = generate({ root, about });
    expect(r.problems.map((p) => p.where).sort()).toEqual([
      "NOTICE §6",
      "rust:ring 1.0.0",
    ]);
    expect(r.problems[0].message + r.problems[1].message).toContain("p1");

    write(
      root,
      "NOTICE",
      notice("- P1: handy-computer/p1-gguf", "- ring: Apache NOTICE text"),
    );
    const ok = generate({ root, about });
    expect(ok.problems).toEqual([]);
    expect(ok.markdown).toContain("Apache NOTICE of ring");
  });

  test("only non-NC CC-BY models need an attribution", () => {
    expect(missingModelAttributions(notice("(none)"), catalog)).toEqual([
      "p1 (handy-computer/p1-gguf, cc-by-4.0)",
    ]);
  });

  test("fences survive licence texts with backtick runs", () => {
    const md = render({
      npm: [
        {
          ecosystem: "npm",
          name: "x",
          version: "1",
          license: "MIT",
          texts: [{ file: "LICENSE", text: "a\n```\nb\n````\n" }],
          notices: [],
        },
      ],
      native: [],
      notice: notice("- m"),
    });
    expect(md).toContain("`````text\na\n```\nb\n````\n`````");
  });
});

describe("the real tree", () => {
  const notice = readFileSync(join(REPO_ROOT, "NOTICE"), "utf8");

  test("NOTICE sections parse and list the AGPL sanitizer", () => {
    expect(noticeSection(notice, "Model attributions")).toContain(
      "handy-computer/parakeet-tdt-0.6b-v3-gguf",
    );
    expect(noticeSection(notice, "Fonts")).toContain("Noto Sans CJK");
    expect(noticeSection(notice, "Files licensed AGPL-3.0-only")).toContain(
      "src-tauri/src/fork/diagnostics/sanitizer.rs",
    );
    expect(noticeApacheNames(notice).size).toBe(0);
  });

  test("npm, manual and NOTICE checks pass without cargo-about", () => {
    const r = generate({ skipRust: true });
    expect(r.problems).toEqual([]);
    expect(r.markdown).toContain("Not generated: cargo-about was not run");
  });
});
