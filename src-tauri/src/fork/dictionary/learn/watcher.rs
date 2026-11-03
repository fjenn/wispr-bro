// SPDX-License-Identifier: GPL-3.0-or-later
//
// The watch follows BetterWispr's `CorrectionWatcher` and the commit
// triggers TypeWhisper's `TargetAppCorrectionLearningService`:
//   Source: BetterWispr, https://github.com/opennookorg/betterwispr
//   Commit: 304335a073a98f7439e5f7b4fbc2c31b810b807e
//   File:   Sources/BetterWisprCore/Integrations/CorrectionWatcher.swift
//           (`watch`: the dictated field polled after a paste, the
//           baseline as the first read that contains the paste, the
//           100,000-character cap, Secure Input stops the watch, the
//           secure-field skip, AXManualAccessibility for Electron;
//           `SettledText`'s rule that an emptied field keeps the text it
//           held before), GPL-3.0 (adapts VoiceInk
//           c09cc1f677f40f2ee665843a61f07670210f012e, BetterWispr
//           docs/oss-reuse.md).
//   And TypeWhisper, https://github.com/TypeWhisper/typewhisper-mac
//   Commit: 2007bc99f2902a3b51cc5c06ffcf6636dcdc659d
//   File:   TypeWhisper/Services/TargetAppCorrectionLearningService.swift
//           (the commit signals: Return, keypad Enter, Tab, a focus change
//           or another app in front; nothing learned without a commit
//           before the time-out), Copyright (c) 2026 TypeWhisper,
//           GPL-3.0-or-later. Idea only; no code copied.
//   Modified by Wispr Bro contributors, 2026-11-02: ported to Rust on the
//   S-05 AX thread and lease (BetterWispr never restores
//   AXManualAccessibility); polled every 250 ms for at most 30 s; learning
//   happens only at a commit (R5 S-34), not when the text settles; the
//   S-20 exclusions (password managers, finance, terminals, own app,
//   secure, numeric and sensitive fields, browser address bars, banking
//   sites) apply before anything is read.
//
//! The watch of one pasted dictation (S-34).
//!
//! [`Session`] is pure: it is fed one [`Observation`] per poll (and
//! whether a commit key was pressed since the last one) and decides when
//! the watch is over and what the user changed. The macOS side
//! ([`mac`]) arms the watch on the AX thread (the focused element at the
//! paste, after the exclusions), reads it per poll and disarms it.
//!
//! The field's text lives only in a [`Session`] in memory and is dropped
//! when the watch ends; it is never logged or written anywhere.

use super::diff::{comparable, edit_of};

/// The watch ends this long after the paste (R5 S-34: "within 30 s").
pub const WINDOW_MS: u64 = 30_000;
/// How often the field is read.
pub const POLL_MS: u64 = 250;
/// The paste must show up in the field within this long.
pub const BASELINE_WAIT_MS: u64 = 3_000;
/// A field longer than this is not watched (BetterWispr).
pub const MAX_FIELD_CHARS: usize = 100_000;
/// A dictation longer than this is not watched (VoiceInk
/// `maximumPastedCharacters`).
pub const MAX_PASTED_CHARS: usize = 12_000;
/// Unreadable polls in a row that end the watch.
pub const MAX_MISSES: u32 = 8;

/// One read of the watched field.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Observation {
    /// The field's text (as AX gives it).
    Value(String),
    /// Focus left the field, or another app is in front.
    Moved,
    /// AX did not answer.
    Unreadable,
    /// Secure Event Input came on (a password field somewhere).
    SecureInput,
    /// The field grew past [`MAX_FIELD_CHARS`].
    TooLong,
}

/// What ended the watch with an edit to learn from.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Commit {
    /// Return, keypad Enter or Tab.
    Key,
    /// Focus moved to another element or app.
    Focus,
    /// The field emptied (the message was sent).
    Emptied,
}

