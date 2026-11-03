// SPDX-License-Identifier: GPL-3.0-or-later
//! The resting Flow Bar as a native AppKit panel (macOS, S-03).
//!
//! RS-03 measured the always-visible web view pill growing its WebContent
//! process by ~49 MB over 97 takes, at the 50 MB ceiling of R5 §5.2, so the
//! resting bar is drawn natively (the RS-03 fallback; anarlog's Swift pill
//! and Voquill's `rust_macos_pill` were the references for the approach,
//! no code is taken from either) and the `fork-pill` web view exists only
//! around a take (`panel.rs`).
//!
//! The panel: a borderless, non-activating `NSPanel` subclass that can never
//! become key or main, at the status level, on every Space and over
//! full-screen apps, `hidesOnDeactivate` off (R5 §4.9; no private API,
//! DEC-28). Its one view draws a thin opaque capsule (Design W4: 38 × 6 pt,
//! inside a slightly larger hit area); on hover (a native tracking area,
//! which fires while another app is active: RS-03) it grows into a black
//! bubble with the localized hint. Every size change runs on the bar's
//! spring (`geometry::spring`, the web view's `SPRING.bar`), except under
//! Reduce Motion: the bridge grows out of the resting line when a take
//! starts, and the resting line grows back out of the last active capsule
//! when the take ends. A Dictate click starts hands-free
//! dictation; the optional S-24 group adds a Scratchpad button beside it,
//! without taking focus from the frontmost app (`acceptsFirstMouse`, the
//! panel never activates the app); right-click offers "Hide for an hour";
//! a horizontal drag moves the bar along its edge and the position is
//! remembered per display. While the web view loads for a new take, the
//! same view shows a "bridge" capsule of the active size, so the bar
//! reacts at once.
//!
//! VoiceOver: the single-slot view is a button named "Dictation bar" with
//! the hint as its help text. With S-24 enabled, two native NSButtons expose
//! independent Dictate and Scratchpad names, actions and tooltips instead.
//! Announcements come from `accessibility::announce`.

use std::cell::{Cell, RefCell};

use objc2::rc::Retained;
use objc2::runtime::AnyObject;
use objc2::{
    define_class, msg_send, sel, AnyThread, DefinedClass, MainThreadMarker, MainThreadOnly,
};
use objc2_app_kit::{
    NSAppearance, NSAppearanceCustomization, NSAppearanceNameAqua, NSAppearanceNameDarkAqua,
    NSBackingStoreType, NSBezierPath, NSButton, NSColor, NSEvent, NSFont, NSFontAttributeName,
    NSFontWeightMedium, NSForegroundColorAttributeName, NSMenu, NSMenuItem, NSPanel, NSResponder,
    NSScreen, NSStatusWindowLevel, NSTrackingArea, NSTrackingAreaOptions, NSView,
    NSWindowAnimationBehavior, NSWindowCollectionBehavior, NSWindowSharingType, NSWindowStyleMask,
};
use objc2_foundation::{NSArray, NSDictionary, NSObject, NSPoint, NSRect, NSSize, NSString};

use super::geometry::{self, Shape};
use super::placement::{self, Anchor, Rect, Screen};
use super::slots::{self, Slot};

/// What the native view shows.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Mode {
    /// The thin resting bar (hover shows the hint).
    Rest,
    /// The active shape while the web view loads.
    Bridge(Shape),
}

/// What the user did on the native bar.
#[derive(Clone, Debug, PartialEq)]
pub enum NativeEvent {
    Click,
    Scratchpad,
    HideForAnHour,
    /// A drag ended on display `key`; `x_frac` `None` = back at the centre.
    Dragged {
        key: String,
        x_frac: Option<f64>,
    },
}

type Callback = Box<dyn Fn(NativeEvent)>;

/// Localized strings of the native bar.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Texts {
    /// The hover hint ("Click or hold fn to dictate").
    pub hint: String,
    /// VoiceOver name ("Dictation bar").
    pub name: String,
    /// The context menu item.
    pub hide_for_an_hour: String,
    /// S-24 grouped bar labels and per-slot hover/VoiceOver help.
    pub dictate: String,
    pub dictate_hint: String,
    pub scratchpad: String,
    pub scratchpad_hint: String,
}

struct Drag {
    start_mouse_x: f64,
    start_centre_x: f64,
    moved: bool,
}

pub struct ViewIvars {
    mode: Cell<Mode>,
    hover: Cell<bool>,
    hover_slot: Cell<Slot>,
    scratchpad_enabled: Cell<bool>,
    texts: RefCell<Texts>,
    slot_buttons: RefCell<Option<(Retained<NSButton>, Retained<NSButton>)>>,
    anchor: Cell<Option<Anchor>>,
    screen: RefCell<Option<Screen>>,
    drag: RefCell<Option<Drag>>,
    /// Design W4: the capsule drawn now, the size it is heading for, and
    /// the running spring morph between them (`geometry::spring`).
    shown: Cell<(f64, f64)>,
    target: Cell<(f64, f64)>,
    morph: Cell<Option<Morph>>,
    /// Where the capsule sits inside the window (window coordinates).
    capsule: Cell<NSRect>,
    /// The take's appearance time (Unix ms): the bridge's grow out of the
    /// resting line starts there, the same clock the web view continues.
    grown_at: Cell<Option<u64>>,
    tick_pending: Cell<bool>,
    on_event: Callback,
}

