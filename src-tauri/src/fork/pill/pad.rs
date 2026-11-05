// SPDX-License-Identifier: GPL-3.0-or-later
//! The Scratchpad circle beside the Flow Bar (S-24, macOS).
//!
//! With "Scratchpad on the bar" on, the bar itself stays exactly as it is
//! and a small round button sits [`geometry::PAD_GAP`] beside it, in its own
//! borderless non-activating panel (the same kind as the resting bar), so
//! each window keeps exactly its own hit area and the bar's never changes
//! shape for it. The circle is as tall as the bar the user sees (the 6 pt
//! resting line, the hover bubble, the take's capsule; the web view's
//! during a take), on the bar's anchored edge, on the trailing side
//! (mirrored in right-to-left languages; flipped at a display edge). It
//! has no label; on hover it grows to the hover height, shows its glyph and
//! a "Scratchpad" tooltip in the label pill's place and style. A click
//! opens the Scratchpad without taking focus from the frontmost app.
//! Size changes run on the bar's spring, at once under Reduce Motion.
//! Layout: [`placement::pad_layout`].

use std::cell::{Cell, RefCell};

use objc2::rc::Retained;
use objc2::runtime::AnyObject;
use objc2::{
    define_class, msg_send, sel, AnyThread, DefinedClass, MainThreadMarker, MainThreadOnly,
};
use objc2_app_kit::{
    NSBezierPath, NSEvent, NSLineCapStyle, NSLineJoinStyle, NSResponder, NSTrackingArea,
    NSTrackingAreaOptions, NSView, NSWindow,
};
use objc2_foundation::{NSObject, NSPoint, NSRect, NSSize, NSString};

use super::geometry::{self, Shape};
use super::native::{
    bar_panel, capsule_radius, draw_text, is_dark, now_ms, reduce_motion, rgba, text_size,
    Callback, Morph, NativeEvent, RestingPanel, WebBar,
};
use super::placement::{self, Anchor, PadLayout, Rect};

/// Where the bar the circle follows is.
#[derive(Clone, Copy, Debug, PartialEq)]
struct Bar {
    anchor: Anchor,
    /// Its capsule drawn now and the one it heads for (a web view bar runs
    /// its own spring; the native bar hands over each frame).
    shown: (f64, f64),
    target: (f64, f64),
    morph: Option<Morph>,
    labelled: bool,
}

pub struct PadIvars {
    bar: Cell<Option<Bar>>,
    hover: Cell<bool>,
    rtl: Cell<bool>,
    /// Tooltip / VoiceOver name and help.
    texts: RefCell<(String, String)>,
    /// Hover grow: diameter morph and the tooltip's fade.
    hover_morph: Cell<Option<Morph>>,
    shown_d: Cell<f64>,
    layout: Cell<Option<PadLayout>>,
    tick_pending: Cell<bool>,
    on_event: Callback,
}

/// A shrinking web view capsule leads; the circle follows this much later,
/// so it never overlaps a capsule still on its way down.
const FOLLOW_DELAY_MS: f64 = 60.0;

define_class!(
    // SAFETY: NSView has no subclassing requirements. The ivars are only
    // touched on the main thread (the class is MainThreadOnly). Every
    // overridden method has the signature AppKit declares.
    #[unsafe(super(NSView, NSResponder, NSObject))]
    #[thread_kind = MainThreadOnly]
    #[name = "WisprBroPadView"]
    #[ivars = PadIvars]
    pub struct PadView;

    impl PadView {
        #[unsafe(method(isFlipped))]
        fn is_flipped(&self) -> bool {
            false
        }

        #[unsafe(method(acceptsFirstMouse:))]
        fn accepts_first_mouse(&self, _event: Option<&NSEvent>) -> bool {
            true
        }

        #[unsafe(method(drawRect:))]
        fn draw_rect(&self, _dirty: NSRect) {
            self.draw();
        }

        #[unsafe(method(viewDidChangeEffectiveAppearance))]
        fn appearance_changed(&self) {
            self.setNeedsDisplay(true);
        }

        #[unsafe(method(mouseEntered:))]
        fn mouse_entered(&self, _event: &NSEvent) {
            self.set_hover(true);
        }

        #[unsafe(method(mouseExited:))]
        fn mouse_exited(&self, _event: &NSEvent) {
            self.set_hover(false);
        }

        #[unsafe(method(mouseDown:))]
        fn mouse_down(&self, _event: &NSEvent) {}

        #[unsafe(method(mouseUp:))]
        fn mouse_up(&self, _event: &NSEvent) {
            if self.mouse_inside() {
                (self.ivars().on_event)(NativeEvent::Scratchpad);
            }
        }

        #[unsafe(method(padTick:))]
        fn pad_tick(&self, _sender: Option<&AnyObject>) {
            self.ivars().tick_pending.set(false);
            self.relayout();
        }

        // --- Accessibility (VoiceOver) ---

        #[unsafe(method(isAccessibilityElement))]
        fn ax_is_element(&self) -> bool {
            true
        }

        #[unsafe(method_id(accessibilityRole))]
        fn ax_role(&self) -> Option<Retained<NSString>> {
            Some(NSString::from_str("AXButton"))
        }

        #[unsafe(method_id(accessibilityLabel))]
        fn ax_label(&self) -> Option<Retained<NSString>> {
            Some(NSString::from_str(&self.ivars().texts.borrow().0))
        }

        #[unsafe(method_id(accessibilityHelp))]
        fn ax_help(&self) -> Option<Retained<NSString>> {
            Some(NSString::from_str(&self.ivars().texts.borrow().1))
        }

        #[unsafe(method(accessibilityPerformPress))]
        fn ax_press(&self) -> bool {
            (self.ivars().on_event)(NativeEvent::Scratchpad);
            true
        }
    }
);

