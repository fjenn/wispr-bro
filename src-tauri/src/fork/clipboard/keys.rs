// SPDX-License-Identifier: MIT AND GPL-3.0-or-later
// Copyright (c) 2026 Christopher Langton (hex: key events, typing, modifier wait)
// Copyright (c) 2025 CJ Pais (Handy: layout-aware ⌘V key code)
// Modified by Wispr Bro contributors, 2026-10-30 (RS-05 spike; lifted into
// fork/clipboard by S-04 the same day, plus `post_return`).
//
// Ported from hex (anomalyco/hex on GitHub, branch dev, commit
// 3607595d92345af4be9f0eaa86ae3750c8851e39, MIT): `shortcut_event_specs`,
// `typing_strokes`, `KeyboardEvent` and `type_text` from `src/keyboard.rs`;
// `wait_for_modifiers_released` from `crates/capture/src/capture/macos.rs`.
// `resolve_command_v_keycode` is ported from Handy `src-tauri/src/input.rs`
// (MIT). Changes: the modifier wait takes its flag source, clock and sleep
// as arguments (testable), also waits for fn, and reports how long it
// waited and whether it gave up; event posting reports errors as strings.
// MIT texts: LICENSES/MIT-hex.txt, LICENSES/MIT-Handy.txt. See docs/fork/PORTS.md.

//! Synthetic keys: the ⌘V paste (layout-aware), typing (the Chromium 155/156
//! path), and the wait for the hotkey's modifiers to be physically released
//! before a paste, so a held ⌃⌘ (paste-last, S-06) cannot turn ⌘V into ⌃⌘V.

use serde::Serialize;
use std::ffi::c_void;
use std::time::{Duration, Instant};

pub const SHIFT_FLAG: u64 = 1 << 17;
pub const CONTROL_FLAG: u64 = 1 << 18;
pub const ALTERNATE_FLAG: u64 = 1 << 19;
pub const COMMAND_FLAG: u64 = 1 << 20;
/// kCGEventFlagMaskSecondaryFn.
pub const FN_FLAG: u64 = 1 << 23;
/// Modifiers that must be up before we post ⌘V.
pub const HOTKEY_MODIFIERS: u64 =
    SHIFT_FLAG | CONTROL_FLAG | ALTERNATE_FLAG | COMMAND_FLAG | FN_FLAG;

const COMMAND_KEY_CODE: u16 = 55;
const SHIFT_KEY_CODE: u16 = 56;
const CONTROL_KEY_CODE: u16 = 59;
const RETURN_KEY_CODE: u16 = 36;
/// kVK_ANSI_V, the fallback when the layout cannot be read.
pub const ANSI_V_KEYCODE: u16 = 9;
/// Quartz keeps at most 20 UTF-16 units of a keyboard event's Unicode string.
pub const MAX_UNICODE_UNITS: usize = 20;
/// kCGEventSourceUserData: marks our own events so a listening tap can tell
/// them apart.
pub const EVENT_SOURCE_USER_DATA: u32 = 42;
pub const SYNTHETIC_EVENT_MARKER: i64 = 0x5742_5253_3035; // "WBRS05"
const HID_EVENT_TAP: u32 = 0;

/// Names of the modifiers in `flags`, e.g. "⌃⌘".
pub fn modifier_names(flags: u64) -> String {
    let mut s = String::new();
    for (f, n) in [
        (FN_FLAG, "fn"),
        (CONTROL_FLAG, "⌃"),
        (ALTERNATE_FLAG, "⌥"),
        (SHIFT_FLAG, "⇧"),
        (COMMAND_FLAG, "⌘"),
    ] {
        if flags & f != 0 {
            s.push_str(n);
        }
    }
    s
}

// ---------------------------------------------------------------------------
// Modifier wait

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
pub struct ModifierWait {
    pub waited_ms: u64,
    /// The modifiers were still down when `max` ran out.
    pub timed_out: bool,
    /// What was held when the wait started.
    pub held_at_start: u64,
}

/// Polls `flags` until no hotkey modifier is down or `max` has passed.
pub fn wait_for_modifiers_released(
    max: Duration,
    mut flags: impl FnMut() -> u64,
    mut now: impl FnMut() -> Instant,
    mut sleep: impl FnMut(Duration),
) -> ModifierWait {
    let start = now();
    let held_at_start = flags() & HOTKEY_MODIFIERS;
    let mut held = held_at_start;
    while held != 0 {
        if now().saturating_duration_since(start) >= max {
            return ModifierWait {
                waited_ms: max.as_millis() as u64,
                timed_out: true,
                held_at_start,
            };
        }
        sleep(Duration::from_millis(10));
        held = flags() & HOTKEY_MODIFIERS;
    }
    ModifierWait {
        waited_ms: now().saturating_duration_since(start).as_millis() as u64,
        timed_out: false,
        held_at_start,
    }
}

