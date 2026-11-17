// SPDX-License-Identifier: GPL-3.0-or-later
// Drives the real keyboard extension in the simulator and saves screenshots.
// Expects the Wispr Bro keyboard to be enabled in the simulator:
//   xcrun simctl spawn <udid> defaults write -g AppleKeyboards -array "en_US@sw=QWERTY;hw=Automatic" "emoji@sw=Emoji" "com.wisprbro.ios.keyboard"
//   xcrun simctl spawn <udid> defaults write com.apple.keyboard.preferences AutomaticMinimizationEnabled -bool NO
// Screenshots go to TEST_RUNNER_WB_SHOT_DIR, named real-<step>-<TEST_RUNNER_WB_SHOT_SUFFIX>.png.

import XCTest

final class WBKeyboardFlowUITests: XCTestCase {
    private var shotDirectory: String { ProcessInfo.processInfo.environment["WB_SHOT_DIR"] ?? NSTemporaryDirectory() }
    private var shotSuffix: String { ProcessInfo.processInfo.environment["WB_SHOT_SUFFIX"] ?? "light" }

    private let app = XCUIApplication(bundleIdentifier: "com.wisprbro.ios")
    private let springboard = XCUIApplication(bundleIdentifier: "com.apple.springboard")
    private var apps: [XCUIApplication] { [app, springboard] }
    /// The process the keyboard's elements show up in, once found.
    private var keyboardHost: XCUIApplication?

    override func setUp() {
        continueAfterFailure = true
        XCUIDevice.shared.orientation = .portrait
    }

    override func tearDown() {
        XCUIDevice.shared.orientation = .portrait
    }

    // MARK: Helpers

    private func shot(_ name: String) {
        let data = XCUIScreen.main.screenshot().pngRepresentation
        let url = URL(fileURLWithPath: shotDirectory).appendingPathComponent("real-\(name)-\(shotSuffix).png")
        try? data.write(to: url)
    }

    /// A screenshot taken while `action` (a long press) is still running.
    private func shot(_ name: String, after delay: TimeInterval, during action: () -> Void) {
        let done = expectation(description: name)
        DispatchQueue.global().asyncAfter(deadline: .now() + delay) {
            self.shot(name)
            done.fulfill()
        }
        action()
        wait(for: [done], timeout: 15)
    }

    private func find(_ label: String, in apps: [XCUIApplication], timeout: TimeInterval = 4) -> XCUIElement? {
        let deadline = Date().addingTimeInterval(timeout)
        repeat {
            for app in apps {
                for query in [app.buttons, app.staticTexts, app.otherElements] {
                    let element = query[label]
                    if element.exists { return element }
                }
            }
            usleep(200_000)
        } while Date() < deadline
        return nil
    }

    /// A typing key of our keyboard, by its name: a character, or shift, delete,
    /// space, return, 123, #+=, ABC.
    private func key(_ name: String, timeout: TimeInterval = 3) -> XCUIElement? {
        let identifier = "wb.key.\(name)"
        let deadline = Date().addingTimeInterval(timeout)
        repeat {
            for host in keyboardHost.map({ [$0] }) ?? apps {
                let element = host.descendants(matching: .any).matching(identifier: identifier).firstMatch
                if element.exists {
                    keyboardHost = host
                    return element
                }
            }
            usleep(150_000)
        } while Date() < deadline
        return nil
    }

    private func tap(_ name: String, file: StaticString = #filePath, line: UInt = #line) {
        guard let element = key(name) else {
            return XCTFail("no key \(name)", file: file, line: line)
        }
        element.tap()
    }

    /// Lowercase letters and spaces, as taps.
    private func type(_ text: String) {
        for character in text {
            tap(character == " " ? "space" : String(character))
        }
    }

    private var fieldElement: XCUIElement { app.descendants(matching: .any)["wb.try.field"] }
    private var text: String { (fieldElement.value as? String) ?? "" }

