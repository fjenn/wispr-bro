// SPDX-License-Identifier: GPL-3.0-or-later
// Run: bun src/fork/scratchpad/liveNote.test.ts
// The caret of an open note that another device changed.
import assert from "node:assert/strict";
import { mapCaret } from "./liveNote";

// Text added after the caret: the caret stays.
assert.equal(mapCaret("Hello world", "Hello world, again", 5), 5);
// Text added before the caret: the caret moves with its words.
assert.equal(mapCaret("world", "Hello world", 3), 9);
// Text removed before the caret.
assert.equal(mapCaret("Hello world", "world", 8), 2);
// Inside the changed part: its end.
assert.equal(mapCaret("a bad day", "a good day", 3), 5);
// At the end, and an emptied note.
assert.equal(mapCaret("abc", "abcd", 3), 3);
assert.equal(mapCaret("abc", "", 2), 0);
console.log("liveNote: ok");
