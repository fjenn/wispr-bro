// SPDX-License-Identifier: GPL-3.0-or-later
/**
 * The Rust defaults of the fork settings (`ForkSettings::default()`), as
 * `fork_get_settings` sends them, for specs that mock it. Keep in step with
 * `src-tauri/src/fork/settings.rs` (a missing field breaks every section
 * that reads it).
 */
export const forkSettingsDefaults = {
  schema_version: 6,
  hotkeys: {
    cancel: [{ chord: "escape", activation: "press" }],
    dictate: [{ chord: "fn", activation: "hold" }],
    hands_free: [
      { chord: "fn", activation: "double_tap" },
      { chord: "fn+space", activation: "press" },
    ],
    paste_last: [{ chord: "ctrl+command+v", activation: "press" }],
    copy_last: [{ chord: "ctrl+command+c", activation: "press" }],
    "transform.polish": [{ chord: "option+1", activation: "press" }],
    "transform.prompt_engineer": [{ chord: "option+2", activation: "press" }],
  },
  cleanup_level: "medium",
  styles: {
    personal: "relaxed",
    work: "formal",
    email: "formal",
    other: "formal",
  },
  context_awareness: true,
  transforms_enabled: false,
  command_mode_enabled: false,
  auto_apply_transform: null,
  bar_always_visible: true,
  bar_position: "bottom",
  bar_hidden_until_ms: null,
  bar_hide_in_recordings: false,
  bar_x_by_display: {},
  live_text_in_pill: false,
  scratchpad_open_behavior: "resume_last",
  scratchpad_on_bar: false,
  scratchpad_text_size: 16,
  text_retention_days: null,
  audio_retention_days: 14,
  notifications: {
    system: true,
    model_ready: true,
    update_ready: true,
    queued_takes_ready: true,
    errors: true,
  },
  update_checks_enabled: true,
  update_channel: "stable",
  show_in_dock: true,
  onboarding: {
    status: "not_started",
    current_step: null,
    completed_steps: [],
    checklist_done: [],
    completed_at_ms: null,
  },
  dismissed_banners: [],
  seen_new_tags: [],
  first_use_choice: null,
  typing_wpm: null,
  typing_hours_per_day: 5.0,
  greeting_name: null,
  auto_learn_enabled: true,
  apple_dictation_shortcut_off: true,
  vibe_coding: {
    enabled: false,
    setup_completed: false,
    editors: [],
    file_tags_enabled: false,
  },
  applied_first_run_defaults: [],
};