/// A size morph on the bar's spring.
#[derive(Clone, Copy, Debug, PartialEq)]
struct Morph {
    from: (f64, f64),
    to: (f64, f64),
    /// Start, Unix ms (fractional).
    t0: f64,
}

/// A drag starts after this many points of horizontal movement.
const DRAG_THRESHOLD: f64 = 3.0;

define_class!(
    // SAFETY: NSPanel has no subclassing requirements; the overrides only
    // return constants.
    #[unsafe(super(NSPanel))]
    #[thread_kind = MainThreadOnly]
    #[name = "WisprBroRestingPanel"]
    pub struct RestingPanel;

    impl RestingPanel {
        #[unsafe(method(canBecomeKeyWindow))]
        fn can_become_key(&self) -> bool {
            false
        }

        #[unsafe(method(canBecomeMainWindow))]
        fn can_become_main(&self) -> bool {
            false
        }
    }
);

define_class!(
    // SAFETY: NSView has no subclassing requirements. The ivars are only
    // touched on the main thread (the class is MainThreadOnly). Every
    // overridden method has the signature AppKit declares.
    #[unsafe(super(NSView, NSResponder, NSObject))]
    #[thread_kind = MainThreadOnly]
    #[name = "WisprBroRestingView"]
    #[ivars = ViewIvars]
    pub struct RestingView;

    impl RestingView {
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
            if self.ivars().mode.get() == Mode::Rest && !self.ivars().scratchpad_enabled.get() {
                self.update_hover_slot();
                if !self.ivars().hover.get() {
                    self.ivars().hover.set(true);
                    self.relayout();
                }
            }
        }

        #[unsafe(method(mouseMoved:))]
        fn mouse_moved(&self, _event: &NSEvent) {
            if self.ivars().mode.get() == Mode::Rest
                && self.ivars().hover.get()
                && !self.ivars().scratchpad_enabled.get()
            {
                self.update_hover_slot();
            }
        }

        #[unsafe(method(mouseExited:))]
        fn mouse_exited(&self, _event: &NSEvent) {
            if self.ivars().drag.borrow().is_some() {
                return; // keep the bubble while dragging past its edge
            }
            if self.ivars().hover.get() {
                self.ivars().hover.set(false);
                self.relayout();
            }
        }

        #[unsafe(method(mouseDown:))]
        fn mouse_down(&self, _event: &NSEvent) {
            let Some(anchor) = self.ivars().anchor.get() else {
                return;
            };
            *self.ivars().drag.borrow_mut() = Some(Drag {
                start_mouse_x: NSEvent::mouseLocation().x,
                start_centre_x: anchor.centre_x,
                moved: false,
            });
        }

        #[unsafe(method(mouseDragged:))]
        fn mouse_dragged(&self, _event: &NSEvent) {
            if self.ivars().mode.get() != Mode::Rest {
                return;
            }
            let x = NSEvent::mouseLocation().x;
            let centre = {
                let mut drag = self.ivars().drag.borrow_mut();
                let Some(d) = drag.as_mut() else { return };
                let dx = x - d.start_mouse_x;
                if !d.moved && dx.abs() < DRAG_THRESHOLD {
                    return;
                }
                d.moved = true;
                d.start_centre_x + dx
            };
            let width = self.current_size().0;
            let screen = self.ivars().screen.borrow().clone();
            if let (Some(mut anchor), Some(screen)) = (self.ivars().anchor.get(), screen) {
                // Free movement while dragging, inside the visible frame.
                let half = width / 2.0;
                let b = screen.visible;
                anchor.centre_x = if b.w >= width {
                    centre.clamp(b.x + half, b.max_x() - half)
                } else {
                    b.mid_x()
                };
                self.ivars().anchor.set(Some(anchor));
                self.relayout();
            }
        }

        #[unsafe(method(mouseUp:))]
        fn mouse_up(&self, _event: &NSEvent) {
            let Some(drag) = self.ivars().drag.borrow_mut().take() else {
                return;
            };
            if !drag.moved {
                self.update_hover_slot();
                let event = if self.ivars().scratchpad_enabled.get()
                    && self.ivars().hover_slot.get() == Slot::Scratchpad
                {
                    NativeEvent::Scratchpad
                } else {
                    NativeEvent::Click
                };
                (self.ivars().on_event)(event);
                return;
            }
            let screen = self.ivars().screen.borrow().clone();
            let (Some(mut anchor), Some(screen)) = (self.ivars().anchor.get(), screen) else {
                return;
            };
            let width = self.current_size().0;
            // Where the pointer let go (the last drag event may have been
            // coalesced away).
            let released = drag.start_centre_x + (NSEvent::mouseLocation().x - drag.start_mouse_x);
            anchor.centre_x = placement::dragged_centre(&screen, released, width);
            self.ivars().anchor.set(Some(anchor));
            self.relayout();
            let centred = (anchor.centre_x - screen.frame.mid_x()).abs() < 0.5;
            (self.ivars().on_event)(NativeEvent::Dragged {
                key: screen.key.clone(),
                x_frac: (!centred).then(|| placement::x_frac(&screen, anchor.centre_x)),
            });
            // The pointer may have left the bubble during the drag.
            if !self.mouse_inside() && self.ivars().hover.get() {
                self.ivars().hover.set(false);
                self.relayout();
            }
        }

        #[unsafe(method(rightMouseDown:))]
        fn right_mouse_down(&self, event: &NSEvent) {
            if self.ivars().mode.get() != Mode::Rest {
                return;
            }
            let mtm = self.mtm();
            let title = NSString::from_str(&self.ivars().texts.borrow().hide_for_an_hour);
            let menu = NSMenu::new(mtm);
            // SAFETY: `hideForAnHour:` is defined below with the action
            // signature; the view outlives the menu it pops up.
            let item = unsafe {
                NSMenuItem::initWithTitle_action_keyEquivalent(
                    NSMenuItem::alloc(mtm),
                    &title,
                    Some(sel!(hideForAnHour:)),
                    &NSString::from_str(""),
                )
            };
            // SAFETY: the target is this view, alive for the menu's lifetime.
            unsafe { item.setTarget(Some(self)) };
            menu.addItem(&item);
            NSMenu::popUpContextMenu_withEvent_forView(&menu, event, self);
        }

        #[unsafe(method(hideForAnHour:))]
        fn hide_for_an_hour(&self, _sender: Option<&AnyObject>) {
            (self.ivars().on_event)(NativeEvent::HideForAnHour);
        }

        #[unsafe(method(morphTick:))]
        fn morph_tick(&self, _sender: Option<&AnyObject>) {
            self.ivars().tick_pending.set(false);
            self.relayout();
        }

        #[unsafe(method(dictateSlot:))]
        fn dictate_slot(&self, _sender: Option<&AnyObject>) {
            (self.ivars().on_event)(NativeEvent::Click);
        }

        #[unsafe(method(scratchpadSlot:))]
        fn scratchpad_slot(&self, _sender: Option<&AnyObject>) {
            (self.ivars().on_event)(NativeEvent::Scratchpad);
        }

        // --- Accessibility (VoiceOver) ---

        #[unsafe(method(isAccessibilityElement))]
        fn ax_is_element(&self) -> bool {
            !self.ivars().scratchpad_enabled.get()
        }

        #[unsafe(method_id(accessibilityRole))]
        fn ax_role(&self) -> Option<Retained<NSString>> {
            Some(NSString::from_str("AXButton"))
        }

        #[unsafe(method_id(accessibilityLabel))]
        fn ax_label(&self) -> Option<Retained<NSString>> {
            let texts = self.ivars().texts.borrow();
            let label = if self.ivars().scratchpad_enabled.get()
                && self.ivars().hover_slot.get() == Slot::Scratchpad
            {
                &texts.scratchpad
            } else if self.ivars().scratchpad_enabled.get() {
                &texts.dictate
            } else {
                &texts.name
            };
            Some(NSString::from_str(label))
        }

        #[unsafe(method_id(accessibilityHelp))]
        fn ax_help(&self) -> Option<Retained<NSString>> {
            let texts = self.ivars().texts.borrow();
            let help = if self.ivars().scratchpad_enabled.get()
                && self.ivars().hover_slot.get() == Slot::Scratchpad
            {
                &texts.scratchpad_hint
            } else if self.ivars().scratchpad_enabled.get() {
                &texts.dictate_hint
            } else {
                &texts.hint
            };
            Some(NSString::from_str(help))
        }

        #[unsafe(method(accessibilityPerformPress))]
        fn ax_press(&self) -> bool {
            let event = if self.ivars().scratchpad_enabled.get()
                && self.ivars().hover_slot.get() == Slot::Scratchpad
            {
                NativeEvent::Scratchpad
            } else {
                NativeEvent::Click
            };
            (self.ivars().on_event)(event);
            true
        }
    }
);

