// SPDX-License-Identifier: GPL-3.0-or-later
//
// S-25 real-app driver (docs/fork/MAC-TEST-SCRIPT.md, S-25; S-16's driver
// plus the save panel keys): clicks and types into the test app's own window
// only (its save panel is a sheet of that window), so the Insights page can
// be used without a person. Every event is posted only while the given PID is
// the frontmost app; otherwise the run stops (exit 3) before posting.
//
//   swift scripts/wb-s25/ui.swift <pid> 'click 140 220; type Siobhán; key return'
//
// Commands (separated by `;`):
//   front                  bring the PID's app to the front (NSRunningApplication)
//   click <x> <y>          left click at a point in the app's largest window,
//                          in points from its top-left corner
//   clickown <x> <y>       click without `front`, only if this PID's window is on
//                          top there (a floating window of an accessory app)
//   scroll <x> <y> <n>     n wheel steps over a window point (negative = down)
//   type <text>            the text as Unicode key events (any script)
//   keys <text>            a path as US-layout key presses (the save panel)
//   key <name>             return | escape | tab | space | down | up | cmd-a | cmd-shift-g | cmd-d
//                          | cmd-c | cmd-v | cmd-x | cmd-z | cmd-shift-z | cmd-shift-slash (S-26a)
//                          | cmd-slash (S-26b) | cmd-s (OT2-D)
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
        // Normal and floating windows (the Scratchpad note floats at
        // layer 3-8); the largest one wins, so the Flow Bar never does.
        guard (w[kCGWindowOwnerPID as String] as? Int32) == pid,
              let layer = w[kCGWindowLayer as String] as? Int, (0...8).contains(layer),
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
        guard let layer = w[kCGWindowLayer as String] as? Int, (0...8).contains(layer),
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
            // Explicit flags, as in typeKeys: a modifier left in the
            // `.hidSystemState` source (e.g. a stuck fn) makes WebKit drop
            // every Unicode-string key event.
            e?.flags = []
            e?.keyboardSetUnicodeString(stringLength: units.count, unicodeString: units)
            post(e)
        }
    }
}

/// US-layout key codes for the characters of a path. The system save panel
/// runs out of process and ignores Unicode-string key events, so paths are
/// typed as real key presses.
let usKeys: [Character: CGKeyCode] = [
    "a": 0, "s": 1, "d": 2, "f": 3, "h": 4, "g": 5, "z": 6, "x": 7, "c": 8, "v": 9,
    "b": 11, "q": 12, "w": 13, "e": 14, "r": 15, "y": 16, "t": 17, "1": 18, "2": 19,
    "3": 20, "4": 21, "6": 22, "5": 23, "9": 25, "7": 26, "-": 27, "8": 28, "0": 29,
    "o": 31, "u": 32, "i": 34, "p": 35, "l": 37, "j": 38, "k": 40, "n": 45, "m": 46,
    ".": 47, "/": 44,
]

func typeKeys(_ text: String) {
    for ch in text {
        let lower = Character(ch.lowercased())
        guard let code = usKeys[lower] else { print("no key for \(ch)"); exit(2) }
        requireFront()
        for down in [true, false] {
            let e = CGEvent(keyboardEventSource: source, virtualKey: code, keyDown: down)
            // Explicit flags: an event from a `.hidSystemState` source can
            // otherwise carry a modifier from the previous command (a stray
            // ⌘ turns "m" into Minimize).
            e?.flags = ch.isUppercase ? .maskShift : []
            post(e)
        }
    }
}

let keys: [String: (CGKeyCode, CGEventFlags)] = [
    "return": (36, []), "escape": (53, []), "tab": (48, []), "space": (49, []),
    "down": (125, []), "up": (126, []), "cmd-a": (0, .maskCommand),
    "cmd-shift-g": (5, [.maskCommand, .maskShift]), "cmd-d": (2, .maskCommand),
    // S-26a: the Edit menu's keys and Help's search box (⌘?).
    "cmd-c": (8, .maskCommand), "cmd-v": (9, .maskCommand), "cmd-x": (7, .maskCommand),
    "cmd-z": (6, .maskCommand), "cmd-s": (1, .maskCommand), "cmd-shift-z": (6, [.maskCommand, .maskShift]),
    "cmd-shift-slash": (44, [.maskCommand, .maskShift]),
    "cmd-slash": (44, [.maskCommand]),
]

func key(_ name: String) {
    guard let (code, flags) = keys[name] else { print("unknown key \(name)"); exit(2) }
    requireFront()
    for down in [true, false] {
        let e = CGEvent(keyboardEventSource: source, virtualKey: code, keyDown: down)
        e?.flags = flags
        post(e)
    }
    // Both events carry the chord's flags, so the HID event state kept ⌘
    // "held" afterwards and the app's delivery waited on it ("modifier keys
    // were still held", Mac wave 7). A modifier event with no flags ends it.
    if !flags.isEmpty {
        let up = CGEvent(keyboardEventSource: source, virtualKey: 55, keyDown: false)
        up?.type = .flagsChanged
        up?.flags = []
        post(up)
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
    case "clickown":
        // clickown <x> <y>: a click that activates the app, for a window that
        // cannot be brought forward first (a floating note of an accessory
        // app). Posted only if this PID's window is on top at that point.
        let xy = parts[1].split(separator: " ").compactMap { Double($0) }
        guard let o = windowOrigin() else { print("ABORT: no window"); exit(3) }
        let p = CGPoint(x: o.x + CGFloat(xy[0]), y: o.y + CGFloat(xy[1]))
        guard topWindowOwner(at: p) == pid else { print("ABORT: another window is on top at \(p)"); exit(3) }
        for t in [CGEventType.mouseMoved, .leftMouseDown, .leftMouseUp] {
            post(CGEvent(mouseEventSource: source, mouseType: t, mouseCursorPosition: p, mouseButton: .left))
        }
    case "scroll":
        // scroll <x> <y> <lines>: wheel over a window point (negative = down)
        let v = parts[1].split(separator: " ").compactMap { Double($0) }
        guard let o = windowOrigin() else { print("ABORT: no window"); exit(3) }
        requireFront()
        let p = CGPoint(x: o.x + CGFloat(v[0]), y: o.y + CGFloat(v[1]))
        post(CGEvent(mouseEventSource: source, mouseType: .mouseMoved, mouseCursorPosition: p, mouseButton: .left))
        for _ in 0..<Int(abs(v[2])) {
            post(CGEvent(scrollWheelEvent2Source: source, units: .line, wheelCount: 1, wheel1: v[2] < 0 ? -3 : 3, wheel2: 0, wheel3: 0))
        }
    case "type":
        typeText(parts.count > 1 ? parts[1] : "")
    case "keys":
        typeKeys(parts.count > 1 ? parts[1] : "")
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
