// SPDX-License-Identifier: GPL-3.0-or-later
//! Layout-aware labels for hotkey chords ("⌃⌘V", "fn Space"), plus the
//! spoken form for accessibility ("Control Command V").
//!
//! Chords name keys by **position** (handy-keys `Key::Q` is the ANSI Q key,
//! macOS keycode 12), so the label shows the character the active layout
//! puts there: `Key::Q` reads "A" on AZERTY, `Key::Y` reads "Z" on QWERTZ,
//! `Key::V` reads "K" on Dvorak. The live layout comes from
//! `UCKeyTranslate`, the same technique as Handy's `input.rs` (MIT), which
//! resolves the layout's ⌘V key; that module is private, so the few FFI
//! calls are repeated here. Modifier glyphs follow Apple's order ⌃⌥⇧⌘.

use handy_keys::{Hotkey, Key, Modifiers};
use serde::Serialize;
use specta::Type;

use super::decision::compound;
use crate::fork::i18n;

/// The character a keycode types on a layout, with no modifiers.
pub trait Layout {
    fn char_for(&self, keycode: u16) -> Option<char>;
}

/// US ANSI: the label is the key's own name.
pub struct Ansi;

impl Layout for Ansi {
    fn char_for(&self, keycode: u16) -> Option<char> {
        ANSI.iter()
            .find(|(_, c, _)| *c == keycode)
            .map(|(_, _, ch)| *ch)
    }
}

/// (key, macOS virtual keycode, US character) for every key a layout can
/// rename (Carbon `Events.h`, `kVK_ANSI_*`, `kVK_ISO_Section`).
const ANSI: [(Key, u16, char); 48] = [
    (Key::A, 0, 'a'),
    (Key::S, 1, 's'),
    (Key::D, 2, 'd'),
    (Key::F, 3, 'f'),
    (Key::H, 4, 'h'),
    (Key::G, 5, 'g'),
    (Key::Z, 6, 'z'),
    (Key::X, 7, 'x'),
    (Key::C, 8, 'c'),
    (Key::V, 9, 'v'),
    (Key::Section, 10, '§'),
    (Key::B, 11, 'b'),
    (Key::Q, 12, 'q'),
    (Key::W, 13, 'w'),
    (Key::E, 14, 'e'),
    (Key::R, 15, 'r'),
    (Key::Y, 16, 'y'),
    (Key::T, 17, 't'),
    (Key::Num1, 18, '1'),
    (Key::Num2, 19, '2'),
    (Key::Num3, 20, '3'),
    (Key::Num4, 21, '4'),
    (Key::Num6, 22, '6'),
    (Key::Num5, 23, '5'),
    (Key::Equal, 24, '='),
    (Key::Num9, 25, '9'),
    (Key::Num7, 26, '7'),
    (Key::Minus, 27, '-'),
    (Key::Num8, 28, '8'),
    (Key::Num0, 29, '0'),
    (Key::RightBracket, 30, ']'),
    (Key::O, 31, 'o'),
    (Key::U, 32, 'u'),
    (Key::LeftBracket, 33, '['),
    (Key::I, 34, 'i'),
    (Key::P, 35, 'p'),
    (Key::L, 37, 'l'),
    (Key::J, 38, 'j'),
    (Key::Quote, 39, '\''),
    (Key::K, 40, 'k'),
    (Key::Semicolon, 41, ';'),
    (Key::Backslash, 42, '\\'),
    (Key::Comma, 43, ','),
    (Key::Slash, 44, '/'),
    (Key::N, 45, 'n'),
    (Key::M, 46, 'm'),
    (Key::Period, 47, '.'),
    (Key::Grave, 50, '`'),
];

/// The keycode of a layout-dependent key.
pub fn keycode(key: Key) -> Option<u16> {
    ANSI.iter().find(|(k, _, _)| *k == key).map(|(_, c, _)| *c)
}

/// A chord's label.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Type)]
pub struct Label {
    /// Shown text: modifier glyphs then the key ("⌃⌘V", "fn Space").
    pub text: String,
    /// For screen readers ("Control Command V").
    pub spoken: String,
}

/// Fixed glyphs and their i18n key (`rust.hotkeys.keys.<name>`).
fn named_key(key: Key) -> Option<(&'static str, &'static str)> {
    Some(match key {
        Key::Space => ("", "space"),
        Key::Escape => ("esc", "escape"),
        Key::Return => ("↩", "return"),
        Key::Tab => ("⇥", "tab"),
        Key::Delete => ("⌫", "delete"),
        Key::ForwardDelete => ("⌦", "forwardDelete"),
        Key::LeftArrow => ("←", "left"),
        Key::RightArrow => ("→", "right"),
        Key::UpArrow => ("↑", "up"),
        Key::DownArrow => ("↓", "down"),
        Key::Home => ("↖", "home"),
        Key::End => ("↘", "end"),
        Key::PageUp => ("⇞", "pageUp"),
        Key::PageDown => ("⇟", "pageDown"),
        _ => return None,
    })
}

/// Labels a chord string. `None` when it does not parse.
pub fn label(chord: &str, layout: &dyn Layout, locale: &str) -> Option<Label> {
    let hotkey: Hotkey = chord.parse().ok()?;
    Some(label_hotkey(&hotkey, layout, locale))
}