impl RestingView {
    fn new(mtm: MainThreadMarker, frame: NSRect, on_event: Callback) -> Retained<Self> {
        let this = Self::alloc(mtm).set_ivars(ViewIvars {
            mode: Cell::new(Mode::Rest),
            hover: Cell::new(false),
            hover_slot: Cell::new(Slot::Dictate),
            scratchpad_enabled: Cell::new(false),
            texts: RefCell::new(Texts::default()),
            slot_buttons: RefCell::new(None),
            anchor: Cell::new(None),
            screen: RefCell::new(None),
            drag: RefCell::new(None),
            shown: Cell::new(Shape::Rest.size()),
            target: Cell::new(Shape::Rest.size()),
            morph: Cell::new(None),
            capsule: Cell::new(NSRect::ZERO),
            grown_at: Cell::new(None),
            tick_pending: Cell::new(false),
            on_event,
        });
        // SAFETY: NSView's designated initializer.
        let view: Retained<Self> = unsafe { msg_send![super(this), initWithFrame: frame] };
        // Hover works while another app is active (RS-03: 20/20).
        let options = NSTrackingAreaOptions::MouseEnteredAndExited
            | NSTrackingAreaOptions::MouseMoved
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
        let dictate = slot_button(mtm, &view, sel!(dictateSlot:));
        let scratchpad = slot_button(mtm, &view, sel!(scratchpadSlot:));
        view.addSubview(&dictate);
        view.addSubview(&scratchpad);
        *view.ivars().slot_buttons.borrow_mut() = Some((dictate, scratchpad));
        view.layout_slot_buttons();
        view
    }

