// SPDX-License-Identifier: GPL-3.0-or-later
// S-37b.1 helper: each count item of the dumped menu bar must equal the
// locale's plural form for its number (see plurals.sh).
import { readFileSync } from "fs";
import path from "path";

const [lang, dump] = process.argv.slice(2);
const root = path.join(import.meta.dir, "..", "..");
const strings = JSON.parse(
  readFileSync(path.join(root, "src/i18n/locales", lang, "fork.json"), "utf8"),
) as { rust: { menu: Record<string, string> } };
const rules = new Intl.PluralRules(lang);
const lines = readFileSync(dump, "utf8")
  .split("\n")
  .map((l) => l.trim());
let fail = 0;
for (const base of ["wordsDictated", "dictionaryWords", "snippetCount"]) {
  const forms = Object.entries(strings.rust.menu).filter(([k]) =>
    k.startsWith(`${base}_`),
  );
  // The line that matches some form of this group, with its number.
  let found = false;
  for (const line of lines) {
    const n = line.match(/\d+/)?.[0];
    if (!n) continue;
    const hit = forms.find(
      ([, f]) => f.replaceAll("{{count}}", n) === line.replace(/ \[.*\]$/, ""),
    );
    if (!hit) continue;
    found = true;
    const want = `${base}_${rules.select(Number(n))}`;
    const wanted = strings.rust.menu[want]?.replaceAll("{{count}}", n);
    const ok = wanted === line.replace(/ \[.*\]$/, "");
    console.log(
      `${ok ? "PASS" : "FAIL"} ${base} ${n}: "${line}", want ${want} "${wanted}"`,
    );
    if (!ok) fail = 1;
  }
  if (!found) {
    console.log(`FAIL ${base}: no menu item in ${lang} found`);
    fail = 1;
  }
}
process.exit(fail);