impl Commit {
    pub fn as_str(self) -> &'static str {
        match self {
            Commit::Key => "key",
            Commit::Focus => "focus",
            Commit::Emptied => "emptied",
        }
    }
}

/// Why a watch ended without an edit.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Reason {
    /// The paste never showed up in the field.
    NotFound,
    /// No Return, Tab or focus change within [`WINDOW_MS`].
    NoCommit,
    /// Committed unchanged.
    NoEdit,
    /// The edited dictation could not be told apart from the rest.
    Ambiguous,
    Unreadable,
    TooLong,
    SecureInput,
    /// A newer paste, the setting turned off, or the app quitting.
    Cancelled,
}

impl Reason {
    pub fn as_str(self) -> &'static str {
        match self {
            Reason::NotFound => "not_found",
            Reason::NoCommit => "no_commit",
            Reason::NoEdit => "no_edit",
            Reason::Ambiguous => "ambiguous",
            Reason::Unreadable => "unreadable",
            Reason::TooLong => "too_long",
            Reason::SecureInput => "secure_input",
            Reason::Cancelled => "cancelled",
        }
    }
}

/// How a watch ended.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Outcome {
    /// The dictation as pasted and as the user left it ([`comparable`]).
    Edited {
        commit: Commit,
        original: String,
        edited: String,
    },
    Nothing(Reason),
}

/// The watch of one dictation.
#[derive(Debug)]
pub struct Session {
    inserted: String,
    baseline: Option<String>,
    last: Option<String>,
    misses: u32,
}

impl Session {
    /// A watch of pasted `text`; `None` when it is empty or too long.
    pub fn new(text: &str) -> Option<Session> {
        let inserted = comparable(text);
        if inserted.is_empty() || inserted.chars().count() > MAX_PASTED_CHARS {
            return None;
        }
        Some(Session {
            inserted,
            baseline: None,
            last: None,
            misses: 0,
        })
    }

    fn commit(&self, commit: Commit) -> Outcome {
        let (Some(base), Some(last)) = (&self.baseline, &self.last) else {
            return Outcome::Nothing(Reason::NotFound);
        };
        if base == last {
            return Outcome::Nothing(Reason::NoEdit);
        }
        match edit_of(&self.inserted, base, last) {
            None => Outcome::Nothing(Reason::Ambiguous),
            Some(e) if e.trim() == self.inserted => Outcome::Nothing(Reason::NoEdit),
            Some(e) => Outcome::Edited {
                commit,
                original: self.inserted.clone(),
                edited: e.trim().to_string(),
            },
        }
    }

    /// One poll, `elapsed_ms` after the paste; `key`: a commit key was
    /// pressed since the last poll. `Some` ends the watch.
    pub fn observe(&mut self, elapsed_ms: u64, obs: Observation, key: bool) -> Option<Outcome> {
        let watching = self.baseline.is_some();
        match obs {
            Observation::SecureInput => return Some(Outcome::Nothing(Reason::SecureInput)),
            Observation::TooLong => return Some(Outcome::Nothing(Reason::TooLong)),
            Observation::Moved => {
                return Some(if watching {
                    self.commit(Commit::Focus)
                } else {
                    Outcome::Nothing(Reason::NotFound)
                });
            }
            Observation::Unreadable => {
                self.misses += 1;
                if key && watching {
                    return Some(self.commit(Commit::Key));
                }
                if self.misses >= MAX_MISSES {
                    return Some(Outcome::Nothing(Reason::Unreadable));
                }
            }
            Observation::Value(raw) => {
                self.misses = 0;
                if raw.chars().count() > MAX_FIELD_CHARS {
                    return Some(Outcome::Nothing(Reason::TooLong));
                }
                let value = comparable(&raw);
                if watching {
                    if value.is_empty() {
                        return Some(self.commit(Commit::Emptied));
                    }
                    self.last = Some(value);
                    if key {
                        return Some(self.commit(Commit::Key));
                    }
                } else if value.contains(&self.inserted) {
                    self.baseline = Some(value.clone());
                    self.last = Some(value);
                    if key {
                        return Some(Outcome::Nothing(Reason::NoEdit));
                    }
                } else if key || elapsed_ms >= BASELINE_WAIT_MS {
                    return Some(Outcome::Nothing(Reason::NotFound));
                }
            }
        }
        (elapsed_ms >= WINDOW_MS).then_some(Outcome::Nothing(Reason::NoCommit))
    }
}

