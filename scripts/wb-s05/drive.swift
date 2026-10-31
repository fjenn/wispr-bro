// SPDX-License-Identifier: GPL-3.0-or-later
//
// S-05 real-Mac helper (docs/fork/MAC-TEST-SCRIPT.md S-05). Posts no key
// events; the only click it posts lands on an empty spot of the desktop.
//
//   drive stopswitch <pid> <delay_ms> <bundle id>
//       SIGUSR2 to the test app (Handy's toggle: stops the take = key-up),
//       then after <delay_ms> activates the app <bundle id> (a window this
//       run opened). For "focus moved after key-up → card".
//   drive activate <bundle id>
//   drive desktop
//       clicks a spot of the main display that no window covers (the
//       desktop), and waits until Finder is frontmost. Prints the point.
//   drive front
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

func pump(_ s: Double) { RunLoop.current.run(until: Date(timeIntervalSinceNow: s)) }

func front() -> String {
    pump(0.002)
    return NSWorkspace.shared.frontmostApplication?.bundleIdentifier ?? "?"
}

func activate(_ bundle: String) -> Bool {
    guard let app = NSRunningApplication.runningApplications(withBundleIdentifier: bundle).first
    else { return false }
    return app.activate(options: [])
}

/// A point (top-left origin, points) on the main display that no window
/// above the desktop covers, away from the right edge (desktop icons).
func emptyDesktopPoint() -> CGPoint? {
    guard let screen = NSScreen.screens.first else { return nil }
    let full = screen.frame
    let vis = screen.visibleFrame
    // visibleFrame in top-left coordinates.
    let top = full.maxY - vis.maxY
    let list = (CGWindowListCopyWindowInfo([.optionOnScreenOnly, .excludeDesktopElements],
                                           kCGNullWindowID) as? [[String: Any]]) ?? []
    let rects: [CGRect] = list.compactMap { w in
        guard let b = w[kCGWindowBounds as String] as? [String: Double],
              (w[kCGWindowLayer as String] as? Int ?? 0) >= 0,
              (w[kCGWindowAlpha as String] as? Double ?? 1) > 0 else { return nil }
        return CGRect(x: b["X"] ?? 0, y: b["Y"] ?? 0, width: b["Width"] ?? 0, height: b["Height"] ?? 0)
    }
    let step = 40.0
    var y = top + 60
    while y < top + vis.height - 140 {
        var x = vis.minX + 60
        while x < vis.minX + vis.width * 0.6 {
            let p = CGPoint(x: x, y: y)
            if !rects.contains(where: { $0.insetBy(dx: -12, dy: -12).contains(p) }) {
                return p
            }
            x += step
        }
        y += step
    }
    return nil
}

func click(_ p: CGPoint) {
    let src = CGEventSource(stateID: .hidSystemState)
    for type in [CGEventType.mouseMoved, .leftMouseDown, .leftMouseUp] {
        let e = CGEvent(mouseEventSource: src, mouseType: type, mouseCursorPosition: p,
                        mouseButton: .left)
        e?.post(tap: .cghidEventTap)
        usleep(40_000)
    }
}

let a = CommandLine.arguments
switch a.count > 1 ? a[1] : "" {
case "stopswitch":
    guard a.count == 5, let pid = Int32(a[2]), let delay = Double(a[3]) else {
        out(["error": "stopswitch <pid> <delay_ms> <bundle>"]); exit(2)
    }
    kill(pid, SIGUSR2)
    out(["ev": "sigusr2", "front": front()])
    usleep(useconds_t(delay * 1000))
    let ok = activate(a[4])
    out(["ev": "activate", "bundle": a[4], "ok": ok])
    for _ in 0..<40 {
        if front() == a[4] { break }
        pump(0.005)
    }
    out(["ev": "front", "front": front()])
case "activate":
    let ok = activate(a[2])
    for _ in 0..<100 {
        if front() == a[2] { break }
        pump(0.01)
    }
    out(["ev": "activate", "ok": ok, "front": front()])
case "desktop":
    guard let p = emptyDesktopPoint() else { out(["error": "no empty desktop spot"]); exit(3) }
    click(p)
    for _ in 0..<100 {
        if front() == "com.apple.finder" { break }
        pump(0.01)
    }
    out(["ev": "desktop", "x": p.x, "y": p.y, "front": front()])
case "front":
    out(["front": front()])
default:
    out(["error": "usage: drive stopswitch|activate|desktop|front"])
    exit(2)
}