// ---------------------------------------------------------------------------
// Event specs (pure)

/// (key code, key down, flags) for: modifiers down in order, the key
/// `count` times, modifiers up in reverse.
pub fn shortcut_event_specs(
    key_code: u16,
    modifiers: &[(u64, u16)],
    count: u8,
) -> Vec<(u16, bool, u64)> {
    let flags = modifiers.iter().fold(0, |flags, (flag, _)| flags | flag);
    let mut active_flags = 0;
    let mut remaining_flags = flags;
    modifiers
        .iter()
        .map(|(flag, key)| {
            active_flags |= flag;
            (*key, true, active_flags)
        })
        .chain((0..count).flat_map(|_| [(key_code, true, flags), (key_code, false, flags)]))
        .chain(modifiers.iter().rev().map(|(flag, key)| {
            remaining_flags &= !flag;
            (*key, false, remaining_flags)
        }))
        .collect()
}

#[derive(Debug, PartialEq, Eq)]
pub enum Stroke {
    Text(Vec<u16>),
    LineBreak,
}

/// Splits text into key events of at most `MAX_UNICODE_UNITS` UTF-16 units,
/// without splitting a surrogate pair. `\r\n`, `\n`, and `\r` each become one
/// line break (posted as Shift-Return so chat boxes do not send).
pub fn typing_strokes(text: &str) -> Vec<Stroke> {
    let mut strokes = Vec::new();
    let mut units = Vec::new();
    let mut characters = text.chars().peekable();
    while let Some(character) = characters.next() {
        if matches!(character, '\r' | '\n') {
            if character == '\r' {
                characters.next_if_eq(&'\n');
            }
            if !units.is_empty() {
                strokes.push(Stroke::Text(std::mem::take(&mut units)));
            }
            strokes.push(Stroke::LineBreak);
            continue;
        }
        let mut encoded = [0; 2];
        let encoded = character.encode_utf16(&mut encoded);
        if units.len() + encoded.len() > MAX_UNICODE_UNITS {
            strokes.push(Stroke::Text(std::mem::take(&mut units)));
        }
        units.extend_from_slice(encoded);
    }
    if !units.is_empty() {
        strokes.push(Stroke::Text(units));
    }
    strokes
}

// ---------------------------------------------------------------------------
// Posting (CoreGraphics)

type EventRef = *mut c_void;

#[link(name = "ApplicationServices", kind = "framework")]
extern "C" {
    fn CGEventCreateKeyboardEvent(source: *mut c_void, key: u16, down: bool) -> EventRef;
    fn CGEventSetFlags(event: EventRef, flags: u64);
    fn CGEventKeyboardSetUnicodeString(event: EventRef, length: usize, text: *const u16);
    fn CGEventSetIntegerValueField(event: EventRef, field: u32, value: i64);
    fn CGEventPost(tap: u32, event: EventRef);
    fn CGEventSourceFlagsState(state_id: i32) -> u64;
}

#[link(name = "CoreFoundation", kind = "framework")]
extern "C" {
    fn CFRelease(value: *const c_void);
    fn CFDataGetBytePtr(data: *const c_void) -> *const u8;
}

/// Modifiers down right now: the HID state (the physical keys) or'd with the
/// combined session state, as hex does.
pub fn flags_now() -> u64 {
    // SAFETY: read-only queries of documented event-source states
    // (1 = kCGEventSourceStateHIDSystemState, 0 = CombinedSessionState).
    unsafe { CGEventSourceFlagsState(0) | CGEventSourceFlagsState(1) }
}

struct KeyboardEvent(EventRef);

impl KeyboardEvent {
    fn new((key, down, flags): (u16, bool, u64)) -> Result<Self, String> {
        // SAFETY: null selects the default event source; released in Drop.
        let event = unsafe { CGEventCreateKeyboardEvent(std::ptr::null_mut(), key, down) };
        if event.is_null() {
            return Err("could not create a keyboard event".into());
        }
        // SAFETY: `event` is a valid event we own.
        unsafe {
            CGEventSetFlags(event, flags);
            CGEventSetIntegerValueField(event, EVENT_SOURCE_USER_DATA, SYNTHETIC_EVENT_MARKER);
        }
        Ok(Self(event))
    }

    fn post(&self) {
        // SAFETY: this value owns a valid CoreGraphics keyboard event.
        unsafe { CGEventPost(HID_EVENT_TAP, self.0) };
    }

