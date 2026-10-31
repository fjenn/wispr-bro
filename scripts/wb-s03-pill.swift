// SPDX-License-Identifier: GPL-3.0-or-later
//
// S-03 real-Mac driver: watches the Flow Bar's windows through the window
// server and drives it with synthetic fn and mouse events, so the bar's
// acceptance checks run without a person (docs/fork/MAC-TEST-SCRIPT.md,
// S-03). The fn events are the ones `wb-s01-keys.swift` posts.
//
//   swift scripts/wb-s03-pill.swift -e '<commands>'   (WB_PID=<app pid>)
//
// Commands (separated by `;`):
//   windows                 the app's on-screen windows (bounds in AppKit
//                           points, top-left origin; layer; alpha; sharing)
//   rest                    wait (≤ 3 s) until only the resting bar shows;
//                           prints its bounds
//   appear <n> <hold_ms>    n push-to-talk takes: fn down, time until an
//                           active window (height ≥ 30) is on screen and
//                           until the web view's window is, hold, fn up,
//                           wait for rest
//   hover <n>               n hovers: glide onto the resting bar from
//                           150 pt above, time until it grows (bubble),
//                           move away, time until it shrinks
//   clickbar                click the resting bar's centre
//   clickat <dx> <dy>       click at an offset from the active window's
//                           bottom-left (points, y up), e.g. its ✓
//   rclickbar               right-click the resting bar
//   drag <dx>               drag the resting bar horizontally by dx points
//   move <x> <y>            move the cursor (top-left origin)
//   fn down | fn up
//   key <name>              escape | return | space | x | s | a | n
//   cmd <name>              the same with ⌘ (e.g. `cmd s` saves)
//   wait <ms>
//   front                   the frontmost app
//   watchfront <ms>         print every change of the frontmost app
//   flags | clearmods       print / clear the HID modifier state
//   procs                   RSS (KiB) and CPU seconds of the app and of the
//                           WebKit processes it is responsible for
//   shot <path>             capture a 640 × 120 pt strip around the bar
//   mark <text>
//
// WB_FRONT=<app name>: every fn-down, key-down and click is posted only
// while that app is frontmost; otherwise the run aborts (exit 3) after
// releasing fn. Hold `/usr/bin/lockf -k ~/wb-bench/gui.lock` around GUI
// runs. Needs Accessibility (events) and Screen Recording (`shot`) for the
// process that runs it. Output: one JSON object per line, `t` in ms.

import AppKit
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

_ = pthread_set_qos_class_self_np(QOS_CLASS_USER_INTERACTIVE, 0)
let env = ProcessInfo.processInfo.environment
guard let pidText = env["WB_PID"], let appPid = Int32(pidText) else {
    out(["error": "set WB_PID=<app pid>"])
    exit(2)
}
let requiredFront = env["WB_FRONT"]
let source = CGEventSource(stateID: .hidSystemState)
var fnHeld = false

@_silgen_name("responsibility_get_pid_responsible_for_pid")
func responsiblePid(_ pid: pid_t) -> pid_t

func releaseFn() {
    if fnHeld {
        fnHeld = false
        let e = CGEvent(keyboardEventSource: source, virtualKey: 63, keyDown: true)
        e?.type = .flagsChanged
        e?.flags = []
        e?.post(tap: .cghidEventTap)
    }
}

/// The frontmost app's name. Spins the run loop first: without it a
/// command-line process keeps a stale `frontmostApplication`.
func frontName() -> String {
    RunLoop.current.run(until: Date(timeIntervalSinceNow: 0.002))
    return NSWorkspace.shared.frontmostApplication?.localizedName ?? "?"
}

func checkFront() {
    guard let want = requiredFront else { return }
    let front = frontName()
    if front != want {
        out(["abort": "frontmost is \(front), not \(want)"])
        releaseFn()
        exit(3)
    }
}

struct Win {
    let number: Int
    let x, y, w, h: Double
    let layer: Int
    let alpha: Double
    let sharing: Int
}

