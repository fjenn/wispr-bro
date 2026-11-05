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
 *
 * Design W5: the stage is a colourful wallpaper (our own CSS mesh
 * gradient) with the drawn app on it like a screenshot, the bar under it
 * and the spoken words as a dark caption pill under the bar; the pause
 * control sits in the stage's corner like a player's.
 */
import { useState } from "react";
import { useTranslation } from "react-i18next";
import { FORK_NAMESPACE } from "../../i18n";
import { PauseButton } from "../../tutorials/PauseButton";
import { CHOICE_ICONS, type FirstUseChoice } from "../firstUse";
import { DrawnBar } from "./DrawnBar";
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
        : "rest";
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
        className={
          still ? "wb-scene-stage" : "wb-scene-stage wb-scene-stage--controls"
        }
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
        <DrawnBar
          state={pill}
          text={pill === "work" ? t("pill.transcribing") : undefined}
          aria-hidden="true"
        />
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
      </div>
      {!still && (
        <PauseButton
          paused={paused}
          onToggle={() => setPaused((p) => !p)}
          pauseLabel={t("firstUse.scene.pause")}
          playLabel={t("firstUse.scene.play")}
          testId="wb-scene-pause"
          tone="hud"
          className="wb-scene-pause"
        />
      )}
    </div>
  );
}
