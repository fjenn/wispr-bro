// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) FluidVoice contributors (FluidVoice; GPL-3.0)
// Modified by Wispr Bro contributors, 2026-10-31, 2026-11-01 and 2026-11-02.
//
// The budgeted read of the window around the dictation is ported from
// FluidVoice `Sources/Fluid/Services/DictationContextCapture.swift`
// (https://github.com/altic-dev/FluidVoice, commit
// ea94c8a6736d453c764a2431503832b3fb134911), GPL-3.0: the privacy check
// (Secure Input, password-manager bundle markers, secure roles and
// subroles), the skipped and labelled role sets, `textEntryValue` (a text
// field contributes only its visible range, UTF-16 offsets, never the full
// value when the range is missing and the value is long), the window-frame
// visibility test, the visited-element and time budgets, a batched attribute
// fetch with an individual-attribute fallback, and `LineCollector`
// (trimmed lines with a letter, no repeats, a per-line cap). Its tests
// `testShortScrolledFieldIncludesOnlyVisibleWords`,
// `testLongScrolledFieldIncludesOnlyVisibleWords`,
// `testVisibleRangeUsesUTF16Offsets`,
// `testEmptyOrInvalidVisibleRangeNeverExposesTheFullValue`,
// `testMissingVisibleRangeKeepsOnlyTheBoundedFallback`,
// `testPasswordManagerDropsCursorAndScreenContext` and
// `testCursorContextIsBounded` are re-expressed below. Changed: Rust on the
// fork's one AX thread (`fork/ax/thread.rs`) instead of a detached Task; the
// walk starts at a focused text field and widens through its ancestors (text
// nearest the caret first), or walks the whole window for generic focus; R5's
// budget (≤ 1,500 characters before, selected and after the caret plus
// nearby lines; ≤ 100 ms or nothing); the caret window before, selected and
// after (SpeakoFlow's ranged `AXStringForRange` read, as in the RS-04 spike)
// instead of the text before the cursor only; Wispr's exclusions (URL bars,
// numeric and sensitive fields, terminals, banking and finance apps and
// sites, `exclusions.json`); no OCR fallback (it needs Screen Recording);
// the text never leaves this file: [`super::hints::extract`] keeps only
// the names in it. 2026-11-02 (S-36): in a set-up editor the same walk
// also collects code names and visible file names (`IdeScan`, its own
// budget of 8,000 characters and 1,500 elements, still within the time
// budget); only those names leave the read.
//
//! Context awareness: the read (S-20, D-58, DEC-24). At record start the AX
//! thread reads the text around the caret once, within [`READ_BUDGET`], and
//! turns it into spelling hints at once; the text is dropped there. Every
//! rule that says "do not read" is checked before any text is read.

use std::sync::OnceLock;
use std::time::{Duration, Instant};

use serde::Deserialize;

/// The whole read: over this, nothing is used (R5 S-20: "≤ 100 ms or skip").
pub const READ_BUDGET: Duration = Duration::from_millis(100);
/// The walk around the field stops here, leaving room for the last call.
pub const WALK_BUDGET: Duration = Duration::from_millis(70);
/// Messaging timeout of each call of the read (the product's 0.2 s would
/// let one hung call eat the whole budget).
pub const CALL_TIMEOUT_S: f32 = 0.05;
/// Characters read in all: before, selected and after the caret, then the
/// lines around the field (R5 S-20).
pub const MAX_CHARS: usize = 1_500;
/// The caret window's share of [`MAX_CHARS`]; what it leaves goes to the
/// lines around the field.
pub const BEFORE_CHARS: usize = 800;
pub const SELECTED_CHARS: usize = 300;
pub const AFTER_CHARS: usize = 400;
/// FluidVoice `maxLineCharacters`.
pub const MAX_LINE_CHARS: usize = 400;
/// FluidVoice `longValueCharacters`: a field longer than this that cannot
/// say what is visible contributes nothing.
pub const LONG_VALUE_UNITS: usize = 4_000;
/// Elements the walk visits at most (FluidVoice: 4,000 in 0.5 s; ours has
/// 70 ms).
pub const MAX_VISITED: usize = 600;
/// A field's whole `AXValue` is read only up to this many UTF-16 units
/// (when `AXStringForRange` is missing).
pub const MAX_VALUE_UNITS: usize = 200_000;

/// Why nothing was read (counts and names only reach the log).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Skip {
    /// Context awareness is off.
    Off,
    /// No Accessibility permission, or no frontmost app.
    NoAccess,
    /// Secure Event Input is on (a password field somewhere).
    SecureInput,
    PasswordManager,
    Finance,
    Terminal,
    /// Wispr Bro itself.
    OwnApp,
    SecureField,
    /// A browser's address or search bar.
    UrlBar,
    Numeric,
    Sensitive,
    /// The read took longer than [`READ_BUDGET`].
    Timeout,
}

impl Skip {
    pub fn as_str(self) -> &'static str {
        match self {
            Skip::Off => "off",
            Skip::NoAccess => "no_access",
            Skip::SecureInput => "secure_input",
            Skip::PasswordManager => "password_manager",
            Skip::Finance => "finance",
            Skip::Terminal => "terminal",
            Skip::OwnApp => "own_app",
            Skip::SecureField => "secure_field",
            Skip::UrlBar => "url_bar",
            Skip::Numeric => "numeric",
            Skip::Sensitive => "sensitive",
            Skip::Timeout => "timeout",
        }
    }
}

/// What a read leaves behind: bounded hints, and numbers for the log.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Read {
    pub hints: Vec<String>,
    /// S-36: code names visible in a set-up editor (nearest first).
    pub identifiers: Vec<String>,
    /// S-36: "Recognize code names" applied to this read (its dotfile pass
    /// runs even when no code name was visible).
    pub code_names: bool,
    /// S-36: file names in the editor's tabs and explorer, read only when
    /// the focus is its chat input and "Tag files in chat" is on.
    pub file_names: Vec<String>,
    /// S-36: the editor this read was for, and whether its chat input had
    /// focus (ids only, for the log).
    pub ide: Option<(super::ide::Editor, bool)>,
    /// S-36: our lease switched this Chromium/Electron app's tree on moments
    /// ago, so it may not be built yet: the read is worth repeating.
    pub warming: bool,
    /// Characters read (never kept).
    pub chars: usize,
    pub ms: f64,
    pub skip: Option<Skip>,
}

impl Read {
    pub fn skipped(skip: Skip, ms: f64) -> Read {
        Read {
            skip: Some(skip),
            ms,
            ..Read::default()
        }
    }
}

// ---------------------------------------------------------------------------
// Exclusions (`exclusions.json`)

#[derive(Debug, Deserialize)]
struct ExclusionsFile {
    password_manager_markers: Vec<String>,
    bundle_ids: Vec<String>,
    finance_markers: Vec<String>,
    finance_hosts: Vec<String>,
    sensitive_words: Vec<String>,
}

/// The exclusion lists, lower case.
#[derive(Debug)]
pub struct Exclusions {
    password_manager_markers: Vec<String>,
    bundle_ids: Vec<String>,
    finance_markers: Vec<String>,
    finance_hosts: Vec<String>,
    sensitive_words: Vec<String>,
}

