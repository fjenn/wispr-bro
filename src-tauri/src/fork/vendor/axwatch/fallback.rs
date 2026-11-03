// SPDX-License-Identifier: MIT OR Apache-2.0
// Copyright (c) 2026 AprilNEA. axwatch 0.1.0, https://github.com/AprilNEA/axwatch
// (commit 89f73ea349c21de6ee38b1c4089b3e4441d32723), src/fallback.rs.
// Vendored by Wispr Bro contributors, 2026-11-02 (S-02); changes are listed in mod.rs.

//! Every platform but macOS: there is no Accessibility grant, so nothing to
//! watch. Trust is unconditional and a watch delivers only its heartbeat.

use std::io;

use super::Handler;

pub(crate) fn is_trusted() -> bool {
    true
}

pub(crate) fn request() {}

/// Nothing to register: the handler is dropped, and only the heartbeat's
/// clone of it ever runs.
pub(crate) struct Observer;

impl Observer {
    #[allow(
        clippy::unnecessary_wraps,
        reason = "the macOS registration can fail; this shares its signature"
    )]
    pub(crate) fn install(_handler: Handler) -> io::Result<Self> {
        Ok(Self)
    }
}
