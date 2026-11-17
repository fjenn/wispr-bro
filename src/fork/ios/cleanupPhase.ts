// SPDX-License-Identifier: GPL-3.0-or-later
import { useEffect, useState } from "react";
import { commands, type AppleAvailability } from "../bindings";

/**
 * Whether this iPhone can clean up dictations. Apple's on-device model
 * does it (`fork_llm_apple_status`), so it needs Apple Intelligence: iOS 26
 * on an iPhone that has it, switched on. Asked again every few seconds,
 * because the person may switch it on in Settings and come back.
 *
 * - `ready`: cleanup works.
 * - `preparing`: Apple's model is still downloading; levels can be chosen.
 * - `off`: Apple Intelligence is switched off.
 * - `none`: this iPhone or iOS version has no Apple Intelligence.
 */
export type CleanupPhase = "unknown" | "ready" | "preparing" | "off" | "none";

function phaseOf(availability: AppleAvailability): CleanupPhase {
  switch (availability) {
    case "available":
      return "ready";
    case "model_not_ready":
      return "preparing";
    case "not_enabled":
      return "off";
    default:
      return "none";
  }
}

export function useCleanupPhase(): CleanupPhase {
  const [phase, setPhase] = useState<CleanupPhase>("unknown");
  useEffect(() => {
    let alive = true;
    const ask = () =>
      commands
        .forkLlmAppleStatus()
        .then((status) => {
          if (alive) setPhase(status ? phaseOf(status.availability) : "none");
        })
        .catch(() => alive && setPhase("none"));
    void ask();
    const timer = window.setInterval(() => void ask(), 3000);
    return () => {
      alive = false;
      window.clearInterval(timer);
    };
  }, []);
  return phase;
}
