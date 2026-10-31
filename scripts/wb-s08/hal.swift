// SPDX-License-Identifier: GPL-3.0-or-later
//
// Mac test S-08 helper (docs/fork/MAC-TEST-SCRIPT.md): reads and changes the
// CoreAudio state the slice depends on, and answers the two macOS
// microphone dialogs by Accessibility. Never opens an input stream.
//
//   swiftc -O -o /tmp/wbhal scripts/wb-s08/hal.swift
//   wbhal state                         JSON: default input/output, output
//                                       mute and volume scalars (exact bits),
//                                       every input with its transport
//   wbhal set-default-input <uid>       make <uid> the default input
//   wbhal agg-hold <name> <uid> <sub-uid> <stop-file> <max-s>
//                                       create a public aggregate input
//                                       wrapping <sub-uid> (a stand-in USB
//                                       mic), keep it until <stop-file>
//                                       exists or <max-s> passed, destroy it
//   wbhal buttons <process>             titles of the buttons in <process>'s
//                                       windows (for system dialogs only)
//   wbhal press <process> <title> [timeout-s]
//                                       wait for and press that button
//
// The terminal running it needs Accessibility for `buttons`/`press`.
import ApplicationServices
import AppKit
import CoreAudio
import Foundation

func fourcc(_ v: UInt32) -> String {
    let b = [24, 16, 8, 0].map { UInt8((v >> UInt32($0)) & 0xff) }
    return String(bytes: b, encoding: .ascii) ?? "\(v)"
}

func addr(_ sel: AudioObjectPropertySelector, _ scope: AudioObjectPropertyScope = kAudioObjectPropertyScopeGlobal, _ el: UInt32 = kAudioObjectPropertyElementMain) -> AudioObjectPropertyAddress {
    AudioObjectPropertyAddress(mSelector: sel, mScope: scope, mElement: el)
}

func u32(_ obj: AudioObjectID, _ sel: AudioObjectPropertySelector, _ scope: AudioObjectPropertyScope = kAudioObjectPropertyScopeGlobal, _ el: UInt32 = kAudioObjectPropertyElementMain) -> UInt32? {
    var a = addr(sel, scope, el)
    var v: UInt32 = 0
    var size = UInt32(MemoryLayout<UInt32>.size)
    return AudioObjectGetPropertyData(obj, &a, 0, nil, &size, &v) == 0 ? v : nil
}

func f32(_ obj: AudioObjectID, _ sel: AudioObjectPropertySelector, _ scope: AudioObjectPropertyScope, _ el: UInt32) -> Float32? {
    var a = addr(sel, scope, el)
    var v: Float32 = 0
    var size = UInt32(MemoryLayout<Float32>.size)
    return AudioObjectGetPropertyData(obj, &a, 0, nil, &size, &v) == 0 ? v : nil
}

func str(_ obj: AudioObjectID, _ sel: AudioObjectPropertySelector) -> String? {
    var a = addr(sel)
    var v: Unmanaged<CFString>?
    var size = UInt32(MemoryLayout<Unmanaged<CFString>?>.size)
    guard AudioObjectGetPropertyData(obj, &a, 0, nil, &size, &v) == 0, let s = v else { return nil }
    return s.takeRetainedValue() as String
}

func ids(_ obj: AudioObjectID, _ sel: AudioObjectPropertySelector, _ scope: AudioObjectPropertyScope = kAudioObjectPropertyScopeGlobal) -> [AudioObjectID] {
    var a = addr(sel, scope)
    var size: UInt32 = 0
    guard AudioObjectGetPropertyDataSize(obj, &a, 0, nil, &size) == 0, size > 0 else { return [] }
    var out = [AudioObjectID](repeating: 0, count: Int(size) / MemoryLayout<AudioObjectID>.size)
    guard AudioObjectGetPropertyData(obj, &a, 0, nil, &size, &out) == 0 else { return [] }
    return out
}

let system = AudioObjectID(kAudioObjectSystemObject)

func devices() -> [AudioObjectID] { ids(system, kAudioHardwarePropertyDevices) }

func device(uid: String) -> AudioObjectID? { devices().first { str($0, kAudioDevicePropertyDeviceUID) == uid } }

func describe(_ d: AudioObjectID) -> [String: Any] {
    [
        "uid": str(d, kAudioDevicePropertyDeviceUID) ?? "",
        "name": str(d, kAudioObjectPropertyName) ?? "",
        "transport": fourcc(u32(d, kAudioDevicePropertyTransportType) ?? 0),
    ]
}

func out(_ v: Any) {
    let data = try! JSONSerialization.data(withJSONObject: v, options: [.sortedKeys])
    print(String(data: data, encoding: .utf8)!)
}

