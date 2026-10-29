// SPDX-License-Identifier: GPL-3.0-or-later
//
// Identity guard (R5 §4.12, §4.11). The effective config is
// `src-tauri/tauri.conf.json` with `src-tauri/tauri.fork.conf.json` applied as
// an RFC 7396 JSON merge patch (what `tauri --config` does). It fails when:
//   - the effective config contains `com.pais.handy`, `cjpais/Handy` or the
//     upstream updater pubkey prefix;
//   - `productName` is not "Wispr Bro" or `identifier` is not ours;
//   - the updater pubkey or endpoint is missing, or an endpoint is not on
//     `github.com/<owner>/wispr-bro`;
//   - `Cargo.toml`, `package.json` and the overlay versions disagree, or the
//     git tag (on tag builds) is not `v<version>`;
//   - `--release` only: F-02 placeholders (`<owner>`, `<hf-org>`, `TODO-F02`)
//     remain in the overlay or in fork code.
//
// Usage: bun scripts/wb-guard-identity.ts [--release] [--tag v1.2.3]
// The tag defaults to $GITHUB_REF_NAME when $GITHUB_REF_TYPE is `tag`.

import {
  argValue,
  cliMain,
  isForkCodePath,
  isTextFile,
  listFiles,
  newResult,
  readIfExists,
  REPO_ROOT,
  type GuardResult,
} from "./wb-guard-lib";

export const PRODUCT_NAME = "Wispr Bro";
export const IDENTIFIER = "com.wisprbro.desktop";
export const BASE_CONFIG = "src-tauri/tauri.conf.json";
export const OVERLAY = "src-tauri/tauri.fork.conf.json";

/** Strings that must never appear in the effective config. */
export const FORBIDDEN_IDENTITY: { needle: string; why: string }[] = [
  { needle: "com.pais.handy", why: "Handy's bundle identifier" },
  { needle: "cjpais/Handy", why: "Handy's repository / update feed" },
  // Base64 of "untrusted comment: minisign public key: BAB7209520…": the
  // first 60 characters of Handy's updater pubkey (tauri.conf.json:86).
  {
    needle: "dW50cnVzdGVkIGNvbW1lbnQ6IG1pbmlzaWduIHB1YmxpYyBrZXk6IEJBQjcyMDk1",
    why: "Handy's updater public key",
  },
];

/** F-02 owner-input placeholders: allowed on PRs, fatal in release mode. */
export const PLACEHOLDERS = ["<owner>", "<hf-org>", "TODO-F02"];

type Json = null | boolean | number | string | Json[] | { [k: string]: Json };

/** RFC 7396 JSON merge patch: objects merge, `null` deletes, the rest replaces. */
export function mergePatch(target: Json, patch: Json): Json {
  if (patch === null || typeof patch !== "object" || Array.isArray(patch)) {
    return patch;
  }
  const out: { [k: string]: Json } =
    target !== null && typeof target === "object" && !Array.isArray(target)
      ? { ...target }
      : {};
  for (const [k, v] of Object.entries(patch)) {
    if (v === null) delete out[k];
    else out[k] = mergePatch(out[k] ?? null, v);
  }
  return out;
}

function cargoVersion(toml: string): string | undefined {
  const pkg = toml.split(/^\[/m).find((s) => s.startsWith("package]"));
  return pkg?.match(/^version\s*=\s*"([^"]+)"/m)?.[1];
}

export interface IdentityOptions {
  root?: string;
  release?: boolean;
  tag?: string;
}

