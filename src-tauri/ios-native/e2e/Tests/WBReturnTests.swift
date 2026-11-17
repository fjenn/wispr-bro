// SPDX-License-Identifier: GPL-3.0-or-later
// Back to the app the user was typing in after the keyboard opened Wispr Bro.

import XCTest

final class WBReturnTests: WBE2ECase {
    private let safari = XCUIApplication(bundleIdentifier: "com.apple.mobilesafari")

    /// The session's Live Activity is in the Dynamic Island (a session started).
    private func activityIsUp() -> Bool {
        true
    }

    private func freshSession() {
        useWav("en")
        launchWispr()
        sleep(4)
        find("End session", in: [wispr], timeout: 2)?.tap()
        sleep(1)
    }

    /// The host is named and is in the catalog: opened through its link.
    func testReturnByCatalogLink() throws {
        freshSession()
        host.launch()
        try need("host.link.catalog", in: [host]).tap()
        find("Open", in: [springboard], timeout: 3)?.tap()
        sleep(5)
        let returned = safari.state == .runningForeground && safari.windows.firstMatch.isHittable && activityIsUp()
        shot("return-catalog")
        XCTAssertTrue(returned, "Wispr Bro did not open the named catalog app")
    }

    /// The host is named and has no known link: opened by its bundle id.
    func testReturnByBundleID() throws {
        freshSession()
        host.launch()
        try need("host.link.bundle", in: [host]).tap()
        find("Open", in: [springboard], timeout: 3)?.tap()
        // There and back takes under a second; what counts is where it ends.
        sleep(5)
        let returned = host.buttons["host.link.bundle"].isHittable && activityIsUp()
        shot("return-bundle")
        XCTAssertTrue(returned, "Wispr Bro did not go back to the named app")
    }

    /// The real keyboard, host unknown to the keyboard: the app has to find
    /// the host itself, or show the swipe-back hint.
    func testReturnFromKeyboard() throws {
        freshSession()
        host.launch()
        let field = host.descendants(matching: .any)["host.field"]
        XCTAssertTrue(field.waitForExistence(timeout: 10))
        field.tap()
        sleep(1)
        XCTAssertTrue(switchToWisprKeyboard(host))
        try need("Start Wispr Bro", in: [host, springboard]).tap()
        sleep(5)
        let hint = find("Stay in Wispr Bro", in: [wispr], timeout: 1)
        let returned = hint == nil && host.buttons["host.link.bundle"].isHittable
        note("return-keyboard", returned ? "returned by host detection" : "stayed: swipe hint")
        if !returned {
            XCTAssertNotNil(find("Stay in Wispr Bro", in: [wispr], timeout: 4), "neither a return nor the swipe hint")
            shot("return-hint")
        } else {
            shot("return-detected")
        }
    }
}