func windows() -> [Win] {
    guard let list = CGWindowListCopyWindowInfo([.optionOnScreenOnly], kCGNullWindowID)
        as? [[String: Any]] else { return [] }
    return list.compactMap { w in
        guard (w[kCGWindowOwnerPID as String] as? Int32) == appPid,
              let b = w[kCGWindowBounds as String] as? [String: Double] else { return nil }
        return Win(
            number: w[kCGWindowNumber as String] as? Int ?? 0,
            x: b["X"] ?? 0, y: b["Y"] ?? 0, w: b["Width"] ?? 0, h: b["Height"] ?? 0,
            layer: w[kCGWindowLayer as String] as? Int ?? 0,
            alpha: w[kCGWindowAlpha as String] as? Double ?? 0,
            sharing: w[kCGWindowSharingState as String] as? Int ?? -1)
    }
}

/// The bar's windows: status-level (layer 25), visible, not the Hub.
func barWindows() -> [Win] { windows().filter { $0.layer == 25 && $0.alpha > 0.01 } }

func describe(_ w: Win) -> [String: Any] {
    ["n": w.number, "x": w.x, "y": w.y, "w": w.w, "h": w.h, "alpha": w.alpha,
     "layer": w.layer, "sharing": w.sharing]
}

func restingBar() -> Win? {
    let bars = barWindows()
    return bars.count == 1 && bars[0].h < 30 ? bars[0] : nil
}

func activeBar() -> Win? { barWindows().first { $0.h >= 30 } }

func poll(_ timeoutMs: Double, _ cond: () -> Bool) -> Double? {
    let t0 = now()
    while now() - t0 < timeoutMs {
        if cond() { return now() - t0 }
        usleep(1000)
    }
    return nil
}

func fn(_ down: Bool) {
    if down { checkFront() }
    fnHeld = down
    let e = CGEvent(keyboardEventSource: source, virtualKey: 63, keyDown: true)
    e?.type = .flagsChanged
    e?.flags = down ? .maskSecondaryFn : []
    e?.post(tap: .cghidEventTap)
}

let keycodes: [String: CGKeyCode] = [
    "escape": 53, "return": 36, "space": 49, "x": 7, "s": 1, "a": 0, "n": 45,
]

func key(_ name: String, command: Bool = false) {
    guard let code = keycodes[name] else { out(["error": "unknown key \(name)"]); return }
    checkFront()
    // ⌘ as a real keyboard sends it: its own flagsChanged down and up, so
    // the HID modifier state never stays "held" afterwards.
    if command { commandEdge(true) }
    for down in [true, false] {
        let e = CGEvent(keyboardEventSource: source, virtualKey: code, keyDown: down)
        var flags: CGEventFlags = fnHeld ? .maskSecondaryFn : []
        if command { flags.insert(.maskCommand) }
        e?.flags = flags
        e?.post(tap: .cghidEventTap)
        usleep(20_000)
    }
    if command { commandEdge(false) }
}

func commandEdge(_ down: Bool) {
    let e = CGEvent(keyboardEventSource: source, virtualKey: 55, keyDown: down)
    e?.type = .flagsChanged
    var flags: CGEventFlags = fnHeld ? .maskSecondaryFn : []
    if down { flags.insert(.maskCommand) }
    e?.flags = flags
    e?.post(tap: .cghidEventTap)
    usleep(10_000)
}

func mouse(_ type: CGEventType, _ p: CGPoint, _ button: CGMouseButton = .left) {
    CGEvent(mouseEventSource: source, mouseType: type, mouseCursorPosition: p, mouseButton: button)?
        .post(tap: .cghidEventTap)
}

func move(_ p: CGPoint) { mouse(.mouseMoved, p) }

func click(_ p: CGPoint, right: Bool = false) {
    checkFront()
    move(p)
    usleep(30_000)
    mouse(right ? .rightMouseDown : .leftMouseDown, p, right ? .right : .left)
    usleep(20_000)
    mouse(right ? .rightMouseUp : .leftMouseUp, p, right ? .right : .left)
}

