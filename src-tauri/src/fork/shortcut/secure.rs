// SPDX-License-Identifier: GPL-3.0-or-later
//! Secure Event Input and the fork's bindings (S-02, R5 §4.8, A-205).
//!
//! While any process holds Secure Input, event taps get no key-down/up
//! (flags still flow): fn hold and double-tap keep working, fn+Space and
//! every keyed binding do not. Handy's `secure_input.rs` keeps keyed
//! bindings alive by shadow-registering them through the Carbon-backed
//! global-shortcut path while Secure Input is sustained (≥ 3 s). The fork
//! owns `transcribe`, `transcribe_with_post_process` and `cancel`
//! (`owns()`), so without this module the shadows would cover nothing of
//! ours and Esc could not cancel under Secure Input.
//!
//! - [`secure_input_bindings`] is the one hook in Handy's
//!   `reconcile_fallback`: the bindings it shadows are Handy's minus the
//!   fork-owned ids, plus the fork's keyed bindings as `fork:<action>`
//!   (⌃⌘V, ⌃⌘C, ⌥1/⌥2/⌥O when on, Ctrl+Opt+Space…) and Esc as
//!   `fork:cancel` **only while a take records or is processed**, so an
//!   idle Esc in a password field is never swallowed. Handy's
//!   covered/degraded/uncovered lists (the A-205 diagnostic, the banner and
//!   the tray) then describe the fork's bindings; fn+key chords cannot be
//!   expressed through Carbon and show as uncovered.
//! - [`on_shadow_event`] is the hook in Handy's `handle_shortcut_event`: a
//!   Carbon press on a `fork:` id goes to the fork's engine (which drops it
//!   if the tap saw the same key a moment earlier).
//! - [`holder_name`] names the app holding Secure Input for the bar,
//!   from Handy's IORegistry lookup (`kCGSSessionSecureInputPID`, the Yap
//!   technique R4 keeps) and `NSRunningApplication`.

use std::collections::{BTreeMap, HashMap};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Mutex;

use handy_keys::{Hotkey, Key};

use super::bindings::Profile;
use crate::fork::settings::hotkey_actions;
use crate::settings::ShortcutBinding;

/// Prefix of the fork's shadow binding ids.
pub const SHADOW_PREFIX: &str = "fork:";

/// What the fork wants shadowed, kept up to date by the engine.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ShadowSpec {
    /// `(action, hotkey)` for every keyed binding the engine runs.
    pub keyed: Vec<(String, Hotkey)>,
    /// `cancel` is Esc.
    pub esc_cancels: bool,
}

impl ShadowSpec {
    pub fn from_profile(p: &Profile) -> ShadowSpec {
        ShadowSpec {
            keyed: p
                .keyed
                .iter()
                .map(|b| (b.action.clone(), b.hotkey))
                .collect(),
            esc_cancels: p.esc_cancels,
        }
    }
}

static SPEC: Mutex<Option<ShadowSpec>> = Mutex::new(None);
/// A take records or is being processed (Esc then cancels).
static ACTIVE: AtomicBool = AtomicBool::new(false);
/// Runs Handy's `reconcile_fallback` off the calling thread (set by the
/// engine at start; only product builds).
static RECONCILE: Mutex<Option<Box<dyn Fn() + Send + Sync>>> = Mutex::new(None);

/// The engine started (or reloaded its bindings).
pub fn set_spec(spec: ShadowSpec) {
    let changed = {
        let mut s = SPEC.lock().unwrap_or_else(|e| e.into_inner());
        let changed = s.as_ref() != Some(&spec);
        *s = Some(spec);
        changed
    };
    if changed {
        reconcile();
    }
}

/// How Handy's fallback is re-run (Wry only).
pub fn set_reconciler(f: impl Fn() + Send + Sync + 'static) {
    *RECONCILE.lock().unwrap_or_else(|e| e.into_inner()) = Some(Box::new(f));
}

fn reconcile() {
    if let Some(f) = RECONCILE.lock().unwrap_or_else(|e| e.into_inner()).as_ref() {
        f();
    }
}

