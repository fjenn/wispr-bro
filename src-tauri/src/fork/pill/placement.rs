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

use super::geometry::{
    Shape, EDGE_GAP, HOVER_H, LABEL_GAP, LABEL_H, PAD_GAP, REST_HIT_PAD_X, REST_HIT_PAD_Y,
};

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
    native_frame_beside(anchor, capsule, None)
}

/// [`native_frame`] with the Scratchpad circle on `pad`'s side (S-24): the
/// hit area stops half way across the gap on that side, where the
/// circle's own window ([`pad_layout`]) starts, so the two never overlap.
pub fn native_frame_beside(
    anchor: &Anchor,
    capsule: (f64, f64),
    pad: Option<Side>,
) -> (Rect, Rect) {
    let c = frame(anchor, capsule);
    let rest = frame(anchor, Shape::Rest.size());
    let pad_x = |side: Side| {
        if pad == Some(side) {
            PAD_GAP / 2.0
        } else {
            REST_HIT_PAD_X
        }
    };
    let (left, right) = (pad_x(Side::Left), pad_x(Side::Right));
    let hit = Rect::new(
        rest.x - left,
        rest.y - REST_HIT_PAD_Y,
        rest.w + left + right,
        rest.h + 2.0 * REST_HIT_PAD_Y,
    );
    let x = c.x.min(hit.x);
    let y = c.y.min(hit.y);
    let w = c.max_x().max(hit.max_x()) - x;
    let h = c.max_y().max(hit.max_y()) - y;
    let window = Rect::new(x, y, w, h);
    (window, Rect::new(c.x - x, c.y - y, c.w, c.h))
}

/// A side of the bar.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Side {
    Left,
    Right,
}

/// Which side of the bar's capsule `bar` the Scratchpad circle takes
/// (S-24): the trailing one (right; left in a right-to-left language),
/// unless the largest circle would leave the bounds there and fits on the
/// other side (a bar dragged to a display edge).
pub fn pad_side(anchor: &Anchor, bar: &Rect, rtl: bool) -> Side {
    let need = PAD_GAP + HOVER_H + REST_HIT_PAD_X;
    let b = anchor.bounds;
    let fits = |side: Side| match side {
        Side::Right => bar.max_x() + need <= b.max_x(),
        Side::Left => bar.x - need >= b.x,
    };
    let (preferred, other) = if rtl {
        (Side::Left, Side::Right)
    } else {
        (Side::Right, Side::Left)
    };
    if !fits(preferred) && fits(other) {
        other
    } else {
        preferred
    }
}

/// The Scratchpad circle's window (S-24), in global coordinates, with the
/// circle and the hover tooltip inside it (window coordinates, y up).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PadLayout {
    pub window: Rect,
    pub circle: Rect,
    pub tip: Option<Rect>,
    pub side: Side,
}