func centre(_ w: Win) -> CGPoint { CGPoint(x: w.x + w.w / 2, y: w.y + w.h / 2) }

func glide(from a: CGPoint, to b: CGPoint, steps: Int = 5, ms: Int = 40) {
    for i in 1...steps {
        let f = Double(i) / Double(steps)
        move(CGPoint(x: a.x + (b.x - a.x) * f, y: a.y + (b.y - a.y) * f))
        usleep(useconds_t(ms * 1000 / steps))
    }
}

func appear(_ n: Int, _ hold: Int) {
    for i in 0..<n {
        guard poll(3000, { restingBar() != nil || activeBar() == nil }) != nil else {
            out(["trial": i, "skip": "bar not at rest"])
            continue
        }
        let restNumber = restingBar()?.number
        let unixUs = Int(Date().timeIntervalSince1970 * 1e6)
        fn(true)
        let t0 = now()
        var firstActive: Double? = nil
        var web: Double? = nil
        var webWin: Win? = nil
        while now() - t0 < 2000 {
            if firstActive == nil, activeBar() != nil { firstActive = now() - t0 }
            // The web view's window (the native bar's own window grows
            // into the "bridge" first, under the same number).
            if web == nil, let w = barWindows().first(where: {
                $0.h >= 30 && $0.number != restNumber
            }) {
                web = now() - t0
                webWin = w
            }
            if firstActive != nil && web != nil { break }
            usleep(1000)
        }
        usleep(useconds_t(max(0, Double(hold) - (now() - t0)) * 1000))
        fn(false)
        let back = poll(6000) { restingBar() != nil }
        var r: [String: Any] = ["trial": i, "fn_down_unix_us": unixUs]
        r["active_ms"] = firstActive.map { ($0 * 10).rounded() / 10 } ?? NSNull()
        r["web_ms"] = web.map { ($0 * 10).rounded() / 10 } ?? NSNull()
        r["rest_after_up_ms"] = back.map { $0.rounded() } ?? NSNull()
        if let w = webWin { r["web"] = describe(w) }
        out(r)
        usleep(1_500_000)
    }
}

func hover(_ n: Int) {
    for i in 0..<n {
        guard let bar = restingBar() else {
            out(["trial": i, "skip": "no resting bar"])
            usleep(1_000_000)
            continue
        }
        let c = centre(bar)
        let above = CGPoint(x: c.x, y: c.y - 150)
        move(above)
        usleep(400_000)
        let t0 = now()
        glide(from: above, to: c)
        let grown = poll(1000) { (barWindows().first?.w ?? 0) > 100 }
        let grownWin = barWindows().first
        move(CGPoint(x: c.x + 400, y: c.y - 300))
        let t1 = now()
        let shrunk = poll(1000) { restingBar().map { $0.w < 100 } ?? false }
        var r: [String: Any] = ["trial": i, "glide_ms": (t1 - t0).rounded()]
        r["bubble_ms"] = grown.map { ($0 * 10).rounded() / 10 } ?? NSNull()
        r["shrink_ms"] = shrunk.map { ($0 * 10).rounded() / 10 } ?? NSNull()
        if let w = grownWin { r["bubble"] = describe(w) }
        out(r)
        usleep(500_000)
    }
}

