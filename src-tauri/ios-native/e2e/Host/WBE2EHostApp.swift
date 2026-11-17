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
                Spacer()
            }
            .padding(20)
            .onOpenURL { _ in opened += 1 }
        }
    }
}
