// SPDX-License-Identifier: GPL-3.0-or-later
//
// Mac test S-17: snapshots the general pasteboard (every item, every type)
// to a file before the run and puts it back after, so a test run never
// leaves the owner's clipboard changed (the app restores it after each
// paste too; this covers the run as a whole).
//
//   swift scripts/wb-s17/clip.swift save <file>
//   swift scripts/wb-s17/clip.swift restore <file>
//   swift scripts/wb-s17/clip.swift digest      # one line: items, types, sha256
import AppKit
import CryptoKit
import Foundation

let args = CommandLine.arguments
guard args.count >= 2 else {
    FileHandle.standardError.write("usage: clip.swift save|restore <file> | digest\n".data(using: .utf8)!)
    exit(2)
}
let pb = NSPasteboard.general

func snapshot() -> [[String: Data]] {
    (pb.pasteboardItems ?? []).map { item in
        var types: [String: Data] = [:]
        for t in item.types {
            if let d = item.data(forType: t) { types[t.rawValue] = d }
        }
        return types
    }
}

func digest(_ items: [[String: Data]]) -> String {
    var h = SHA256()
    for item in items {
        for key in item.keys.sorted() {
            h.update(data: key.data(using: .utf8)!)
            h.update(data: item[key]!)
        }
    }
    let types = items.reduce(0) { $0 + $1.count }
    return "items=\(items.count) types=\(types) sha256=" + h.finalize().map { String(format: "%02x", $0) }.joined()
}

switch args[1] {
case "save":
    let items = snapshot()
    let plist = items.map { $0.mapValues { $0 as NSData } as NSDictionary } as NSArray
    let data = try PropertyListSerialization.data(fromPropertyList: plist, format: .binary, options: 0)
    try data.write(to: URL(fileURLWithPath: args[2]), options: .atomic)
    print("saved", digest(items))
case "restore":
    let data = try Data(contentsOf: URL(fileURLWithPath: args[2]))
    let plist = try PropertyListSerialization.propertyList(from: data, format: nil) as? [[String: Data]] ?? []
    pb.clearContents()
    let items: [NSPasteboardItem] = plist.map { types in
        let item = NSPasteboardItem()
        for (t, d) in types { item.setData(d, forType: NSPasteboard.PasteboardType(t)) }
        return item
    }
    if !items.isEmpty { pb.writeObjects(items) }
    print("restored", digest(snapshot()))
case "digest":
    print(digest(snapshot()))
default:
    exit(2)
}
