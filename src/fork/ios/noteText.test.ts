// SPDX-License-Identifier: GPL-3.0-or-later
// Run: bun src/fork/ios/noteText.test.ts
// A take dictated with a note's Record button lands at the caret with the
// spaces a typed word would have; the word count.
import assert from "node:assert/strict";
import { countWords, insertAtCaret } from "./noteText";

assert.deepEqual(insertAtCaret("", "Hello there.", null), {
  body: "Hello there.",
  caret: 12,
});
// After a word: one space in front. No caret: the end of the note.
assert.deepEqual(insertAtCaret("First.", "Second.", null), {
  body: "First. Second.",
  caret: 14,
});
// In the middle of a line: a space on both sides, the caret after the take.
assert.deepEqual(insertAtCaret("one three", "two", { start: 3, end: 3 }), {
  body: "one two three",
  caret: 7,
});
// After a space or a new line, and before punctuation: nothing added.
assert.equal(
  insertAtCaret("one \nthree", "two", { start: 5, end: 5 }).body,
  "one \ntwo three",
);
assert.equal(
  insertAtCaret("(done)", "all", { start: 1, end: 1 }).body,
  "(all done)",
);
assert.equal(
  insertAtCaret("Wait.", "for it", { start: 4, end: 4 }).body,
  "Wait for it.",
);
// A selection is replaced.
assert.deepEqual(insertAtCaret("keep old text", "new", { start: 5, end: 8 }), {
  body: "keep new text",
  caret: 8,
});
// A caret past the end is the end.
assert.equal(insertAtCaret("a", "b", { start: 99, end: 120 }).body, "a b");

// Two takes in a row into the same note: one space between the sentences.
{
  const first = insertAtCaret("", "And ask Sam about the spare key.", null);
  const second = insertAtCaret(
    first.body,
    "Remember to book the train tickets.",
    { start: first.caret, end: first.caret },
  );
  assert.equal(
    second.body,
    "And ask Sam about the spare key. Remember to book the train tickets.",
  );
  // The same without a caret (the note never had the keyboard).
  assert.equal(
    insertAtCaret(first.body, "Next.", null).body,
    `${first.body} Next.`,
  );
}
// After a letter, a digit or closing punctuation: a space.
for (const before of [
  "word",
  "42",
  "done.",
  "why?",
  "(aside)",
  "“quoted”",
  "50%",
]) {
  assert.equal(insertAtCaret(before, "Next", null).body, `${before} Next`);
}
// At the start, after white space or a new line, after an opener: none.
for (const before of [
  "",
  "word ",
  "line\n",
  "tab\t",
  "(",
  "[",
  "“",
  "«",
  "¿",
]) {
  assert.equal(insertAtCaret(before, "Next", null).body, `${before}Next`);
}
// After a symbol or a dash: none (the Mac's rule).
for (const before of ["@", "#", "a-", "x/"]) {
  assert.equal(insertAtCaret(before, "Next", null).body, `${before}Next`);
}
// A take that starts with closing punctuation attaches.
assert.equal(insertAtCaret("Wait", "... and go", null).body, "Wait... and go");
// The text after the caret already starts with a space: not doubled.
assert.deepEqual(insertAtCaret("one three", "two", { start: 3, end: 3 }), {
  body: "one two three",
  caret: 7,
});
assert.equal(
  insertAtCaret("one  three", "two", { start: 4, end: 4 }).body,
  "one two three",
);
// White space around the take itself is dropped.
assert.equal(insertAtCaret("A.", "  B.\n", null).body, "A. B.");

assert.equal(countWords(""), 0);
assert.equal(countWords("  \n "), 0);
assert.equal(countWords("Milk, eggs and two lemons."), 5);
assert.equal(countWords("don't stop\nme now"), 4);
console.log("noteText: ok");
