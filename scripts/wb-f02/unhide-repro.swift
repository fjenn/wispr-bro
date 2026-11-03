// SPDX-License-Identifier: GPL-3.0-or-later
// Mac wave 4, S-28.8: what unhides a hidden app when the bar builds its web
// view? Off screen (all windows at -20000), no take, no Wispr Bro.
//
//   xcrun swiftc -O scripts/wb-f02/unhide-repro.swift -o /tmp/wbunhide
//   for m in none panel-only panel-canhide activate ignoring webview webview-guard swizzled rehide; do
//     echo "== $m"; perl -e 'alarm 10; exec @ARGV' /tmp/wbunhide $m; done
//
// A regular app shows a "Hub" window, hides itself (NSApp.hide), then runs
// one mode and prints NSApp.isHidden 1 s later. Results on macOS 26.6
// (M5 Max, 2026-11-03): `none`, `panel-only` (canHide off) and
// `webview-guard` (canHide off + WKWebView, no activate) stay hidden;
// `panel-canhide` (canHide on, orderFrontRegardless), `activate` (what wry
// 0.55 calls after every web view build), `ignoring` and `webview` (the
// whole wry sequence) unhide the app ~0.5 s later and the Hub comes back;
// `swizzled` (activate swapped for a no-op around the build, as
// `fork/pill/activation.rs` does) stays hidden.
import AppKit
import WebKit
let mode = CommandLine.arguments.count > 1 ? CommandLine.arguments[1] : "activate"
let app = NSApplication.shared
app.setActivationPolicy(.regular)
class D: NSObject, NSApplicationDelegate {
  var hub: NSWindow!
  var keep: [Any] = []
  func applicationDidFinishLaunching(_ n: Notification) {
    hub = NSWindow(contentRect: NSRect(x: -20000, y: -20000, width: 200, height: 100), styleMask: [.titled], backing: .buffered, defer: false)
    hub.orderFront(nil)
    for name in [NSApplication.didUnhideNotification, NSApplication.didHideNotification, NSApplication.willUnhideNotification] {
      NotificationCenter.default.addObserver(forName: name, object: nil, queue: nil) { n in print("  note \(n.name.rawValue) t=\(Date().timeIntervalSince1970)") }
    }
    DispatchQueue.main.asyncAfter(deadline: .now() + 0.5) {
      NSApp.hide(nil)
      DispatchQueue.main.asyncAfter(deadline: .now() + 0.5) {
        print("before: hidden=\(NSApp.isHidden) active=\(NSApp.isActive)")
        switch mode {
        case "activate": NSApp.activate()
        case "ignoring": NSApp.activate(ignoringOtherApps: true)
        case "webview", "webview-guard":
          let p = NSPanel(contentRect: NSRect(x: -20000, y: -21000, width: 100, height: 50), styleMask: [.borderless, .nonactivatingPanel], backing: .buffered, defer: false)
          p.canHide = false
          let wv = WKWebView(frame: p.contentView!.bounds)
          p.contentView = wv
          p.makeFirstResponder(wv)
          self.keep += [p, wv]
          if mode == "webview" { NSApp.activate() }  // what wry does
          p.orderFrontRegardless()
        case "swizzled":
          let m = class_getInstanceMethod(NSApplication.self, #selector(NSApplication.activate as (NSApplication) -> () -> Void))!
          let block: @convention(block) (AnyObject) -> Void = { _ in }
          let old = method_setImplementation(m, imp_implementationWithBlock(block))
          let p = NSPanel(contentRect: NSRect(x: -20000, y: -21000, width: 100, height: 50), styleMask: [.borderless, .nonactivatingPanel], backing: .buffered, defer: false)
          p.canHide = false
          let wv = WKWebView(frame: p.contentView!.bounds)
          p.contentView = wv
          self.keep += [p, wv]
          NSApp.activate()  // wry's call, swallowed
          method_setImplementation(m, old)
          p.orderFrontRegardless()
        case "rehide":
          NSApp.activate(); NSApp.hide(nil)
        case "rehide-later":
          NSApp.activate(); DispatchQueue.main.async { NSApp.hide(nil) }
        case "panel-canhide":
          let p = NSPanel(contentRect: NSRect(x: -20000, y: -21000, width: 100, height: 50), styleMask: [.borderless, .nonactivatingPanel], backing: .buffered, defer: false)
          self.keep.append(p)
          p.orderFrontRegardless()
          print("  panel visible=\(p.isVisible)")
        case "panel-only":
          let p = NSPanel(contentRect: NSRect(x: -20000, y: -21000, width: 100, height: 50), styleMask: [.borderless, .nonactivatingPanel], backing: .buffered, defer: false)
          p.canHide = false
          self.keep.append(p)
          p.orderFrontRegardless()
        default: break
        }
        print("right after: hidden=\(NSApp.isHidden)")
        DispatchQueue.main.asyncAfter(deadline: .now() + 1.0) {
          print("after 1 s: hidden=\(NSApp.isHidden) active=\(NSApp.isActive) hubVisible=\(self.hub.isVisible)")
          exit(0)
        }
      }
    }
  }
}
let d = D(); app.delegate = d; app.run()
