// SPDX-License-Identifier: GPL-3.0-or-later
/**
 * What's New content (S-28, C-99): our own release notes, one Markdown file
 * per version in this folder (`1.0.0-alpha.2.md`). Handy's offline mechanism
 * is kept (bundled Markdown, the `whats_new_last_seen_version` setting,
 * Handy's `MarkdownContent` renderer); Handy's own notes in
 * `src/content/release-notes/` are never shown.
 *
 * Unlike Handy's, versions compare with their prerelease part (semver), so
 * `1.0.0-alpha.2` has its own note after `1.0.0-alpha.1`.
 */

export interface ReleaseNote {
  version: string;
  markdown: string;
}

const modules = import.meta.glob<string>("./*.md", {
  eager: true,
  import: "default",
  query: "?raw",
});

interface Parsed {
  core: [number, number, number];
  pre: string[];
}

export function parseVersion(version: string): Parsed | null {
  const match = version
    .trim()
    .replace(/^v/i, "")
    .match(/^(\d+)\.(\d+)\.(\d+)(?:-([0-9A-Za-z.-]+))?(?:\+.*)?$/);
  if (!match) return null;
  return {
    core: [Number(match[1]), Number(match[2]), Number(match[3])],
    pre: match[4] ? match[4].split(".") : [],
  };
}

function compareIdentifiers(a: string, b: string): number {
  const na = /^\d+$/.test(a);
  const nb = /^\d+$/.test(b);
  if (na && nb) return Number(a) - Number(b);
  if (na) return -1;
  if (nb) return 1;
  return a < b ? -1 : a > b ? 1 : 0;
}

/** Semver precedence: < 0 when `a` is older than `b`. Unparsable = equal. */
export function compareVersions(a: string, b: string): number {
  const pa = parseVersion(a);
  const pb = parseVersion(b);
  if (!pa || !pb) return 0;
  for (let i = 0; i < 3; i += 1) {
    if (pa.core[i] !== pb.core[i]) return pa.core[i] - pb.core[i];
  }
  if (pa.pre.length === 0 || pb.pre.length === 0) {
    return pb.pre.length - pa.pre.length;
  }
  const n = Math.max(pa.pre.length, pb.pre.length);
  for (let i = 0; i < n; i += 1) {
    if (pa.pre[i] === undefined) return -1;
    if (pb.pre[i] === undefined) return 1;
    const c = compareIdentifiers(pa.pre[i], pb.pre[i]);
    if (c !== 0) return c;
  }
  return 0;
}

export function notesFrom(files: Record<string, string>): ReleaseNote[] {
  return Object.entries(files)
    .map(([path, markdown]) => ({
      version: path.replace(/^.*\//, "").replace(/\.md$/, ""),
      markdown,
    }))
    .filter((note) => parseVersion(note.version) !== null);
}

const bundled = notesFrom(modules);

/**
 * The newest note newer than `lastSeen` and not newer than `current`, or
 * null. An unparsable `lastSeen` (an install from before the setting) shows
 * the current version's note, as Handy does.
 */
export function noteToShow(
  current: string,
  lastSeen: string,
  notes: ReleaseNote[] = bundled,
): ReleaseNote | null {
  if (!parseVersion(current)) return null;
  const seen = parseVersion(lastSeen) !== null;
  return (
    notes
      .filter((n) => compareVersions(n.version, current) <= 0)
      .filter((n) => !seen || compareVersions(n.version, lastSeen) > 0)
      .sort((a, b) => compareVersions(b.version, a.version))[0] ?? null
  );
}

/** Every bundled note, newest first (the About page lists the latest). */
export function bundledNotes(): ReleaseNote[] {
  return [...bundled].sort((a, b) => compareVersions(b.version, a.version));
}