impl PadView {
    fn new(mtm: MainThreadMarker, frame: NSRect, on_event: Callback) -> Retained<Self> {
        let this = Self::alloc(mtm).set_ivars(PadIvars {
            bar: Cell::new(None),
            hover: Cell::new(false),
            rtl: Cell::new(false),
            texts: RefCell::new((String::new(), String::new())),
            hover_morph: Cell::new(None),
            shown_d: Cell::new(geometry::REST_H),
            layout: Cell::new(None),
            tick_pending: Cell::new(false),
            on_event,
        });
        // SAFETY: NSView's designated initializer.
        let view: Retained<Self> = unsafe { msg_send![super(this), initWithFrame: frame] };
        let options = NSTrackingAreaOptions::MouseEnteredAndExited
            | NSTrackingAreaOptions::ActiveAlways
            | NSTrackingAreaOptions::InVisibleRect;
        // SAFETY: the owner is the view itself, which keeps the area.
        let area = unsafe {
            NSTrackingArea::initWithRect_options_owner_userInfo(
                NSTrackingArea::alloc(),
                NSRect::ZERO,
                options,
                Some(&view),
                None,
            )
        };
        view.addTrackingArea(&area);
        view
    }

    fn mouse_inside(&self) -> bool {
        let Some(window) = self.window() else {
            return false;
        };
        let f = window.frame();
        let p = NSEvent::mouseLocation();
        Rect::new(f.origin.x, f.origin.y, f.size.width, f.size.height).contains(p.x, p.y)
    }

    fn set_hover(&self, hover: bool) {
        if self.ivars().hover.replace(hover) == hover {
            return;
        }
        let from = self.ivars().shown_d.get();
        let to = self.target_d();
        self.ivars().hover_morph.set(if reduce_motion() {
            None
        } else {
            Some(Morph {
                from: (from, if hover { 0.0 } else { 1.0 }),
                to: (to, if hover { 1.0 } else { 0.0 }),
                t0: now_ms(),
            })
        });
        self.relayout();
    }

    /// The diameter the circle heads for: the hover height while hovered,
    /// else as tall as the bar's capsule (never more than the hover height).
    fn target_d(&self) -> f64 {
        if self.ivars().hover.get() {
            return geometry::HOVER_H;
        }
        let h = self
            .ivars()
            .bar
            .get()
            .map_or(geometry::REST_H, |b| b.shown.1);
        h.clamp(geometry::REST_H, geometry::HOVER_H)
    }

    /// The tooltip shows on hover, unless the web view's label pill takes
    /// its place above the bar, or the bar is a tall card.
    fn tip_width(&self) -> Option<f64> {
        let bar = self.ivars().bar.get()?;
        if !self.ivars().hover.get() || bar.labelled || bar.shown.1 > geometry::HOVER_H {
            return None;
        }
        let w = text_size(&self.ivars().texts.borrow().0).width + 2.0 * geometry::HOVER_PAD_X;
        Some(w.clamp(geometry::HOVER_H, geometry::HOVER_MAX_W).ceil())
    }

