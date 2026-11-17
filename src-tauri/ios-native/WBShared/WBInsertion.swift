// SPDX-License-Identifier: GPL-3.0-or-later
// How a finished take is placed at the cursor.

import Foundation

enum WBInsertion {
    /// Characters after which no space is added before the dictated text.
    private static let openers: Set<Character> = ["(", "[", "{", "\"", "“", "‘", "'", "«", "/", "@", "#", "-", "–", "—", "_"]

    /// The text to insert: the take, with a leading space when the text before
    /// the cursor ends in something that is not white space (or an opener).
    static func text(for take: String, before context: String?) -> String {
        let text = take.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !text.isEmpty else { return "" }
        guard let last = context?.last else { return text }
        if last.isWhitespace || last.isNewline || openers.contains(last) { return text }
        if let first = text.first, ".,;:!?)]}".contains(first) { return text }
        return " " + text
    }
}
