// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (c) 2024 OpenWhispr Team (OpenWhispr; MIT, notice below)
// Modified by Wispr Bro contributors, 2026-10-30 and 2026-10-31.
//
// The text-role rule is ported from OpenWhispr
// `resources/macos-text-monitor.swift` `isEditableTextElement`
// (https://github.com/OpenWhispr/openwhispr, commit
// b9e9ebefac4b961456c269164a93ddfdd53438b9), MIT: disabled → no; secure
// subrole → no; role in TextField/TextArea/ComboBox and `AXValue` or
// `AXSelectedText` settable → yes. Changed: the facts are read elsewhere
// (probe.rs) so the rule is pure and unit-tested; OpenWhispr's "a live
// selection means not editable" check is dropped (it protects a different
// feature; for dictation a selection is the normal replace case); a text role
// that is not settable is "unknown" (paste anyway), not "no". Added: VoiceInk's
// `AXEditable` / `AXProtectedContent` checks (`AccessibilityFocusReader`
// `editableState`, GPL-3.0, re-expressed), the search-field and web roles, the
// terminal and Secure Input cases, the Chromium/Electron "tree not built"
// case, and R5 §4.6's fail-open rule (an empty AX answer with a focused
// window means "paste anyway"; the rule as TypeWhisper describes it, no code
// copied). S-05 (2026-10-31): moved from the RS-04 spike into the product;
// added the Gecko family (fail open, TypeWhisper's Gecko exclusion as
// behaviour) and the Finder family (the desktop and file windows have no
// place to type).
//
// MIT License
//
// Copyright (c) 2024 OpenWhispr Team
//
// Permission is hereby granted, free of charge, to any person obtaining a copy
// of this software and associated documentation files (the "Software"), to deal
// in the Software without restriction, including without limitation the rights
// to use, copy, modify, merge, publish, distribute, sublicense, and/or sell
// copies of the Software, and to permit persons to whom the Software is
// furnished to do so, subject to the following conditions:
//
// The above copyright notice and this permission notice shall be included in all
// copies or substantial portions of the Software.
//
// THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
// IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
// FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE
// AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
// LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM,
// OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE
// SOFTWARE.

//! `is_editable` (R5 §4.6) as a pure decision over facts read from AX, and
//! what the insert layer does with it. Every caller tolerates `Unknown`.

use serde::Serialize;

/// The app family, which changes how much an empty AX answer means.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Family {
    Native,
    /// Safari and other WebKit hosts (always-on tree).
    Webkit,
    /// Chromium browsers: tree only after `AXManualAccessibility`.
    Chromium,
    /// Electron apps: same as Chromium.
    Electron,
    Terminal,
    /// Firefox and its forks, Thunderbird: AX answers are not trusted for
    /// a card (fail open), and no AX writes.
    Gecko,
    /// Finder (the desktop and file windows).
    Finder,
}

impl Family {
    pub fn builds_tree_on_request(self) -> bool {
        matches!(self, Family::Chromium | Family::Electron)
    }
}

