// SPDX-License-Identifier: MIT
// Copyright (c) handy-computer (handy-keys 0.3.4, https://github.com/handy-computer/handy-keys).
// Modified by Wispr Bro contributors, 2026-11-17: iPhone-only subset without the
// desktop backends (see this crate's Cargo.toml).

//! Platform-specific keyboard utilities
//!
//! Wispr Bro (iOS): no platform backend. An iPhone app has no global
//! keyboard hook; `KeyboardListener::new` reports `Error::Platform`.

pub(crate) mod state;
