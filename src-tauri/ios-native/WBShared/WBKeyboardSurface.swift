// SPDX-License-Identifier: GPL-3.0-or-later
// The keyboard's dictation state and the dictation strip above the typing
// keys (FullKeyboard/). Lives in WBShared so the app can show the same view
// as a preview.
//
// Parts ported from Hex (MIT, (c) 2026 Christopher Langton):
// ios/HEXShared/KeyboardSurface.swift (pending-command reconciliation).
// See ../NOTICE-hex.md.

import SwiftUI
import UIKit

// MARK: - Model

enum WBKeyboardPhase: Equatable {
    case needsFullAccess
    case noSession
    case idle
    case listening
    case transcribing
    case error(String)
}

final class WBKeyboardModel: ObservableObject {
    struct Pending: Equatable {
        let kind: WBCommandKind
        let takeID: UInt64
        let sentAt: TimeInterval
    }

    /// How long the keyboard shows its own guess before the app confirms.
    static let pendingLifetime: TimeInterval = 2.5

    @Published var phase: WBKeyboardPhase = .noSession
    @Published var needsGlobeKey = false
    /// "Tap to talk" beside the mic: the first few takes only.
    @Published var showsTalkHint = false

    var pending: Pending?
    var levelSource: () -> Float = { 0 }
    /// Still previews only.
    var frozenWave: [Double]?

    var onMicTap: () -> Void = {}
    var onFinishTap: () -> Void = {}
    var onCancelTap: () -> Void = {}
    var onOpenApp: () -> Void = {}

    /// What to show, given what the app last published and what the keyboard
    /// just asked for. Returns whether the pending command is settled.
    static func resolve(
        hasFullAccess: Bool,
        snapshot: WBSnapshot,
        pending: Pending?,
        now: TimeInterval
    ) -> (phase: WBKeyboardPhase, pendingSettled: Bool) {
        guard hasFullAccess else { return (.needsFullAccess, true) }

        let base: WBKeyboardPhase
        switch snapshot.state {
        case .noSession: base = .noSession
        case .idle: base = .idle
        case .listening: base = .listening
        case .transcribing: base = .transcribing
        case .error: base = .error(snapshot.message ?? "Something went wrong")
        }

        guard let pending else { return (base, true) }
        guard now - pending.sentAt < pendingLifetime else { return (base, true) }

        let busy = snapshot.state == .listening || snapshot.state == .transcribing
        let sameTake = snapshot.takeID == pending.takeID
        switch pending.kind {
        case .start:
            if sameTake && busy { return (base, true) }
            if snapshot.state == .error && snapshot.updatedAt >= pending.sentAt { return (base, true) }
            if snapshot.state == .noSession { return (base, true) }
            return (.listening, false)
        case .stop:
            if sameTake && snapshot.state == .listening { return (.transcribing, false) }
            return (base, true)
        case .cancel:
            if sameTake && busy { return (.idle, false) }
            return (base, true)
        }
    }

    func apply(hasFullAccess: Bool, snapshot: WBSnapshot, now: TimeInterval) {
        let resolved = Self.resolve(hasFullAccess: hasFullAccess, snapshot: snapshot, pending: pending, now: now)
        if resolved.pendingSettled { pending = nil }
        if phase != resolved.phase { phase = resolved.phase }
    }
}

// MARK: - Dictation strip

/// The row above the keys: the mic, and while a take runs the listening capsule.
/// Its height is fixed by the keyboard's layout, so nothing below it ever moves.
struct WBDictationStrip: View {
    @ObservedObject var model: WBKeyboardModel
    @Environment(\.colorScheme) private var scheme

    private enum Kind: Equatable {
        case mic
        case label
        case wave
        case working
    }

    private var kind: Kind {
        switch model.phase {
        case .listening: .wave
        case .transcribing: .working
        case .idle: model.showsTalkHint ? .label : .mic
        case .needsFullAccess, .noSession, .error: .label
        }
    }

    var body: some View {
        GeometryReader { geometry in
            let height = max(28, min(40, geometry.size.height - 4))
            pill(height: height)
                .frame(maxWidth: .infinity, maxHeight: .infinity)
        }
    }

