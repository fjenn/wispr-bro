// SPDX-License-Identifier: GPL-3.0-or-later
//
// S-38 soak helper: the AX read-back of R5 S-38's harness. Reads (or
// clears) the focused text element of the frontmost app. It posts no key
// or mouse events. Needs Accessibility for the process that runs it.
//
//   axread front          {"front": "<bundle id>"}
//   axread value          {"front": …, "role": …, "value": "<text>"}
//                         (the field holds only the run's own dictations)
//   axread clear          sets the focused element's value to "" (so each
//                         take starts from an empty field)
//   axread activate <pid> brings that process to the front (by pid, never by
//                         bundle id: the owner's TextEdit has the same one)
// Every reply carries "pid" (the frontmost process). With WB_FRONT_PID set,
// `clear` and `value` refuse (exit 5) unless that process is frontmost, so
// a field of another instance (the owner's) is never read or cleared.
//
// Build once: swiftc -O -o <bin>/axread scripts/wb-soak/axread.swift

import AppKit
import ApplicationServices
import Foundation

func out(_ d: [String: Any]) {
    if let data = try? JSONSerialization.data(withJSONObject: d, options: [.sortedKeys]),
       let s = String(data: data, encoding: .utf8) {
        print(s)
        fflush(stdout)
    }
}

RunLoop.current.run(until: Date(timeIntervalSinceNow: 0.002))
let args = Array(CommandLine.arguments.dropFirst())
if args.first == "activate" {
    guard args.count > 1, let p = Int32(args[1]),
          let target = NSRunningApplication(processIdentifier: p) else {
        out(["error": "activate <pid>: no such process"])
        exit(2)
    }
    target.activate(options: [])
    for _ in 0..<40 {
        RunLoop.current.run(until: Date(timeIntervalSinceNow: 0.05))
        if NSWorkspace.shared.frontmostApplication?.processIdentifier == p { break }
    }
    let f = NSWorkspace.shared.frontmostApplication
    out(["front": f?.bundleIdentifier ?? "?", "pid": Int(f?.processIdentifier ?? 0), "activated": f?.processIdentifier == p])
    exit(0)
}
guard let app = NSWorkspace.shared.frontmostApplication else {
    out(["error": "no frontmost app"])
    exit(3)
}
let bundle = app.bundleIdentifier ?? "?"
let cmd = CommandLine.arguments.dropFirst().first ?? "value"
let frontPid = Int(app.processIdentifier)
if cmd == "front" {
    out(["front": bundle, "pid": frontPid])
    exit(0)
}
if let want = ProcessInfo.processInfo.environment["WB_FRONT_PID"], let w = Int(want), w != frontPid {
    out(["front": bundle, "pid": frontPid, "error": "front pid \(frontPid) is not \(w); not touched"])
    exit(5)
}

let axApp = AXUIElementCreateApplication(app.processIdentifier)
var focused: CFTypeRef?
guard AXUIElementCopyAttributeValue(axApp, kAXFocusedUIElementAttribute as CFString, &focused) == .success,
      let element = focused, CFGetTypeID(element) == AXUIElementGetTypeID() else {
    out(["front": bundle, "error": "no focused element"])
    exit(4)
}
let el = element as! AXUIElement
var role: CFTypeRef?
AXUIElementCopyAttributeValue(el, kAXRoleAttribute as CFString, &role)
let roleName = (role as? String) ?? "?"

switch cmd {
case "clear":
    let rc = AXUIElementSetAttributeValue(el, kAXValueAttribute as CFString, "" as CFString)
    out(["front": bundle, "pid": frontPid, "role": roleName, "cleared": rc == .success])
case "value":
    var value: CFTypeRef?
    AXUIElementCopyAttributeValue(el, kAXValueAttribute as CFString, &value)
    out(["front": bundle, "pid": frontPid, "role": roleName, "value": (value as? String) ?? ""])
default:
    out(["error": "unknown command \(cmd)"])
    exit(2)
}
