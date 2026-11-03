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
guard let app = NSWorkspace.shared.frontmostApplication else {
    out(["error": "no frontmost app"])
    exit(3)
}
let bundle = app.bundleIdentifier ?? "?"
let cmd = CommandLine.arguments.dropFirst().first ?? "value"
if cmd == "front" {
    out(["front": bundle])
    exit(0)
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
    out(["front": bundle, "role": roleName, "cleared": rc == .success])
case "value":
    var value: CFTypeRef?
    AXUIElementCopyAttributeValue(el, kAXValueAttribute as CFString, &value)
    out(["front": bundle, "role": roleName, "value": (value as? String) ?? ""])
default:
    out(["error": "unknown command \(cmd)"])
    exit(2)
}