/// What AX told us about the focused element (None = not answered).
#[derive(Clone, Debug, Default, PartialEq, Serialize)]
pub struct Facts {
    /// Secure Event Input is on and held by the focused app (or by a process
    /// the window server does not name): the product checks the holder, so
    /// Terminal's Secure Keyboard Entry does not refuse a paste elsewhere.
    pub secure_input: bool,
    pub has_focused_window: bool,
    pub has_focused_element: bool,
    pub role: Option<String>,
    pub subrole: Option<String>,
    pub enabled: Option<bool>,
    /// `AXEditable` (WebKit/Chromium expose it on editable web content).
    pub editable_attr: Option<bool>,
    pub protected_content: Option<bool>,
    pub value_settable: Option<bool>,
    pub selected_text_settable: Option<bool>,
    pub has_selected_range: bool,
    /// Chromium/Electron: our lease (or someone's) has the tree switched on.
    pub tree_on: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Reason {
    TextRole,
    EditableAttr,
    SettableSelection,
    WebAreaSettable,
    SecureSubrole,
    ProtectedContent,
    SecureInput,
    NoElementWithWindow,
    NoWindow,
    Disabled,
    TextRoleNotSettable,
    TreeNotBuilt,
    ControlRole,
    WebAreaNotEditable,
    ContainerRole,
    UnrecognizedRole,
    /// Gecko said "not a text field"; not trusted, so paste.
    GeckoNotTrusted,
    /// Finder with no text field focused (desktop, file list, sidebar).
    FinderNoTextField,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(tag = "verdict", content = "reason", rename_all = "snake_case")]
pub enum Verdict {
    Editable(Reason),
    NotEditable(Reason),
    Secure(Reason),
    /// Terminals: paste, never read (scrollback is the "text").
    Terminal,
    /// AX could not say; the fail-open rule pastes anyway.
    Unknown(Reason),
}

/// What the insert layer does (S-05).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Delivery {
    Paste,
    PasteFailOpen,
    /// "Click into a text field first" card with the transcript.
    Card,
    /// Password field: refused with a card.
    Refuse,
}

const TEXT_ROLES: &[&str] = &["AXTextField", "AXTextArea", "AXComboBox", "AXSearchField"];

/// Roles that are never a place to type.
const CONTROL_ROLES: &[&str] = &[
    "AXButton",
    "AXCheckBox",
    "AXRadioButton",
    "AXPopUpButton",
    "AXMenuButton",
    "AXMenuItem",
    "AXMenuBarItem",
    "AXLink",
    "AXList",
    "AXOutline",
    "AXTable",
    "AXRow",
    "AXCell",
    "AXColumn",
    "AXStaticText",
    "AXImage",
    "AXSlider",
    "AXTabGroup",
    "AXToolbar",
    "AXDisclosureTriangle",
    "AXIncrementor",
    "AXBrowser",
    "AXCollection",
    "AXSegmentedControl",
    "AXHeading",
];

/// Containers: on a native app they are not a text field; on a web app
/// without its tree they are what an unbuilt tree looks like.
const CONTAINER_ROLES: &[&str] = &[
    "AXGroup",
    "AXScrollArea",
    "AXSplitGroup",
    "AXWindow",
    "AXApplication",
    "AXUnknown",
    "AXLayoutArea",
];

pub fn verdict(f: &Facts, family: Family) -> Verdict {
    use Reason::*;
    if f.subrole.as_deref() == Some("AXSecureTextField") {
        return Verdict::Secure(SecureSubrole);
    }
    if f.protected_content == Some(true) {
        return Verdict::Secure(ProtectedContent);
    }
    if family == Family::Terminal {
        // Secure Keyboard Entry in a terminal is not a password field.
        return Verdict::Terminal;
    }
    // Secure Input is global and lingers: Safari keeps it on after focus
    // leaves a password field for a button, a link or the page (RS-04
    // auto run, 2026-10-30). So it only decides when the element itself
    // cannot: an unknown answer under Secure Input is refused, not pasted.
    let v = element_verdict(f, family);
    if f.secure_input && matches!(v, Verdict::Unknown(_)) {
        return Verdict::Secure(SecureInput);
    }
    match (family, v) {
        // Gecko: a "not a text field" answer is not trusted (fail open).
        (Family::Gecko, Verdict::NotEditable(NoWindow)) => v,
        (Family::Gecko, Verdict::NotEditable(_)) => Verdict::Unknown(GeckoNotTrusted),
        // Finder hosts no web content or custom editors: anything that is
        // not a text field (rename, search, Go to Folder) is the desktop or
        // a file view.
        (
            Family::Finder,
            Verdict::Unknown(ContainerRole | UnrecognizedRole | NoElementWithWindow),
        ) => Verdict::NotEditable(FinderNoTextField),
        _ => v,
    }
}

