// SPDX-License-Identifier: GPL-3.0-or-later
//! Pill sizes, defined once (R5 §4.9, fixes RE-13).
//!
//! Every window of the Flow Bar is sized exactly to the shape it shows: no
//! transparent margin, so nothing ever needs click-through toggling. The
//! native resting pill reads these constants directly; the active web view
//! reads them from `src/fork/pill/geometry.gen.json`, which a test keeps in
//! sync with [`geometry_json`] (`WB_WRITE_PILL_GEOMETRY=1 cargo test --lib
//! pill_geometry_json_is_current` rewrites it).

use serde::{Deserialize, Serialize};
use specta::Type;

/// One shape of the bar, in points.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize, Type)]
#[serde(rename_all = "snake_case")]
pub enum Shape {
    /// The thin resting bar (native).
    Rest,
    /// Held (push to talk): the waveform.
    Held,
    /// Held, with the microphone name for its first 1.5 s.
    HeldLabel,
    /// Hands-free: ✕, the waveform, ✓.
    Locked,
    /// Hands-free with a text in the middle (microphone name, warning).
    LockedLabel,
    /// "Transcribing…" / "Cleaning up…".
    Status,
    /// A message or an error, with an optional Fix button.
    Message,
    /// "Setting up… 40%" while the first speech model downloads (S-07a).
    Setup,
    /// A finished take's text with Copy and ✕ (S-07a queued takes).
    Card,
}

impl Shape {
    pub const ALL: [Shape; 9] = [
        Shape::Rest,
        Shape::Held,
        Shape::HeldLabel,
        Shape::Locked,
        Shape::LockedLabel,
        Shape::Status,
        Shape::Message,
        Shape::Setup,
        Shape::Card,
    ];

    /// Width and height in points.
    pub const fn size(self) -> (f64, f64) {
        match self {
            Shape::Rest => (REST_W, REST_H),
            Shape::Held => (112.0, ACTIVE_H),
            Shape::HeldLabel => (272.0, ACTIVE_H),
            Shape::Locked => (168.0, ACTIVE_H),
            Shape::LockedLabel => (328.0, ACTIVE_H),
            Shape::Status => (208.0, ACTIVE_H),
            Shape::Message => (360.0, ACTIVE_H),
            Shape::Setup => (248.0, ACTIVE_H),
            Shape::Card => (400.0, CARD_H),
        }
    }

    /// The JSON / CSS name.
    pub fn name(self) -> &'static str {
        match self {
            Shape::Rest => "rest",
            Shape::Held => "held",
            Shape::HeldLabel => "held_label",
            Shape::Locked => "locked",
            Shape::LockedLabel => "locked_label",
            Shape::Status => "status",
            Shape::Message => "message",
            Shape::Setup => "setup",
            Shape::Card => "card",
        }
    }
}

/// The resting bar: a thin capsule.
pub const REST_W: f64 = 60.0;
pub const REST_H: f64 = 12.0;

/// Height of every active shape but the card.
pub const ACTIVE_H: f64 = 34.0;
/// The card: two lines of text beside Copy and ✕.
pub const CARD_H: f64 = 58.0;

/// The hover bubble of the resting bar: height, horizontal padding around
/// its text, and its width limits (the text is measured, localized).
pub const HOVER_H: f64 = 30.0;
pub const HOVER_PAD_X: f64 = 16.0;
pub const HOVER_MIN_W: f64 = 160.0;
pub const HOVER_MAX_W: f64 = 420.0;
/// Font size of the hover text (points).
pub const HOVER_FONT: f64 = 12.5;

/// Gap between the bar and the edge it rests on (above the Dock or the
/// screen bottom; below the menu bar in the top position).
pub const EDGE_GAP: f64 = 8.0;

/// Number of waveform bars.
pub const BARS: usize = 9;

/// The geometry the web view needs, as written to `geometry.gen.json`.
pub fn geometry_json() -> serde_json::Value {
    let shapes: serde_json::Map<String, serde_json::Value> = Shape::ALL
        .iter()
        .map(|s| {
            let (w, h) = s.size();
            (s.name().to_string(), serde_json::json!({ "w": w, "h": h }))
        })
        .collect();
    serde_json::json!({
        "_comment": "Generated from src-tauri/src/fork/pill/geometry.rs; do not edit.",
        "shapes": shapes,
        "bars": BARS,
        "edgeGap": EDGE_GAP,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    const GEN_PATH: &str = "../src/fork/pill/geometry.gen.json";

    #[test]
    fn pill_geometry_json_is_current() {
        let expected = format!(
            "{}\n",
            serde_json::to_string_pretty(&geometry_json()).unwrap()
        );
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join(GEN_PATH);
        if std::env::var_os("WB_WRITE_PILL_GEOMETRY").is_some() {
            std::fs::write(&path, &expected).unwrap();
        }
        let actual = std::fs::read_to_string(&path).unwrap_or_default();
        assert_eq!(
            actual, expected,
            "src/fork/pill/geometry.gen.json is stale: run \
             WB_WRITE_PILL_GEOMETRY=1 cargo test --lib pill_geometry_json_is_current"
        );
    }

    #[test]
    fn shapes_are_distinct_and_sane() {
        for s in Shape::ALL {
            let (w, h) = s.size();
            assert!(w >= h && h > 0.0, "{s:?}");
            // Whole points, so the window sits on pixel boundaries at 1x.
            assert_eq!(w.fract(), 0.0);
            assert_eq!(h.fract(), 0.0);
        }
        let names: std::collections::HashSet<_> = Shape::ALL.iter().map(|s| s.name()).collect();
        assert_eq!(names.len(), Shape::ALL.len());
        // Labels are wider than the shape they label.
        assert!(Shape::HeldLabel.size().0 > Shape::Held.size().0);
        assert!(Shape::LockedLabel.size().0 > Shape::Locked.size().0);
        const { assert!(HOVER_MIN_W < HOVER_MAX_W) };
    }
}
