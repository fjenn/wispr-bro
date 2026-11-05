// SPDX-License-Identifier: GPL-3.0-or-later
//! A click outside the bar dismisses its open hint (rc9 owner report;
//! macOS, main thread).
//!
//! The bar's hints (the hover bubble and the Scratchpad tooltip) used to go
//! away only with Esc (the S-30 spotlight is not one of them: it asks the
//! user to click where they want to write, so a click never ends it). While one is open an
//! `NSEvent` mouse-down monitor (left and right button) watches for a click:
//! a global monitor for clicks in other apps, a local one for clicks in our
//! own windows (the Hub, the note). A click inside the bar group (the bar,
//! the gap, the Scratchpad circle and the take's web view) is the bar's own
//! and dismisses nothing. Monitors only observe: the click reaches its
//! target unchanged, and no window takes focus. They are installed when the
//! first hint opens and removed when the last one closes.
//!
//! Global mouse monitors need no permission (only key events need
//! Accessibility).

use std::cell::{Cell, RefCell};
use std::ptr::NonNull;
use std::rc::Rc;

use block2::RcBlock;
use objc2::rc::Retained;
use objc2::runtime::AnyObject;
use objc2_app_kit::{NSEvent, NSEventMask};

/// A hint that a click outside dismisses.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Hint {
    /// The bar's hover bubble and the Scratchpad circle's tooltip.
    Hover,
}

type Dismiss = Rc<dyn Fn()>;
type Inside = Rc<dyn Fn(f64, f64) -> bool>;

#[derive(Default)]
struct State {
    open: Vec<(Hint, Dismiss)>,
    /// Whether a global point is inside the bar group.
    inside: Option<Inside>,
    monitors: Vec<Retained<AnyObject>>,
}

thread_local! {
    static STATE: RefCell<State> = RefCell::new(State::default());
    /// A monitor's handler runs: removal waits until it returns.
    static IN_HANDLER: Cell<bool> = const { Cell::new(false) };
}

/// Where the bar group is (`native.rs` registers it once).
pub fn set_inside(inside: impl Fn(f64, f64) -> bool + 'static) {
    STATE.with(|s| s.borrow_mut().inside = Some(Rc::new(inside)));
}

/// `hint` is on screen; `dismiss` takes it away.
pub fn open(hint: Hint, dismiss: impl Fn() + 'static) {
    STATE.with(|s| {
        let mut s = s.borrow_mut();
        s.open.retain(|(h, _)| *h != hint);
        s.open.push((hint, Rc::new(dismiss)));
    });
    sync();
}

/// `hint` is gone (by any route).
pub fn close(hint: Hint) {
    let changed = STATE.with(|s| {
        let mut s = s.borrow_mut();
        let before = s.open.len();
        s.open.retain(|(h, _)| *h != hint);
        before != s.open.len()
    });
    if changed {
        sync();
    }
}

/// Installs the monitors while a hint is open, removes them otherwise.
fn sync() {
    if IN_HANDLER.with(Cell::get) {
        return; // `on_click` syncs when the handler is done
    }
    let (want, have) = STATE.with(|s| {
        let s = s.borrow();
        (!s.open.is_empty(), !s.monitors.is_empty())
    });
    if want && !have {
        install();
    } else if !want && have {
        let monitors = STATE.with(|s| std::mem::take(&mut s.borrow_mut().monitors));
        for m in monitors {
            // SAFETY: each token came from `addGlobal/LocalMonitor…` and is
            // removed once.
            unsafe { NSEvent::removeMonitor(&m) };
        }
    }
}

fn mask() -> NSEventMask {
    NSEventMask::LeftMouseDown | NSEventMask::RightMouseDown
}

fn install() {
    let mut monitors = Vec::new();
    let global = RcBlock::new(|_event: NonNull<NSEvent>| on_click());
    if let Some(m) = NSEvent::addGlobalMonitorForEventsMatchingMask_handler(mask(), &global) {
        monitors.push(m);
    }
    let local = RcBlock::new(|event: NonNull<NSEvent>| -> *mut NSEvent {
        on_click();
        event.as_ptr() // never swallowed
    });
    // SAFETY: the block returns the event it was given, a valid pointer.
    if let Some(m) =
        unsafe { NSEvent::addLocalMonitorForEventsMatchingMask_handler(mask(), &local) }
    {
        monitors.push(m);
    }
    if monitors.is_empty() {
        log::warn!("pill: no mouse monitor for dismissing hints");
    }
    STATE.with(|s| s.borrow_mut().monitors = monitors);
}

/// A mouse-down anywhere: outside the bar group, every open hint closes.
fn on_click() {
    let p = NSEvent::mouseLocation();
    let (inside, open) = STATE.with(|s| {
        let s = s.borrow();
        (
            s.inside.clone(),
            s.open.iter().map(|(_, d)| d.clone()).collect::<Vec<_>>(),
        )
    });
    if inside.is_some_and(|f| f(p.x, p.y)) {
        return;
    }
    IN_HANDLER.with(|h| h.set(true));
    for dismiss in open {
        dismiss();
    }
    IN_HANDLER.with(|h| h.set(false));
    // Removing a monitor from inside its own handler is left to the run
    // loop's next turn: AppKit still holds the running block then.
    if STATE.with(|s| s.borrow().open.is_empty()) {
        dispatch_sync_later();
    }
}

/// Runs [`sync`] on the main queue's next turn.
fn dispatch_sync_later() {
    let block = RcBlock::new(sync);
    // SAFETY: `-[NSOperationQueue addOperationWithBlock:]` on the main
    // queue; the block is copied by the queue.
    unsafe {
        let queue: *mut AnyObject = objc2::msg_send![objc2::class!(NSOperationQueue), mainQueue];
        if !queue.is_null() {
            let _: () = objc2::msg_send![queue, addOperationWithBlock: &*block];
        }
    }
}
