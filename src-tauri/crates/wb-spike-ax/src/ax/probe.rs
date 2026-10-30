// SPDX-License-Identifier: GPL-3.0-or-later
//! One full read of the focused element, timed call by call: what S-05
//! (`focused_target`, `is_editable`, `caret_window`), S-19b
//! (`document_url`, `window_title`), S-20 (caret window) and S-21
//! (`selected_text`, `set_selected_text`) will each ask. Runs on the AX
//! thread only.
//!
//! Privacy: nothing the owner typed is kept. Text is returned only when the
//! caller says the field is one of the tool's own test fields
//! (`allow_text`); otherwise only lengths, roles and the URL host.
//!
//! The focused-element lookup (app `AXFocusedUIElement`, then the
//! system-wide one filtered by PID) and the caret-window read follow
//! SpeakoFlow `smart_spacing/macos.rs` (MIT; see element.rs).

use std::time::{Duration, Instant};

use objc2_application_services::{AXError, AXUIElement};
use objc2_core_foundation::{CFBoolean, CFNumber, CFRange, CFRetained, CFString, CGRect, Type};
use serde::Serialize;

use super::editable::{self, Delivery, Facts, Family, Verdict};
use super::element::{self as el, error_name};
use super::markers;
use super::thread::{front_pid, Ctx};

/// The sentinel the `AXSelectedText` write test inserts (and removes).
pub const SENTINEL: &str = "[wb]";
/// Caret window either side of the selection, UTF-16 units (S-20 reads
/// ≤ 1,500 characters in all; this measures the call, not the budget).
const WINDOW_UNITS: isize = 200;
const MAX_VALUE_UNITS: usize = 200_000;
/// Test-page window titles start with this; only those titles are stored.
pub const TEST_TITLE: &str = "WB-RS04";

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum LeaseMode {
    /// Chromium/Electron: our lease switched off just before the probe.
    Off,
    /// Lease held for the probe (the product's path).
    On,
    /// Not a Chromium/Electron app: no lease involved.
    NotNeeded,
}

#[derive(Clone, Debug)]
pub struct ProbeOpts {
    pub timeout: f32,
    pub lease: LeaseMode,
    pub allow_text: bool,
    pub set_test: bool,
    pub menu_copy: bool,
}

#[derive(Clone, Debug, Serialize)]
pub struct Call {
    pub name: &'static str,
    pub phase: &'static str,
    pub ms: f64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub err: Option<&'static str>,
}

#[derive(Default)]
struct Rec {
    calls: Vec<Call>,
}

impl Rec {
    fn t<T>(
        &mut self,
        name: &'static str,
        phase: &'static str,
        f: impl FnOnce() -> Result<T, AXError>,
    ) -> Result<T, AXError> {
        let start = Instant::now();
        let r = f();
        self.calls.push(Call {
            name,
            phase,
            ms: start.elapsed().as_micros() as f64 / 1000.0,
            err: r.as_ref().err().map(|e| error_name(*e)),
        });
        r
    }

    /// An attribute read, typed by `conv`; a wrong type counts as NoValue.
    fn get<T>(
        &mut self,
        element: &AXUIElement,
        name: &'static str,
        phase: &'static str,
        conv: impl FnOnce(&objc2_core_foundation::CFType) -> Option<T>,
    ) -> Result<T, AXError> {
        self.t(name, phase, || {
            el::attr(element, name).and_then(|v| conv(&v).ok_or(AXError::NoValue))
        })
    }

    fn string(
        &mut self,
        e: &AXUIElement,
        name: &'static str,
        phase: &'static str,
    ) -> Option<String> {
        self.get(e, name, phase, |v| {
            v.downcast_ref::<CFString>().map(|s| s.to_string())
        })
        .ok()
    }

    fn boolean(
        &mut self,
        e: &AXUIElement,
        name: &'static str,
        phase: &'static str,
    ) -> Option<bool> {
        self.get(e, name, phase, |v| {
            v.downcast_ref::<CFBoolean>()
                .map(|b| b.as_bool())
                .or_else(|| {
                    v.downcast_ref::<CFNumber>()
                        .and_then(|n| n.as_i64())
                        .map(|n| n != 0)
                })
        })
        .ok()
    }

