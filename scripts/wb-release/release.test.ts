// SPDX-License-Identifier: GPL-3.0-or-later
// F-02 release helpers: tag format and order, latest.json, minisign check.
import { describe, expect, test } from "bun:test";
import { spawnSync } from "node:child_process";
import { readFileSync } from "node:fs";
import { join } from "node:path";
import { buildManifest } from "./manifest";
import { overlayPubkey, verifyMinisign } from "./minisign";
import { checkTag, compareTags } from "./tag";

const ROOT = join(import.meta.dir, "../..");
const TESTDATA = join(ROOT, "src-tauri/src/fork/update/testdata");
const artifact = readFileSync(join(TESTDATA, "update.app.tar.gz"));
const signature = readFileSync(join(TESTDATA, "update.app.tar.gz.sig"), "utf8");
const testKey = readFileSync(join(TESTDATA, "test-updater.key.pub"), "utf8");

describe("release tag", () => {
  test("accepts our formats and picks the channel", () => {
    expect(
      checkTag("v1.0.0-alpha.2", ["v1.0.0-alpha.1"], "1.0.0-alpha.2"),
    ).toEqual({
      ok: true,
      errors: [],
      version: "1.0.0-alpha.2",
      channel: "stable",
      releaseTag: "v1.0.0-alpha.2",
    });
    const beta = checkTag("v1.0.0-beta.1", ["v1.0.0-alpha.9"], "1.0.0-beta.1");
    expect(beta.ok).toBe(true);
    expect(beta.channel).toBe("beta");
    expect(beta.releaseTag).toBe("beta");
    expect(checkTag("v1.0.0", ["v1.0.0-rc.3"], "1.0.0").channel).toBe("stable");
  });

  test("rejects bad formats, version mismatches and non-monotonic tags", () => {
    for (const bad of ["1.0.0", "v1.0", "v1.0.0-alpha", "v1.0.0-dev.1", "beta"])
      expect(checkTag(bad, [], bad.replace(/^v/, "")).ok).toBe(false);
    expect(checkTag("v1.0.0-alpha.2", [], "1.0.0-alpha.1").errors[0]).toContain(
      "≠ v1.0.0-alpha.1",
    );
    const older = checkTag(
      "v1.0.0-alpha.2",
      ["v1.0.0-alpha.10", "v1.0.0-alpha.2", "beta"],
      "1.0.0-alpha.2",
    );
    expect(older.ok).toBe(false);
    expect(older.errors).toEqual([
      "tag v1.0.0-alpha.2 is not newer than existing tag v1.0.0-alpha.10",
    ]);
  });

  test("orders tags by semver, numerically", () => {
    expect(compareTags("v1.0.0-alpha.10", "v1.0.0-alpha.9")).toBe(1);
    expect(compareTags("v1.0.0-beta.1", "v1.0.0-alpha.9")).toBe(1);
    expect(compareTags("v1.0.0", "v1.0.0-rc.1")).toBe(1);
    expect(compareTags("v1.0.0-beta.3", "v1.0.0")).toBe(-1);
  });
});

describe("minisign", () => {
  test("verifies the fixture against its key and rejects tampering", () => {
    const ok = verifyMinisign(artifact, signature, testKey);
    expect(ok.ok).toBe(true);
    expect(ok.keyId).toMatch(/^[0-9A-F]{16}$/);
    const tampered = Buffer.from(artifact);
    tampered[tampered.length - 1] ^= 0xff;
    expect(verifyMinisign(tampered, signature, testKey).error).toBe(
      "signature does not match the file",
    );
  });

  test("rejects a signature from another key (our pubkey vs the test key)", () => {
    const r = verifyMinisign(artifact, signature, overlayPubkey(ROOT));
    expect(r.ok).toBe(false);
    expect(r.keyId).toBe("89E085217ACC46CF");
    expect(r.error).toContain("signed by key");
  });

  test("rejects a forged trusted comment", () => {
    const lines = Buffer.from(signature.trim(), "base64")
      .toString("utf8")
      .split("\n");
    lines[2] = "trusted comment: timestamp:0\tfile:evil";
    const forged = Buffer.from(lines.join("\n")).toString("base64");
    expect(verifyMinisign(artifact, forged, testKey).error).toBe(
      "trusted comment signature is invalid",
    );
  });
});

