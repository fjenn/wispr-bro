// SPDX-License-Identifier: GPL-3.0-or-later
// Ported from Hex (MIT, (c) 2026 Christopher Langton): ios/HEXMobileTests/FullKeyboardEngineTests.swift.
// See ../NOTICE-hex.md.

import UIKit
import XCTest
@testable import WBNativeHarness

@MainActor
final class WBKeyEngineTests: XCTestCase {
    private var target: WBFakeTextTarget!
    private var engine: WBKeyEngine!
    private var now: TimeInterval = 100

    override func setUp() async throws {
        target = WBFakeTextTarget()
        target.autocapitalizationType = UITextAutocapitalizationType.none
        engine = WBKeyEngine(target: target)
        engine.width = 393
        engine.clock = { [unowned self] in self.now }
        engine.configureForField()
    }

    private func point(_ kind: WBKeyKind) -> CGPoint {
        let key = engine.layout.key(kind)!
        return CGPoint(x: key.frame.midX, y: key.frame.midY)
    }

    private func down(_ id: Int, _ kind: WBKeyKind) { engine.touchBegan(id: id, at: point(kind)) }
    private func up(_ id: Int, _ kind: WBKeyKind) { engine.touchEnded(id: id, at: point(kind)) }

    /// The layer can change on touch-down, so the finger lifts where it landed.
    private func tap(_ kind: WBKeyKind) {
        let at = point(kind)
        engine.touchBegan(id: 1, at: at)
        engine.touchEnded(id: 1, at: at)
    }

    private func type(_ text: String) {
        for ch in text {
            tap(ch == " " ? .space : .character(String(ch)))
            now += 0.5
        }
    }

    func testRolloverKeepsOrder() {
        down(1, .character("t"))
        down(2, .character("h"))
        up(1, .character("t"))
        up(2, .character("h"))
        XCTAssertEqual(target.text, "th")
    }

    func testRolloverWhenTheSecondKeyLiftsFirst() {
        down(1, .character("t"))
        down(2, .character("h"))
        up(2, .character("h"))
        up(1, .character("t"))
        XCTAssertEqual(target.text, "th")
    }

    func testSlidingToAnotherKeyTypesWhereTheFingerLifts() {
        down(1, .character("f"))
        engine.touchMoved(id: 1, to: point(.character("g")))
        XCTAssertEqual(engine.previewKey?.kind, .character("g"))
        up(1, .character("g"))
        XCTAssertEqual(target.text, "g")
    }

    func testShiftChordAndOneShot() {
        down(1, .shift)
        down(2, .character("a"))
        up(2, .character("a"))
        down(3, .character("b"))
        up(3, .character("b"))
        up(1, .shift)
        tap(.character("c"))
        XCTAssertEqual(target.text, "ABc")

        tap(.shift)
        tap(.character("d"))
        tap(.character("e"))
        XCTAssertEqual(target.text, "ABcDe")
    }

    func testDoubleTapShiftLocks() {
        tap(.shift)
        now += 0.1
        tap(.shift)
        type("hi")
        XCTAssertEqual(target.text, "HI")
        XCTAssertEqual(engine.shift.state, .locked)
    }

    func testHoldNumbersSlideToADigitAndLetGo() {
        down(1, .layerSwitch(.numbers))
        XCTAssertEqual(engine.layout.layer, .numbers)
        engine.touchMoved(id: 1, to: point(.character("7")))
        engine.touchEnded(id: 1, at: point(.character("7")))
        XCTAssertEqual(target.text, "7")
        XCTAssertEqual(engine.layout.layer, .letters)
    }

    func testNumbersStayUntilSpace() {
        tap(.layerSwitch(.numbers))
        XCTAssertEqual(engine.layout.layer, .numbers)
        tap(.character("4"))
        tap(.character("2"))
        XCTAssertEqual(engine.layout.layer, .numbers)
        tap(.space)
        XCTAssertEqual(engine.layout.layer, .letters)
        XCTAssertEqual(target.text, "42 ")
    }

