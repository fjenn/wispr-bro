// SPDX-License-Identifier: GPL-3.0-or-later
// Ported from Hex (MIT, (c) 2026 Christopher Langton): ios/HEXMobileTests/ShiftStateMachineTests.swift.
// See ../NOTICE-hex.md.

import UIKit
import XCTest
@testable import WBNativeHarness

final class WBShiftStateTests: XCTestCase {
    func testTapTurnsShiftOnForOneCharacter() {
        var shift = WBShiftStateMachine()
        shift.shiftDown(now: 0)
        shift.shiftUp()
        XCTAssertEqual(shift.state, .on)
        XCTAssertTrue(shift.isUppercase)
        shift.didType()
        XCTAssertEqual(shift.state, .off)
        XCTAssertFalse(shift.isUppercase)
    }

    func testTappingShiftAgainTurnsItOff() {
        var shift = WBShiftStateMachine()
        shift.shiftDown(now: 0)
        shift.shiftUp()
        shift.shiftDown(now: 1)
        shift.shiftUp()
        XCTAssertEqual(shift.state, .off)
    }

    func testChordTypesCapitalsWhileHeldThenTurnsOff() {
        var shift = WBShiftStateMachine()
        shift.shiftDown(now: 0)
        XCTAssertTrue(shift.isUppercase)
        shift.didType()  // "A"
        XCTAssertTrue(shift.isUppercase, "still held")
        shift.didType()  // "B"
        XCTAssertTrue(shift.isUppercase)
        shift.shiftUp()
        XCTAssertEqual(shift.state, .off)
    }

    func testChordDoesNotCountTowardDoubleTap() {
        var shift = WBShiftStateMachine()
        shift.shiftDown(now: 0)
        shift.didType()
        shift.shiftUp()
        shift.shiftDown(now: 0.1)
        XCTAssertEqual(shift.state, .on, "a fresh tap, not caps lock")
    }

    func testDoubleTapLocksAndTapUnlocks() {
        var shift = WBShiftStateMachine()
        shift.shiftDown(now: 0)
        shift.shiftUp()
        shift.shiftDown(now: 0.2)
        shift.shiftUp()
        XCTAssertEqual(shift.state, .locked)
        shift.didType()
        shift.didType()
        XCTAssertEqual(shift.state, .locked)
        shift.refreshAutoCapitalization(false)
        XCTAssertEqual(shift.state, .locked, "auto-caps never clears caps lock")
        shift.shiftDown(now: 5)
        shift.shiftUp()
        XCTAssertEqual(shift.state, .off)
    }

    func testSlowSecondTapTurnsShiftOff() {
        var shift = WBShiftStateMachine()
        shift.shiftDown(now: 0)
        shift.shiftUp()
        shift.shiftDown(now: 0.31)
        shift.shiftUp()
        XCTAssertEqual(shift.state, .off)
    }

    func testDoubleTapFromAutoCapsLocks() {
        var shift = WBShiftStateMachine()
        shift.refreshAutoCapitalization(true)
        shift.shiftDown(now: 0)
        shift.shiftUp()
        XCTAssertEqual(shift.state, .off, "one tap clears auto-caps")
        shift.shiftDown(now: 0.15)
        shift.shiftUp()
        XCTAssertEqual(shift.state, .locked)
    }

    func testAutoCapsLeavesAHeldShiftAlone() {
        var shift = WBShiftStateMachine()
        shift.shiftDown(now: 0)
        shift.refreshAutoCapitalization(false)
        XCTAssertTrue(shift.isUppercase)
    }

    // MARK: - Auto-capitalization

    func testSentenceCapitalization() {
        let cases: [(String?, Bool, Bool)] = [
            (nil, false, true),
            (nil, true, false),  // web view lost the context
            ("", true, true),
            ("Hello", true, false),
            ("Hello ", true, false),
            ("Hello. ", true, true),
            ("Hello.  ", true, true),
            ("Really? ", true, true),
            ("Wow! ", true, true),
            ("He said \"hi.\" ", true, true),
            ("(Done.) ", true, true),
            ("e.g.", true, false),
            ("Line one\n", true, true),
            ("Line one\n  ", true, true),
            ("   ", true, true),
            ("7:30 ", true, false),
        ]
        for (before, hasText, expected) in cases {
            XCTAssertEqual(
                WBAutoCapitalization.shouldCapitalize(mode: .sentences, before: before, hasText: hasText),
                expected,
                "before: \(String(describing: before)) hasText: \(hasText)"
            )
        }
    }

