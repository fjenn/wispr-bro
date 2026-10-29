// SPDX-License-Identifier: GPL-3.0-or-later
/**
 * The `fork` i18n namespace.
 *
 * Strings live in `src/i18n/locales/<lng>/fork.json` (26 locales, English is
 * the source). Non-English files are kept in key parity by
 * `scripts/wb-translate-missing.ts`, which records copied or machine entries in
 * a sidecar `fork.flags.json` (never loaded at runtime).
 *
 * A top-level `overrides` object in a `fork.json` (added by F-01 for brand
 * overrides) is merged into Handy's `translation` namespace instead of the
 * `fork` namespace, so Handy's `translation.json` files are never edited.
 */
import type { i18n as I18n } from "i18next";

export const FORK_NAMESPACE = "fork";

type ForkResource = Record<string, unknown> & {
  overrides?: Record<string, unknown>;
};

const forkModules = import.meta.glob<{ default: ForkResource }>(
  "../i18n/locales/*/fork.json",
  { eager: true },
);

/** `{ [lng]: fork.json contents }` for every locale that has a file. */
export const forkResources: Record<string, ForkResource> = {};
for (const [path, module] of Object.entries(forkModules)) {
  const lng = path.match(/\/locales\/([^/]+)\/fork\.json$/)?.[1];
  if (lng) forkResources[lng] = module.default;
}

/** Adds the `fork` namespace (and any Handy-key overrides) to `i18n`. */
export function registerForkI18n(i18n: I18n): void {
  for (const [lng, resource] of Object.entries(forkResources)) {
    const { overrides, ...strings } = resource;
    i18n.addResourceBundle(lng, FORK_NAMESPACE, strings, true, true);
    if (overrides && Object.keys(overrides).length > 0) {
      i18n.addResourceBundle(lng, "translation", overrides, true, true);
    }
  }
}
