// SPDX-License-Identifier: GPL-3.0-or-later
//! Hover over the bar group (rc9 owner report): the resting bar, the gap
//! and the S-24 Scratchpad circle hover as one.
//!
//! The bar and the circle are two panels whose windows abut half way
//! across the [`geometry::PAD_GAP`] (`placement::native_frame_beside`,
//! `placement::pad_layout`). Each tracking area on its own made the bar
//! lose hover the moment the pointer crossed into the circle's window: the
//! bar shrank, the circle (which sits beside the bar) slid back under the
//! pointer and shrank too, and the circle could not be reached. Here the
//! group's hover is one state:
//!
//! - it starts when the pointer enters the bar's window (the circle alone
//!   grows by itself and shows its tooltip, the bar stays at rest, so the
//!   circle never runs away from a pointer that came straight to it);
//! - it holds while the pointer is anywhere in [`group_contains`]: the bar's
//!   window, the circle's window and the strip between them;
//! - it ends [`EXIT_GRACE_MS`] after the pointer left that region (back in
//!   time cancels the exit), or at once on [`Group::dismiss`] (a click
//!   outside, the bar leaving rest).
//!
//! Pure and tested here; `native.rs` feeds it the pointer.
//!
//! [`geometry::PAD_GAP`]: super::geometry::PAD_GAP

use super::placement::Rect;

/// How long the pointer may be outside the group before it shrinks.
pub const EXIT_GRACE_MS: f64 = 150.0;
/// How often the native bar re-reads the pointer while the group is grown
/// and the pointer is not over the bar itself (tracking areas do not fire
/// when a window moves out from under a still pointer).
pub const POLL_MS: f64 = 30.0;

/// Whether `(x, y)` (global) is in the bar group: the bar's window, the
/// circle's window (when the circle is shown) or the strip between them,
/// over the height the two windows share.
pub fn group_contains(bar: &Rect, pad: Option<&Rect>, x: f64, y: f64) -> bool {
    if bar.contains(x, y) {
        return true;
    }
    let Some(pad) = pad else {
        return false;
    };
    if pad.contains(x, y) {
        return true;
    }
    let (x0, x1) = if pad.x >= bar.max_x() {
        (bar.max_x(), pad.x)
    } else if pad.max_x() <= bar.x {
        (pad.max_x(), bar.x)
    } else {
        return false; // overlapping windows: covered above
    };
    let y0 = bar.y.max(pad.y);
    let y1 = bar.max_y().min(pad.max_y());
    x >= x0 && x < x1 && y >= y0 && y < y1
}

/// The group's hover state.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Group {
    grown: bool,
    /// When the pointer left the group (ms), while the exit grace runs.
    left_at: Option<f64>,
}

impl Group {
    /// The bar shows its hover bubble (and the circle keeps its hover size).
    pub fn grown(&self) -> bool {
        self.grown
    }

    /// One pointer reading at `now` (ms): over the bar's own window, and
    /// inside [`group_contains`]. Returns whether the group is grown.
    pub fn pointer(&mut self, over_bar: bool, in_group: bool, now: f64) -> bool {
        if over_bar {
            self.grown = true;
            self.left_at = None;
        } else if self.grown {
            if in_group {
                self.left_at = None;
            } else {
                let since = *self.left_at.get_or_insert(now);
                if now - since >= EXIT_GRACE_MS {
                    self.dismiss();
                }
            }
        }
        self.grown
    }

    /// When the next pointer reading is due (ms), `None` while nothing is
    /// pending (at rest, or the pointer is over the bar, whose tracking
    /// area reports the exit).
    pub fn next_check(&self, over_bar: bool, now: f64) -> Option<f64> {
        if !self.grown || over_bar {
            return None;
        }
        Some(match self.left_at {
            Some(t) => (t + EXIT_GRACE_MS).min(now + POLL_MS).max(now),
            None => now + POLL_MS,
        })
    }

    /// Shrinks at once.
    pub fn dismiss(&mut self) {
        self.grown = false;
        self.left_at = None;
    }
}

#[cfg(test)]
mod tests {
    use super::super::geometry::{Shape, HOVER_H, HOVER_MIN_W, PAD_GAP};
    use super::super::placement::{
        anchor, display_key, frame, native_frame_beside, pad_layout, pad_side, Anchor, Edge,
        PadLayout, Screen, Side,
    };
    use super::*;

