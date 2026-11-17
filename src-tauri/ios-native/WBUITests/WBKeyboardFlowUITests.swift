// SPDX-License-Identifier: GPL-3.0-or-later
// Drives the real keyboard extension in the simulator and saves screenshots.
// Expects the Wispr Bro keyboard to be enabled in the simulator:
//   xcrun simctl spawn <udid> defaults write -g AppleKeyboards -array "en_US@sw=QWERTY;hw=Automatic" "emoji@sw=Emoji" "com.wisprbro.ios.keyboard"
//   xcrun simctl spawn <udid> defaults write com.apple.keyboard.preferences AutomaticMinimizationEnabled -bool NO

import XCTest

final class WBKeyboardFlowUITests: XCTestCase {
    private var shotDirectory: String { ProcessInfo.processInfo.environment["WB_SHOT_DIR"] ?? NSTemporaryDirectory() }
    private var shotSuffix: String { ProcessInfo.processInfo.environment["WB_SHOT_SUFFIX"] ?? "light" }

    private func shot(_ name: String) {
        let data = XCUIScreen.main.screenshot().pngRepresentation
        let url = URL(fileURLWithPath: shotDirectory).appendingPathComponent("real-\(name)-\(shotSuffix).png")
        try? data.write(to: url)
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

    func testKeyboardFlow() throws {
        continueAfterFailure = true
        let app = XCUIApplication(bundleIdentifier: "com.wisprbro.ios")
        let springboard = XCUIApplication(bundleIdentifier: "com.apple.springboard")
        let apps = [app, springboard]
        app.launchArguments = ["--wb-fake-mic"]
        app.launch()

        let field = app.descendants(matching: .any)["wb.try.field"]
        XCTAssertTrue(field.waitForExistence(timeout: 10))
        field.tap()
        sleep(2)

        // Cycle keyboards until ours is up.
        var ours = find("Start Wispr Bro", in: apps, timeout: 1) ?? find("Start dictating", in: apps, timeout: 1)
        var tries = 0
        while ours == nil, tries < 4 {
            tries += 1
            guard let globe = find("Next keyboard", in: apps, timeout: 2) else { break }
            globe.tap()
            sleep(1)
            ours = find("Start Wispr Bro", in: apps, timeout: 2) ?? find("Start dictating", in: apps, timeout: 1)
        }
        shot("01-first-state")
        let mic = try XCTUnwrap(ours, "the Wispr Bro keyboard never came up")

        // No session: the mic opens the app, which starts the session and the take.
        mic.tap()
        sleep(3)
        shot("02-after-start")
        if let stay = find("Stay in Wispr Bro", in: apps, timeout: 3) {
            stay.tap()
            sleep(1)
        }
        if find("Finish and insert", in: apps, timeout: 2) == nil {
            field.tap()
            sleep(2)
        }
        shot("03-listening")
        let finish = try XCTUnwrap(find("Finish and insert", in: apps, timeout: 4), "not listening")
        finish.tap()
        usleep(150_000)
        shot("04-transcribing")
        sleep(2)
        shot("05-inserted")
        XCTAssertTrue(((field.value as? String) ?? "").contains("Hello from the stub"), "text was not inserted: \(String(describing: field.value))")

        // A warm take: no app switch.
        let again = try XCTUnwrap(find("Start dictating", in: apps, timeout: 4), "not idle after the take")
        again.tap()
        sleep(2)
        shot("06-warm-listening")
        try XCTUnwrap(find("Finish and insert", in: apps, timeout: 3)).tap()
        sleep(2)
        let value = (field.value as? String) ?? ""
        XCTAssertEqual(value.components(separatedBy: "Hello from the stub").count - 1, 2, "expected exactly two insertions: \(value)")
        XCTAssertTrue(value.contains(". Hello"), "second take should get a leading space: \(value)")

        // Cancel discards.
        try XCTUnwrap(find("Start dictating", in: apps, timeout: 4)).tap()
        sleep(1)
        try XCTUnwrap(find("Cancel", in: apps, timeout: 3)).tap()
        sleep(2)
        XCTAssertEqual(((field.value as? String) ?? "").components(separatedBy: "Hello from the stub").count - 1, 2)
        shot("07-idle")
    }
}
