// SPDX-License-Identifier: GPL-3.0-or-later
// Starts, updates and ends the session's Live Activity.
//
// Ported from Hex (MIT, (c) 2026 Christopher Langton):
// ios/HEXMobile/LiveActivityCoordinator.swift. See ../NOTICE-hex.md.

import ActivityKit
import Foundation

@MainActor
final class WBLiveActivityCoordinator {
    private typealias Attributes = WBSessionActivityAttributes
    private var currentID: String?
    private var lastStage: Attributes.Stage?

    /// Starts the activity (the app must be in front) or updates it.
    func show(_ stage: WBSessionActivityAttributes.Stage, since: Date = Date()) {
        guard ActivityAuthorizationInfo().areActivitiesEnabled else { return }
        guard stage != lastStage || currentID == nil else { return }
        lastStage = stage
        let content = ActivityContent(
            state: Attributes.ContentState(stage: stage, since: since),
            staleDate: nil
        )
        if let id = currentID {
            Task.detached {
                await Activity<Attributes>.activities.first { $0.id == id }?.update(content)
            }
            return
        }
        let attributes = Attributes(sessionID: UUID().uuidString)
        currentID = try? Activity.request(attributes: attributes, content: content, pushType: nil).id
    }

    /// Ends this session's activity and any left over from an earlier run.
    func end() {
        currentID = nil
        lastStage = nil
        Task.detached {
            let content = ActivityContent(
                state: Attributes.ContentState(stage: .ended, since: Date()),
                staleDate: nil
            )
            for activity in Activity<Attributes>.activities {
                await activity.end(content, dismissalPolicy: .immediate)
            }
        }
    }
}
