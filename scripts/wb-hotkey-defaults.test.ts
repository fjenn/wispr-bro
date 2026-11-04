// SPDX-License-Identifier: GPL-3.0-or-later
// Run: bun test scripts/wb-hotkey-defaults.test.ts  (also part of `bun run wb:test:scripts`)
//
// Audit OT2: Settings › Shortcuts › Reset to defaults writes the defaults of
// `src/fork/settings/hotkeys/model.ts`, while the app starts from
// `default_hotkeys()` in `src-tauri/src/fork/settings.rs`. The two drifted
// (the transform shortcuts were missing, so Reset unbound ⌥1, ⌥2 and ⌥O);
// this keeps them equal.
import { expect, test } from "bun:test";
import { readFileSync } from "fs";
import { join } from "path";
import { defaultHotkeys } from "../src/fork/settings/hotkeys/model";

const ROOT = join(import.meta.dir, "..");
const ACTIVATIONS: Record<string, string> = {
  Hold: "hold",
  Toggle: "toggle",
  DoubleTap: "double_tap",
  Press: "press",
};

function rustDefaults(): Record<
  string,
  { chord: string; activation: string }[]
> {
  const src = readFileSync(
    join(ROOT, "src-tauri/src/fork/settings.rs"),
    "utf8",
  );
  const consts = new Map<string, string>();
  for (const m of src.matchAll(/pub const ([A-Z_]+): &str = "([a-z_.]+)";/g))
    consts.set(m[1], m[2]);
  const body = src.split("pub fn default_hotkeys()")[1].split("\n}\n")[0];
  const out: Record<string, { chord: string; activation: string }[]> = {};
  const entry =
    /\(\s*(?:"([a-z_.]+)"|([A-Z_]+))\.into\(\),\s*vec!\[([\s\S]*?)\],?\s*\)/g;
  for (const m of body.matchAll(entry)) {
    const id = m[1] ?? consts.get(m[2]);
    if (!id) throw new Error(`unknown action constant ${m[2]}`);
    out[id] = [
      ...m[3].matchAll(/HotkeyBinding::new\("([^"]+)",\s*(\w+)\)/g),
    ].map((b) => ({ chord: b[1], activation: ACTIVATIONS[b[2]] }));
  }
  return out;
}

test("Reset to defaults restores what a fresh install starts with", () => {
  const rust = rustDefaults();
  expect(Object.keys(rust).length).toBeGreaterThan(8);
  const ts = defaultHotkeys();
  // Rust leaves an unset action out; the panel saves it as an empty list.
  const bound = Object.fromEntries(
    Object.entries(ts).filter(([, bindings]) => bindings.length > 0),
  );
  expect(bound).toEqual(rust);
});
