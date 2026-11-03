// SPDX-License-Identifier: GPL-3.0-or-later
/**
 * The bar spotlight's page (S-30, A-16): the whole display dimmed except a
 * rounded cut-out around the Flow Bar, a soft ring on the cut-out, and the
 * hint beside it (above the bar at the bottom, below it at the top):
 * "Next step · Open your email", how to dictate, and "Press Esc to close
 * this hint". The window ignores the mouse (every click reaches the app
 * below), so the page has no buttons.
 *
 * State: `ForkSpotlightState` events and `fork_spotlight_state` once the
 * listener is in place; a lower `seq` never replaces a higher one. The
 * shortcut wording follows the hotkey profile (fn, or Control+Option).
 */
import { useEffect, useState } from "react";
import { useTranslation } from "react-i18next";
import { getLanguageDirection } from "@/lib/utils/rtl";
import { commands, events, type ForkSpotlightState } from "../bindings";
import { FORK_NAMESPACE } from "../i18n";
import { howKey } from "../onboarding/firstUseStore";

/** Gap between the cut-out and the hint card. */
const HINT_GAP = 14;

function newer(
  prev: ForkSpotlightState | null,
  next: ForkSpotlightState,
): ForkSpotlightState {
  return !prev || next.seq >= prev.seq ? next : prev;
}

export function useSpotlightState(): ForkSpotlightState | null {
  const [state, setState] = useState<ForkSpotlightState | null>(null);
  useEffect(() => {
    let alive = true;
    const off = events.forkSpotlightState.listen(({ payload }) => {
      setState((prev) => newer(prev, payload));
    });
    void off.then(async () => {
      try {
        const now = await commands.forkSpotlightState();
        if (alive) setState((prev) => newer(prev, now));
      } catch (e) {
        console.warn("spotlight: state failed", e);
      }
    });
    return () => {
      alive = false;
      void off.then((f) => f()).catch(() => undefined);
    };
  }, []);
  return state;
}

export function Spotlight() {
  const { t, i18n } = useTranslation(FORK_NAMESPACE);
  const state = useSpotlightState();
  const [trigger, setTrigger] = useState<string | null>("fn");

  useEffect(() => {
    document.documentElement.dir = getLanguageDirection(i18n.language);
    document.documentElement.lang = i18n.language;
  }, [i18n.language]);

  useEffect(() => {
    void commands
      .forkHotkeyStatus()
      .then((r) => r.status === "ok" && setTrigger(r.data.trigger))
      .catch(() => undefined);
  }, []);

  if (!state?.active || !state.choice || !state.hole) {
    return <div data-testid="wb-spotlight" data-active="false" />;
  }
  const hole = state.hole;
  const bottom = state.edge === "bottom";
  const radius = hole.h / 2;
  const hintStyle = bottom
    ? { bottom: `calc(100% - ${hole.y - HINT_GAP}px)` }
    : { top: `${hole.y + hole.h + HINT_GAP}px` };

  return (
    <div
      data-testid="wb-spotlight"
      data-active="true"
      data-edge={state.edge}
      className="wb-spotlight"
    >
      <svg
        className="wb-spotlight-dim"
        width="100%"
        height="100%"
        aria-hidden="true"
      >
        <defs>
          <mask id="wb-spotlight-hole">
            <rect width="100%" height="100%" fill="white" />
            <rect
              x={hole.x}
              y={hole.y}
              width={hole.w}
              height={hole.h}
              rx={radius}
              fill="black"
            />
          </mask>
        </defs>
        <rect
          width="100%"
          height="100%"
          className="wb-spotlight-shade"
          mask="url(#wb-spotlight-hole)"
        />
        <rect
          x={hole.x}
          y={hole.y}
          width={hole.w}
          height={hole.h}
          rx={radius}
          className="wb-spotlight-ring"
          data-testid="wb-spotlight-hole"
        />
      </svg>
      <div
        role="status"
        data-testid="wb-spotlight-hint"
        className="wb-spotlight-hint"
        style={{
          left: `${hole.x + hole.w / 2}px`,
          ...hintStyle,
        }}
      >
        <p className="wb-spotlight-label">{t("rust.spotlight.label")}</p>
        <p className="wb-spotlight-title">
          {t(`rust.spotlight.hint.${state.choice}`)}
        </p>
        <p className="wb-spotlight-how">{t(howKey(trigger))}</p>
        <p className="wb-spotlight-esc">{t("rust.spotlight.escape")}</p>
      </div>
    </div>
  );
}
