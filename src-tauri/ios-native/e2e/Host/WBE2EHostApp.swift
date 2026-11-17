// SPDX-License-Identifier: GPL-3.0-or-later
// A stand-in for "the app the user is typing in": one text field, nothing
// else. Own bundle id (com.wisprbro.e2e.host) and URL scheme (wbe2ehost://).

import SwiftUI

@main
struct WBE2EHostApp: App {
    @State private var text = ""
    @State private var opened = 0
    @FocusState private var focused: Bool

    var body: some Scene {
        WindowGroup {
            VStack(alignment: .leading, spacing: 12) {
                Text("Notepad")
                    .font(.largeTitle.bold())
                TextEditor(text: $text)
                    .focused($focused)
                    .font(.body)
                    .frame(height: 220)
                    .padding(8)
                    .background(Color(.secondarySystemBackground), in: RoundedRectangle(cornerRadius: 12))
                    .accessibilityIdentifier("host.field")
                HStack {
                    Button("Clear") { text = "" }
                        .accessibilityIdentifier("host.clear")
                    Spacer()
                    Text("opened by link: \(opened)")
                        .font(.footnote)
                        .foregroundStyle(.secondary)
                        .accessibilityIdentifier("host.opened")
                }
                // What a keyboard that can name its host sends (iOS before
                // 26.4): a session link carrying the host's bundle id.
                HStack {
                    Button("Link: Safari") { openSession(host: "com.apple.mobilesafari") }
                        .accessibilityIdentifier("host.link.catalog")
                    Spacer()
                    Button("Link: this app") { openSession(host: "com.wisprbro.e2e.host") }
                        .accessibilityIdentifier("host.link.bundle")
                }
                .font(.footnote)
                Spacer()
            }
            .padding(20)
            .onOpenURL { _ in opened += 1 }
        }
    }

    private func openSession(host: String) {
        var components = URLComponents(string: "wisprbro://session/start")!
        components.queryItems = [
            URLQueryItem(name: "req", value: UUID().uuidString),
            URLQueryItem(name: "ts", value: String(format: "%.3f", Date().timeIntervalSince1970)),
            URLQueryItem(name: "host", value: host),
        ]
        UIApplication.shared.open(components.url!)
    }
}