func state() {
    var s: [String: Any] = [:]
    if let i = u32(system, kAudioHardwarePropertyDefaultInputDevice) { s["default_input"] = describe(i) }
    if let o = u32(system, kAudioHardwarePropertyDefaultOutputDevice) {
        var od = describe(o)
        let scope = kAudioDevicePropertyScopeOutput
        od["mute"] = u32(o, kAudioDevicePropertyMute, scope).map { Int($0) } ?? -1
        var vols: [String: Any] = [:]
        for el: UInt32 in [0, 1, 2] {
            if let v = f32(o, kAudioDevicePropertyVolumeScalar, scope, el) {
                vols["\(el)"] = ["value": v, "bits": String(v.bitPattern, radix: 16)]
            }
        }
        od["volume"] = vols
        s["default_output"] = od
    }
    s["inputs"] = devices().filter { !ids($0, kAudioDevicePropertyStreams, kAudioObjectPropertyScopeInput).isEmpty }.map(describe)
    out(s)
}

func setDefaultInput(_ uid: String) -> Bool {
    guard var d = device(uid: uid) else { return false }
    var a = addr(kAudioHardwarePropertyDefaultInputDevice)
    return AudioObjectSetPropertyData(system, &a, 0, nil, UInt32(MemoryLayout<AudioObjectID>.size), &d) == 0
}

func aggHold(name: String, uid: String, sub: String, stop: String, maxS: Double) {
    let desc: [String: Any] = [
        kAudioAggregateDeviceNameKey: name,
        kAudioAggregateDeviceUIDKey: uid,
        kAudioAggregateDeviceIsPrivateKey: 0,
        kAudioAggregateDeviceMainSubDeviceKey: sub,
        kAudioAggregateDeviceSubDeviceListKey: [[kAudioSubDeviceUIDKey: sub]],
    ]
    var id: AudioObjectID = 0
    let err = AudioHardwareCreateAggregateDevice(desc as CFDictionary, &id)
    guard err == 0 else { out(["error": "create \(err)"]); exit(1) }
    out(["created": id, "uid": uid])
    fflush(stdout)
    let start = Date()
    while !FileManager.default.fileExists(atPath: stop) && Date().timeIntervalSince(start) < maxS {
        Thread.sleep(forTimeInterval: 0.02)
    }
    let destroyed = AudioHardwareDestroyAggregateDevice(id)
    out(["destroyed": destroyed, "after_s": Date().timeIntervalSince(start)])
}

func apps(named name: String) -> [pid_t] {
    NSWorkspace.shared.runningApplications.filter { ($0.localizedName ?? "") == name || ($0.bundleIdentifier ?? "").hasSuffix(name) }.map { $0.processIdentifier }
}

func attr(_ e: AXUIElement, _ a: String) -> AnyObject? {
    var v: AnyObject?
    return AXUIElementCopyAttributeValue(e, a as CFString, &v) == .success ? v : nil
}

func buttons(_ e: AXUIElement, _ depth: Int = 0, _ acc: inout [(String, AXUIElement)]) {
    guard depth < 12 else { return }
    if (attr(e, kAXRoleAttribute) as? String) == kAXButtonRole {
        acc.append(((attr(e, kAXTitleAttribute) as? String) ?? "", e))
    }
    for c in (attr(e, kAXChildrenAttribute) as? [AXUIElement]) ?? [] { buttons(c, depth + 1, &acc) }
}

func allButtons(_ process: String) -> [(String, AXUIElement)] {
    var acc: [(String, AXUIElement)] = []
    for pid in apps(named: process) {
        let app = AXUIElementCreateApplication(pid)
        for w in (attr(app, kAXWindowsAttribute) as? [AXUIElement]) ?? [] { buttons(w, 0, &acc) }
    }
    return acc
}

let args = CommandLine.arguments
switch args.count > 1 ? args[1] : "" {
case "state": state()
case "set-default-input": exit(setDefaultInput(args[2]) ? 0 : 1)
case "agg-hold": aggHold(name: args[2], uid: args[3], sub: args[4], stop: args[5], maxS: Double(args[6]) ?? 30)
case "buttons": out(allButtons(args[2]).map { $0.0 })
case "press":
    let timeout = args.count > 4 ? Double(args[4]) ?? 10 : 10
    let start = Date()
    while Date().timeIntervalSince(start) < timeout {
        if let b = allButtons(args[2]).first(where: { $0.0 == args[3] }) {
            let r = AXUIElementPerformAction(b.1, kAXPressAction as CFString)
            out(["pressed": r == .success, "after_s": Date().timeIntervalSince(start)])
            exit(r == .success ? 0 : 1)
        }
        Thread.sleep(forTimeInterval: 0.1)
    }
    out(["pressed": false, "note": "no such button"])
    exit(1)
default:
    FileHandle.standardError.write("usage: see the header\n".data(using: .utf8)!)
    exit(2)
}