    fn relayout(&self) {
        let Some(mut bar) = self.ivars().bar.get() else {
            return;
        };
        let now = now_ms();
        let mut running = false;
        if let Some(m) = bar.morph {
            let t = now - m.t0;
            if t >= geometry::MORPH_MS {
                bar.morph = None;
                bar.shown = m.to;
            } else {
                bar.shown = geometry::morph(m.from, m.to, t.max(0.0));
                running = true;
            }
            self.ivars().bar.set(Some(bar));
        }
        let target = self.target_d();
        let d = match self.ivars().hover_morph.get() {
            Some(m) if now - m.t0 < geometry::MORPH_MS => {
                running = true;
                // Head for the live target (the bar may be growing too).
                let to = (target, m.to.1);
                geometry::morph(m.from, to, now - m.t0).0
            }
            Some(_) => {
                self.ivars().hover_morph.set(None);
                target
            }
            None => target,
        };
        self.ivars().shown_d.set(d);
        let bar_rect = placement::frame(&bar.anchor, bar.shown);
        let layout = placement::pad_layout(
            &bar.anchor,
            &bar_rect,
            d.round(),
            self.tip_width(),
            self.ivars().rtl.get(),
        );
        self.ivars().layout.set(Some(layout));
        if let Some(window) = self.window() {
            let w = layout.window;
            let target = NSRect::new(NSPoint::new(w.x, w.y), NSSize::new(w.w, w.h));
            if window.frame() != target {
                window.setFrame_display(target, true);
            }
        }
        self.setNeedsDisplay(true);
        if running && !self.ivars().tick_pending.replace(true) {
            // SAFETY: `padTick:` is defined on this class with the
            // one-object-argument signature; the run loop retains the view
            // until the call.
            unsafe {
                let _: () = msg_send![
                    self,
                    performSelector: sel!(padTick:),
                    withObject: std::ptr::null::<AnyObject>(),
                    afterDelay: 1.0f64 / 120.0
                ];
            }
        }
    }

    /// The tooltip's opacity (it fades in with the hover grow).
    fn tip_alpha(&self) -> f64 {
        match self.ivars().hover_morph.get() {
            Some(m) => {
                let (p, _) = geometry::spring(now_ms() - m.t0);
                let p = if m.to.1 > m.from.1 { p } else { 1.0 - p };
                ((p - 0.5) / 0.5).clamp(0.0, 1.0)
            }
            None => 1.0,
        }
    }

    fn draw(&self) {
        let Some(layout) = self.ivars().layout.get() else {
            return;
        };
        let dark = is_dark(self);
        let c = layout.circle;
        let rect = NSRect::new(NSPoint::new(c.x, c.y), NSSize::new(c.w, c.h));
        // The bar's material: opaque black, a touch translucent while as
        // thin as the resting line, a faint hairline in dark mode.
        let grown =
            ((c.h - geometry::REST_H) / (geometry::ACTIVE_H - geometry::REST_H)).clamp(0.0, 1.0);
        rgba(0.0, 0.0, 0.0, 0.82 + 0.18 * grown).setFill();
        NSBezierPath::bezierPathWithOvalInRect(rect).fill();
        if dark {
            hairline(rect, c.h / 2.0);
        }
        let glyph = ((c.h - geometry::PAD_GLYPH_MIN_D) / 8.0).clamp(0.0, 1.0);
        if glyph > 0.0 {
            draw_glyph(c.x + c.w / 2.0, c.y + c.h / 2.0, glyph);
        }
        if let Some(t) = layout.tip {
            let alpha = self.tip_alpha();
            if alpha > 0.0 {
                let r = NSRect::new(NSPoint::new(t.x, t.y), NSSize::new(t.w, t.h));
                let radius = capsule_radius(t.h);
                rgba(0.0, 0.0, 0.0, alpha).setFill();
                NSBezierPath::bezierPathWithRoundedRect_xRadius_yRadius(r, radius, radius).fill();
                if dark {
                    hairline(r, radius);
                }
                let label = self.ivars().texts.borrow().0.clone();
                let size = text_size(&label);
                let x = (t.x + (t.w - size.width) / 2.0).max(t.x + geometry::HOVER_PAD_X / 2.0);
                let y = t.y + (t.h - size.height) / 2.0;
                draw_text(&label, NSPoint::new(x, y), alpha);
            }
        }
    }
}

/// The dark-mode hairline of the bar (`native.rs`), around `r`.
fn hairline(r: NSRect, radius: f64) {
    let inset = NSRect::new(
        NSPoint::new(r.origin.x + 0.25, r.origin.y + 0.25),
        NSSize::new(r.size.width - 0.5, r.size.height - 0.5),
    );
    let radius = (radius - 0.25).max(0.0);
    let edge = NSBezierPath::bezierPathWithRoundedRect_xRadius_yRadius(inset, radius, radius);
    rgba(255.0, 255.0, 255.0, 0.12).setStroke();
    edge.setLineWidth(0.5);
    edge.stroke();
}

