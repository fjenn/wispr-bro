// SPDX-License-Identifier: GPL-3.0-or-later
// The keyboard's whole surface: a mic-first stage and one row of utility keys.
// Lives in WBShared so the app can show the same view as a preview.
//
// Parts ported from Hex (MIT, (c) 2026 Christopher Langton):
// ios/HEXShared/KeyboardSurface.swift (pending-command reconciliation, the
// globe key, the repeating delete key). See ../NOTICE-hex.md.

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
    @Published var needsGlobeKey = true
    /// nil shows the return arrow; otherwise "Send", "Search", "Go", "Done".
    @Published var returnLabel: String?

    var pending: Pending?
    var levelSource: () -> Float = { 0 }
    /// Still previews only.
    var frozenWave: [Double]?
    weak var inputViewController: UIInputViewController?

    var onMicTap: () -> Void = {}
    var onFinishTap: () -> Void = {}
    var onCancelTap: () -> Void = {}
    var onOpenApp: () -> Void = {}
    var onSpace: () -> Void = {}
    var onDelete: () -> Void = {}
    var onReturn: () -> Void = {}

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

// MARK: - Palette

private struct WBKeyPalette {
    let key: Color
    let keyPressed: Color
    let keyShadow: Color
    let ink: Color
    let muted: Color
    let card: Color
    let cardStroke: Color
    let pillStroke: Color

    init(_ scheme: ColorScheme) {
        if scheme == .dark {
            key = Color.white.opacity(0.17)
            keyPressed = Color.white.opacity(0.30)
            keyShadow = Color.black.opacity(0.35)
            ink = Color(wbHex: 0xF3F2EE)
            muted = Color(wbHex: 0xF3F2EE, opacity: 0.58)
            card = WBTheme.pillCard
            cardStroke = Color.white.opacity(0.08)
            pillStroke = Color.white.opacity(0.16)
        } else {
            key = Color.white
            keyPressed = Color(wbHex: 0xDAD9D4)
            keyShadow = Color.black.opacity(0.22)
            ink = Color(wbHex: 0x1A1A1A)
            muted = Color(wbHex: 0x1A1A1A, opacity: 0.58)
            card = Color(wbHex: 0xFCFCFA)
            cardStroke = Color.black.opacity(0.06)
            pillStroke = Color.clear
        }
    }
}

// MARK: - Keys

private struct WBKeyStyle: ButtonStyle {
    let palette: WBKeyPalette

    func makeBody(configuration: Configuration) -> some View {
        configuration.label
            .foregroundStyle(palette.ink)
            .frame(maxWidth: .infinity, maxHeight: .infinity)
            .background(
                configuration.isPressed ? palette.keyPressed : palette.key,
                in: RoundedRectangle(cornerRadius: WBTheme.r2, style: .continuous)
            )
            .shadow(color: palette.keyShadow, radius: 0, x: 0, y: 1)
            .contentShape(Rectangle())
    }
}

/// The system "next keyboard" key: a tap switches, a hold shows the list.
private struct WBGlobeKey: UIViewRepresentable {
    weak var controller: UIInputViewController?
    let tint: UIColor

    func makeUIView(context: Context) -> UIButton {
        let button = UIButton(type: .system)
        var config = UIButton.Configuration.plain()
        config.image = UIImage(
            systemName: "globe",
            withConfiguration: UIImage.SymbolConfiguration(pointSize: 17, weight: .regular)
        )
        config.baseForegroundColor = tint
        button.configuration = config
        button.accessibilityLabel = "Next keyboard"
        if let controller {
            button.addTarget(
                controller,
                action: #selector(UIInputViewController.handleInputModeList(from:with:)),
                for: .allTouchEvents
            )
        }
        return button
    }

    func updateUIView(_ uiView: UIButton, context: Context) {
        uiView.configuration?.baseForegroundColor = tint
    }
}

/// Deletes once on touch, then repeats while held.
private struct WBDeleteKey: View {
    let palette: WBKeyPalette
    let onDelete: () -> Void
    @State private var pressing = false
    @State private var repeatTask: Task<Void, Never>?

