// SPDX-License-Identifier: GPL-3.0-or-later
/**
 * The label pill above the capsule (Design W4): the microphone name for
 * the first 2 s of a take, "Sends automatically" in its place (S-19a), a
 * warning ("stops soon", silence) or the Secure Input notice (S-02). It
 * blurs in and leaves on its own (`Pill.tsx`); the capsule keeps the
 * waveform meanwhile. Only on the labelled shapes (`geometry.ts`).
 */
import type { ForkPillState } from "../../bindings";
import { labelled } from "../geometry";
import { LockIcon, MicIcon, ReturnIcon } from "./icons";
import { secureInputText } from "./SecureInput";

export type BarLabelKind = "mic" | "sends" | "warning" | "secure";

export interface BarLabelContent {
  kind: BarLabelKind;
  text: string;
}

const TEST_IDS: Record<BarLabelKind, string> = {
  mic: "wb-pill-mic",
  sends: "wb-pill-sends",
  warning: "wb-pill-warning",
  secure: "wb-pill-secure-input",
};

/** What the label pill says for `state`, or `null` (no label). */
export function barLabel(
  state: ForkPillState,
  t: (key: string, opts?: Record<string, string>) => string,
): BarLabelContent | null {
  if (state.phase !== "held" && state.phase !== "locked") return null;
  if (!labelled(state.shape)) return null;
  if (state.warning === "secure_input") {
    return {
      kind: "secure",
      text: secureInputText(state, t),
    };
  }
  if (state.warning) {
    return { kind: "warning", text: t(`pill.warning.${state.warning}`) };
  }
  if (state.sends_automatically) {
    return { kind: "sends", text: t("pill.sendsAutomatically") };
  }
  if (state.mic) {
    return { kind: "mic", text: t("pill.mic", { name: state.mic }) };
  }
  return null;
}

export function BarLabel({ label }: { label: BarLabelContent }) {
  const icon =
    label.kind === "mic" ? (
      <MicIcon />
    ) : label.kind === "sends" ? (
      <ReturnIcon />
    ) : label.kind === "secure" ? (
      <LockIcon />
    ) : null;
  return (
    <span
      className="wb-pill-label"
      data-kind={label.kind}
      data-testid={TEST_IDS[label.kind]}
      title={label.text}
    >
      {icon}
      <span className="wb-pill-text">{label.text}</span>
    </span>
  );
}
