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
    /// Held, with the microphone name for its first 2 s.
    HeldLabel,
    /// Hands-free: ✕, the waveform, ✓.
    Locked,
    /// Hands-free with a text in the middle (microphone name, warning).
    LockedLabel,
    /// S-02: held, with the Secure Input notice (a longer text).
    HeldNotice,
    /// S-02: hands-free, with the Secure Input notice.
    LockedNotice,
    /// "Transcribing…" / "Cleaning up…".
    Status,
    /// A message or an error, with an optional Fix button.
    Message,
    /// "Setting up… 40%" while the first speech model downloads (S-07a).
    Setup,
    /// A finished take's text with Copy and ✕ (S-07a queued takes).
    Card,
    /// A transform's word diff and actions (S-22).
    Diff,
}

impl Shape {
    pub const ALL: [Shape; 12] = [
        Shape::Rest,
        Shape::Held,
        Shape::HeldLabel,
        Shape::Locked,
        Shape::LockedLabel,
        Shape::HeldNotice,
        Shape::LockedNotice,
        Shape::Status,
        Shape::Message,
        Shape::Setup,
        Shape::Card,
        Shape::Diff,
    ];

    /// Width and height of the window, in points.
    pub const fn size(self) -> (f64, f64) {
        match self {
            Shape::Rest => (REST_W, REST_H),
            Shape::Held => (HELD_W, ACTIVE_H),
            Shape::HeldLabel | Shape::LockedLabel => (LABEL_W, LABELLED_H),
            Shape::Locked => (LOCKED_W, ACTIVE_H),
            Shape::HeldNotice | Shape::LockedNotice => (NOTICE_W, LABELLED_H),
            Shape::Status => (184.0, ACTIVE_H),
            Shape::Message => (400.0, ACTIVE_H),
            Shape::Setup => (260.0, ACTIVE_H),
            Shape::Card => (440.0, CARD_H),
            Shape::Diff => (560.0, DIFF_H),
        }
    }

    /// The capsule inside the window. Equal to [`Shape::size`] except for
    /// the labelled shapes, whose label pill floats above the capsule (below
    /// it when the bar sits at the top of the display).
    pub const fn capsule(self) -> (f64, f64) {
        match self {
            Shape::HeldLabel | Shape::HeldNotice => Shape::Held.size(),
            Shape::LockedLabel | Shape::LockedNotice => Shape::Locked.size(),
            _ => self.size(),
        }
    }

    /// Whether a label pill floats beside the capsule.
    pub const fn labelled(self) -> bool {
        matches!(
            self,
            Shape::HeldLabel | Shape::LockedLabel | Shape::HeldNotice | Shape::LockedNotice
        )
    }

    /// The JSON / CSS name.
    pub fn name(self) -> &'static str {
        match self {
            Shape::Rest => "rest",
            Shape::Held => "held",
            Shape::HeldLabel => "held_label",
            Shape::Locked => "locked",
            Shape::LockedLabel => "locked_label",
            Shape::HeldNotice => "held_notice",
            Shape::LockedNotice => "locked_notice",
            Shape::Status => "status",
            Shape::Message => "message",
            Shape::Setup => "setup",
            Shape::Card => "card",
            Shape::Diff => "diff",
        }
    }
}

/// The resting bar: a thin capsule (Design W4: Wispr Flow's proportions).
pub const REST_W: f64 = 38.0;
pub const REST_H: f64 = 6.0;
/// The resting bar's window is larger than the capsule it draws, so the
/// 6 pt line is still easy to hover and click: this much on each side.
pub const REST_HIT_PAD_X: f64 = 8.0;
pub const REST_HIT_PAD_Y: f64 = 6.0;