pub fn exclusions() -> &'static Exclusions {
    static E: OnceLock<Exclusions> = OnceLock::new();
    E.get_or_init(|| {
        let f: ExclusionsFile = serde_json::from_str(include_str!("exclusions.json"))
            .expect("exclusions.json is valid (tested)");
        let low = |v: Vec<String>| v.into_iter().map(|s| s.to_lowercase()).collect();
        Exclusions {
            password_manager_markers: low(f.password_manager_markers),
            bundle_ids: low(f.bundle_ids),
            finance_markers: low(f.finance_markers),
            finance_hosts: low(f.finance_hosts),
            sensitive_words: low(f.sensitive_words),
        }
    })
}

/// `pid` is this process (Wispr Bro itself, bundled or not).
pub(crate) fn is_own_pid(pid: i32) -> bool {
    crate::fork::ax::is_own_pid(pid)
}

fn words_of(s: &str) -> Vec<String> {
    s.to_lowercase()
        .split(|c: char| !c.is_alphanumeric())
        .filter(|w| !w.is_empty())
        .map(str::to_owned)
        .collect()
}

impl Exclusions {
    /// The app may not be read: a password manager, a banking or finance
    /// app, a terminal, or Wispr Bro itself.
    pub fn app(&self, bundle_id: Option<&str>, name: Option<&str>, terminal: bool) -> Option<Skip> {
        let id = bundle_id.unwrap_or("").to_lowercase();
        if terminal {
            return Some(Skip::Terminal);
        }
        if id.starts_with("com.wisprbro.") {
            return Some(Skip::OwnApp);
        }
        let name_words = words_of(name.unwrap_or(""));
        if self
            .password_manager_markers
            .iter()
            .any(|m| id.contains(m.as_str()))
            || name_words.iter().any(|w| {
                self.password_manager_markers
                    .iter()
                    .any(|m| !m.contains('.') && w == m)
            })
        {
            return Some(Skip::PasswordManager);
        }
        if self.bundle_ids.contains(&id) {
            return Some(Skip::Finance);
        }
        let segments: Vec<&str> = id.split(['.', '-', '_']).collect();
        if self.finance_markers.iter().any(|m| {
            // Segments may carry a marker inside a longer word
            // ("mobilebanking"); over-matching only reads less, which is
            // the safe direction for a privacy exclusion.
            segments.iter().any(|s| s.contains(m.as_str())) || name_words.contains(m)
        }) {
            return Some(Skip::Finance);
        }
        None
    }

    /// A page on a banking or finance site (`host` of the page's URL).
    pub fn host(&self, host: &str) -> bool {
        let host = host.trim_end_matches('.').to_lowercase();
        let host = host.strip_prefix("www.").unwrap_or(&host);
        if host.split('.').any(|label| label.contains("bank")) {
            return true;
        }
        self.finance_hosts
            .iter()
            .any(|h| host == h || host.ends_with(&format!(".{h}")))
    }

    /// A field whose label, placeholder or identifier names a secret.
    pub fn sensitive_label(&self, label: &str) -> bool {
        let words = words_of(label);
        if words.is_empty() {
            return false;
        }
        let joined = format!(" {} ", words.join(" "));
        self.sensitive_words.iter().any(|w| {
            let phrase = words_of(w).join(" ");
            !phrase.is_empty() && joined.contains(&format!(" {phrase} "))
        })
    }
}

/// The host of a URL (`https://huggingface.co/x` → `huggingface.co`).
pub fn url_host(url: &str) -> Option<String> {
    let rest = url.split_once("://")?.1;
    let authority = rest.split(['/', '?', '#']).next()?;
    let host = authority.rsplit('@').next()?.split(':').next()?;
    (!host.is_empty()).then(|| host.to_ascii_lowercase())
}

/// A focused element that is a number field (Wispr: numeric fields are
/// never read). Stepper inputs report `AXIncrementor`; WebKit reports a
/// number input's spin buttons the same way.
pub fn is_numeric_role(role: &str, subrole: &str) -> bool {
    role == "AXIncrementor"
        || role == "AXStepper"
        || role == "AXDateTimeArea"
        || subrole.contains("Number")
}

/// FluidVoice `isSecure`: a secure role, the secure subrole, or any subrole
/// that says "secure".
pub fn is_secure_role(role: &str, subrole: &str) -> bool {
    role == "AXSecureTextField"
        || subrole == "AXSecureTextField"
        || subrole.to_lowercase().contains("secure")
}

pub(crate) fn field_exclusion(
    role: &str,
    subrole: &str,
    protected: bool,
    labels: &str,
) -> Option<Skip> {
    if is_secure_role(role, subrole) || protected {
        return Some(Skip::SecureField);
    }
    if is_numeric_role(role, subrole) {
        return Some(Skip::Numeric);
    }
    if exclusions().sensitive_label(labels) {
        return Some(Skip::Sensitive);
    }
    None
}

/// FluidVoice `skippedRoles`.
pub const SKIPPED_ROLES: &[&str] = &[
    "AXSecureTextField",
    "AXScrollBar",
    "AXValueIndicator",
    "AXSplitter",
    "AXGrowArea",
    "AXIncrementor",
    "AXMenuBar",
    "AXMenu",
];

/// FluidVoice `labelledRoles`: their description is their visible label.
pub const LABELLED_ROLES: &[&str] = &[
    "AXButton",
    "AXLink",
    "AXImage",
    "AXMenuButton",
    "AXPopUpButton",
    "AXRadioButton",
    "AXCheckBox",
    "AXTab",
];

/// FluidVoice `textEntryRoles`.
pub const TEXT_ENTRY_ROLES: &[&str] = &["AXTextArea", "AXTextField", "AXComboBox"];

fn accepts_walk_value(role: &str) -> bool {
    role != "AXWebArea"
}

fn browser_context_skip(browser: bool, inspectable_page: bool) -> Option<Skip> {
    (browser && !inspectable_page).then_some(Skip::NoAccess)
}

// ---------------------------------------------------------------------------
// The caret window and the lines around it (pure)

/// FluidVoice `textEntryValue`: the visible part of a text field's value
/// (UTF-16 offsets); without a range, the value only if it is short.
pub fn text_entry_value(value: &str, visible: Option<(isize, isize)>) -> String {
    let units: Vec<u16> = value.encode_utf16().collect();
    let Some((location, length)) = visible else {
        return if units.len() <= LONG_VALUE_UNITS {
            value.to_owned()
        } else {
            String::new()
        };
    };
    let len = units.len() as isize;
    if location < 0 || length <= 0 || location > len || length > len - location {
        return String::new();
    }
    String::from_utf16_lossy(&units[location as usize..(location + length) as usize])
}

/// The last `n` characters of `s`.
pub fn last_chars(s: &str, n: usize) -> &str {
    match s.char_indices().rev().nth(n.saturating_sub(1)) {
        Some((at, _)) if n > 0 => &s[at..],
        _ if n == 0 => "",
        _ => s,
    }
}

/// The first `n` characters of `s`.
pub fn first_chars(s: &str, n: usize) -> &str {
    match s.char_indices().nth(n) {
        Some((at, _)) => &s[..at],
        None => s,
    }
}

