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
            Shape::Diff => (DIFF_W, DIFF_MAX_H),
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
/// The transform diff card: header, the changes and the actions. Its
/// height follows the changes ([`diff_height`]), from one line of text up
/// to [`DIFF_MAX_H`] (longer changes scroll); [`Shape::size`] is the
/// largest.
pub const DIFF_W: f64 = 560.0;
/// Everything but the text lines: padding, header row, actions row and
/// the gaps between them (`.wb-pill-diff` in `pill.css`; a Playwright test
/// checks the page lays the card out to these numbers).
pub const DIFF_CHROME_H: f64 = 104.0;
/// One line of the changes (15 px text, 24 px lines).
pub const DIFF_LINE_H: f64 = 24.0;
/// At most this many lines show before the text scrolls.
pub const DIFF_MAX_LINES: f64 = 7.0;
pub const DIFF_MIN_H: f64 = DIFF_CHROME_H + DIFF_LINE_H;
pub const DIFF_MAX_H: f64 = DIFF_CHROME_H + DIFF_MAX_LINES * DIFF_LINE_H;
/// Width of the text column: the card less its side padding (18 + 14).
pub const DIFF_TEXT_W: f64 = DIFF_W - 32.0;

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

/// A shape as it is shown: the shape and its window height. Every shape
/// has one size except the diff card, whose height follows its text.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Fit {
    pub shape: Shape,
    /// Window height in whole points.
    h: u32,
}

impl Fit {
    /// `shape` at its one size (the diff card at its largest).
    pub const fn of(shape: Shape) -> Self {
        Self {
            shape,
            h: shape.size().1 as u32,
        }
    }

    /// The diff card `h` points tall (clamped to its limits).
    pub fn diff(h: u32) -> Self {
        Self {
            shape: Shape::Diff,
            h: h.clamp(DIFF_MIN_H as u32, DIFF_MAX_H as u32),
        }
    }

    /// The window, in points.
    pub fn size(self) -> (f64, f64) {
        (self.shape.size().0, f64::from(self.h))
    }

    /// The capsule inside the window (see [`Shape::capsule`]).
    pub fn capsule(self) -> (f64, f64) {
        if self.shape.labelled() {
            self.shape.capsule()
        } else {
            self.size()
        }
    }
}

/// How tall the diff card is for `ops`: an estimate of the lines the text
/// wraps to in the card's text column, so short changes get a short card.
/// It errs on the tall side (wide glyphs, chip padding, word-wrap waste);
/// past [`DIFF_MAX_H`] the text scrolls.
pub fn diff_height(ops: &[crate::fork::text::diff::DiffOp]) -> u32 {
    use crate::fork::text::diff::DiffOpKind;
    // Figtree at 15 px averages under 7.5 px a glyph for Latin text; CJK
    // and emoji take about two. A line keeps ~10 % for word wrap.
    const NARROW_PX: f64 = 7.5;
    let per_line = DIFF_TEXT_W * 0.9;
    let max_lines = DIFF_MAX_LINES as u32;
    let mut lines = 0u32;
    let mut width = 0.0f64;
    for op in ops {
        if op.kind != DiffOpKind::Equal {
            // Chip padding and the gap between neighbouring chips.
            width += 9.0;
        }
        for c in op.text.chars() {
            if c == '\n' {
                lines += (width / per_line).ceil().max(1.0) as u32;
                width = 0.0;
            } else {
                width += if is_wide(c) {
                    2.0 * NARROW_PX
                } else {
                    NARROW_PX
                };
            }
        }
        if lines > max_lines {
            break;
        }
    }
    lines += (width / per_line).ceil().max(1.0) as u32;
    let lines = lines.clamp(1, max_lines);
    Fit::diff((DIFF_CHROME_H + f64::from(lines) * DIFF_LINE_H) as u32).h
}

/// East Asian wide and full-width glyphs, and emoji.
fn is_wide(c: char) -> bool {
    matches!(u32::from(c),
        0x1100..=0x115F
        | 0x2E80..=0x303E
        | 0x3041..=0x33FF
        | 0x3400..=0x4DBF
        | 0x4E00..=0x9FFF
        | 0xA000..=0xA4CF
        | 0xAC00..=0xD7A3
        | 0xF900..=0xFAFF
        | 0xFE30..=0xFE4F
        | 0xFF00..=0xFF60
        | 0xFFE0..=0xFFE6
        | 0x1F300..=0x1FAFF
        | 0x20000..=0x3FFFD)
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
        "diff": {
            "chrome": DIFF_CHROME_H,
            "line": DIFF_LINE_H,
            "minH": DIFF_MIN_H,
            "maxH": DIFF_MAX_H,
        },
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

    fn ops(
        parts: &[(&str, crate::fork::text::diff::DiffOpKind)],
    ) -> Vec<crate::fork::text::diff::DiffOp> {
        parts
            .iter()
            .map(|(text, kind)| crate::fork::text::diff::DiffOp {
                kind: *kind,
                text: (*text).to_string(),
            })
            .collect()
    }

    /// Polish: the diff card is as tall as its text (one line at least,
    /// scrolling past the largest card).
    #[test]
    fn the_diff_card_fits_its_text() {
        use crate::fork::text::diff::DiffOpKind::{Delete, Equal, Insert};
        let one = diff_height(&ops(&[
            ("Hi ", Equal),
            ("perhaps", Delete),
            ("We could", Insert),
            (" meet.", Equal),
        ]));
        assert_eq!(f64::from(one), DIFF_MIN_H);
        let words = "word ".repeat(40); // ~200 glyphs: three or four lines
        let three = diff_height(&ops(&[(&words, Equal)]));
        assert!(
            f64::from(three) >= DIFF_CHROME_H + 3.0 * DIFF_LINE_H,
            "{three}"
        );
        assert!(
            f64::from(three) <= DIFF_CHROME_H + 4.0 * DIFF_LINE_H,
            "{three}"
        );
        // Paragraphs count as lines.
        let paras = diff_height(&ops(&[("a\nb\nc", Equal)]));
        assert_eq!(f64::from(paras), DIFF_CHROME_H + 3.0 * DIFF_LINE_H);
        // Wide glyphs take more room than the same count of Latin ones.
        let cjk = diff_height(&ops(&[(&"字".repeat(70), Equal)]));
        let latin = diff_height(&ops(&[(&"a".repeat(70), Equal)]));
        assert!(cjk > latin, "{cjk} vs {latin}");
        // Long text stops at the largest card; nothing is ever below one line.
        let long = diff_height(&ops(&[(&"word ".repeat(2000), Equal)]));
        assert_eq!(f64::from(long), DIFF_MAX_H);
        assert_eq!(f64::from(diff_height(&[])), DIFF_MIN_H);
        assert_eq!(Fit::diff(1).size(), (DIFF_W, DIFF_MIN_H));
        assert_eq!(Fit::diff(10_000).size(), (DIFF_W, DIFF_MAX_H));
        assert_eq!(Fit::of(Shape::HeldLabel).capsule(), Shape::Held.size());
        assert_eq!(Fit::of(Shape::Card).size(), Shape::Card.size());
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