    fn number(&mut self, e: &AXUIElement, name: &'static str, phase: &'static str) -> Option<i64> {
        self.get(e, name, phase, |v| {
            v.downcast_ref::<CFNumber>().and_then(|n| n.as_i64())
        })
        .ok()
    }

    fn element(
        &mut self,
        e: &AXUIElement,
        name: &'static str,
        phase: &'static str,
    ) -> Option<CFRetained<AXUIElement>> {
        self.t(name, phase, || {
            el::attr(e, name)
                .and_then(|v| v.downcast::<AXUIElement>().map_err(|_| AXError::NoValue))
        })
        .ok()
    }

    fn settable(&mut self, e: &AXUIElement, name: &'static str) -> Option<bool> {
        let label: &'static str = match name {
            "AXValue" => "settable:AXValue",
            "AXSelectedText" => "settable:AXSelectedText",
            _ => "settable",
        };
        self.t(label, "core", || el::is_settable(e, name)).ok()
    }
}

#[derive(Clone, Debug, Default, Serialize)]
pub struct Selected {
    /// `AXSelectedText` length (UTF-16 units), if it answered.
    pub ax_len: Option<usize>,
    pub marker_supported: bool,
    pub marker_len: Option<usize>,
    /// Which step of the chain produced a non-empty selection.
    pub source: Option<&'static str>,
    /// The chain's text length equals the selected range's length.
    pub matches_range: Option<bool>,
    /// Only for the tool's own test fields.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub text: Option<String>,
}

#[derive(Clone, Debug, Default, Serialize)]
pub struct UrlInfo {
    /// none | file | http | other
    pub kind: &'static str,
    /// The host only (never the path) for web pages.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub host: Option<String>,
    pub test_page: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source: Option<&'static str>,
}

#[derive(Clone, Debug, Default, Serialize)]
pub struct SetTest {
    pub write_error: Option<&'static str>,
    /// The sentinel is in the field afterwards.
    pub verified: bool,
    /// The field is back to its length before the test.
    pub reverted: bool,
}

#[derive(Clone, Debug, Serialize)]
pub struct Probe {
    pub timeout: f32,
    pub lease: LeaseMode,
    pub lease_held: bool,
    /// Why switching `AXManualAccessibility` on failed, if it did.
    pub lease_error: Option<&'static str>,
    /// `AXManualAccessibility` as the app reported it before this probe.
    pub manual_ax_before: Option<bool>,
    /// System-wide `AXFocusedApplication` (None = the call failed).
    pub ax_app_pid: Option<i32>,
    /// The app probed: the AX answer, else NSWorkspace's frontmost.
    pub app_pid: Option<i32>,
    pub app_source: Option<&'static str>,
    pub app: String,
    pub family: Family,
    pub focus_source: Option<&'static str>,
    pub facts: Facts,
    pub verdict: Verdict,
    pub delivery: Delivery,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub dom_id: Option<String>,
    pub sel_range: Option<[i64; 2]>,
    pub char_count: Option<i64>,
    pub caret_window_units: Option<usize>,
    pub selected: Selected,
    pub edit_copy_enabled: Option<bool>,
    pub caret_bounds: Option<[f64; 4]>,
    pub frame: Option<[f64; 4]>,
    pub window_title_len: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub window_title: Option<String>,
    pub url: UrlInfo,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub set_test: Option<SetTest>,
    pub total_ms: f64,
    pub calls: Vec<Call>,
}

pub fn family_of(bundle: &str) -> Family {
    let name = el::bundle_name(bundle);
    if el::is_terminal(&name) {
        Family::Terminal
    } else if el::is_chromium(&name) {
        Family::Chromium
    } else if el::is_electron(bundle) {
        Family::Electron
    } else if name == "safari" || name == "safari technology preview" {
        Family::Webkit
    } else {
        Family::Native
    }
}

fn rect4(r: CGRect) -> [f64; 4] {
    [r.origin.x, r.origin.y, r.size.width, r.size.height]
}

