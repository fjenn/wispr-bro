// SPDX-License-Identifier: GPL-3.0-or-later
// Shared helpers for the end-to-end checks.

import XCTest

class WBE2ECase: XCTestCase {
    let wispr = XCUIApplication(bundleIdentifier: "com.wisprbro.ios")
    let host = XCUIApplication(bundleIdentifier: "com.wisprbro.e2e.host")
    let springboard = XCUIApplication(bundleIdentifier: "com.apple.springboard")
    let settings = XCUIApplication(bundleIdentifier: "com.apple.Preferences")

    var env: [String: String] { ProcessInfo.processInfo.environment }
    var shotDirectory: String { env["WB_SHOT_DIR"] ?? NSTemporaryDirectory() }
    var shotSuffix: String { env["WB_SHOT_SUFFIX"] ?? "light" }
    /// Folder with en.wav / nl.wav / … (16 kHz mono 16-bit).
    var wavDirectory: String { env["WB_WAV_DIR"] ?? "" }
    /// The file the app's fake microphone plays; swapped before a take.
    var fakeWav: String { env["WB_FAKE_WAV"] ?? (NSTemporaryDirectory() + "wb-fake-mic.wav") }

    override func setUp() {
        continueAfterFailure = true
    }

    func shot(_ name: String) {
        let data = XCUIScreen.main.screenshot().pngRepresentation
        let url = URL(fileURLWithPath: shotDirectory).appendingPathComponent("\(name)-\(shotSuffix).png")
        try? FileManager.default.createDirectory(atPath: shotDirectory, withIntermediateDirectories: true)
        try? data.write(to: url)
    }

    func note(_ name: String, _ text: String) {
        let url = URL(fileURLWithPath: shotDirectory).appendingPathComponent("\(name)-\(shotSuffix).txt")
        try? text.write(to: url, atomically: true, encoding: .utf8)
    }

    func useWav(_ name: String) {
        let from = URL(fileURLWithPath: wavDirectory).appendingPathComponent("\(name).wav")
        let to = URL(fileURLWithPath: fakeWav)
        try? FileManager.default.removeItem(at: to)
        XCTAssertNoThrow(try FileManager.default.copyItem(at: from, to: to), "missing WAV \(from.path)")
    }

    func launchWispr(_ extra: [String: String] = [:]) {
        wispr.launchEnvironment = ["WB_FAKE_MIC_WAV": fakeWav].merging(extra) { _, new in new }
        wispr.launch()
    }

    /// An element with this label or identifier in any of the apps.
    func find(_ label: String, in apps: [XCUIApplication]? = nil, timeout: TimeInterval = 4) -> XCUIElement? {
        let apps = apps ?? [host, wispr, settings, springboard]
        let deadline = Date().addingTimeInterval(timeout)
        repeat {
            for app in apps where app.state == .runningForeground || app === springboard {
                let element = app.descendants(matching: .any).matching(
                    NSPredicate(format: "label == %@ OR identifier == %@", label, label)
                ).firstMatch
                if element.exists { return element }
            }
            usleep(200_000)
        } while Date() < deadline
        return nil
    }

    func need(_ label: String, in apps: [XCUIApplication]? = nil, timeout: TimeInterval = 6,
              file: StaticString = #filePath, line: UInt = #line) throws -> XCUIElement {
        try XCTUnwrap(find(label, in: apps, timeout: timeout), "not on screen: \(label)", file: file, line: line)
    }

    /// Brings the Wispr Bro keyboard up for the focused field.
    @discardableResult
    func switchToWisprKeyboard(_ app: XCUIApplication) -> Bool {
        let ours = ["Start Wispr Bro", "Start dictating"]
        func up() -> Bool { ours.contains { find($0, in: [app, springboard], timeout: 0.6) != nil } }
        // A fresh simulator explains swipe typing and the globe key first.
        func dismissIntro() {
            let titles = app.staticTexts.matching(NSPredicate(
                format: "label BEGINSWITH 'Quickly Change Keyboards' OR label BEGINSWITH 'Speed up your typing'"
            ))
            var rounds = 0
            while titles.firstMatch.waitForExistence(timeout: 1.2), rounds < 4 {
                rounds += 1
                // The card's own Continue is the lowest one on screen.
                let buttons = app.buttons.matching(NSPredicate(format: "label == 'Continue'")).allElementsBoundByIndex
                guard let lowest = buttons.max(by: { $0.frame.minY < $1.frame.minY }) else { break }
                let frame = lowest.frame
                app.coordinate(withNormalizedOffset: .zero)
                    .withOffset(CGVector(dx: frame.midX, dy: frame.midY)).tap()
                sleep(1)
            }
        }
        dismissIntro()
        var tries = 0
        while !up(), tries < 3 {
            tries += 1
            guard let globe = find("Next keyboard", in: [app, springboard], timeout: 2) else { break }
            // Hold the globe key and pick Wispr Bro from the list, as a person does.
            globe.press(forDuration: 1.0)
            let row = app.cells.matching(NSPredicate(format: "label BEGINSWITH 'Wispr Bro'")).firstMatch
            if row.waitForExistence(timeout: 2) {
                let frame = row.frame
                app.coordinate(withNormalizedOffset: .zero)
                    .withOffset(CGVector(dx: frame.midX, dy: frame.midY)).tap()
            }
            sleep(1)
            dismissIntro()
        }
        return up()
    }

    func occurrences(of needle: String, in text: String) -> Int {
        text.lowercased().components(separatedBy: needle.lowercased()).count - 1
    }
}
