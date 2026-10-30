// SPDX-License-Identifier: GPL-3.0-only
// Copyright (c) the VoiceInk authors (VoiceInk, GPL-3.0)
// Modified by Wispr Bro contributors, 2026-10-30.
//
// Ported from VoiceInk
// `VoiceInk/Infrastructure/SystemIntegration/Accessibility/AccessibilityTextReader.swift`
// (`markerSnapshot`, `stringForMarkerRange`; https://github.com/Beingpax/VoiceInk,
// commit 19546d2a7f85578f49efb538eb888b37ff2f8f8c), GPL-3.0. Kept: the
// support check on the parameterized attribute names, reading
// `AXSelectedTextMarkerRange` and turning it into text with
// `AXStringForTextMarkerRange`, accepting a string or an attributed string.
// Changed: Swift → Rust on objc2; only the selected text is read (RS-04
// step 2 of the selection chain), not the text before and after; the
// result is capped like the rest of the AX layer.

//! Selected text through WebKit/Chromium text markers: the second step of
//! the selection chain (R5 §4.6: AX → text markers → Edit › Copy → ⌘C).

use objc2_application_services::AXUIElement;
use objc2_core_foundation::{CFAttributedString, CFString};

use super::element::{attr, copy_names, parameterized};

/// Longest selection we accept (UTF-16 units), as in VoiceInk.
pub const MAX_UNITS: usize = 100_000;

/// Whether the element answers the marker calls at all.
pub fn supported(element: &AXUIElement) -> bool {
    copy_names(element, true)
        .iter()
        .any(|n| n == "AXStringForTextMarkerRange")
}

/// The selected text through markers, or None.
pub fn selected_text(element: &AXUIElement) -> Option<String> {
    let range = attr(element, "AXSelectedTextMarkerRange").ok()?;
    let value = parameterized(element, "AXStringForTextMarkerRange", &range).ok()?;
    let text = if let Some(s) = value.downcast_ref::<CFString>() {
        s.to_string()
    } else {
        value
            .downcast_ref::<CFAttributedString>()?
            .string()?
            .to_string()
    };
    (text.encode_utf16().count() <= MAX_UNITS).then_some(text)
}
