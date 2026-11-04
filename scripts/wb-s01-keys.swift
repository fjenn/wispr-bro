// SPDX-License-Identifier: GPL-3.0-or-later
//
// S-01 real-Mac driver: posts synthetic fn / key events with CGEvent and
// watches the microphone, so the hotkey acceptance checks run without a
// person at the keyboard (docs/fork/MAC-TEST-SCRIPT.md, S-01).
//
//   swift scripts/wb-s01-keys.swift <script-file>
//   swift scripts/wb-s01-keys.swift -e 'fn down; wait 900; fn up'
//
// Script commands (one per line or separated by `;`):
//   fn down | fn up             flagsChanged for the fn key (keycode 63,
//                               SecondaryFn flag)
//   mods down ctrl,option | mods up ctrl,option
//   key <name> [down|up]        a key press (both edges unless given), with
//                               the modifiers currently held
//   wait <ms>                   sleep
//   waitmic on|off <ms>         wait until the default input device runs
//                               (or stops) somewhere; prints how long after
//                               the last `fn down` / `mods down` it changed
//   mic                         print whether the input runs now, and the
//                               PIDs running input (WB_MIC_PID=<pid> limits
//                               every mic check to that process)
//   watchmic <ms>               sample the mic every 2 ms for <ms>; prints
//                               whether it ran at all (for "no indicator")
//   mark <text>                 print a marker line
//   flags                       print fn in the HID and combined flag state
//   click <x> <y>               a left click at the global point (top-left
//                               origin), with the modifiers currently held
//                               (OT2-A: clicking into another window while
//                               the trigger is held)
//   activate <bundle id>        brings that app to the front (an app switch
//                               without keys); prints the new frontmost app
//   front                       prints the frontmost app's bundle id
//   expect <app name>           from here on, WB_FRONT is <app name> (a run
//                               that switches apps on purpose moves its
//                               guard with it)
//
// WB_LISTEN=1 adds a listen-only tap at the tail of the session taps and
// prints, at the end, the keycodes of every key-down that got past the taps
// before it (so a chord key the app swallowed is missing from the list).
// Real keyboards send fn+arrows as Home/End/PageUp/PageDown and fn+Delete
// as Forward Delete with the fn flag; post those keys (`key home` while fn
// is down) to reproduce a real chord.
//
// Output: one JSON object per line, `t` in ms since start.
//
// WB_FRONT=<app name> (e.g. TextEdit): every key-down and modifier-down is
// posted only while that app is frontmost; otherwise the run aborts (exit
// 3) after releasing anything held. Run GUI bursts under the shared lock:
// `/usr/bin/lockf -k ~/wb-bench/gui.lock …`.
//
// Notes: synthetic events reach event taps (the fork's tap sees them) but
// do not trigger the Globe key's system action, so DEC-44 behaviour is
// covered by RS-01, not by this tool. The mic check reads
// kAudioDevicePropertyDeviceIsRunningSomewhere, the state behind the orange
// indicator. Needs Accessibility for the process that runs it.

import AppKit
import CoreAudio
import CoreGraphics
import Foundation

let start = DispatchTime.now().uptimeNanoseconds
func now() -> Double { Double(DispatchTime.now().uptimeNanoseconds - start) / 1e6 }
func out(_ dict: [String: Any]) {
    var d = dict
    d["t"] = (now() * 10).rounded() / 10
    if let data = try? JSONSerialization.data(withJSONObject: d, options: [.sortedKeys]),
       let s = String(data: data, encoding: .utf8) {
        print(s)
        fflush(stdout)
    }
}

let keycodes: [String: CGKeyCode] = [
    "a": 0, "s": 1, "d": 2, "f": 3, "h": 4, "g": 5, "z": 6, "x": 7, "c": 8, "v": 9,
    "b": 11, "q": 12, "w": 13, "e": 14, "r": 15, "y": 16, "t": 17, "o": 31, "u": 32,
    "i": 34, "p": 35, "l": 37, "j": 38, "k": 40, "n": 45, "m": 46,
    // S-21 transforms (⌥1, ⌥2) and Command Mode tests.
    "1": 18, "2": 19, "3": 20, "4": 21, "5": 23, "6": 22, "7": 26, "8": 28, "9": 25, "0": 29,
    "return": 36, "tab": 48, "space": 49, "delete": 51, "escape": 53,
    "forwarddelete": 117, "home": 115, "end": 119, "pageup": 116, "pagedown": 121,
    "left": 123, "right": 124, "down": 125, "up": 126,
    "f1": 122, "f2": 120, "f3": 99, "f4": 118, "f5": 96, "f6": 97, "f7": 98, "f8": 100,
    "f9": 101, "f10": 109, "f11": 103, "f12": 111,
]
let modFlags: [String: (CGEventFlags, CGKeyCode)] = [
    "ctrl": (.maskControl, 59), "option": (.maskAlternate, 58),
    "shift": (.maskShift, 56), "command": (.maskCommand, 55),
]