    var body: some View {
        Image(systemName: pressing ? "delete.left.fill" : "delete.left")
            .font(.system(size: 18, weight: .regular))
            .foregroundStyle(palette.ink)
            .frame(maxWidth: .infinity, maxHeight: .infinity)
            .background(
                pressing ? palette.keyPressed : palette.key,
                in: RoundedRectangle(cornerRadius: WBTheme.r2, style: .continuous)
            )
            .shadow(color: palette.keyShadow, radius: 0, x: 0, y: 1)
            .contentShape(Rectangle())
            .gesture(
                DragGesture(minimumDistance: 0)
                    .onChanged { _ in
                        guard !pressing else { return }
                        pressing = true
                        onDelete()
                        repeatTask = Task { @MainActor in
                            try? await Task.sleep(nanoseconds: 380_000_000)
                            var interval: UInt64 = 90_000_000
                            while !Task.isCancelled {
                                onDelete()
                                try? await Task.sleep(nanoseconds: interval)
                                interval = max(40_000_000, interval - 4_000_000)
                            }
                        }
                    }
                    .onEnded { _ in
                        pressing = false
                        repeatTask?.cancel()
                        repeatTask = nil
                    }
            )
            .onDisappear { repeatTask?.cancel() }
            .accessibilityLabel("Delete")
            .accessibilityAddTraits(.isButton)
    }
}

// MARK: - Surface

struct WBKeyboardSurfaceView: View {
    static let preferredHeight: CGFloat = 252

    @ObservedObject var model: WBKeyboardModel
    @Environment(\.colorScheme) private var scheme

    private var palette: WBKeyPalette { WBKeyPalette(scheme) }

    var body: some View {
        VStack(spacing: 8) {
            stage
                .frame(maxWidth: .infinity, maxHeight: .infinity)
            keyRow
                .frame(height: 44)
        }
        .padding(.horizontal, 6)
        .padding(.top, 8)
        .padding(.bottom, 4)
    }

    // MARK: Stage

    @ViewBuilder
    private var stage: some View {
        if model.phase == .needsFullAccess {
            fullAccessCard
                .transition(.opacity)
        } else {
            VStack(spacing: 12) {
                pill
                caption
            }
            .transition(.opacity)
        }
    }

    private enum PillShape: Equatable {
        case mic, wave, working

        var size: CGSize {
            switch self {
            case .mic: CGSize(width: 76, height: 76)
            case .wave: CGSize(width: 212, height: 56)
            case .working: CGSize(width: 184, height: 56)
            }
        }
    }

    private var pillShape: PillShape {
        switch model.phase {
        case .listening: .wave
        case .transcribing: .working
        default: .mic
        }
    }

    private var pill: some View {
        let shape = pillShape
        return ZStack {
            Capsule(style: .continuous)
                .fill(WBTheme.pill)
                .overlay(Capsule(style: .continuous).strokeBorder(palette.pillStroke, lineWidth: 0.5))
                .shadow(color: .black.opacity(scheme == .dark ? 0 : 0.18), radius: 8, x: 0, y: 3)

            switch shape {
            case .mic:
                Button(action: model.onMicTap) {
                    Image(systemName: "mic.fill")
                        .font(.system(size: 30, weight: .medium))
                        .foregroundStyle(WBTheme.pillInk)
                        .frame(maxWidth: .infinity, maxHeight: .infinity)
                        .contentShape(Capsule())
                }
                .buttonStyle(WBPressStyle())
                .accessibilityLabel(model.phase == .noSession ? "Start Wispr Bro" : "Start dictating")
                .transition(.opacity)

            case .wave:
                HStack(spacing: 0) {
                    disc(symbol: "xmark", fill: WBTheme.pillControl, ink: WBTheme.pillInk, label: "Cancel", action: model.onCancelTap)
                    Spacer(minLength: 0)
                    WBWaveformView(level: model.levelSource, scale: 1.6, frozen: model.frozenWave)
                    Spacer(minLength: 0)
                    disc(symbol: "checkmark", fill: WBTheme.pillInk, ink: WBTheme.pill, label: "Finish and insert", action: model.onFinishTap)
                }
                .padding(.horizontal, 8)
                .transition(.opacity)

            case .working:
                Button(action: model.onCancelTap) {
                    HStack(spacing: 10) {
                        Text("Transcribing…")
                            .font(WBTheme.pillFont)
                            .foregroundStyle(WBTheme.pillInk)
                        WBSpinner(size: 18)
                    }
                    .frame(maxWidth: .infinity, maxHeight: .infinity)
                    .contentShape(Capsule())
                }
                .buttonStyle(WBPressStyle())
                .accessibilityLabel("Transcribing. Tap to cancel.")
                .transition(.opacity)
            }
        }
        .frame(width: shape.size.width, height: shape.size.height)
        .animation(WBTheme.spring, value: shape)
    }

