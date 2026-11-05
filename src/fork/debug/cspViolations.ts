// SPDX-License-Identifier: GPL-3.0-or-later
/**
 * Records every `securitypolicyviolation` in the page from the moment the
 * fork frontend starts (F-08, DEC-43), so the CSP smoke check also reports
 * violations from page load, not only from its own probes. Tiny and loaded
 * eagerly by `src/fork/index.ts`; the smoke check itself is lazy.
 */

export interface CspViolation {
  directive: string;
  blockedURI: string;
  sourceFile: string;
  sample: string;
}

const recorded: CspViolation[] = [];
let installed = false;

/** Starts recording (idempotent). */
export function installCspViolationRecorder(): void {
  if (installed || typeof document === "undefined") return;
  installed = true;
  document.addEventListener("securitypolicyviolation", (e) => {
    recorded.push({
      directive: e.effectiveDirective || e.violatedDirective,
      blockedURI: e.blockedURI,
      sourceFile: e.sourceFile,
      sample: e.sample,
    });
  });
}

/** Violations recorded so far (a copy). */
export function cspViolations(): CspViolation[] {
  return recorded.slice();
}
