// SPDX-License-Identifier: GPL-3.0-or-later
import assert from "node:assert/strict";
import {
  actionDefinition,
  actionIds,
  canAddBinding,
  defaultHotkeys,
  HOTKEY_ACTIONS,
  MAX_BINDINGS,
  nextBindingActivation,
  TRANSFORM_DEFAULTS,
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
assert.equal(canAddBinding([]), true);
assert.equal(
  canAddBinding(
    Array.from({ length: 4 }, () => ({
      chord: "fn",
      activation: "hold",
    })),
  ),
  false,
);

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