/// The caret window, trimmed to its budget.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct CaretWindow {
    pub before: String,
    pub selected: String,
    pub after: String,
}

impl CaretWindow {
    pub fn new(before: &str, selected: &str, after: &str) -> CaretWindow {
        CaretWindow {
            before: last_chars(before, BEFORE_CHARS).to_owned(),
            selected: first_chars(selected, SELECTED_CHARS).to_owned(),
            after: first_chars(after, AFTER_CHARS).to_owned(),
        }
    }

    pub fn chars(&self) -> usize {
        self.before.chars().count() + self.selected.chars().count() + self.after.chars().count()
    }
}

/// The UTF-16 ranges to read around a selection at `location`, `length` in
/// a field of `total` units (`None`: unknown): before, selected, after.
/// Twice the character budget in units covers any text (surrogate pairs
/// are two units); [`CaretWindow::new`] trims to characters.
pub fn caret_ranges(location: isize, length: isize, total: Option<isize>) -> [(isize, isize); 3] {
    let location = location.max(0);
    let length = length.max(0);
    let before_lo = (location - 2 * BEFORE_CHARS as isize).max(0);
    let selected_len = length.min(2 * SELECTED_CHARS as isize);
    let after_lo = location + length;
    let mut after_len = 2 * AFTER_CHARS as isize;
    if let Some(t) = total {
        after_len = after_len.min((t - after_lo).max(0));
    }
    [
        (before_lo, location - before_lo),
        (location, selected_len),
        (after_lo, after_len),
    ]
}

/// S-36: characters of walked text an editor read may scan for code and
/// file names after the lines' own budget is spent. Only the names leave
/// the read; the text is dropped as it is scanned.
pub const IDE_SCAN_CHARS: usize = 8_000;
/// S-36: elements an editor read may visit (Electron trees are deep); the
/// time budget still bounds it.
pub const IDE_MAX_VISITED: usize = 1_500;

/// S-36: the code and file names an editor read collects while it walks.
#[derive(Debug, Default)]
pub struct IdeScan {
    /// `Some` when "Recognize code names" applies to this take.
    identifiers: Option<super::ide::Identifiers>,
    /// `Some` when the focus is a chat input and "Tag files in chat" is on.
    files: Option<crate::fork::text::file_tags::FileNames>,
    left: usize,
}

impl IdeScan {
    /// `None` when neither feature applies.
    pub fn new(identifiers: bool, files: bool) -> Option<IdeScan> {
        (identifiers || files).then(|| IdeScan {
            identifiers: identifiers.then(Default::default),
            files: files.then(Default::default),
            left: IDE_SCAN_CHARS,
        })
    }

    fn wants_more(&self) -> bool {
        self.left > 0
            && (self.identifiers.as_ref().is_some_and(|i| !i.is_full())
                || self.files.as_ref().is_some_and(|f| !f.is_full()))
    }

    fn scan(&mut self, text: &str, file_row: bool) {
        if self.left == 0 {
            return;
        }
        let text = first_chars(text, self.left);
        self.left -= text.chars().count();
        if let Some(ids) = self.identifiers.as_mut() {
            ids.push(text);
            if file_row {
                ids.push_dotnames(text);
            }
        }
        if file_row {
            if let Some(files) = self.files.as_mut() {
                files.push(text);
            }
        }
    }
}

/// FluidVoice `LineCollector`, with a character budget: trimmed lines that
/// have a letter, each once, at most [`MAX_LINE_CHARS`] each. With an
/// [`IdeScan`] (S-36), walked text is also scanned for code and file names,
/// within its own budget.
#[derive(Debug)]
pub struct LineCollector {
    lines: Vec<String>,
    seen: std::collections::HashSet<String>,
    left: usize,
    ide: Option<IdeScan>,
}

impl LineCollector {
    pub fn new(budget: usize) -> LineCollector {
        LineCollector::with_ide(budget, None)
    }

    pub fn with_ide(budget: usize, ide: Option<IdeScan>) -> LineCollector {
        LineCollector {
            lines: Vec::new(),
            seen: Default::default(),
            left: budget,
            ide,
        }
    }

    pub fn is_full(&self) -> bool {
        self.left == 0
    }

    /// The walk may stop: the lines are full and no editor scan wants more.
    fn done(&self) -> bool {
        self.is_full() && !self.ide.as_ref().is_some_and(IdeScan::wants_more)
    }

    fn max_visited(&self) -> usize {
        if self.ide.is_some() {
            IDE_MAX_VISITED
        } else {
            MAX_VISITED
        }
    }

    /// One walked element's text: the lines, and the editor scan.
    fn add_walked(&mut self, text: &str, file_row: bool) {
        self.add(text);
        if let Some(ide) = self.ide.as_mut() {
            ide.scan(text, file_row);
        }
    }

    pub fn add(&mut self, text: &str) {
        for raw in text.lines() {
            if self.is_full() {
                return;
            }
            let line = first_chars(raw.trim(), MAX_LINE_CHARS.min(self.left));
            if !line.chars().any(char::is_alphabetic) || !self.seen.insert(line.to_owned()) {
                continue;
            }
            self.left -= line.chars().count();
            self.lines.push(line.to_owned());
        }
    }

    pub fn chars(&self) -> usize {
        self.lines.iter().map(|l| l.chars().count()).sum()
    }

    pub fn into_lines(self) -> Vec<String> {
        self.lines
    }

    fn into_parts(self) -> (Vec<String>, Option<IdeScan>) {
        (self.lines, self.ide)
    }
}

struct WalkItem<T> {
    role: String,
    exclusion: Option<Skip>,
    texts: Vec<String>,
    visible: bool,
    /// S-36: an editor tab or a file explorer row (its subtree's text may
    /// name a file).
    file_row: bool,
    children: Vec<T>,
}

/// S-36: an element whose text may be a visible file name: a tab
/// (Chromium maps ARIA tabs to radio buttons with the tab-button subrole)
/// or a row of a list, outline or table (the explorer).
pub fn is_file_row(role: &str, subrole: &str) -> bool {
    matches!(role, "AXTab" | "AXRow" | "AXOutlineRow" | "AXCell")
        || (role == "AXRadioButton" && subrole == "AXTabButton")
}

fn walk_roots<T: PartialEq>(
    roots: Vec<(T, Option<T>)>,
    include_root_text: bool,
    lines: &mut LineCollector,
    deadline: Instant,
    mut inspect: impl FnMut(&T) -> WalkItem<T>,
) -> Result<(), Skip> {
    let mut visited = 0usize;
    let max_visited = lines.max_visited();
    'roots: for (root, skip) in roots {
        let mut stack = vec![(root, false)];
        let mut first = true;
        while let Some((e, in_row)) = stack.pop() {
            if visited >= max_visited || lines.done() || Instant::now() > deadline {
                break 'roots;
            }
            visited += 1;
            let item = inspect(&e);
            if let Some(skip) = item.exclusion {
                return Err(skip);
            }
            let skip_text = SKIPPED_ROLES.contains(&item.role.as_str()) || !item.visible;
            let row = in_row || item.file_row;
            if (!first || include_root_text) && !skip_text {
                for text in &item.texts {
                    lines.add_walked(text, row);
                }
            }
            first = false;
            for child in item.children.into_iter().rev() {
                if skip.as_ref().is_some_and(|s| s == &child) {
                    continue;
                }
                stack.push((child, row));
            }
        }
    }
    Ok(())
}