    func testDoubleSpaceTypesAPeriod() {
        type("ok")
        tap(.space)
        now += 0.2
        tap(.space)
        XCTAssertEqual(target.text, "ok. ")
        now += 0.2
        tap(.space)
        XCTAssertEqual(target.text, "ok.  ", "a third space is just a space")
    }

    func testSlowDoubleSpaceStaysTwoSpaces() {
        type("ok")
        tap(.space)
        now += 0.5
        tap(.space)
        XCTAssertEqual(target.text, "ok  ")
    }

    func testSentenceAutoCapitalization() {
        target.autocapitalizationType = .sentences
        engine.configureForField()
        XCTAssertEqual(engine.shift.state, .on)
        type("hi")
        tap(.space)
        now += 0.2
        tap(.space)
        type("there")
        XCTAssertEqual(target.text, "Hi. There")
    }

    func testDeleteAndWordDelete() {
        type("one two")
        down(1, .delete)
        XCTAssertEqual(target.text, "one tw")
        engine.repeatDelete(heldFor: WBKeyEngine.wordDeleteAfter)
        XCTAssertEqual(target.text, "one ")
        engine.repeatDelete(heldFor: WBKeyEngine.wordDeleteAfter)
        XCTAssertEqual(target.text, "")
        up(1, .delete)
    }

    func testEmailFieldShowsAtAndDot() {
        target.keyboardType = .emailAddress
        engine.configureForField()
        tap(.character("@"))
        tap(.character("."))
        XCTAssertEqual(target.text, "@.")
    }

    func testNumberPadOpensOnNumbers() {
        target.keyboardType = .numberPad
        engine.configureForField()
        XCTAssertEqual(engine.layout.layer, .numbers)
    }

    // MARK: Wispr Bro additions

    private func hold(_ id: Int) -> Bool {
        engine.holdElapsed(id: id, generation: engine.touches[id]!.hold.generation)
    }

    func testLongPressTypesTheAccentOverTheKey() {
        down(1, .character("e"))
        XCTAssertTrue(hold(1))
        XCTAssertNil(engine.previewKey, "the row replaces the balloon")
        XCTAssertEqual(engine.alternates?.menu.options.first, "é")
        up(1, .character("e"))
        XCTAssertEqual(target.text, "é")
        XCTAssertNil(engine.alternates)
    }

    func testLongPressSlideToAnotherAccent() {
        type("gr")
        down(1, .character("u"))
        XCTAssertTrue(hold(1))
        let menu = engine.alternates!.menu
        let index = menu.options.firstIndex(of: "ü")!
        let cell = menu.cellFrame(index)
        engine.touchMoved(id: 1, to: CGPoint(x: cell.midX, y: cell.midY))
        XCTAssertEqual(engine.alternates?.selected, index)
        engine.touchEnded(id: 1, at: CGPoint(x: cell.midX, y: cell.midY))
        type("n")
        XCTAssertEqual(target.text, "grün")
    }

    func testLongPressFollowsShiftAndUsesItUp() {
        tap(.shift)
        down(1, .character("e"))
        XCTAssertTrue(hold(1))
        XCTAssertEqual(engine.alternates?.menu.options.first, "É")
        up(1, .character("e"))
        type("t")
        XCTAssertEqual(target.text, "Ét")
    }

    func testLongPressOnAKeyWithoutAlternatesIsAPlainKey() {
        down(1, .character("q"))
        XCTAssertFalse(hold(1))
        up(1, .character("q"))
        XCTAssertEqual(target.text, "q")
    }

    func testAStaleHoldTimerDoesNothingAfterSlidingAway() {
        down(1, .character("e"))
        let stale = engine.touches[1]!.hold.generation
        engine.touchMoved(id: 1, to: point(.character("r")))
        XCTAssertFalse(engine.holdElapsed(id: 1, generation: stale))
        up(1, .character("r"))
        XCTAssertEqual(target.text, "r")
    }

    func testASecondFingerCommitsTheOpenAccent() {
        down(1, .character("n"))
        XCTAssertTrue(hold(1))
        down(2, .character("o"))
        XCTAssertNil(engine.alternates)
        up(1, .character("n"))
        up(2, .character("o"))
        XCTAssertEqual(target.text, "ño")
    }