fn element_verdict(f: &Facts, family: Family) -> Verdict {
    use Reason::*;
    if !f.has_focused_element {
        return if f.has_focused_window {
            Verdict::Unknown(NoElementWithWindow)
        } else {
            Verdict::NotEditable(NoWindow)
        };
    }
    if f.enabled == Some(false) {
        return Verdict::NotEditable(Disabled);
    }
    let role = f.role.as_deref().unwrap_or("");
    let settable = f.value_settable == Some(true) || f.selected_text_settable == Some(true);
    if TEXT_ROLES.contains(&role) {
        if settable || f.editable_attr == Some(true) {
            return Verdict::Editable(TextRole);
        }
        return Verdict::Unknown(TextRoleNotSettable);
    }
    if f.editable_attr == Some(true) {
        return Verdict::Editable(EditableAttr);
    }
    if role == "AXWebArea" {
        if settable {
            return Verdict::Editable(WebAreaSettable);
        }
        if family.builds_tree_on_request() && !f.tree_on {
            return Verdict::Unknown(TreeNotBuilt);
        }
        // RS-04 owner run: a focused web area or container is often the
        // host of a field AX does not report (Safari, Obsidian, Cursor,
        // ChatGPT), so it fails open rather than showing the card.
        return Verdict::Unknown(WebAreaNotEditable);
    }
    if CONTROL_ROLES.contains(&role) {
        return Verdict::NotEditable(ControlRole);
    }
    if f.has_selected_range && f.selected_text_settable == Some(true) {
        return Verdict::Editable(SettableSelection);
    }
    if family.builds_tree_on_request() && (!f.tree_on || role.is_empty()) {
        return Verdict::Unknown(TreeNotBuilt);
    }
    if CONTAINER_ROLES.contains(&role) {
        return Verdict::Unknown(ContainerRole);
    }
    Verdict::Unknown(UnrecognizedRole)
}

