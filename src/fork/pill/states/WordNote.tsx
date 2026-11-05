// SPDX-License-Identifier: GPL-3.0-or-later
/**
 * S-34: "New word: Kubernetes" after auto-learn picked up a correction.
 * Add puts the word into the dictionary at once ("Added to Dictionary"
 * follows briefly); ✕ or the 15 s countdown leaves it waiting in
 * Dictionary › Suggested. A thin ring around ✕ shows the time left; the
 * backend pauses it while the pointer is over the note (`paused`), and
 * sends the time left with every change, so the ring restarts from there.
 * Reduce Motion: the ring stands still at the time left.
 */
import { useEffect, useRef } from "react";
import { useReducedMotion } from "motion/react";
import { useTranslation } from "react-i18next";
import { commands, type ForkPillWord } from "../../bindings";
import { FORK_NAMESPACE } from "../../i18n";
import { CheckIcon, CloseIcon, InfoIcon, SparkIcon } from "./icons";

/** The ring's radius and length (a 24 × 24 box around the 22 pt ✕). */
const R = 11;
const LENGTH = 2 * Math.PI * R;

export function WordNote({ word }: { word: ForkPillWord }) {
  const { t } = useTranslation(FORK_NAMESPACE);
  if (word.result) {
    const added = word.result === "added";
    return (
      <span
        className="wb-pill-message wb-pill-word-result"
        role="status"
        data-testid="wb-pill-word-result"
        data-result={word.result}
      >
        {added ? <CheckIcon /> : <InfoIcon />}
        <span className="wb-pill-text">
          {t(added ? "rust.pill.word.added" : "rust.pill.word.full")}
        </span>
      </span>
    );
  }
  return (
    <>
      <span
        className="wb-pill-message wb-pill-word"
        role="status"
        data-testid="wb-pill-word"
      >
        <SparkIcon />
        <span className="wb-pill-text">
          <span className="wb-pill-word-label">{t("pill.word.new")}</span>{" "}
          <strong className="wb-pill-word-term" dir="auto">
            {word.word}
          </strong>
        </span>
      </span>
      <button
        type="button"
        className="wb-pill-fix"
        data-testid="wb-pill-word-add"
        aria-label={t("pill.word.addLabel", { word: word.word })}
        onClick={() => void commands.forkPillAction("add_word")}
      >
        {t("pill.word.add")}
      </button>
      <span className="wb-pill-word-close">
        <Countdown word={word} />
        <button
          type="button"
          className="wb-pill-disc wb-pill-dismiss"
          data-testid="wb-pill-dismiss"
          aria-label={t("pill.word.notNow")}
          title={t("pill.word.notNow")}
          onClick={() => void commands.forkPillAction("dismiss")}
        >
          <CloseIcon />
        </button>
      </span>
    </>
  );
}

/** The time left, as a ring that empties (paused: it waits). */
function Countdown({ word }: { word: ForkPillWord }) {
  const reduced = useReducedMotion() ?? false;
  const ring = useRef<SVGCircleElement>(null);
  const left = word.total_ms > 0 ? word.remaining_ms / word.total_ms : 0;
  const from = LENGTH * (1 - Math.min(1, Math.max(0, left)));

  useEffect(() => {
    const el = ring.current;
    if (!el || reduced || word.paused || typeof el.animate !== "function")
      return;
    const run = el.animate(
      [{ strokeDashoffset: from }, { strokeDashoffset: LENGTH }],
      { duration: word.remaining_ms, easing: "linear", fill: "forwards" },
    );
    return () => run.cancel();
  }, [from, reduced, word.paused, word.remaining_ms, word.id]);

  return (
    <svg
      className="wb-pill-word-ring"
      viewBox="0 0 26 26"
      aria-hidden="true"
      focusable="false"
      data-testid="wb-pill-word-ring"
      data-paused={word.paused ? "true" : undefined}
    >
      <circle
        ref={ring}
        cx="13"
        cy="13"
        r={R}
        strokeDasharray={LENGTH}
        strokeDashoffset={from}
      />
    </svg>
  );
}
