// SPDX-License-Identifier: GPL-3.0-or-later
// Run: bun src/fork/hub/checklistRegistry.test.ts
// S-30: later slices add checklist items without changes to the card or
// the registry; order, replacement, removal, progress and the next item.
import assert from "node:assert/strict";
import { Circle } from "lucide-react";
import {
  checklistItems,
  checklistProgress,
  nextOpenItem,
  registerChecklistItem,
  type ChecklistItem,
} from "./checklistRegistry";

const item = (id: string, order: number): ChecklistItem => ({
  id,
  order,
  icon: Circle,
  titleKey: `checklist.items.${id}.title`,
  actionKey: `checklist.items.${id}.action`,
  run: () => undefined,
});

const offA = registerChecklistItem(item("try_in_app", 10));
registerChecklistItem(item("launch_at_login", 40));
registerChecklistItem(item("add_word", 20));
assert.deepEqual(
  checklistItems().map((i) => i.id),
  ["try_in_app", "add_word", "launch_at_login"],
);

// A later slice (S-31) registers its own item in between, from its module.
const offT = registerChecklistItem(item("try_transform", 35));
assert.deepEqual(
  checklistItems().map((i) => i.id),
  ["try_in_app", "add_word", "try_transform", "launch_at_login"],
);

// Same id replaces (a hot reload or a second import never duplicates).
registerChecklistItem({ ...item("try_transform", 36), titleKey: "x" });
assert.equal(
  checklistItems().filter((i) => i.id === "try_transform").length,
  1,
);
assert.equal(checklistItems()[2].titleKey, "x");

// Progress counts only registered items; ticks of unknown ids are ignored.
const done = ["add_word", "gone_item"];
assert.deepEqual(checklistProgress(checklistItems(), done), {
  done: 1,
  total: 4,
});
assert.equal(nextOpenItem(checklistItems(), done)?.id, "try_in_app");
assert.equal(
  nextOpenItem(checklistItems(), ["try_in_app", "add_word"])?.id,
  "try_transform",
);
assert.equal(
  nextOpenItem(checklistItems(), [
    "try_in_app",
    "add_word",
    "try_transform",
    "launch_at_login",
  ]),
  undefined,
);

// Removal.
offA();
offT(); // the replaced registration is a different object: still there
assert.deepEqual(
  checklistItems().map((i) => i.id),
  ["add_word", "try_transform", "launch_at_login"],
);

console.log("checklistRegistry: ok");
