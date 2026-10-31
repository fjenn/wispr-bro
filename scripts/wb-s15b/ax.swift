// SPDX-License-Identifier: GPL-3.0-or-later
//
// S-15b real-app driver (docs/fork/MAC-TEST-SCRIPT.md, S-15b): finds
// controls in the test app's own window through the Accessibility API (the
// web view exposes its buttons, menu items and fields with their labels)
// and presses them, so History's row actions can be used without a person
// and without guessing screen coordinates. It acts only on the given PID's
// elements, and posts key events only while that PID is frontmost.
//
//   swift scripts/wb-s15b/ax.swift <pid> 'press Play recording#2; wait 500; dump'
//
// Commands (separated by `;`):
//   front                 bring the app to the front
//   move <x> <y>          move the app's window (screen points)
//   raise                 raise the app's windows (AXRaise) and make it frontmost
//   press <label>[#n]     AXPress the n-th (1-based, default 1) element whose
//                         label (description, title or value) contains <label>
//                         (`=<label>`: the whole label equals it)
//   focus <label>[#n]     focus that element
//   type <text>           Unicode key events into the focused element
//   key <name>            return | escape | tab | down | up
//   dump                  print every labelled element (role, label, enabled)
//   find <label>          print the matches for <label> (role, label, enabled)
//   wait <ms>             sleep
// Prints one line per command; exits 3 when an element is missing or the
// app is not frontmost before a key event.
import AppKit
import ApplicationServices
import Foundation

let args = CommandLine.arguments
guard args.count >= 3, let pid = Int32(args[1]) else {
    FileHandle.standardError.write("usage: ax.swift <pid> '<commands>'\n".data(using: .utf8)!)
    exit(2)
}
let app = AXUIElementCreateApplication(pid)

func attr(_ e: AXUIElement, _ name: String) -> AnyObject? {
    var v: AnyObject?
    return AXUIElementCopyAttributeValue(e, name as CFString, &v) == .success ? v : nil
}

func label(_ e: AXUIElement) -> String {
    for name in [kAXDescriptionAttribute, kAXTitleAttribute, kAXValueAttribute] {
        if let s = attr(e, name as String) as? String, !s.isEmpty { return s }
    }
    return ""
}

func role(_ e: AXUIElement) -> String { attr(e, kAXRoleAttribute as String) as? String ?? "?" }
func enabled(_ e: AXUIElement) -> Bool { attr(e, kAXEnabledAttribute as String) as? Bool ?? true }

/// Every element under the app, depth first (windows, then the web view).
func walk(_ e: AXUIElement, depth: Int = 0, into out: inout [AXUIElement]) {
    if depth > 60 { return }
    out.append(e)
    guard let children = attr(e, kAXChildrenAttribute as String) as? [AXUIElement] else { return }
    for c in children { walk(c, depth: depth + 1, into: &out) }
}

func all() -> [AXUIElement] {
    var out: [AXUIElement] = []
    walk(app, into: &out)
    return out
}

func matches(_ spec: String) -> [AXUIElement] {
    var text = spec
    var n = 1
    if let hash = spec.lastIndex(of: "#"), let k = Int(spec[spec.index(after: hash)...]) {
        text = String(spec[..<hash])
        n = k
    }
    // "=Label" matches the whole label only ("=Dictation" is the sidebar
    // item, not the "Dictation bar" pill, which would start a take).
    let exact = text.hasPrefix("=")
    if exact { text.removeFirst() }
    let found = all().filter {
        (exact ? label($0) == text : label($0).contains(text)) && role($0) != "AXStaticText"
    }
    return n <= found.count ? [found[n - 1]] : []
}

func frontPid() -> Int32? {
    RunLoop.current.run(until: Date(timeIntervalSinceNow: 0.05))
    return NSWorkspace.shared.frontmostApplication?.processIdentifier
}

let source = CGEventSource(stateID: .hidSystemState)
func post(_ e: CGEvent?) {
    e?.post(tap: .cghidEventTap)
    usleep(12_000)
}
func requireFront() {
    guard frontPid() == pid else {
        print("ABORT: pid \(pid) is not frontmost")
        exit(3)
    }
}

let keys: [String: CGKeyCode] = ["return": 36, "escape": 53, "tab": 48, "down": 125, "up": 126]

for raw in args[2].split(separator: ";") {
    let cmd = raw.trimmingCharacters(in: .whitespaces)
    if cmd.isEmpty { continue }
    let parts = cmd.split(separator: " ", maxSplits: 1).map(String.init)
    let rest = parts.count > 1 ? parts[1] : ""
    switch parts[0] {
    case "front":
        NSRunningApplication(processIdentifier: pid)?.activate(options: [])
        usleep(700_000)
        requireFront()
    case "raise":
        // Brings the app's own windows above others (another test app may
        // sit at the same default position), without touching other apps.
        for w in (attr(app, kAXWindowsAttribute as String) as? [AXUIElement]) ?? [] {
            AXUIElementPerformAction(w, kAXRaiseAction as CFString)
        }
        AXUIElementSetAttributeValue(app, kAXFrontmostAttribute as CFString, kCFBooleanTrue)
        usleep(700_000)
        requireFront()
    case "move":
        // Moves the app's own main window (out from under another window).
        let xy = rest.split(separator: " ").compactMap { Double($0) }
        var p = CGPoint(x: xy[0], y: xy[1])
        func area(_ w: AXUIElement) -> Double {
            var s = CGSize.zero
            if let v = attr(w, kAXSizeAttribute as String) { AXValueGetValue(v as! AXValue, .cgSize, &s) }
            return s.width * s.height
        }
        let windows = (attr(app, kAXWindowsAttribute as String) as? [AXUIElement]) ?? []
        if let w = windows.max(by: { area($0) < area($1) }), let v = AXValueCreate(.cgPoint, &p) {
            AXUIElementSetAttributeValue(w, kAXPositionAttribute as CFString, v)
        }
    case "press", "focus":
        guard let e = matches(rest).first else {
            print("ABORT: no element \(rest)")
            exit(3)
        }
        if parts[0] == "press" {
            let r = AXUIElementPerformAction(e, kAXPressAction as CFString)
            print("pressed \(role(e)) \(label(e)) -> \(r.rawValue)")
        } else {
            AXUIElementSetAttributeValue(e, kAXFocusedAttribute as CFString, kCFBooleanTrue)
            print("focused \(role(e)) \(label(e))")
        }
    case "type":
        for ch in rest {
            requireFront()
            let units = Array(String(ch).utf16)
            for down in [true, false] {
                let e = CGEvent(keyboardEventSource: source, virtualKey: 0, keyDown: down)
                e?.keyboardSetUnicodeString(stringLength: units.count, unicodeString: units)
                post(e)
            }
        }
    case "key":
        guard let code = keys[rest] else { print("unknown key \(rest)"); exit(2) }
        requireFront()
        for down in [true, false] { post(CGEvent(keyboardEventSource: source, virtualKey: code, keyDown: down)) }
    case "dump", "find":
        for e in all() {
            let l = label(e)
            if l.isEmpty || (parts[0] == "find" && !l.contains(rest)) { continue }
            print("\(role(e))\t\(enabled(e) ? "on" : "off")\t\(l.prefix(120))")
        }
    case "wait":
        usleep(UInt32(Int(rest)! * 1000))
    default:
        print("unknown command \(cmd)")
        exit(2)
    }
    print("ok \(cmd)")
}