fn sections<'a>(window: &'a CaretWindow, lines: &'a [String]) -> Vec<&'a str> {
    // The text just before the caret is the nearest: read it last line first.
    let before_lines: Vec<&str> = window.before.lines().rev().collect();
    let mut sections: Vec<&str> = Vec::with_capacity(before_lines.len() + lines.len() + 2);
    sections.extend(before_lines);
    sections.push(&window.selected);
    sections.push(&window.after);
    sections.extend(lines.iter().map(String::as_str));
    sections
}

/// Hints from a caret window and the lines around it, nearest the caret
/// first. The text goes no further than this call.
pub fn hints_of(window: &CaretWindow, lines: &[String], languages: &[String]) -> Vec<String> {
    super::hints::extract(&sections(window, lines), languages)
}

fn read_from_walk(
    caret: CaretWindow,
    walk: Result<LineCollector, Skip>,
    languages: &[String],
) -> Read {
    let (lines, ide) = match walk {
        Ok(lines) => lines.into_parts(),
        Err(skip) => return Read::skipped(skip, 0.0),
    };
    let chars = caret.chars() + lines.iter().map(|l| l.chars().count()).sum::<usize>();
    let hints = hints_of(&caret, &lines, languages);
    let IdeScan {
        identifiers, files, ..
    } = ide.unwrap_or_default();
    let code_names = identifiers.is_some();
    // Code names: the caret window first (nearest), then the walk's.
    let identifiers = identifiers
        .map(|walked| {
            let mut ids = super::ide::Identifiers::default();
            for section in sections(&caret, &[]) {
                ids.push(section);
            }
            for id in walked.into_vec() {
                if id.starts_with('.') {
                    ids.push_dotnames(&id);
                } else {
                    ids.push(&id);
                }
            }
            ids.into_vec()
        })
        .unwrap_or_default();
    let file_names = files.map(|f| f.into_vec()).unwrap_or_default();
    // `caret` and `lines` (the text) are dropped here.
    Read {
        hints,
        identifiers,
        code_names,
        file_names,
        ide: None,
        warming: false,
        chars,
        ms: 0.0,
        skip: None,
    }
}

// ---------------------------------------------------------------------------
// The read (macOS, on the AX thread)

#[cfg(target_os = "macos")]
pub mod mac {
    use std::ffi::c_void;
    use std::ptr::NonNull;
    use std::time::Instant;

    use objc2_app_kit::NSWorkspace;
    use objc2_application_services::{
        AXCopyMultipleAttributeOptions, AXUIElement, AXValue, AXValueType,
    };
    use objc2_core_foundation::Type;
    use objc2_core_foundation::{
        CFArray, CFBoolean, CFRange, CFRetained, CFString, CFType, CGPoint, CGRect, CGSize,
    };

    use super::super::ide::Access;
    use super::*;
    use crate::fork::ax::editable::Family;
    use crate::fork::ax::element as el;
    use crate::fork::ax::thread::Ctx;
    use crate::fork::settings::VibeCodingSettings;

    /// The frontmost app: pid, bundle id, name.
    fn frontmost() -> Option<(i32, Option<String>, Option<String>)> {
        let app = NSWorkspace::sharedWorkspace().frontmostApplication()?;
        let pid = app.processIdentifier();
        (pid > 0).then(|| {
            (
                pid,
                app.bundleIdentifier().map(|s| s.to_string()),
                app.localizedName().map(|s| s.to_string()),
            )
        })
    }

    fn string(e: &AXUIElement, name: &str) -> String {
        el::attr_string(e, name).unwrap_or_default()
    }

    fn point(v: &CFType) -> Option<CGPoint> {
        let v = v.downcast_ref::<AXValue>()?;
        let mut p = CGPoint::default();
        // SAFETY: `p` matches the CGPoint AXValue type asked for.
        let ok = unsafe { v.value(AXValueType::CGPoint, NonNull::from(&mut p).cast::<c_void>()) };
        ok.then_some(p)
    }

    fn size(v: &CFType) -> Option<CGSize> {
        let v = v.downcast_ref::<AXValue>()?;
        let mut s = CGSize::default();
        // SAFETY: `s` matches the CGSize AXValue type asked for.
        let ok = unsafe { v.value(AXValueType::CGSize, NonNull::from(&mut s).cast::<c_void>()) };
        ok.then_some(s)
    }

    fn intersects(a: &CGRect, b: &CGRect) -> bool {
        a.origin.x < b.origin.x + b.size.width
            && b.origin.x < a.origin.x + a.size.width
            && a.origin.y < b.origin.y + b.size.height
            && b.origin.y < a.origin.y + a.size.height
    }

    const FETCHED: [&str; 14] = [
        "AXRole",
        "AXSubrole",
        "AXValue",
        "AXTitle",
        "AXDescription",
        "AXPosition",
        "AXSize",
        "AXChildren",
        "AXVisibleCharacterRange",
        "AXPlaceholderValue",
        "AXIdentifier",
        "AXDOMIdentifier",
        "AXHelp",
        "AXProtectedContent",
    ];

    fn element_text(
        e: &AXUIElement,
        names: &CFArray,
        window_frame: Option<&CGRect>,
    ) -> WalkItem<CFRetained<AXUIElement>> {
        let mut out: *const CFArray = std::ptr::null();
        // SAFETY: valid out-pointer; +1 reference on success.
        let err = unsafe {
            e.copy_multiple_attribute_values(
                names,
                AXCopyMultipleAttributeOptions(0),
                NonNull::from(&mut out),
            )
        };
        let multiple = if err == objc2_application_services::AXError::Success {
            NonNull::new(out as *mut CFArray).map(|ptr| unsafe { CFRetained::from_raw(ptr) })
        } else {
            None
        };
        let values = multiple.and_then(|values| {
            if values.len() != FETCHED.len() {
                return None;
            }
            // SAFETY: an array of CFTypes; every entry is checked by downcast.
            let values: &CFArray<CFType> = unsafe { values.cast_unchecked() };
            Some(
                (0..FETCHED.len())
                    .map(|i| values.get(i))
                    .collect::<Vec<_>>(),
            )
        });
        let values = values.unwrap_or_else(|| {
            FETCHED
                .iter()
                .map(|name| el::attr(e, name).ok())
                .collect::<Vec<_>>()
        });
        let get = |i: usize| values.get(i).and_then(|v| v.as_deref());
        let text =
            |i: usize| get(i).and_then(|v| v.downcast_ref::<CFString>().map(|s| s.to_string()));
        let role = text(0).unwrap_or_default();
        let subrole = text(1).unwrap_or_default();
        let labels = [3, 4, 9, 10, 11, 12]
            .into_iter()
            .filter_map(text)
            .collect::<Vec<_>>()
            .join(" ");
        let protected =
            get(13).is_some_and(|v| v.downcast_ref::<CFBoolean>().is_some_and(|b| b.as_bool()));
        let exclusion = field_exclusion(&role, &subrole, protected, &labels);
        let mut texts = Vec::new();
        if accepts_walk_value(&role) {
            if let Some(value) = text(2) {
                if TEXT_ENTRY_ROLES.contains(&role.as_str()) {
                    let visible = get(8)
                        .and_then(el::value_range)
                        .map(|r| (r.location, r.length));
                    texts.push(text_entry_value(&value, visible));
                } else {
                    texts.push(value);
                }
            }
        }
        if let Some(title) = text(3) {
            texts.push(title);
        }
        if LABELLED_ROLES.contains(&role.as_str()) {
            if let Some(d) = text(4) {
                texts.push(d);
            }
        }
        let frame = match (get(5).and_then(point), get(6).and_then(size)) {
            (Some(origin), Some(size)) => Some(CGRect { origin, size }),
            _ => None,
        };
        let visible = match (window_frame, frame.as_ref()) {
            (Some(window), Some(frame)) if frame.size.width > 0.0 && frame.size.height > 0.0 => {
                intersects(window, frame)
            }
            _ => true,
        };
        let children = get(7)
            .and_then(|v| v.downcast_ref::<CFArray>())
            .map(|a| {
                // SAFETY: AXChildren is an array of AXUIElements; checked.
                let a: &CFArray<CFType> = unsafe { a.cast_unchecked() };
                (0..a.len())
                    .filter_map(|i| a.get(i))
                    .filter_map(|v| v.downcast::<AXUIElement>().ok())
                    .collect()
            })
            .unwrap_or_default();
        WalkItem {
            exclusion,
            file_row: is_file_row(&role, &subrole),
            role,
            texts,
            visible,
            children,
        }
    }

