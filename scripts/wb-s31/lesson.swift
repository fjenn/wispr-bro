// SPDX-License-Identifier: GPL-3.0-or-later
//
// S-31 real-app driver (docs/fork/MAC-TEST-SCRIPT.md, S-31): drives the
// lesson modal in the test app's Hub through the Accessibility API (buttons
// by their accessible title, the practice field's value and selection) and
// posts the few keys the lesson needs, only while that app is frontmost
// (else exit 3 before posting). The terminal needs Accessibility.
//
//   swiftc -O scripts/wb-s31/lesson.swift -o <scratch>/wblesson
//   wblesson <pid> '<cmd>; <cmd>; …'
//
// Commands:
//   front                 bring the app to the front
//   press <title>         AXPress the first button whose title or
//                         description is <title>, or whose title is <title>
//                         plus a badge (exit 4 if none in 10 s)
//   press-if <title>      the same, if it shows within 2 s (else go on): a
//                         first-run popup such as "Got it"
//   wait-text <text>      wait up to 60 s for a static text / heading
//                         containing <text> (exit 5 on timeout)
//   field                 print the practice field's value ("field: …")
//   select-all            select all of the practice field's text
//   key <name>            opt-1 | opt-o | ctrl-opt-1 | cmd-z | escape
//   wait <ms>             sleep
//   pressany <title>      AXPress the first element of any role (tab, link,
//                         row) whose title or description is <title>
//   close                 press the close button of the app's first window
//                         that has one
//   windows               print each window's title, position and size
import AppKit
import ApplicationServices
import Foundation

let args = CommandLine.arguments
guard args.count >= 3, let pid = Int32(args[1]) else {
    FileHandle.standardError.write("usage: lesson.swift <pid> '<cmds>'\n".data(using: .utf8)!)
    exit(2)
}
let app = AXUIElementCreateApplication(pid)

func attr(_ e: AXUIElement, _ name: String) -> AnyObject? {
    var value: AnyObject?
    return AXUIElementCopyAttributeValue(e, name as CFString, &value) == .success ? value : nil
}
func str(_ e: AXUIElement, _ name: String) -> String { attr(e, name) as? String ?? "" }
func kids(_ e: AXUIElement) -> [AXUIElement] { attr(e, kAXChildrenAttribute) as? [AXUIElement] ?? [] }

/// Depth-first search over the app's windows (bounded).
func find(_ match: (AXUIElement) -> Bool) -> AXUIElement? {
    var stack = (attr(app, kAXWindowsAttribute) as? [AXUIElement]) ?? []
    var seen = 0
    while let e = stack.popLast(), seen < 20_000 {
        seen += 1
        if match(e) { return e }
        stack.append(contentsOf: kids(e).reversed())
    }
    return nil
}
func poll(_ seconds: Double, _ match: @escaping (AXUIElement) -> Bool) -> AXUIElement? {
    let end = Date().addingTimeInterval(seconds)
    repeat {
        if let e = find(match) { return e }
        usleep(200_000)
    } while Date() < end
    return nil
}
func practiceField() -> AXUIElement? {
    poll(10) { str($0, kAXRoleAttribute) == kAXTextAreaRole as String }
}
func frontmost() -> Bool { NSWorkspace.shared.frontmostApplication?.processIdentifier == pid }

func post(_ code: CGKeyCode, _ flags: CGEventFlags) {
    guard frontmost() else {
        print("not frontmost: stop before posting")
        exit(3)
    }
    let src = CGEventSource(stateID: .hidSystemState)
    // Modifier presses as real flagsChanged events around the key: the fork's
    // event tap follows the modifier state from them, so a key event that
    // only carries the flags reaches the web view (in-app) but not the tap's
    // chord matcher in another app (Mac wave 2, S-31.3).
    let mods: [(CGEventFlags, CGKeyCode)] = [(.maskCommand, 55), (.maskControl, 59), (.maskAlternate, 58)]
        .filter { flags.contains($0.0) }
    var held: CGEventFlags = []
    func modifier(_ key: CGKeyCode, _ state: CGEventFlags) {
        let e = CGEvent(keyboardEventSource: src, virtualKey: key, keyDown: true)
        e?.type = .flagsChanged
        e?.flags = state
        e?.post(tap: .cghidEventTap)
        usleep(30_000)
    }
    for (flag, key) in mods { held.insert(flag); modifier(key, held) }
    for down in [true, false] {
        let e = CGEvent(keyboardEventSource: src, virtualKey: code, keyDown: down)
        e?.flags = flags
        e?.post(tap: .cghidEventTap)
        usleep(30_000)
    }
    for (flag, key) in mods.reversed() { held.remove(flag); modifier(key, held) }
}

