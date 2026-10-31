// SPDX-License-Identifier: GPL-3.0-or-later
/**
 * The active Flow Bar (S-03): held, hands-free, processing, messages.
 *
 * The window is exactly the size of the shape it shows (sizes from Rust,
 * `geometry.gen.json`); the backend resizes it once this page has painted
 * a new state. Clicks work without taking focus from the frontmost app
 * (non-activating panel, `acceptsFirstMouse`; RS-03). Hover is not used
 * here: a web view gets no mouse-moved events while its app is inactive.
 */
import { useEffect } from "react";
import { useTranslation } from "react-i18next";
import { getLanguageDirection } from "@/lib/utils/rtl";
import { FORK_NAMESPACE } from "../i18n";
import { shapeSize } from "./geometry";
import { Listening } from "./states/Listening";
import { Message } from "./states/Message";
import { Working } from "./states/Working";
import { usePillState } from "./usePill";

export function Pill() {
  const { t, i18n } = useTranslation(FORK_NAMESPACE);
  const state = usePillState();

  useEffect(() => {
    document.documentElement.dir = getLanguageDirection(i18n.language);
    document.documentElement.lang = i18n.language;
  }, [i18n.language]);

  if (!state) return <div data-testid="wb-pill-idle" />;

  const size = shapeSize(state.shape);
  let body;
  switch (state.phase) {
    case "held":
    case "locked":
      body = <Listening state={state} />;
      break;
    case "transcribing":
    case "cleaning_up":
      body = <Working state={state} />;
      break;
    case "message":
      body = <Message state={state} />;
      break;
  }

  return (
    <div
      className="wb-pill"
      role="group"
      aria-label={t("pill.region")}
      data-testid="wb-pill"
      data-phase={state.phase}
      data-shape={state.shape}
      data-tone={state.tone ?? undefined}
      style={{ width: size.w, height: size.h }}
    >
      {body}
    </div>
  );
}