    fn layout_slot_buttons(&self) {
        let slot_buttons = self.ivars().slot_buttons.borrow();
        let Some(buttons) = slot_buttons.as_ref() else {
            return;
        };
        let enabled = self.ivars().scratchpad_enabled.get();
        let half = slots::GROUP_WIDTH / 2.0;
        // On the capsule, which sits inside the hit area (`native_frame`).
        let origin = self.ivars().capsule.get().origin;
        for (button, x) in [(&buttons.0, 0.0), (&buttons.1, half)] {
            button.setFrame(NSRect::new(
                NSPoint::new(origin.x + x, origin.y),
                NSSize::new(half, slots::GROUP_HEIGHT),
            ));
            button.setHidden(!enabled);
        }
    }

    fn mouse_inside(&self) -> bool {
        let Some(window) = self.window() else {
            return false;
        };
        let f = window.frame();
        let p = NSEvent::mouseLocation();
        Rect::new(f.origin.x, f.origin.y, f.size.width, f.size.height).contains(p.x, p.y)
    }

    fn update_hover_slot(&self) {
        let Some(window) = self.window() else {
            return;
        };
        let x = NSEvent::mouseLocation().x - window.frame().origin.x;
        let next = slots::hit_test(
            self.ivars().scratchpad_enabled.get(),
            x,
            window.frame().size.width,
        );
        if self.ivars().hover_slot.replace(next) != next {
            self.relayout();
        }
    }

    fn hover_text(&self) -> String {
        let texts = self.ivars().texts.borrow();
        if !self.ivars().scratchpad_enabled.get() {
            return texts.hint.clone();
        }
        match self.ivars().hover_slot.get() {
            Slot::Dictate => texts.dictate_hint.clone(),
            Slot::Scratchpad => texts.scratchpad_hint.clone(),
        }
    }

    /// The capsule this view is heading for (points).
    fn target_size(&self) -> (f64, f64) {
        match self.ivars().mode.get() {
            Mode::Bridge(shape) => shape.capsule(),
            Mode::Rest if self.ivars().scratchpad_enabled.get() => {
                (slots::GROUP_WIDTH, slots::GROUP_HEIGHT)
            }
            Mode::Rest if self.ivars().hover.get() => {
                let w = text_size(&self.hover_text()).width + 2.0 * geometry::HOVER_PAD_X;
                (
                    w.clamp(geometry::HOVER_MIN_W, geometry::HOVER_MAX_W).ceil(),
                    geometry::HOVER_H,
                )
            }
            Mode::Rest => Shape::Rest.size(),
        }
    }

    /// The width the bar takes for dragging (its target, not a frame of a
    /// morph).
    fn current_size(&self) -> (f64, f64) {
        self.target_size()
    }

    /// Starts the morph to `to` from what is drawn now, at `t0` (Unix ms).
    fn start_morph(&self, to: (f64, f64), t0: f64) {
        let from = self.ivars().shown.get();
        self.ivars().target.set(to);
        if reduce_motion() || from == to {
            self.ivars().morph.set(None);
            self.ivars().shown.set(to);
        } else {
            self.ivars().morph.set(Some(Morph { from, to, t0 }));
        }
    }

    /// Resizes the window around the anchor (following the running morph)
    /// and redraws.
    fn relayout(&self) {
        let target = self.target_size();
        if target != self.ivars().target.get() {
            // The bridge grows from the take's appearance time, so a web
            // view that paints mid-way continues the same curve.
            let t0 = match (self.ivars().mode.get(), self.ivars().grown_at.get()) {
                (Mode::Bridge(_), Some(at)) => (at as f64).min(now_ms()),
                _ => now_ms(),
            };
            self.start_morph(target, t0);
        }
        let shown = match self.ivars().morph.get() {
            Some(m) => {
                let t = now_ms() - m.t0;
                if t >= geometry::MORPH_MS {
                    self.ivars().morph.set(None);
                    m.to
                } else {
                    geometry::morph(m.from, m.to, t)
                }
            }
            None => target,
        };
        self.ivars().shown.set(shown);
        let Some(anchor) = self.ivars().anchor.get() else {
            return;
        };
        let (win, cap) = placement::native_frame(&anchor, shown);
        let capsule = NSRect::new(NSPoint::new(cap.x, cap.y), NSSize::new(cap.w, cap.h));
        if self.ivars().capsule.replace(capsule) != capsule {
            self.layout_slot_buttons();
        }
        if let Some(window) = self.window() {
            let target = NSRect::new(NSPoint::new(win.x, win.y), NSSize::new(win.w, win.h));
            if window.frame() != target {
                window.setFrame_display(target, true);
            }
        }
        self.setNeedsDisplay(true);
        if self.ivars().morph.get().is_some() && !self.ivars().tick_pending.replace(true) {
            // SAFETY: `morphTick:` is defined on this class with the
            // one-object-argument signature; the run loop retains the view
            // until the call.
            unsafe {
                let _: () = msg_send![
                    self,
                    performSelector: sel!(morphTick:),
                    withObject: std::ptr::null::<AnyObject>(),
                    afterDelay: 1.0f64 / 120.0
                ];
            }
        }
    }

