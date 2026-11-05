// SPDX-License-Identifier: GPL-3.0-or-later
//
// RS-14 (scripted part): does the Carbon hotkey path S-02 uses for its
// Secure Input shadows (Handy's `secure_input.rs` registers through
// tauri-plugin-global-shortcut → global-hotkey → `RegisterEventHotKey`)
// still fire while another process holds Secure Event Input, when an event
// tap no longer sees key-downs? And is an unshadowed Esc left alone (an
// idle Esc in a password field must never be swallowed)?
//
// One process plays everything, with its own window only:
//   - a child copy of this script holds Secure Input (`EnableSecureEventInput`);
//   - this process owns a small window (counts key-downs it receives), a
//     listen-only session tap (counts key-downs and fn flag changes) and a
//     Carbon hotkey (counts presses), and posts synthetic events at the HID
//     level (CGEventPost) — the same entry point as a hardware keyboard.
//
// Build, then run under the GUI lock (it activates its own window for a few
// seconds; the holder is the same binary started with `hold`):
//   swiftc -O scripts/wb-s02/rs14-carbon.swift -o "$TMPDIR/wb-rs14-carbon"
//   /usr/bin/lockf -k ~/wb-bench/gui.lock "$TMPDIR/wb-rs14-carbon" [trials]
// The posting process needs Accessibility (or Input Monitoring) for the tap.
//
// Prints one line per phase and a RESULT line. Nothing else on the Mac is
// touched; Secure Input is released when the child exits (≤ 40 s).

import AppKit
import Carbon
import Foundation

let args = CommandLine.arguments
if args.count >= 3, args[1] == "hold" {
  // Child: hold Secure Input for N seconds.
  _ = NSApplication.shared
  EnableSecureEventInput()
  let seconds = Double(args[2]) ?? 30
  let deadline = Date().addingTimeInterval(seconds)
  while Date() < deadline {
    RunLoop.current.run(until: Date().addingTimeInterval(0.2))
    if getppid() == 1 { break }  // parent gone
  }
  DisableSecureEventInput()
  exit(0)
}

let trials = args.count >= 2 ? Int(args[1]) ?? 10 : 10
let kVKEscape: CGKeyCode = 53
let kVKV: CGKeyCode = 9

// ------------------------------------------------------------ counters
final class Counts {
  var tapEsc = 0, tapV = 0, tapFn = 0, windowEsc = 0, carbonEsc = 0, carbonV = 0
  func reset() { tapEsc = 0; tapV = 0; tapFn = 0; windowEsc = 0; carbonEsc = 0; carbonV = 0 }
}
let counts = Counts()

// ------------------------------------------------------------ window
final class KeyView: NSView {
  override var acceptsFirstResponder: Bool { true }
  override func keyDown(with event: NSEvent) {
    if event.keyCode == kVKEscape { counts.windowEsc += 1 }
  }
}
let app = NSApplication.shared
app.setActivationPolicy(.regular)
app.finishLaunching()
let window = NSWindow(
  contentRect: NSRect(x: 200, y: 200, width: 240, height: 80),
  styleMask: [.titled], backing: .buffered, defer: false)
window.title = "RS-14 probe"
let view = KeyView(frame: window.contentView!.bounds)
window.contentView = view
window.makeKeyAndOrderFront(nil)
window.makeFirstResponder(view)
app.activate(ignoringOtherApps: true)

// Pump AppKit events (not just the run loop): Carbon hot-key events and the
// window's key events are only dispatched through NSApplication.
func spin(_ seconds: Double) {
  let until = Date().addingTimeInterval(seconds)
  while Date() < until {
    if let e = app.nextEvent(matching: .any, until: until, inMode: .default, dequeue: true) {
      app.sendEvent(e)
    }
  }
}

// ------------------------------------------------------------ tap
let mask: CGEventMask = (1 << CGEventType.keyDown.rawValue) | (1 << CGEventType.flagsChanged.rawValue)
guard
  let tap = CGEvent.tapCreate(
    tap: .cgSessionEventTap, place: .headInsertEventTap, options: .listenOnly,
    eventsOfInterest: mask,
    callback: { _, type, event, _ in
      if type == .keyDown {
        let code = CGKeyCode(event.getIntegerValueField(.keyboardEventKeycode))
        if code == kVKEscape { counts.tapEsc += 1 }
        if code == kVKV { counts.tapV += 1 }
      } else if type == .flagsChanged, event.flags.contains(.maskSecondaryFn) {
        counts.tapFn += 1
      }
      return Unmanaged.passUnretained(event)
    }, userInfo: nil)
else {
  print("FAIL: cannot create the listen tap (grant Accessibility/Input Monitoring to the shell)")
  exit(2)
}
let source = CFMachPortCreateRunLoopSource(nil, tap, 0)
CFRunLoopAddSource(CFRunLoopGetMain(), source, .commonModes)
CGEvent.tapEnable(tap: tap, enable: true)

