// SPDX-License-Identifier: GPL-3.0-or-later
// Shift, auto-capitalisation, double-space period, key planes and field routing.
//
// Ported from Hex (MIT, (c) 2026 Christopher Langton):
// ios/HEXShared/FullKeyboard/ShiftStateMachine.swift. See ../../NOTICE-hex.md.

import Foundation
import UIKit

enum WBShiftState: Equatable, Sendable {
    case off
    /// One-shot: the next character is uppercase, then shift turns off.
    case on
    /// Caps lock.
    case locked
}

/// Shift as a pure state machine. Shift changes on touch-down so a left-thumb chord
/// (shift down, letter down) types a capital; a quick double tap locks.
struct WBShiftStateMachine: Equatable, Sendable {
    static let doubleTapWindow: TimeInterval = 0.3

    private(set) var state: WBShiftState = .off
    private(set) var isHeld = false
    private var typedWhileHeld = false
    private var lastTapDown: TimeInterval?

    init(state: WBShiftState = .off) {
        self.state = state
    }

    var isUppercase: Bool { state != .off || isHeld }

    mutating func shiftDown(now: TimeInterval) {
        isHeld = true
        typedWhileHeld = false
        if state == .locked {
            state = .off
            lastTapDown = nil
        } else if let last = lastTapDown, now - last < Self.doubleTapWindow {
            state = .locked
            lastTapDown = nil
        } else {
            state = state == .off ? .on : .off
            lastTapDown = now
        }
    }

    mutating func shiftUp() {
        isHeld = false
        if typedWhileHeld {
            // A chord, not a tap: it neither stays on nor counts toward a double tap.
            typedWhileHeld = false
            lastTapDown = nil
            if state != .locked {
                state = .off
            }
        }
    }

    /// Call after a character key commits.
    mutating func didType() {
        if isHeld {
            typedWhileHeld = true
        } else if state == .on {
            state = .off
        }
        lastTapDown = nil
    }

    /// Auto-capitalization after an edit or a field change. Leaves caps lock and a held shift alone.
    mutating func refreshAutoCapitalization(_ shouldCapitalize: Bool) {
        guard state != .locked, !isHeld else { return }
        state = shouldCapitalize ? .on : .off
    }

    mutating func reset() {
        self = WBShiftStateMachine()
    }
}

enum WBAutoCapitalization {
    enum Mode: Equatable, Sendable {
        case none
        case words
        case sentences
        case allCharacters

        init(_ type: UITextAutocapitalizationType?) {
            switch type {
            case .some(UITextAutocapitalizationType.none): self = Mode.none
            case .some(.words): self = .words
            case .some(.allCharacters): self = .allCharacters
            default: self = .sentences
            }
        }
    }

    private static let sentenceEnders: Set<Character> = [".", "!", "?"]
    private static let closers: Set<Character> = ["\"", "'", ")", "]", "\u{201D}", "\u{2019}"]

    /// Whether the next letter should be a capital. `before` is the text before the cursor,
    /// which web views sometimes report as `nil` even when the field has text: only the
    /// start of an empty field capitalizes then.
    static func shouldCapitalize(mode: Mode, before: String?, hasText: Bool) -> Bool {
        switch mode {
        case .none:
            return false
        case .allCharacters:
            return true
        case .words, .sentences:
            break
        }
        guard let before else { return !hasText }
        guard let last = before.last else { return true }
        if last.isNewline { return true }
        guard last.isWhitespace else { return false }
        if mode == .words { return true }

        var trimmed = Substring(before)
        while let c = trimmed.last, c.isWhitespace, !c.isNewline {
            trimmed = trimmed.dropLast()
        }
        guard let lastWritten = trimmed.last else { return true }
        if lastWritten.isNewline { return true }
        while let c = trimmed.last, closers.contains(c) {
            trimmed = trimmed.dropLast()
        }
        guard let ender = trimmed.last else { return false }
        return sentenceEnders.contains(ender)
    }
}

/// Two spaces in a row after a word become ". ".
enum WBDoubleSpacePeriod {
    static let window: TimeInterval = 0.45

    /// `lastSpaceAt` is when this keyboard typed the space now before the cursor (nil if
    /// anything else happened since). Requires a letter or digit right before that space.
    static func shouldReplace(before: String?, lastSpaceAt: TimeInterval?, now: TimeInterval) -> Bool {
        guard let before, let lastSpaceAt, now - lastSpaceAt <= window, now >= lastSpaceAt else {
            return false
        }
        guard before.last == " " else { return false }
        guard let previous = before.dropLast().last else { return false }
        return previous.isLetter || previous.isNumber
    }
}

/// Which key page shows. 123 and #+= are transient: space or return goes back to letters.
struct WBKeyLayerState: Equatable, Sendable {
    private(set) var layer: WBKeyLayer

    init(layer: WBKeyLayer = .letters) {
        self.layer = layer
    }

    mutating func switchTo(_ next: WBKeyLayer) {
        layer = next
    }

    mutating func didInsert(_ text: String) {
        guard layer != .letters else { return }
        if text == " " || text == "\n" || text == "'" {
            layer = .letters
        }
    }

    mutating func reset(to initial: WBKeyLayer = .letters) {
        layer = initial
    }
}

enum WBFieldRoute: Equatable, Sendable {
    /// These keys, with this field kind and first page.
    case keys(WBFieldKind, initialLayer: WBKeyLayer)
    /// Number pads go to the next keyboard; 123 shows in case there is none.
    case handOff
}

enum WBFieldRouting {
    static func route(for keyboardType: UIKeyboardType?) -> WBFieldRoute {
        switch keyboardType {
        case .URL?:
            return .keys(.url, initialLayer: .letters)
        case .emailAddress?:
            return .keys(.email, initialLayer: .letters)
        case .webSearch?:
            return .keys(.webSearch, initialLayer: .letters)
        case .numberPad?, .decimalPad?, .asciiCapableNumberPad?, .phonePad?, .namePhonePad?:
            return .handOff
        case .numbersAndPunctuation?:
            return .keys(.text, initialLayer: .numbers)
        default:
            return .keys(.text, initialLayer: .letters)
        }
    }
}
