// SPDX-License-Identifier: GPL-3.0-or-later
// A held character key as a pure state machine (the accent pop-up).
//
// Adapted from Hex (MIT, (c) 2026 Christopher Langton):
// ios/HEXShared/FullKeyboard/HexKeyGesture.swift (there: the Hex key's hold and
// style menu). See ../../NOTICE-hex.md.

import CoreGraphics
import Foundation

/// One finger on one character key. A tap types the key. Staying on the key for
/// `holdDuration` opens its alternates; from then on moves pick an alternate and the
/// release types it. Sliding to another key before that starts over on the new key.
struct WBKeyHoldGesture: Equatable, Sendable {
    static let holdDuration: TimeInterval = 0.4

    enum State: Equatable, Sendable {
        case idle
        case pressing
        case menu
    }

    enum Output: Equatable, Sendable {
        case ignored
        /// The release of a plain press: type the key.
        case tap
        case openMenu
        case dragMenu
        case releaseMenu
    }

    private(set) var state: State = .idle
    /// Counts presses, so a timer started for an earlier key is recognised as stale.
    private(set) var generation = 0

    /// The finger landed on a key, or slid onto another one.
    mutating func touchDown() {
        state = .pressing
        generation += 1
    }

    mutating func touchMoved() -> Output {
        state == .menu ? .dragMenu : .ignored
    }

    /// The hold timer started at `generation` ran out. `hasAlternates`: the key has any.
    mutating func holdElapsed(generation: Int, hasAlternates: Bool) -> Output {
        guard state == .pressing, generation == self.generation, hasAlternates else { return .ignored }
        state = .menu
        return .openMenu
    }

    mutating func touchUp() -> Output {
        let output: Output
        switch state {
        case .idle: output = .ignored
        case .pressing: output = .tap
        case .menu: output = .releaseMenu
        }
        state = .idle
        return output
    }

    mutating func cancel() {
        state = .idle
    }
}