// ------------------------------------------------------------ Carbon
var handlerRef: EventHandlerRef?
var spec = EventTypeSpec(eventClass: OSType(kEventClassKeyboard), eventKind: UInt32(kEventHotKeyPressed))
InstallEventHandler(
  GetApplicationEventTarget(),
  { _, event, _ in
    var id = EventHotKeyID()
    GetEventParameter(
      event, EventParamName(kEventParamDirectObject), EventParamType(typeEventHotKeyID), nil,
      MemoryLayout<EventHotKeyID>.size, nil, &id)
    if id.id == 1 { counts.carbonEsc += 1 }
    if id.id == 2 { counts.carbonV += 1 }
    return noErr
  }, 1, &spec, nil, &handlerRef)

var escRef: EventHotKeyRef?
var vRef: EventHotKeyRef?
func registerEsc() {
  RegisterEventHotKey(UInt32(kVKEscape), 0, EventHotKeyID(signature: 0x5742_5232, id: 1),
    GetApplicationEventTarget(), 0, &escRef)
}
func unregisterEsc() {
  if let r = escRef { UnregisterEventHotKey(r); escRef = nil }
}
func registerV() {
  RegisterEventHotKey(UInt32(kVKV), UInt32(controlKey | cmdKey),
    EventHotKeyID(signature: 0x5742_5232, id: 2), GetApplicationEventTarget(), 0, &vRef)
}
func unregisterV() {
  if let r = vRef { UnregisterEventHotKey(r); vRef = nil }
}

// ------------------------------------------------------------ posting
let src = CGEventSource(stateID: .hidSystemState)
func postKey(_ code: CGKeyCode, flags: CGEventFlags = []) {
  for down in [true, false] {
    let e = CGEvent(keyboardEventSource: src, virtualKey: code, keyDown: down)!
    e.flags = flags
    e.post(tap: .cghidEventTap)
    spin(0.03)
  }
  spin(0.12)
}
func postFn() {
  // fn down then up as flagsChanged (keycode 63).
  for flags: CGEventFlags in [.maskSecondaryFn, []] {
    let e = CGEvent(keyboardEventSource: src, virtualKey: 63, keyDown: !flags.isEmpty)!
    e.type = .flagsChanged
    e.flags = flags
    e.post(tap: .cghidEventTap)
    spin(0.05)
  }
  spin(0.1)
}

func secure() -> Bool { IsSecureEventInputEnabled() }

var results: [String] = []
func phase(_ name: String, _ body: () -> Void) {
  counts.reset()
  app.activate(ignoringOtherApps: true)
  window.makeKeyAndOrderFront(nil)
  spin(0.4)
  body()
  spin(0.3)
  let line =
    "\(name): secure=\(secure()) tapEsc=\(counts.tapEsc) windowEsc=\(counts.windowEsc) "
    + "carbonEsc=\(counts.carbonEsc) tapV=\(counts.tapV) carbonV=\(counts.carbonV) tapFn=\(counts.tapFn)"
  print(line)
  results.append(line)
}

spin(0.5)
if secure() {
  print("FAIL: Secure Input is already held by another process; run again later")
  exit(3)
}

// A: no Secure Input, Esc shadow registered (control).
registerEsc()
phase("A no-secure, Esc registered (\(trials)×Esc)") { for _ in 0..<trials { postKey(kVKEscape) } }
unregisterEsc()
// A2: no Secure Input, nothing registered: the window gets Esc.
phase("A2 no-secure, idle (\(trials)×Esc)") { for _ in 0..<trials { postKey(kVKEscape) } }

// Start the holder.
let holder = Process()
holder.executableURL = URL(fileURLWithPath: Bundle.main.executablePath!)
holder.arguments = ["hold", "40"]
try holder.run()
var waited = 0.0
while !secure() && waited < 30 { spin(0.2); waited += 0.2 }
if !secure() {
  print("FAIL: the holder did not enable Secure Input")
  holder.terminate()
  exit(4)
}
print("holder pid \(holder.processIdentifier) holds Secure Input after \(waited) s")

// B: Secure Input held, Esc shadow registered (the S-02 path while a take runs).
registerEsc()
phase("B secure, Esc registered (\(trials)×Esc)") { for _ in 0..<trials { postKey(kVKEscape) } }
unregisterEsc()
// C: Secure Input held, Esc not registered (idle): Esc must reach the window.
phase("C secure, idle (\(trials)×Esc)") { for _ in 0..<trials { postKey(kVKEscape) } }
// D: Secure Input held, ⌃⌘V shadow registered.
registerV()
phase("D secure, ⌃⌘V registered (\(trials)×⌃⌘V)") {
  for _ in 0..<trials { postKey(kVKV, flags: [.maskControl, .maskCommand]) }
}
unregisterV()
// E: Secure Input held: fn flag changes still reach the tap.
phase("E secure, fn (\(trials)×fn)") { for _ in 0..<trials { postFn() } }

holder.terminate()
holder.waitUntilExit()
spin(1.5)
print("after the holder: secure=\(secure())")
print("RESULT: " + results.joined(separator: " | "))
exit(0)