    fn screen() -> Screen {
        let frame = Rect::new(0.0, 0.0, 1512.0, 982.0);
        Screen {
            key: display_key("Built-in Retina Display", &frame),
            frame,
            visible: Rect::new(0.0, 70.0, 1512.0, 982.0 - 38.0 - 70.0),
            scale: 2.0,
        }
    }

    /// The hover bubble's size (the hint's width is measured by AppKit; any
    /// width in range behaves the same).
    const HOVER: (f64, f64) = (HOVER_MIN_W + 76.0, HOVER_H);

    /// The bar's window and the circle's layout as `native.rs` and `pad.rs`
    /// place them: the bar grown or at rest, the circle hovered (its own
    /// tooltip) or following the bar's height.
    fn scene(a: &Anchor, grown: bool, pad_hover: bool, rtl: bool) -> (Rect, PadLayout) {
        let size = if grown { HOVER } else { Shape::Rest.size() };
        let bar = frame(a, size);
        let side = pad_side(a, &bar, rtl);
        let (win, _) = native_frame_beside(a, size, Some(side));
        let d = if pad_hover { HOVER_H } else { size.1 };
        let tip = pad_hover.then_some(110.0);
        (win, pad_layout(a, &bar, d, tip, rtl))
    }

    fn circle(p: &PadLayout) -> Rect {
        Rect::new(
            p.window.x + p.circle.x,
            p.window.y + p.circle.y,
            p.circle.w,
            p.circle.h,
        )
    }

    /// One glide of the pointer along `y` from `from` to `to` (1 pt steps,
    /// 4 ms apart, the scene re-laid out after every step as the panels do).
    /// Returns the last scene, and asserts the group never shrank.
    fn glide(a: &Anchor, rtl: bool, from: f64, to: f64, y: f64, start_grown: bool) -> Group {
        glide_from(a, rtl, from, to, y, start_grown, false)
    }

    fn glide_from(
        a: &Anchor,
        rtl: bool,
        from: f64,
        to: f64,
        y: f64,
        start_grown: bool,
        start_on_pad: bool,
    ) -> Group {
        let mut g = Group::default();
        if start_grown {
            g.pointer(true, true, 0.0);
        }
        let steps = (to - from).abs().ceil() as usize;
        let dir = (to - from).signum();
        let mut was_grown = g.grown();
        // The circle's own hover: it starts in its resting window and holds
        // in its hovered one (taller, with the tooltip).
        let mut pad_hover = start_on_pad;
        for i in 0..=steps {
            let x = from + dir * i as f64;
            let now = 4.0 * i as f64;
            let (bar, rest_pad) = scene(a, g.grown(), false, rtl);
            let (_, hover_pad) = scene(a, g.grown(), true, rtl);
            let over_bar = bar.contains(x, y);
            pad_hover = if pad_hover {
                hover_pad.window.contains(x, y)
            } else {
                rest_pad.window.contains(x, y)
            };
            let pad = if pad_hover { hover_pad } else { rest_pad };
            let inside = group_contains(&bar, Some(&pad.window), x, y);
            assert!(
                inside,
                "x={x} y={y} left the group (bar {bar:?}, pad {:?})",
                pad.window
            );
            g.pointer(over_bar, inside, now);
            assert!(!was_grown || g.grown(), "shrank at x={x}");
            was_grown = g.grown();
        }
        g
    }

    /// Gliding from the hovered bar onto the circle keeps both grown the
    /// whole way, and the pointer ends on the circle (it did not run away).
    #[test]
    fn gliding_from_the_bar_onto_the_circle_keeps_the_group_grown() {
        for rtl in [false, true] {
            for edge in [Edge::Bottom, Edge::Top] {
                let a = anchor(&screen(), edge, None, false);
                let (bar, pad) = scene(&a, true, false, rtl);
                assert_eq!(pad.side, if rtl { Side::Left } else { Side::Right });
                let c = circle(&pad);
                let y = c.y + c.h / 2.0;
                let start = frame(&a, HOVER).mid_x();
                let end = c.mid_x();
                let g = glide(&a, rtl, start, end, y, true);
                assert!(g.grown(), "rtl={rtl} {edge:?}");
                // At the end the pointer is over the circle itself.
                let (_, pad) = scene(&a, true, true, rtl);
                assert!(circle(&pad).contains(end, y), "rtl={rtl} {edge:?}");
                assert!(!bar.contains(end, y));
            }
        }
    }