for raw in args[2].split(separator: ";") {
    let cmd = raw.trimmingCharacters(in: .whitespaces)
    let parts = cmd.split(separator: " ", maxSplits: 1).map(String.init)
    guard let name = parts.first else { continue }
    let rest = parts.count > 1 ? parts[1] : ""
    switch name {
    case "front":
        NSRunningApplication(processIdentifier: pid)?.activate()
        usleep(500_000)
        print("front: \(frontmost())")
    case "press", "press-if":
        // A sidebar button's title carries its badge ("Transforms Beta"), so
        // a title that starts with <title> and a space also matches.
        let found = poll(name == "press" ? 10 : 2, {
            let t = str($0, kAXTitleAttribute)
            return str($0, kAXRoleAttribute) == kAXButtonRole as String
                && (t == rest || t.hasPrefix(rest + " ") || str($0, kAXDescriptionAttribute) == rest)
        })
        guard let b = found else {
            print("\(name) \(rest): not found")
            if name == "press" { exit(4) } else { continue }
        }
        let r = AXUIElementPerformAction(b, kAXPressAction as CFString)
        print("press \(rest): \(r.rawValue)")
    case "wait-text":
        let start = Date()
        guard poll(60, {
            str($0, kAXValueAttribute).contains(rest) || str($0, kAXTitleAttribute).contains(rest)
        }) != nil else { print("wait-text \(rest): timeout"); exit(5) }
        print(String(format: "wait-text %@: %.1f s", rest, Date().timeIntervalSince(start)))
    case "field":
        print("field: \(practiceField().map { str($0, kAXValueAttribute) } ?? "<none>")")
    case "select-all":
        guard let f = practiceField() else { print("select-all: no field"); exit(4) }
        AXUIElementSetAttributeValue(f, kAXFocusedAttribute as CFString, kCFBooleanTrue)
        var range = CFRange(location: 0, length: str(f, kAXValueAttribute).utf16.count)
        let value = AXValueCreate(.cfRange, &range)!
        let r = AXUIElementSetAttributeValue(f, kAXSelectedTextRangeAttribute as CFString, value)
        print("select-all: \(r.rawValue)")
    case "key":
        switch rest {
        case "opt-1": post(18, .maskAlternate)
        case "ctrl-opt-1": post(18, [.maskAlternate, .maskControl])
        case "opt-o": post(31, .maskAlternate)
        case "cmd-z": post(6, .maskCommand)
        case "escape": post(53, [])
        default: print("key \(rest): unknown"); exit(2)
        }
        print("key \(rest)")
    case "wait":
        usleep(UInt32((Int(rest) ?? 0) * 1000))
    case "pressany":
        guard let b = poll(10, {
            str($0, kAXTitleAttribute) == rest || str($0, kAXDescriptionAttribute) == rest
        }) else { print("pressany \(rest): not found"); exit(4) }
        let r = AXUIElementPerformAction(b, kAXPressAction as CFString)
        print("pressany \(rest): \(r.rawValue) (\(str(b, kAXRoleAttribute)))")
    case "close":
        // The first window that has a close button (the bar has none).
        guard let w = ((attr(app, kAXWindowsAttribute) as? [AXUIElement]) ?? [])
                .first(where: { attr($0, kAXCloseButtonAttribute) != nil }),
              let c = attr(w, kAXCloseButtonAttribute) else { print("close: no window"); exit(4) }
        let r = AXUIElementPerformAction(c as! AXUIElement, kAXPressAction as CFString)
        print("close \(str(w, kAXTitleAttribute)): \(r.rawValue)")
    case "windows":
        for w in (attr(app, kAXWindowsAttribute) as? [AXUIElement]) ?? [] {
            var p = CGPoint.zero, sz = CGSize.zero
            if let v = attr(w, kAXPositionAttribute) { AXValueGetValue(v as! AXValue, .cgPoint, &p) }
            if let v = attr(w, kAXSizeAttribute) { AXValueGetValue(v as! AXValue, .cgSize, &sz) }
            print("window '\(str(w, kAXTitleAttribute))' \(Int(p.x)),\(Int(p.y)) \(Int(sz.width))x\(Int(sz.height))")
        }
    default:
        print("unknown command \(name)")
        exit(2)
    }
}