/// The Scratchpad glyph (our own drawing): a note sheet with two lines,
/// 11 × 13 pt, centred on `(cx, cy)`.
fn draw_glyph(cx: f64, cy: f64, alpha: f64) {
    let (w, h) = (11.0, 13.0);
    let sheet = NSRect::new(NSPoint::new(cx - w / 2.0, cy - h / 2.0), NSSize::new(w, h));
    let path = NSBezierPath::bezierPathWithRoundedRect_xRadius_yRadius(sheet, 2.5, 2.5);
    path.setLineWidth(1.4);
    rgba(255.0, 255.0, 255.0, 0.92 * alpha).setStroke();
    path.stroke();
    let lines = NSBezierPath::bezierPath();
    for dy in [2.0, -1.5] {
        lines.moveToPoint(NSPoint::new(cx - 2.6, cy + dy));
        lines.lineToPoint(NSPoint::new(cx + 2.6, cy + dy));
    }
    lines.setLineWidth(1.4);
    lines.setLineCapStyle(NSLineCapStyle::Round);
    lines.setLineJoinStyle(NSLineJoinStyle::Round);
    lines.stroke();
}

/// The circle's panel and view.
pub struct Pad {
    panel: Retained<RestingPanel>,
    view: Retained<PadView>,
}

impl Pad {
    /// Builds the panel (hidden). Main thread only.
    pub fn new(mtm: MainThreadMarker, on_event: Callback) -> Pad {
        let (w, h) = Shape::Rest.size();
        let rect = NSRect::new(NSPoint::new(0.0, 0.0), NSSize::new(w, h));
        let panel = bar_panel(mtm, rect);
        let view = PadView::new(mtm, rect, on_event);
        panel.setContentView(Some(&view));
        Pad { panel, view }
    }

    pub fn panel(&self) -> &NSWindow {
        &self.panel
    }

    pub fn set_texts(&self, label: &str, help: &str) {
        let next = (label.to_string(), help.to_string());
        if *self.view.ivars().texts.borrow() != next {
            *self.view.ivars().texts.borrow_mut() = next;
            self.view.relayout();
        }
    }

    pub fn set_rtl(&self, rtl: bool) {
        if self.view.ivars().rtl.replace(rtl) != rtl {
            self.view.relayout();
        }
    }

    /// Beside the native bar, whose capsule is `shown` now (the native
    /// view runs the spring and calls this on every frame).
    pub fn follow_native(&self, anchor: Anchor, shown: (f64, f64)) {
        let next = Bar {
            anchor,
            shown,
            target: shown,
            morph: None,
            labelled: false,
        };
        if self.view.ivars().bar.replace(Some(next)) != Some(next) {
            self.view.relayout();
        }
    }

    /// Beside the web view's capsule. The web view springs to a new size by
    /// itself: the circle jumps out at once when the capsule grows (it never
    /// covers it) and follows a shrink on the same spring, a beat later.
    pub fn follow_web(&self, web: WebBar) {
        let old = self.view.ivars().bar.get();
        if old.is_some_and(|b| {
            b.anchor == web.anchor && b.target == web.capsule && b.labelled == web.labelled
        }) {
            return;
        }
        let from = old.map_or(web.capsule, |b| b.shown);
        let shrinks = web.capsule.0 < from.0 && old.is_some_and(|b| b.anchor == web.anchor);
        let morph = (shrinks && !reduce_motion()).then(|| Morph {
            from,
            to: web.capsule,
            t0: now_ms() + FOLLOW_DELAY_MS,
        });
        self.view.ivars().bar.set(Some(Bar {
            anchor: web.anchor,
            shown: if morph.is_some() { from } else { web.capsule },
            target: web.capsule,
            morph,
            labelled: web.labelled,
        }));
        self.view.relayout();
    }

    pub fn show(&self) {
        if !self.panel.isVisible() {
            self.view.relayout();
            self.panel.orderFrontRegardless();
        }
    }

    pub fn hide(&self) {
        if self.panel.isVisible() {
            self.panel.orderOut(None);
        }
        self.view.ivars().hover.set(false);
        self.view.ivars().hover_morph.set(None);
    }

    /// Back in front (a Space change, or the web view ordered in above it).
    pub fn reassert(&self) {
        if self.panel.isVisible() {
            self.panel.orderFrontRegardless();
        }
    }
}
