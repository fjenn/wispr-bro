// SPDX-License-Identifier: GPL-3.0-or-later
//
//! iOS stand-in for `fork/tray.rs` (path-swapped in `fork/mod.rs`): no tray.

use super::i18n;

pub fn last_tooltip() -> Option<String> {
    None
}

/// Same label as on the Mac (About and the self-check read it).
pub fn version_label(locale: &str) -> String {
    let key = if cfg!(debug_assertions) {
        "rust.tray.versionDev"
    } else {
        "rust.tray.version"
    };
    i18n::t(
        i18n::current_locale(Some(locale)),
        key,
        &[("version", env!("CARGO_PKG_VERSION"))],
    )
}

pub(crate) fn set_parked_appearance(_appearance: Option<crate::settings::Theme>) {}