    func testOtherCapitalizationModes() {
        XCTAssertFalse(WBAutoCapitalization.shouldCapitalize(mode: .none, before: "", hasText: false))
        XCTAssertTrue(WBAutoCapitalization.shouldCapitalize(mode: .allCharacters, before: "abc", hasText: true))
        XCTAssertTrue(WBAutoCapitalization.shouldCapitalize(mode: .words, before: "Jane ", hasText: true))
        XCTAssertFalse(WBAutoCapitalization.shouldCapitalize(mode: .words, before: "Ja", hasText: true))
        XCTAssertEqual(WBAutoCapitalization.Mode(UITextAutocapitalizationType.none), .none)
        XCTAssertEqual(WBAutoCapitalization.Mode(nil), .sentences)
    }

    // MARK: - Double-space period

    func testDoubleSpaceAfterAWordTypesAPeriod() {
        XCTAssertTrue(WBDoubleSpacePeriod.shouldReplace(before: "late ", lastSpaceAt: 10, now: 10.2))
        XCTAssertTrue(WBDoubleSpacePeriod.shouldReplace(before: "at 7 ", lastSpaceAt: 10, now: 10.35))
    }

    func testDoubleSpaceEdgeCases() {
        XCTAssertFalse(WBDoubleSpacePeriod.shouldReplace(before: "late ", lastSpaceAt: 10, now: 10.5), "too slow (Wispr Bro: 0.45 s window)")
        XCTAssertFalse(WBDoubleSpacePeriod.shouldReplace(before: "late ", lastSpaceAt: nil, now: 10), "not our space")
        XCTAssertFalse(WBDoubleSpacePeriod.shouldReplace(before: "late. ", lastSpaceAt: 10, now: 10.1), "already punctuated")
        XCTAssertFalse(WBDoubleSpacePeriod.shouldReplace(before: "late  ", lastSpaceAt: 10, now: 10.1), "third space")
        XCTAssertFalse(WBDoubleSpacePeriod.shouldReplace(before: " ", lastSpaceAt: 10, now: 10.1), "start of field")
        XCTAssertFalse(WBDoubleSpacePeriod.shouldReplace(before: nil, lastSpaceAt: 10, now: 10.1), "no context")
        XCTAssertFalse(WBDoubleSpacePeriod.shouldReplace(before: "late", lastSpaceAt: 10, now: 10.1), "cursor moved")
    }

    // MARK: - Layers and fields

    func testNumbersGoBackToLettersOnSpaceOrReturn() {
        var layer = WBKeyLayerState()
        layer.switchTo(.numbers)
        layer.didInsert("7")
        XCTAssertEqual(layer.layer, .numbers)
        layer.didInsert(" ")
        XCTAssertEqual(layer.layer, .letters)

        layer.switchTo(.symbols)
        layer.didInsert("#")
        layer.didInsert("\n")
        XCTAssertEqual(layer.layer, .letters)

        layer.switchTo(.numbers)
        layer.reset()
        XCTAssertEqual(layer.layer, .letters)
    }

    func testFieldRouting() {
        XCTAssertEqual(WBFieldRouting.route(for: .default), .keys(.text, initialLayer: .letters))
        XCTAssertEqual(WBFieldRouting.route(for: nil), .keys(.text, initialLayer: .letters))
        XCTAssertEqual(WBFieldRouting.route(for: .URL), .keys(.url, initialLayer: .letters))
        XCTAssertEqual(WBFieldRouting.route(for: .emailAddress), .keys(.email, initialLayer: .letters))
        XCTAssertEqual(WBFieldRouting.route(for: .numbersAndPunctuation), .keys(.text, initialLayer: .numbers))
        XCTAssertEqual(WBFieldRouting.route(for: .numberPad), .handOff)
        XCTAssertEqual(WBFieldRouting.route(for: .decimalPad), .handOff)
        XCTAssertEqual(WBFieldRouting.route(for: .asciiCapableNumberPad), .handOff)
    }
}