    /// A web element's DOM classes, space-separated (empty elsewhere).
    fn class_list(e: &AXUIElement) -> String {
        let Ok(value) = el::attr(e, "AXDOMClassList") else {
            return String::new();
        };
        let Some(list) = value.downcast_ref::<CFArray>() else {
            return String::new();
        };
        // SAFETY: an array of CFTypes; every entry is checked by downcast.
        let list: &CFArray<CFType> = unsafe { list.cast_unchecked() };
        (0..list.len().min(16))
            .filter_map(|i| list.get(i))
            .filter_map(|v| v.downcast_ref::<CFString>().map(|s| s.to_string()))
            .collect::<Vec<_>>()
            .join(" ")
    }

    /// S-36: the focused field's own DOM classes, and the DOM ids and
    /// classes of its nearest ancestors (Cursor's agent input says only
    /// "Prompt"; its panel is `workbench.panel.aichat.<id>`). Names only:
    /// no text is read here.
    fn chat_labels(field: &AXUIElement) -> (String, String) {
        let own = class_list(field);
        let mut container = Vec::new();
        let mut node = el::attr_element(field, "AXParent");
        while let Some(n) = node {
            if container.len() >= super::super::ide::CHAT_ANCESTORS {
                break;
            }
            el::set_timeout(&n, CALL_TIMEOUT_S);
            let role = string(&n, "AXRole");
            if role == "AXWindow" || role == "AXApplication" || role == "AXWebArea" {
                break;
            }
            container.push(format!(
                "{} {}",
                string(&n, "AXDOMIdentifier"),
                class_list(&n)
            ));
            node = el::attr_element(&n, "AXParent");
        }
        (own, container.join(" "))
    }

    /// The parents of `e`, nearest first, up to (not including) its window.
    fn ancestors(e: &AXUIElement) -> Vec<CFRetained<AXUIElement>> {
        let mut out = Vec::new();
        let mut node = el::attr_element(e, "AXParent");
        while let Some(n) = node {
            el::set_timeout(&n, CALL_TIMEOUT_S);
            let role = string(&n, "AXRole");
            if role == "AXWindow" || role == "AXApplication" || out.len() >= 40 {
                break;
            }
            node = el::attr_element(&n, "AXParent");
            out.push(n);
        }
        out
    }

    fn range_text(
        e: &AXUIElement,
        (location, length): (isize, isize),
        value: &mut Option<Option<Vec<u16>>>,
    ) -> String {
        if length <= 0 {
            return String::new();
        }
        let range = CFRange { location, length };
        if let Some(units) = el::string_for_range(e, range) {
            return String::from_utf16_lossy(&units);
        }
        // SpeakoFlow's fallback: a slice of `AXValue` (read once).
        let units = value.get_or_insert_with(|| {
            el::attr_string(e, "AXValue")
                .map(|v| v.encode_utf16().collect::<Vec<u16>>())
                .filter(|u| u.len() <= MAX_VALUE_UNITS)
        });
        let Some(units) = units.as_ref() else {
            return String::new();
        };
        let lo = (location as usize).min(units.len());
        let hi = ((location + length) as usize).min(units.len());
        String::from_utf16_lossy(&units[lo..hi])
    }

    /// The caret window of a text field (nothing without a selection range).
    fn caret_window(e: &AXUIElement) -> CaretWindow {
        let Some(sel) = el::attr_range(e, "AXSelectedTextRange") else {
            return CaretWindow::default();
        };
        let total = el::attr_isize(e, "AXNumberOfCharacters");
        let [b, s, a] = caret_ranges(sel.location, sel.length, total);
        let mut value = None;
        CaretWindow::new(
            &range_text(e, b, &mut value),
            &range_text(e, s, &mut value),
            &range_text(e, a, &mut value),
        )
    }

    /// The lines around the field: its siblings first, then each ancestor's
    /// other children, within the walk budget (FluidVoice's walk).
    fn lines_around(
        window: &AXUIElement,
        field: Option<&AXUIElement>,
        budget: usize,
        ide: Option<IdeScan>,
        deadline: Instant,
    ) -> Result<LineCollector, Skip> {
        let names: Vec<CFRetained<CFString>> =
            FETCHED.iter().map(|n| CFString::from_str(n)).collect();
        let names = CFArray::from_retained_objects(&names);
        // SAFETY: an array of CFStrings is an array of CFTypes.
        let names: &CFArray = unsafe { names.cast_unchecked() };
        let mut lines = LineCollector::with_ide(budget, ide);
        el::set_timeout(window, CALL_TIMEOUT_S);
        let window_frame = el::attr_rect(window);
        // The roots to walk, nearest the field first; the subtree already
        // walked (the field, then each ancestor) is skipped.
        let mut roots: Vec<(CFRetained<AXUIElement>, Option<CFRetained<AXUIElement>>)> = Vec::new();
        match field {
            Some(f) => {
                let mut came_from: CFRetained<AXUIElement> = f.retain();
                for a in ancestors(f) {
                    roots.push((a.clone(), Some(came_from)));
                    came_from = a;
                }
                roots.push((window.retain(), Some(came_from)));
            }
            None => roots.push((window.retain(), None)),
        }
        walk_roots(roots, field.is_none(), &mut lines, deadline, |e| {
            el::set_timeout(e, CALL_TIMEOUT_S);
            element_text(e, names, window_frame.as_ref())
        })?;
        Ok(lines)
    }