/// The URL as stored: host only for web pages; our test page by name.
pub fn classify_url(url: &str, test_page_name: &str) -> UrlInfo {
    let lower = url.to_ascii_lowercase();
    if lower.starts_with("file:") {
        return UrlInfo {
            kind: "file",
            host: None,
            test_page: url.contains(test_page_name),
            source: None,
        };
    }
    if let Some(rest) = lower
        .strip_prefix("https://")
        .or_else(|| lower.strip_prefix("http://"))
    {
        let authority = rest.split(['/', '?', '#']).next().unwrap_or("");
        let host = authority.rsplit('@').next().unwrap_or("");
        let host = host.split(':').next().unwrap_or("").to_string();
        return UrlInfo {
            kind: "http",
            host: (!host.is_empty()).then_some(host),
            test_page: false,
            source: None,
        };
    }
    UrlInfo {
        kind: if url.is_empty() { "none" } else { "other" },
        ..UrlInfo::default()
    }
}

pub const TEST_PAGE_NAME: &str = "rs04-test-page.html";

struct Focus {
    /// What system-wide `AXFocusedApplication` answered (None = failed).
    ax_pid: Option<i32>,
    /// The PID used: the AX answer, else NSWorkspace's frontmost app.
    pid: Option<i32>,
    app_source: Option<&'static str>,
    element: Option<CFRetained<AXUIElement>>,
    source: Option<&'static str>,
}

/// The focused application's PID and element, SpeakoFlow's way, with one
/// addition: if system-wide `AXFocusedApplication` fails (seen on macOS 26
/// from some process trees: kAXErrorCannotComplete at once), the
/// frontmost app from NSWorkspace (`thread::front_pid`) is used instead.
fn focused(rec: &mut Rec, ctx: &mut Ctx, timeout: f32) -> Focus {
    let system = ctx.system.clone();
    el::set_timeout(&system, timeout);
    let ax_pid = rec
        .element(&system, "AXFocusedApplication", "core")
        .and_then(|a| el::element_pid(&a));
    let (pid, app_source) = match ax_pid {
        Some(p) => (Some(p), Some("ax")),
        None => match front_pid() {
            Some(p) => (Some(p), Some("workspace")),
            None => (None, None),
        },
    };
    let mut out = Focus {
        ax_pid,
        pid,
        app_source,
        element: None,
        source: None,
    };
    let Some(pid) = pid else { return out };
    let app = ctx.app(pid);
    el::set_timeout(&app, timeout);
    if let Some(f) = rec.element(&app, "AXFocusedUIElement", "core") {
        out.element = Some(f);
        out.source = Some("app");
        return out;
    }
    if ax_pid.is_some() {
        out.element = rec
            .element(&system, "AXFocusedUIElement", "core")
            .filter(|e| el::element_pid(e) == Some(pid));
        out.source = out.element.as_ref().map(|_| "system");
    }
    out
}

/// The focused app's family (cheap: one or two calls).
pub fn focused_family(ctx: &mut Ctx) -> Option<Family> {
    let mut rec = Rec::default();
    let pid = focused(&mut rec, ctx, 0.25).pid?;
    Some(family_of(&el::app_bundle_path(pid).unwrap_or_default()))
}

fn find_web_area(
    rec: &mut Rec,
    start: &AXUIElement,
    window: Option<&AXUIElement>,
    deadline: Instant,
) -> Option<(CFRetained<AXUIElement>, &'static str)> {
    // Up from the focused element.
    let mut cur: Option<CFRetained<AXUIElement>> = Some(start.retain());
    for _ in 0..40 {
        let Some(c) = cur.take() else { break };
        if rec.string(&c, "AXRole", "url").as_deref() == Some("AXWebArea") {
            return Some((c, "ancestor"));
        }
        cur = rec.element(&c, "AXParent", "url");
        if Instant::now() > deadline {
            return None;
        }
    }
    // Down from the window, breadth first, bounded.
    let w = window?;
    let mut queue: std::collections::VecDeque<CFRetained<AXUIElement>> =
        el::children(w).into_iter().collect();
    let mut seen = 0;
    while let Some(c) = queue.pop_front() {
        seen += 1;
        if seen > 300 || Instant::now() > deadline {
            break;
        }
        if rec.string(&c, "AXRole", "url").as_deref() == Some("AXWebArea") {
            return Some((c, "window_tree"));
        }
        queue.extend(el::children(&c));
    }
    None
}

