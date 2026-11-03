// SPDX-License-Identifier: GPL-3.0-or-later
//! Builds the bar's web view panels without wry activating the app (macOS).
//!
//! Mac wave 4, S-28.8 (1h/1i): every top-level web view wry 0.55 creates
//! ends with `NSApplication::activate()` (`activateIgnoringOtherApps:`
//! before macOS 14), "so the window is always on top". On a hidden app
//! (⌘H) that call does not activate it, but AppKit unhides it about 0.5 s
//! later and every window comes back, the Hub included. `canHide` off on
//! the panel does not help: the unhide comes from the activation request,
//! not from ordering the panel in. Reproduced off screen with a tiny
//! AppKit program (hidden app, `activate()` → `isHidden` false after 0.5 s;
//! the same panel and WKWebView without the call → stays hidden).
//!
//! [`without_app_activation`] turns both NSApplication methods into no-ops
//! for the length of one synchronous build on the main thread and restores
//! them before it returns (also on unwind). The bar's panels are
//! non-activating and never key, so wry's activation is never wanted for
//! them; wry's other windows (the Hub, the Scratchpad) are built outside
//! this guard and still activate the app as before.

use objc2::runtime::{AnyClass, AnyObject, Bool, Imp, Method, Sel};
use objc2::{sel, ClassType, MainThreadMarker};
use objc2_app_kit::NSApplication;

unsafe extern "C-unwind" fn no_activate(_this: *mut AnyObject, _cmd: Sel) {}

unsafe extern "C-unwind" fn no_activate_ignoring(_this: *mut AnyObject, _cmd: Sel, _flag: Bool) {}

/// The original implementations, restored on drop.
struct Restore(Vec<(&'static Method, Imp)>);

impl Drop for Restore {
    fn drop(&mut self) {
        for (method, imp) in self.0.drain(..).rev() {
            // SAFETY: puts back the implementation this guard replaced.
            unsafe {
                method.set_implementation(imp);
            }
        }
    }
}

fn method(class: &'static AnyClass, sel: Sel) -> Option<&'static Method> {
    class.instance_method(sel)
}

/// Runs `build` (one synchronous web view build on the main thread) with
/// `-[NSApplication activate]` and `-[NSApplication
/// activateIgnoringOtherApps:]` doing nothing.
pub(super) fn without_app_activation<T>(_mtm: MainThreadMarker, build: impl FnOnce() -> T) -> T {
    let class = NSApplication::class();
    let mut restore = Restore(Vec::new());
    let noops: [(Sel, Imp); 2] = [
        // SAFETY: both functions have the selectors' signatures (receiver,
        // selector, and a BOOL for the second); `Imp` is the untyped form.
        (sel!(activate), unsafe {
            std::mem::transmute::<unsafe extern "C-unwind" fn(*mut AnyObject, Sel), Imp>(
                no_activate,
            )
        }),
        (sel!(activateIgnoringOtherApps:), unsafe {
            std::mem::transmute::<unsafe extern "C-unwind" fn(*mut AnyObject, Sel, Bool), Imp>(
                no_activate_ignoring,
            )
        }),
    ];
    for (sel, noop) in noops {
        // `activate` exists from macOS 14 on.
        if let Some(m) = method(class, sel) {
            // SAFETY: same signature (above); main thread only (`_mtm`), and
            // `restore` puts the original back before this returns.
            let old = unsafe { m.set_implementation(noop) };
            restore.0.push((m, old));
        }
    }
    build()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Both tests swap process-wide method pointers; run them one at a time.
    static SERIAL: std::sync::Mutex<()> = std::sync::Mutex::new(());

    fn imp(sel: Sel) -> Option<usize> {
        method(NSApplication::class(), sel).map(|m| m.implementation() as usize)
    }

    /// The guard swaps both methods only while the build runs and puts the
    /// originals back afterwards, also when the build panics.
    #[test]
    fn the_guard_restores_app_activation_after_the_build() {
        let _serial = SERIAL.lock().unwrap_or_else(|e| e.into_inner());
        let before = (imp(sel!(activate)), imp(sel!(activateIgnoringOtherApps:)));
        assert!(before.1.is_some(), "activateIgnoringOtherApps: exists");
        // SAFETY: test-only; `cargo test` runs this off the main thread, but
        // the guard only swaps method pointers and calls no AppKit UI.
        let mtm = unsafe { MainThreadMarker::new_unchecked() };
        let inside = without_app_activation(mtm, || {
            (imp(sel!(activate)), imp(sel!(activateIgnoringOtherApps:)))
        });
        assert_ne!(inside.1, before.1, "swapped during the build");
        if before.0.is_some() {
            assert_ne!(inside.0, before.0);
        }
        let after = (imp(sel!(activate)), imp(sel!(activateIgnoringOtherApps:)));
        assert_eq!(after, before, "restored after the build");

        let panicked = std::panic::catch_unwind(|| {
            without_app_activation(mtm, || panic!("build failed"));
        });
        assert!(panicked.is_err());
        let after = (imp(sel!(activate)), imp(sel!(activateIgnoringOtherApps:)));
        assert_eq!(after, before, "restored after a panicking build");
    }

    /// Inside the guard the installed implementations are the no-ops.
    #[test]
    fn inside_the_guard_activation_is_a_no_op() {
        let _serial = SERIAL.lock().unwrap_or_else(|e| e.into_inner());
        // SAFETY: test-only, see above.
        let mtm = unsafe { MainThreadMarker::new_unchecked() };
        without_app_activation(mtm, || {
            assert_eq!(
                imp(sel!(activateIgnoringOtherApps:)),
                Some(no_activate_ignoring as *const () as usize)
            );
            if let Some(i) = imp(sel!(activate)) {
                assert_eq!(i, no_activate as *const () as usize);
            }
        });
    }
}
