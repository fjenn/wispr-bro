// SPDX-License-Identifier: GPL-3.0-or-later
//
//! The session settings the app, the Swift session and the keyboard share:
//! the App Group defaults suite [`SUITE`] (the Swift side reads the same
//! keys, `WBBridgeKey` in `ios-native/WBShared/WBBridge.swift`).

use objc2::AnyThread;
use objc2_foundation::{NSString, NSUserDefaults};

pub const SUITE: &str = "group.com.wisprbro.ios";
/// Int: seconds until an idle session ends; 0 = immediately, -1 = never.
pub const TIMEOUT_KEY: &str = "wb.session.timeoutSeconds";
pub const TIMEOUT_DEFAULT: i64 = 300;
/// Bool: go back to the previous app after a session starts.
pub const AUTO_RETURN_KEY: &str = "wb.session.autoReturn";
pub const AUTO_RETURN_DEFAULT: bool = true;

fn suite() -> Option<objc2::rc::Retained<NSUserDefaults>> {
    NSUserDefaults::initWithSuiteName(NSUserDefaults::alloc(), Some(&NSString::from_str(SUITE)))
}

fn is_set(defaults: &NSUserDefaults, key: &NSString) -> bool {
    defaults.objectForKey(key).is_some()
}

pub fn session_timeout() -> i64 {
    let key = NSString::from_str(TIMEOUT_KEY);
    match suite() {
        Some(defaults) if is_set(&defaults, &key) => defaults.integerForKey(&key) as i64,
        _ => TIMEOUT_DEFAULT,
    }
}

pub fn set_session_timeout(seconds: i64) -> Result<(), String> {
    let defaults = suite().ok_or("The shared settings are not available")?;
    defaults.setInteger_forKey(seconds.max(-1) as isize, &NSString::from_str(TIMEOUT_KEY));
    Ok(())
}

pub fn auto_return() -> bool {
    let key = NSString::from_str(AUTO_RETURN_KEY);
    match suite() {
        Some(defaults) if is_set(&defaults, &key) => defaults.boolForKey(&key),
        _ => AUTO_RETURN_DEFAULT,
    }
}

pub fn set_auto_return(enabled: bool) -> Result<(), String> {
    let defaults = suite().ok_or("The shared settings are not available")?;
    defaults.setBool_forKey(enabled, &NSString::from_str(AUTO_RETURN_KEY));
    Ok(())
}