    func testLettingGoFarFromTheRowTypesNothing() {
        down(1, .character("e"))
        XCTAssertTrue(hold(1))
        let far = CGPoint(x: 200, y: engine.layout.totalHeight + 200)
        engine.touchMoved(id: 1, to: far)
        engine.touchEnded(id: 1, at: far)
        XCTAssertEqual(target.text, "")
    }

    func testPunctuationHasAlternatesToo() {
        tap(.layerSwitch(.numbers))
        down(1, .character("?"))
        XCTAssertTrue(hold(1))
        up(1, .character("?"))
        XCTAssertEqual(target.text, "¿")
    }

    func testEszettStaysOneLetterInCapitals() {
        XCTAssertEqual(WBKeyAlternates.options(for: "s", uppercase: true).first, "ẞ")
        XCTAssertEqual(WBKeyAlternates.options(for: "e", uppercase: true).first, "É")
        for letter in ["a", "e", "i", "o", "u", "c", "n", "s"] {
            XCTAssertFalse(WBKeyAlternates.options(for: letter, uppercase: false).isEmpty, letter)
        }
        for needed in ["é", "ë", "ï", "ü", "ö", "ä", "ß", "ñ", "ç", "è", "à", "ê", "ô", "û", "î", "œ", "á", "í", "ó", "ú"] {
            let found = "aeioucnsy".contains { WBKeyAlternates.options(for: String($0), uppercase: false).contains(needed) }
            XCTAssertTrue(found, needed)
        }
    }

    func testALetterAfterADictatedSentenceGetsASpaceAndACapital() {
        target.autocapitalizationType = .sentences
        engine.configureForField()
        target.insertText("Hello from the stub.")
        engine.didInsertDictation("Hello from the stub.")
        XCTAssertEqual(engine.shift.state, .on)
        type("and so")
        XCTAssertEqual(target.text, "Hello from the stub. And so")
    }

    func testPunctuationOrDeleteAfterADictationGetsNoSpace() {
        target.insertText("Fine.")
        engine.didInsertDictation("Fine.")
        down(1, .delete)
        up(1, .delete)
        type("x")
        XCTAssertEqual(target.text, "Finex")

        target.insertText(" ok,")
        engine.didInsertDictation(" ok,")
        tap(.layerSwitch(.numbers))
        tap(.character("!"))
        XCTAssertEqual(target.text, "Finex ok,!")
    }

    func testADictationEndingInALetterOwesNoSpace() {
        target.insertText("station")
        engine.didInsertDictation("station")
        type("s")
        XCTAssertEqual(target.text, "stations")
    }

    func testOwnEditsReportedByTheSystemKeepTheDoubleSpace() {
        type("ok")
        tap(.space)
        engine.textDidChange()
        now += 0.2
        tap(.space)
        XCTAssertEqual(target.text, "ok. ")
    }

    func testUrlFieldTypesDotComInOneKey() {
        target.keyboardType = .URL
        engine.configureForField()
        type("wisprbro")
        tap(.shift)
        tap(.character(".com"))
        XCTAssertEqual(target.text, "wisprbro.com")
    }

    func testReturnWaitsForTextWhenTheFieldAsks() {
        target.enablesReturnKeyAutomatically = true
        XCTAssertFalse(engine.isReturnEnabled)
        tap(.returnKey)
        XCTAssertEqual(target.text, "")
        type("a")
        XCTAssertTrue(engine.isReturnEnabled)
        tap(.returnKey)
        XCTAssertEqual(target.text, "a\n")
    }

    func testDeleteRepeatSpeedsUpThenGoesWordWise() {
        let early = WBKeyEngine.repeatInterval(heldFor: WBKeyEngine.repeatDelay)
        let later = WBKeyEngine.repeatInterval(heldFor: 1.2)
        XCTAssertGreaterThan(early, later)
        XCTAssertGreaterThan(WBKeyEngine.repeatInterval(heldFor: WBKeyEngine.wordDeleteAfter), later, "words go slower")
    }
}
