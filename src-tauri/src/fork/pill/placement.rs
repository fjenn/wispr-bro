// SPDX-License-Identifier: GPL-3.0-or-later
//! Where the bar sits (R5 §4.9), as pure functions over screen rectangles.
//!
//! Coordinates are AppKit's global space: points, y grows upwards, the
//! origin is the bottom-left corner of the primary display, other displays
//! may have negative coordinates. `frame` is a display's full rectangle and
//! `visible` its `visibleFrame` (without the menu bar and the Dock; on a
//! notched display the menu bar is taller, and `visible` already excludes
//! it).
//!
//! Rules:
//! - The display is the one under the cursor (the first display when the
//!   cursor is on none, e.g. right after an unplug).
//! - Bottom (default): the bar's bottom edge sits [`EDGE_GAP`] above the
//!   visible frame, so it clears the Dock when the Dock is at the bottom,
//!   and sits near the screen edge when the Dock is hidden or on a side. In
//!   a full-screen Space it sits [`EDGE_GAP`] above the screen edge.
//! - Top: [`EDGE_GAP`] below the menu bar (the top of the visible frame).
//! - Horizontally: the centre of the display, or the position the user
//!   dragged the bar to on that display (a fraction of its visible width),
//!   always kept inside the visible frame (a side Dock never covers it).
//! - Positions are rounded to the display's pixel grid.

use super::geometry::{Shape, EDGE_GAP, REST_HIT_PAD_X, REST_HIT_PAD_Y};

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Rect {
    pub x: f64,
    pub y: f64,
    pub w: f64,
    pub h: f64,
}

impl Rect {
    pub const fn new(x: f64, y: f64, w: f64, h: f64) -> Rect {
        Rect { x, y, w, h }
    }

    pub fn max_x(&self) -> f64 {
        self.x + self.w
    }

    pub fn max_y(&self) -> f64 {
        self.y + self.h
    }

    pub fn mid_x(&self) -> f64 {
        self.x + self.w / 2.0
    }

    /// Half-open containment, like `NSMouseInRect` for a non-flipped view.
    pub fn contains(&self, x: f64, y: f64) -> bool {
        x >= self.x && x < self.max_x() && y >= self.y && y < self.max_y()
    }
}

/// One display as AppKit reports it.
#[derive(Clone, Debug, PartialEq)]
pub struct Screen {
    /// Stable key for remembered positions ([`display_key`]).
    pub key: String,
    pub frame: Rect,
    pub visible: Rect,
    /// Backing scale (2.0 on Retina).
    pub scale: f64,
}

/// Which edge the bar rests on (`ForkSettings::bar_position`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Edge {
    Bottom,
    Top,
}

/// The index of the display under `cursor`, else the first one.
pub fn screen_under(screens: &[Screen], cursor: (f64, f64)) -> Option<usize> {
    if screens.is_empty() {
        return None;
    }
    Some(
        screens
            .iter()
            .position(|s| s.frame.contains(cursor.0, cursor.1))
            .unwrap_or(0),
    )
}

/// Where the bar's anchor is on `screen`: the x of its centre and the y of
/// the edge it rests on (its bottom edge for [`Edge::Bottom`], its top edge
/// for [`Edge::Top`]). Every shape is laid out around the same anchor, so
/// the bar grows from where it rests.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Anchor {
    pub centre_x: f64,
    pub edge_y: f64,
    pub edge: Edge,
    pub scale: f64,
    /// The visible frame the bar is kept inside.
    pub bounds: Rect,
}

/// The anchor on `screen`. `x_frac` is a remembered drag position (0 = the
/// left end of the visible frame, 1 = the right end). `full_screen`: the
/// frontmost app fills the display (no Dock, no menu bar shown).
pub fn anchor(screen: &Screen, edge: Edge, x_frac: Option<f64>, full_screen: bool) -> Anchor {
    let bounds = if full_screen {
        screen.frame
    } else {
        screen.visible
    };
    let centre_x = match x_frac {
        Some(f) if f.is_finite() => bounds.x + f.clamp(0.0, 1.0) * bounds.w,
        _ => screen.frame.mid_x(),
    };
    let edge_y = match edge {
        Edge::Bottom => bounds.y + EDGE_GAP,
        Edge::Top => bounds.max_y() - EDGE_GAP,
    };
    Anchor {
        centre_x,
        edge_y,
        edge,
        scale: if screen.scale > 0.0 {
            screen.scale
        } else {
            1.0
        },
        bounds,
    }
}

fn snap(v: f64, scale: f64) -> f64 {
    (v * scale).round() / scale
}