    fn is_dark(&self) -> bool {
        let names = NSArray::from_slice(&[unsafe { NSAppearanceNameAqua }, unsafe {
            NSAppearanceNameDarkAqua
        }]);
        self.effectiveAppearance()
            .bestMatchFromAppearancesWithNames(&names)
            .is_some_and(|n| &*n == unsafe { NSAppearanceNameDarkAqua })
    }

    fn draw(&self) {
        let cap = self.ivars().capsule.get();
        if cap.size.width <= 0.0 || cap.size.height <= 0.0 {
            return;
        }
        let (w, h) = (cap.size.width, cap.size.height);
        let radius = capsule_radius(h);
        let path = NSBezierPath::bezierPathWithRoundedRect_xRadius_yRadius(cap, radius, radius);
        // Design W4: an opaque black capsule in both themes, no white edge
        // (the old 55 % edge read as a halo on light windows). The resting
        // line is a touch translucent, like the reference, and turns fully
        // opaque as it grows. In dark mode a faint hairline keeps it findable
        // on black windows.
        let grown =
            ((h - geometry::REST_H) / (geometry::ACTIVE_H - geometry::REST_H)).clamp(0.0, 1.0);
        rgba(0.0, 0.0, 0.0, 0.82 + 0.18 * grown).setFill();
        path.fill();
        if self.is_dark() {
            let inset = NSRect::new(
                NSPoint::new(cap.origin.x + 0.25, cap.origin.y + 0.25),
                NSSize::new(w - 0.5, h - 0.5),
            );
            let r = (radius - 0.25).max(0.0);
            let edge = NSBezierPath::bezierPathWithRoundedRect_xRadius_yRadius(inset, r, r);
            rgba(255.0, 255.0, 255.0, 0.12).setStroke();
            edge.setLineWidth(0.5);
            edge.stroke();
        }

        // Content fades in over the second half of a morph.
        let content = match self.ivars().morph.get() {
            Some(m) => {
                let (p, _) = geometry::spring(now_ms() - m.t0);
                ((p - 0.5) / 0.5).clamp(0.0, 1.0)
            }
            None => 1.0,
        };
        if content <= 0.0 {
            return;
        }
        match self.ivars().mode.get() {
            Mode::Rest if self.ivars().hover.get() => {
                let hint = self.hover_text();
                let size = text_size(&hint);
                let x = cap.origin.x + (w - size.width) / 2.0;
                let y = cap.origin.y + (h - size.height) / 2.0;
                draw_text(
                    &hint,
                    NSPoint::new(x.max(cap.origin.x + geometry::HOVER_PAD_X / 2.0), y),
                    content,
                );
            }
            Mode::Rest if self.ivars().scratchpad_enabled.get() => {
                rgba(255.0, 255.0, 255.0, 0.24 * content).setFill();
                NSBezierPath::bezierPathWithRect(NSRect::new(
                    NSPoint::new(cap.origin.x + w / 2.0, cap.origin.y + 7.0),
                    NSSize::new(1.0, (h - 14.0).max(0.0)),
                ))
                .fill();
            }
            Mode::Bridge(shape) => draw_bridge(shape, cap, content),
            Mode::Rest => {}
        }
    }
}

/// Corner radius of a capsule `h` high: round ends, the cards 18 pt (the
/// web view's `capsuleRadius`; continuous, so a morph never jumps).
fn capsule_radius(h: f64) -> f64 {
    (h / 2.0).min(18.0)
}

