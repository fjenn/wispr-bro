// SPDX-License-Identifier: GPL-3.0-or-later
import type {
  ForkSettings,
  HotkeyActivation,
  HotkeyBinding,
} from "../../bindings";

export const MAX_BINDINGS = 4;

export interface HotkeyAction {
  id: string;
  activation: HotkeyActivation;
  defaults: HotkeyBinding[];
}

export const HOTKEY_ACTIONS: readonly HotkeyAction[] = [
  {
    id: "dictate",
    activation: "hold",
    defaults: [{ chord: "fn", activation: "hold" }],
  },
  {
    id: "hands_free",
    activation: "double_tap",
    defaults: [
      { chord: "fn", activation: "double_tap" },
      { chord: "fn+space", activation: "press" },
    ],
  },
  {
    id: "cancel",
    activation: "press",
    defaults: [{ chord: "escape", activation: "press" }],
  },
  {
    id: "command_mode",
    activation: "hold",
    defaults: [{ chord: "fn+ctrl", activation: "hold" }],
  },
  {
    id: "paste_last",
    activation: "press",
    defaults: [{ chord: "ctrl+command+v", activation: "press" }],
  },
  {
    id: "copy_last",
    activation: "press",
    defaults: [{ chord: "ctrl+command+c", activation: "press" }],
  },
  { id: "press_enter", activation: "press", defaults: [] },
  { id: "undo_cancel", activation: "press", defaults: [] },
  { id: "scratchpad.open", activation: "press", defaults: [] },
];

export function actionDefinition(id: string): HotkeyAction {
  return (
    HOTKEY_ACTIONS.find((action) => action.id === id) ?? {
      id,
      activation: "press",
      defaults: [],
    }
  );
}

export function actionIds(hotkeys: ForkSettings["hotkeys"]): string[] {
  return [
    ...HOTKEY_ACTIONS.map((action) => action.id),
    ...Object.keys(hotkeys)
      .filter((id) => !HOTKEY_ACTIONS.some((action) => action.id === id))
      .sort(),
  ];
}

/**
 * Built-in transform shortcuts (S-21/S-22), listed apart from
 * `HOTKEY_ACTIONS` so their rows keep their place after the core actions.
 * Same values as `default_hotkeys()` in `src-tauri/src/fork/settings.rs`.
 * Audit OT2: Reset to defaults saved these as unset before.
 */
export const TRANSFORM_DEFAULTS: Readonly<Record<string, HotkeyBinding[]>> = {
  "transform.polish": [{ chord: "option+1", activation: "press" }],
  "transform.prompt_engineer": [{ chord: "option+2", activation: "press" }],
  "transform.view_changes": [{ chord: "option+o", activation: "press" }],
};

/** Every action's default bindings, as `default_hotkeys()` in Rust. */
export function defaultHotkeys(): Record<string, HotkeyBinding[]> {
  return Object.fromEntries([
    ...HOTKEY_ACTIONS.map(
      (action) =>
        [
          action.id,
          action.defaults.map((binding) => ({ ...binding })),
        ] as const,
    ),
    ...Object.entries(TRANSFORM_DEFAULTS).map(
      ([id, bindings]) =>
        [id, bindings.map((binding) => ({ ...binding }))] as const,
    ),
  ]);
}

export function nextBindingActivation(
  actionId: string,
  bindings: HotkeyBinding[],
): HotkeyActivation {
  if (actionId === "hands_free" && bindings.length > 0) return "press";
  return actionDefinition(actionId).activation;
}

export function canAddBinding(bindings: HotkeyBinding[]): boolean {
  return bindings.length < MAX_BINDINGS;
}

export function chordLabel(chord: string): string {
  const names: Record<string, string> = {
    command: "⌘",
    cmd: "⌘",
    ctrl: "⌃",
    control: "⌃",
    option: "⌥",
    alt: "⌥",
    shift: "⇧",
    space: "Space",
    escape: "Esc",
    return: "Return",
  };
  return chord
    .split("+")
    .map(
      (part) => names[part] ?? (part.length === 1 ? part.toUpperCase() : part),
    )
    .join(" ");
}
