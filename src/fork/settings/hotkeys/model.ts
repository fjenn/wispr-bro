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

/**
 * How many shortcuts an action really takes (audit OT2). The engine
 * (`compile` in `src-tauri/src/fork/shortcut/bindings.rs`) runs dictation
 * on one trigger: Hold to talk is that trigger held, hands-free its
 * double-tap plus trigger+Space, cancel is Esc, Command Mode fn+Ctrl.
 * Every other action takes up to `MAX_BINDINGS`.
 */
export const ACTION_LIMITS: Readonly<Record<string, number>> = {
  dictate: 1,
  hands_free: 2,
  cancel: 1,
  command_mode: 1,
};

export function maxBindings(actionId: string): number {
  return ACTION_LIMITS[actionId] ?? MAX_BINDINGS;
}

export function canAddBinding(
  actionId: string,
  bindings: HotkeyBinding[],
): boolean {
  return bindings.length < maxBindings(actionId);
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

/**
 * The dictation key (owner test: "this was just about one key"). The engine
 * runs hold-to-talk, hands-free double-tap and key+Space on one trigger
 * (`Trigger::from_hotkey` in `src-tauri/src/fork/shortcut/decision.rs`):
 * fn alone, or two or more modifier groups without fn. Returns the chord
 * normalized the way `normalize` in `bindings.rs` writes it, or null when the
 * keys cannot be a trigger.
 */
export function triggerChord(chord: string): string | null {
  const groups = new Set<string>();
  for (const raw of chord.toLowerCase().split("+").filter(Boolean)) {
    const part = raw.replace(/^(left|right)[_ ]?|[_ ]?(left|right)$/g, "");
    if (part === "fn") groups.add("fn");
    else if (part === "ctrl" || part === "control") groups.add("ctrl");
    else if (part === "option" || part === "opt" || part === "alt")
      groups.add("option");
    else if (part === "shift") groups.add("shift");
    else if (["command", "cmd", "super", "meta"].includes(part))
      groups.add("command");
    else return null;
  }
  if (groups.has("fn")) return groups.size === 1 ? "fn" : null;
  if (groups.size < 2) return null;
  return ["ctrl", "option", "shift", "command"]
    .filter((group) => groups.has(group))
    .join("+");
}

/**
 * The bindings that move with the dictation key: hold to talk, the
 * hands-free double-tap and key+Space. Other hands-free bindings stay.
 */
export function dictationKeyBindings(
  hotkeys: ForkSettings["hotkeys"],
  key: string,
): { dictate: HotkeyBinding[]; hands_free: HotkeyBinding[] } {
  const previous =
    hotkeys.dictate?.find((binding) => binding.activation === "hold")?.chord ??
    "fn";
  const handsFree = hotkeys.hands_free ?? [];
  const rest = handsFree.filter(
    (binding) =>
      binding.activation !== "double_tap" &&
      binding.chord !== `${previous}+space`,
  );
  return {
    dictate: [{ chord: key, activation: "hold" }],
    hands_free: (
      [
        { chord: key, activation: "double_tap" },
        { chord: `${key}+space`, activation: "press" },
        ...rest,
      ] satisfies HotkeyBinding[]
    ).slice(0, maxBindings("hands_free")),
  };
}

/** The other action already using `key` or `key`+Space, if any. */
export function dictationKeyConflict(
  hotkeys: ForkSettings["hotkeys"],
  key: string,
): string | null {
  const taken = new Set([key, `${key}+space`]);
  for (const [action, bindings] of Object.entries(hotkeys)) {
    if (action === "dictate" || action === "hands_free") continue;
    const clash = (bindings ?? []).some((binding) =>
      taken.has(triggerChord(binding.chord) ?? binding.chord),
    );
    if (clash) return action;
  }
  return null;
}
