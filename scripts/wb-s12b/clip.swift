// SPDX-License-Identifier: GPL-3.0-or-later
//
// Mac test S-12b: saves the general pasteboard (every item, every type) to a
// file and puts it back afterwards, so the takes never leave the owner's
// clipboard changed. Also prints a digest to compare before and after.
//
// usage: swift scripts/wb-s12b/clip.swift save|restore|digest <file>
import AppKit
import CryptoKit

let args = CommandLine.arguments
guard args.count == 3 else {
    FileHandle.standardError.write("usage: clip.swift save|restore|digest <file>\n".data(using: .utf8)!)
    exit(2)
}
let pb = NSPasteboard.general
let path = URL(fileURLWithPath: args[2])

func snapshot() -> [[String: Data]] {
    (pb.pasteboardItems ?? []).map { item in
        var d: [String: Data] = [:]
        for t in item.types {
            if let data = item.data(forType: t) { d[t.rawValue] = data }
        }
        return d
    }
}

func digest(_ items: [[String: Data]]) -> String {
    var h = SHA256()
    for item in items {
        for k in item.keys.sorted() {
            h.update(data: k.data(using: .utf8)!)
            h.update(data: item[k]!)
        }
    }
    return h.finalize().map { String(format: "%02x", $0) }.joined().prefix(16) + " (\(items.count) item(s))"
}

switch args[1] {
case "save":
    let items = snapshot()
    let data = try PropertyListSerialization.data(fromPropertyList: items, format: .binary, options: 0)
    try data.write(to: path)
    print("saved", digest(items))
case "restore":
    let data = try Data(contentsOf: path)
    let items = try PropertyListSerialization.propertyList(from: data, format: nil) as! [[String: Data]]
    pb.clearContents()
    let objs: [NSPasteboardItem] = items.map { d in
        let it = NSPasteboardItem()
        for (k, v) in d { it.setData(v, forType: NSPasteboard.PasteboardType(k)) }
        return it
    }
    if !objs.isEmpty { pb.writeObjects(objs) }
    print("restored", digest(snapshot()))
case "digest":
    print(digest(snapshot()))
default:
    exit(2)
}
