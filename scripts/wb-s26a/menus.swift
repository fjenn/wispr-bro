// SPDX-License-Identifier: GPL-3.0-or-later
//
// S-26a real-app probe (docs/fork/MAC-TEST-SCRIPT.md, S-26a): reads the test
// app's menus through the Accessibility API. It posts no key or mouse events
// (key presses go through scripts/wb-s25/ui.swift, which checks that the app
// is in front first). The terminal needs Accessibility.
//
//   swiftc -O scripts/wb-s26a/menus.swift -o <scratch>/wbmenus
//   wbmenus <pid> menubar            the app menu as indented lines:
//                                    title [disabled] [⌘key] [✓]
//   wbmenus <pid> tray               the status item's count (0 or 1+)
//   wbmenus <pid> wait-tray <s>      polls until the status item exists again
//                                    (exit 1 after <s> seconds)
//   wbmenus <pid> press <A>/<B>[/<C>] AXPress a menu item by titles, e.g.
//                                    'Dictionary/Add word…'
//   wbmenus <pid> focused            the focused element's role and value
//   wbmenus <pid> help-results       titles of the rows macOS's Help search
//                                    shows (after `ui.swift … key
//                                    cmd-shift-slash; type <query>`)
//   wbmenus 0 clip-save <file>       saves the general pasteboard (all types)
//   wbmenus 0 clip-restore <file>    restores it
//   wbmenus 0 clip-text              prints the pasteboard's plain text
import AppKit
import ApplicationServices
import Foundation

let args = CommandLine.arguments
guard args.count >= 3, let pid = Int32(args[1]) else {
    FileHandle.standardError.write("usage: menus.swift <pid> <command> [arg]\n".data(using: .utf8)!)
    exit(2)
}
let command = args[2]
let argument = args.count > 3 ? args[3] : ""

func attr(_ e: AXUIElement, _ name: String) -> AnyObject? {
    var value: AnyObject?
    return AXUIElementCopyAttributeValue(e, name as CFString, &value) == .success ? value : nil
}

func children(_ e: AXUIElement) -> [AXUIElement] {
    (attr(e, kAXChildrenAttribute) as? [AXUIElement]) ?? []
}

func title(_ e: AXUIElement) -> String {
    (attr(e, kAXTitleAttribute) as? String) ?? ""
}

/// A menu bar item's or menu item's items (through its AXMenu child).
func items(_ e: AXUIElement) -> [AXUIElement] {
    children(e).flatMap { child -> [AXUIElement] in
        (attr(child, kAXRoleAttribute) as? String) == kAXMenuRole ? children(child) : []
    }
}

func describe(_ e: AXUIElement) -> String {
    var line = title(e)
    if line.isEmpty { return "—" }
    if (attr(e, kAXEnabledAttribute) as? Bool) == false { line += " [disabled]" }
    if let key = attr(e, kAXMenuItemCmdCharAttribute) as? String, !key.isEmpty {
        let mods = (attr(e, kAXMenuItemCmdModifiersAttribute) as? Int) ?? 0
        // kAXMenuItemModifier: 1 shift, 2 option, 4 control, 8 no command.
        var prefix = ""
        if mods & 4 != 0 { prefix += "⌃" }
        if mods & 2 != 0 { prefix += "⌥" }
        if mods & 1 != 0 { prefix += "⇧" }
        if mods & 8 == 0 { prefix += "⌘" }
        line += " [\(prefix)\(key)]"
    }
    if let mark = attr(e, kAXMenuItemMarkCharAttribute) as? String, !mark.isEmpty {
        line += " [✓]"
    }
    return line
}

func dump(_ e: AXUIElement, depth: Int) {
    for item in items(e) {
        print(String(repeating: "  ", count: depth) + describe(item))
        if depth < 3 { dump(item, depth: depth + 1) }
    }
}

let app = AXUIElementCreateApplication(pid)

func find(path: [String]) -> AXUIElement? {
    guard let bar = attr(app, kAXMenuBarAttribute) else { return nil }
    var current = [bar as! AXUIElement]
    var found: AXUIElement?
    for (i, name) in path.enumerated() {
        let pool = i == 0 ? children(current[0]) : items(current[0])
        guard let next = pool.first(where: { title($0) == name }) else { return nil }
        found = next
        current = [next]
    }
    return found
}

func trayCount() -> Int {
    guard let extras = attr(app, "AXExtrasMenuBar") else { return 0 }
    return children(extras as! AXUIElement).count
}

guard command.hasPrefix("clip-") || AXIsProcessTrusted() else {
    print("ABORT: this terminal needs Accessibility")
    exit(3)
}

switch command {
case "menubar":
    guard let bar = attr(app, kAXMenuBarAttribute) else { print("no menu bar"); exit(1) }
    for top in children(bar as! AXUIElement) {
        print(describe(top))
        dump(top, depth: 1)
    }
case "tray":
    print(trayCount())
case "wait-tray":
    let deadline = Date().addingTimeInterval(Double(argument) ?? 30)
    while Date() < deadline {
        if trayCount() > 0 { print("tray back"); exit(0) }
        usleep(250_000)
    }
    print("tray missing")
    exit(1)
case "press":
    guard let item = find(path: argument.split(separator: "/").map(String.init)) else {
        print("no item \(argument)"); exit(1)
    }
    let result = AXUIElementPerformAction(item, kAXPressAction as CFString)
    print(result == .success ? "pressed \(argument)" : "press failed \(result.rawValue)")
case "focused":
    guard let focused = attr(app, kAXFocusedUIElementAttribute) else { print("none"); exit(1) }
    let e = focused as! AXUIElement
    print("\((attr(e, kAXRoleAttribute) as? String) ?? "?") \((attr(e, kAXValueAttribute) as? String) ?? "")")
case "help-results":
    // While the Help menu is open with a query, its first items are the
    // search field and the matched menu items / topics.
    guard let help = find(path: [argument.isEmpty ? "Help" : argument]) else { print("no Help menu"); exit(1) }
    for item in items(help) { print(describe(item)) }
case "clip-save":
    let board = NSPasteboard.general
    var saved: [[String: String]] = []
    for item in board.pasteboardItems ?? [] {
        var entry: [String: String] = [:]
        for type in item.types {
            if let data = item.data(forType: type) { entry[type.rawValue] = data.base64EncodedString() }
        }
        saved.append(entry)
    }
    let data = try! JSONSerialization.data(withJSONObject: saved)
    try! data.write(to: URL(fileURLWithPath: argument))
    print("saved \(saved.count) item(s)")
case "clip-restore":
    let data = try! Data(contentsOf: URL(fileURLWithPath: argument))
    let saved = try! JSONSerialization.jsonObject(with: data) as! [[String: String]]
    let board = NSPasteboard.general
    board.clearContents()
    let restored = saved.map { entry -> NSPasteboardItem in
        let item = NSPasteboardItem()
        for (type, b64) in entry {
            if let bytes = Data(base64Encoded: b64) { item.setData(bytes, forType: NSPasteboard.PasteboardType(type)) }
        }
        return item
    }
    board.writeObjects(restored)
    print("restored \(restored.count) item(s)")
case "clip-text":
    print(NSPasteboard.general.string(forType: .string) ?? "")
default:
    print("unknown command \(command)")
    exit(2)
}
