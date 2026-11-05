// SPDX-License-Identifier: GPL-3.0-or-later
//! No window restoration (Mac wave 2 finding 1).
//!
//! After a crash, AppKit's state restoration asks "The last time you opened
//! Wispr Bro, it unexpectedly quit while reopening windows. Do you want to
//! try to reopen its windows again?" from inside `finishLaunching`
//! (`NSPersistentUIRestorer … NSAlert runModal`). Until someone answers,
//! startup stops: no bar, no hotkeys, no crash notice, and an app that
//! starts hidden at login shows nothing else. Wispr Bro opens its own
//! windows (the Hub where it was, S-38), so it never needs AppKit's.
//!
//! [`opt_out`] runs from `fork::early_init` (before Tauri creates
//! `NSApplication`) and registers two defaults in the registration domain
//! (never written to disk): `ApplePersistenceIgnoreState` = YES, so saved
//! state, and with it the crash prompt, is ignored, and
//! `NSQuitAlwaysKeepsWindows` = NO. [`not_restorable`] also marks the Hub
//! window non-restorable, so nothing new is saved for it. No Handy file and
//! no Info.plist key: there is no plist key for this.

/// The registration-domain defaults [`opt_out`] sets.
pub const DEFAULTS: &[(&str, bool)] = &[
    ("ApplePersistenceIgnoreState", true),
    ("NSQuitAlwaysKeepsWindows", false),
];

/// Registers [`DEFAULTS`]. Idempotent; call before `NSApplication` starts.
pub fn opt_out() {
    #[cfg(target_os = "macos")]
    {
        use objc2::rc::Retained;
        use objc2::runtime::AnyObject;
        use objc2_foundation::{NSDictionary, NSNumber, NSString, NSUserDefaults};

        let keys: Vec<Retained<NSString>> = DEFAULTS
            .iter()
            .map(|(k, _)| NSString::from_str(k))
            .collect();
        let values: Vec<Retained<AnyObject>> = DEFAULTS
            .iter()
            .map(|(_, v)| {
                Retained::into_super(Retained::into_super(Retained::into_super(
                    NSNumber::new_bool(*v),
                )))
            })
            .collect();
        let key_refs: Vec<&NSString> = keys.iter().map(|k| &**k).collect();
        let value_refs: Vec<&AnyObject> = values.iter().map(|v| &**v).collect();
        let dict = NSDictionary::from_slices(&key_refs, &value_refs);
        // SAFETY: every key is an NSString and every value an NSNumber,
        // property-list types as `registerDefaults:` requires.
        unsafe { NSUserDefaults::standardUserDefaults().registerDefaults(&dict) };
    }
}

/// `true` when the defaults read back as [`opt_out`] set them (the
/// start-up self-check logs it; elsewhere than macOS there is nothing to
/// restore).
pub fn opted_out() -> bool {
    #[cfg(target_os = "macos")]
    {
        use objc2_foundation::{NSString, NSUserDefaults};
        let defaults = NSUserDefaults::standardUserDefaults();
        // `NSQuitAlwaysKeepsWindows` follows the user's "Close windows when
        // quitting an application" (global domain), which outranks the
        // registration domain; the ignore flag is what stops the prompt.
        defaults.boolForKey(&NSString::from_str(DEFAULTS[0].0))
    }
    #[cfg(not(target_os = "macos"))]
    {
        true
    }
}

/// Marks a window non-restorable. Main thread.
#[cfg(target_os = "macos")]
pub fn not_restorable(window: &objc2_app_kit::NSWindow) {
    window.setRestorable(false);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_crash_prompt_is_switched_off_before_launch() {
        // Registration domain only: nothing is written to the test
        // binary's (or anyone's) preferences.
        opt_out();
        opt_out();
        assert!(opted_out());
        #[cfg(target_os = "macos")]
        {
            use objc2_foundation::{NSString, NSUserDefaults};
            let d = NSUserDefaults::standardUserDefaults();
            assert!(d.boolForKey(&NSString::from_str("ApplePersistenceIgnoreState")));
        }
    }

    #[test]
    fn early_init_opts_out() {
        // A guard against the call being dropped in a refactor: early_init
        // is the only place that runs before NSApplication exists.
        let src = include_str!("mod.rs");
        let body = src
            .split("pub fn early_init()")
            .nth(1)
            .and_then(|rest| rest.split("\n}\n").next())
            .expect("early_init");
        assert!(
            body.contains("restore::opt_out()"),
            "early_init must call restore::opt_out()"
        );
    }
}
