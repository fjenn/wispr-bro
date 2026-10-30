// SPDX-License-Identifier: GPL-3.0-or-later
//! Word and character counts of a transcript (DEC-11, W-14).
//!
//! Words are UAX #29 word segments (`unicode-segmentation`) that contain a
//! letter or digit, with two script rules for languages written without
//! spaces:
//!
//! - **Han, Hiragana, Katakana:** one word per character (Wispr's documented
//!   rule; a Japanese or Chinese "word" has no reliable boundary without a
//!   dictionary).
//! - **Thai, Lao, Khmer, Myanmar:** grapheme clusters ÷ 4, rounded to the
//!   nearest whole word, at least one per run (an estimate; R4 leaves ICU4X
//!   dictionary segmentation open until Thai counts matter).
//!
//! Hangul is written with spaces and counts like English. Punctuation, emoji
//! and symbols are not words. The counts are computed once at save time and
//! stored per entry (`entry_meta.word_count`, `char_count`), so the rule can
//! change later without rewriting history.

use unicode_segmentation::UnicodeSegmentation;

/// How a character counts.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Class {
    /// Counted one word per character.
    PerChar,
    /// Counted by grapheme clusters ÷ 4.
    SouthEastAsian,
    /// A letter or digit of a space-separated script.
    Plain,
    /// Punctuation, whitespace, symbols.
    None,
}

fn class(c: char) -> Class {
    let cp = c as u32;
    match cp {
        // Han: CJK Unified Ideographs (+ Ext. A, B..), Compatibility.
        0x3400..=0x4DBF | 0x4E00..=0x9FFF | 0xF900..=0xFAFF | 0x20000..=0x323AF
        // 々 〆 (iteration and closing marks).
        | 0x3005..=0x3007
        // Hiragana, Katakana, Katakana Phonetic Extensions, halfwidth Katakana.
        | 0x3040..=0x309F | 0x30A0..=0x30FF | 0x31F0..=0x31FF | 0xFF66..=0xFF9F => {
            Class::PerChar
        }
        // Thai, Lao, Myanmar (+ Ext. A, B), Khmer (+ Symbols).
        0x0E00..=0x0EFF | 0x1000..=0x109F | 0xA9E0..=0xA9FF | 0xAA60..=0xAA7F
        | 0x1780..=0x17FF | 0x19E0..=0x19FF => Class::SouthEastAsian,
        _ if c.is_alphanumeric() => Class::Plain,
        _ => Class::None,
    }
}

/// Words in `text` (see the module docs).
pub fn word_count(text: &str) -> u32 {
    let mut words: u64 = 0;
    // UAX #29 has no dictionary for Thai and friends, so a run of those
    // letters comes out as many segments; their clusters are summed until
    // something else (a space, punctuation, another script) ends the run.
    let mut sea_run: u64 = 0;
    for segment in text.split_word_bounds() {
        let sea = south_east_asian_clusters(segment);
        if sea > 0 {
            sea_run += sea;
            continue;
        }
        words += run_words(std::mem::take(&mut sea_run));
        let mut plain = false;
        for c in segment.chars() {
            match class(c) {
                Class::PerChar => words += 1,
                Class::Plain => plain = true,
                Class::SouthEastAsian | Class::None => {}
            }
        }
        if plain && !segment.chars().any(|c| class(c) == Class::PerChar) {
            words += 1;
        }
    }
    words += run_words(sea_run);
    u32::try_from(words).unwrap_or(u32::MAX)
}

/// Thai/Lao/Khmer/Myanmar grapheme clusters in `segment`, or 0 when the
/// segment holds anything else.
fn south_east_asian_clusters(segment: &str) -> u64 {
    if !segment.chars().all(|c| class(c) == Class::SouthEastAsian) {
        return 0;
    }
    segment.graphemes(true).count() as u64
}

/// Words for a run of `clusters`: ÷ 4, rounded, at least 1.
fn run_words(clusters: u64) -> u64 {
    if clusters == 0 {
        0
    } else {
        ((clusters + 2) / 4).max(1)
    }
}

/// User-perceived characters (extended grapheme clusters), whitespace
/// included.
pub fn char_count(text: &str) -> u32 {
    u32::try_from(text.graphemes(true).count()).unwrap_or(u32::MAX)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn english() {
        assert_eq!(word_count(""), 0);
        assert_eq!(word_count("   \n\t "), 0);
        assert_eq!(word_count("Hello, world!"), 2);
        assert_eq!(word_count("Don't stop — it's 3.5 times faster."), 6);
        assert_eq!(word_count("Line one\nline two\n\nline three"), 6);
        assert_eq!(word_count("... — !!! 🙂"), 0);
        assert_eq!(word_count("e-mail me at 9am"), 5);
    }

    #[test]
    fn german() {
        assert_eq!(word_count("Größenänderung der Straße"), 3);
        assert_eq!(
            word_count("Können Sie mir bitte die Unterlagen schicken?"),
            7
        );
    }

    #[test]
    fn japanese_counts_one_per_character() {
        // 今日は(3) 良い(2) 天気(2) です(2) ね(1); punctuation is not a word.
        assert_eq!(word_count("今日は良い天気ですね。"), 10);
        // Katakana runs are one UAX #29 segment but still count per character.
        assert_eq!(word_count("コンピューター"), 7);
        // Halfwidth Katakana.
        assert_eq!(word_count("ｺﾝﾋﾟｭｰﾀｰ"), 8);
    }

    #[test]
    fn chinese_counts_one_per_character() {
        assert_eq!(word_count("今天天气很好，我们去公园。"), 11);
        assert_eq!(word_count("你好"), 2);
    }

    #[test]
    fn thai_counts_clusters_divided_by_four() {
        // สวัสดีครับ: ส วั ส ดี ค รั บ = 7 clusters → round(7 / 4) = 2.
        assert_eq!(word_count("สวัสดีครับ"), 2);
        // Two runs separated by a space: each is at least one word.
        assert_eq!(word_count("ดี ดี"), 2);
        // ผมชอบกินข้าวผัดมาก ("I like fried rice a lot", 6 real words): 15
        // clusters → 4.
        assert_eq!(word_count("ผมชอบกินข้าวผัดมาก"), 4);
    }

    #[test]
    fn korean_counts_like_english() {
        assert_eq!(word_count("안녕하세요 반갑습니다"), 2);
    }

    #[test]
    fn mixed_scripts() {
        // "I" "love" (2) + 東京(2) + "and" (1) + ラーメン(4) + "!".
        assert_eq!(word_count("I love 東京 and ラーメン!"), 9);
        // Japanese with an English product name glued on.
        assert_eq!(word_count("Wispr Broは便利"), 5);
        // Thai and English.
        assert_eq!(word_count("Meeting สวัสดีครับ today"), 4);
    }

    #[test]
    fn char_counts_graphemes() {
        assert_eq!(char_count(""), 0);
        assert_eq!(char_count("abc def"), 7);
        assert_eq!(char_count("é"), 1);
        assert_eq!(char_count("e\u{301}"), 1);
        assert_eq!(char_count("👍🏽"), 1);
        assert_eq!(char_count("สวัสดี"), 4);
    }
}