    /// One read, on the AX thread. `languages`: the user's spoken languages
    /// (their stopwords are never hints); `vibe`: the vibe-coding settings
    /// (S-36), applied when the frontmost app is a listed editor.
    pub fn read(ctx: &mut Ctx, languages: &[String], vibe: &VibeCodingSettings) -> Read {
        let start = Instant::now();
        let ms = |s: Instant| s.elapsed().as_micros() as f64 / 1000.0;
        let Some((pid, bundle_id, name)) = frontmost() else {
            return Read::skipped(Skip::NoAccess, ms(start));
        };
        // Our own window in front (the Hub, the Scratchpad note): never read
        // it. An unbundled build (`tauri dev`, `--no-bundle`) has no bundle
        // id for the exclusion below, and AX calls into this process from
        // the AX thread reach WebKit off the main thread, which crashes the
        // app (`crashDueToApplicationCallingMainThreadOnlyWebKitAPIFromBackgroundThread`).
        if super::is_own_pid(pid) {
            return Read::skipped(Skip::OwnApp, ms(start));
        }
        let ide = super::super::ide::access(vibe, bundle_id.as_deref());
        if el::secure_input_on() {
            return Read::skipped(Skip::SecureInput, ms(start));
        }
        let bundle = el::app_bundle_path(pid).unwrap_or_default();
        let family = el::family_of(&bundle);
        if let Some(skip) = exclusions().app(
            bundle_id.as_deref(),
            name.as_deref(),
            family == Family::Terminal,
        ) {
            return Read::skipped(skip, ms(start));
        }
        let lease = family.builds_tree_on_request() && ctx.lease_acquire(pid);
        let warming = lease && ctx.tree_warming(pid);
        let read = read_app(ctx, pid, family, languages, start, ide);
        if lease {
            // The lease itself stays 60 s after its last reader (lease.rs).
            ctx.lease_release(pid);
        }
        let took = ms(start);
        if start.elapsed() > READ_BUDGET {
            return Read {
                warming,
                ..Read::skipped(Skip::Timeout, took)
            };
        }
        Read {
            ms: took,
            warming,
            ..read
        }
    }

    pub(super) fn narrows_walk_to_focused_field(role: &str) -> bool {
        TEXT_ENTRY_ROLES.contains(&role) || role == "AXSearchField"
    }

    pub(super) fn focused_caret(role: &str, read: impl FnOnce() -> CaretWindow) -> CaretWindow {
        if narrows_walk_to_focused_field(role) {
            read()
        } else {
            CaretWindow::default()
        }
    }