func procs() {
    let task = Process()
    task.executableURL = URL(fileURLWithPath: "/bin/ps")
    task.arguments = ["-A", "-o", "pid=,rss=,time=,etime=,comm="]
    let pipe = Pipe()
    task.standardOutput = pipe
    try? task.run()
    let data = pipe.fileHandleForReading.readDataToEndOfFile()
    task.waitUntilExit()
    let text = String(data: data, encoding: .utf8) ?? ""
    var rows: [[String: Any]] = []
    var total = 0
    // An app started from a terminal shares the terminal's responsible
    // process with its WebKit services, so a WebKit process is the app's
    // when it has the same responsible process and is not older than the
    // app (the `age_s` column lets a run audit this).
    func seconds(_ etime: Substring) -> Double {
        let dayParts = etime.split(separator: "-")
        let days = dayParts.count == 2 ? Double(dayParts[0]) ?? 0 : 0
        let hms = (dayParts.last ?? "").split(separator: ":").compactMap { Double($0) }
        let s = hms.reversed().enumerated().reduce(0.0) { $0 + $1.element * pow(60, Double($1.offset)) }
        return days * 86400 + s
    }
    var appAge = Double.infinity
    let lines = text.split(separator: "\n").map {
        $0.split(separator: " ", omittingEmptySubsequences: true)
    }
    for parts in lines where parts.count >= 5 && Int32(parts[0]) == appPid {
        appAge = seconds(parts[3])
    }
    let appResp = responsiblePid(appPid)
    for parts in lines {
        guard parts.count >= 5, let pid = Int32(parts[0]), let rss = Int(parts[1]) else { continue }
        let comm = parts[4...].joined(separator: " ")
        let age = seconds(parts[3])
        let ours = pid == appPid
            || (comm.contains("com.apple.WebKit") && responsiblePid(pid) == appResp
                && age <= appAge + 1)
        if !ours { continue }
        let t = parts[2].split(separator: ":").compactMap { Double($0) }
        let cpu = t.count == 2 ? t[0] * 60 + t[1] : (t.count == 3 ? t[0] * 3600 + t[1] * 60 + t[2] : 0)
        rows.append(["pid": Int(pid), "rss_kb": rss, "cpu_s": cpu, "age_s": age,
                     "name": String(comm.split(separator: "/").last ?? "")])
        total += rss
    }
    out(["procs": rows, "total_rss_kb": total])
}

func shot(_ path: String) {
    let b = barWindows().first
    let c = b.map(centre) ?? CGPoint(x: 0, y: 0)
    let r = "\(Int(c.x - 320)),\(Int(c.y - 80)),640,120"
    let task = Process()
    task.executableURL = URL(fileURLWithPath: "/usr/sbin/screencapture")
    task.arguments = ["-x", "-R", r, path]
    try? task.run()
    task.waitUntilExit()
    out(["shot": path, "rect": r])
}

func activeOffset(_ dx: Double, _ dy: Double) -> CGPoint? {
    guard let w = activeBar() else { return nil }
    return CGPoint(x: w.x + dx, y: w.y + w.h - dy)
}

var script = ""
let args = CommandLine.arguments
if args.count >= 3 && args[1] == "-e" { script = args[2] }
else if args.count >= 2 { script = (try? String(contentsOfFile: args[1], encoding: .utf8)) ?? "" }

signal(SIGINT) { _ in releaseFn(); exit(130) }

