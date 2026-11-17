// SPDX-License-Identifier: GPL-3.0-or-later
import XCTest

/// Runs a small script against the simulator (WB_SCRIPT = a file path), one
/// command a line, for walking screens and taking screenshots without a
/// rebuild:
///   launch | launchhost | terminate | activate | activatehost | home
///   tap <label or identifier>        (fails the test when missing)
///   try <label or identifier>        (tap when present)
///   field | fieldvalue <name>       (tap / save the text of the first text area)
///   tapat <x> <y>                    (normalized 0…1, in the Wispr Bro app)
///   wait <label>                     (up to 20 s; fails when missing)
///   gone <label>                     (up to 20 s; fails when still there)
///   type <text> | key <return|delete>
///   keyboard                         (switch to the Wispr Bro keyboard)
///   swipe <up|down|left|right> | scroll <label>
///   wav <name> | sleep <seconds> | shot <name> | dump <name> | value <label> <name>
///   expect <label> <substring>       (the element's value contains it)
final class WBScriptTests: WBE2ECase {
    private func front() -> XCUIApplication {
        if host.state == .runningForeground { return host }
        if wispr.state == .runningForeground { return wispr }
        if settings.state == .runningForeground { return settings }
        return springboard
    }

    func testScript() throws {
        let path = try XCTUnwrap(env["WB_SCRIPT"], "set WB_SCRIPT")
        let script = try String(contentsOfFile: path, encoding: .utf8)
        for raw in script.components(separatedBy: .newlines) {
            let line = raw.trimmingCharacters(in: .whitespaces)
            if line.isEmpty || line.hasPrefix("#") { continue }
            let parts = line.split(separator: " ", maxSplits: 1).map(String.init)
            let arg = parts.count > 1 ? parts[1] : ""
            switch parts[0] {
            case "launch": launchWispr()
            case "launchhost": host.launch()
            case "launchsettings": settings.terminate(); settings.launch()
            case "terminate": wispr.terminate()
            case "activate": wispr.activate()
            case "activatehost": host.activate()
            case "home": XCUIDevice.shared.press(.home)
            case "tap":
                guard let element = find(arg, timeout: 8) else { XCTFail("tap: not on screen: \(arg)"); shot("fail"); continue }
                element.tap()
            case "try": find(arg, timeout: 2)?.tap()
            case "field":
                // The first text area on screen (a label can also match its heading).
                let area = front().textViews.firstMatch
                XCTAssertTrue(area.waitForExistence(timeout: 6), "no text area on screen")
                area.tap()
            case "fieldvalue": note(arg, (front().textViews.firstMatch.value as? String) ?? "<none>")
            case "hold": find(arg, timeout: 4)?.press(forDuration: 1.2)
            case "tapat":
                let xy = arg.split(separator: " ").compactMap { Double($0) }
                front().coordinate(withNormalizedOffset: CGVector(dx: xy[0], dy: xy[1])).tap()
            case "wait": XCTAssertNotNil(find(arg, timeout: 20), "wait: never appeared: \(arg)")
            case "gone":
                let deadline = Date().addingTimeInterval(20)
                while find(arg, timeout: 0.3) != nil, Date() < deadline { usleep(300_000) }
                XCTAssertNil(find(arg, timeout: 0.3), "gone: still on screen: \(arg)")
            case "type": front().typeText(arg)
            case "key": front().typeText(arg == "return" ? "\n" : XCUIKeyboardKey.delete.rawValue)
            case "keyboard": XCTAssertTrue(switchToWisprKeyboard(front()), "the Wispr Bro keyboard never came up")
            case "swipe":
                let app = front()
                switch arg {
                case "up": app.swipeUp()
                case "down": app.swipeDown()
                case "left": app.swipeLeft()
                default: app.swipeRight()
                }
            case "wav": useWav(arg)
            case "sleep": usleep(UInt32((Double(arg) ?? 1) * 1_000_000))
            case "shot": shot(arg)
            case "dump": note(arg, front().debugDescription)
            case "value":
                let two = arg.split(separator: " ", maxSplits: 1).map(String.init)
                note(two[1], (find(two[0], timeout: 4)?.value as? String) ?? "<none>")
            case "expect":
                let two = arg.split(separator: " ", maxSplits: 1).map(String.init)
                let value = (find(two[0], timeout: 4)?.value as? String) ?? ""
                XCTAssertTrue(value.contains(two[1]), "expect \(two[0]) to contain \(two[1]): \(value)")
            default: XCTFail("unknown command: \(line)")
            }
        }
    }
}