    private func disc(symbol: String, fill: Color, ink: Color, label: String, action: @escaping () -> Void) -> some View {
        Button(action: action) {
            Image(systemName: symbol)
                .font(.system(size: 16, weight: .semibold))
                .foregroundStyle(ink)
                .frame(width: 40, height: 40)
                .background(fill, in: Circle())
                .contentShape(Circle().inset(by: -6))
        }
        .buttonStyle(WBPressStyle())
        .accessibilityLabel(label)
    }

    private var captionText: (title: String, detail: String) {
        switch model.phase {
        case .needsFullAccess: ("", "")
        case .noSession: ("Tap to start", "Opens Wispr Bro for a moment")
        case .idle: ("Tap to talk", "Everything stays on this iPhone")
        case .listening: ("Listening", "Talk, and your words appear")
        case .transcribing: ("Almost there", "Tap to cancel")
        case .error(let message): (message, "Tap to try again")
        }
    }

    private var caption: some View {
        let text = captionText
        return VStack(spacing: 2) {
            Text(text.title)
                .font(.system(size: 15, weight: .semibold))
                .foregroundStyle(palette.ink)
            Text(text.detail)
                .font(.system(size: 13))
                .foregroundStyle(palette.muted)
        }
        .multilineTextAlignment(.center)
        .lineLimit(2)
        .minimumScaleFactor(0.85)
        .padding(.horizontal, 16)
        .animation(WBTheme.quick, value: text.title)
    }

    private var fullAccessCard: some View {
        VStack(alignment: .leading, spacing: 8) {
            Text("Turn on Full Access")
                .font(.system(size: 17, weight: .semibold))
                .foregroundStyle(palette.ink)
            Text("Settings › Wispr Bro › Keyboards › Allow Full Access")
                .font(.system(size: 13, weight: .medium))
                .foregroundStyle(palette.ink)
            Text("The keyboard needs it to reach Wispr Bro's microphone. Everything stays on this iPhone.")
                .font(.system(size: 13))
                .foregroundStyle(palette.muted)
                .fixedSize(horizontal: false, vertical: true)
            Spacer(minLength: 0)
            Button(action: model.onOpenApp) {
                Text("Open Wispr Bro")
                    .font(.system(size: 15, weight: .semibold))
                    .foregroundStyle(WBTheme.pillInk)
                    .padding(.horizontal, 18)
                    .frame(height: 38)
                    .background(WBTheme.pill, in: Capsule())
            }
            .buttonStyle(WBPressStyle())
        }
        .padding(16)
        .frame(maxWidth: .infinity, maxHeight: .infinity, alignment: .leading)
        .background(palette.card, in: RoundedRectangle(cornerRadius: WBTheme.r4, style: .continuous))
        .overlay(
            RoundedRectangle(cornerRadius: WBTheme.r4, style: .continuous)
                .strokeBorder(palette.cardStroke, lineWidth: 1)
        )
        .padding(.horizontal, 2)
    }

    // MARK: Key row

    private var keyRow: some View {
        HStack(spacing: 6) {
            if model.needsGlobeKey {
                WBGlobeKey(
                    controller: model.inputViewController,
                    tint: UIColor(wbHex: scheme == .dark ? 0xF3F2EE : 0x1A1A1A)
                )
                .frame(width: 46)
                .background(palette.key, in: RoundedRectangle(cornerRadius: WBTheme.r2, style: .continuous))
                .shadow(color: palette.keyShadow, radius: 0, x: 0, y: 1)
            }

            Button(action: model.onSpace) {
                Text("space")
                    .font(.system(size: 16))
            }
            .buttonStyle(WBKeyStyle(palette: palette))
            .accessibilityLabel("Space")

            WBDeleteKey(palette: palette, onDelete: model.onDelete)
                .frame(width: 54)

            Button(action: model.onReturn) {
                if let label = model.returnLabel {
                    Text(label).font(.system(size: 16))
                } else {
                    Image(systemName: "return").font(.system(size: 17, weight: .regular))
                }
            }
            .buttonStyle(WBKeyStyle(palette: palette))
            .frame(width: 88)
            .accessibilityLabel(model.returnLabel ?? "Return")
        }
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
