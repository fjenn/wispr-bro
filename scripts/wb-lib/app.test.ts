// SPDX-License-Identifier: GPL-3.0-or-later
//
// Self-tests for scripts/wb-lib/app.sh (Mac wave 1 finding 7): the drivers
// find `wispr-bro` (bundle CFBundleExecutable or a `--no-bundle` build) and
// fall back to `handy` (a plain `cargo build`), every driver uses the helper
// instead of a hard-coded `./handy`, and `wb_cancel` never starts an app.

import { afterAll, describe, expect, test } from "bun:test";
import { spawnSync } from "node:child_process";
import {
  chmodSync,
  mkdirSync,
  mkdtempSync,
  readFileSync,
  readdirSync,
  rmSync,
  statSync,
  utimesSync,
  writeFileSync,
} from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";

const HERE = import.meta.dir;
const LIB = join(HERE, "app.sh");
const SCRIPTS = join(HERE, "..");
const tmp = mkdtempSync(join(tmpdir(), "wb-app-lib-"));
afterAll(() => rmSync(tmp, { recursive: true, force: true }));

function sh(script: string, env: Record<string, string> = {}) {
  const r = spawnSync("/bin/bash", ["-c", `. "${LIB}"; ${script}`], {
    encoding: "utf8",
    env: { ...process.env, ...env },
  });
  return { code: r.status, out: r.stdout.trim(), err: r.stderr.trim() };
}

function exe(path: string, body = "#!/bin/sh\nexit 0\n", mtime?: number) {
  writeFileSync(path, body);
  chmodSync(path, 0o755);
  if (mtime !== undefined) utimesSync(path, mtime, mtime);
}

describe("wb_bin_name", () => {
  test("a bundle uses CFBundleExecutable", () => {
    const app = join(tmp, "Wispr Bro.app");
    mkdirSync(join(app, "Contents", "MacOS"), { recursive: true });
    writeFileSync(
      join(app, "Contents", "Info.plist"),
      `<?xml version="1.0" encoding="UTF-8"?>
<plist version="1.0"><dict><key>CFBundleExecutable</key><string>wispr-bro</string></dict></plist>`,
    );
    if (process.platform === "darwin") {
      expect(sh(`wb_bin_name "${app}"`).out).toBe("wispr-bro");
      expect(sh(`wb_bin_path "${app}"`).out).toBe(
        join(app, "Contents", "MacOS", "wispr-bro"),
      );
    }
  });

  test("a build folder prefers the newest of wispr-bro and handy", () => {
    const dir = join(tmp, "debug");
    mkdirSync(dir);
    // Nothing built yet: the dev-tree name.
    expect(sh(`wb_bin_name "${dir}"`).out).toBe("handy");
    exe(join(dir, "handy"), undefined, 1_000);
    expect(sh(`wb_bin_name "${dir}"`).out).toBe("handy");
    exe(join(dir, "wispr-bro"), undefined, 2_000);
    expect(sh(`wb_bin_name "${dir}"`).out).toBe("wispr-bro");
    // A later `cargo build` is newer than the old `--no-bundle` binary.
    utimesSync(join(dir, "handy"), 3_000, 3_000);
    expect(sh(`wb_bin_name "${dir}"`).out).toBe("handy");
    expect(statSync(join(dir, "handy")).isFile()).toBe(true);
  });

  test("the pgrep pattern matches both names", () => {
    const re = new RegExp(`${sh("echo $WB_BIN_RE").out} --start-hidden`);
    expect(re.test("./wispr-bro --start-hidden")).toBe(true);
    expect(re.test("./handy --start-hidden")).toBe(true);
  });
});

describe("wb_cli / wb_cancel", () => {
  test("a CLI call that never returns is killed after the timeout", () => {
    const dir = join(tmp, "hang");
    mkdirSync(dir);
    exe(join(dir, "wispr-bro"), "#!/bin/sh\nexec sleep 30\n");
    const t = Date.now();
    const r = sh(`wb_cli "${dir}" --cancel`, { WB_CLI_TIMEOUT: "1" });
    expect(r.code).toBe(124);
    expect(Date.now() - t).toBeLessThan(10_000);
  }, 30_000);

  test("a CLI call that returns passes its status through", () => {
    const dir = join(tmp, "quick");
    mkdirSync(dir);
    exe(join(dir, "wispr-bro"), "#!/bin/sh\nexit 3\n");
    expect(sh(`wb_cli "${dir}" --cancel`).code).toBe(3);
  }, 30_000);

  test("--cancel is not sent when no app runs from the folder", () => {
    const dir = join(tmp, "idle");
    mkdirSync(dir);
    const marker = join(tmp, "started");
    exe(join(dir, "wispr-bro"), `#!/bin/sh\ntouch "${marker}"\n`);
    const r = sh(`wb_cancel "${dir}"`);
    expect(r.code).toBe(1);
    expect(() => statSync(marker)).toThrow();
  }, 30_000);
});

test("no driver hard-codes ./handy any more", () => {
  const offenders: string[] = [];
  for (const d of readdirSync(SCRIPTS, { withFileTypes: true })) {
    const files = d.isDirectory()
      ? readdirSync(join(SCRIPTS, d.name)).map((f) => join(d.name, f))
      : [d.name];
    for (const f of files) {
      if (!f.startsWith("wb-s") || !/\.(sh|py)$/.test(f)) continue;
      const text = readFileSync(join(SCRIPTS, f), "utf8");
      for (const line of text.split("\n")) {
        if (line.trimStart().startsWith("#")) continue;
        if (/\.\/handy\b|"handy --|pgrep -f "\^?\.?\/?handy/.test(line))
          offenders.push(`${f}: ${line.trim()}`);
      }
    }
  }
  expect(offenders).toEqual([]);
});