/// A take started or ended (recording or processing). Esc is shadowed
/// only in between.
pub fn set_active(active: bool) {
    if ACTIVE.swap(active, Ordering::SeqCst) != active {
        log::debug!("fork hotkeys: Esc shadow wanted: {active}");
        reconcile();
    }
}

pub fn is_active() -> bool {
    ACTIVE.load(Ordering::SeqCst)
}

/// The `secure_input.rs` hook: the bindings Handy's Carbon fallback
/// shadows. Plain Handy builds get Handy's bindings unchanged.
pub fn secure_input_bindings(
    handy: &HashMap<String, ShortcutBinding>,
) -> BTreeMap<String, ShortcutBinding> {
    let spec = SPEC.lock().unwrap_or_else(|e| e.into_inner()).clone();
    plan(
        handy,
        crate::fork::is_product_build(),
        spec.as_ref(),
        is_active(),
    )
}

/// [`secure_input_bindings`], pure.
pub fn plan(
    handy: &HashMap<String, ShortcutBinding>,
    product_build: bool,
    spec: Option<&ShadowSpec>,
    active: bool,
) -> BTreeMap<String, ShortcutBinding> {
    let mut out: BTreeMap<String, ShortcutBinding> = handy
        .iter()
        .filter(|(id, _)| !super::owns_in(product_build, id))
        .map(|(id, b)| (id.clone(), b.clone()))
        .collect();
    if !product_build {
        return out;
    }
    let Some(spec) = spec else { return out };
    let mut add = |action: &str, chord: String, name: &str| {
        let id = format!("{SHADOW_PREFIX}{action}");
        out.insert(
            id.clone(),
            ShortcutBinding {
                id,
                name: name.into(),
                description: "Wispr Bro shortcut, shadowed under Secure Input".into(),
                default_binding: chord.clone(),
                current_binding: chord,
            },
        );
    };
    for (action, hotkey) in &spec.keyed {
        add(action, hotkey.to_handy_string(), action);
    }
    if spec.esc_cancels && active {
        let esc = Hotkey {
            modifiers: handy_keys::Modifiers::empty(),
            key: Some(Key::Escape),
        };
        add(hotkey_actions::CANCEL, esc.to_handy_string(), "Cancel");
    }
    out
}

/// The fork action behind a shadow id.
pub fn shadow_action(binding_id: &str) -> Option<&str> {
    binding_id.strip_prefix(SHADOW_PREFIX)
}

/// The `shortcut/handler.rs` hook (first in `handle_shortcut_event`): a
/// Carbon shadow of a fork binding fired. Returns `true` when the id is
/// the fork's (handled or deliberately ignored), so Handy's handler stops.
pub fn on_shadow_event(binding_id: &str, is_pressed: bool) -> bool {
    let Some(action) = shadow_action(binding_id) else {
        return false;
    };
    if is_pressed {
        log::info!("fork hotkeys: Secure Input shadow fired ({action})");
        super::shadow_fired(action);
    }
    true
}

/// The name of the app holding Secure Input, for the bar. `pid` and
/// `process` come from Handy's monitor (IORegistry, `ps`). `loginwindow`
/// (a stuck holder) and a vanished process name nobody.
pub fn holder_name(pid: Option<i32>, process: Option<&str>) -> Option<String> {
    let from_app = pid.and_then(app_name);
    let name = from_app.or_else(|| process.map(str::to_string))?;
    let trimmed = name.trim();
    let lower = trimmed.to_ascii_lowercase();
    if trimmed.is_empty() || lower == "loginwindow" || trimmed.starts_with('(') {
        return None;
    }
    Some(trimmed.to_string())
}

#[cfg(target_os = "macos")]
fn app_name(pid: i32) -> Option<String> {
    use objc2_app_kit::NSRunningApplication;
    let app = NSRunningApplication::runningApplicationWithProcessIdentifier(pid)?;
    app.localizedName().map(|n| n.to_string())
}

#[cfg(not(target_os = "macos"))]
fn app_name(_pid: i32) -> Option<String> {
    None
}