/// The window frame of a `(w, h)` shape at `anchor`, kept inside the
/// anchor's bounds and snapped to the pixel grid.
pub fn frame(anchor: &Anchor, size: (f64, f64)) -> Rect {
    let (w, h) = size;
    let b = anchor.bounds;
    let mut x = anchor.centre_x - w / 2.0;
    // Inside the visible frame horizontally (when it fits at all).
    if w <= b.w {
        x = x.clamp(b.x, b.max_x() - w);
    }
    let y = match anchor.edge {
        Edge::Bottom => anchor.edge_y,
        Edge::Top => anchor.edge_y - h,
    };
    Rect::new(snap(x, anchor.scale), snap(y, anchor.scale), w, h)
}

/// The native bar's window and the capsule it draws (in window
/// coordinates, y up) for a capsule of `capsule` size at `anchor`.
///
/// The capsule sits where the web view's capsule sits for the same size
/// (its edge on the anchor, centred, kept on screen), so the two hand over
/// without a jump. The window is the capsule plus the resting bar's hit
/// area ([`REST_HIT_PAD_X`] / [`REST_HIT_PAD_Y`] around the 38 × 6 line),
/// so the thin resting bar stays easy to hover and click, and is never
/// smaller than that area while a morph passes through it.
pub fn native_frame(anchor: &Anchor, capsule: (f64, f64)) -> (Rect, Rect) {
    let c = frame(anchor, capsule);
    let rest = frame(anchor, Shape::Rest.size());
    let hit = Rect::new(
        rest.x - REST_HIT_PAD_X,
        rest.y - REST_HIT_PAD_Y,
        rest.w + 2.0 * REST_HIT_PAD_X,
        rest.h + 2.0 * REST_HIT_PAD_Y,
    );
    let x = c.x.min(hit.x);
    let y = c.y.min(hit.y);
    let w = c.max_x().max(hit.max_x()) - x;
    let h = c.max_y().max(hit.max_y()) - y;
    let window = Rect::new(x, y, w, h);
    (window, Rect::new(c.x - x, c.y - y, c.w, c.h))
}

/// The drag position to remember for a bar whose centre is at `centre_x`.
pub fn x_frac(screen: &Screen, centre_x: f64) -> f64 {
    let b = screen.visible;
    if b.w <= 0.0 {
        return 0.5;
    }
    ((centre_x - b.x) / b.w).clamp(0.0, 1.0)
}

/// A drag that ends within this distance of the display's centre snaps back
/// to it (and forgets the remembered position).
pub const CENTRE_SNAP: f64 = 12.0;

/// The centre x a drag lands on: clamped to where the bar still fits, and
/// snapped to the display's centre when close to it.
pub fn dragged_centre(screen: &Screen, centre_x: f64, width: f64) -> f64 {
    let b = screen.visible;
    let half = width / 2.0;
    let x = if b.w >= width {
        centre_x.clamp(b.x + half, b.max_x() - half)
    } else {
        b.mid_x()
    };
    if (x - screen.frame.mid_x()).abs() <= CENTRE_SNAP {
        screen.frame.mid_x()
    } else {
        x
    }
}

