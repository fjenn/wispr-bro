// SPDX-License-Identifier: GPL-3.0-or-later
//
// Mac-wave helper: a TextEdit instance of the driver's own, addressed by its
// pid (scripts/wb-lib/app.sh wb_textedit_open). AppleScript's
// `tell application "TextEdit"` and an activate by bundle id pick whichever
// TextEdit runs first, which may be the owner's with their documents open;
// these commands only ever touch the given pid. Posts no key or mouse event.
//
//   te text <pid>       the text of the instance's first text area (AX)
//   te point <pid>      centre of its frontmost window, global top-left points
//   te activate <pid>   brings that instance to the front
//   te front            {"pid":…,"bundle":…} of the frontmost app
//
// Build: swiftc -O -o <bin>/te scripts/wb-lib/te.swift

import AppKit
import ApplicationServices
import Foundation

func fail(_ msg: String) -> Never {
    FileHandle.standardError.write((msg + "\n").data(using: .utf8)!)
    exit(1)
}

func attr(_ el: AXUIElement, _ name: String) -> AnyObject? {
    var v: AnyObject?
    return AXUIElementCopyAttributeValue(el, name as CFString, &v) == .success ? v : nil
}

func textArea(_ el: AXUIElement, depth: Int = 0) -> AXUIElement? {
    if depth > 12 { return nil }
    if (attr(el, kAXRoleAttribute) as? String) == (kAXTextAreaRole as String) { return el }
    for child in (attr(el, kAXChildrenAttribute) as? [AXUIElement]) ?? [] {
        if let hit = textArea(child, depth: depth + 1) { return hit }
    }
    return nil
}

func pidArg() -> pid_t {
    guard CommandLine.arguments.count > 2, let p = Int32(CommandLine.arguments[2]) else {
        fail("usage: te text|point|activate <pid> | te front")
    }
    guard let app = NSRunningApplication(processIdentifier: p),
          app.bundleIdentifier == "com.apple.TextEdit" else { fail("pid \(p) is not TextEdit") }
    return p
}

let cmd = CommandLine.arguments.count > 1 ? CommandLine.arguments[1] : ""
switch cmd {
case "text":
    let app = AXUIElementCreateApplication(pidArg())
    let windows = (attr(app, kAXWindowsAttribute) as? [AXUIElement]) ?? []
    for w in windows {
        if let t = textArea(w) {
            print((attr(t, kAXValueAttribute) as? String) ?? "", terminator: "")
            exit(0)
        }
    }
    fail("no text area")
case "point":
    let p = pidArg()
    let list = (CGWindowListCopyWindowInfo([.optionOnScreenOnly, .excludeDesktopElements],
                                           kCGNullWindowID) as? [[String: Any]]) ?? []
    for w in list where (w[kCGWindowOwnerPID as String] as? Int32) == p
        && (w[kCGWindowLayer as String] as? Int ?? 1) == 0 {
        if let b = w[kCGWindowBounds as String] as? [String: Double],
           let x = b["X"], let y = b["Y"], let wd = b["Width"], let h = b["Height"], h > 60 {
            print(Int(x + wd / 2), Int(y + h / 2))
            exit(0)
        }
    }
    fail("no window")
case "activate":
    let p = pidArg()
    _ = NSRunningApplication(processIdentifier: p)?.activate(options: [])
    for _ in 0..<40 {
        RunLoop.current.run(until: Date(timeIntervalSinceNow: 0.05))
        if NSWorkspace.shared.frontmostApplication?.processIdentifier == p { exit(0) }
    }
    fail("pid \(p) did not come to the front")
case "front":
    RunLoop.current.run(until: Date(timeIntervalSinceNow: 0.002))
    let f = NSWorkspace.shared.frontmostApplication
    print("{\"pid\":\(f?.processIdentifier ?? -1),\"bundle\":\"\(f?.bundleIdentifier ?? "?")\"}")
default:
    fail("usage: te text|point|activate <pid> | te front")
}
