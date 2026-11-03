// SPDX-License-Identifier: GPL-3.0-or-later
/**
 * One first-use scene (S-30): a drawn app window for the chosen use, the
 * spoken words as a large caption, the Flow Bar listening under it, then
 * the clean text landing in the field. Every scene shares this stage; the
 * per-choice content is in `fork.json` › `firstUse.scenes.<id>` (our own
 * wording, never Wispr Flow's). The whole stage is one image to assistive
 * tech (its label names the example and says what is written); a Pause
 * button stops the loop (WCAG 2.2.2), and reduced motion shows the
 * finished frame only.
 */
import { useState } from "react";
import { Pause, Play } from "../../ui/icons";
import { useTranslation } from "react-i18next";
import { FORK_NAMESPACE } from "../../i18n";
import { CHOICE_ICONS, type FirstUseChoice } from "../firstUse";
import { useSceneLoop } from "./engine";
import "./scenes.css";

export function Scene({ choice }: { choice: FirstUseChoice }) {
  const { t } = useTranslation(FORK_NAMESPACE);
  const [paused, setPaused] = useState(false);
  const said = t(`firstUse.scenes.${choice}.said`);
  const words = said.split(/\s+/).filter(Boolean);
  const { frame, still } = useSceneLoop(choice, words.length, paused);
  const Icon = CHOICE_ICONS[choice];
  const pill =
    frame.beat === "press" || frame.beat === "speak"
      ? "rec"
      : frame.beat === "release"
        ? "work"
        : "hidden";
  const landed = frame.beat === "land";
  const appName = t("rust.appName");
  const text = t(`firstUse.scenes.${choice}.text`);

  return (
    <div className="wb-scene" data-testid="wb-scene" data-choice={choice}>
      <div
        role="img"
        aria-label={`${t("firstUse.scene.label", {
          choice: t(`firstUse.choices.${choice}`),
        })}. ${t("firstUse.scene.written", { appName })}: ${text}`}
        className="wb-scene-stage"
        data-beat={frame.beat}
      >
        <div className="wb-scene-window" aria-hidden="true">
          <div className="wb-scene-titlebar">
            <span className="wb-scene-dots">
              <i />
              <i />
              <i />
            </span>
            <Icon size={14} aria-hidden="true" />
            <span>{t(`firstUse.scenes.${choice}.app`)}</span>
          </div>
          <div className="wb-scene-body">
            <p className="wb-scene-context">
              {t(`firstUse.scenes.${choice}.context`)}
            </p>
            <div className="wb-scene-field" data-filled={landed || undefined}>
              <span
                key={landed ? "landed" : "empty"}
                className={landed && !still ? "wb-scene-landed" : undefined}
                data-testid="wb-scene-text"
              >
                {landed ? text : ""}
              </span>
              <span className="wb-scene-caret" />
            </div>
          </div>
        </div>
        <div className="wb-scene-caption" aria-hidden="true">
          <span className="wb-scene-caption-label">
            {t("firstUse.scene.said")}
          </span>
          <span className="wb-scene-caption-words" data-testid="wb-scene-said">
            {words.map((word, i) => (
              <span
                key={`${word}-${i}`}
                className={i < frame.words ? "is-on" : undefined}
              >
                {word}{" "}
              </span>
            ))}
          </span>
        </div>
        <div className="wb-scene-pill" data-state={pill} aria-hidden="true">
          {pill === "rec" && (
            <span className="wb-scene-bars">
              {Array.from({ length: 7 }, (_, i) => (
                <i key={i} style={{ animationDelay: `${i * 90}ms` }} />
              ))}
            </span>
          )}
          {pill === "work" && <span className="wb-scene-spin" />}
        </div>
      </div>
      {!still && (
        <button
          type="button"
          className="wb-scene-pause"
          onClick={() => setPaused((p) => !p)}
          data-testid="wb-scene-pause"
        >
          {paused ? (
            <Play size={14} aria-hidden="true" />
          ) : (
            <Pause size={14} aria-hidden="true" />
          )}
          {paused ? t("firstUse.scene.play") : t("firstUse.scene.pause")}
        </button>
      )}
    </div>
  );
}
