// SPDX-License-Identifier: GPL-3.0-or-later
//
// Adds one platform to a release that is already published: the Intel build
// of a release that so far had only the Apple Silicon one. Reads the
// published `latest.json` and the single-platform `latest.json` that
// macos.sh wrote for the new build, and writes the published feed with the
// new platform's two entries added. Every entry that was there stays as it
// was (same keys, same values, same order), and so do `version`, `notes`
// and `pub_date`, so installs already on that release see no change.
//
// Usage:
//   bun scripts/wb-release/add-platform.ts --published <latest.json> \
//     --add <new single-platform latest.json> --asset <new .app.tar.gz> \
//     --out <merged latest.json>
// The asset's `.sig` must match the new feed's signature and verify against
// the overlay pubkey.

import { readFileSync, writeFileSync } from "node:fs";
import { basename } from "node:path";
import type { Manifest } from "./manifest";
import { overlayPubkey, verifyMinisign } from "./minisign";

export function addPlatform(published: string, added: string): string {
  const pub = JSON.parse(published) as Manifest;
  const add = JSON.parse(added) as Manifest;
  if (pub.version !== add.version) {
    throw new Error(
      `version ${add.version} of the new build ≠ published ${pub.version}`,
    );
  }
  const newKeys = Object.keys(add.platforms);
  if (newKeys.length === 0) throw new Error("the new feed has no platform");
  const pubUrl = Object.values(pub.platforms)[0]?.url ?? "";
  const pubDir = pubUrl.slice(0, pubUrl.lastIndexOf("/") + 1);
  for (const k of newKeys) {
    if (k in pub.platforms) {
      throw new Error(`${k} is already in the published feed`);
    }
    const url = add.platforms[k].url;
    if (!pubDir || !url.startsWith(pubDir)) {
      throw new Error(`${k} points at ${url}, not at the release ${pubDir}`);
    }
  }
  const merged: Manifest = {
    ...pub,
    platforms: { ...pub.platforms },
  };
  for (const k of newKeys) merged.platforms[k] = add.platforms[k];
  return JSON.stringify(merged, null, 2) + "\n";
}

if (import.meta.main) {
  const argv = process.argv.slice(2);
  const opt = (name: string) => {
    const i = argv.indexOf(name);
    return i >= 0 ? argv[i + 1] : undefined;
  };
  const published = opt("--published");
  const add = opt("--add");
  const asset = opt("--asset");
  const out = opt("--out");
  if (!published || !add || !asset || !out) {
    console.error(
      "usage: add-platform.ts --published FILE --add FILE --asset FILE --out FILE",
    );
    process.exit(2);
  }
  const addText = readFileSync(add, "utf8");
  const feed = JSON.parse(addText) as Manifest;
  const signature = readFileSync(`${asset}.sig`, "utf8");
  const check = verifyMinisign(
    readFileSync(asset),
    signature,
    opt("--pubkey") ?? overlayPubkey(),
  );
  if (!check.ok) throw new Error(`${asset}: ${check.error}`);
  for (const [k, e] of Object.entries(feed.platforms)) {
    if (
      e.signature !== signature.trim() ||
      basename(e.url) !== basename(asset)
    ) {
      throw new Error(`${k} does not describe ${basename(asset)}`);
    }
  }
  const text = addPlatform(readFileSync(published, "utf8"), addText);
  writeFileSync(out, text);
  console.log(
    `✓ ${out}: added ${Object.keys(feed.platforms).join(", ")}; published entries unchanged`,
  );
}