export function runIdentity(opts: IdentityOptions = {}): GuardResult {
  const root = opts.root ?? REPO_ROOT;
  const r = newResult(opts.release ? "identity (release)" : "identity");
  const v = (file: string, rule: string, message: string, line?: number) =>
    r.violations.push({ file, rule, message, line });

  const baseText = readIfExists(root, BASE_CONFIG);
  const overlayText = readIfExists(root, OVERLAY);
  if (!baseText || !overlayText) {
    v(baseText ? OVERLAY : BASE_CONFIG, "identity/config", "file is missing");
    return r;
  }
  let overlay: Json;
  let effective: { [k: string]: Json };
  try {
    overlay = JSON.parse(overlayText) as Json;
    effective = mergePatch(JSON.parse(baseText) as Json, overlay) as {
      [k: string]: Json;
    };
  } catch (e) {
    v(OVERLAY, "identity/config", `invalid JSON: ${(e as Error).message}`);
    return r;
  }

  const effText = JSON.stringify(effective, null, 2);
  for (const { needle, why } of FORBIDDEN_IDENTITY) {
    if (effText.includes(needle)) {
      const path = findJsonPath(effective, needle);
      v(
        OVERLAY,
        "identity/upstream",
        `effective config contains ${why} (${needle.slice(0, 24)}…) at ${path}; override it in the overlay`,
      );
    }
  }

  if (effective.productName !== PRODUCT_NAME) {
    v(
      OVERLAY,
      "identity/productName",
      `productName is ${JSON.stringify(effective.productName)}, expected "${PRODUCT_NAME}"`,
    );
  }
  if (effective.identifier !== IDENTIFIER) {
    v(
      OVERLAY,
      "identity/identifier",
      `identifier is ${JSON.stringify(effective.identifier)}, expected "${IDENTIFIER}"`,
    );
  }

  const updater = ((effective.plugins as { [k: string]: Json } | undefined)
    ?.updater ?? {}) as { pubkey?: Json; endpoints?: Json };
  if (typeof updater.pubkey !== "string" || updater.pubkey.trim() === "") {
    v(OVERLAY, "identity/pubkey", "plugins.updater.pubkey is missing");
  }
  const endpoints = Array.isArray(updater.endpoints) ? updater.endpoints : [];
  if (endpoints.length === 0) {
    v(OVERLAY, "identity/endpoint", "plugins.updater.endpoints is empty");
  }
  for (const ep of endpoints) {
    if (
      typeof ep !== "string" ||
      !/^https:\/\/github\.com\/[^/]+\/wispr-bro\/releases\//.test(ep)
    ) {
      v(
        OVERLAY,
        "identity/endpoint",
        `updater endpoint ${JSON.stringify(ep)} is not https://github.com/<owner>/wispr-bro/releases/…`,
      );
    }
  }

  // Versions (DEC-48).
  const overlayVersion =
    overlay && typeof overlay === "object" && !Array.isArray(overlay)
      ? overlay.version
      : undefined;
  const cargo = readIfExists(root, "src-tauri/Cargo.toml");
  const pkgText = readIfExists(root, "package.json");
  const cargoV = cargo ? cargoVersion(cargo) : undefined;
  const pkgV = pkgText
    ? (JSON.parse(pkgText) as { version?: string }).version
    : undefined;
  if (typeof overlayVersion !== "string") {
    v(OVERLAY, "identity/version", "overlay has no version");
  } else {
    if (cargoV !== overlayVersion) {
      v(
        "src-tauri/Cargo.toml",
        "identity/version",
        `[package] version ${cargoV} ≠ overlay version ${overlayVersion}`,
      );
    }
    if (pkgV !== overlayVersion) {
      v(
        "package.json",
        "identity/version",
        `version ${pkgV} ≠ overlay version ${overlayVersion}`,
      );
    }
    const tag =
      opts.tag ??
      (process.env.GITHUB_REF_TYPE === "tag"
        ? process.env.GITHUB_REF_NAME
        : undefined);
    if (tag && tag !== "beta" && tag !== `v${overlayVersion}`) {
      v(
        OVERLAY,
        "identity/tag",
        `git tag ${tag} ≠ v${overlayVersion} (overlay version)`,
      );
    }
  }

  // Placeholders.
  const placeholderHits: { file: string; line: number; ph: string }[] = [];
  const scan = (file: string, text: string) => {
    text.split("\n").forEach((l, i) => {
      for (const ph of PLACEHOLDERS)
        if (l.includes(ph)) placeholderHits.push({ file, line: i + 1, ph });
    });
  };
  scan(OVERLAY, overlayText);
  for (const f of listFiles(root)) {
    if (!isForkCodePath(f) || !isTextFile(f)) continue;
    // The guards define the placeholder list itself.
    if (/^scripts\/wb-guard/.test(f)) continue;
    scan(f, readIfExists(root, f) ?? "");
  }
  if (opts.release) {
    for (const h of placeholderHits) {
      v(
        h.file,
        "identity/placeholder",
        `F-02 placeholder ${h.ph} must be replaced before a release`,
        h.line,
      );
    }
  } else if (placeholderHits.length) {
    r.notes.push(
      `${placeholderHits.length} F-02 placeholder(s) allowed in PR mode (fatal with --release): ` +
        [...new Set(placeholderHits.map((h) => `${h.file}:${h.line}`))].join(
          ", ",
        ),
    );
  }
  return r;
}

function findJsonPath(obj: Json, needle: string, path = "$"): string {
  if (typeof obj === "string") return obj.includes(needle) ? path : "";
  if (obj && typeof obj === "object") {
    for (const [k, val] of Object.entries(obj)) {
      const p = findJsonPath(val, needle, `${path}.${k}`);
      if (p) return p;
    }
  }
  return "";
}

if (import.meta.main) {
  const argv = process.argv.slice(2);
  cliMain(() =>
    runIdentity({
      release: argv.includes("--release"),
      tag: argValue(argv, "--tag"),
    }),
  );
}
