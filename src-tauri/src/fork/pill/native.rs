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
//! DEC-28). Its one view draws a thin capsule; on hover (a native tracking
//! area, which fires while another app is active: RS-03) it grows into a
//! bubble with the localized hint. A Dictate click starts hands-free
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
//! Announcements for takes come from `panel::announce`.

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
    on_event: Callback,
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
        for (button, x) in [(&buttons.0, 0.0), (&buttons.1, half)] {
            button.setFrame(NSRect::new(
                NSPoint::new(x, 0.0),
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

    /// The size this view wants now (points).
    fn current_size(&self) -> (f64, f64) {
        match self.ivars().mode.get() {
            Mode::Bridge(shape) => shape.size(),
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

    /// Resizes the window around the anchor and redraws.
    fn relayout(&self) {
        let Some(anchor) = self.ivars().anchor.get() else {
            return;
        };
        let r = placement::frame(&anchor, self.current_size());
        if let Some(window) = self.window() {
            let target = NSRect::new(NSPoint::new(r.x, r.y), NSSize::new(r.w, r.h));
            if window.frame() != target {
                window.setFrame_display(target, true);
            }
        }
        self.setNeedsDisplay(true);
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
        let bounds = self.bounds();
        let dark = self.is_dark();
        let rect = NSRect::new(
            NSPoint::new(bounds.origin.x + 0.5, bounds.origin.y + 0.5),
            NSSize::new(bounds.size.width - 1.0, bounds.size.height - 1.0),
        );
        let radius = rect.size.height / 2.0;
        let path = NSBezierPath::bezierPathWithRoundedRect_xRadius_yRadius(rect, radius, radius);
        // A dark capsule in both themes (it sits over any app); in dark mode
        // a lighter fill and a stronger edge keep it visible on dark windows.
        let (fill, edge) = if dark {
            (
                rgba(44.0, 44.0, 48.0, 0.94),
                rgba(255.0, 255.0, 255.0, 0.32),
            )
        } else {
            (
                rgba(24.0, 24.0, 27.0, 0.88),
                rgba(255.0, 255.0, 255.0, 0.55),
            )
        };
        fill.setFill();
        path.fill();
        edge.setStroke();
        path.setLineWidth(1.0);
        path.stroke();

        match self.ivars().mode.get() {
            Mode::Rest if self.ivars().hover.get() => {
                let hint = self.hover_text();
                let size = text_size(&hint);
                let x = bounds.origin.x + (bounds.size.width - size.width) / 2.0;
                let y = bounds.origin.y + (bounds.size.height - size.height) / 2.0;
                draw_text(&hint, NSPoint::new(x.max(geometry::HOVER_PAD_X / 2.0), y));
            }
            Mode::Rest if self.ivars().scratchpad_enabled.get() => {
                rgba(255.0, 255.0, 255.0, 0.24).setFill();
                NSBezierPath::bezierPathWithRect(NSRect::new(
                    NSPoint::new(bounds.size.width / 2.0, 7.0),
                    NSSize::new(1.0, bounds.size.height - 14.0),
                ))
                .fill();
            }
            Mode::Bridge(_) => {
                // Three quiet dots: the take has started, the waveform loads.
                let dot = 4.0;
                let gap = 5.0;
                let total = 3.0 * dot + 2.0 * gap;
                let mut x = bounds.origin.x + (bounds.size.width - total) / 2.0;
                let y = bounds.origin.y + (bounds.size.height - dot) / 2.0;
                rgba(255.0, 255.0, 255.0, 0.85).setFill();
                for _ in 0..3 {
                    let r = NSRect::new(NSPoint::new(x, y), NSSize::new(dot, dot));
                    NSBezierPath::bezierPathWithOvalInRect(r).fill();
                    x += dot + gap;
                }
            }
            Mode::Rest => {}
        }
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
        &NSFont::systemFontOfSize_weight(geometry::HOVER_FONT, NSFontWeightMedium)
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

fn rgba(r: f64, g: f64, b: f64, a: f64) -> Retained<NSColor> {
    NSColor::colorWithSRGBRed_green_blue_alpha(r / 255.0, g / 255.0, b / 255.0, a)
}

fn text_attributes() -> Retained<NSDictionary<NSString, AnyObject>> {
    // SAFETY: plain class methods and framework constants.
    let font = unsafe { NSFont::systemFontOfSize_weight(geometry::HOVER_FONT, NSFontWeightMedium) };
    let color = NSColor::colorWithSRGBRed_green_blue_alpha(1.0, 1.0, 1.0, 0.96);
    let keys: [&NSString; 2] = unsafe { [NSFontAttributeName, NSForegroundColorAttributeName] };
    let values: [&AnyObject; 2] = [&font, &color];
    NSDictionary::from_slices(&keys, &values)
}

fn text_size(text: &str) -> NSSize {
    let s = NSString::from_str(text);
    let attrs = text_attributes();
    // SAFETY: `-[NSString sizeWithAttributes:]` (NSStringDrawing).
    unsafe { msg_send![&*s, sizeWithAttributes: &*attrs] }
}

fn draw_text(text: &str, at: NSPoint) {
    let s = NSString::from_str(text);
    let attrs = text_attributes();
    // SAFETY: `-[NSString drawAtPoint:withAttributes:]` inside `drawRect:`.
    unsafe {
        let _: () = msg_send![&*s, drawAtPoint: at, withAttributes: &*attrs];
    }
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

    pub fn set_mode(&self, mode: Mode) {
        if self.view.ivars().mode.get() != mode {
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

/// Posts a VoiceOver announcement (`NSAccessibilityAnnouncementRequested`).
pub fn announce(element: &AnyObject, text: &str, high: bool) {
    use objc2_app_kit::{
        NSAccessibilityAnnouncementKey, NSAccessibilityAnnouncementRequestedNotification,
        NSAccessibilityPostNotificationWithUserInfo, NSAccessibilityPriorityKey,
        NSAccessibilityPriorityLevel,
    };
    use objc2_foundation::NSNumber;
    let message = NSString::from_str(text);
    let priority = NSNumber::new_isize(if high {
        NSAccessibilityPriorityLevel::High.0
    } else {
        NSAccessibilityPriorityLevel::Medium.0
    });
    // SAFETY: framework constants; the dictionary holds an NSString and an
    // NSNumber as AppKit documents for these keys.
    unsafe {
        let keys: [&NSString; 2] = [NSAccessibilityAnnouncementKey, NSAccessibilityPriorityKey];
        let values: [&AnyObject; 2] = [&message, &priority];
        let info = NSDictionary::from_slices(&keys, &values);
        NSAccessibilityPostNotificationWithUserInfo(
            element,
            NSAccessibilityAnnouncementRequestedNotification,
            Some(&info),
        );
    }
}
