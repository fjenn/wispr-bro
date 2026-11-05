// SPDX-License-Identifier: GPL-3.0-or-later
//! VoiceOver for the bar (S-37, R5 §5.5): every `NSAccessibility`
//! announcement the app makes, in one place.
//!
//! - [`announcement`]: which `rust.pill.*` key a change of the bar speaks
//!   (listening, transcribing, cleaning up, setting up, the card's reason,
//!   View changes, a message, a warning, cancelled), and its priority.
//! - [`spoken`]: that key localized through `fork/i18n.rs`, plus the card's
//!   own text (never a password field's: the view has no text then).
//! - [`on_inserted`]: "Text inserted" after a take was pasted (the bar
//!   simply goes back to rest, so nothing else would say so).
//! - [`announce_text`] / [`announce_main`]: post from the application
//!   element, on the main thread (S-30's spotlight hint uses these).
//!
//! Announcements come from the application element rather than the bar:
//! the resting bar may be hidden while a take shows, and VoiceOver drops
//! announcements from hidden elements. The bar's own names (the resting
//! bar "Dictation bar", the Dictate and Scratchpad buttons) are set on the
//! native views in `native.rs`; the take's web view names its controls in
//! `src/fork/pill/`.

use tauri::{AppHandle, Runtime};

use super::model::{ActiveView, PillMessage, PillPhase, PillTone, PillWarning, WordResult};

/// The card's text is cut here in an announcement; the full text stays on
/// the card (VoiceOver reads it there) and in History.
pub const CARD_TEXT_CHARS: usize = 280;

/// The VoiceOver announcement for a change of the bar, if any (`rust.pill.*`
/// keys), and whether it is high priority.
pub fn announcement(
    before: Option<&ActiveView>,
    after: Option<&ActiveView>,
) -> Option<(String, bool)> {
    let phase = |v: Option<&ActiveView>| v.map(|a| a.phase);
    // S-34: each note, and Add's confirmation, is spoken once.
    if let Some(w) = after
        .filter(|a| a.phase == PillPhase::Word)
        .and_then(|a| a.word.as_ref())
    {
        let id = |v: Option<&ActiveView>| {
            v.filter(|a| a.phase == PillPhase::Word)
                .and_then(|a| a.word.as_ref())
                .map(|w| (w.id, w.result))
        };
        if id(before) == Some((w.id, w.result)) {
            return None;
        }
        let key = match w.result {
            None => "rust.pill.announce.newWord",
            Some(WordResult::Added) => "rust.pill.word.added",
            Some(WordResult::Full) => "rust.pill.word.full",
        };
        return Some((key.into(), false));
    }
    if phase(before) == phase(after) && before.map(|a| a.message) == after.map(|a| a.message) {
        let warned = after.and_then(|a| a.warning);
        if warned.is_some() && before.and_then(|a| a.warning) != warned {
            let key = match warned? {
                PillWarning::Silence => "rust.pill.announce.silenceWarning",
                PillWarning::Length => "rust.pill.announce.lengthWarning",
                PillWarning::MicLost => "rust.pill.announce.micLostWarning",
                PillWarning::SecureInput => "rust.pill.announce.secureInputWarning",
            };
            return Some((key.into(), true));
        }
        return None;
    }
    match (phase(before), after) {
        (None | Some(PillPhase::Message), Some(a))
            if matches!(a.phase, PillPhase::Held | PillPhase::Locked) =>
        {
            Some(("rust.pill.announce.listening".into(), false))
        }
        (_, Some(a)) if a.phase == PillPhase::Transcribing => {
            Some(("rust.pill.announce.transcribing".into(), false))
        }
        (_, Some(a)) if a.phase == PillPhase::CleaningUp => {
            Some(("rust.pill.announce.cleaningUp".into(), false))
        }
        (_, Some(a)) if a.phase == PillPhase::SettingUp => {
            Some(("rust.pill.announce.settingUp".into(), false))
        }
        (_, Some(a)) if a.phase == PillPhase::Card => Some((
            a.card_reason
                .map_or_else(|| "rust.pill.announce.card".into(), super::card_key),
            true,
        )),
        (_, Some(a)) if a.phase == PillPhase::Diff => {
            Some(("rust.pill.announce.diff".into(), false))
        }
        (_, Some(a)) if a.phase == PillPhase::Message => a.message.map(|m| {
            let key = match m {
                // "Cancelled" was just announced by the take's end.
                PillMessage::Cancelled => "rust.pill.announce.undo".to_string(),
                _ => format!("rust.pill.message.{}", m.key()),
            };
            (key, m.tone() == PillTone::Error)
        }),
        (Some(PillPhase::Held | PillPhase::Locked), None) => {
            Some(("rust.pill.announce.cancelled".into(), false))
        }
        _ => None,
    }
}

