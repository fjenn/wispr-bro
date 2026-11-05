// SPDX-License-Identifier: GPL-3.0-or-later
import { create } from "zustand";
import { events, type ForkTransformViewChanges } from "../bindings";
import { navigate } from "../hub/nav";

interface TransformViewState {
  currentRun: ForkTransformViewChanges | null;
}

export const useTransformViewStore = create<TransformViewState>()(() => ({
  currentRun: null,
}));

let listening: Promise<void> | null = null;

/** Receives S-21's View changes handoff even when another Hub page is open. */
export function listenForTransformChanges(): Promise<void> {
  listening ??= events.forkTransformViewChanges
    .listen((event) => {
      useTransformViewStore.setState({ currentRun: event.payload });
      navigate("transforms", "changes");
    })
    .then(() => undefined)
    .catch((error) => {
      listening = null;
      console.warn("transforms: listening for View changes failed", error);
    });
  return listening;
}
