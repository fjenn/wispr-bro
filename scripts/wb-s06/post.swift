// SPDX-License-Identifier: GPL-3.0-or-later
//
// S-06 real-Mac helper (docs/fork/MAC-TEST-SCRIPT.md S-06): posts the
// synthetic chords, keys and mouse buttons the recovery shortcuts react to,
// at the HID tap, so the fork's tap sees them like a real keyboard or mouse.
//
//   post front                          frontmost bundle id
//   post chord <mods> <key> [hold_ms]   e.g. `chord ctrl,command v 600`:
//                                       modifiers down, key down/up, the
//                                       modifiers stay held hold_ms, then up
//   post key <name>                     a plain key press (escape, return)
//   post mouse <2|3|4> [x y]            other-mouse button down/up (2 middle,
//                                       3 back, 4 forward) at a point (top-left
//                                       points; default: where the pointer is)
//   post flags                          modifier flags as the system sees them
//
// WB_FRONT=<bundle id> is required for chord, key and mouse: the event is
// posted only while that app is frontmost (a window the run opened);
// otherwise nothing is posted and the exit code is 3. Anything held is
// released before exiting.
//
// Output: one JSON object per line, `t` in ms since start.

import AppKit
import CoreGraphics
import Foundation

let start = DispatchTime.now().uptimeNanoseconds
func now() -> Double { Double(DispatchTime.now().uptimeNanoseconds - start) / 1e6 }
func out(_ d: [String: Any]) {
    var d = d
    d["t"] = (now() * 10).rounded() / 10
    if let data = try? JSONSerialization.data(withJSONObject: d, options: [.sortedKeys]),
       let s = String(data: data, encoding: .utf8) {
        print(s)
        fflush(stdout)
    }
}

func front() -> String {
    RunLoop.current.run(until: Date(timeIntervalSinceNow: 0.002))
    return NSWorkspace.shared.frontmostApplication?.bundleIdentifier ?? "?"
}

let keycodes: [String: CGKeyCode] = [
    "a": 0, "c": 8, "v": 9, "z": 6, "x": 7, "escape": 53, "return": 36, "space": 49,
    "f13": 105, "tab": 48,
]
let modifiers: [String: (CGEventFlags, CGKeyCode)] = [
    "ctrl": (.maskControl, 59), "option": (.maskAlternate, 58),
    "shift": (.maskShift, 56), "command": (.maskCommand, 55),
]
let source = CGEventSource(stateID: .hidSystemState)

func guardFront() {
    guard let want = ProcessInfo.processInfo.environment["WB_FRONT"] else {
        out(["error": "WB_FRONT is required"])
        exit(2)
    }
    let f = front()
    if f != want {
        out(["refused": "front is \(f), expected \(want)"])
        exit(3)
    }
}

func flagsEvent(_ code: CGKeyCode, _ flags: CGEventFlags) {
    let e = CGEvent(keyboardEventSource: source, virtualKey: code, keyDown: true)!
    e.type = .flagsChanged
    e.flags = flags
    e.post(tap: .cghidEventTap)
}

func keyEvent(_ code: CGKeyCode, down: Bool, _ flags: CGEventFlags) {
    let e = CGEvent(keyboardEventSource: source, virtualKey: code, keyDown: down)!
    e.flags = flags
    e.post(tap: .cghidEventTap)
}

func chord(_ mods: [String], _ key: String, hold: Double) {
    guard let code = keycodes[key] else { out(["error": "unknown key \(key)"]); exit(2) }
    guardFront()
    var held: CGEventFlags = []
    var codes: [(CGEventFlags, CGKeyCode)] = []
    for m in mods {
        guard let (flag, mcode) = modifiers[m] else { out(["error": "unknown modifier \(m)"]); exit(2) }
        held.insert(flag)
        codes.append((flag, mcode))
        flagsEvent(mcode, held)
        usleep(15_000)
    }
    out(["ev": "mods_down", "mods": mods])
    keyEvent(code, down: true, held)
    usleep(40_000)
    keyEvent(code, down: false, held)
    out(["ev": "key", "key": key])
    usleep(useconds_t(hold * 1000))
    for (flag, mcode) in codes.reversed() {
        held.remove(flag)
        flagsEvent(mcode, held)
        usleep(15_000)
    }
    out(["ev": "mods_up"])
}

func mouse(_ button: Int64, at point: CGPoint?) {
    guardFront()
    let p = point ?? CGEvent(source: nil)!.location
    let button = CGMouseButton(rawValue: UInt32(button)) ?? .center
    let down = CGEvent(mouseEventSource: source, mouseType: .otherMouseDown,
                       mouseCursorPosition: p, mouseButton: button)!
    down.setIntegerValueField(.mouseEventButtonNumber, value: Int64(button.rawValue))
    let up = CGEvent(mouseEventSource: source, mouseType: .otherMouseUp,
                     mouseCursorPosition: p, mouseButton: button)!
    up.setIntegerValueField(.mouseEventButtonNumber, value: Int64(button.rawValue))
    down.post(tap: .cghidEventTap)
    usleep(60_000)
    up.post(tap: .cghidEventTap)
    out(["ev": "mouse", "button": button.rawValue, "x": p.x, "y": p.y])
}

let a = Array(CommandLine.arguments.dropFirst())
func arg(_ i: Int) -> String { i < a.count ? a[i] : "" }
switch arg(0) {
case "front": out(["front": front()])
case "flags":
    let f = CGEventSource.flagsState(.hidSystemState)
    out(["flags": f.rawValue])
case "chord":
    let mods = arg(1).split(separator: ",").map(String.init)
    chord(mods, arg(2), hold: Double(arg(3)) ?? 0)
case "key":
    guard let code = keycodes[arg(1)] else { out(["error": "unknown key \(arg(1))"]); exit(2) }
    guardFront()
    keyEvent(code, down: true, [])
    usleep(30_000)
    keyEvent(code, down: false, [])
    out(["ev": "key", "key": arg(1)])
case "mouse":
    let x = Double(arg(2)), y = Double(arg(3))
    mouse(Int64(arg(1)) ?? 2, at: (x != nil && y != nil) ? CGPoint(x: x!, y: y!) : nil)
default:
    FileHandle.standardError.write("usage: post front|flags|chord|key|mouse …\n".data(using: .utf8)!)
    exit(2)
}
