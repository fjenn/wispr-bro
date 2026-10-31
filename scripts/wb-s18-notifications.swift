// SPDX-License-Identifier: GPL-3.0-or-later
// S-18 Mac test helper: reads and drives macOS notification banners through
// the Accessibility API of the NotificationCenter process, so the S-18
// procedure (docs/fork/MAC-TEST-SCRIPT.md) can click a banner or one of its
// buttons without a person. Needs Accessibility permission for the terminal
// that runs it. Touches only Notification Center's banners and the test
// app's own window.
//
//   swiftc -O -o /tmp/wbnotif scripts/wb-s18-notifications.swift
//   wbnotif dump                         # Notification Center's AX tree
//   wbnotif list                         # visible banners and their actions
//   wbnotif wait   <text> [seconds]      # until a banner shows <text>
//   wbnotif click  <text>                # AXPress (the notification's click)
//   wbnotif action <text> <action name>  # a named action (a button)
//   wbnotif frame  <text>                # x,y,w,h for `screencapture -R`
//   wbnotif app-press  <bundle id> <name>  # press a button in the app's window
//   wbnotif app-window <bundle id>         # window id for `screencapture -l`
//   wbnotif app-click  <bundle id> <x> <y> # click inside the app window (points)
//   wbnotif key-escape                     # one Esc to the frontmost app
//
// `<text>` matches a banner whose text contains it (title or body). macOS
// exposes a notification's buttons as named AX actions; on some releases
// they appear only after AXShowMenu (hover), which `action` tries. Use
// `key-escape` only while the test app is in front.
import AppKit
import ApplicationServices
import Foundation

func attr(_ e: AXUIElement, _ name: String) -> AnyObject? {
    var v: AnyObject?
    return AXUIElementCopyAttributeValue(e, name as CFString, &v) == .success ? v : nil
}

func str(_ e: AXUIElement, _ name: String) -> String {
    (attr(e, name) as? String) ?? ""
}

func children(_ e: AXUIElement) -> [AXUIElement] {
    (attr(e, kAXChildrenAttribute) as? [AXUIElement]) ?? []
}

func actions(_ e: AXUIElement) -> [String] {
    var names: CFArray?
    guard AXUIElementCopyActionNames(e, &names) == .success, let list = names as? [String] else {
        return []
    }
    return list
}

func center() -> AXUIElement? {
    guard let app = NSRunningApplication.runningApplications(
        withBundleIdentifier: "com.apple.notificationcenterui"
    ).first else { return nil }
    return AXUIElementCreateApplication(app.processIdentifier)
}

/// Text of an element and its descendants (titles, values, descriptions).
func text(_ e: AXUIElement, depth: Int = 0) -> String {
    if depth > 8 { return "" }
    var parts = [str(e, kAXDescriptionAttribute), str(e, kAXTitleAttribute)]
    if let v = attr(e, kAXValueAttribute) as? String { parts.append(v) }
    for c in children(e) { parts.append(text(c, depth: depth + 1)) }
    return parts.filter { !$0.isEmpty }.joined(separator: " | ")
}

/// Elements that look like a notification: subrole AXNotificationCenter*.
func banners(_ e: AXUIElement, depth: Int = 0, into out: inout [AXUIElement]) {
    if depth > 14 { return }
    if str(e, kAXSubroleAttribute).hasPrefix("AXNotificationCenter") {
        out.append(e)
        return
    }
    for c in children(e) { banners(c, depth: depth + 1, into: &out) }
}

func allBanners() -> [AXUIElement] {
    guard let root = center() else { return [] }
    var out: [AXUIElement] = []
    banners(root, into: &out)
    return out
}

func dump(_ e: AXUIElement, depth: Int = 0) {
    if depth > 14 { return }
    let pad = String(repeating: "  ", count: depth)
    print("\(pad)\(str(e, kAXRoleAttribute)) [\(str(e, kAXSubroleAttribute))] "
        + "desc=\(str(e, kAXDescriptionAttribute).debugDescription) "
        + "title=\(str(e, kAXTitleAttribute).debugDescription) actions=\(actions(e))")
    for c in children(e) { dump(c, depth: depth + 1) }
}

func find(_ needle: String) -> AXUIElement? {
    allBanners().first { text($0).localizedCaseInsensitiveContains(needle) }
}

func frame(_ e: AXUIElement) -> CGRect? {
    guard let p = attr(e, kAXPositionAttribute), let z = attr(e, kAXSizeAttribute),
          CFGetTypeID(p) == AXValueGetTypeID(), CFGetTypeID(z) == AXValueGetTypeID()
    else { return nil }
    var point = CGPoint.zero
    var size = CGSize.zero
    AXValueGetValue(p as! AXValue, .cgPoint, &point)  // type checked above
    AXValueGetValue(z as! AXValue, .cgSize, &size)
    return CGRect(origin: point, size: size)
}

func appElement(_ bundleId: String) -> (AXUIElement, pid_t)? {
    guard let app = NSRunningApplication.runningApplications(withBundleIdentifier: bundleId).first
    else { return nil }
    return (AXUIElementCreateApplication(app.processIdentifier), app.processIdentifier)
}

