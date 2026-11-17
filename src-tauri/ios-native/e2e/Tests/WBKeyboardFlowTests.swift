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

    /// Home's "Total words", in the running app (Home must be showing):
    /// the number is the text right after its caption in the page.
    private func totalWords() -> Int? {
        let labels = wispr.staticTexts.allElementsBoundByIndex.map(\.label)
        guard let at = labels.firstIndex(of: "Total words"), at + 1 < labels.count else { return nil }
        return Int(labels[at + 1].filter(\.isNumber))
    }

    /// Settings › End session after › `choice`, in the running app.
    private func setSessionTimeout(_ choice: String) throws {
        try need("Settings", in: [wispr], timeout: 15).tap()
        try need("End session after", in: [wispr]).tap()
        try need(choice, in: [wispr]).tap()
        sleep(1)
    }

    /// The keyboard's mic with no session: Wispr Bro opens, starts the
    /// session and the take, and the host comes back (by itself, or by the
    /// swipe the hint asks for). Ends listening, in the host.
    private func coldStart(shots: String) throws {
        try need("Start Wispr Bro", in: [host, springboard]).tap()
        sleep(3)
        let hint = find("Stay in Wispr Bro", in: [wispr], timeout: 1)
        note("\(shots)-return", hint == nil ? "returned to the host automatically" : "swipe-back hint")
        if hint != nil {
            shot("\(shots)-swipe-hint")
            host.activate()
            sleep(1)
        }
        _ = try need("Finish and insert", in: [host, springboard], timeout: 10)
    }

    func testKeyboardFlow() throws {
        // A launch with the environment makes the app remember the WAV
        // microphone; then it is gone, as after a day without dictating.
        useWav("en")
        launchWispr()
        _ = try need("Total words", in: [wispr], timeout: 15)
        let wordsBefore = totalWords()
        try setSessionTimeout("5 minutes")
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
        try coldStart(shots: "flow-02")
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

        // A correction typed with the letter keys under the dictation strip:
        // delete the last character, then type a word. The text before it is
        // what was dictated.
        try need("wb.key.delete", in: [host, springboard], timeout: 6).tap()
        for key in ["wb.key.space", "wb.key.o", "wb.key.k"] {
            try need(key, in: [host, springboard], timeout: 4).tap()
        }
        usleep(400_000)
        shot("flow-07b-typed-correction")
        let typed = fieldText
        XCTAssertTrue(typed.lowercased().hasSuffix(" ok"), "typed with the letter keys: \(typed)")
        XCTAssertTrue(typed.hasPrefix(String(both.dropLast())), "the dictated text stays: \(typed)")
        XCTAssertNotNil(find("Start dictating", in: [host, springboard], timeout: 4), "the dictation strip stays over the keys")

        // The app kept up: both takes are in History, and Home counts their words.
        wispr.activate()
        sleep(2)
        try need("History", in: [wispr], timeout: 10).tap()
        for words in [expectEN, expectNL] {
            let row = wispr.staticTexts.matching(NSPredicate(format: "label CONTAINS[c] %@", words)).firstMatch
            XCTAssertTrue(row.waitForExistence(timeout: 8), "not in History: \(words)")
        }
        shot("flow-08-history")
        try need("Home", in: [wispr]).tap()
        _ = try need("Total words", in: [wispr], timeout: 8)
        sleep(1)
        let wordsAfter = totalWords()
        shot("flow-09-home-stats")
        note("flow-words", "total words \(wordsBefore ?? -1) → \(wordsAfter ?? -1)")
        if let wordsBefore, let wordsAfter {
            XCTAssertGreaterThanOrEqual(wordsAfter - wordsBefore, 45, "Home's word count did not grow by the two takes")
        } else {
            XCTFail("Home's word count could not be read")
        }
    }

    /// "End session after: Immediately": the session ends right after a take.
    func testImmediateTimeout() throws {
        useWav("en")
        launchWispr()
        try setSessionTimeout("Immediately")
        shot("timeout-01-setting")
        defer {
            // Back to the default for the other checks.
            wispr.activate()
            try? setSessionTimeout("5 minutes")
        }
        find("Home", in: [wispr], timeout: 2)?.tap()
        find("End session", in: [wispr], timeout: 2)?.tap()

        host.launch()
        XCTAssertTrue(field.waitForExistence(timeout: 10))
        try? need("host.clear", in: [host]).tap()
        field.tap()
        sleep(1)
        XCTAssertTrue(switchToWisprKeyboard(host))
        try coldStart(shots: "timeout-02")
        sleep(8)
        try need("Finish and insert", in: [host, springboard]).tap()
        XCTAssertTrue(waitForText(expectEN), "expected “\(expectEN)” in: \(fieldText)")
        // The session is gone: the keyboard offers to start Wispr Bro again.
        XCTAssertNotNil(find("Start Wispr Bro", in: [host, springboard], timeout: 8), "the session did not end after the take")
        shot("timeout-03-session-ended")
    }
}
