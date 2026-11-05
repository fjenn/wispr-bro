// SPDX-License-Identifier: GPL-3.0-or-later
//
// Writes the updater feed `latest.json` for one release (F-02, E-57), in the
// format tauri-plugin-updater 2.10.1 reads: `version`, `notes`, `pub_date`
// and one entry per platform with the artifact URL and the contents of its
// `.sig`. Both `darwin-aarch64-app` (what a bundled app looks up first) and
// `darwin-aarch64` are written. Every signature is verified against the
// overlay pubkey first, so a feed can never point at an artifact our installs
// would reject.
//
// Usage:
//   bun scripts/wb-release/manifest.ts --version 1.0.0-alpha.2 \
//     --release-tag v1.0.0-alpha.2 --asset darwin-aarch64=<…app.tar.gz> \
//     [--notes <text>] [--date <RFC 3339>] [--base <repo URL>] --out latest.json
// The asset URL is <base>/releases/download/<release-tag>/<file name>; the
// base defaults to https://github.com/fjenn/wispr-bro (a loopback stand-in in
// the Mac update test).

import { readFileSync, writeFileSync } from "node:fs";
import { basename } from "node:path";
import { overlayPubkey, verifyMinisign } from "./minisign";

export const GITHUB_BASE = "https://github.com/fjenn/wispr-bro";
export const PLATFORMS = ["darwin-aarch64", "darwin-x86_64"] as const;
export type Platform = (typeof PLATFORMS)[number];

export interface ManifestInput {
  version: string;
  releaseTag: string;
  assets: { platform: Platform; file: string; signature: string }[];
  notes?: string;
  date?: string;
  base?: string;
}

export interface Manifest {
  version: string;
  notes: string;
  pub_date: string;
  platforms: Record<string, { signature: string; url: string }>;
}

export function buildManifest(input: ManifestInput): Manifest {
  if (!/^\d+\.\d+\.\d+(-[0-9A-Za-z.]+)?$/.test(input.version)) {
    throw new Error(`bad version ${input.version}`);
  }
  if (input.assets.length === 0) throw new Error("no assets");
  const base = (input.base ?? GITHUB_BASE).replace(/\/+$/, "");
  const platforms: Manifest["platforms"] = {};
  for (const a of input.assets) {
    const name = basename(a.file);
    // GitHub rewrites spaces in asset names; the URL would not match.
    if (!/^[A-Za-z0-9._-]+$/.test(name)) {
      throw new Error(
        `asset name ${JSON.stringify(name)} has unsafe characters`,
      );
    }
    if (!name.includes(input.version)) {
      throw new Error(`asset ${name} does not carry version ${input.version}`);
    }
    const entry = {
      signature: a.signature.trim(),
      url: `${base}/releases/download/${input.releaseTag}/${name}`,
    };
    platforms[`${a.platform}-app`] = entry;
    platforms[a.platform] = entry;
  }
  return {
    version: input.version,
    notes: input.notes ?? `Wispr Bro ${input.version}`,
    pub_date: input.date ?? new Date().toISOString().replace(/\.\d{3}Z$/, "Z"),
    platforms,
  };
}

if (import.meta.main) {
  const argv = process.argv.slice(2);
  const opt = (name: string) => {
    const i = argv.indexOf(name);
    return i >= 0 ? argv[i + 1] : undefined;
  };
  const assets: ManifestInput["assets"] = [];
  const pubkey = opt("--pubkey") ?? overlayPubkey();
  argv.forEach((a, i) => {
    if (a !== "--asset") return;
    const [platform, file] = argv[i + 1].split(/=(.*)/s);
    if (!PLATFORMS.includes(platform as Platform)) {
      throw new Error(`unknown platform ${platform}`);
    }
    const signature = readFileSync(`${file}.sig`, "utf8");
    const check = verifyMinisign(readFileSync(file), signature, pubkey);
    if (!check.ok) throw new Error(`${file}: ${check.error}`);
    assets.push({ platform: platform as Platform, file, signature });
  });
  const version = opt("--version");
  const releaseTag = opt("--release-tag");
  const out = opt("--out");
  if (!version || !releaseTag || !out) {
    console.error(
      "usage: manifest.ts --version V --release-tag T --asset P=FILE --out FILE",
    );
    process.exit(2);
  }
  const manifest = buildManifest({
    version,
    releaseTag,
    assets,
    notes: opt("--notes"),
    date: opt("--date"),
    base: opt("--base"),
  });
  writeFileSync(out, JSON.stringify(manifest, null, 2) + "\n");
  console.log(
    `✓ ${out}: ${version} → ${Object.keys(manifest.platforms).join(", ")}`,
  );
}