    fn set_unicode(&self, text: &[u16]) {
        // SAFETY: the event is valid and Quartz copies the buffer.
        unsafe { CGEventKeyboardSetUnicodeString(self.0, text.len(), text.as_ptr()) };
    }
}

impl Drop for KeyboardEvent {
    fn drop(&mut self) {
        // SAFETY: final owner of the retained event.
        unsafe { CFRelease(self.0.cast_const()) };
    }
}

fn post_specs(specs: &[(u16, bool, u64)]) -> Result<(), String> {
    let events = specs
        .iter()
        .map(|s| KeyboardEvent::new(*s))
        .collect::<Result<Vec<_>, _>>()?;
    for event in events {
        event.post();
    }
    Ok(())
}

/// Posts ⌘<key> (⌘ down, key down/up, ⌘ up) with explicit flags.
pub fn post_command_key(key_code: u16) -> Result<(), String> {
    post_specs(&shortcut_event_specs(
        key_code,
        &[(COMMAND_FLAG, COMMAND_KEY_CODE)],
        1,
    ))
}

/// ⌘V on the active layout.
pub fn post_command_v() -> Result<u16, String> {
    let code = command_v_keycode();
    post_command_key(code)?;
    Ok(code)
}

/// A plain key press (no modifiers), e.g. Escape (53).
pub fn post_key(key_code: u16) -> Result<(), String> {
    post_specs(&[(key_code, true, 0), (key_code, false, 0)])
}

/// Return with `flags` (auto-submit after a paste: plain, ⌃ or ⌘).
pub fn post_return(flags: u64) -> Result<(), String> {
    let mods: Vec<(u64, u16)> = [
        (CONTROL_FLAG, CONTROL_KEY_CODE),
        (COMMAND_FLAG, COMMAND_KEY_CODE),
    ]
    .into_iter()
    .filter(|(f, _)| flags & f != 0)
    .collect();
    post_specs(&shortcut_event_specs(RETURN_KEY_CODE, &mods, 1))
}