/// Height of every active capsule but the card and the diff.
pub const ACTIVE_H: f64 = 30.0;
/// Push to talk (held): the waveform alone.
pub const HELD_W: f64 = 72.0;
/// Hands-free: ✕, the waveform, ✓.
pub const LOCKED_W: f64 = 100.0;
/// The label pill above the capsule (microphone name, warnings) and the
/// gap between the two.
pub const LABEL_H: f64 = 34.0;
pub const LABEL_GAP: f64 = 8.0;
/// Width of the window while a label shows (the pill is as wide as its
/// text, up to this).
pub const LABEL_W: f64 = 300.0;
/// The same for the longer Secure Input notice (S-02).
pub const NOTICE_W: f64 = 460.0;
/// Height of a labelled window: label, gap, capsule.
pub const LABELLED_H: f64 = LABEL_H + LABEL_GAP + ACTIVE_H;
/// The card: a label line and two lines of text beside Copy and ✕.
pub const CARD_H: f64 = 76.0;
/// The transform diff card: header, scrollable changes and the actions.
pub const DIFF_H: f64 = 280.0;

/// The hover bubble of the resting bar: height, horizontal padding around
/// its text, and its width limits (the text is measured, localized).
pub const HOVER_H: f64 = 34.0;
pub const HOVER_PAD_X: f64 = 16.0;
pub const HOVER_MIN_W: f64 = 120.0;
pub const HOVER_MAX_W: f64 = 420.0;
/// Font size of every text on the bar (points; the web view uses the same).
pub const HOVER_FONT: f64 = 15.0;

/// Gap between the bar and the edge it rests on (above the Dock or the
/// screen bottom; below the menu bar in the top position).
pub const EDGE_GAP: f64 = 8.0;

/// Number of waveform bars.
pub const BARS: usize = 11;

/// The spring every size change of the bar follows, native and web alike
/// (`SPRING.bar` in `src/fork/ui/motion.ts`; a test keeps the two equal).
pub const SPRING_STIFFNESS: f64 = 600.0;
pub const SPRING_DAMPING: f64 = 38.0;
pub const SPRING_MASS: f64 = 0.8;
/// After this the spring is treated as settled (it is within 0.1 % well
/// before).
pub const MORPH_MS: f64 = 400.0;

/// Progress (0 → 1) and velocity (per second) of a [`SPRING_STIFFNESS`] /
/// [`SPRING_DAMPING`] / [`SPRING_MASS`] spring released from rest at 0,
/// `t_ms` after the release: the closed form of the damped oscillator, so
/// the native bar and the web view (`src/fork/pill/spring.ts`) agree on
/// every frame without sharing a clock tick.
pub fn spring(t_ms: f64) -> (f64, f64) {
    if !t_ms.is_finite() || t_ms <= 0.0 {
        return (0.0, 0.0);
    }
    if t_ms >= MORPH_MS {
        return (1.0, 0.0);
    }
    let t = t_ms / 1000.0;
    let w0 = (SPRING_STIFFNESS / SPRING_MASS).sqrt();
    let zeta = SPRING_DAMPING / (2.0 * (SPRING_STIFFNESS * SPRING_MASS).sqrt());
    let a = zeta * w0;
    let wd = w0 * (1.0 - zeta * zeta).sqrt();
    let decay = (-a * t).exp();
    let x = 1.0 - decay * ((wd * t).cos() + (a / wd) * (wd * t).sin());
    let v = decay * (w0 * w0 / wd) * (wd * t).sin();
    (x, v)
}

/// A size `t_ms` into a morph from `from` to `to` (points).
pub fn morph(from: (f64, f64), to: (f64, f64), t_ms: f64) -> (f64, f64) {
    let (p, _) = spring(t_ms);
    (from.0 + (to.0 - from.0) * p, from.1 + (to.1 - from.1) * p)
}

