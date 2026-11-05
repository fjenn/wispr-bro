// SPDX-License-Identifier: GPL-3.0-or-later
// Run: bun src/fork/tutorials/SuccessDetector.test.ts
// S-31: a practice step recognises its shortcuts from keydown events in
// the fork hotkey grammar (exact modifiers, layout-independent keys).
import assert from "node:assert/strict";
import {
  actionForEvent,
  eventKeyName,
  matchChord,
  type KeyLike,
} from "./SuccessDetector";

const key = (code: string, mods: Partial<KeyLike> = {}, k = ""): KeyLike => ({
  key: k,
  code,
  altKey: false,
  ctrlKey: false,
  metaKey: false,
  shiftKey: false,
  ...mods,
});

// ⌥1 types "¡" on a US layout: the code decides, not the character.
assert.equal(eventKeyName(key("Digit1", { altKey: true }, "¡")), "1");
assert.equal(eventKeyName(key("KeyO", { altKey: true }, "ø")), "o");
assert.ok(matchChord("option+1", key("Digit1", { altKey: true }, "¡")));
assert.ok(matchChord("alt+1", key("Digit1", { altKey: true })));
assert.ok(
  matchChord("ctrl+option+o", key("KeyO", { altKey: true, ctrlKey: true })),
);
// Exact modifiers: ⌃⌥1 is not ⌥1 and the other way round.
assert.ok(
  !matchChord("option+1", key("Digit1", { altKey: true, ctrlKey: true })),
);
assert.ok(!matchChord("ctrl+option+1", key("Digit1", { altKey: true })));
assert.ok(!matchChord("option+1", key("Digit1")));
assert.ok(!matchChord("option+1", key("Digit2", { altKey: true })));
// No key, modifier-only, fn or malformed chords never match a keydown.
assert.ok(!matchChord("option", key("AltLeft", { altKey: true }, "Alt")));
assert.ok(!matchChord("fn", key("Digit1")));
assert.ok(!matchChord("option+1+2", key("Digit1", { altKey: true })));
assert.ok(!matchChord("", key("Digit1")));
// Named keys come from event.key (Return in the hotkey grammar).
assert.ok(
  matchChord("command+return", key("Enter", { metaKey: true }, "Enter")),
);

const chords = {
  "transform.polish": ["option+1"],
  "transform.prompt_engineer": ["option+2", "ctrl+option+2"],
  "transform.view_changes": ["option+o"],
};
assert.equal(
  actionForEvent(chords, key("Digit1", { altKey: true })),
  "transform.polish",
);
assert.equal(
  actionForEvent(chords, key("Digit2", { altKey: true, ctrlKey: true })),
  "transform.prompt_engineer",
);
assert.equal(
  actionForEvent(chords, key("KeyO", { altKey: true })),
  "transform.view_changes",
);
assert.equal(actionForEvent(chords, key("KeyA")), null);
console.log("SuccessDetector: all assertions passed");