/// Edit › Copy found by its ⌘C shortcut (not its localised title), and
/// whether it is enabled: step 3 of the selection chain is available.
fn edit_copy_enabled(rec: &mut Rec, app: &AXUIElement) -> Option<bool> {
    let bar = rec.element(app, "AXMenuBar", "menu")?;
    for top in el::children(&bar).into_iter().take(8) {
        for menu in el::children(&top) {
            for item in el::children(&menu) {
                let ch = rec.string(&item, "AXMenuItemCmdChar", "menu");
                if ch.as_deref().map(str::to_ascii_lowercase).as_deref() != Some("c") {
                    continue;
                }
                // 0 = ⌘ only (kAXMenuItemModifierNone).
                if rec.number(&item, "AXMenuItemCmdModifiers", "menu") != Some(0) {
                    continue;
                }
                return rec.boolean(&item, "AXEnabled", "menu");
            }
        }
    }
    None
}

fn value_window(rec: &mut Rec, field: &AXUIElement, low: isize, high: isize) -> Option<Vec<u16>> {
    let value = rec.string(field, "AXValue", "caret")?;
    let units: Vec<u16> = value.encode_utf16().collect();
    if units.len() > MAX_VALUE_UNITS {
        return None;
    }
    let (low, high) = (low as usize, (high as usize).min(units.len()));
    (low <= high).then(|| units[low..high].to_vec())
}

fn text_at(rec: &mut Rec, field: &AXUIElement, loc: isize, len: isize) -> Option<String> {
    let range = CFRange {
        location: loc,
        length: len,
    };
    rec.t("AXStringForRange", "set", || {
        el::string_for_range(field, range).ok_or(AXError::NoValue)
    })
    .ok()
    .or_else(|| value_window(rec, field, loc, loc + len))
    .map(|u| String::from_utf16_lossy(&u))
}

fn length_of(rec: &mut Rec, field: &AXUIElement) -> Option<i64> {
    rec.number(field, "AXNumberOfCharacters", "set")
        .or_else(|| {
            rec.string(field, "AXValue", "set")
                .map(|v| v.encode_utf16().count() as i64)
        })
}

fn set_test(
    rec: &mut Rec,
    field: &AXUIElement,
    range: CFRange,
    original_selection: Option<String>,
) -> SetTest {
    let before = length_of(rec, field);
    let write = rec.t("set:AXSelectedText", "set", || {
        match el::set_string(field, "AXSelectedText", SENTINEL) {
            AXError::Success => Ok(()),
            e => Err(e),
        }
    });
    let n = SENTINEL.encode_utf16().count() as isize;
    // Some hosts apply the write asynchronously: look for up to 500 ms.
    let (mut after, mut verified) = (None, false);
    for _ in 0..5 {
        std::thread::sleep(Duration::from_millis(100));
        after = length_of(rec, field);
        let at = text_at(rec, field, range.location, n);
        verified = at.as_deref() == Some(SENTINEL)
            || (at.is_none()
                && matches!((before, after), (Some(b), Some(a)) if a == b - range.length as i64 + n as i64));
        if verified || after != before {
            break;
        }
    }
    let changed = before != after || verified;
    let mut reverted = !changed;
    if changed {
        let _ = rec.t("set:AXSelectedTextRange", "set", || {
            match el::set_range(
                field,
                "AXSelectedTextRange",
                CFRange {
                    location: range.location,
                    length: n,
                },
            ) {
                AXError::Success => Ok(()),
                e => Err(e),
            }
        });
        let restore = original_selection.unwrap_or_default();
        let _ = rec.t(
            "set:AXSelectedText(revert)",
            "set",
            || match el::set_string(field, "AXSelectedText", &restore) {
                AXError::Success => Ok(()),
                e => Err(e),
            },
        );
        std::thread::sleep(Duration::from_millis(100));
        reverted = length_of(rec, field) == before && before.is_some();
    }
    SetTest {
        write_error: write.err().map(error_name),
        verified,
        reverted,
    }
}

