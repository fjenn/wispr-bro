// SPDX-License-Identifier: MIT
// Copyright (c) handy-computer (handy-keys 0.3.4, https://github.com/handy-computer/handy-keys).
// Modified by Wispr Bro contributors, 2026-11-17: iPhone-only subset without the
// desktop backends (see this crate's Cargo.toml).

//! Core types for keyboard shortcuts

mod hotkey;
mod key;
mod modifiers;

pub use hotkey::{Hotkey, HotkeyEvent, HotkeyId, HotkeyState, KeyEvent};
pub use key::Key;
pub use modifiers::Modifiers;
