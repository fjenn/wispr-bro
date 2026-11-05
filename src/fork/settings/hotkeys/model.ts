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
    .map((part) => {
      // The recorder writes one side ("option_left"); the cap is the same.
      const side = part.replace(/_(left|right)$/, "");
      const name = names[side] ? side : part;
      return names[name] ?? (name.length === 1 ? name.toUpperCase() : name);
    })
    .join(" ");
}

/**
 * The dictation key (owner test: "this was just about one key"). The engine
 * runs hold-to-talk and the hands-free double-tap on one trigger
 * (`Trigger::from_hotkey` in `src-tauri/src/fork/shortcut/decision.rs`): fn
 * alone, two or more modifier groups without fn, or a key with ⌃, ⌥ or ⌘
 * (such as ⌥Space or ⌃⌥D; never fn, Shift alone or Esc). The Globe key set
 * to Start Dictation records as fn plus its dictation keycode: that is fn.
 * Returns the chord normalized the way `normalize` in `bindings.rs` writes
 * it, or null when the keys cannot be a trigger.
 */
export function triggerChord(chord: string): string | null {
  const groups = new Set<string>();
  const keys: string[] = [];
  const modifier = (part: string): string | null => {
    const bare = part.replace(/^(left|right)[_ ]?|[_ ]?(left|right)$/g, "");
    if (bare === "fn") return "fn";
    if (bare === "ctrl" || bare === "control") return "ctrl";
    if (bare === "option" || bare === "opt" || bare === "alt") return "option";
    if (bare === "shift") return "shift";
    if (["command", "cmd", "super", "meta"].includes(bare)) return "command";
    return null;
  };
  for (const part of chord.toLowerCase().split("+").filter(Boolean)) {
    const group = modifier(part);
    if (group) groups.add(group);
    else if (["dictation", "globe", "voice"].includes(part)) groups.add("fn");
    else keys.push(part);
  }
  const mods = ["ctrl", "option", "shift", "command"].filter((group) =>
    groups.has(group),
  );
  if (keys.length > 1) return null;
  if (keys.length === 1) {
    const [key] = keys;
    const usable =
      !groups.has("fn") &&
      mods.some((group) => group !== "shift") &&
      !["escape", "esc"].includes(key) &&
      !key.startsWith("mouse");
    return usable ? [...mods, key].join("+") : null;
  }
  if (groups.has("fn")) return groups.size === 1 ? "fn" : null;
  if (mods.length < 2) return null;
  return mods.join("+");
}

/** A trigger with a key besides its modifiers (⌥Space), not fn or ⌃⌥. */
export function isKeyedTrigger(key: string): boolean {
  return key.split("+").some((part) => !TRIGGER_MODIFIERS.has(part));
}

const TRIGGER_MODIFIERS = new Set(["fn", "ctrl", "option", "shift", "command"]);

/**
 * The bindings that move with the dictation key: hold to talk, the
 * hands-free double-tap and key+Space (not for a keyed trigger such as
 * ⌥Space: ⌥Space+Space is not a chord). Other hands-free bindings stay.
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
        ...(isKeyedTrigger(key)
          ? []
          : [{ chord: `${key}+space`, activation: "press" as const }]),
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
  const taken = new Set(isKeyedTrigger(key) ? [key] : [key, `${key}+space`]);
  for (const [action, bindings] of Object.entries(hotkeys)) {
    if (action === "dictate" || action === "hands_free") continue;
    const clash = (bindings ?? []).some((binding) =>
      taken.has(triggerChord(binding.chord) ?? binding.chord),
    );
    if (clash) return action;
  }
  return null;
}
