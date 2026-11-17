// SPDX-License-Identifier: GPL-3.0-or-later
// Adapted from Hex (MIT, (c) 2026 Christopher Langton): ios/HEXMobileTests/HexKeyGestureTests.swift.
// See ../NOTICE-hex.md.

import CoreGraphics
import XCTest
@testable import WBNativeHarness

final class WBKeyHoldGestureTests: XCTestCase {
    func testTapTypesTheKey() {
        var gesture = WBKeyHoldGesture()
        gesture.touchDown()
        XCTAssertEqual(gesture.touchMoved(), .ignored, "a jitter is still a tap")
        XCTAssertEqual(gesture.touchUp(), .tap)
        XCTAssertEqual(gesture.holdElapsed(generation: gesture.generation, hasAlternates: true), .ignored, "a timer left over from the tap")
        XCTAssertEqual(gesture.state, .idle)
    }

    func testHoldOpensTheMenuOnceAndTheReleasePicks() {
        var gesture = WBKeyHoldGesture()
        gesture.touchDown()
        let generation = gesture.generation
        XCTAssertEqual(gesture.holdElapsed(generation: generation, hasAlternates: true), .openMenu)
        XCTAssertEqual(gesture.holdElapsed(generation: generation, hasAlternates: true), .ignored)
        XCTAssertEqual(gesture.touchMoved(), .dragMenu)
        XCTAssertEqual(gesture.touchUp(), .releaseMenu)
        XCTAssertEqual(gesture.state, .idle)
    }

    func testAKeyWithoutAlternatesStaysAPress() {
        var gesture = WBKeyHoldGesture()
        gesture.touchDown()
        XCTAssertEqual(gesture.holdElapsed(generation: gesture.generation, hasAlternates: false), .ignored)
        XCTAssertEqual(gesture.touchUp(), .tap)
    }

    func testSlidingToAnotherKeyStartsOver() {
        var gesture = WBKeyHoldGesture()
        gesture.touchDown()
        let first = gesture.generation
        gesture.touchDown()
        XCTAssertEqual(gesture.holdElapsed(generation: first, hasAlternates: true), .ignored, "the first key's timer is stale")
        XCTAssertEqual(gesture.holdElapsed(generation: gesture.generation, hasAlternates: true), .openMenu)
    }

    func testCancelResets() {
        var gesture = WBKeyHoldGesture()
        gesture.touchDown()
        gesture.cancel()
        XCTAssertEqual(gesture.state, .idle)
        XCTAssertEqual(gesture.holdElapsed(generation: gesture.generation, hasAlternates: true), .ignored)
        XCTAssertEqual(gesture.touchMoved(), .ignored)
        XCTAssertEqual(gesture.touchUp(), .ignored)
    }
}