pub fn delivery(v: Verdict) -> Delivery {
    match v {
        Verdict::Editable(_) | Verdict::Terminal => Delivery::Paste,
        Verdict::Unknown(_) => Delivery::PasteFailOpen,
        Verdict::NotEditable(_) => Delivery::Card,
        Verdict::Secure(_) => Delivery::Refuse,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use Reason::*;

    fn el(role: &str) -> Facts {
        Facts {
            has_focused_window: true,
            has_focused_element: true,
            role: Some(role.into()),
            enabled: Some(true),
            ..Facts::default()
        }
    }

    #[test]
    fn native_text_field_settable() {
        let mut f = el("AXTextField");
        f.value_settable = Some(true);
        assert_eq!(verdict(&f, Family::Native), Verdict::Editable(TextRole));
        let mut f = el("AXTextArea");
        f.selected_text_settable = Some(true);
        assert_eq!(verdict(&f, Family::Native), Verdict::Editable(TextRole));
        let mut f = el("AXSearchField");
        f.value_settable = Some(true);
        assert_eq!(delivery(verdict(&f, Family::Native)), Delivery::Paste);
    }

    #[test]
    fn text_role_not_settable_fails_open() {
        let f = el("AXTextArea");
        assert_eq!(
            verdict(&f, Family::Native),
            Verdict::Unknown(TextRoleNotSettable)
        );
        assert_eq!(
            delivery(verdict(&f, Family::Native)),
            Delivery::PasteFailOpen
        );
    }

    #[test]
    fn a_selection_does_not_make_a_field_read_only() {
        // OpenWhispr returns false here; dictation replaces the selection.
        let mut f = el("AXTextArea");
        f.value_settable = Some(true);
        f.has_selected_range = true;
        assert_eq!(verdict(&f, Family::Native), Verdict::Editable(TextRole));
    }

    #[test]
    fn secure_cases() {
        let mut f = el("AXTextField");
        f.subrole = Some("AXSecureTextField".into());
        f.value_settable = Some(true);
        assert_eq!(verdict(&f, Family::Webkit), Verdict::Secure(SecureSubrole));
        assert_eq!(delivery(verdict(&f, Family::Webkit)), Delivery::Refuse);
        let mut f = el("AXTextField");
        f.protected_content = Some(true);
        assert_eq!(
            verdict(&f, Family::Native),
            Verdict::Secure(ProtectedContent)
        );
        // Secure Input alone does not override what the element says...
        let mut f = el("AXTextArea");
        f.secure_input = true;
        f.value_settable = Some(true);
        assert_eq!(verdict(&f, Family::Native), Verdict::Editable(TextRole));
        let mut b = el("AXButton");
        b.secure_input = true;
        assert_eq!(
            verdict(&b, Family::Webkit),
            Verdict::NotEditable(ControlRole)
        );
        // ...but refuses when AX cannot tell.
        let g = Facts {
            secure_input: true,
            has_focused_window: true,
            ..Facts::default()
        };
        assert_eq!(verdict(&g, Family::Electron), Verdict::Secure(SecureInput));
        // Secure Keyboard Entry in a terminal still pastes.
        assert_eq!(verdict(&f, Family::Terminal), Verdict::Terminal);
        assert_eq!(delivery(Verdict::Terminal), Delivery::Paste);
    }

    #[test]
    fn disabled_is_not_editable() {
        let mut f = el("AXTextField");
        f.enabled = Some(false);
        f.value_settable = Some(true);
        assert_eq!(verdict(&f, Family::Native), Verdict::NotEditable(Disabled));
    }

    #[test]
    fn fail_open_without_element_but_card_without_window() {
        let mut f = Facts {
            has_focused_window: true,
            ..Facts::default()
        };
        assert_eq!(
            verdict(&f, Family::Electron),
            Verdict::Unknown(NoElementWithWindow)
        );
        f.has_focused_window = false;
        assert_eq!(verdict(&f, Family::Native), Verdict::NotEditable(NoWindow));
        assert_eq!(delivery(verdict(&f, Family::Native)), Delivery::Card);
    }

    #[test]
    fn web_content() {
        let mut f = el("AXGroup");
        f.editable_attr = Some(true);
        assert_eq!(verdict(&f, Family::Webkit), Verdict::Editable(EditableAttr));
        let f = el("AXWebArea");
        assert_eq!(
            verdict(&f, Family::Webkit),
            Verdict::Unknown(WebAreaNotEditable)
        );
        let mut f = el("AXWebArea");
        f.value_settable = Some(true);
        assert_eq!(
            verdict(&f, Family::Webkit),
            Verdict::Editable(WebAreaSettable)
        );
    }

    #[test]
    fn electron_without_tree_fails_open() {
        let f = el("AXWebArea");
        assert_eq!(
            verdict(&f, Family::Electron),
            Verdict::Unknown(TreeNotBuilt)
        );
        let f = el("AXGroup");
        assert_eq!(
            verdict(&f, Family::Chromium),
            Verdict::Unknown(TreeNotBuilt)
        );
        let mut f = el("AXWebArea");
        f.tree_on = true;
        assert_eq!(
            verdict(&f, Family::Chromium),
            Verdict::Unknown(WebAreaNotEditable)
        );
        let mut f = el("AXGroup");
        f.tree_on = true;
        assert_eq!(
            verdict(&f, Family::Electron),
            Verdict::Unknown(ContainerRole)
        );
        // A control is a control even before the lease.
        let f = el("AXButton");
        assert_eq!(
            verdict(&f, Family::Electron),
            Verdict::NotEditable(ControlRole)
        );
    }

    #[test]
    fn native_containers_and_controls() {
        for r in ["AXOutline", "AXTable", "AXButton", "AXLink", "AXStaticText"] {
            assert_eq!(
                verdict(&el(r), Family::Native),
                Verdict::NotEditable(ControlRole)
            );
        }
        assert_eq!(
            verdict(&el("AXScrollArea"), Family::Native),
            Verdict::Unknown(ContainerRole)
        );
        let mut f = el("AXGroup");
        f.has_selected_range = true;
        f.selected_text_settable = Some(true);
        assert_eq!(
            verdict(&f, Family::Native),
            Verdict::Editable(SettableSelection)
        );
        assert_eq!(
            verdict(&el("AXSomethingNew"), Family::Native),
            Verdict::Unknown(UnrecognizedRole)
        );
    }
}