/// Switches the lease as `mode` asks, then reads everything.
pub fn probe(ctx: &mut Ctx, opts: &ProbeOpts) -> Probe {
    let start = Instant::now();
    let mut rec = Rec::default();
    let secure_input = el::secure_input_on();
    let focus = focused(&mut rec, ctx, opts.timeout);
    let (pid, ax_pid, app_source) = (focus.pid, focus.ax_pid, focus.app_source);
    let (field, focus_source) = (focus.element, focus.source);
    let bundle = pid.and_then(el::app_bundle_path).unwrap_or_default();
    let family = family_of(&bundle);
    let app_name = el::bundle_name(&bundle);

    // Lease.
    let mut lease_held = false;
    let mut lease_error = None;
    let mut manual_ax_before = None;
    let mut field = field;
    let mut focus_source = focus_source;
    if let (Some(pid), true) = (pid, family.builds_tree_on_request()) {
        let app = ctx.app(pid);
        manual_ax_before = rec.boolean(&app, "AXManualAccessibility", "lease");
        match opts.lease {
            LeaseMode::Off => {
                if ctx.lease_force_off(pid) {
                    std::thread::sleep(Duration::from_millis(300));
                }
            }
            LeaseMode::On => {
                let fresh = !ctx.leases.held(pid);
                ctx.last_lease_error = None;
                let (held, _) = ctx.lease_acquire(pid, opts.timeout);
                lease_held = held;
                lease_error = ctx.last_lease_error;
                if held && fresh && manual_ax_before != Some(true) {
                    // Give the app a moment to build its tree, then look again.
                    std::thread::sleep(Duration::from_millis(300));
                }
            }
            LeaseMode::NotNeeded => {}
        }
        if opts.lease != LeaseMode::NotNeeded {
            let again = focused(&mut rec, ctx, opts.timeout);
            field = again.element;
            focus_source = again.source;
        }
    }
    let mut tree_on = lease_held || manual_ax_before == Some(true);

    let app_el = pid.map(|p| ctx.app(p));
    let window = app_el
        .as_ref()
        .and_then(|a| rec.element(a, "AXFocusedWindow", "core"));
    let window_title = window
        .as_ref()
        .and_then(|w| rec.string(w, "AXTitle", "title"));

    let mut facts = Facts {
        secure_input,
        has_focused_window: window.is_some(),
        has_focused_element: field.is_some(),
        tree_on,
        ..Facts::default()
    };
    let mut out = Probe {
        timeout: opts.timeout,
        lease: opts.lease,
        lease_held,
        lease_error,
        manual_ax_before,
        ax_app_pid: ax_pid,
        app_pid: pid,
        app_source,
        app: app_name,
        family,
        focus_source,
        facts: Facts::default(),
        verdict: Verdict::Unknown(editable::Reason::UnrecognizedRole),
        delivery: Delivery::PasteFailOpen,
        dom_id: None,
        sel_range: None,
        char_count: None,
        caret_window_units: None,
        selected: Selected::default(),
        edit_copy_enabled: None,
        caret_bounds: None,
        frame: None,
        window_title_len: window_title.as_ref().map(|t| t.chars().count()),
        window_title: window_title.filter(|t| opts.allow_text || t.starts_with(TEST_TITLE)),
        url: UrlInfo {
            kind: "none",
            ..UrlInfo::default()
        },
        set_test: None,
        total_ms: 0.0,
        calls: Vec::new(),
    };

    if let Some(f) = field.as_ref() {
        el::set_timeout(f, opts.timeout);
        facts.role = rec.string(f, "AXRole", "core");
        // A web area with children means the tree is built, whoever built it
        // (Chrome builds it for any AX client it notices, lease or not).
        if family.builds_tree_on_request() && !tree_on {
            tree_on = facts.role.as_deref() == Some("AXWebArea") && !el::children(f).is_empty()
                || facts
                    .role
                    .as_deref()
                    .is_some_and(|r| r != "AXWebArea" && r != "AXGroup" && r != "AXWindow");
            facts.tree_on = tree_on;
        }
        facts.subrole = rec.string(f, "AXSubrole", "core");
        facts.enabled = rec.boolean(f, "AXEnabled", "core");
        facts.editable_attr = rec.boolean(f, "AXEditable", "core");
        facts.protected_content = rec.boolean(f, "AXProtectedContent", "core");
        facts.value_settable = rec.settable(f, "AXValue");
        facts.selected_text_settable = rec.settable(f, "AXSelectedText");
        let range = rec
            .get(f, "AXSelectedTextRange", "core", el::value_range)
            .ok();
        facts.has_selected_range = range.is_some();
        out.frame = el::attr_rect(f).map(rect4);
        let dom = rec.string(f, "AXDOMIdentifier", "core");
        let secure = facts.subrole.as_deref() == Some("AXSecureTextField")
            || facts.protected_content == Some(true);
        let terminal = family == Family::Terminal;
        if let Some(r) = range.filter(|r| r.location >= 0 && r.length >= 0) {
            out.sel_range = Some([r.location as i64, r.length as i64]);
        }

        // Nothing is read from secure fields; terminals: lengths only.
        if !secure {
            out.char_count = rec.number(f, "AXNumberOfCharacters", "core");
            if let Some(r) = range.filter(|r| r.location >= 0 && r.length >= 0) {
                // Caret window (S-20), SpeakoFlow's ranged read.
                if !terminal {
                    let low = (r.location - WINDOW_UNITS).max(0);
                    let wanted = r.location + r.length + WINDOW_UNITS;
                    let high = out.char_count.map_or(wanted, |c| wanted.min(c as isize));
                    if high >= low {
                        let win = rec
                            .t("AXStringForRange", "caret", || {
                                el::string_for_range(
                                    f,
                                    CFRange {
                                        location: low,
                                        length: high - low,
                                    },
                                )
                                .ok_or(AXError::NoValue)
                            })
                            .ok()
                            .or_else(|| value_window(&mut rec, f, low, high));
                        out.caret_window_units = win.map(|w| w.len());
                    }
                }
                // Caret / selection bounds (pill anchoring, S-21 diff card).
                let br = CFRange {
                    location: r.location,
                    length: r.length,
                };
                out.caret_bounds = rec
                    .t("AXBoundsForRange", "bounds", || el::bounds_for_range(f, br))
                    .ok()
                    .map(rect4);
            }
            // Selected text chain: AX, then text markers.
            let ax_sel = rec.string(f, "AXSelectedText", "selection");
            let mut sel = Selected {
                ax_len: ax_sel.as_ref().map(|s| s.encode_utf16().count()),
                ..Selected::default()
            };
            let mut text = ax_sel.filter(|s| !s.is_empty());
            if text.is_some() {
                sel.source = Some("ax");
            }
            let want_marker = text.is_none() && out.sel_range.is_some_and(|r| r[1] > 0)
                || text.is_none() && family != Family::Native;
            if want_marker {
                sel.marker_supported = rec
                    .t("markers:supported", "selection", || {
                        Ok(markers::supported(f))
                    })
                    .unwrap_or(false);
                if sel.marker_supported {
                    let m = rec
                        .t("markers:selected_text", "selection", || {
                            markers::selected_text(f).ok_or(AXError::NoValue)
                        })
                        .ok();
                    sel.marker_len = m.as_ref().map(|s| s.encode_utf16().count());
                    if let Some(m) = m.filter(|s| !s.is_empty()) {
                        sel.source = Some("markers");
                        text = Some(m);
                    }
                }
            }
            if let (Some(t), Some(r)) = (text.as_ref(), out.sel_range) {
                sel.matches_range = Some(t.encode_utf16().count() as i64 == r[1]);
            }
            if opts.allow_text {
                sel.text = text.clone();
            }
            out.selected = sel;

            if opts.set_test && !terminal {
                if let Some(r) = range.filter(|r| r.location >= 0 && r.length >= 0) {
                    out.set_test = Some(set_test(&mut rec, f, r, text));
                }
            }
        }

        // URL (S-19b): the web area's AXURL, else the window's AXDocument.
        let deadline = Instant::now() + Duration::from_millis(150);
        let url = find_web_area(&mut rec, f, window.as_deref(), deadline)
            .and_then(|(area, src)| {
                rec.t("AXURL", "url", || {
                    el::attr_url(&area, "AXURL").ok_or(AXError::NoValue)
                })
                .ok()
                .map(|u| (u, src))
            })
            .or_else(|| {
                window.as_ref().and_then(|w| {
                    rec.string(w, "AXDocument", "url")
                        .map(|u| (u, "window_document"))
                })
            });
        if let Some((u, src)) = url {
            out.url = classify_url(&u, TEST_PAGE_NAME);
            out.url.source = Some(src);
        }
        if opts.allow_text || out.url.test_page {
            out.dom_id = dom;
        }
    }
    if opts.menu_copy {
        if let Some(a) = app_el.as_ref() {
            out.edit_copy_enabled = edit_copy_enabled(&mut rec, a);
        }
    }

    out.verdict = editable::verdict(&facts, family);
    out.delivery = editable::delivery(out.verdict);
    out.facts = facts;
    if lease_held {
        if let Some(p) = pid {
            ctx.lease_release(p);
        }
    }
    out.total_ms = start.elapsed().as_micros() as f64 / 1000.0;
    out.calls = rec.calls;
    out
}

