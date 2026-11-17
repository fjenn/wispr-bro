// SPDX-License-Identifier: GPL-3.0-or-later
// A tiny app to run the native parts on their own: a text field for the
// keyboard, session buttons, and still previews of every keyboard state
// (`--wb-preview <state> [--wb-dark]`).

import SwiftUI
import UIKit

@main
struct WBHarnessApp: App {
    private let arguments = ProcessInfo.processInfo.arguments

    private var preview: String? {
        guard let index = arguments.firstIndex(of: "--wb-preview"), index + 1 < arguments.count else { return nil }
        return arguments[index + 1]
    }

    private var forcedScheme: ColorScheme? {
        if arguments.contains("--wb-dark") { return .dark }
        if arguments.contains("--wb-light") { return .light }
        return nil
    }

    var body: some Scene {
        WindowGroup {
            Group {
                if preview == "hint" {
                    WBSwipeBackHintView(frozenPhase: 0.55)
                } else if let preview {
                    WBKeyboardPreviewStage(state: preview)
                } else {
                    WBHarnessHome()
                }
            }
            .preferredColorScheme(preview == "hint" ? .dark : forcedScheme)
            .onOpenURL { url in
                WBSessionController.shared.handleOpen(url: url)
            }
        }
    }
}

// MARK: - Home

private struct WBHarnessHome: View {
    @State private var text = ""
    @State private var json: [String: Any] = [:]
    @FocusState private var focused: Bool
    private let timer = Timer.publish(every: 0.5, on: .main, in: .common).autoconnect()

    private var state: String { json["state"] as? String ?? "noSession" }
    private var active: Bool { json["active"] as? Bool ?? false }

    var body: some View {
        ScrollView {
            VStack(alignment: .leading, spacing: 20) {
                HStack(spacing: 12) {
                    WBBrandMark()
                        .fill(WBTheme.ink)
                        .frame(width: 30, height: 30)
                    Text("Wispr Bro")
                        .font(.system(size: 28, weight: .semibold))
                        .foregroundStyle(WBTheme.ink)
                }
                .padding(.top, 12)

                Text("Talk, and your words appear. Everything stays on this iPhone.")
                    .font(.system(size: 17))
                    .foregroundStyle(WBTheme.ink.opacity(0.62))

                card {
                    Text("Try the keyboard")
                        .font(.system(size: 13, weight: .semibold))
                        .foregroundStyle(WBTheme.ink.opacity(0.62))
                    TextField("Tap here, then pick Wispr Bro with the globe key", text: $text, axis: .vertical)
                        .lineLimit(3...8)
                        .focused($focused)
                        .font(.system(size: 17))
                        .accessibilityIdentifier("wb.try.field")
                }

                card {
                    row("Session", value: label(for: state), on: active)
                    Divider()
                    row("Keyboard added", value: (json["keyboardEnabled"] as? Bool ?? false) ? "Yes" : "Not yet", on: json["keyboardEnabled"] as? Bool ?? false)
                    Divider()
                    row("Full Access", value: fullAccess, on: (json["fullAccess"] as? Int ?? -1) == 1)
                    Divider()
                    row("Microphone", value: (json["mic"] as? String ?? "undetermined").capitalized, on: json["mic"] as? String == "granted")
                }

                HStack(spacing: 10) {
                    Button(active ? "End session" : "Start session") {
                        if active {
                            WBSessionController.shared.endSession()
                        } else {
                            WBSessionController.shared.startSession()
                        }
                    }
                    .buttonStyle(WBHarnessButton(primary: true))
                    .accessibilityIdentifier("wb.session.toggle")

                    Button("Settings") { wb_ios_open_settings() }
                        .buttonStyle(WBHarnessButton(primary: false))
                }

                if let message = json["message"] as? String {
                    Text(message)
                        .font(.system(size: 15))
                        .foregroundStyle(WBTheme.accent)
                }
            }
            .padding(.horizontal, 20)
            .padding(.bottom, 40)
        }
        .scrollDismissesKeyboard(.interactively)
        .background(WBTheme.chrome.ignoresSafeArea())
        .onReceive(timer) { _ in refresh() }
        .onAppear {
            refresh()
            if ProcessInfo.processInfo.arguments.contains("--wb-focus") { focused = true }
        }
    }

