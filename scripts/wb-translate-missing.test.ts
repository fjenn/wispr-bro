// SPDX-License-Identifier: GPL-3.0-or-later
import { describe, expect, test } from "bun:test";
import { duplicateKeys } from "./wb-translate-missing";

describe("duplicateKeys", () => {
  test("finds a key repeated in the same object, with its path", () => {
    const text = `{"a":{"b":1,"c":"x\\"y","b":2},"d":[{"e":1},{"e":2}],"a":3}`;
    expect(duplicateKeys(text)).toEqual(["a.b", "a"]);
  });
  test("same key in sibling objects or array items is fine", () => {
    expect(
      duplicateKeys(`{"x":{"k":1},"y":{"k":2},"z":[{"k":1},{"k":1}]}`),
    ).toEqual([]);
  });
});