/// The bridge's content while the web view loads, laid out like the web
/// view's (`pill.css`) so the hand-over does not move anything: the dotted
/// idle waveform, and for hands-free the ✕ (dark) and ✓ (white) discs.
fn draw_bridge(shape: Shape, cap: NSRect, alpha: f64) {
    let (x0, y0, w, h) = (cap.origin.x, cap.origin.y, cap.size.width, cap.size.height);
    let mid_y = y0 + h / 2.0;
    match shape {
        Shape::Held
        | Shape::HeldLabel
        | Shape::HeldNotice
        | Shape::Locked
        | Shape::LockedLabel
        | Shape::LockedNotice => {
            // Dotted line: one 2 pt dot per bar, 4.5 pt apart.
            let n = geometry::BARS as f64;
            let total = n * WAVE_BAR + (n - 1.0) * WAVE_GAP;
            let mut x = x0 + (w - total) / 2.0;
            rgba(255.0, 255.0, 255.0, 0.55 * alpha).setFill();
            for _ in 0..geometry::BARS {
                let r = NSRect::new(NSPoint::new(x, mid_y - 1.0), NSSize::new(2.0, 2.0));
                NSBezierPath::bezierPathWithOvalInRect(r).fill();
                x += WAVE_BAR + WAVE_GAP;
            }
            if shape.capsule() == Shape::Locked.size() {
                let d = BUTTON_D;
                let cancel = NSRect::new(
                    NSPoint::new(x0 + BUTTON_INSET, mid_y - d / 2.0),
                    NSSize::new(d, d),
                );
                let finish = NSRect::new(
                    NSPoint::new(x0 + w - BUTTON_INSET - d, mid_y - d / 2.0),
                    NSSize::new(d, d),
                );
                rgba(48.0, 48.0, 47.0, alpha).setFill();
                NSBezierPath::bezierPathWithOvalInRect(cancel).fill();
                rgba(255.0, 255.0, 255.0, alpha).setFill();
                NSBezierPath::bezierPathWithOvalInRect(finish).fill();
                // ✕: two 7 pt strokes; ✓: a short and a long stroke.
                let cross = NSBezierPath::bezierPath();
                let (cx, cy) = (cancel.origin.x + d / 2.0, mid_y);
                cross.moveToPoint(NSPoint::new(cx - 3.5, cy - 3.5));
                cross.lineToPoint(NSPoint::new(cx + 3.5, cy + 3.5));
                cross.moveToPoint(NSPoint::new(cx - 3.5, cy + 3.5));
                cross.lineToPoint(NSPoint::new(cx + 3.5, cy - 3.5));
                cross.setLineWidth(1.6);
                cross.setLineCapStyle(objc2_app_kit::NSLineCapStyle::Round);
                rgba(255.0, 255.0, 255.0, alpha).setStroke();
                cross.stroke();
                let check = NSBezierPath::bezierPath();
                let (kx, ky) = (finish.origin.x + d / 2.0, mid_y);
                check.moveToPoint(NSPoint::new(kx - 4.0, ky + 0.2));
                check.lineToPoint(NSPoint::new(kx - 1.2, ky - 2.6));
                check.lineToPoint(NSPoint::new(kx + 4.0, ky + 3.0));
                check.setLineWidth(1.8);
                check.setLineCapStyle(objc2_app_kit::NSLineCapStyle::Round);
                check.setLineJoinStyle(objc2_app_kit::NSLineJoinStyle::Round);
                rgba(0.0, 0.0, 0.0, alpha).setStroke();
                check.stroke();
            }
        }
        _ => {
            // Three quiet dots: the bar has something to say, the page loads.
            let dot = 4.0;
            let gap = 5.0;
            let total = 3.0 * dot + 2.0 * gap;
            let mut x = x0 + (w - total) / 2.0;
            let y = mid_y - dot / 2.0;
            rgba(255.0, 255.0, 255.0, 0.85 * alpha).setFill();
            for _ in 0..3 {
                let r = NSRect::new(NSPoint::new(x, y), NSSize::new(dot, dot));
                NSBezierPath::bezierPathWithOvalInRect(r).fill();
                x += dot + gap;
            }
        }
    }
}

/// Waveform bar width and gap (pt), as in `pill.css`.
const WAVE_BAR: f64 = 2.0;
const WAVE_GAP: f64 = 2.5;
/// Hands-free ✕ / ✓ disc diameter and inset from the capsule's ends.
const BUTTON_D: f64 = 18.0;
const BUTTON_INSET: f64 = 6.0;

/// Wall-clock milliseconds (the clock the web view's morph shares).
fn now_ms() -> f64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs_f64() * 1000.0)
        .unwrap_or(0.0)
}

/// macOS Reduce Motion: sizes change at once.
fn reduce_motion() -> bool {
    // SAFETY: plain class and property getters on NSWorkspace.
    unsafe {
        let ws: *mut AnyObject = msg_send![objc2::class!(NSWorkspace), sharedWorkspace];
        if ws.is_null() {
            return false;
        }
        msg_send![ws, accessibilityDisplayShouldReduceMotion]
    }
}

fn slot_button(
    mtm: MainThreadMarker,
    target: &RestingView,
    action: objc2::runtime::Sel,
) -> Retained<NSButton> {
    // SAFETY: NSButton's frame initializer and ordinary control setters.
    let button: Retained<NSButton> =
        unsafe { msg_send![NSButton::alloc(mtm), initWithFrame: NSRect::ZERO] };
    button.setBordered(false);
    // SAFETY: medium system font and a light title over the dark capsule.
    button.setFont(Some(unsafe {
        &NSFont::systemFontOfSize_weight(SLOT_FONT, NSFontWeightMedium)
    }));
    // SAFETY: target and action use the RestingView methods defined above;
    // the view owns these buttons and outlives their callbacks.
    unsafe {
        button.setTarget(Some(target));
        button.setAction(Some(action));
        button.setContentTintColor(Some(&NSColor::whiteColor()));
    }
    button.setHidden(true);
    button
}

