// SPDX-License-Identifier: GPL-3.0-or-later
import assert from "node:assert/strict";
import {
  actionDefinition,
  actionIds,
  canAddBinding,
  chordLabel,
  defaultHotkeys,
  HOTKEY_ACTIONS,
  MAX_BINDINGS,
  maxBindings,
  nextBindingActivation,
  TRANSFORM_DEFAULTS,
  dictationKeyBindings,
  dictationKeyConflict,
  triggerChord,
} from "./model";

const first = defaultHotkeys();
const second = defaultHotkeys();
assert.deepEqual(Object.keys(first), [
  ...HOTKEY_ACTIONS.map((action) => action.id),
  ...Object.keys(TRANSFORM_DEFAULTS),
]);
// Audit OT2: Reset to defaults restores the built-in transform shortcuts.
assert.deepEqual(first["transform.polish"], [
  { chord: "option+1", activation: "press" },
]);
assert.deepEqual(first["transform.view_changes"], [
  { chord: "option+o", activation: "press" },
]);
first["transform.polish"][0].chord = "option+9";
assert.equal(second["transform.polish"][0].chord, "option+1");
assert.deepEqual(first.dictate, [{ chord: "fn", activation: "hold" }]);
assert.deepEqual(first.press_enter, []);
first.dictate[0].chord = "ctrl+option";
assert.equal(second.dictate[0].chord, "fn");

assert.equal(MAX_BINDINGS, 4);
const press = (n: number) =>
  Array.from({ length: n }, () => ({
    chord: "f5",
    activation: "press" as const,
  }));
assert.equal(canAddBinding("paste_last", []), true);
assert.equal(canAddBinding("paste_last", press(3)), true);
assert.equal(canAddBinding("paste_last", press(4)), false);
// Audit OT2: dictation and cancel take only the keys the engine runs on.
assert.equal(maxBindings("dictate"), 1);
assert.equal(maxBindings("hands_free"), 2);
assert.equal(maxBindings("cancel"), 1);
assert.equal(maxBindings("command_mode"), 1);
assert.equal(maxBindings("transform.custom-3"), MAX_BINDINGS);
assert.equal(canAddBinding("dictate", first.dictate), false);
assert.equal(canAddBinding("dictate", []), true);
assert.equal(canAddBinding("hands_free", first.hands_free), false);
assert.equal(canAddBinding("hands_free", first.hands_free.slice(1)), true);
assert.equal(canAddBinding("cancel", first.cancel), false);

const withRuntimeAction = actionIds({ "scratchpad.open": [] });
assert.equal(
  withRuntimeAction[withRuntimeAction.length - 1],
  "scratchpad.open",
);
assert.equal(actionDefinition("scratchpad.open").activation, "press");
assert.equal(nextBindingActivation("hands_free", []), "double_tap");
assert.equal(
  nextBindingActivation("hands_free", [
    { chord: "fn", activation: "double_tap" },
  ]),
  "press",
);

// The dictation key: fn alone, two modifier groups, or modifiers + a key.
assert.equal(triggerChord("fn"), "fn");
assert.equal(triggerChord("option+ctrl"), "ctrl+option");
assert.equal(triggerChord("cmd+shift"), "shift+command");
assert.equal(triggerChord("right_option+left_ctrl"), "ctrl+option");
assert.equal(triggerChord("option"), null);
assert.equal(triggerChord("fn+ctrl"), null);
assert.equal(triggerChord("a"), null);
// Owner report (rc8): fn from a Globe key set to Start Dictation, and the
// shortcuts people expect.
assert.equal(triggerChord("fn+dictation"), "fn");
assert.equal(chordLabel("option_left+space"), "⌥ Space");
assert.equal(triggerChord("option+space"), "option+space");
assert.equal(triggerChord("ctrl+space"), "ctrl+space");
assert.equal(triggerChord("option_left+ctrl_right+d"), "ctrl+option+d");
assert.equal(triggerChord("command+shift+k"), "shift+command+k");
assert.equal(triggerChord("shift+a"), null);
assert.equal(triggerChord("fn+space"), null);
assert.equal(triggerChord("option+escape"), null);
assert.deepEqual(dictationKeyBindings(defaultHotkeys(), "option+space"), {
  dictate: [{ chord: "option+space", activation: "hold" }],
  hands_free: [{ chord: "option+space", activation: "double_tap" }],
});
assert.deepEqual(dictationKeyBindings(first, "ctrl+option"), {
  dictate: [{ chord: "ctrl+option", activation: "hold" }],
  hands_free: [
    { chord: "ctrl+option", activation: "double_tap" },
    { chord: "ctrl+option+space", activation: "press" },
  ],
});
assert.equal(dictationKeyConflict(first, "fn"), null);
assert.equal(
  dictationKeyConflict(
    { ...first, paste_last: [{ chord: "option+ctrl", activation: "press" }] },
    "ctrl+option",
  ),
  "paste_last",
);