    private var fullAccess: String {
        switch json["fullAccess"] as? Int ?? -1 {
        case 1: "On"
        case 0: "Off"
        default: "Not seen yet"
        }
    }

    private func label(for state: String) -> String {
        switch state {
        case "idle": "Ready"
        case "listening": "Listening"
        case "transcribing": "Transcribing…"
        case "error": "Needs attention"
        default: "Off"
        }
    }

    private func refresh() {
        guard let data = WBSystemState.stateJSON().data(using: .utf8),
              let object = try? JSONSerialization.jsonObject(with: data) as? [String: Any] else { return }
        json = object
    }

    private func card<Content: View>(@ViewBuilder _ content: () -> Content) -> some View {
        VStack(alignment: .leading, spacing: 12, content: content)
            .padding(16)
            .frame(maxWidth: .infinity, alignment: .leading)
            .background(WBTheme.panel, in: RoundedRectangle(cornerRadius: WBTheme.r4, style: .continuous))
            .overlay(
                RoundedRectangle(cornerRadius: WBTheme.r4, style: .continuous)
                    .strokeBorder(WBTheme.ink.opacity(0.08), lineWidth: 1)
            )
    }

    private func row(_ title: String, value: String, on: Bool) -> some View {
        HStack {
            Text(title)
                .font(.system(size: 16))
                .foregroundStyle(WBTheme.ink)
            Spacer()
            Text(value)
                .font(.system(size: 16, weight: .medium))
                .foregroundStyle(on ? WBTheme.accent : WBTheme.ink.opacity(0.5))
        }
    }
}

private struct WBHarnessButton: ButtonStyle {
    let primary: Bool

    func makeBody(configuration: Configuration) -> some View {
        configuration.label
            .font(.system(size: 16, weight: .semibold))
            .foregroundStyle(primary ? WBTheme.panel : WBTheme.ink)
            .padding(.horizontal, 20)
            .frame(height: 46)
            .background(primary ? WBTheme.ink : WBTheme.ink.opacity(0.08), in: Capsule())
            .opacity(configuration.isPressed ? 0.75 : 1)
    }
}

// MARK: - Keyboard preview stage

/// The keyboard surface on a system-keyboard-like backdrop, under a mock
/// message field, frozen in one state. After Hex's KeyboardPreviewHost.
struct WBKeyboardPreviewStage: View {
    let state: String
    @StateObject private var model = WBKeyboardModel()
    @Environment(\.colorScheme) private var scheme

    var body: some View {
        VStack(spacing: 0) {
            VStack(alignment: .leading, spacing: 10) {
                Text("Keyboard preview · \(state)")
                    .font(.system(size: 13, weight: .semibold))
                    .foregroundStyle(WBTheme.ink.opacity(0.5))
                Text("Meet you at the station at six. I'll bring the tickets.")
                    .font(.system(size: 17))
                    .foregroundStyle(WBTheme.ink)
                    .padding(12)
                    .frame(maxWidth: .infinity, alignment: .leading)
                    .background(WBTheme.panel, in: RoundedRectangle(cornerRadius: WBTheme.r4, style: .continuous))
            }
            .padding(20)
            Spacer(minLength: 0)
            WBKeyboardSurfaceView(model: model)
                .frame(height: WBKeyboardSurfaceView.preferredHeight)
                .padding(.bottom, 34)
                .background(
                    (scheme == .dark ? Color(wbHex: 0x2C2C2E) : Color(wbHex: 0xD6D8DE))
                        .ignoresSafeArea(edges: .bottom)
                )
        }
        .background(WBTheme.chrome.ignoresSafeArea())
        .onAppear(perform: configure)
    }

    private func configure() {
        model.needsGlobeKey = true
        switch state {
        case "fullaccess": model.phase = .needsFullAccess
        case "nosession": model.phase = .noSession
        case "idle": model.phase = .idle
        case "listening-quiet":
            model.phase = .listening
            model.frozenWave = [Double](repeating: 0, count: 11)
        case "listening":
            model.phase = .listening
            model.frozenWave = [0.18, 0.34, 0.5, 0.74, 0.9, 1.0, 0.82, 0.66, 0.42, 0.3, 0.14]
        case "transcribing": model.phase = .transcribing
        case "error": model.phase = .error("Microphone access is off")
        case "send":
            model.phase = .idle
            model.returnLabel = "Send"
            model.needsGlobeKey = false
        default: model.phase = .idle
        }
    }
}