pub fn label_hotkey(hotkey: &Hotkey, layout: &dyn Layout, locale: &str) -> Label {
    let word = |k: &str| i18n::t(locale, &format!("rust.hotkeys.keys.{k}"), &[]);
    let m = compound(hotkey.modifiers);
    let mut glyphs = String::new();
    let mut spoken: Vec<String> = Vec::new();
    for (bit, glyph, name) in [
        (Modifiers::CTRL, "⌃", "control"),
        (Modifiers::OPT, "⌥", "option"),
        (Modifiers::SHIFT, "⇧", "shift"),
        (Modifiers::CMD, "⌘", "command"),
    ] {
        if m.contains(bit) {
            glyphs.push_str(glyph);
            spoken.push(word(name));
        }
    }
    let mut parts: Vec<String> = Vec::new();
    if m.contains(Modifiers::FN) {
        parts.push("fn".into());
        spoken.insert(0, word("fn"));
    }
    if !glyphs.is_empty() {
        parts.push(glyphs);
    }
    if let Some(key) = hotkey.key {
        let (shown, said) = key_label(key, layout, locale);
        match parts.last_mut() {
            // Glyphs and a one-character key read as one token: "⌃⌘V".
            Some(last) if shown.chars().count() == 1 && !last.starts_with("fn") => {
                last.push_str(&shown)
            }
            _ => parts.push(shown),
        }
        spoken.push(said);
    }
    Label {
        text: parts.join(" "),
        spoken: spoken.join(" "),
    }
}

fn key_label(key: Key, layout: &dyn Layout, locale: &str) -> (String, String) {
    let word = |k: &str| i18n::t(locale, &format!("rust.hotkeys.keys.{k}"), &[]);
    if let Some((glyph, name)) = named_key(key) {
        let said = word(name);
        let shown = if glyph.is_empty() {
            said.clone()
        } else {
            glyph.to_string()
        };
        return (shown, said);
    }
    if let Some(code) = keycode(key) {
        let ch = layout
            .char_for(code)
            .or_else(|| Ansi.char_for(code))
            .map(|c| c.to_uppercase().collect::<String>())
            .unwrap_or_else(|| key.to_string());
        return (ch.clone(), ch);
    }
    // F-keys, keypad and media keys: their own name.
    let name = key.to_string();
    (name.clone(), name)
}

/// The active keyboard layout (macOS). `char_for` must run on the main
/// thread (TIS); [`CurrentLayout::load`] returns `None` elsewhere or when
/// the layout has no Unicode data, and callers fall back to [`Ansi`].
#[cfg(target_os = "macos")]
pub struct CurrentLayout {
    chars: Vec<(u16, char)>,
}

#[cfg(target_os = "macos")]
impl CurrentLayout {
    pub fn load() -> Option<CurrentLayout> {
        let chars = platform::layout_chars(ANSI.iter().map(|(_, c, _)| *c))?;
        Some(CurrentLayout { chars })
    }
}

#[cfg(target_os = "macos")]
impl Layout for CurrentLayout {
    fn char_for(&self, keycode: u16) -> Option<char> {
        self.chars
            .iter()
            .find(|(c, _)| *c == keycode)
            .map(|(_, ch)| *ch)
    }
}

#[cfg(target_os = "macos")]
mod platform {
    use std::ffi::c_void;

    type Ref = *const c_void;

    #[link(name = "Carbon", kind = "framework")]
    extern "C" {
        fn TISCopyCurrentKeyboardLayoutInputSource() -> Ref;
        fn TISGetInputSourceProperty(source: Ref, key: Ref) -> Ref;
        static kTISPropertyUnicodeKeyLayoutData: Ref;
        fn UCKeyTranslate(
            layout: *const u8,
            keycode: u16,
            action: u16,
            modifiers: u32,
            keyboard_type: u32,
            options: u32,
            dead_key_state: *mut u32,
            max_len: usize,
            actual_len: *mut usize,
            chars: *mut u16,
        ) -> i32;
        fn LMGetKbdType() -> u8;
    }

    #[link(name = "CoreFoundation", kind = "framework")]
    extern "C" {
        fn CFDataGetBytePtr(data: Ref) -> *const u8;
        fn CFRelease(value: Ref);
    }

    extern "C" {
        fn pthread_main_np() -> i32;
    }

    const KEY_ACTION_DISPLAY: u16 = 3;
    const NO_DEAD_KEYS: u32 = 1;

    pub fn layout_chars(codes: impl Iterator<Item = u16>) -> Option<Vec<(u16, char)>> {
        // SAFETY: plain query of the calling thread.
        if unsafe { pthread_main_np() } == 0 {
            return None; // TIS is main-thread only
        }
        // SAFETY: Create Rule; released below. Pointers are checked, the
        // layout bytes stay valid while the source is retained, and every
        // out-pointer is local storage of the given size.
        unsafe {
            let source = TISCopyCurrentKeyboardLayoutInputSource();
            if source.is_null() {
                return None;
            }
            let data = TISGetInputSourceProperty(source, kTISPropertyUnicodeKeyLayoutData);
            let layout = if data.is_null() {
                std::ptr::null()
            } else {
                CFDataGetBytePtr(data)
            };
            let out = (!layout.is_null()).then(|| {
                let kbd = LMGetKbdType() as u32;
                codes
                    .filter_map(|code| {
                        let mut dead = 0u32;
                        let mut buf = [0u16; 4];
                        let mut len = 0usize;
                        let status = UCKeyTranslate(
                            layout,
                            code,
                            KEY_ACTION_DISPLAY,
                            0,
                            kbd,
                            NO_DEAD_KEYS,
                            &mut dead,
                            buf.len(),
                            &mut len,
                            buf.as_mut_ptr(),
                        );
                        if status != 0 || len != 1 {
                            return None;
                        }
                        char::from_u32(buf[0] as u32)
                            .filter(|c| !c.is_control())
                            .map(|c| (code, c))
                    })
                    .collect()
            });
            CFRelease(source);
            out
        }
    }
}
