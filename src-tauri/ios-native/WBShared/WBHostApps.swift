// SPDX-License-Identifier: GPL-3.0-or-later
// Apps Wispr Bro can send the user back to after a session start.
//
// Ported from Hex (MIT, (c) 2026 Christopher Langton): HostAppCatalog in
// ios/HEXShared/KeyboardBridge.swift. See ../NOTICE-hex.md. Added: bundle ids,
// so a detected host can be matched to its public URL scheme.

import Foundation

struct WBHostApp: Identifiable, Equatable, Sendable {
    let id: String
    let name: String
    let schemeURL: String
    let bundleIDs: [String]

    var url: URL? { URL(string: schemeURL) }
}

enum WBHostAppCatalog {
    static let all: [WBHostApp] = [
        WBHostApp(id: "messages", name: "Messages", schemeURL: "messages://", bundleIDs: ["com.apple.MobileSMS"]),
        WBHostApp(id: "notes", name: "Notes", schemeURL: "mobilenotes://", bundleIDs: ["com.apple.mobilenotes"]),
        WBHostApp(id: "mail", name: "Mail", schemeURL: "message://", bundleIDs: ["com.apple.mobilemail"]),
        WBHostApp(id: "safari", name: "Safari", schemeURL: "x-web-search://", bundleIDs: ["com.apple.mobilesafari"]),
        WBHostApp(id: "reminders", name: "Reminders", schemeURL: "x-apple-reminderkit://", bundleIDs: ["com.apple.reminders"]),
        WBHostApp(id: "whatsapp", name: "WhatsApp", schemeURL: "whatsapp://", bundleIDs: ["net.whatsapp.WhatsApp"]),
        WBHostApp(id: "telegram", name: "Telegram", schemeURL: "tg://", bundleIDs: ["ph.telegra.Telegraph"]),
        WBHostApp(id: "signal", name: "Signal", schemeURL: "sgnl://", bundleIDs: ["org.whispersystems.signal"]),
        WBHostApp(id: "slack", name: "Slack", schemeURL: "slack://", bundleIDs: ["com.tinyspeck.chatlyio"]),
        WBHostApp(id: "discord", name: "Discord", schemeURL: "discord://", bundleIDs: ["com.hammerandchisel.discord"]),
        WBHostApp(id: "claude", name: "Claude", schemeURL: "claude://", bundleIDs: ["com.anthropic.claude"]),
        WBHostApp(id: "chatgpt", name: "ChatGPT", schemeURL: "chatgpt://", bundleIDs: ["com.openai.chat"]),
        WBHostApp(id: "gmail", name: "Gmail", schemeURL: "googlegmail://", bundleIDs: ["com.google.Gmail"]),
        WBHostApp(id: "obsidian", name: "Obsidian", schemeURL: "obsidian://", bundleIDs: ["md.obsidian"]),
        WBHostApp(id: "linear", name: "Linear", schemeURL: "linear://", bundleIDs: ["com.linear"]),
        WBHostApp(id: "notion", name: "Notion", schemeURL: "notion://", bundleIDs: ["notion.id"]),
        WBHostApp(id: "x", name: "X", schemeURL: "twitter://", bundleIDs: ["com.atebits.Tweetie2"]),
        WBHostApp(id: "instagram", name: "Instagram", schemeURL: "instagram://", bundleIDs: ["com.burbn.instagram"]),
    ]

    static func app(id: String?) -> WBHostApp? {
        guard let id, !id.isEmpty else { return nil }
        return all.first { $0.id == id }
    }

    static func app(bundleID: String?) -> WBHostApp? {
        guard let bundleID, !bundleID.isEmpty else { return nil }
        return all.first { $0.bundleIDs.contains(bundleID) }
    }
}