    private func pill(height: CGFloat) -> some View {
        let kind = kind
        return ZStack {
            switch kind {
            case .mic:
                Button(action: model.onMicTap) {
                    Image(systemName: "mic.fill")
                        .font(.system(size: height * 0.44, weight: .medium))
                        .foregroundStyle(WBTheme.pillInk)
                        .frame(width: height * 1.7, height: height)
                        .contentShape(Capsule().inset(by: -6))
                }
                .buttonStyle(WBPressStyle())
                .accessibilityLabel("Start dictating")
                .transition(.opacity)

            case .label:
                labelled(height: height)
                    .transition(.opacity)

            case .wave:
                HStack(spacing: 0) {
                    disc(symbol: "xmark", size: height - 8, fill: WBTheme.pillControl, ink: WBTheme.pillInk, label: "Cancel", action: model.onCancelTap)
                    Spacer(minLength: 0)
                    WBWaveformView(level: model.levelSource, scale: height / 30, frozen: model.frozenWave)
                    Spacer(minLength: 0)
                    disc(symbol: "checkmark", size: height - 8, fill: WBTheme.pillInk, ink: WBTheme.pill, label: "Finish and insert", action: model.onFinishTap)
                }
                .padding(.horizontal, 4)
                .frame(width: 216, height: height)
                .transition(.opacity)

            case .working:
                Button(action: model.onCancelTap) {
                    HStack(spacing: 10) {
                        Text("Transcribing…")
                            .font(WBTheme.pillFont)
                            .foregroundStyle(WBTheme.pillInk)
                        WBSpinner(size: 17)
                    }
                    .frame(width: 176, height: height)
                    .contentShape(Capsule().inset(by: -6))
                }
                .buttonStyle(WBPressStyle())
                .accessibilityLabel("Transcribing. Tap to cancel.")
                .transition(.opacity)
            }
        }
        .background(
            Capsule(style: .continuous)
                .fill(WBTheme.pill)
                .overlay(
                    Capsule(style: .continuous)
                        .strokeBorder(scheme == .dark ? Color.white.opacity(0.16) : .clear, lineWidth: 0.5)
                )
                .shadow(color: .black.opacity(scheme == .dark ? 0 : 0.16), radius: 4, x: 0, y: 1.5)
        )
        .animation(WBTheme.spring, value: kind)
    }

    private struct Label {
        let symbol: String
        let symbolInk: Color
        let text: String
        let trailing: String?
        let accessibility: String
        let action: () -> Void
    }

    private var label: Label {
        switch model.phase {
        case .needsFullAccess:
            Label(
                symbol: "mic.slash.fill", symbolInk: WBTheme.pillInk,
                text: "Allow Full Access to dictate", trailing: "arrow.up.forward",
                accessibility: "Open Wispr Bro to allow Full Access", action: model.onOpenApp
            )
        case .noSession:
            Label(
                symbol: "mic.fill", symbolInk: WBTheme.pillInk,
                text: "Open Wispr Bro to start", trailing: "arrow.up.forward",
                accessibility: "Start Wispr Bro", action: model.onMicTap
            )
        case .error(let message):
            Label(
                symbol: "exclamationmark.circle.fill", symbolInk: Color(wbHex: 0xC3A0EE),
                text: message, trailing: "arrow.clockwise",
                accessibility: "Start dictating", action: model.onMicTap
            )
        default:
            Label(
                symbol: "mic.fill", symbolInk: WBTheme.pillInk,
                text: "Tap to talk", trailing: nil,
                accessibility: "Start dictating", action: model.onMicTap
            )
        }
    }

    private func labelled(height: CGFloat) -> some View {
        let label = label
        return Button(action: label.action) {
            HStack(spacing: 8) {
                Image(systemName: label.symbol)
                    .font(.system(size: 15, weight: .medium))
                    .foregroundStyle(label.symbolInk)
                Text(label.text)
                    .font(WBTheme.pillFont)
                    .foregroundStyle(WBTheme.pillInk)
                    .lineLimit(1)
                    .minimumScaleFactor(0.8)
                if let trailing = label.trailing {
                    Image(systemName: trailing)
                        .font(.system(size: 11, weight: .semibold))
                        .foregroundStyle(WBTheme.pillInk.opacity(0.55))
                }
            }
            .padding(.horizontal, 16)
            .frame(height: height)
            .frame(maxWidth: 340)
            .fixedSize(horizontal: true, vertical: false)
            .contentShape(Capsule().inset(by: -6))
        }
        .buttonStyle(WBPressStyle())
        .accessibilityLabel(label.accessibility)
    }

    private func disc(symbol: String, size: CGFloat, fill: Color, ink: Color, label: String, action: @escaping () -> Void) -> some View {
        Button(action: action) {
            Image(systemName: symbol)
                .font(.system(size: size * 0.42, weight: .semibold))
                .foregroundStyle(ink)
                .frame(width: size, height: size)
                .background(fill, in: Circle())
                .contentShape(Rectangle().inset(by: -8))
        }
        .buttonStyle(WBPressStyle())
        .accessibilityLabel(label)
    }
}

struct WBPressStyle: ButtonStyle {
    func makeBody(configuration: Configuration) -> some View {
        configuration.label
            .opacity(configuration.isPressed ? 0.72 : 1)
            .scaleEffect(configuration.isPressed ? 0.97 : 1)
            .animation(WBTheme.fast, value: configuration.isPressed)
    }
}