describe("latest.json", () => {
  test("points every platform entry at our release asset", () => {
    const m = buildManifest({
      version: "1.0.0-alpha.2",
      releaseTag: "v1.0.0-alpha.2",
      assets: [
        {
          platform: "darwin-aarch64",
          file: "/out/Wispr-Bro_1.0.0-alpha.2_aarch64.app.tar.gz",
          signature: "c2ln\n",
        },
      ],
      date: "2026-11-02T12:00:00Z",
    });
    const url =
      "https://github.com/fjenn/wispr-bro/releases/download/v1.0.0-alpha.2/Wispr-Bro_1.0.0-alpha.2_aarch64.app.tar.gz";
    expect(m).toEqual({
      version: "1.0.0-alpha.2",
      notes: "Wispr Bro 1.0.0-alpha.2",
      pub_date: "2026-11-02T12:00:00Z",
      platforms: {
        "darwin-aarch64-app": { signature: "c2ln", url },
        "darwin-aarch64": { signature: "c2ln", url },
      },
    });
  });

  test("an Intel asset adds its own entries and leaves Apple Silicon's as they were", () => {
    const arm = {
      platform: "darwin-aarch64" as const,
      file: "/out/Wispr-Bro_1.0.0-alpha.6_aarch64.app.tar.gz",
      signature: "YQ==",
    };
    const intel = {
      platform: "darwin-x86_64" as const,
      file: "/x/Wispr-Bro_1.0.0-alpha.6_x86_64.app.tar.gz",
      signature: "aQ==",
    };
    const base = {
      version: "1.0.0-alpha.6",
      releaseTag: "v1.0.0-alpha.6",
      date: "2026-11-14T12:00:00Z",
    };
    const armOnly = buildManifest({ ...base, assets: [arm] });
    const both = buildManifest({ ...base, assets: [arm, intel] });
    expect(Object.keys(both.platforms).sort()).toEqual([
      "darwin-aarch64",
      "darwin-aarch64-app",
      "darwin-x86_64",
      "darwin-x86_64-app",
    ]);
    for (const k of ["darwin-aarch64", "darwin-aarch64-app"]) {
      expect(both.platforms[k]).toEqual(armOnly.platforms[k]);
    }
    expect(both.platforms["darwin-x86_64-app"]).toEqual({
      signature: "aQ==",
      url: "https://github.com/fjenn/wispr-bro/releases/download/v1.0.0-alpha.6/Wispr-Bro_1.0.0-alpha.6_x86_64.app.tar.gz",
    });
  });

  test("beta assets live on the rolling beta release; a stand-in base works", () => {
    const m = buildManifest({
      version: "1.0.0-beta.1",
      releaseTag: "beta",
      assets: [
        {
          platform: "darwin-aarch64",
          file: "Wispr-Bro_1.0.0-beta.1_aarch64.app.tar.gz",
          signature: "x",
        },
      ],
      base: "http://127.0.0.1:18090/fjenn/wispr-bro/",
    });
    expect(m.platforms["darwin-aarch64"].url).toBe(
      "http://127.0.0.1:18090/fjenn/wispr-bro/releases/download/beta/Wispr-Bro_1.0.0-beta.1_aarch64.app.tar.gz",
    );
  });

  test("refuses names GitHub would rewrite and assets of another version", () => {
    const one = (file: string) => () =>
      buildManifest({
        version: "1.0.0-alpha.2",
        releaseTag: "v1.0.0-alpha.2",
        assets: [{ platform: "darwin-aarch64", file, signature: "x" }],
      });
    expect(one("Wispr Bro_1.0.0-alpha.2_aarch64.app.tar.gz")).toThrow(
      "unsafe characters",
    );
    expect(one("Wispr-Bro_1.0.0-alpha.1_aarch64.app.tar.gz")).toThrow(
      "does not carry version",
    );
  });
});