/// The cheap read the session polls to notice that the owner moved focus.
#[derive(Clone, Debug, Serialize)]
pub struct Poll {
    pub ax_app_pid: Option<i32>,
    pub app_pid: Option<i32>,
    pub has_element: bool,
    /// Same element as at the last `mark`.
    pub same_as_baseline: Option<bool>,
    pub sel_range: Option<[i64; 2]>,
    pub role: Option<String>,
    /// Only test-page titles.
    pub test_title: Option<String>,
    pub ms: f64,
}

pub fn poll(ctx: &mut Ctx, mark: bool, keep_lease: bool) -> Poll {
    let start = Instant::now();
    let mut rec = Rec::default();
    let focus = focused(&mut rec, ctx, 0.1);
    let (pid, ax_pid) = (focus.pid, focus.ax_pid);
    let mut field = focus.element;
    if let (Some(p), true) = (pid, keep_lease) {
        let bundle = el::app_bundle_path(p).unwrap_or_default();
        if family_of(&bundle).builds_tree_on_request() {
            let fresh = !ctx.leases.held(p);
            ctx.lease_acquire(p, 0.1);
            ctx.lease_release(p);
            if fresh {
                std::thread::sleep(Duration::from_millis(200));
                field = focused(&mut rec, ctx, 0.1).element;
            }
        }
    }
    let same = match (&ctx.baseline, &field) {
        (Some(b), Some(f)) => Some(**b == **f),
        (None, None) => Some(true),
        _ => Some(false),
    };
    let sel_range = field.as_ref().and_then(|f| {
        el::attr_range(f, "AXSelectedTextRange").map(|r| [r.location as i64, r.length as i64])
    });
    let role = field.as_ref().and_then(|f| el::attr_string(f, "AXRole"));
    let test_title = pid
        .map(|p| ctx.app(p))
        .and_then(|a| el::attr_element(&a, "AXFocusedWindow"))
        .and_then(|w| el::attr_string(&w, "AXTitle"))
        .filter(|t| t.starts_with(TEST_TITLE));
    if mark {
        ctx.baseline = field.clone();
    }
    Poll {
        ax_app_pid: ax_pid,
        app_pid: pid,
        has_element: field.is_some(),
        same_as_baseline: if mark { None } else { same },
        sel_range,
        role,
        test_title,
        ms: start.elapsed().as_micros() as f64 / 1000.0,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn urls_keep_the_host_only() {
        let u = classify_url(
            "https://mail.example.com:443/mail/u/0/#inbox?x=1",
            TEST_PAGE_NAME,
        );
        assert_eq!(u.kind, "http");
        assert_eq!(u.host.as_deref(), Some("mail.example.com"));
        let u = classify_url("http://user@docs.example.com/d/abc", TEST_PAGE_NAME);
        assert_eq!(u.host.as_deref(), Some("docs.example.com"));
        let u = classify_url(
            "file:///Users/x/wb-bench/rs04-test-page.html#3",
            TEST_PAGE_NAME,
        );
        assert_eq!((u.kind, u.test_page), ("file", true));
        assert_eq!(classify_url("", TEST_PAGE_NAME).kind, "none");
        assert_eq!(classify_url("about:blank", TEST_PAGE_NAME).kind, "other");
    }

    #[test]
    fn families() {
        assert_eq!(
            family_of("/Applications/Google Chrome.app"),
            Family::Chromium
        );
        assert_eq!(family_of("/Applications/Safari.app"), Family::Webkit);
        assert_eq!(
            family_of("/System/Applications/Utilities/Terminal.app"),
            Family::Terminal
        );
        assert_eq!(
            family_of("/System/Applications/TextEdit.app"),
            Family::Native
        );
    }
}