    fn read_app(
        ctx: &mut Ctx,
        pid: i32,
        family: Family,
        languages: &[String],
        start: Instant,
        ide: Option<Access>,
    ) -> Read {
        let app = ctx.app(pid);
        el::set_timeout(&app, CALL_TIMEOUT_S);
        let window = el::attr_element(&app, "AXFocusedWindow");
        let field = el::attr_element(&app, "AXFocusedUIElement");
        let mut walk_field = None;
        let mut caret = CaretWindow::default();
        let mut chat = false;
        let browser = matches!(family, Family::Chromium | Family::Webkit | Family::Gecko);
        if field.is_none() {
            if let Some(skip) = browser_context_skip(browser, false) {
                return Read::skipped(skip, 0.0);
            }
        }
        if let Some(f) = field.as_deref() {
            el::set_timeout(f, CALL_TIMEOUT_S);
            let role = string(f, "AXRole");
            let subrole = string(f, "AXSubrole");
            let labels = [
                "AXDescription",
                "AXTitle",
                "AXPlaceholderValue",
                "AXIdentifier",
                "AXDOMIdentifier",
                "AXHelp",
            ]
            .iter()
            .map(|a| string(f, a))
            .collect::<Vec<_>>()
            .join(" ");
            if let Some(skip) = field_exclusion(
                &role,
                &subrole,
                el::attr_bool(f, "AXProtectedContent") == Some(true),
                &labels,
            ) {
                return Read::skipped(skip, 0.0);
            }
            chat = ide.is_some_and(|access| {
                access.file_tags && {
                    let (own, container) = chat_labels(f);
                    super::super::ide::is_chat_input(&role, &format!("{labels} {own}"), &container)
                }
            });
            if browser {
                // Browser chrome is not page context. A text field outside
                // any page is the address or search bar; a banking site is
                // not read.
                let up = ancestors(f);
                let web = if role == "AXWebArea" {
                    Some(f.retain())
                } else {
                    up.iter()
                        .find(|a| string(a, "AXRole") == "AXWebArea")
                        .cloned()
                };
                match web {
                    None if TEXT_ENTRY_ROLES.contains(&role.as_str())
                        || role == "AXSearchField" =>
                    {
                        return Read::skipped(Skip::UrlBar, 0.0);
                    }
                    None => {
                        if let Some(skip) = browser_context_skip(browser, false) {
                            return Read::skipped(skip, 0.0);
                        }
                    }
                    Some(w) => {
                        let host = el::attr_url(&w, "AXURL").and_then(|u| url_host(&u));
                        if host.is_some_and(|h| exclusions().host(&h)) {
                            return Read::skipped(Skip::Finance, 0.0);
                        }
                    }
                }
            }
            if narrows_walk_to_focused_field(&role) {
                walk_field = Some(f);
            }
            caret = focused_caret(&role, || caret_window(f));
        }
        let left = MAX_CHARS.saturating_sub(caret.chars());

        let scan = || IdeScan::new(ide.is_some_and(|a| a.recognize), chat);
        let walk = match window.as_deref() {
            Some(w) if (left > 0 || scan().is_some()) && start.elapsed() < WALK_BUDGET => {
                lines_around(w, walk_field, left, scan(), start + WALK_BUDGET)
            }
            _ => Ok(LineCollector::with_ide(left, scan())),
        };
        Read {
            ide: ide.map(|a| (a.editor, chat)),
            ..read_from_walk(caret, walk, languages)
        }
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn own_process_is_recognised_without_a_bundle_id() {
        assert!(super::is_own_pid(std::process::id() as i32));
        assert!(!super::is_own_pid(0));
        assert!(!super::is_own_pid(-1));
    }

    use super::*;

    #[test]
    fn exclusions_load_and_match() {
        let e = exclusions();
        // FluidVoice testPasswordManagerDropsCursorAndScreenContext.
        assert_eq!(
            e.app(Some("com.1password.1password"), None, false),
            Some(Skip::PasswordManager)
        );
        assert_eq!(
            e.app(Some("com.bitwarden.desktop"), Some("Bitwarden"), false),
            Some(Skip::PasswordManager)
        );
        assert_eq!(
            e.app(Some("com.apple.Passwords"), Some("Passwords"), false),
            Some(Skip::PasswordManager)
        );
        assert_eq!(
            e.app(Some("com.apple.keychainaccess"), None, false),
            Some(Skip::PasswordManager)
        );
        assert_eq!(
            e.app(Some("com.revolut.Revolut"), Some("Revolut"), false),
            Some(Skip::Finance)
        );
        assert_eq!(
            e.app(Some("nl.ing.mobilebanking"), None, false),
            Some(Skip::Finance)
        );
        assert_eq!(
            e.app(Some("com.example.app"), Some("My Bank"), false),
            Some(Skip::Finance)
        );
        assert_eq!(
            e.app(Some("com.apple.Terminal"), None, true),
            Some(Skip::Terminal)
        );
        assert_eq!(
            e.app(Some("com.wisprbro.desktop"), None, false),
            Some(Skip::OwnApp)
        );
        // Ordinary apps are read.
        for (id, name) in [
            ("com.tinyspeck.slackmacgap", "Slack"),
            ("com.apple.TextEdit", "TextEdit"),
            ("com.google.Chrome", "Google Chrome"),
            ("com.apple.mail", "Mail"),
            ("com.microsoft.Word", "Microsoft Word"),
            ("com.apple.Notes", "Notes"),
        ] {
            assert_eq!(e.app(Some(id), Some(name), false), None, "{id}");
        }
        assert_eq!(e.app(None, None, false), None);
    }

    #[test]
    fn finance_hosts_and_bank_labels() {
        let e = exclusions();
        for h in [
            "www.paypal.com",
            "secure.chase.com",
            "mijn.ing.nl",
            "online.mybank.example",
            "app.ynab.com",
            "www.bankofamerica.com",
        ] {
            assert!(e.host(h), "{h}");
        }
        for h in [
            "mail.google.com",
            "app.slack.com",
            "notion.so",
            "example.com",
            "github.com",
        ] {
            assert!(!e.host(h), "{h}");
        }
        assert_eq!(
            url_host("https://user@Example.com:443/u/0?x#y").as_deref(),
            Some("example.com")
        );
        assert_eq!(url_host("file:///tmp/x.html"), None);
        assert_eq!(url_host("not a url"), None);
    }

    #[test]
    fn sensitive_and_numeric_fields() {
        let e = exclusions();
        for l in [
            "Password",
            "Enter your PIN",
            "Card number",
            "one-time code",
            "IBAN",
            "Wachtwoord",
            "otp-input",
        ] {
            assert!(e.sensitive_label(l), "{l}");
        }
        for l in [
            "Message #general",
            "Search",
            "Subject",
            "Spinner",
            "pinned messages",
            "",
        ] {
            assert!(!e.sensitive_label(l), "{l}");
        }
        assert!(is_numeric_role("AXIncrementor", ""));
        assert!(!is_numeric_role("AXTextField", ""));
        assert!(is_secure_role("AXTextField", "AXSecureTextField"));
        assert!(is_secure_role("AXSecureTextField", ""));
        assert!(!is_secure_role("AXTextArea", ""));
    }

    fn no_focus_read(elements: &[(&str, &str, bool, &str, &str)]) -> Read {
        let mut lines = LineCollector::new(MAX_CHARS);
        let walk = (|| {
            for &(role, subrole, protected, labels, text) in elements {
                if let Some(skip) = field_exclusion(role, subrole, protected, labels) {
                    return Err(skip);
                }
                lines.add(text);
            }
            Ok(lines)
        })();
        read_from_walk(CaretWindow::default(), walk, &[])
    }

    #[derive(Clone, PartialEq)]
    struct TestElement {
        role: &'static str,
        labels: &'static str,
        value: Option<&'static str>,
        visible: bool,
        children: Vec<TestElement>,
    }

    impl Default for TestElement {
        fn default() -> Self {
            TestElement {
                role: "AXGroup",
                labels: "",
                value: None,
                visible: true,
                children: Vec::new(),
            }
        }
    }

    fn tree_read(root: TestElement) -> Read {
        let mut lines = LineCollector::new(MAX_CHARS);
        let walk = walk_roots(
            vec![(root, None)],
            true,
            &mut lines,
            Instant::now() + WALK_BUDGET,
            |e| {
                let texts = e
                    .value
                    .filter(|_| accepts_walk_value(e.role))
                    .map(str::to_owned)
                    .into_iter()
                    .collect();
                WalkItem {
                    role: e.role.to_owned(),
                    exclusion: field_exclusion(e.role, "", false, e.labels),
                    texts,
                    visible: e.visible,
                    file_row: is_file_row(e.role, ""),
                    children: e.children.clone(),
                }
            },
        );
        read_from_walk(CaretWindow::default(), walk.map(|()| lines), &[])
    }

    #[test]
    fn web_area_text_cannot_bypass_sensitive_children_in_unreadable_containers() {
        for (role, visible) in [("AXGroup", false), ("AXScrollBar", true)] {
            let read = tree_read(TestElement {
                role: "AXWebArea",
                value: Some("Zebulonix Quarterbridge"),
                children: vec![TestElement {
                    role,
                    visible,
                    children: vec![TestElement {
                        role: "AXTextField",
                        labels: "Card number Card number",
                        value: Some("4111 1111 1111 1111"),
                        ..TestElement::default()
                    }],
                    ..TestElement::default()
                }],
                ..TestElement::default()
            });
            assert_eq!(read.skip, Some(Skip::Sensitive), "{role}");
            assert_eq!(read.chars, 0, "{role}");
            assert!(read.hints.is_empty(), "{role}");
        }
    }

    #[test]
    fn web_area_walk_reads_leaf_text_instead_of_its_aggregate_value() {
        let read = tree_read(TestElement {
            role: "AXWebArea",
            value: Some("Aggregate page text"),
            children: vec![TestElement {
                role: "AXStaticText",
                value: Some("Ines Nowak"),
                ..TestElement::default()
            }],
            ..TestElement::default()
        });
        assert_eq!(read.skip, None);
        assert_eq!(read.chars, "Ines Nowak".chars().count());
        assert_eq!(read.hints, ["Ines", "Nowak"]);
    }

    #[test]
    fn browser_window_text_requires_an_inspectable_page() {
        assert_eq!(browser_context_skip(true, false), Some(Skip::NoAccess));
        assert_eq!(browser_context_skip(true, true), None);
        assert_eq!(browser_context_skip(false, false), None);
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn generic_focus_uses_the_whole_window_walk() {
        for role in ["AXWebArea", "AXWindow", "AXGroup"] {
            assert!(!mac::narrows_walk_to_focused_field(role), "{role}");
        }
        for role in ["AXTextField", "AXTextArea", "AXComboBox", "AXSearchField"] {
            assert!(mac::narrows_walk_to_focused_field(role), "{role}");
        }
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn generic_focus_does_not_preread_caret_before_the_privacy_walk() {
        for role in ["AXWebArea", "AXWindow", "AXGroup"] {
            let called = std::cell::Cell::new(false);
            let caret = mac::focused_caret(role, || {
                called.set(true);
                CaretWindow::new("Zebulonix Quarterbridge", "", "")
            });
            assert!(!called.get(), "{role}");
            let read = read_from_walk(caret, Ok(LineCollector::new(MAX_CHARS)), &[]);
            assert_eq!(read.chars, 0, "{role}");
            assert!(read.hints.is_empty(), "{role}");
        }

        let caret = mac::focused_caret("AXTextArea", || CaretWindow::new("Ines Nowak", "", ""));
        assert_eq!(caret.before, "Ines Nowak");
    }

    #[test]
    fn no_focus_card_number_markup_discards_the_whole_walk() {
        let read = no_focus_read(&[
            ("AXStaticText", "", false, "", "Zebulonix Quarterbridge"),
            (
                "AXTextField",
                "",
                false,
                "Card number Card number",
                "4111 1111 1111 1111",
            ),
        ]);
        assert_eq!(read.skip, Some(Skip::Sensitive));
        assert_eq!(read.chars, 0);
        assert!(read.hints.is_empty());
    }

    #[test]
    fn no_focus_numeric_and_secure_markup_discards_the_whole_walk() {
        for (role, subrole, protected, expected) in [
            ("AXIncrementor", "", false, Skip::Numeric),
            ("AXTextField", "AXSecureTextField", false, Skip::SecureField),
            ("AXTextField", "", true, Skip::SecureField),
        ] {
            let read = no_focus_read(&[(role, subrole, protected, "", "Private nearby text")]);
            assert_eq!(read.skip, Some(expected));
            assert_eq!(read.chars, 0);
            assert!(read.hints.is_empty());
        }
    }

    #[test]
    fn no_focus_non_sensitive_walk_still_reads_context() {
        let text = "Ines Nowak";
        let read = no_focus_read(&[("AXStaticText", "", false, "Message", text)]);
        assert_eq!(read.skip, None);
        assert_eq!(read.chars, text.chars().count());
        assert_eq!(read.hints, ["Ines", "Nowak"]);
    }

    #[test]
    fn text_fields_contribute_only_their_visible_range() {
        // FluidVoice testShortScrolledFieldIncludesOnlyVisibleWords /
        // testLongScrolledFieldIncludesOnlyVisibleWords.
        let value = "Hidden start. Visible name. Hidden end.";
        let at = value.find("Visible name").unwrap() as isize;
        assert_eq!(text_entry_value(value, Some((at, 12))), "Visible name");
        let long = format!("{}Visible name{}", "x".repeat(5000), "y".repeat(5000));
        assert_eq!(text_entry_value(&long, Some((5000, 12))), "Visible name");
        // testVisibleRangeUsesUTF16Offsets.
        let v = "Zoë 🐈 rest";
        let units: Vec<u16> = "Zoë 🐈".encode_utf16().collect();
        assert_eq!(
            text_entry_value(v, Some((0, units.len() as isize))),
            "Zoë 🐈"
        );
        // testEmptyOrInvalidVisibleRangeNeverExposesTheFullValue.
        for r in [(0, 0), (-1, 3), (20, 1), (2, 100)] {
            assert_eq!(text_entry_value("hidden text", Some(r)), "", "{r:?}");
        }
        // testMissingVisibleRangeKeepsOnlyTheBoundedFallback.
        assert_eq!(text_entry_value("Short field", None), "Short field");
        assert_eq!(text_entry_value(&"x".repeat(4001), None), "");
    }

    #[test]
    fn the_caret_window_is_bounded() {
        // FluidVoice testCursorContextIsBounded, with R5's three parts.
        let before = format!("{}end", "a".repeat(5000));
        let w = CaretWindow::new(&before, &"s".repeat(1000), &"b".repeat(1000));
        assert_eq!(w.before.chars().count(), BEFORE_CHARS);
        assert!(w.before.ends_with("end"));
        assert_eq!(w.selected.chars().count(), SELECTED_CHARS);
        assert_eq!(w.after.chars().count(), AFTER_CHARS);
        assert!(w.chars() <= MAX_CHARS);
        assert_eq!(last_chars("héllo", 3), "llo");
        assert_eq!(last_chars("hi", 5), "hi");
        assert_eq!(last_chars("hi", 0), "");
        assert_eq!(first_chars("héllo", 2), "hé");
        // Ranges in UTF-16 units: twice the budget, clipped to the field.
        let [b, s, a] = caret_ranges(3000, 10, Some(3100));
        assert_eq!(b, (3000 - 1600, 1600));
        assert_eq!(s, (3000, 10));
        assert_eq!(a, (3010, 90));
        let [b, s, a] = caret_ranges(5, 5000, None);
        assert_eq!(b, (0, 5));
        assert_eq!(s, (5, 600));
        assert_eq!(a, (5005, 800));
    }

    #[test]
    fn lines_are_unique_trimmed_and_within_the_budget() {
        let mut c = LineCollector::new(30);
        c.add(
            "  Siobhán Kowalczyk  \n\n12:30\nSiobhán Kowalczyk\nA reply that is long enough to cut",
        );
        assert!(c.is_full());
        assert_eq!(c.chars(), 30);
        let lines = c.into_lines();
        assert_eq!(lines[0], "Siobhán Kowalczyk");
        assert_eq!(lines.len(), 2, "{lines:?}");
        let mut c = LineCollector::new(MAX_CHARS);
        c.add(&"w".repeat(1000));
        assert_eq!(c.chars(), MAX_LINE_CHARS);
    }

    #[test]
    fn hints_come_nearest_the_caret_first() {
        let w = CaretWindow::new("Notes from Priya\nThanks Bjørn, see you", "", " and Aoife.");
        let hints = hints_of(
            &w,
            &["Kowalczyk joined the thread".into(), "Ines Nowak".into()],
            &[],
        );
        // A line's first word alone is not a name; an author line is.
        assert_eq!(hints, ["Bjørn", "Priya", "Aoife", "Ines", "Nowak"]);
    }
}

/// S-36 live check of the editor read against a running throwaway editor in
/// front (never in CI): a cold read is `warming`, the AX thread's
/// keep-warm builds the tree, and a later read finds the chat input, code
/// names and visible file names. Prints counts and names (test data only).
/// `WB_S36_LIVE_PID=<pid> cargo test --lib live_editor_read -- --ignored --nocapture`
#[cfg(all(test, target_os = "macos"))]
mod live_tests {
    use std::time::Duration;

    use crate::fork::ax::thread;
    use crate::fork::settings::VibeCodingSettings;

    #[test]
    #[ignore = "needs a throwaway editor in front (WB_S36_LIVE_PID)"]
    fn live_editor_read() {
        if std::env::var("WB_S36_LIVE_PID").is_err() {
            eprintln!("WB_S36_LIVE_PID not set: skipped");
            return;
        }
        let vibe = VibeCodingSettings {
            enabled: true,
            setup_completed: true,
            editors: vec!["cursor".into(), "vscode".into(), "windsurf".into()],
            file_tags_enabled: true,
        };
        let read = |vibe: VibeCodingSettings| {
            thread::run(Duration::from_secs(2), move |ctx| {
                super::mac::read(ctx, &[], &vibe)
            })
            .expect("the AX thread answered")
        };
        let first = read(vibe.clone());
        eprintln!(
            "first read: warming={} skip={:?} ide={:?} ids={:?} files={:?} chars={} {:.1} ms",
            first.warming,
            first.skip,
            first.ide,
            first.identifiers,
            first.file_names,
            first.chars,
            first.ms
        );
        thread::keep_warm(super::super::ide::warm_bundles(&vibe, true));
        std::thread::sleep(Duration::from_millis(3_000));
        let later = read(vibe.clone());
        eprintln!(
            "after keep-warm: warming={} skip={:?} ide={:?} ids={:?} files={:?} chars={} {:.1} ms",
            later.warming,
            later.skip,
            later.ide,
            later.identifiers,
            later.file_names,
            later.chars,
            later.ms
        );
        thread::keep_warm(Vec::new());
        assert!(!later.warming);
        assert!(later.ide.is_some());
    }
}