describe("macOS bundle metadata (Finder / Spotlight app search)", () => {
  // Tauri writes LSRequiresCarbon=true into every macOS bundle (Carbon-era
  // and meaningless now); the overlay's Info.plist merge turns it off and
  // pins the display name. CFBundleVersion must be numeric (Apple's format),
  // not the marketing version "1.0.0-alpha.1".
  const plist = () => {
    const r = spawnSync(
      "plutil",
      ["-convert", "json", "-o", "-", join(ROOT, "src-tauri", "Info.plist")],
      { encoding: "utf8" },
    );
    expect(r.status).toBe(0);
    return JSON.parse(r.stdout) as Record<string, unknown>;
  };

  test("the overlay sets a numeric CFBundleVersion and keeps the identity", () => {
    const conf = JSON.parse(
      readFileSync(join(ROOT, "src-tauri/tauri.fork.conf.json"), "utf8"),
    );
    expect(conf.identifier).toBe("com.wisprbro.desktop");
    expect(conf.productName).toBe("Wispr Bro");
    const version = conf.bundle?.macOS?.bundleVersion;
    expect(typeof version).toBe("string");
    expect(version).toMatch(/^\d+(\.\d+){0,2}$/);
    // The marketing version stays the user-facing one (package.json's).
    const pkg = JSON.parse(
      readFileSync(join(ROOT, "package.json"), "utf8"),
    ) as { version: string };
    expect(conf.version).toBe(pkg.version);
  });

  test("Info.plist turns LSRequiresCarbon off and names the app", () => {
    const p = plist();
    expect(p.LSRequiresCarbon).toBe(false);
    expect(p.CFBundleDisplayName).toBe("Wispr Bro");
  });
});

describe("DMG install window (OT2-E)", () => {
  test("the mounted volume is the installer, not the app", () => {
    const build = readFileSync(
      join(ROOT, "scripts/wb-release/dmg/build.sh"),
      "utf8",
    );
    // The volume's name and icon are constants in build.sh, used for the
    // dmgbuild call itself (not a copy somewhere else).
    expect(build).toContain('WB_DMG_VOLUME_NAME="Wispr Bro Installer"');
    expect(build).toContain(
      'WB_DMG_VOLUME_ICON="${WB_ROOT}/src-tauri/icons-fork/dmg/volume.icns"',
    );
    expect(build).toContain('"$WB_DMG_VOLUME_NAME" "$out"');
    expect(build).not.toMatch(/(^|\s)"Wispr Bro" "\$out"/);
    // Its own artwork, not the app icon, and a real icns.
    const icons = join(ROOT, "src-tauri", "icons-fork");
    const volume = readFileSync(join(icons, "dmg", "volume.icns"));
    const app = readFileSync(join(icons, "icon.icns"));
    expect(volume.subarray(0, 4).toString("ascii")).toBe("icns");
    expect(volume.equals(app)).toBe(false);
    // The SVG source `scripts/wb-icons.ts` regenerates it from.
    const svg = readFileSync(join(icons, "src", "volume-icon.svg"), "utf8");
    expect(svg).toContain('viewBox="0 0 1024 1024"');
  });

  test("the overlay's layout fits the window and the background matches it", async () => {
    const { dmgLayout, pngSize, ICON_SIZE } = await import("./dmg/background");
    const l = dmgLayout();
    expect(l.width).toBe(660);
    expect(l.height).toBe(400);
    for (const p of [l.app, l.applications]) {
      // Icon and a two-line label stay inside the window.
      expect(p.x - ICON_SIZE / 2).toBeGreaterThan(0);
      expect(p.x + ICON_SIZE / 2).toBeLessThan(l.width);
      expect(p.y + ICON_SIZE / 2 + 40).toBeLessThan(l.height);
    }
    expect(l.app.x).toBeLessThan(l.applications.x);
    const bg = join(ROOT, "src-tauri", l.background);
    expect(pngSize(bg)).toEqual([l.width, l.height]);
    expect(pngSize(bg.replace(/\.png$/, "@2x.png"))).toEqual([
      2 * l.width,
      2 * l.height,
    ]);
  });

  test("the tray tone reads with Finder's dark and light labels", async () => {
    const { TRAY } = await import("./dmg/background");
    const ch = (i: number) => {
      const c = parseInt(TRAY.slice(i, i + 2), 16) / 255;
      return c <= 0.04045 ? c / 12.92 : ((c + 0.055) / 1.055) ** 2.4;
    };
    const L = 0.2126 * ch(1) + 0.7152 * ch(3) + 0.0722 * ch(5);
    expect(1.05 / (L + 0.05)).toBeGreaterThanOrEqual(4.5);
    expect((L + 0.05) / 0.05).toBeGreaterThanOrEqual(4.5);
  });
});