/// The grouped bar's two labels share a 184 pt capsule: a point smaller
/// than the bar's 15 pt text so longer translations fit.
const SLOT_FONT: f64 = 14.0;

fn rgba(r: f64, g: f64, b: f64, a: f64) -> Retained<NSColor> {
    NSColor::colorWithSRGBRed_green_blue_alpha(r / 255.0, g / 255.0, b / 255.0, a)
}

fn text_attributes(alpha: f64) -> Retained<NSDictionary<NSString, AnyObject>> {
    // SAFETY: plain class methods and framework constants.
    let font = unsafe { NSFont::systemFontOfSize_weight(geometry::HOVER_FONT, NSFontWeightMedium) };
    let color = NSColor::colorWithSRGBRed_green_blue_alpha(1.0, 1.0, 1.0, alpha);
    let keys: [&NSString; 2] = unsafe { [NSFontAttributeName, NSForegroundColorAttributeName] };
    let values: [&AnyObject; 2] = [&font, &color];
    NSDictionary::from_slices(&keys, &values)
}

fn text_size(text: &str) -> NSSize {
    let s = NSString::from_str(text);
    let attrs = text_attributes(1.0);
    // SAFETY: `-[NSString sizeWithAttributes:]` (NSStringDrawing).
    unsafe { msg_send![&*s, sizeWithAttributes: &*attrs] }
}

fn draw_text(text: &str, at: NSPoint, alpha: f64) {
    let s = NSString::from_str(text);
    let attrs = text_attributes(alpha);
    // SAFETY: `-[NSString drawAtPoint:withAttributes:]` inside `drawRect:`.
    unsafe {
        let _: () = msg_send![&*s, drawAtPoint: at, withAttributes: &*attrs];
    }
}

/// Keeps one of the bar's panels out of the app's hide (⌘H, `NSApp.hide`).
///
/// AppKit unhides a hidden app when one of its windows that can hide is
/// ordered in, `orderFrontRegardless` on a non-activating panel included,
/// and the unhide brings every window back, the Hub too (Mac wave 3,
/// S-28.1d-f: a take started while the app was hidden showed the Hub 2 s
/// in). A panel with `canHide` off stays on screen while the app is hidden
/// and ordering it front leaves the app hidden. Call it on every panel the
/// take path orders in (the resting bar, the `fork-pill` web view, the
/// tutorial spotlight), before its first order-in.
pub(super) fn stay_when_app_hidden(window: &objc2_app_kit::NSWindow) {
    window.setCanHide(false);
}

/// The resting panel and its view.
pub struct Resting {
    panel: Retained<RestingPanel>,
    view: Retained<RestingView>,
}

impl Resting {
    /// Builds the panel (hidden). Main thread only.
    pub fn new(mtm: MainThreadMarker, on_event: impl Fn(NativeEvent) + 'static) -> Resting {
        let (w, h) = Shape::Rest.size();
        let rect = NSRect::new(NSPoint::new(0.0, 0.0), NSSize::new(w, h));
        let style = NSWindowStyleMask::Borderless | NSWindowStyleMask::NonactivatingPanel;
        let this = RestingPanel::alloc(mtm).set_ivars(());
        // SAFETY: NSPanel's designated initializer.
        let panel: Retained<RestingPanel> = unsafe {
            msg_send![super(this), initWithContentRect: rect, styleMask: style, backing: NSBackingStoreType::Buffered, defer: false]
        };
        // SAFETY: plain setters on a window we own.
        unsafe {
            panel.setReleasedWhenClosed(false);
        }
        panel.setFloatingPanel(true);
        panel.setBecomesKeyOnlyIfNeeded(true);
        panel.setHidesOnDeactivate(false);
        stay_when_app_hidden(&panel);
        panel.setLevel(NSStatusWindowLevel);
        panel.setCollectionBehavior(
            NSWindowCollectionBehavior::CanJoinAllSpaces
                | NSWindowCollectionBehavior::FullScreenAuxiliary
                | NSWindowCollectionBehavior::Stationary
                | NSWindowCollectionBehavior::IgnoresCycle,
        );
        panel.setOpaque(false);
        panel.setBackgroundColor(Some(&NSColor::clearColor()));
        panel.setHasShadow(false);
        panel.setMovable(false);
        panel.setAnimationBehavior(NSWindowAnimationBehavior::None);
        let view = RestingView::new(mtm, rect, Box::new(on_event));
        panel.setContentView(Some(&view));
        Resting { panel, view }
    }