/// Whether a key-down commits what the user typed: Return or keypad Enter
/// without Shift or Option (those add a line in chat apps), or a plain
/// Tab.
pub fn is_commit_key(key: handy_keys::Key, modifiers: handy_keys::Modifiers) -> bool {
    use handy_keys::{Key, Modifiers};
    match key {
        Key::Return | Key::KeypadEnter => !modifiers.intersects(Modifiers::SHIFT | Modifiers::OPT),
        Key::Tab => (modifiers - Modifiers::FN).is_empty(),
        _ => false,
    }
}

#[cfg(target_os = "macos")]
pub mod mac {
    //! The AX side: every function runs on the AX thread (`fork::ax::thread`).

    use objc2_app_kit::NSWorkspace;
    use objc2_application_services::AXUIElement;
    use objc2_core_foundation::{CFRetained, Type};

    use super::Observation;
    use crate::fork::ax::editable::Family;
    use crate::fork::ax::element as el;
    use crate::fork::ax::thread::{Ctx, Watched, CALL_TIMEOUT_S};
    use crate::fork::context::caret::{exclusions, field_exclusion, url_host, TEXT_ENTRY_ROLES};

    /// How far up the tree a browser field looks for its page.
    const MAX_ANCESTORS: usize = 40;

    fn string(e: &AXUIElement, name: &str) -> String {
        el::attr_string(e, name).unwrap_or_default()
    }

    fn frame(e: &AXUIElement) -> Option<[f64; 4]> {
        el::attr_rect(e).map(|r| [r.origin.x, r.origin.y, r.size.width, r.size.height])
    }

    fn web_area(field: &AXUIElement) -> Option<CFRetained<AXUIElement>> {
        if string(field, "AXRole") == "AXWebArea" {
            return Some(field.retain());
        }
        let mut at = el::attr_element(field, "AXParent");
        for _ in 0..MAX_ANCESTORS {
            let e = at?;
            el::set_timeout(&e, CALL_TIMEOUT_S);
            if string(&e, "AXRole") == "AXWebArea" {
                return Some(e);
            }
            at = el::attr_element(&e, "AXParent");
        }
        None
    }

