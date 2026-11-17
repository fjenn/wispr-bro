// SPDX-License-Identifier: GPL-3.0-or-later
// The product end to end: another app's text field, the Wispr Bro keyboard,
// the real app and its speech engine, speech from a WAV file.
//
// Needs the app set up once in the simulator (onboarding finished, the
// keyboard added in Settings, the microphone allowed) and en.wav / nl.wav in
// WB_WAV_DIR. The expected words come from WB_EXPECT_EN / WB_EXPECT_NL (a
// few words each WAV says).

import XCTest

final class WBKeyboardFlowTests: WBE2ECase {
    private var expectEN: String { env["WB_EXPECT_EN"] ?? "meeting is moved to Thursday" }
    private var expectNL: String { env["WB_EXPECT_NL"] ?? "vergadering is verplaatst" }

    private var field: XCUIElement { host.descendants(matching: .any)["host.field"] }
    private var fieldText: String { (field.value as? String) ?? "" }

    private func waitForText(_ needle: String, timeout: TimeInterval = 25) -> Bool {
        let deadline = Date().addingTimeInterval(timeout)
        while Date() < deadline {
            if fieldText.lowercased().contains(needle.lowercased()) { return true }
            usleep(300_000)
        }
        return false
    }

    /// Mic → listen for `seconds` → finish. Returns when the text is in.
    private func take(wav: String, seconds: UInt32, expect: String, shots: String) throws {
        useWav(wav)
        try need("Start dictating", in: [host, springboard], timeout: 8).tap()
        _ = try need("Finish and insert", in: [host, springboard], timeout: 6)
        shot("\(shots)-listening")
        sleep(seconds)
        try need("Finish and insert", in: [host, springboard]).tap()
        usleep(250_000)
        shot("\(shots)-writing")
        XCTAssertTrue(waitForText(expect), "\(wav): expected “\(expect)” in: \(fieldText)")
        shot("\(shots)-inserted")
    }

    func testKeyboardFlow() throws {
        // A launch with the environment makes the app remember the WAV
        // microphone; then it is gone, as after a day without dictating.
        useWav("en")
        launchWispr()
        sleep(4)
        wispr.terminate()

        host.launch()
        XCTAssertTrue(field.waitForExistence(timeout: 10))
        try? need("host.clear", in: [host]).tap()
        field.tap()
        sleep(1)
        XCTAssertTrue(switchToWisprKeyboard(host), "the Wispr Bro keyboard never came up")
        shot("flow-01-keyboard-no-session")

        // Cold start: the mic opens Wispr Bro, which starts the session and
        // the take and goes back to the host.
        try need("Start Wispr Bro", in: [host, springboard]).tap()
        sleep(2)
        shot("flow-02-app-opened")
        let returned = host.wait(for: .runningForeground, timeout: 12)
        note("flow-return", returned ? "returned to the host automatically" : "stayed in Wispr Bro")
        XCTAssertTrue(returned, "the app did not go back to the host by itself")
        if !returned {
            host.activate()
            field.tap()
        }
        _ = try need("Finish and insert", in: [host, springboard], timeout: 10)
        shot("flow-03-cold-listening")
        sleep(8)
        try need("Finish and insert", in: [host, springboard]).tap()
        usleep(250_000)
        shot("flow-04-cold-writing")
        XCTAssertTrue(waitForText(expectEN), "cold take: expected “\(expectEN)” in: \(fieldText)")
        shot("flow-05-cold-inserted")
        XCTAssertEqual(occurrences(of: expectEN, in: fieldText), 1, "inserted once: \(fieldText)")

        // Warm take, Dutch: no app switch, one space between the two takes.
        try take(wav: "nl", seconds: 8, expect: expectNL, shots: "flow-06-warm")
        let both = fieldText
        XCTAssertEqual(occurrences(of: expectEN, in: both), 1, "English still once: \(both)")
        XCTAssertEqual(occurrences(of: expectNL, in: both), 1, "Dutch once: \(both)")
        let seam = both.range(of: "Hallo", options: .caseInsensitive).map { String(both[..<$0.lowerBound].suffix(2)) } ?? ""
        XCTAssertTrue(seam.hasSuffix(" ") && !seam.hasSuffix("  "), "one space before the second take: “\(seam)” in \(both)")
        note("flow-text", both)

        // Cancel: nothing is inserted.
        useWav("en")
        try need("Start dictating", in: [host, springboard], timeout: 8).tap()
        _ = try need("Finish and insert", in: [host, springboard], timeout: 6)
        sleep(2)
        try need("Cancel", in: [host, springboard]).tap()
        sleep(4)
        shot("flow-07-cancelled")
        XCTAssertEqual(fieldText, both, "a cancelled take inserts nothing")
        XCTAssertNotNil(find("Start dictating", in: [host, springboard], timeout: 4), "idle after a cancel")
    }

    /// "End session after: Immediately": the session ends right after a take.
    func testImmediateTimeout() throws {
        useWav("en")
        launchWispr()
        try need("Settings", in: [wispr], timeout: 15).tap()
        try need("End session after", in: [wispr]).tap()
        try need("Immediately", in: [wispr]).tap()
        sleep(1)
        shot("timeout-01-setting")

        host.launch()
        XCTAssertTrue(field.waitForExistence(timeout: 10))
        try? need("host.clear", in: [host]).tap()
        field.tap()
        sleep(1)
        XCTAssertTrue(switchToWisprKeyboard(host))
        if let start = find("Start Wispr Bro", in: [host, springboard], timeout: 2) {
            start.tap()
            XCTAssertTrue(host.wait(for: .runningForeground, timeout: 12), "no return to the host")
        } else {
            try need("Start dictating", in: [host, springboard]).tap()
        }
        _ = try need("Finish and insert", in: [host, springboard], timeout: 10)
        sleep(8)
        try need("Finish and insert", in: [host, springboard]).tap()
        XCTAssertTrue(waitForText(expectEN), "expected “\(expectEN)” in: \(fieldText)")
        // The session is gone: the keyboard offers to start Wispr Bro again.
        XCTAssertNotNil(find("Start Wispr Bro", in: [host, springboard], timeout: 8), "the session did not end after the take")
        shot("timeout-02-session-ended")

        // Back to the default for the other checks.
        wispr.activate()
        try need("Settings", in: [wispr], timeout: 10).tap()
        try? need("End session after", in: [wispr], timeout: 3).tap()
        try need("5 minutes", in: [wispr]).tap()
        sleep(1)
    }
}
