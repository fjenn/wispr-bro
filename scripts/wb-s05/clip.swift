// SPDX-License-Identifier: GPL-3.0-or-later
//
// S-05 real-Mac helper: saves and restores the whole general pasteboard
// (every item and type, byte for byte) around the card's Copy checks, so
// the owner's clipboard is put back exactly. Never prints clipboard
// contents except with `ours <text>`, which only says whether the
// clipboard holds exactly that text (the run's own transcript).
//
//   swift scripts/wb-s05/clip.swift save <dir>      every item/type to <dir>
//   swift scripts/wb-s05/clip.swift restore <dir>   put <dir> back
//   swift scripts/wb-s05/clip.swift digest          sha-like digest + count
//   swift scripts/wb-s05/clip.swift ours <file>     JSON: text equals the
//                                                   file's text? markers?

import AppKit
import CryptoKit
import Foundation

let args = CommandLine.arguments
let pb = NSPasteboard.general
let fm = FileManager.default

func out(_ d: [String: Any]) {
    if let data = try? JSONSerialization.data(withJSONObject: d, options: [.sortedKeys]),
       let s = String(data: data, encoding: .utf8) {
        print(s)
    }
}

func hexName(_ s: String) -> String { s.utf8.map { String(format: "%02x", $0) }.joined() }
func fromHex(_ h: String) -> String {
    var bytes = [UInt8]()
    var i = h.startIndex
    while i < h.endIndex {
        let j = h.index(i, offsetBy: 2)
        bytes.append(UInt8(h[i..<j], radix: 16) ?? 0)
        i = j
    }
    return String(decoding: bytes, as: UTF8.self)
}

func digest() -> (String, Int) {
    var h = SHA256()
    var n = 0
    for (i, item) in (pb.pasteboardItems ?? []).enumerated() {
        for t in item.types {
            h.update(data: Data("\(i):\(t.rawValue)".utf8))
            if let d = item.data(forType: t) { h.update(data: d) }
            n += 1
        }
    }
    return (h.finalize().map { String(format: "%02x", $0) }.joined(), n)
}

switch args.count > 1 ? args[1] : "" {
case "save":
    let dir = URL(fileURLWithPath: args[2])
    try? fm.removeItem(at: dir)
    try fm.createDirectory(at: dir, withIntermediateDirectories: true)
    for (i, item) in (pb.pasteboardItems ?? []).enumerated() {
        let idir = dir.appendingPathComponent(String(format: "%03d", i))
        try fm.createDirectory(at: idir, withIntermediateDirectories: true)
        for (k, t) in item.types.enumerated() {
            let name = String(format: "%03d-", k) + hexName(t.rawValue)
            try (item.data(forType: t) ?? Data()).write(to: idir.appendingPathComponent(name))
        }
    }
    let (d, n) = digest()
    out(["saved": n, "digest": d])
case "restore":
    let dir = URL(fileURLWithPath: args[2])
    var items: [NSPasteboardItem] = []
    for idir in try fm.contentsOfDirectory(atPath: dir.path).sorted() {
        let item = NSPasteboardItem()
        let p = dir.appendingPathComponent(idir)
        for f in try fm.contentsOfDirectory(atPath: p.path).sorted() {
            let type = fromHex(String(f.dropFirst(4)))
            let data = try Data(contentsOf: p.appendingPathComponent(f))
            item.setData(data, forType: NSPasteboard.PasteboardType(type))
        }
        items.append(item)
    }
    pb.clearContents()
    if !items.isEmpty { pb.writeObjects(items) }
    let (d, n) = digest()
    out(["restored": n, "digest": d])
case "digest":
    let (d, n) = digest()
    out(["types": n, "digest": d])
case "ours":
    let want = (try? String(contentsOfFile: args[2], encoding: .utf8)) ?? ""
    let item = pb.pasteboardItems?.first
    let types = item?.types.map(\.rawValue) ?? []
    let text = item?.string(forType: .string) ?? ""
    out([
        "items": pb.pasteboardItems?.count ?? 0,
        "equal": text.trimmingCharacters(in: .whitespacesAndNewlines)
            == want.trimmingCharacters(in: .whitespacesAndNewlines),
        "length": text.count,
        "concealed": types.contains("org.nspasteboard.ConcealedType"),
        "transient": types.contains("org.nspasteboard.TransientType"),
    ])
default:
    out(["error": "usage: clip.swift save|restore <dir> | digest | ours <file>"])
    exit(2)
}
