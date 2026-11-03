// SPDX-License-Identifier: GPL-3.0-or-later
//
// S-26b real-app probe (docs/fork/MAC-TEST-SCRIPT.md, S-26b): what is on
// screen after a Help or Share action. Reads only (Accessibility and
// NSWorkspace); key presses go through scripts/wb-s25/ui.swift.
//
//   swiftc -O scripts/wb-s26b/share.swift -o <scratch>/wbshare
//   wbshare <pid> picker <s>     waits up to <s> seconds for the share picker
//                                (a menu or popover of the app's process),
//                                prints its item titles; exit 1 if none
//   wbshare 0 frontmost <s>      waits up to <s> seconds for a browser to be
//                                frontmost; prints its bundle id and, when
//                                it exposes one, the focused window's URL
import AppKit
import ApplicationServices
import Foundation

let args = CommandLine.arguments
guard args.count >= 4, let pid = Int32(args[1]), let seconds = Double(args[3]) else {
    FileHandle.standardError.write("usage: share.swift <pid> picker|frontmost <seconds>\n".data(using: .utf8)!)
    exit(2)
}

func attr(_ e: AXUIElement, _ name: String) -> AnyObject? {
    var value: AnyObject?
    return AXUIElementCopyAttributeValue(e, name as CFString, &value) == .success ? value : nil
}

func children(_ e: AXUIElement) -> [AXUIElement] {
    (attr(e, kAXChildrenAttribute) as? [AXUIElement]) ?? []
}

func role(_ e: AXUIElement) -> String { (attr(e, kAXRoleAttribute) as? String) ?? "" }

func label(_ e: AXUIElement) -> String {
    for name in [kAXTitleAttribute, kAXDescriptionAttribute, kAXValueAttribute] {
        if let s = attr(e, name) as? String, !s.isEmpty { return s }
    }
    return ""
}

/// Titles of menu items / buttons below `e` (the picker's services).
func titles(_ e: AXUIElement, depth: Int = 0) -> [String] {
    if depth > 6 { return [] }
    var out: [String] = []
    for child in children(e) {
        let r = role(child)
        if r == kAXMenuItemRole || r == kAXButtonRole || r == "AXCell" {
            let t = label(child)
            if !t.isEmpty { out.append(t) }
        }
        out += titles(child, depth: depth + 1)
    }
    return out
}

guard AXIsProcessTrusted() else { print("ABORT: this terminal needs Accessibility"); exit(3) }
let deadline = Date().addingTimeInterval(seconds)

switch args[2] {
case "picker":
    let app = AXUIElementCreateApplication(pid)
    while Date() < deadline {
        // NSSharingServicePicker shows a menu (or, on newer macOS, a popover
        // window) owned by the app's process.
        var candidates: [AXUIElement] = children(app).filter { role($0) == kAXMenuRole }
        for window in (attr(app, kAXWindowsAttribute) as? [AXUIElement]) ?? [] {
            let sub = (attr(window, kAXSubroleAttribute) as? String) ?? ""
            if role(window) == "AXPopover" || sub == "AXDialog" || sub == kAXUnknownSubrole {
                candidates.append(window)
            }
            candidates += children(window).filter { role($0) == "AXPopover" || role($0) == kAXMenuRole }
        }
        let found = candidates.flatMap { titles($0) }
        if !found.isEmpty {
            print("picker items: \(found.joined(separator: " | "))")
            exit(0)
        }
        usleep(250_000)
    }
    print("no share picker")
    exit(1)
case "frontmost":
    let browsers = ["com.apple.Safari", "com.google.Chrome", "org.mozilla.firefox",
                    "company.thebrowser.Browser", "com.brave.Browser", "com.microsoft.edgemac"]
    while Date() < deadline {
        if let front = NSWorkspace.shared.frontmostApplication,
           let id = front.bundleIdentifier, browsers.contains(id) {
            let ax = AXUIElementCreateApplication(front.processIdentifier)
            var url = ""
            if let window = attr(ax, kAXFocusedWindowAttribute) {
                url = (attr(window as! AXUIElement, kAXDocumentAttribute) as? String) ?? ""
                if url.isEmpty { url = (attr(window as! AXUIElement, kAXTitleAttribute) as? String) ?? "" }
            }
            print("browser \(id): \(url)")
            exit(0)
        }
        usleep(250_000)
    }
    print("no browser in front")
    exit(1)
default:
    print("unknown command \(args[2])")
    exit(2)
}
