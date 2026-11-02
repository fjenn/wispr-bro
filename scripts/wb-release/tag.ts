// SPDX-License-Identifier: GPL-3.0-or-later
//
// Release tag check (F-02). A release tag is `v<major>.<minor>.<patch>` with
// an optional `-alpha.N`, `-beta.N` or `-rc.N`, equals `v<overlay version>`
// (the identity guard checks that too) and is strictly newer than every other
// `v*` tag, so the updater (which only moves to a higher semver) can always
// reach it. The channel follows from the tag: `-beta.N` → the rolling `beta`
// prerelease; everything else → a draft release on the stable feed
// (`releases/latest`) that the owner publishes.
//
// Usage: bun scripts/wb-release/tag.ts <tag> [--tags <file>]
// Prints `version=…`, `channel=…` and `release_tag=…` lines (for
// $GITHUB_OUTPUT); exits 1 with the reasons on a bad tag. Other tags come from
// `git tag -l 'v*'` unless --tags names a file with one tag per line.

import { execFileSync } from "node:child_process";
import { readFileSync } from "node:fs";
import { join } from "node:path";

const REPO_ROOT = join(import.meta.dir, "../..");

export const TAG_RE = /^v(\d+)\.(\d+)\.(\d+)(?:-(alpha|beta|rc)\.(\d+))?$/;

export type Channel = "stable" | "beta";

export interface TagCheck {
  ok: boolean;
  errors: string[];
  version?: string;
  channel?: Channel;
  /** GitHub release the assets go to: the tag itself, or `beta`. */
  releaseTag?: string;
}

/** semver order of two `v…` tags in our format (-1, 0, 1). */
export function compareTags(a: string, b: string): number {
  return Bun.semver.order(a.replace(/^v/, ""), b.replace(/^v/, ""));
}

export function checkTag(
  tag: string,
  otherTags: string[],
  overlayVersion: string,
): TagCheck {
  const errors: string[] = [];
  const m = TAG_RE.exec(tag);
  if (!m) {
    errors.push(
      `tag ${JSON.stringify(tag)} is not vX.Y.Z or vX.Y.Z-(alpha|beta|rc).N`,
    );
    return { ok: false, errors };
  }
  const version = tag.slice(1);
  if (version !== overlayVersion) {
    errors.push(
      `tag ${tag} ≠ v${overlayVersion} (src-tauri/tauri.fork.conf.json version)`,
    );
  }
  for (const other of otherTags) {
    if (other === tag || !TAG_RE.test(other)) continue;
    if (compareTags(tag, other) <= 0) {
      errors.push(`tag ${tag} is not newer than existing tag ${other}`);
    }
  }
  const channel: Channel = m[4] === "beta" ? "beta" : "stable";
  return {
    ok: errors.length === 0,
    errors,
    version,
    channel,
    releaseTag: channel === "beta" ? "beta" : tag,
  };
}

export function overlayVersion(root = REPO_ROOT): string {
  return JSON.parse(
    readFileSync(join(root, "src-tauri/tauri.fork.conf.json"), "utf8"),
  ).version;
}

if (import.meta.main) {
  const argv = process.argv.slice(2);
  const tag = argv[0];
  if (!tag) {
    console.error("usage: tag.ts <tag> [--tags <file>]");
    process.exit(2);
  }
  const i = argv.indexOf("--tags");
  const others = (
    i >= 0
      ? readFileSync(argv[i + 1], "utf8")
      : execFileSync("git", ["tag", "-l", "v*"], {
          cwd: REPO_ROOT,
          encoding: "utf8",
        })
  )
    .split("\n")
    .map((s) => s.trim())
    .filter(Boolean);
  const r = checkTag(tag, others, overlayVersion());
  if (!r.ok) {
    for (const e of r.errors) console.error(`✗ ${e}`);
    process.exit(1);
  }
  console.log(`version=${r.version}`);
  console.log(`channel=${r.channel}`);
  console.log(`release_tag=${r.releaseTag}`);
}
