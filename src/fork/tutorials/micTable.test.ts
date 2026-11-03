// SPDX-License-Identifier: GPL-3.0-or-later
// Run: bun src/fork/tutorials/micTable.test.ts
// S-32: quiz → recommendation rules, over every combination of answers
// (4 comfortable kinds + unanswered, 3 workplaces + unanswered, 2 + 1
// moves, 16 sets of owned mics = 960), plus the cases the rules exist for.
import assert from "node:assert/strict";
import {
  DEFAULT_ANSWERS,
  MIC_KINDS,
  MIC_TRAITS,
  MOVES,
  WORKPLACES,
  micScore,
  recommend,
  type MicKind,
  type QuizAnswers,
} from "./micTable";

function subsets<T>(list: readonly T[]): T[][] {
  return list.reduce<T[][]>(
    (all, item) => [...all, ...all.map((s) => [...s, item])],
    [[]],
  );
}

let combos = 0;
const bestCounts = new Map<MicKind, number>();
for (const comfortable of [null, ...MIC_KINDS]) {
  for (const workplace of [null, ...WORKPLACES]) {
    for (const moves of [null, ...MOVES]) {
      for (const owned of subsets(MIC_KINDS)) {
        const answers: QuizAnswers = { comfortable, workplace, moves, owned };
        const { rows, best } = recommend(answers);
        combos += 1;
        bestCounts.set(best, (bestCounts.get(best) ?? 0) + 1);
        const label = JSON.stringify(answers);

        // One row per kind, in order, exactly one best match.
        assert.deepEqual(
          rows.map((r) => r.kind),
          [...MIC_KINDS],
          label,
        );
        assert.equal(rows.filter((r) => r.best).length, 1, label);
        assert.equal(rows.find((r) => r.best)?.kind, best, label);

        // The best match has the highest score among what can be had.
        const candidates = rows.filter(
          (r) => r.kind !== "built_in" || owned.includes("built_in"),
        );
        const top = Math.max(...candidates.map((r) => r.score));
        assert.equal(rows.find((r) => r.best)?.score, top, label);

        // Never a built-in mic the Mac does not have.
        if (!owned.includes("built_in")) {
          assert.notEqual(best, "built_in", label);
        }
        // Someone who moves around never gets the mic that stays on a desk.
        if (moves === "different_places") {
          assert.ok(MIC_TRAITS[best].portable, label);
        }
        // Whispering in shared spaces never ends on the built-in mic unless
        // the user finds it the comfortable one.
        if (workplace === "shared" && comfortable !== "built_in") {
          assert.notEqual(best, "built_in", label);
        }
        // The ratings shown never change with the answers.
        for (const row of rows) {
          assert.equal(row.quality, MIC_TRAITS[row.kind].quality);
          assert.equal(row.owned, owned.includes(row.kind));
        }
      }
    }
  }
}
assert.equal(combos, 5 * 4 * 3 * 16);
// Every kind is the best match for somebody.
for (const kind of MIC_KINDS) {
  assert.ok((bestCounts.get(kind) ?? 0) > 0, `${kind} is never recommended`);
}

const ask = (over: Partial<QuizAnswers>) =>
  recommend({ ...DEFAULT_ANSWERS, ...over }).best;

// Nothing answered: a headset (the closest mic for a whisper).
assert.equal(ask({}), "wired_headset");
// One quiet desk at home: the desk mic.
assert.equal(ask({ workplace: "private", moves: "one_desk" }), "desk_usb");
// At home, on the move, happy with the laptop's own mic: keep it.
assert.equal(
  ask({
    workplace: "private",
    moves: "different_places",
    comfortable: "built_in",
  }),
  "built_in",
);
// Shared office, moving around, earbuds owned and liked: the earbuds.
assert.equal(
  ask({
    workplace: "shared",
    moves: "different_places",
    comfortable: "wireless_earbuds",
    owned: ["built_in", "wireless_earbuds"],
  }),
  "wireless_earbuds",
);
// A Mac without a microphone, nothing else owned: never the built-in.
assert.notEqual(ask({ owned: [] }), "built_in");
// Owning one counts: a desk-bound user who has a headset keeps it.
assert.equal(
  ask({
    workplace: "mixed",
    moves: "one_desk",
    owned: ["built_in", "wired_headset"],
  }),
  "wired_headset",
);

// Scores are deterministic and the comfortable pick adds exactly 2.
const base = { ...DEFAULT_ANSWERS, workplace: "mixed" as const };
for (const kind of MIC_KINDS) {
  assert.equal(
    micScore(kind, { ...base, comfortable: kind }) - micScore(kind, base),
    2,
  );
}

console.log(`micTable: ok (${combos} combinations)`);