func findButton(_ e: AXUIElement, _ name: String, depth: Int = 0) -> AXUIElement? {
    if depth > 40 { return nil }
    let role = str(e, kAXRoleAttribute)
    if ["AXButton", "AXMenuItem", "AXCheckBox"].contains(role) {
        let labels = [str(e, kAXTitleAttribute), str(e, kAXDescriptionAttribute), text(e)]
        if labels.contains(where: { $0 == name || $0.hasPrefix(name) }) { return e }
    }
    for c in children(e) {
        if let hit = findButton(c, name, depth: depth + 1) { return hit }
    }
    return nil
}

let args = Array(CommandLine.arguments.dropFirst())
func arg(_ i: Int) -> String { i < args.count ? args[i] : "" }

guard AXIsProcessTrusted() else {
    FileHandle.standardError.write("needs Accessibility permission\n".data(using: .utf8)!)
    exit(2)
}
switch arg(0) {
case "dump":
    if let root = center() { dump(root) }
case "list":
    for b in allBanners() {
        print("\(str(b, kAXSubroleAttribute))\t\(text(b))\tactions=\(actions(b))")
    }
case "wait":
    let seconds = Double(arg(2)) ?? 15
    let deadline = Date().addingTimeInterval(seconds)
    while Date() < deadline {
        if let b = find(arg(1)) {
            print("found\t\(text(b))\tactions=\(actions(b))")
            exit(0)
        }
        Thread.sleep(forTimeInterval: 0.25)
    }
    print("not found: \(arg(1))")
    exit(1)
case "click":
    guard let b = find(arg(1)) else { print("not found"); exit(1) }
    let r = AXUIElementPerformAction(b, kAXPressAction as CFString)
    print("AXPress → \(r.rawValue)")
    exit(r == .success ? 0 : 1)
case "action":
    guard let b = find(arg(1)) else { print("not found"); exit(1) }
    let wanted = arg(2)
    var names = actions(b)
    if !names.contains(where: { $0.localizedCaseInsensitiveContains(wanted) }) {
        _ = AXUIElementPerformAction(b, kAXShowMenuAction as CFString)
        Thread.sleep(forTimeInterval: 0.6)
        names = actions(b)
    }
    guard let name = names.first(where: { $0.localizedCaseInsensitiveContains(wanted) }) else {
        print("no action \(wanted.debugDescription); actions=\(names)")
        exit(1)
    }
    let r = AXUIElementPerformAction(b, name as CFString)
    print("\(name.split(separator: "\n").first ?? "") → \(r.rawValue)")
    exit(r == .success ? 0 : 1)
case "frame":
    guard let b = find(arg(1)), let f = frame(b) else { print("not found"); exit(1) }
    print("\(Int(f.minX)),\(Int(f.minY)),\(Int(f.width)),\(Int(f.height))")
case "app-press":
    guard let (app, _) = appElement(arg(1)) else { print("app not running"); exit(1) }
    guard let button = findButton(app, arg(2)) else { print("no button \(arg(2).debugDescription)"); exit(1) }
    let r = AXUIElementPerformAction(button, kAXPressAction as CFString)
    print("pressed \(arg(2).debugDescription) → \(r.rawValue)")
    exit(r == .success ? 0 : 1)
case "app-window":
    guard let (_, pid) = appElement(arg(1)) else { print("app not running"); exit(1) }
    let info = CGWindowListCopyWindowInfo([.optionOnScreenOnly], kCGNullWindowID) as? [[String: Any]] ?? []
    let mine = info.filter {
        ($0[kCGWindowOwnerPID as String] as? pid_t) == pid && ($0[kCGWindowLayer as String] as? Int) == 0
    }
    guard let id = mine.first?[kCGWindowNumber as String] as? Int else { print("no window"); exit(1) }
    print(id)
case "app-click":
    // A left click at (x, y) points inside the test app's own window (the
    // webview's buttons are not all reachable through AX).
    guard let (_, pid) = appElement(arg(1)), let x = Double(arg(2)), let y = Double(arg(3)) else {
        print("usage: app-click <bundle id> <x> <y>"); exit(64)
    }
    let info = CGWindowListCopyWindowInfo([.optionOnScreenOnly], kCGNullWindowID) as? [[String: Any]] ?? []
    guard let w = info.first(where: {
        ($0[kCGWindowOwnerPID as String] as? pid_t) == pid && ($0[kCGWindowLayer as String] as? Int) == 0
    }), let b = w[kCGWindowBounds as String] as? [String: Double],
          let ox = b["X"], let oy = b["Y"] else { print("no window"); exit(1) }
    NSRunningApplication(processIdentifier: pid)?.activate()
    Thread.sleep(forTimeInterval: 0.3)
    let at = CGPoint(x: ox + x, y: oy + y)
    for type in [CGEventType.leftMouseDown, .leftMouseUp] {
        CGEvent(mouseEventSource: nil, mouseType: type, mouseCursorPosition: at, mouseButton: .left)?
            .post(tap: .cghidEventTap)
        Thread.sleep(forTimeInterval: 0.05)
    }
    print("clicked \(Int(at.x)),\(Int(at.y))")
case "key-escape":
    let source = CGEventSource(stateID: .hidSystemState)
    for down in [true, false] {
        CGEvent(keyboardEventSource: source, virtualKey: 53, keyDown: down)?.post(tap: .cghidEventTap)
        Thread.sleep(forTimeInterval: 0.05)
    }
    print("escape posted")
default:
    print("usage: dump | list | wait <text> [s] | click <text> | action <text> <name> | "
        + "frame <text> | app-press <bundle id> <name> | app-window <bundle id> | key-escape")
    exit(64)
}