    /// Back from the circle onto the bar: the circle came first (the bar at
    /// rest), entering the bar grows the group, and it stays grown back
    /// across the gap.
    #[test]
    fn gliding_from_the_circle_onto_the_bar_grows_the_group_and_holds() {
        for rtl in [false, true] {
            let a = anchor(&screen(), Edge::Bottom, None, false);
            let (_, pad) = scene(&a, false, true, rtl);
            let c = circle(&pad);
            // Along the bar's line (the circle sits on the same edge).
            let line = frame(&a, Shape::Rest.size());
            let y = line.y + line.h / 2.0;
            let bar_mid = line.mid_x();
            let g = glide_from(&a, rtl, c.mid_x(), bar_mid, y, false, true);
            assert!(g.grown(), "rtl={rtl}");
            // And back out to the (now moved) circle, at its middle.
            let (_, pad) = scene(&a, true, false, rtl);
            let c = circle(&pad);
            let g2 = glide(&a, rtl, bar_mid, c.mid_x(), c.y + c.h / 2.0, true);
            assert!(g2.grown(), "rtl={rtl}");
        }
    }

    /// A bar dragged to the display's right edge puts the circle on its
    /// left; the glide holds the same way.
    #[test]
    fn the_glide_holds_with_the_circle_flipped_at_the_display_edge() {
        let s = screen();
        let a = anchor(&s, Edge::Bottom, Some(1.0), false);
        let (_, pad) = scene(&a, true, false, false);
        assert_eq!(pad.side, Side::Left, "flipped at the edge");
        let c = circle(&pad);
        let y = c.y + c.h / 2.0;
        let g = glide(&a, false, frame(&a, HOVER).mid_x(), c.mid_x(), y, true);
        assert!(g.grown());
        // And right to left at the left edge flips it to the right.
        let a = anchor(&s, Edge::Bottom, Some(0.0), false);
        let (_, pad) = scene(&a, true, false, true);
        assert_eq!(pad.side, Side::Right);
        let c = circle(&pad);
        let g = glide(&a, true, frame(&a, HOVER).mid_x(), c.mid_x(), y, true);
        assert!(g.grown());
    }

    /// The strip between the two windows counts, over the height they
    /// share; above it does not.
    #[test]
    fn the_gap_between_the_windows_is_part_of_the_group() {
        let bar = Rect::new(100.0, 10.0, 50.0, 40.0);
        let pad = Rect::new(150.0 + PAD_GAP, 4.0, 30.0, 50.0);
        let mid = 150.0 + PAD_GAP / 2.0;
        assert!(group_contains(&bar, Some(&pad), mid, 20.0));
        assert!(!group_contains(&bar, Some(&pad), mid, 52.0));
        assert!(!group_contains(&bar, None, mid, 20.0));
        // Mirrored (circle on the left).
        let pad = Rect::new(100.0 - PAD_GAP - 30.0, 4.0, 30.0, 50.0);
        assert!(group_contains(
            &bar,
            Some(&pad),
            100.0 - PAD_GAP / 2.0,
            20.0
        ));
        assert!(!group_contains(&bar, Some(&pad), 40.0, 20.0));
    }

    /// Leaving the group shrinks it only after the grace; coming back in
    /// time cancels the exit.
    #[test]
    fn leaving_the_group_shrinks_after_the_grace() {
        let mut g = Group::default();
        assert!(
            !g.pointer(false, true, 0.0),
            "the circle alone does not grow the bar"
        );
        assert!(g.pointer(true, true, 10.0));
        assert_eq!(g.next_check(true, 10.0), None);
        assert!(g.pointer(false, false, 100.0));
        assert_eq!(g.next_check(false, 100.0), Some(130.0));
        assert!(g.pointer(false, false, 240.0));
        assert_eq!(g.next_check(false, 240.0), Some(250.0));
        // Back in the gap before the grace ran out.
        assert!(g.pointer(false, true, 245.0));
        assert!(g.pointer(false, false, 300.0));
        assert!(g.pointer(false, false, 449.0));
        assert!(!g.pointer(false, false, 450.0));
        assert_eq!(g.next_check(false, 450.0), None);
        // Dismiss: at once.
        g.pointer(true, true, 500.0);
        g.dismiss();
        assert!(!g.grown());
    }
}
