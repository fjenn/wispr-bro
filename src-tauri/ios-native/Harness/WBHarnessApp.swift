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
    @State private var other = ""
    @FocusState private var otherFocused: Bool
    /// `--wb-field search|email|url|dark`: a second, single-line field of that kind.
    private var fieldKind: String? {
        let arguments = ProcessInfo.processInfo.arguments
        guard let index = arguments.firstIndex(of: "--wb-field"), index + 1 < arguments.count else { return nil }
        return arguments[index + 1]
    }
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
                    if let fieldKind {
                        Divider()
                        TextField("A \(fieldKind) field", text: $other)
                            .keyboardType(fieldKind == "email" ? .emailAddress : fieldKind == "url" ? .URL : .default)
                            .submitLabel(fieldKind == "search" ? .search : fieldKind == "email" ? .send : fieldKind == "url" ? .go : .return)
                            .textInputAutocapitalization(fieldKind == "search" || fieldKind == "dark" ? .sentences : .never)
                            .focused($otherFocused)
                            .font(.system(size: 17))
                            .accessibilityIdentifier("wb.other.field")
                    }
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
            if ProcessInfo.processInfo.arguments.contains("--wb-focus-other") { otherFocused = true }
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

/// A text field that is only a string: previews and tests type into it.
@MainActor
final class WBFakeTextTarget: WBTextTarget {
    private(set) var text: String
    var keyboardAppearance: UIKeyboardAppearance?
    var autocapitalizationType: UITextAutocapitalizationType? = .sentences
    var keyboardType: UIKeyboardType? = .default
    var returnKeyType: UIReturnKeyType? = .default
    var enablesReturnKeyAutomatically: Bool? = false

    init(text: String = "", appearance: UIKeyboardAppearance? = nil) {
        self.text = text
        keyboardAppearance = appearance
    }

    var documentContextBeforeInput: String? { text }
    var hasText: Bool { !text.isEmpty }
    func insertText(_ inserted: String) { text += inserted }
    func deleteBackward() { if !text.isEmpty { text.removeLast() } }
}

/// The real keyboard view in the app, typing into a `WBFakeTextTarget`.
private struct WBKeyboardPreview: UIViewRepresentable {
    let model: WBKeyboardModel
    let dark: Bool
    let configure: (WBFullKeyboardView, WBFakeTextTarget) -> Void

    func makeUIView(context: Context) -> WBFullKeyboardView {
        let target = WBFakeTextTarget(text: "Meet you at the station at six. ")
        let view = WBFullKeyboardView(model: model, target: target)
        view.appearanceOverride = dark
        view.drawsBackdrop = true
        configure(view, target)
        view.prepareForField()
        return view
    }

    func updateUIView(_ uiView: WBFullKeyboardView, context: Context) {
        uiView.appearanceOverride = dark
    }

}

/// The keyboard under a mock message field, frozen in one state
/// (`--wb-preview <state>`). After Hex's KeyboardPreviewHost.
struct WBKeyboardPreviewStage: View {
    let state: String
    @StateObject private var model = WBKeyboardModel()
    @Environment(\.colorScheme) private var scheme
    @Environment(\.verticalSizeClass) private var vertical

    var body: some View {
        let landscape = vertical == .compact
        VStack(spacing: 0) {
            if !landscape {
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
            }
            Spacer(minLength: 0)
            preview
                .frame(height: WBKeyLayout.height(for: landscape ? .landscape : .portrait))
                .padding(.bottom, landscape ? 0 : 34)
                .background(
                    Color(WBKeyPalette(isDark: scheme == .dark).backdrop)
                        .ignoresSafeArea(edges: [.bottom, .horizontal])
                )
        }
        .background(WBTheme.chrome.ignoresSafeArea())
        .onAppear(perform: configure)
    }

    private var preview: some View {
        let state = state
        return WBKeyboardPreview(model: model, dark: scheme == .dark) { _, target in
            switch state {
            case "send": target.returnKeyType = .send
            case "search":
                target.returnKeyType = .search
                target.keyboardType = .webSearch
            case "email": target.keyboardType = .emailAddress
            case "url":
                target.keyboardType = .URL
                target.returnKeyType = .go
            default: break
            }
        }
        .staged { view in
            switch state {
            case "popup": view.debugPress("t")
            case "accent": view.debugPress("e", alternates: true)
            case "accent-slide": view.debugPress("o", alternates: true, slideTo: 3)
            case "numbers": view.debugShow(.numbers)
            case "symbols": view.debugShow(.symbols)
            case "caps":
                view.debugTap(.shift)
                view.debugTap(.shift)
            default: break
            }
        }
    }

    private func configure() {
        model.needsGlobeKey = state == "globe"
        switch state {
        case "fullaccess": model.phase = .needsFullAccess
        case "nosession": model.phase = .noSession
        case "talkhint":
            model.phase = .idle
            model.showsTalkHint = true
        case "listening-quiet":
            model.phase = .listening
            model.frozenWave = [Double](repeating: 0, count: 11)
        case "listening":
            model.phase = .listening
            model.frozenWave = [0.18, 0.34, 0.5, 0.74, 0.9, 1.0, 0.82, 0.66, 0.42, 0.3, 0.14]
        case "transcribing": model.phase = .transcribing
        case "error": model.phase = .error("Microphone access is off")
        default: model.phase = .idle
        }
    }
}

private extension WBKeyboardPreview {
    /// Runs once the view is on screen and laid out.
    func staged(_ stage: @escaping (WBFullKeyboardView) -> Void) -> WBKeyboardPreview {
        WBKeyboardPreview(model: model, dark: dark) { view, target in
            configure(view, target)
            DispatchQueue.main.asyncAfter(deadline: .now() + 0.4) { stage(view) }
        }
    }
}