/// Key under which a display's drag position is remembered: its name and
/// size, as a fork settings id (`[a-z0-9_.-]`, ≤ 64 chars). Display ids
/// change across reboots and replugs; name + size does not.
pub fn display_key(name: &str, frame: &Rect) -> String {
    let mut slug = String::new();
    let mut dash = false;
    for c in name.chars() {
        if c.is_ascii_alphanumeric() {
            slug.push(c.to_ascii_lowercase());
            dash = false;
        } else if !dash && !slug.is_empty() {
            slug.push('-');
            dash = true;
        }
    }
    let size = format!("{}x{}", frame.w.round() as i64, frame.h.round() as i64);
    let slug = slug.trim_end_matches('-');
    let max = 64 - size.len() - 1;
    let slug: String = slug.chars().take(max).collect();
    let slug = slug.trim_end_matches('-');
    if slug.is_empty() {
        format!("display-{size}")
    } else {
        format!("{slug}-{size}")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// MacBook Pro 14" (1512 × 982 pt at 2x), notch: 38 pt menu bar.
    fn builtin(dock: Dock) -> Screen {
        let frame = Rect::new(0.0, 0.0, 1512.0, 982.0);
        let menu = 38.0;
        let visible = match dock {
            Dock::Bottom(h) => Rect::new(0.0, h, 1512.0, 982.0 - menu - h),
            Dock::Left(w) => Rect::new(w, 0.0, 1512.0 - w, 982.0 - menu),
            Dock::Right(w) => Rect::new(0.0, 0.0, 1512.0 - w, 982.0 - menu),
            // An auto-hidden Dock keeps a 4 pt strip.
            Dock::Hidden => Rect::new(0.0, 4.0, 1512.0, 982.0 - menu - 4.0),
        };
        Screen {
            key: display_key("Built-in Retina Display", &frame),
            frame,
            visible,
            scale: 2.0,
        }
    }

    /// A 1x 1920 × 1080 display to the right of the built-in one, its top
    /// aligned with the built-in display's top.
    fn external() -> Screen {
        let frame = Rect::new(1512.0, -98.0, 1920.0, 1080.0);
        Screen {
            key: display_key("DELL U2419H", &frame),
            frame,
            visible: Rect::new(1512.0, -98.0, 1920.0, 1080.0 - 25.0),
            scale: 1.0,
        }
    }

    enum Dock {
        Bottom(f64),
        Left(f64),
        Right(f64),
        Hidden,
    }

    fn rest(anchor: &Anchor) -> Rect {
        frame(anchor, Shape::Rest.size())
    }

    #[test]
    fn one_display_bottom_centre_above_the_dock() {
        let s = builtin(Dock::Bottom(70.0));
        let a = anchor(&s, Edge::Bottom, None, false);
        let r = rest(&a);
        assert_eq!(r, Rect::new(737.0, 70.0 + EDGE_GAP, 38.0, 6.0));
        // Every shape shares the bottom edge and the centre.
        for shape in Shape::ALL {
            let f = frame(&a, shape.size());
            assert_eq!(f.y, r.y, "{shape:?}");
            assert_eq!(f.mid_x(), 756.0, "{shape:?}");
        }
    }

    #[test]
    fn hidden_dock_sits_near_the_edge() {
        let a = anchor(&builtin(Dock::Hidden), Edge::Bottom, None, false);
        assert_eq!(rest(&a).y, 4.0 + EDGE_GAP);
    }

    #[test]
    fn side_docks_never_cover_the_bar() {
        for (dock, left) in [(Dock::Left(80.0), true), (Dock::Right(80.0), false)] {
            let s = builtin(dock);
            let a = anchor(&s, Edge::Bottom, None, false);
            // Centred on the display, near the screen edge.
            let r = frame(&a, Shape::Message.size());
            assert_eq!(r.y, EDGE_GAP);
            assert!(r.x >= s.visible.x && r.max_x() <= s.visible.max_x());
            // Dragged to the Dock side, it stops at the Dock.
            let a = anchor(&s, Edge::Bottom, Some(if left { 0.0 } else { 1.0 }), false);
            let r = frame(&a, Shape::Message.size());
            if left {
                assert_eq!(r.x, 80.0);
            } else {
                assert_eq!(r.max_x(), 1512.0 - 80.0);
            }
        }
    }

    #[test]
    fn top_position_sits_below_the_menu_bar_and_notch() {
        let s = builtin(Dock::Bottom(70.0));
        let a = anchor(&s, Edge::Top, None, false);
        let r = rest(&a);
        assert_eq!(r.max_y(), 982.0 - 38.0 - EDGE_GAP);
        // Active shapes hang down from the same top edge.
        let m = frame(&a, Shape::Message.size());
        assert_eq!(m.max_y(), r.max_y());
    }

    #[test]
    fn full_screen_space_is_flush_with_the_screen_edge() {
        let s = builtin(Dock::Bottom(70.0));
        let a = anchor(&s, Edge::Bottom, None, true);
        assert_eq!(rest(&a).y, EDGE_GAP);
        let a = anchor(&s, Edge::Top, None, true);
        assert_eq!(rest(&a).max_y(), 982.0 - EDGE_GAP);
    }

    #[test]
    fn two_displays_follow_the_cursor() {
        let screens = vec![builtin(Dock::Bottom(70.0)), external()];
        assert_eq!(screen_under(&screens, (100.0, 100.0)), Some(0));
        assert_eq!(screen_under(&screens, (2000.0, 500.0)), Some(1));
        // Below the built-in display's bottom but inside the external one.
        assert_eq!(screen_under(&screens, (2000.0, -50.0)), Some(1));
        // On no display (between arrangements): the first one.
        assert_eq!(screen_under(&screens, (-500.0, -500.0)), Some(0));
        assert_eq!(screen_under(&[], (0.0, 0.0)), None);
    }

    #[test]
    fn mixed_scale_snaps_to_each_displays_pixels() {
        let ext = external();
        let a = anchor(&ext, Edge::Bottom, None, false);
        let r = rest(&a);
        assert_eq!(r, Rect::new(1512.0 + 941.0, -98.0 + EDGE_GAP, 38.0, 6.0));
        // A position that falls between pixels: whole points at 1x, half
        // points at 2x.
        let odd = Shape::Held.size();
        let a1 = Anchor {
            centre_x: 2000.3,
            ..a
        };
        assert_eq!(frame(&a1, odd).x, 1964.0);
        let b = anchor(&builtin(Dock::Bottom(70.0)), Edge::Bottom, None, false);
        let b1 = Anchor {
            centre_x: 700.3,
            ..b
        };
        assert_eq!(frame(&b1, odd).x, 664.5);
    }

    #[test]
    fn unplugged_display_falls_back_to_the_first() {
        // The bar was on the external display; it is gone and the cursor
        // position AppKit reports is now outside every display.
        let screens = vec![builtin(Dock::Bottom(70.0))];
        let i = screen_under(&screens, (2400.0, 300.0)).unwrap();
        let a = anchor(&screens[i], Edge::Bottom, None, false);
        let r = rest(&a);
        assert!(screens[0].visible.contains(r.x, r.y));
    }

    #[test]
    fn drag_positions_are_fractions_kept_inside() {
        let s = builtin(Dock::Bottom(70.0));
        assert_eq!(x_frac(&s, 0.0), 0.0);
        assert_eq!(x_frac(&s, 1512.0), 1.0);
        assert!((x_frac(&s, 378.0) - 0.25).abs() < 1e-9);
        let a = anchor(&s, Edge::Bottom, Some(0.25), false);
        assert_eq!(a.centre_x, 378.0);
        // Out-of-range or broken values are clamped / ignored.
        assert_eq!(anchor(&s, Edge::Bottom, Some(7.0), false).centre_x, 1512.0);
        assert_eq!(
            anchor(&s, Edge::Bottom, Some(f64::NAN), false).centre_x,
            756.0
        );
        // The same fraction on another display lands proportionally.
        let e = external();
        let a = anchor(&e, Edge::Bottom, Some(0.25), false);
        assert_eq!(a.centre_x, 1512.0 + 480.0);
    }

    #[test]
    fn drags_clamp_and_snap_to_the_centre() {
        let s = builtin(Dock::Bottom(70.0));
        assert_eq!(dragged_centre(&s, -100.0, 60.0), 30.0);
        assert_eq!(dragged_centre(&s, 5000.0, 60.0), 1512.0 - 30.0);
        assert_eq!(dragged_centre(&s, 756.0 + 11.0, 60.0), 756.0);
        assert_eq!(dragged_centre(&s, 756.0 + 30.0, 60.0), 786.0);
    }

    #[test]
    fn wide_shapes_near_a_remembered_edge_stay_on_screen() {
        let s = builtin(Dock::Bottom(70.0));
        let a = anchor(&s, Edge::Bottom, Some(0.0), false);
        let r = frame(&a, Shape::Message.size());
        assert_eq!(r.x, 0.0);
        let a = anchor(&s, Edge::Bottom, Some(1.0), false);
        let r = frame(&a, Shape::Message.size());
        assert_eq!(r.max_x(), 1512.0);
    }

    #[test]
    fn the_native_bar_draws_where_the_web_capsule_sits() {
        for edge in [Edge::Bottom, Edge::Top] {
            let a = anchor(&builtin(Dock::Bottom(70.0)), edge, None, false);
            // At rest: a hit area around the 38 × 6 line, the line centred.
            let (win, cap) = native_frame(&a, Shape::Rest.size());
            assert_eq!((win.w, win.h), (38.0 + 16.0, 6.0 + 12.0), "{edge:?}");
            assert_eq!((cap.x, cap.y, cap.w, cap.h), (8.0, 6.0, 38.0, 6.0));
            let line = rest(&a);
            assert_eq!((win.x + cap.x, win.y + cap.y), (line.x, line.y));
            // Active: the capsule is exactly the web view's capsule.
            for shape in Shape::ALL {
                let (win, cap) = native_frame(&a, shape.capsule());
                let web = frame(&a, shape.capsule());
                assert_eq!(
                    (win.x + cap.x, win.y + cap.y, cap.w, cap.h),
                    (web.x, web.y, web.w, web.h),
                    "{shape:?} {edge:?}"
                );
                // The window covers the capsule and the hit area.
                assert!(win.w >= cap.w && win.h >= cap.h);
                assert!(win.h <= cap.h.max(6.0) + 6.0 + 6.0, "{shape:?}");
            }
        }
    }

    #[test]
    fn display_keys_are_valid_setting_ids() {
        let f = Rect::new(0.0, 0.0, 1512.0, 982.0);
        assert_eq!(
            display_key("Built-in Retina Display", &f),
            "built-in-retina-display-1512x982"
        );
        assert_eq!(display_key("  ", &f), "display-1512x982");
        assert_eq!(display_key("LG HDR 4K (2)", &f), "lg-hdr-4k-2-1512x982");
        let long = display_key(&"Very Long Display Name ".repeat(10), &f);
        assert!(long.len() <= 64, "{long}");
        for key in [long, display_key("Écran ü", &f)] {
            assert!(key
                .chars()
                .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || "_.-".contains(c)));
            assert!(!key.starts_with('-') && !key.contains("--"));
        }
    }
}