/// Lays out the Scratchpad circle of diameter `d` beside the bar's capsule
/// `bar` (global): [`PAD_GAP`] away on [`pad_side`], on the bar's anchored
/// edge (so it grows from the edge like the bar). The window is the circle
/// plus the resting bar's hit padding, starting half way across the gap
/// (where [`native_frame_beside`] stops). A tooltip `tip_w` wide floats
/// [`LABEL_GAP`] above the circle (below it at the top of the display), the
/// label pill's place and size; it never reaches over the bar, so the
/// circle's window never overlaps the bar's whatever its state.
pub fn pad_layout(anchor: &Anchor, bar: &Rect, d: f64, tip_w: Option<f64>, rtl: bool) -> PadLayout {
    let side = pad_side(anchor, bar, rtl);
    let b = anchor.bounds;
    let x = match side {
        Side::Right => bar.max_x() + PAD_GAP,
        Side::Left => bar.x - PAD_GAP - d,
    };
    let y = match anchor.edge {
        Edge::Bottom => bar.y,
        Edge::Top => bar.max_y() - d,
    };
    let circle = Rect::new(x, y, d, d);
    let (hit_x0, hit_x1) = match side {
        Side::Right => (bar.max_x() + PAD_GAP / 2.0, circle.max_x() + REST_HIT_PAD_X),
        Side::Left => (circle.x - REST_HIT_PAD_X, bar.x - PAD_GAP / 2.0),
    };
    let hit = Rect::new(
        hit_x0,
        circle.y - REST_HIT_PAD_Y,
        hit_x1 - hit_x0,
        d + 2.0 * REST_HIT_PAD_Y,
    );
    let tip = tip_w.map(|w| {
        let mut tx = circle.mid_x() - w / 2.0;
        match side {
            Side::Right => {
                tx = tx.min(b.max_x() - w).max(hit_x0);
            }
            Side::Left => {
                tx = tx.max(b.x).min(hit_x1 - w);
            }
        }
        let ty = match anchor.edge {
            Edge::Bottom => circle.max_y() + LABEL_GAP,
            Edge::Top => circle.y - LABEL_GAP - LABEL_H,
        };
        Rect::new(snap(tx, anchor.scale), ty, w, LABEL_H)
    });
    let mut window = hit;
    if let Some(t) = tip {
        let x0 = window.x.min(t.x);
        let y0 = window.y.min(t.y);
        let x1 = window.max_x().max(t.max_x());
        let y1 = window.max_y().max(t.max_y());
        window = Rect::new(x0, y0, x1 - x0, y1 - y0);
    }
    let local = |r: Rect| Rect::new(r.x - window.x, r.y - window.y, r.w, r.h);
    PadLayout {
        window,
        circle: local(circle),
        tip: tip.map(local),
        side,
    }
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
    use crate::fork::pill::geometry::REST_H;

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

    fn overlaps(a: &Rect, b: &Rect) -> bool {
        a.x < b.max_x() && b.x < a.max_x() && a.y < b.max_y() && b.y < a.max_y()
    }

    fn global(win: &Rect, r: &Rect) -> Rect {
        Rect::new(win.x + r.x, win.y + r.y, r.w, r.h)
    }

    /// S-24: the Scratchpad circle sits 6 pt beside the bar on its anchored
    /// edge, as tall as the bar (the 6 pt line at rest), and its window and
    /// the bar's abut half way across the gap.
    #[test]
    fn the_scratchpad_circle_sits_beside_the_resting_bar() {
        let a = anchor(&builtin(Dock::Bottom(70.0)), Edge::Bottom, None, false);
        let bar = rest(&a);
        let pad = pad_layout(&a, &bar, REST_H, None, false);
        assert_eq!(pad.side, Side::Right);
        let circle = global(&pad.window, &pad.circle);
        assert_eq!(circle, Rect::new(bar.max_x() + PAD_GAP, bar.y, 6.0, 6.0));
        assert_eq!(pad.tip, None);
        let (win, cap) = native_frame_beside(&a, Shape::Rest.size(), Some(pad.side));
        assert_eq!(global(&win, &cap), bar);
        assert_eq!(win.max_x(), pad.window.x, "the windows abut");
        assert_eq!(pad.window.x, bar.max_x() + PAD_GAP / 2.0);
        // The bar keeps its full hit area on the other side.
        assert_eq!(win.x, bar.x - REST_HIT_PAD_X);
        assert_eq!((win.y, win.h), (pad.window.y, pad.window.h));
        // Right to left: mirrored.
        let pad = pad_layout(&a, &bar, REST_H, None, true);
        assert_eq!(pad.side, Side::Left);
        let circle = global(&pad.window, &pad.circle);
        assert_eq!(circle.max_x(), bar.x - PAD_GAP);
        let (win, _) = native_frame_beside(&a, Shape::Rest.size(), Some(pad.side));
        assert_eq!(pad.window.max_x(), win.x);
    }

    /// Every bar state (rest, hover bubble, each take shape), both edges,
    /// both directions, hovered or not: the circle never touches the bar,
    /// the two windows never overlap, the circle keeps the 6 pt gap and
    /// stays on screen, and the tooltip floats on the far side of it.
    #[test]
    fn the_scratchpad_circle_never_overlaps_the_bar() {
        let hover = (200.0, HOVER_H);
        let mut sizes: Vec<(f64, f64)> = Shape::ALL.iter().map(|s| s.capsule()).collect();
        sizes.push(hover);
        for frac in [None, Some(0.0), Some(1.0)] {
            for edge in [Edge::Bottom, Edge::Top] {
                let a = anchor(&builtin(Dock::Bottom(70.0)), edge, frac, false);
                for &size in &sizes {
                    for rtl in [false, true] {
                        for hovered in [false, true] {
                            let bar = frame(&a, size);
                            let d = if hovered {
                                HOVER_H
                            } else {
                                size.1.min(HOVER_H)
                            };
                            let tip = hovered.then_some(110.0);
                            let pad = pad_layout(&a, &bar, d, tip, rtl);
                            let ctx =
                                format!("{size:?} {edge:?} {frac:?} rtl={rtl} hover={hovered}");
                            let circle = global(&pad.window, &pad.circle);
                            assert!(!overlaps(&circle, &bar), "{ctx}");
                            let gap = match pad.side {
                                Side::Right => circle.x - bar.max_x(),
                                Side::Left => bar.x - circle.max_x(),
                            };
                            assert_eq!(gap, PAD_GAP, "{ctx}");
                            assert!(circle.x >= a.bounds.x && circle.max_x() <= a.bounds.max_x());
                            // The bar's window: native at rest/hover, else the web view's.
                            let side = Some(pad.side);
                            let (native, _) = native_frame_beside(&a, size, side);
                            assert!(!overlaps(&pad.window, &native), "native {ctx}");
                            let web = Shape::ALL
                                .iter()
                                .find(|s| s.capsule() == size && !s.labelled());
                            if let Some(s) = web {
                                let w = frame(&a, s.size());
                                assert!(!overlaps(&pad.window, &w), "web {ctx}");
                            }
                            if let Some(t) = pad.tip {
                                let t = global(&pad.window, &t);
                                assert!(!overlaps(&t, &bar), "{ctx}");
                                match edge {
                                    Edge::Bottom => assert_eq!(t.y, circle.max_y() + LABEL_GAP),
                                    Edge::Top => assert_eq!(t.max_y(), circle.y - LABEL_GAP),
                                }
                            }
                            // On the bar's anchored edge.
                            match edge {
                                Edge::Bottom => assert_eq!(circle.y, bar.y, "{ctx}"),
                                Edge::Top => assert_eq!(circle.max_y(), bar.max_y(), "{ctx}"),
                            }
                        }
                    }
                }
            }
        }
    }

    /// A bar dragged against a display edge puts the circle on the side
    /// with room.
    #[test]
    fn the_scratchpad_circle_flips_at_a_display_edge() {
        let s = builtin(Dock::Bottom(70.0));
        let a = anchor(&s, Edge::Bottom, Some(1.0), false);
        let bar = frame(&a, Shape::Locked.size());
        assert_eq!(pad_side(&a, &bar, false), Side::Left);
        let a = anchor(&s, Edge::Bottom, Some(0.0), false);
        let bar = frame(&a, Shape::Locked.size());
        assert_eq!(pad_side(&a, &bar, true), Side::Right);
        assert_eq!(pad_side(&a, &bar, false), Side::Right);
    }

    /// Writes the bar and circle layout of each state to
    /// `$WB_PAD_LAYOUT_JSON` (for the before/after renders of the S-24 fix).
    #[test]
    #[ignore = "writes a layout dump for screenshots"]
    fn dump_scratchpad_layouts() {
        let Some(path) = std::env::var_os("WB_PAD_LAYOUT_JSON") else {
            return;
        };
        let states: [(&str, (f64, f64), bool, bool); 8] = [
            ("rest", Shape::Rest.size(), false, false),
            ("rest_pad_hover", Shape::Rest.size(), true, false),
            ("bar_hover", (196.0, HOVER_H), false, false),
            ("held", Shape::Held.capsule(), false, false),
            ("locked", Shape::Locked.capsule(), false, false),
            ("locked_pad_hover", Shape::Locked.capsule(), true, false),
            ("transcribing", Shape::Status.capsule(), false, false),
            ("rest_rtl_pad_hover", Shape::Rest.size(), true, true),
        ];
        let a = anchor(&builtin(Dock::Bottom(70.0)), Edge::Bottom, None, false);
        let rows: Vec<serde_json::Value> = states
            .iter()
            .map(|&(name, size, hovered, rtl)| {
                let bar = frame(&a, size);
                let d = if hovered {
                    HOVER_H
                } else {
                    size.1.min(HOVER_H)
                };
                let pad = pad_layout(&a, &bar, d, hovered.then_some(118.0), rtl);
                let (win, _) = native_frame_beside(&a, size, Some(pad.side));
                let g = |r: Rect| serde_json::json!([r.x, r.y, r.w, r.h]);
                serde_json::json!({
                    "name": name, "bar": g(bar), "bar_window": g(win),
                    "pad_window": g(pad.window),
                    "circle": g(global(&pad.window, &pad.circle)),
                    "tip": pad.tip.map(|t| g(global(&pad.window, &t))),
                })
            })
            .collect();
        std::fs::write(path, serde_json::to_string_pretty(&rows).unwrap()).unwrap();
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