    /// Captures the focused element of the frontmost app for watch `id`,
    /// unless it may not be read. Returns the skip reason, if any.
    pub fn arm(ctx: &mut Ctx, id: u64) -> Result<(), &'static str> {
        if let Some(old) = ctx.watched.take() {
            if old.lease {
                ctx.lease_release(old.pid);
            }
        }
        if el::secure_input_on() {
            return Err("secure_input");
        }
        let Some(front) = NSWorkspace::sharedWorkspace().frontmostApplication() else {
            return Err("no_access");
        };
        let pid = front.processIdentifier();
        if pid <= 0 {
            return Err("no_access");
        }
        let bundle_id = front.bundleIdentifier().map(|s| s.to_string());
        let name = front.localizedName().map(|s| s.to_string());
        let family = el::family_of(&el::app_bundle_path(pid).unwrap_or_default());
        if let Some(skip) = exclusions().app(
            bundle_id.as_deref(),
            name.as_deref(),
            family == Family::Terminal,
        ) {
            return Err(skip.as_str());
        }
        let lease = family.builds_tree_on_request() && ctx.lease_acquire(pid);
        let release = |ctx: &mut Ctx| {
            if lease {
                ctx.lease_release(pid);
            }
        };
        let app = ctx.app(pid);
        let Some(field) = el::attr_element(&app, "AXFocusedUIElement") else {
            release(ctx);
            return Err("no_field");
        };
        el::set_timeout(&field, CALL_TIMEOUT_S);
        let role = string(&field, "AXRole");
        let subrole = string(&field, "AXSubrole");
        let labels = [
            "AXDescription",
            "AXTitle",
            "AXPlaceholderValue",
            "AXIdentifier",
            "AXDOMIdentifier",
            "AXHelp",
        ]
        .iter()
        .map(|a| string(&field, a))
        .collect::<Vec<_>>()
        .join(" ");
        if let Some(skip) = field_exclusion(
            &role,
            &subrole,
            el::attr_bool(&field, "AXProtectedContent") == Some(true),
            &labels,
        ) {
            release(ctx);
            return Err(skip.as_str());
        }
        if matches!(family, Family::Chromium | Family::Webkit | Family::Gecko) {
            match web_area(&field) {
                None if TEXT_ENTRY_ROLES.contains(&role.as_str()) || role == "AXSearchField" => {
                    release(ctx);
                    return Err("url_bar");
                }
                Some(w) => {
                    let host = el::attr_url(&w, "AXURL").and_then(|u| url_host(&u));
                    if host.is_some_and(|h| exclusions().host(&h)) {
                        release(ctx);
                        return Err("finance");
                    }
                }
                None => {}
            }
        }
        if el::attr_isize(&field, "AXNumberOfCharacters")
            .is_some_and(|n| n > super::MAX_FIELD_CHARS as isize)
        {
            release(ctx);
            return Err("too_long");
        }
        let frame = frame(&field);
        ctx.watched = Some(Watched {
            id,
            pid,
            element: field,
            role,
            frame,
            lease,
        });
        Ok(())
    }

    /// Reads watch `id`'s field.
    pub fn poll(ctx: &mut Ctx, id: u64) -> Observation {
        if el::secure_input_on() {
            return Observation::SecureInput;
        }
        let Some(w) = ctx.watched.as_ref().filter(|w| w.id == id) else {
            return Observation::Unreadable;
        };
        let pid = w.pid;
        let front = NSWorkspace::sharedWorkspace()
            .frontmostApplication()
            .map(|a| a.processIdentifier());
        if front != Some(pid) {
            return Observation::Moved;
        }
        let app = ctx.app(pid);
        let Some(w) = ctx.watched.as_ref() else {
            return Observation::Unreadable;
        };
        let Some(now) = el::attr_element(&app, "AXFocusedUIElement") else {
            return Observation::Moved;
        };
        if *now != *w.element {
            // An app may hand out a new object for the same field; a chat
            // composer grows in height as the user types, so the same
            // place is the same role, x and width.
            el::set_timeout(&now, CALL_TIMEOUT_S);
            let same = string(&now, "AXRole") == w.role
                && match (frame(&now), w.frame) {
                    (Some(a), Some(b)) => (a[0] - b[0]).abs() < 1.0 && (a[2] - b[2]).abs() < 1.0,
                    _ => false,
                };
            if !same {
                return Observation::Moved;
            }
        }
        if el::attr_isize(&w.element, "AXNumberOfCharacters")
            .is_some_and(|n| n > super::MAX_FIELD_CHARS as isize)
        {
            return Observation::TooLong;
        }
        match el::attr_string(&w.element, "AXValue") {
            Some(v) => Observation::Value(v),
            None => Observation::Unreadable,
        }
    }

    /// Ends watch `id` (a newer watch is left alone) and releases its lease.
    pub fn disarm(ctx: &mut Ctx, id: u64) {
        if ctx.watched.as_ref().is_some_and(|w| w.id == id) {
            if let Some(w) = ctx.watched.take() {
                if w.lease {
                    ctx.lease_release(w.pid);
                }
            }
        }
    }
}
