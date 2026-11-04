// SPDX-License-Identifier: GPL-3.0-or-later
//
// Mac test S-36 (OT2-C): reads back a Cursor chat take through
// Accessibility, without the clipboard. Prints the focused chat input's
// text and the file tags it holds (Cursor 3.28 draws a tag as the file name
// followed by a "Remove mention" button). Read-only: it never types or
// focuses anything. Build: swiftc -O -o $WB_S36_BIN/chat-tags chat-tags.swift
// Usage: chat-tags <pid of the throwaway Cursor>
import ApplicationServices
import Foundation

guard CommandLine.arguments.count == 2, let pid = Int32(CommandLine.arguments[1]) else {
  FileHandle.standardError.write("usage: chat-tags <pid>\n".data(using: .utf8)!)
  exit(2)
}
let app = AXUIElementCreateApplication(pid)
AXUIElementSetMessagingTimeout(app, 1.0)
func attr(_ e: AXUIElement, _ name: String) -> AnyObject? {
  var value: AnyObject?
  return AXUIElementCopyAttributeValue(e, name as CFString, &value) == .success ? value : nil
}
func text(_ e: AXUIElement, _ name: String) -> String { (attr(e, name) as? String) ?? "" }
guard let focused = attr(app, "AXFocusedUIElement") else {
  print("focused: none")
  exit(1)
}
let field = focused as! AXUIElement
print("focused: \(text(field, "AXRole")) \(text(field, "AXDescription"))")
print("value: \(text(field, "AXValue").debugDescription)")
var tags: [String] = []
var last = ""
var visited = 0
func walk(_ e: AXUIElement) {
  visited += 1
  if visited > 400 { return }
  let role = text(e, "AXRole")
  if role == "AXStaticText" {
    let v = text(e, "AXValue").trimmingCharacters(in: .whitespaces)
    if !v.isEmpty { last = v }
  }
  if role == "AXButton" && text(e, "AXDescription") == "Remove mention" { tags.append(last) }
  for child in (attr(e, "AXChildren") as? [AXUIElement]) ?? [] { walk(child) }
}
walk(field)
print("tags: \(tags.count) \(tags)")
