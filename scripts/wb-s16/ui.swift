// SPDX-License-Identifier: GPL-3.0-or-later
//
// S-16 real-app driver (docs/fork/MAC-TEST-SCRIPT.md, S-16): clicks and
// types into the test app's own window only, so the Dictionary page can be
// used without a person. Every event is posted only while the given PID is
// the frontmost app; otherwise the run stops (exit 3) before posting.
//
//   swift scripts/wb-s16/ui.swift <pid> 'click 140 220; type Siobhán; key return'
//
// Commands (separated by `;`):
//   front                  bring the PID's app to the front (NSRunningApplication)
//   click <x> <y>          left click at a point in the app's largest window,
//                          in points from its top-left corner
//   type <text>            the text as Unicode key events (any script)
//   key <name>             return | escape | tab | space | down | up | cmd-a | cmd-shift-d
//   wait <ms>              sleep
// Prints one line per command.
import AppKit
import CoreGraphics
import Foundation

let args = CommandLine.arguments
guard args.count >= 3, let pid = Int32(args[1]) else {
    FileHandle.standardError.write("usage: ui.swift <pid> '<commands>'\n".data(using: .utf8)!)
    exit(2)
}

func windowOrigin() -> CGPoint? {
    let list = CGWindowListCopyWindowInfo([.optionOnScreenOnly], kCGNullWindowID) as? [[String: Any]] ?? []
    var best: CGRect?
    for w in list {
        guard (w[kCGWindowOwnerPID as String] as? Int32) == pid,
              (w[kCGWindowLayer as String] as? Int) == 0,
              let b = w[kCGWindowBounds as String] as? [String: CGFloat] else { continue }
        let r = CGRect(x: b["X"]!, y: b["Y"]!, width: b["Width"]!, height: b["Height"]!)
        if best == nil || r.width * r.height > best!.width * best!.height { best = r }
    }
    return best?.origin
}

/// PID owning the frontmost normal window that contains `p`.
func topWindowOwner(at p: CGPoint) -> Int32? {
    let list = CGWindowListCopyWindowInfo([.optionOnScreenOnly], kCGNullWindowID) as? [[String: Any]] ?? []
    for w in list {  // front to back
        guard (w[kCGWindowLayer as String] as? Int) == 0,
              let b = w[kCGWindowBounds as String] as? [String: CGFloat] else { continue }
        let r = CGRect(x: b["X"]!, y: b["Y"]!, width: b["Width"]!, height: b["Height"]!)
        if r.contains(p) { return w[kCGWindowOwnerPID as String] as? Int32 }
    }
    return nil
}

func frontPid() -> Int32? {
    // NSWorkspace updates `frontmostApplication` from notifications, which a
    // command-line tool only receives while its run loop turns.
    RunLoop.current.run(until: Date(timeIntervalSinceNow: 0.05))
    return NSWorkspace.shared.frontmostApplication?.processIdentifier
}

func requireFront() {
    guard frontPid() == pid else {
        print("ABORT: pid \(pid) is not frontmost")
        exit(3)
    }
}

let source = CGEventSource(stateID: .hidSystemState)

func post(_ e: CGEvent?) {
    e?.post(tap: .cghidEventTap)
    usleep(12_000)
}

func click(_ x: CGFloat, _ y: CGFloat) {
    guard let o = windowOrigin() else { print("ABORT: no window"); exit(3) }
    let p = CGPoint(x: o.x + x, y: o.y + y)
    requireFront()
    post(CGEvent(mouseEventSource: source, mouseType: .mouseMoved, mouseCursorPosition: p, mouseButton: .left))
    post(CGEvent(mouseEventSource: source, mouseType: .leftMouseDown, mouseCursorPosition: p, mouseButton: .left))
    post(CGEvent(mouseEventSource: source, mouseType: .leftMouseUp, mouseCursorPosition: p, mouseButton: .left))
}

func typeText(_ text: String) {
    for ch in text {
        requireFront()
        let units = Array(String(ch).utf16)
        for down in [true, false] {
            let e = CGEvent(keyboardEventSource: source, virtualKey: 0, keyDown: down)
            e?.keyboardSetUnicodeString(stringLength: units.count, unicodeString: units)
            post(e)
        }
    }
}

let keys: [String: (CGKeyCode, CGEventFlags)] = [
    "return": (36, []), "escape": (53, []), "tab": (48, []), "space": (49, []),
    "down": (125, []), "up": (126, []), "cmd-a": (0, .maskCommand),
    // S-12b: the fork debug panel (⌘⇧D).
    "cmd-shift-d": (2, [.maskCommand, .maskShift]),
]

func key(_ name: String) {
    guard let (code, flags) = keys[name] else { print("unknown key \(name)"); exit(2) }
    requireFront()
    for down in [true, false] {
        let e = CGEvent(keyboardEventSource: source, virtualKey: code, keyDown: down)
        e?.flags = flags
        post(e)
    }
}

for raw in args[2].split(separator: ";") {
    let cmd = raw.trimmingCharacters(in: .whitespaces)
    if cmd.isEmpty { continue }
    let parts = cmd.split(separator: " ", maxSplits: 1).map(String.init)
    switch parts[0] {
    case "front":
        NSRunningApplication(processIdentifier: pid)?.activate(options: [])
        usleep(600_000)
        if frontPid() != pid {
            // A background process may not activate another app on recent
            // macOS. Click the app's own empty title strip instead, but only
            // when the topmost window at that point is the app's.
            guard let o = windowOrigin(), topWindowOwner(at: CGPoint(x: o.x + 600, y: o.y + 12)) == pid else {
                print("ABORT: the app's window is covered")
                exit(3)
            }
            let p = CGPoint(x: o.x + 600, y: o.y + 12)
            post(CGEvent(mouseEventSource: source, mouseType: .leftMouseDown, mouseCursorPosition: p, mouseButton: .left))
            post(CGEvent(mouseEventSource: source, mouseType: .leftMouseUp, mouseCursorPosition: p, mouseButton: .left))
            usleep(600_000)
        }
        requireFront()
    case "click":
        let xy = parts[1].split(separator: " ").compactMap { Double($0) }
        click(CGFloat(xy[0]), CGFloat(xy[1]))
    case "type":
        typeText(parts.count > 1 ? parts[1] : "")
    case "key":
        key(parts[1])
    case "wait":
        usleep(UInt32(Int(parts[1])! * 1000))
    default:
        print("unknown command \(cmd)")
        exit(2)
    }
    print("ok \(cmd)")
}