let source = CGEventSource(stateID: .hidSystemState)
// Timing matters (taps vs T_show): ask for the interactive QoS class.
_ = pthread_set_qos_class_self_np(QOS_CLASS_USER_INTERACTIVE, 0)
var held: CGEventFlags = []
var lastTrigger: Double = 0

/// WB_FRONT=<app name>: post nothing unless that app is frontmost. Several
/// agents may drive this Mac's GUI; a burst whose target lost focus is
/// aborted (held modifiers released) instead of typing into another app.
var requiredFront = ProcessInfo.processInfo.environment["WB_FRONT"]

func checkFront() {
    guard let want = requiredFront else { return }
    let front = NSWorkspace.shared.frontmostApplication?.localizedName ?? "?"
    if front != want {
        out(["abort": "frontmost is \(front), not \(want)"])
        if !held.isEmpty {
            held = []
            let e = CGEvent(keyboardEventSource: source, virtualKey: 63, keyDown: true)
            e?.type = .flagsChanged
            e?.flags = []
            e?.post(tap: .cghidEventTap)
        }
        exit(3)
    }
}

func post(_ e: CGEvent?) {
    // Releases always go through (never leave a key held).
    if let e = e, e.type == .keyDown || (e.type == .flagsChanged && !e.flags.isEmpty) {
        checkFront()
    }
    e?.post(tap: .cghidEventTap)
}

func flagsEvent(keycode: CGKeyCode, flags: CGEventFlags) {
    let e = CGEvent(keyboardEventSource: source, virtualKey: keycode, keyDown: true)
    e?.type = .flagsChanged
    e?.flags = flags
    post(e)
}

func fnEdge(_ down: Bool) {
    if down { held.insert(.maskSecondaryFn) } else { held.remove(.maskSecondaryFn) }
    flagsEvent(keycode: 63, flags: held)
    if down { lastTrigger = now() }
    out(["ev": down ? "fn_down" : "fn_up"])
}

func modsEdge(_ names: [String], _ down: Bool) {
    for n in names {
        guard let (flag, code) = modFlags[n] else { continue }
        if down { held.insert(flag) } else { held.remove(flag) }
        flagsEvent(keycode: code, flags: held)
    }
    if down { lastTrigger = now() }
    out(["ev": down ? "mods_down" : "mods_up", "mods": names.joined(separator: "+")])
}

func key(_ name: String, edge: String?) {
    guard let code = keycodes[name] else { out(["error": "unknown key \(name)"]); return }
    var flags = held
    // Arrow and navigation keys carry SecondaryFn and NumericPad on macOS.
    if [115, 116, 117, 119, 121, 123, 124, 125, 126].contains(code) {
        flags.insert(.maskSecondaryFn)
        if code >= 123 { flags.insert(.maskNumericPad) }
    }
    if edge == nil || edge == "down" {
        let e = CGEvent(keyboardEventSource: source, virtualKey: code, keyDown: true)
        e?.flags = flags
        post(e)
    }
    if edge == nil { usleep(20_000) }
    if edge == nil || edge == "up" {
        let e = CGEvent(keyboardEventSource: source, virtualKey: code, keyDown: false)
        e?.flags = flags
        post(e)
    }
    out(["ev": "key", "key": name, "after_trigger_ms": held.isEmpty ? -1 : (now() - lastTrigger).rounded()])
}

func click(_ x: Double, _ y: Double) {
    let p = CGPoint(x: x, y: y)
    checkFront()
    for type in [CGEventType.leftMouseDown, .leftMouseUp] {
        let e = CGEvent(mouseEventSource: source, mouseType: type, mouseCursorPosition: p,
                        mouseButton: .left)
        e?.flags = held
        e?.post(tap: .cghidEventTap)
        usleep(30_000)
    }
    out(["ev": "click", "x": x, "y": y, "held_fn": held.contains(.maskSecondaryFn),
         "after_trigger_ms": held.isEmpty ? -1 : (now() - lastTrigger).rounded()])
}