    pub fn set_texts(&self, texts: &Texts) {
        if let Some(buttons) = self.view.ivars().slot_buttons.borrow().as_ref() {
            for (button, title, help) in [
                (&buttons.0, &texts.dictate, &texts.dictate_hint),
                (&buttons.1, &texts.scratchpad, &texts.scratchpad_hint),
            ] {
                button.setTitle(&NSString::from_str(title));
                // Native tooltips also become each button's accessibility help.
                button.setToolTip(Some(&NSString::from_str(help)));
            }
        }
        if *self.view.ivars().texts.borrow() != *texts {
            *self.view.ivars().texts.borrow_mut() = texts.clone();
            self.view.relayout();
        }
    }

    pub fn set_scratchpad_enabled(&self, enabled: bool) {
        if self.view.ivars().scratchpad_enabled.replace(enabled) != enabled {
            self.view.ivars().hover_slot.set(Slot::Dictate);
            self.view.ivars().hover.set(false);
            self.view.layout_slot_buttons();
            self.view.relayout();
        }
    }

    /// Places the bar (`screen` is the display it is on).
    pub fn set_anchor(&self, anchor: Anchor, screen: &Screen) {
        // A drag in progress owns the position.
        if self
            .view
            .ivars()
            .drag
            .borrow()
            .as_ref()
            .is_some_and(|d| d.moved)
        {
            return;
        }
        *self.view.ivars().screen.borrow_mut() = Some(screen.clone());
        if self.view.ivars().anchor.get() != Some(anchor) {
            self.view.ivars().anchor.set(Some(anchor));
            self.view.relayout();
        }
    }

    /// The take's appearance time (Unix ms), for the bridge's grow.
    pub fn set_grown_at(&self, at: Option<u64>) {
        self.view.ivars().grown_at.set(at);
    }

    /// The bar comes back after the web view showed a `capsule`-sized
    /// capsule: the next morph starts from that size (the take's end
    /// shrinks back into the resting line).
    pub fn morph_from(&self, capsule: (f64, f64)) {
        if !self.panel.isVisible() {
            self.view.ivars().morph.set(None);
            self.view.ivars().shown.set(capsule);
            self.view.ivars().target.set(capsule);
        }
    }

    pub fn set_mode(&self, mode: Mode) {
        if self.view.ivars().mode.get() != mode {
            if matches!(mode, Mode::Bridge(_)) && !self.panel.isVisible() {
                // A bar that was hidden grows from the resting line too.
                self.view.ivars().morph.set(None);
                self.view.ivars().shown.set(Shape::Rest.size());
                self.view.ivars().target.set(Shape::Rest.size());
            }
            self.view.ivars().mode.set(mode);
            if mode != Mode::Rest {
                self.view.ivars().hover.set(false);
                *self.view.ivars().drag.borrow_mut() = None;
            }
            self.view.relayout();
        }
    }

    pub fn show(&self) {
        if !self.panel.isVisible() {
            self.panel.orderFrontRegardless();
            crate::fork::startup::mark(crate::fork::startup::Milestone::Bar);
        }
    }

    pub fn hide(&self) {
        if self.panel.isVisible() {
            self.panel.orderOut(None);
        }
    }

    pub fn is_visible(&self) -> bool {
        self.panel.isVisible()
    }

    /// Brings the panel back in front (Space changed).
    pub fn reassert(&self) {
        if self.panel.isVisible() {
            self.panel.orderFrontRegardless();
        }
    }

    /// Hides the panel from screen capture (or shows it again).
    pub fn set_protected(&self, protected: bool) {
        self.panel.setSharingType(if protected {
            NSWindowSharingType::None
        } else {
            NSWindowSharingType::ReadOnly
        });
    }

    /// Follows the app theme (`None` = the system's).
    pub fn set_appearance(&self, dark: Option<bool>) {
        let appearance = dark.and_then(|d| {
            let name = if d {
                unsafe { NSAppearanceNameDarkAqua }
            } else {
                unsafe { NSAppearanceNameAqua }
            };
            NSAppearance::appearanceNamed(name)
        });
        // SAFETY: NSAppearanceCustomization setter.
        unsafe {
            let _: () = msg_send![&*self.panel, setAppearance: appearance.as_deref()];
        }
    }

    /// The panel's window number (for tests and the debug log).
    pub fn window_number(&self) -> isize {
        self.panel.windowNumber()
    }
}

/// Every display, for placement. Main thread only.
pub fn screens(mtm: MainThreadMarker) -> Vec<Screen> {
    NSScreen::screens(mtm)
        .iter()
        .map(|s| {
            let f = s.frame();
            let v = s.visibleFrame();
            let frame = Rect::new(f.origin.x, f.origin.y, f.size.width, f.size.height);
            Screen {
                key: placement::display_key(&s.localizedName().to_string(), &frame),
                frame,
                visible: Rect::new(v.origin.x, v.origin.y, v.size.width, v.size.height),
                scale: s.backingScaleFactor(),
            }
        })
        .collect()
}

/// The cursor in global AppKit coordinates.
pub fn cursor() -> (f64, f64) {
    let p = NSEvent::mouseLocation();
    (p.x, p.y)
}
