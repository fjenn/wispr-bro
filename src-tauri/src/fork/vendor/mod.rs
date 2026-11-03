// SPDX-License-Identifier: GPL-3.0-or-later
//! Vendored third-party code (R5 §4.1 `fork/vendor/`). Each module keeps its
//! upstream licence header and notes what was changed; `docs/fork/PORTS.md`
//! and `NOTICE` list them.

/// axwatch 0.1.0 (MIT OR Apache-2.0): the Accessibility-grant notifications
/// behind the hotkey health check (S-02, `fork::shortcut::health`).
#[allow(dead_code)]
pub mod axwatch;