func frontBundle() -> String {
    RunLoop.current.run(until: Date(timeIntervalSinceNow: 0.002))
    return NSWorkspace.shared.frontmostApplication?.bundleIdentifier ?? "?"
}

func activate(_ bundle: String) {
    let app = NSRunningApplication.runningApplications(withBundleIdentifier: bundle).first
    _ = app?.activate(options: [])
    let t0 = now()
    while frontBundle() != bundle && now() - t0 < 2000 { usleep(10_000) }
    out(["ev": "activate", "bundle": bundle, "front": frontBundle(),
         "after_trigger_ms": held.isEmpty ? -1 : (now() - lastTrigger).rounded()])
}

func defaultInput() -> AudioObjectID {
    var id = AudioObjectID(0)
    var size = UInt32(MemoryLayout<AudioObjectID>.size)
    var addr = AudioObjectPropertyAddress(
        mSelector: kAudioHardwarePropertyDefaultInputDevice,
        mScope: kAudioObjectPropertyScopeGlobal, mElement: kAudioObjectPropertyElementMain)
    AudioObjectGetPropertyData(AudioObjectID(kAudioObjectSystemObject), &addr, 0, nil, &size, &id)
    return id
}

/// PIDs of the processes running audio input now (macOS 14+ process
/// objects; the same per-app state the indicator reports).
func inputPIDs() -> [Int32] {
    var addr = AudioObjectPropertyAddress(
        mSelector: kAudioHardwarePropertyProcessObjectList,
        mScope: kAudioObjectPropertyScopeGlobal, mElement: kAudioObjectPropertyElementMain)
    var size = UInt32(0)
    let sys = AudioObjectID(kAudioObjectSystemObject)
    guard AudioObjectGetPropertyDataSize(sys, &addr, 0, nil, &size) == noErr else { return [] }
    var ids = [AudioObjectID](repeating: 0, count: Int(size) / MemoryLayout<AudioObjectID>.size)
    guard AudioObjectGetPropertyData(sys, &addr, 0, nil, &size, &ids) == noErr else { return [] }
    var pids: [Int32] = []
    for id in ids {
        var running = UInt32(0)
        var rsize = UInt32(MemoryLayout<UInt32>.size)
        var raddr = AudioObjectPropertyAddress(
            mSelector: kAudioProcessPropertyIsRunningInput,
            mScope: kAudioObjectPropertyScopeGlobal, mElement: kAudioObjectPropertyElementMain)
        AudioObjectGetPropertyData(id, &raddr, 0, nil, &rsize, &running)
        if running == 0 { continue }
        var pid = Int32(0)
        var psize = UInt32(MemoryLayout<Int32>.size)
        var paddr = AudioObjectPropertyAddress(
            mSelector: kAudioProcessPropertyPID,
            mScope: kAudioObjectPropertyScopeGlobal, mElement: kAudioObjectPropertyElementMain)
        AudioObjectGetPropertyData(id, &paddr, 0, nil, &psize, &pid)
        pids.append(pid)
    }
    return pids
}

/// With `WB_MIC_PID` set, only that process counts (other apps may hold
/// the mic); otherwise the device running for anyone.
let micPid: Int32? = ProcessInfo.processInfo.environment["WB_MIC_PID"].flatMap { Int32($0) }

func micRunning() -> Bool {
    if let pid = micPid { return inputPIDs().contains(pid) }
    var running = UInt32(0)
    var size = UInt32(MemoryLayout<UInt32>.size)
    var addr = AudioObjectPropertyAddress(
        mSelector: kAudioDevicePropertyDeviceIsRunningSomewhere,
        mScope: kAudioObjectPropertyScopeGlobal, mElement: kAudioObjectPropertyElementMain)
    AudioObjectGetPropertyData(defaultInput(), &addr, 0, nil, &size, &running)
    return running != 0
}

/// Sleeps `ms` precisely: a plain sleep overshoots by tens of ms when the
/// system is busy (timer coalescing), which turns a 100 ms tap into a
/// 150 ms press. Sleep to 3 ms before the target, then spin.
func preciseWait(_ ms: Double) {
    let target = now() + ms
    let coarse = ms - 3
    if coarse > 0 { usleep(UInt32(coarse * 1000)) }
    while now() < target {}
}