    /// Launches the harness, focuses the field and brings up our keyboard. On the way a
    /// screenshot of the system keyboard is saved for comparison.
    private func openKeyboard(arguments: [String] = ["--wb-fake-mic"], wantsSystemShot: Bool = false) throws {
        app.launchArguments = arguments
        app.launch()
        XCTAssertTrue(fieldElement.waitForExistence(timeout: 10))
        fieldElement.tap()
        sleep(2)

        // Go round the keyboards until ours is up; the system keyboard's picture is
        // saved on the way, for comparison.
        var sawSystem = false
        for _ in 0..<6 {
            if let intro = find("Continue", in: apps, timeout: 0.5) {
                intro.tap()
                sleep(1)
            }
            let ours = key("q", timeout: 1.5) != nil
            if ours, sawSystem || !wantsSystemShot { break }
            if !ours, !sawSystem, app.keys["Q"].exists || app.keys["q"].exists {
                sawSystem = true
                shot("00-system")
                let systemKey = app.keys["Q"].exists ? app.keys["Q"] : app.keys["q"]
                shot("00-system-popup", after: 0.8, during: { systemKey.press(forDuration: 1.4) })
                app.keys["delete"].tap()
            }
            guard let globe = find("Next keyboard", in: apps, timeout: 2) else { break }
            globe.tap()
            sleep(1)
        }
        _ = try XCTUnwrap(key("q", timeout: 3), "the Wispr Bro keyboard never came up")
    }

    // MARK: Tests