/// Types `text` as synthesized key events, never touching the clipboard.
pub fn type_text(text: &str) -> Result<(), String> {
    let mut events = Vec::new();
    for stroke in typing_strokes(text) {
        match stroke {
            Stroke::Text(units) => {
                for down in [true, false] {
                    let event = KeyboardEvent::new((0, down, 0))?;
                    event.set_unicode(&units);
                    events.push(event);
                }
            }
            Stroke::LineBreak => {
                for spec in
                    shortcut_event_specs(RETURN_KEY_CODE, &[(SHIFT_FLAG, SHIFT_KEY_CODE)], 1)
                {
                    events.push(KeyboardEvent::new(spec)?);
                }
            }
        }
    }
    for event in events {
        event.post();
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// Layout-aware ⌘V (Handy `input.rs`)

type TisInputSourceRef = *const c_void;
const KEYCODE_COUNT: u16 = 128;
const UC_KEY_ACTION_DISPLAY: u16 = 3;
const UC_KEY_TRANSLATE_NO_DEAD_KEYS_MASK: u32 = 1;
/// Carbon's cmdKey >> 8.
const COMMAND_MODIFIER_STATE: u32 = 1;

#[link(name = "Carbon", kind = "framework")]
extern "C" {
    fn TISCopyCurrentKeyboardLayoutInputSource() -> TisInputSourceRef;
    fn TISGetInputSourceProperty(source: TisInputSourceRef, key: *const c_void) -> *const c_void;
    static kTISPropertyUnicodeKeyLayoutData: *const c_void;
    fn UCKeyTranslate(
        key_layout: *const u8,
        virtual_key_code: u16,
        key_action: u16,
        modifier_key_state: u32,
        keyboard_type: u32,
        key_translate_options: u32,
        dead_key_state: *mut u32,
        max_string_length: usize,
        actual_string_length: *mut usize,
        unicode_string: *mut u16,
    ) -> i32;
    fn LMGetKbdType() -> u8;
}

/// The physical key macOS reads as `v` while ⌘ is held. Must run on the
/// main thread (TIS).
pub fn resolve_command_v_keycode() -> Result<u16, String> {
    resolve_command_keycode(b'v')
}

/// The physical key macOS reads as `ch` while ⌘ is held (Handy's resolver,
/// generalised from `v` to any ASCII letter for ⌘A / ⌘C).
pub fn resolve_command_keycode(ch: u8) -> Result<u16, String> {
    // SAFETY: Create Rule; released below.
    let source = unsafe { TISCopyCurrentKeyboardLayoutInputSource() };
    if source.is_null() {
        return Err("macOS returned no keyboard layout".into());
    }
    let result = (|| {
        // SAFETY: the source stays retained during the scan.
        let layout_data =
            unsafe { TISGetInputSourceProperty(source, kTISPropertyUnicodeKeyLayoutData) };
        if layout_data.is_null() {
            return Err("the keyboard layout has no Unicode layout data".to_string());
        }
        // SAFETY: CFData owned by the retained source.
        let layout = unsafe { CFDataGetBytePtr(layout_data) };
        if layout.is_null() {
            return Err("the keyboard layout data is empty".to_string());
        }
        // SAFETY: no arguments.
        let keyboard_type = unsafe { LMGetKbdType() } as u32;
        (0..KEYCODE_COUNT)
            .find(|&keycode| {
                let mut dead_key_state = 0;
                let mut chars = [0_u16; 4];
                let mut length = 0_usize;
                // SAFETY: valid layout bytes; outputs point to local storage.
                let status = unsafe {
                    UCKeyTranslate(
                        layout,
                        keycode,
                        UC_KEY_ACTION_DISPLAY,
                        COMMAND_MODIFIER_STATE,
                        keyboard_type,
                        UC_KEY_TRANSLATE_NO_DEAD_KEYS_MASK,
                        &mut dead_key_state,
                        chars.len(),
                        &mut length,
                        chars.as_mut_ptr(),
                    )
                };
                status == 0 && length == 1 && chars[0] == u16::from(ch)
            })
            .ok_or_else(|| format!("no key types {} with ⌘ in this layout", ch as char))
    })();
    // SAFETY: balances the copy above.
    unsafe { CFRelease(source) };
    result
}

pub fn command_v_keycode() -> u16 {
    resolve_command_v_keycode().unwrap_or(ANSI_V_KEYCODE)
}

/// ⌘<letter> on the active layout (ANSI fallback for a, c, v).
pub fn post_command_letter(ch: u8) -> Result<(), String> {
    let fallback = match ch {
        b'a' => 0,
        b'c' => 8,
        _ => ANSI_V_KEYCODE,
    };
    post_command_key(resolve_command_keycode(ch).unwrap_or(fallback))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::Cell;

    #[test]
    fn command_v_specs() {
        assert_eq!(
            shortcut_event_specs(9, &[(COMMAND_FLAG, 55)], 1),
            vec![
                (55, true, COMMAND_FLAG),
                (9, true, COMMAND_FLAG),
                (9, false, COMMAND_FLAG),
                (55, false, 0),
            ]
        );
    }

    #[test]
    fn typing_splits_at_twenty_units_and_line_breaks() {
        let s = typing_strokes("abcdefghijklmnopqrstuvwxyz\r\nok");
        assert_eq!(s.len(), 4);
        assert!(matches!(&s[0], Stroke::Text(u) if u.len() == 20));
        assert!(matches!(&s[1], Stroke::Text(u) if u.len() == 6));
        assert_eq!(s[2], Stroke::LineBreak);
        // A surrogate pair is never split.
        let s = typing_strokes(&format!("{}😀", "a".repeat(19)));
        assert!(matches!(&s[0], Stroke::Text(u) if u.len() == 19));
        assert!(matches!(&s[1], Stroke::Text(u) if u.len() == 2));
    }

    /// A fake clock: each sleep advances it.
    fn run(held_until_ms: u64, max_ms: u64, initial: u64) -> ModifierWait {
        let t0 = Instant::now();
        let elapsed = Cell::new(0u64);
        wait_for_modifiers_released(
            Duration::from_millis(max_ms),
            || {
                if elapsed.get() < held_until_ms {
                    initial
                } else {
                    0
                }
            },
            || t0 + Duration::from_millis(elapsed.get()),
            |d| elapsed.set(elapsed.get() + d.as_millis() as u64),
        )
    }

    #[test]
    fn nothing_held_does_not_wait() {
        let w = run(0, 1000, 0);
        assert_eq!(w.waited_ms, 0);
        assert!(!w.timed_out);
    }

    #[test]
    fn waits_for_control_command_release() {
        let w = run(250, 3000, CONTROL_FLAG | COMMAND_FLAG);
        assert!(!w.timed_out);
        assert_eq!(w.waited_ms, 250);
        assert_eq!(modifier_names(w.held_at_start), "⌃⌘");
    }

    #[test]
    fn gives_up_after_max() {
        let w = run(10_000, 300, COMMAND_FLAG);
        assert!(w.timed_out);
        assert_eq!(w.waited_ms, 300);
    }

    #[test]
    fn caps_lock_is_not_a_hotkey_modifier() {
        let caps = 1 << 16;
        assert!(!run(10_000, 300, caps).timed_out);
        // fn is.
        assert!(run(10_000, 300, FN_FLAG).timed_out);
    }
}
