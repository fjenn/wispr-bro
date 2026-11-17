// SPDX-License-Identifier: GPL-3.0-or-later
// The session Live Activity's data and its Stop intent. Compiled into the app
// and the widget extension.
//
// Ported from Hex (MIT, (c) 2026 Christopher Langton):
// ios/HEXActivity/HexActivityAttributes.swift. See ../NOTICE-hex.md.

import ActivityKit
import AppIntents
import Foundation

struct WBSessionActivityAttributes: ActivityAttributes {
    enum Stage: String, Codable, Hashable, Sendable {
        case ready
        case listening
        case transcribing
        case ended
    }

    struct ContentState: Codable, Hashable, Sendable {
        var stage: Stage
        /// Start of the current stage (the timer counts from here while listening).
        var since: Date
    }

    var sessionID: String
}

enum WBActivityCommand: String, Sendable {
    case endSession
}

/// Carries Live Activity button presses to the session controller. The intent
/// runs in the app's process, where the controller registered a handler.
actor WBActivityCommandBus {
    static let shared = WBActivityCommandBus()

    private var handler: (@MainActor @Sendable (WBActivityCommand) async -> Void)?

    func register(_ handler: @escaping @MainActor @Sendable (WBActivityCommand) async -> Void) {
        self.handler = handler
    }

    func dispatch(_ command: WBActivityCommand) async {
        await handler?(command)
    }
}

struct WBEndSessionIntent: LiveActivityIntent {
    static let title: LocalizedStringResource = "Stop Wispr Bro"
    static let description = IntentDescription("Ends the dictation session and turns the microphone off.")
    static let isDiscoverable = false

    init() {}

    func perform() async throws -> some IntentResult {
        await WBActivityCommandBus.shared.dispatch(.endSession)
        return .result()
    }
}
