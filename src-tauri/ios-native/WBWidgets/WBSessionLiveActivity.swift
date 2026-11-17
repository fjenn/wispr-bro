// SPDX-License-Identifier: GPL-3.0-or-later
// The session's Live Activity: Lock Screen banner and Dynamic Island.
//
// Ported from Hex (MIT, (c) 2026 Christopher Langton):
// ios/HEXWidgets/HexDictationLiveActivity.swift. See ../NOTICE-hex.md.

import ActivityKit
import AppIntents
import SwiftUI
import WidgetKit

private enum WBActivityPalette {
    static let ink = Color.white
    static let muted = Color.white.opacity(0.62)
    static let control = Color(red: 0x30 / 255, green: 0x30 / 255, blue: 0x2F / 255)
    static let accent = Color(red: 0xC3 / 255, green: 0xA0 / 255, blue: 0xEE / 255)
}

struct WBSessionLiveActivity: Widget {
    var body: some WidgetConfiguration {
        ActivityConfiguration(for: WBSessionActivityAttributes.self) { context in
            WBSessionBanner(stage: stage(context), since: context.state.since)
                .padding(.horizontal, 16)
                .padding(.vertical, 14)
                .activityBackgroundTint(.black)
                .activitySystemActionForegroundColor(.white)
        } dynamicIsland: { context in
            let stage = stage(context)
            return DynamicIsland {
                DynamicIslandExpandedRegion(.bottom) {
                    WBSessionBanner(stage: stage, since: context.state.since)
                        .padding(.horizontal, 4)
                        .padding(.bottom, 4)
                }
            } compactLeading: {
                Image(systemName: "mic.fill")
                    .font(.system(size: 13, weight: .semibold))
                    .foregroundStyle(stage == .listening ? WBActivityPalette.accent : WBActivityPalette.ink)
                    .padding(.leading, 3)
            } compactTrailing: {
                switch stage {
                case .listening:
                    Text(timerInterval: context.state.since...Date.distantFuture, countsDown: false)
                        .font(.system(size: 13, weight: .semibold))
                        .monospacedDigit()
                        .foregroundStyle(WBActivityPalette.ink)
                        .frame(maxWidth: 42, alignment: .trailing)
                        .padding(.trailing, 2)
                case .transcribing:
                    Image(systemName: "ellipsis")
                        .font(.system(size: 13, weight: .bold))
                        .foregroundStyle(WBActivityPalette.accent)
                        .padding(.trailing, 4)
                case .ready, .ended:
                    WBMiniWave(active: false)
                        .padding(.trailing, 4)
                }
            } minimal: {
                Image(systemName: "mic.fill")
                    .font(.system(size: 12, weight: .semibold))
                    .foregroundStyle(stage == .listening ? WBActivityPalette.accent : WBActivityPalette.ink)
            }
            .keylineTint(WBActivityPalette.accent)
        }
    }

    private func stage(_ context: ActivityViewContext<WBSessionActivityAttributes>) -> WBSessionActivityAttributes.Stage {
        context.isStale ? .ended : context.state.stage
    }
}

/// A still waveform: Live Activities are snapshots, so this never animates.
private struct WBMiniWave: View {
    let active: Bool
    private static let shape: [CGFloat] = [0.35, 0.6, 1, 0.7, 0.45]

    var body: some View {
        HStack(spacing: 2) {
            ForEach(0..<5, id: \.self) { i in
                Capsule()
                    .fill(WBActivityPalette.ink.opacity(active ? 1 : 0.55))
                    .frame(width: 2, height: active ? 4 + 10 * Self.shape[i] : 2)
            }
        }
        .frame(height: 16)
    }
}

private struct WBSessionBanner: View {
    let stage: WBSessionActivityAttributes.Stage
    let since: Date

    private var title: String {
        switch stage {
        case .ready: "Ready when you are"
        case .listening: "Listening"
        case .transcribing: "Transcribing…"
        case .ended: "Session ended"
        }
    }

    var body: some View {
        HStack(spacing: 12) {
            WBBrandMark()
                .fill(WBActivityPalette.ink)
                .frame(width: 26, height: 26)
                .frame(width: 40, height: 40)
                .background(WBActivityPalette.control, in: Circle())

            VStack(alignment: .leading, spacing: 2) {
                Text(title)
                    .font(.system(size: 15, weight: .semibold))
                    .foregroundStyle(WBActivityPalette.ink)
                if stage == .listening {
                    Text(timerInterval: since...Date.distantFuture, countsDown: false)
                        .font(.system(size: 13, weight: .medium))
                        .monospacedDigit()
                        .foregroundStyle(WBActivityPalette.muted)
                } else {
                    Text(stage == .ended ? "The microphone is off" : "Wispr Bro · on this iPhone")
                        .font(.system(size: 13))
                        .foregroundStyle(WBActivityPalette.muted)
                }
            }

            Spacer(minLength: 8)

            if stage != .ended {
                Button(intent: WBEndSessionIntent()) {
                    Text("Stop")
                        .font(.system(size: 14, weight: .semibold))
                        .foregroundStyle(Color.black)
                        .padding(.horizontal, 16)
                        .frame(height: 34)
                        .background(WBActivityPalette.ink, in: Capsule())
                }
                .buttonStyle(.plain)
                .accessibilityLabel("Stop the dictation session")
            }
        }
    }
}
