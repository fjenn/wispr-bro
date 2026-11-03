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
//                         description is <title> (exit 4 if none in 10 s)
//   wait-text <text>      wait up to 60 s for a static text / heading
//                         containing <text> (exit 5 on timeout)
//   field                 print the practice field's value ("field: …")
//   select-all            select all of the practice field's text
//   key <name>            opt-1 | opt-o | ctrl-opt-1 | cmd-z | escape
//   wait <ms>             sleep
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
    for down in [true, false] {
        let e = CGEvent(keyboardEventSource: src, virtualKey: code, keyDown: down)
        e?.flags = flags
        e?.post(tap: .cghidEventTap)
        usleep(30_000)
    }
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
    case "press":
        guard let b = poll(10, {
            str($0, kAXRoleAttribute) == kAXButtonRole as String
                && (str($0, kAXTitleAttribute) == rest || str($0, kAXDescriptionAttribute) == rest)
        }) else { print("press \(rest): not found"); exit(4) }
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
    default:
        print("unknown command \(name)")
        exit(2)
    }
}