    func testTypingAndDictation() throws {
        try openKeyboard(wantsSystemShot: true)
        shot("01-first-state")

        // Capitals, an accent picked by sliding, punctuation, a number, an accent by holding.
        let h = try XCUnwrap(key("h"))
        shot("02-key-popup", after: 0.9, during: { h.press(forDuration: 1.6) })
        type("ello ")
        tap("shift")
        type("zo")
        let e = try XCUnwrap(key("e"))
        let cell = e.frame.width + 6
        let start = e.coordinate(withNormalizedOffset: CGVector(dx: 0.5, dy: 0.5))
        shot("03-accent-popup", after: 1.3, during: {
            start.press(forDuration: 0.8, thenDragTo: start.withOffset(CGVector(dx: 3 * cell, dy: -50)), withVelocity: 60, thenHoldForDuration: 1.2)
        })
        tap("123")
        shot("04-numbers")
        tap(",")
        tap("space")
        tap("123")
        tap("2")
        tap("space")
        type("caf")
        try XCUnwrap(key("e")).press(forDuration: 0.8)
        type("s")
        tap("123")
        tap("!")
        XCTAssertEqual(text, "Hello Zoë, 2 cafés!")

        // Double space types a period; the next letter is a capital.
        tap("ABC")
        tap("space")
        type("ok")
        try XCUnwrap(key("space")).doubleTap()
        XCTAssertEqual(text, "Hello Zoë, 2 cafés! Ok. ")

        // Caps lock by double tap, off by one tap.
        try XCUnwrap(key("shift")).doubleTap()
        type("wb")
        shot("05-caps-lock")
        tap("shift")
        type("x")
        XCTAssertEqual(text, "Hello Zoë, 2 cafés! Ok. WBx")

        // Pages: 123, #+=, back to 123, back to letters.
        tap("123")
        tap("#+=")
        shot("06-symbols")
        tap("€")
        tap("123")
        XCTAssertNotNil(key("7"), "123 from the symbols page")
        tap("ABC")
        XCTAssertNotNil(key("q"), "letters again")
        XCTAssertEqual(text, "Hello Zoë, 2 cafés! Ok. WBx€")

        // Hold 123, slide to a digit, let go: typed, and letters are back.
        let numbers = try XCUnwrap(key("123"))
        let seven = try XCUnwrap(key("u")).coordinate(withNormalizedOffset: CGVector(dx: 0.5, dy: 0.5))
        numbers.coordinate(withNormalizedOffset: CGVector(dx: 0.5, dy: 0.5))
            .press(forDuration: 0.3, thenDragTo: seven, withVelocity: 300, thenHoldForDuration: 0.2)
        XCTAssertNotNil(key("q"), "letters after the slide")
        XCTAssertEqual(text, "Hello Zoë, 2 cafés! Ok. WBx€7")

        // Delete repeats while held.
        let before = text
        try XCUnwrap(key("delete")).press(forDuration: 1.2)
        let after = text
        XCTAssertTrue(before.hasPrefix(after), "delete only removes from the end: \(after)")
        XCTAssertGreaterThanOrEqual(before.count - after.count, 4, "held delete repeats: \(after)")
        XCTAssertGreaterThan(after.count, 5, "and does not run away: \(after)")

        // Return.
        tap("return")
        XCTAssertEqual(text, after + "\n")
        shot("07-typed")

        // Dictation. No session: the mic opens the app, which starts the session and the take.
        let mic = try XCUnwrap(
            find("Start Wispr Bro", in: apps, timeout: 2) ?? find("Start dictating", in: apps, timeout: 1),
            "no mic in the strip"
        )
        mic.tap()
        sleep(3)
        if let stay = find("Stay in Wispr Bro", in: apps, timeout: 3) {
            stay.tap()
            sleep(1)
        }
        if find("Finish and insert", in: apps, timeout: 2) == nil {
            fieldElement.tap()
            sleep(2)
        }
        shot("08-listening")
        XCTAssertNotNil(key("q", timeout: 2), "the keys stay while listening")
        let finish = try XCUnwrap(find("Finish and insert", in: apps, timeout: 4), "not listening")
        finish.tap()
        usleep(150_000)
        shot("09-transcribing")
        sleep(2)
        shot("10-inserted")
        let stub = "Hello from the stub"
        XCTAssertEqual(text.components(separatedBy: stub).count - 1, 1, "one insertion: \(text)")
        XCTAssertTrue(text.hasPrefix(after + "\n" + stub), "no space at the start of a line: \(text)")

        // Typing right after a dictation: a space and a capital come by themselves.
        let dictated = text
        type("and")
        if dictated.hasSuffix(".") {
            XCTAssertEqual(text, dictated + " And")
        } else {
            XCTAssertEqual(text, dictated + "and")
        }

        // A warm take: no app switch, one space before it, inserted once.
        let again = try XCUnwrap(find("Start dictating", in: apps, timeout: 4), "not idle after the take")
        again.tap()
        sleep(2)
        try XCUnwrap(find("Finish and insert", in: apps, timeout: 3)).tap()
        sleep(2)
        XCTAssertEqual(text.components(separatedBy: stub).count - 1, 2, "exactly two insertions: \(text)")
        XCTAssertTrue(text.contains("nd " + stub), "the second take gets a leading space: \(text)")

        // Cancel discards.
        try XCUnwrap(find("Start dictating", in: apps, timeout: 4)).tap()
        sleep(1)
        try XCUnwrap(find("Cancel", in: apps, timeout: 3)).tap()
        sleep(2)
        XCTAssertEqual(text.components(separatedBy: stub).count - 1, 2)

        // And the keys still type after all that.
        let end = text
        tap("123")
        tap("?")
        XCTAssertEqual(text, end + "?")
        shot("11-idle")
    }

    func testLandscape() throws {
        try openKeyboard()
        XCUIDevice.shared.orientation = .landscapeLeft
        sleep(2)
        if key("q", timeout: 2) == nil {
            fieldElement.tap()
            sleep(1)
        }
        type("wide ")
        tap("123")
        tap("4")
        shot("20-landscape-numbers")
        tap("ABC")
        try XCUnwrap(key("u")).press(forDuration: 0.8)
        shot("21-landscape")
        XCTAssertEqual(text, "Wide 4ü")
        XCUIDevice.shared.orientation = .portrait
        sleep(2)
        XCTAssertNotNil(key("q", timeout: 3), "back in portrait")
        shot("22-portrait-again")
    }

    private func XCUnwrap<T>(_ value: T?, _ message: String = "", file: StaticString = #filePath, line: UInt = #line) throws -> T {
        try XCTUnwrap(value, message, file: file, line: line)
    }
}