for raw in script.split(whereSeparator: { $0 == ";" || $0 == "\n" }) {
    let p = raw.split(separator: " ").map(String.init)
    guard let cmd = p.first else { continue }
    func arg(_ i: Int) -> String { i < p.count ? p[i] : "" }
    switch cmd {
    case "windows": out(["windows": windows().map(describe)])
    case "rest":
        let ok = poll(3000) { restingBar() != nil }
        out(["rest": ok != nil, "bar": restingBar().map(describe) ?? NSNull()])
    case "appear": appear(Int(arg(1)) ?? 1, Int(arg(2)) ?? 900)
    case "hover": hover(Int(arg(1)) ?? 1)
    case "clickbar":
        if let b = restingBar() {
            let t0 = now()
            click(centre(b))
            let a = poll(2000) { activeBar() != nil }
            out(["ev": "clickbar", "active_ms": a.map { ($0 * 10).rounded() / 10 } ?? NSNull(),
                 "clicked_ms": (now() - t0).rounded(),
                 "front": frontName()])
        } else { out(["error": "no resting bar"]) }
    case "rclickbar":
        if let b = restingBar() { click(centre(b), right: true); out(["ev": "rclickbar"]) }
        else { out(["error": "no resting bar"]) }
    case "clickat":
        if let pt = activeOffset(Double(arg(1)) ?? 0, Double(arg(2)) ?? 0) {
            click(pt)
            out(["ev": "clickat", "x": pt.x, "y": pt.y,
                 "front": frontName()])
        } else { out(["error": "no active bar"]) }
    case "drag":
        if let b = restingBar() {
            checkFront()
            let a = centre(b)
            let dx = Double(arg(1)) ?? 0
            move(a)
            usleep(300_000)
            mouse(.leftMouseDown, a)
            for i in 1...12 {
                mouse(.leftMouseDragged, CGPoint(x: a.x + dx * Double(i) / 12, y: a.y))
                usleep(15_000)
            }
            mouse(.leftMouseUp, CGPoint(x: a.x + dx, y: a.y))
            usleep(300_000)
            move(CGPoint(x: a.x + dx, y: a.y - 300))
            out(["ev": "drag", "from": describe(b), "to": restingBar().map(describe) ?? NSNull()])
        } else { out(["error": "no resting bar"]) }
    case "displays":
        var ids = [CGDirectDisplayID](repeating: 0, count: 8)
        var n: UInt32 = 0
        CGGetActiveDisplayList(8, &ids, &n)
        out(["displays": ids.prefix(Int(n)).map { id -> [String: Any] in
            let b = CGDisplayBounds(id)
            return ["id": Int(id), "x": b.minX, "y": b.minY, "w": b.width, "h": b.height,
                    "pixels_w": CGDisplayPixelsWide(id)]
        }])
    case "follow":
        // Move the cursor to (x, y) and time until the resting bar sits on
        // the display under it.
        let p = CGPoint(x: Double(arg(1)) ?? 0, y: Double(arg(2)) ?? 0)
        var id: CGDirectDisplayID = 0
        var n: UInt32 = 0
        CGGetDisplaysWithPoint(p, 1, &id, &n)
        let bounds = CGDisplayBounds(id)
        move(p)
        let dt = poll(5000) {
            guard let b = restingBar() else { return false }
            return bounds.contains(centre(b))
        }
        out(["ev": "follow", "display": Int(id), "ms": dt.map { $0.rounded() } ?? NSNull(),
             "bar": restingBar().map(describe) ?? NSNull()])
    case "click":
        click(CGPoint(x: Double(arg(1)) ?? 0, y: Double(arg(2)) ?? 0))
        out(["ev": "click", "front": frontName()])
    case "move": move(CGPoint(x: Double(arg(1)) ?? 0, y: Double(arg(2)) ?? 0))
    case "fn": fn(arg(1) == "down"); out(["ev": "fn_\(arg(1))"])
    case "key": key(arg(1)); out(["ev": "key", "key": arg(1)])
    case "cmd": key(arg(1), command: true); out(["ev": "cmd", "key": arg(1)])
    case "wait": usleep(useconds_t((Double(arg(1)) ?? 0) * 1000))
    case "watchfront":
        // Every change of the frontmost app for <ms>.
        let t0 = now()
        var last = frontName()
        out(["front": last, "at_ms": 0])
        while now() - t0 < (Double(arg(1)) ?? 1000) {
            let f = frontName()
            if f != last { out(["front": f, "at_ms": (now() - t0).rounded()]); last = f }
            usleep(3000)
        }
    case "flags":
        out(["hid_flags": CGEventSource.flagsState(.hidSystemState).rawValue,
             "combined_flags": CGEventSource.flagsState(.combinedSessionState).rawValue])
    case "clearmods": commandEdge(false); out(["ev": "clearmods"])
    case "front": out(["front": frontName()])
    case "procs": procs()
    case "shot": shot(arg(1))
    case "shotactive":
        // Capture only the active bar's window (window id), at 2x.
        if let w = activeBar() {
            let task = Process()
            task.executableURL = URL(fileURLWithPath: "/usr/sbin/screencapture")
            task.arguments = ["-x", "-o", "-l\(w.number)", arg(1)]
            try? task.run()
            task.waitUntilExit()
            out(["shot": arg(1), "window": describe(w)])
        } else { out(["error": "no active bar"]) }
    case "mark": out(["mark": p.dropFirst().joined(separator: " ")])
    default: out(["error": "unknown command \(cmd)"])
    }
}
releaseFn()
