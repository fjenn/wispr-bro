// SPDX-License-Identifier: GPL-3.0-or-later
// Prints "<window id> <x> <y> <w> <h>" of the largest on-screen window owned by a PID.
import CoreGraphics
import Foundation

let pid = Int32(CommandLine.arguments[1])!
let list = CGWindowListCopyWindowInfo([.optionOnScreenOnly], kCGNullWindowID) as? [[String: Any]] ?? []
var best: (Int, CGRect)? = nil
for w in list {
    guard (w[kCGWindowOwnerPID as String] as? Int32) == pid,
          (w[kCGWindowLayer as String] as? Int) == 0,
          let id = w[kCGWindowNumber as String] as? Int,
          let b = w[kCGWindowBounds as String] as? [String: CGFloat] else { continue }
    let r = CGRect(x: b["X"]!, y: b["Y"]!, width: b["Width"]!, height: b["Height"]!)
    if best == nil || r.width * r.height > best!.1.width * best!.1.height { best = (id, r) }
}
if let (id, r) = best {
    print("\(id) \(Int(r.minX)) \(Int(r.minY)) \(Int(r.width)) \(Int(r.height))")
} else {
    exit(1)
}
