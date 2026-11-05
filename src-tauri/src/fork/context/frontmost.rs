// SPDX-License-Identifier: GPL-3.0-or-later
//! The frontmost app at record start (D-21): bundle id and localized name,
//! from `NSWorkspace.frontmostApplication` (objc2-app-kit, in tree).
//!
//! RS-04 (macOS 26.6.2): the system-wide `AXFocusedApplication` failed on
//! every call, while NSWorkspace's answer was right 40-206 ms after an app
//! switch, so NSWorkspace is the only source. The call is cheap and safe off
//! the main thread (S-04 calls it from the paste thread too).

/// One app, as the history row stores it.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct FrontApp {
    pub bundle_id: Option<String>,
    pub name: Option<String>,
}

/// The frontmost app now, or `None` when there is none (login window) or on
/// another platform.
#[cfg(target_os = "macos")]
pub fn frontmost() -> Option<FrontApp> {
    use objc2_app_kit::NSWorkspace;
    let app = NSWorkspace::sharedWorkspace().frontmostApplication()?;
    let bundle_id = app.bundleIdentifier().map(|s| s.to_string());
    let name = app.localizedName().map(|s| s.to_string());
    if bundle_id.is_none() && name.is_none() {
        return None;
    }
    Some(FrontApp { bundle_id, name })
}

#[cfg(not(target_os = "macos"))]
pub fn frontmost() -> Option<FrontApp> {
    None
}

#[cfg(all(test, target_os = "macos"))]
mod tests {
    #[test]
    fn frontmost_does_not_panic_off_the_main_thread() {
        // Under `cargo test` there may be no GUI session; any answer is fine,
        // as long as the call returns.
        let handle = std::thread::spawn(super::frontmost);
        let _ = handle.join().expect("frontmost() panicked");
    }
}