/// The geometry the web view needs, as written to `geometry.gen.json`.
pub fn geometry_json() -> serde_json::Value {
    let shapes: serde_json::Map<String, serde_json::Value> = Shape::ALL
        .iter()
        .map(|s| {
            let (w, h) = s.size();
            (s.name().to_string(), serde_json::json!({ "w": w, "h": h }))
        })
        .collect();
    let capsules: serde_json::Map<String, serde_json::Value> = Shape::ALL
        .iter()
        .map(|s| {
            let (w, h) = s.capsule();
            (s.name().to_string(), serde_json::json!({ "w": w, "h": h }))
        })
        .collect();
    serde_json::json!({
        "_comment": "Generated from src-tauri/src/fork/pill/geometry.rs; do not edit.",
        "shapes": shapes,
        "capsules": capsules,
        "label": { "h": LABEL_H, "gap": LABEL_GAP },
        "bars": BARS,
        "edgeGap": EDGE_GAP,
        "spring": {
            "stiffness": SPRING_STIFFNESS,
            "damping": SPRING_DAMPING,
            "mass": SPRING_MASS,
            "settleMs": MORPH_MS,
        },
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
        // Every capsule fits its window, and only labelled shapes differ.
        for s in Shape::ALL {
            let ((w, h), (cw, ch)) = (s.size(), s.capsule());
            assert!(cw <= w && ch <= h, "{s:?}");
            assert_eq!(s.labelled(), (cw, ch) != (w, h), "{s:?}");
            if s.labelled() {
                assert_eq!(h, ch + LABEL_GAP + LABEL_H, "{s:?}");
            }
        }
    }

    /// Design W4: Wispr Flow's measured sizes (REFERENCE-SPEC §7).
    #[test]
    fn the_bar_has_the_reference_proportions() {
        assert_eq!(Shape::Rest.size(), (38.0, 6.0));
        assert_eq!(Shape::Held.size(), (72.0, 30.0));
        assert_eq!(Shape::Locked.size(), (100.0, 30.0));
        assert_eq!(LABEL_H, 34.0);
        assert_eq!(HOVER_FONT, 15.0);
    }

    #[test]
    fn the_spring_settles_quickly_without_visible_overshoot() {
        assert_eq!(spring(0.0), (0.0, 0.0));
        assert_eq!(spring(-5.0), (0.0, 0.0));
        assert_eq!(spring(f64::NAN), (0.0, 0.0));
        assert_eq!(spring(MORPH_MS), (1.0, 0.0));
        let mut peak = 0.0f64;
        let mut prev = 0.0;
        let mut settled_at = None;
        for ms in 1..(MORPH_MS as u32) {
            let (x, v) = spring(ms as f64);
            peak = peak.max(x);
            // Velocity is the derivative of the progress.
            let numeric = (x - prev) * 1000.0;
            assert!(
                (numeric - v).abs() < 0.05 * v.abs().max(1.0) + 0.5,
                "{ms}: {numeric} vs {v}"
            );
            prev = x;
            if settled_at.is_none() && (x - 1.0).abs() < 0.02 {
                settled_at = Some(ms);
            }
        }
        // Within 2 % in under 200 ms; at most 1 % past the target; within
        // 0.1 % at the cut-off, so snapping to 1 there is invisible.
        assert!(settled_at.unwrap() < 200, "{settled_at:?}");
        assert!(peak < 1.01, "{peak}");
        assert!((spring(MORPH_MS - 1.0).0 - 1.0).abs() < 0.001);
        assert_eq!(morph((38.0, 6.0), (72.0, 30.0), 0.0), (38.0, 6.0));
        assert_eq!(morph((38.0, 6.0), (72.0, 30.0), MORPH_MS), (72.0, 30.0));
    }

    /// The web view animates with `motion`'s `SPRING.bar`; it must be the
    /// spring the native bar computes here.
    #[test]
    fn the_spring_matches_the_web_motion_preset() {
        let motion = include_str!("../../../../src/fork/ui/motion.ts");
        let line = motion
            .lines()
            .find(|l| l.trim_start().starts_with("bar:"))
            .expect("SPRING.bar in motion.ts");
        for (key, value) in [
            ("stiffness", SPRING_STIFFNESS),
            ("damping", SPRING_DAMPING),
            ("mass", SPRING_MASS),
        ] {
            assert!(line.contains(&format!("{key}: {value}")), "{key} in {line}");
        }
    }
}