func run(_ line: String) {
    let parts = line.split(separator: " ").map(String.init)
    guard let cmd = parts.first else { return }
    switch cmd {
    case "fn": fnEdge(parts.count > 1 && parts[1] == "down")
    case "mods":
        modsEdge(parts.count > 2 ? parts[2].split(separator: ",").map(String.init) : [],
                 parts.count > 1 && parts[1] == "down")
    case "key": key(parts.count > 1 ? parts[1] : "", edge: parts.count > 2 ? parts[2] : nil)
    case "wait": preciseWait(Double(parts.count > 1 ? parts[1] : "0") ?? 0)
    case "mic": out(["mic": micRunning(), "input_pids": inputPIDs()])
    case "waitmic":
        let want = parts.count > 1 && parts[1] == "on"
        let limit = Double(parts.count > 2 ? parts[2] : "2000") ?? 2000
        let t0 = now()
        while micRunning() != want && now() - t0 < limit { usleep(500) }
        let ok = micRunning() == want
        out(["waitmic": want ? "on" : "off", "ok": ok,
             "after_trigger_ms": ((now() - lastTrigger) * 10).rounded() / 10])
    case "watchmic":
        let limit = Double(parts.count > 1 ? parts[1] : "1000") ?? 1000
        let t0 = now()
        var ever = false
        while now() - t0 < limit {
            if micRunning() { ever = true }
            usleep(2000)
        }
        out(["watchmic_ms": limit, "mic_ran": ever])
    case "mark": out(["mark": parts.dropFirst().joined(separator: " ")])
    case "flags":
        out(["hid_fn": CGEventSource.flagsState(.hidSystemState).contains(.maskSecondaryFn),
             "combined_fn": CGEventSource.flagsState(.combinedSessionState).contains(.maskSecondaryFn)])
    case "click":
        click(Double(parts.count > 1 ? parts[1] : "0") ?? 0, Double(parts.count > 2 ? parts[2] : "0") ?? 0)
    case "activate": activate(parts.count > 1 ? parts[1] : "")
    case "front": out(["front": frontBundle()])
    case "expect": requiredFront = parts.dropFirst().joined(separator: " ")
    default: out(["error": "unknown command \(line)"])
    }
}

let args = CommandLine.arguments.dropFirst()
var text = ""
if args.first == "-e" {
    text = args.dropFirst().joined(separator: " ")
} else if let path = args.first, let s = try? String(contentsOfFile: path, encoding: .utf8) {
    text = s
} else {
    FileHandle.standardError.write("usage: wb-s01-keys.swift <script> | -e '<commands>'\n".data(using: .utf8)!)
    exit(2)
}
// WB_LISTEN=1: a listen-only tap appended at the *tail* of the session
// taps counts every key-down that got past the taps before it (the app's
// blocking tap included), so a chord whose key reaches the focused app
// shows up here and a swallowed one does not.
final class Seen { var keys: [Int64] = [] }
let seen = Seen()
if ProcessInfo.processInfo.environment["WB_LISTEN"] == "1" {
    let mask = CGEventMask(1 << CGEventType.keyDown.rawValue)
    let callback: CGEventTapCallBack = { _, _, event, refcon in
        let s = Unmanaged<Seen>.fromOpaque(refcon!).takeUnretainedValue()
        s.keys.append(event.getIntegerValueField(.keyboardEventKeycode))
        return Unmanaged.passUnretained(event)
    }
    if let tap = CGEvent.tapCreate(tap: .cgSessionEventTap, place: .tailAppendEventTap,
                                   options: .listenOnly, eventsOfInterest: mask,
                                   callback: callback,
                                   userInfo: Unmanaged.passUnretained(seen).toOpaque()) {
        let src = CFMachPortCreateRunLoopSource(nil, tap, 0)
        Thread {
            CFRunLoopAddSource(CFRunLoopGetCurrent(), src, .commonModes)
            CGEvent.tapEnable(tap: tap, enable: true)
            CFRunLoopRun()
        }.start()
        usleep(50_000)
    } else {
        out(["error": "cannot create the listen tap"])
    }
}

for raw in text.split(whereSeparator: { $0 == "\n" || $0 == ";" }) {
    let line = raw.trimmingCharacters(in: .whitespaces)
    if line.isEmpty || line.hasPrefix("#") { continue }
    run(line)
}
// Never leave a modifier held.
if !held.isEmpty {
    held = []
    flagsEvent(keycode: 63, flags: [])
}
if ProcessInfo.processInfo.environment["WB_LISTEN"] == "1" {
    usleep(100_000)
    out(["passed_keycodes": seen.keys])
}