/// What VoiceOver says for a change of the bar, localized, and whether it
/// is high priority. A card also reads its text (cut at
/// [`CARD_TEXT_CHARS`]); a password field's card has none.
pub fn spoken(
    locale: &str,
    before: Option<&ActiveView>,
    after: Option<&ActiveView>,
) -> Option<(String, bool)> {
    let (key, high) = announcement(before, after)?;
    let word = after
        .and_then(|a| a.word.as_ref())
        .map_or("", |w| w.word.as_str());
    let mut text = crate::fork::i18n::t(locale, &key, &[("word", word)]);
    if let Some(card) = after
        .filter(|a| a.phase == PillPhase::Card)
        .and_then(|a| a.text.as_deref())
        .map(str::trim)
        .filter(|t| !t.is_empty())
    {
        // A pause between the reason and the words.
        let stop = if text.ends_with(['.', '!', '?', '。', '！', '？']) {
            ""
        } else {
            "."
        };
        text = format!("{text}{stop} {}", cut(card, CARD_TEXT_CHARS));
    }
    Some((text, high))
}

/// `text` cut to `max` characters at a word boundary, with an ellipsis.
fn cut(text: &str, max: usize) -> String {
    if text.chars().count() <= max {
        return text.to_string();
    }
    let head: String = text.chars().take(max).collect();
    let head = match head.rfind(char::is_whitespace) {
        Some(i) if i > max / 2 => &head[..i],
        _ => head.as_str(),
    };
    format!("{}…", head.trim_end())
}

/// A take was pasted into another app: "Text inserted". Takes only (a
/// transform says "Done" through its message). Any thread; nothing where
/// the bar does not run (non-macOS, debug-only builds, unit tests).
pub fn on_inserted() {
    super::announce_key("rust.pill.announce.inserted", false);
}

/// Posts `text` from the main thread. Any thread.
pub fn announce_text<R: Runtime>(app: &AppHandle<R>, text: String, high: bool) {
    #[cfg(target_os = "macos")]
    {
        let _ = app.run_on_main_thread(move || announce(&text, high));
    }
    #[cfg(not(target_os = "macos"))]
    {
        let _ = (app, text, high);
    }
}

/// S-30: a VoiceOver announcement from the application element. Main
/// thread only.
#[cfg(target_os = "macos")]
pub fn announce_main(text: &str) {
    announce(text, false);
}

/// Posts `text` from the application element
/// (`NSAccessibilityAnnouncementRequested`). Main thread only; a call from
/// another thread is dropped.
#[cfg(target_os = "macos")]
pub fn announce(text: &str, high: bool) {
    use objc2::runtime::AnyObject;
    use objc2::MainThreadMarker;
    use objc2_app_kit::{
        NSAccessibilityAnnouncementKey, NSAccessibilityAnnouncementRequestedNotification,
        NSAccessibilityPostNotificationWithUserInfo, NSAccessibilityPriorityKey,
        NSAccessibilityPriorityLevel, NSApplication,
    };
    use objc2_foundation::{NSDictionary, NSNumber, NSString};

    let Some(mtm) = MainThreadMarker::new() else {
        return;
    };
    let app = NSApplication::sharedApplication(mtm);
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
            &app,
            NSAccessibilityAnnouncementRequestedNotification,
            Some(&info),
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn long_card_text_is_cut_at_a_word() {
        let long = "word ".repeat(100);
        let c = cut(&long, 20);
        assert!(c.ends_with('…'));
        assert!(c.chars().count() <= 21);
        assert!(!c.contains("wor…"));
        assert_eq!(cut("short", 20), "short");
    }
}
