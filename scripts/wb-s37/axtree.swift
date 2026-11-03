// SPDX-License-Identifier: GPL-3.0-or-later
//
// S-37 AX-tree check (docs/fork/MAC-TEST-SCRIPT.md, S-37.3): reads every
// control in the running app's windows and panels (the Hub, Settings,
// onboarding, the Scratchpad, lessons and the Flow Bar) through the
// Accessibility API and fails on a control VoiceOver could not name. Read
// only: it presses nothing and posts no keys. The terminal needs
// Accessibility.
//
//   swiftc -O scripts/wb-s37/axtree.swift -o <scratch>/wbaxtree
//   wbaxtree <pid> <label> [out.tsv]
//
// For each element whose role is a control (button, check box, radio
// button, pop-up, menu button, combo box, text field/area, slider, link,
// disclosure triangle, tab, switch) it prints a TSV row: label, role,
// subrole, AXTitle, AXDescription, AXHelp, AXValue (cut), AXTitleUIElement's
// text, enabled, and the window title. A control is **named** when any of
// AXTitle, AXDescription or its AXTitleUIElement text is non-empty (a text
// field may also be named by AXPlaceholderValue, which VoiceOver reads;
// recorded as `placeholder-only`). Hidden and offscreen-only web nodes are
// skipped (AXHidden, zero size).
//
// Exit 0: every control named. Exit 1: at least one `UNNAMED` row (they are
// printed to stderr too). Exit 2: usage / no windows.
import AppKit
import ApplicationServices
import Foundation

let args = CommandLine.arguments
guard args.count >= 3, let pid = Int32(args[1]) else {
    FileHandle.standardError.write("usage: axtree.swift <pid> <label> [out.tsv]\n".data(using: .utf8)!)
    exit(2)
}
let label = args[2]
let outPath = args.count >= 4 ? args[3] : nil
let app = AXUIElementCreateApplication(pid)
AXUIElementSetMessagingTimeout(app, 2.0)

func attr(_ e: AXUIElement, _ name: String) -> AnyObject? {
    var value: AnyObject?
    return AXUIElementCopyAttributeValue(e, name as CFString, &value) == .success ? value : nil
}
func str(_ e: AXUIElement, _ name: String) -> String {
    if let s = attr(e, name) as? String { return s }
    if let n = attr(e, name) as? NSNumber { return n.stringValue }
    return ""
}
func kids(_ e: AXUIElement) -> [AXUIElement] { attr(e, kAXChildrenAttribute) as? [AXUIElement] ?? [] }
func clean(_ s: String, _ max: Int = 80) -> String {
    let one = s.replacingOccurrences(of: "\t", with: " ").replacingOccurrences(of: "\n", with: " ")
    return one.count > max ? String(one.prefix(max)) + "…" : one
}

let controlRoles: Set<String> = [
    kAXButtonRole as String, kAXCheckBoxRole as String, kAXRadioButtonRole as String,
    kAXPopUpButtonRole as String, kAXMenuButtonRole as String, kAXComboBoxRole as String,
    kAXTextFieldRole as String, kAXTextAreaRole as String, kAXSliderRole as String,
    "AXLink", kAXDisclosureTriangleRole as String, "AXTab", "AXSwitch", kAXIncrementorRole as String,
]
let textRoles: Set<String> = [kAXTextFieldRole as String, kAXTextAreaRole as String, kAXComboBoxRole as String]

func size(_ e: AXUIElement) -> CGSize {
    guard let v = attr(e, kAXSizeAttribute) else { return .zero }
    var s = CGSize.zero
    // swiftlint:disable:next force_cast
    AXValueGetValue(v as! AXValue, .cgSize, &s)
    return s
}

var windows = (attr(app, kAXWindowsAttribute) as? [AXUIElement]) ?? []
// Panels without a title bar (the Flow Bar) can be missing from AXWindows
// but are children of the application element.
for k in kids(app) where str(k, kAXRoleAttribute) == kAXWindowRole as String {
    if !windows.contains(where: { CFEqual($0, k) }) { windows.append(k) }
}
guard !windows.isEmpty else {
    FileHandle.standardError.write("no windows for pid \(pid)\n".data(using: .utf8)!)
    exit(2)
}

var rows: [String] = []
var unnamed = 0
var controls = 0
for window in windows {
    let windowTitle = clean(str(window, kAXTitleAttribute), 40)
    var stack: [AXUIElement] = [window]
    var seen = 0
    while let e = stack.popLast(), seen < 40_000 {
        seen += 1
        if (attr(e, "AXHidden") as? Bool) == true { continue }
        stack.append(contentsOf: kids(e).reversed())
        let role = str(e, kAXRoleAttribute)
        guard controlRoles.contains(role) else { continue }
        let sz = size(e)
        if sz.width < 1 || sz.height < 1 { continue }
        controls += 1
        let title = str(e, kAXTitleAttribute)
        let desc = str(e, kAXDescriptionAttribute)
        let help = str(e, kAXHelpAttribute)
        let value = str(e, kAXValueAttribute)
        var titleUI = ""
        if let t = attr(e, kAXTitleUIElementAttribute) {
            // swiftlint:disable:next force_cast
            let te = t as! AXUIElement
            titleUI = str(te, kAXValueAttribute).isEmpty ? str(te, kAXTitleAttribute) : str(te, kAXValueAttribute)
        }
        let placeholder = str(e, kAXPlaceholderValueAttribute)
        let enabled = (attr(e, kAXEnabledAttribute) as? Bool) ?? true
        var verdict = "ok"
        if title.isEmpty && desc.isEmpty && titleUI.isEmpty {
            if textRoles.contains(role) && !placeholder.isEmpty {
                verdict = "placeholder-only"
            } else {
                verdict = "UNNAMED"
                unnamed += 1
            }
        }
        let row = [
            label, verdict, role, str(e, kAXSubroleAttribute), clean(title), clean(desc), clean(help),
            clean(value, 40), clean(titleUI), enabled ? "enabled" : "disabled", windowTitle,
        ].joined(separator: "\t")
        rows.append(row)
        if verdict == "UNNAMED" { FileHandle.standardError.write("UNNAMED \(row)\n".data(using: .utf8)!) }
    }
}

let header = "label\tverdict\trole\tsubrole\ttitle\tdescription\thelp\tvalue\ttitleUI\tenabled\twindow"
let text = ([header] + rows).joined(separator: "\n") + "\n"
if let outPath {
    if let h = FileHandle(forWritingAtPath: outPath) {
        h.seekToEndOfFile()
        h.write(rows.map { $0 + "\n" }.joined().data(using: .utf8)!)
        h.closeFile()
    } else {
        try? text.write(toFile: outPath, atomically: true, encoding: .utf8)
    }
} else {
    print(text, terminator: "")
}
print("\(label): \(controls) controls in \(windows.count) windows, \(unnamed) unnamed")
exit(unnamed == 0 ? 0 : 1)
