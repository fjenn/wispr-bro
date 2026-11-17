// SPDX-License-Identifier: GPL-3.0-or-later
// What a long press on a key offers: accents and related signs, most wanted first.
// Covers Dutch, German, French, Spanish, Portuguese, Italian, the Nordic languages
// and Polish/Czech basics.

import Foundation

enum WBKeyAlternates {
    private static let table: [String: [String]] = [
        "a": ["à", "á", "â", "ä", "æ", "ã", "å", "ā"],
        "c": ["ç", "ć", "č"],
        "e": ["é", "è", "ê", "ë", "ē", "ė", "ę"],
        "g": ["ğ"],
        "i": ["í", "ï", "î", "ì", "ī", "į", "ĳ"],
        "l": ["ł"],
        "n": ["ñ", "ń"],
        "o": ["ó", "ö", "ô", "ò", "œ", "ø", "õ", "ō"],
        "s": ["ß", "ś", "š"],
        "u": ["ü", "ú", "û", "ù", "ū"],
        "y": ["ÿ", "ý"],
        "z": ["ž", "ź", "ż"],
        "0": ["°"],
        "-": ["–", "—", "•"],
        "/": ["\\"],
        "$": ["€", "£", "¥", "¢", "₩"],
        "&": ["§"],
        "\"": ["“", "”", "„", "»", "«"],
        ".": ["…"],
        "?": ["¿"],
        "!": ["¡"],
        "'": ["’", "‘", "`"],
        "%": ["‰"],
        "=": ["≠", "≈"],
        "€": ["$", "£", "¥"],
    ]

    /// The alternates of a key's character, in the case the key is typing in.
    static func options(for character: String, uppercase: Bool) -> [String] {
        guard let options = table[character] else { return [] }
        return uppercase ? options.map(self.uppercase) : options
    }

    /// `uppercased()`, except that ß stays one letter.
    static func uppercase(_ text: String) -> String {
        text == "ß" ? "ẞ" : text.uppercased()
    }
}
