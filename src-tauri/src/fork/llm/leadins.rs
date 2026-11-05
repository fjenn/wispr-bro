// SPDX-License-Identifier: GPL-3.0-or-later
//! Sentence lead-ins removed without the LLM (S-12a, Light and Medium).
//!
//! The owner's filler policy (eval/cleanup/README.md, "Discourse markers")
//! removes discourse markers that only start a sentence: "nou dus ik heb …",
//! "alors du coup j'ai …", "bon ben on se voit …". Muesli 2B removes
//! hesitations well but returns such Dutch and French sentences unchanged
//! (S-12a eval), so the words that are lead-ins whenever they open a
//! sentence are stripped here, with the hesitations around them ("Euh, on
//! se voit …"), before the model sees the text; the policy's
//! own rule is "when in doubt for a lead-in at the start of a sentence, treat
//! it as filler".
//!
//! English has only "so" and "basically" here (the model handles the rest;
//! "Well done", "Right now", "Like I said" open real sentences). Not here
//! either: "ja" / "yes" / "oui" / "okay" (answers), "quoi" ("C'est quoi"),
//! and anything mid-sentence. A lead-in is only stripped when real words follow
//! it in the same sentence, and never when the next word makes it part of a
//! phrase ("alors que", "bon appétit", "genre de").

use super::markers::{self, Word};

/// Lead-ins per language, longest phrases first where they share a word.
fn leadins(language: &str) -> &'static [&'static str] {
    match language {
        "en" => &["so", "basically"],
        "nl" => &["nou ja", "nou", "dus", "zeg maar"],
        "fr" => &["du coup", "alors", "bon", "ben", "enfin", "genre"],
        "de" => &["na ja", "naja", "also"],
        "es" => &["o sea", "pues", "bueno"],
        "it" => &["allora", "cioè", "insomma"],
        "pt" => &["então", "tipo"],
        "sv" => &["alltså"],
        _ => &[],
    }
}

/// Words after a lead-in that make it part of a real phrase.
fn blocked_by(leadin: &str, next: &str) -> bool {
    match leadin {
        // "So far", "so that", "so do I", "so-called": not lead-ins.
        "so" => matches!(
            next,
            "far"
                | "that"
                | "much"
                | "many"
                | "long"
                | "what"
                | "am"
                | "do"
                | "did"
                | "does"
                | "is"
                | "was"
                | "are"
                | "were"
                | "have"
                | "has"
                | "had"
                | "can"
                | "could"
                | "will"
                | "would"
                | "should"
                | "called"
                | "too"
                | "to"
                | "be"
                | "it's"
        ),
        "alors" => matches!(next, "que" | "qu"),
        "bon" => matches!(
            next,
            "appétit"
                | "courage"
                | "voyage"
                | "anniversaire"
                | "week"
                | "travail"
                | "retour"
                | "rétablissement"
                | "marché"
                | "sens"
        ),
        "genre" => matches!(next, "de" | "d" | "du" | "des"),
        "enfin" => matches!(next, "de"),
        "bueno" => matches!(next, "es" | "era"),
        "tipo" => matches!(next, "de" | "do" | "da"),
        "dus" => false,
        _ => false,
    }
}

/// Byte offsets where a sentence starts: the text start, after `.!?` and a
/// space, after a line break.
fn sentence_starts(text: &str, words: &[Word]) -> Vec<usize> {
    let mut starts = Vec::new();
    for (i, w) in words.iter().enumerate() {
        let before = &text[if i == 0 { 0 } else { words[i - 1].end }..w.start];
        let opens = i == 0
            || before.contains('\n')
            || before
                .trim()
                .chars()
                .any(|c| matches!(c, '.' | '!' | '?' | '…'));
        if opens {
            starts.push(i);
        }
    }
    starts
}

/// The text without its sentence lead-ins in `language`, and how many were
/// removed. Hesitations right after a lead-in go with it ("nou eh ik …").
pub fn strip(text: &str, language: Option<&str>) -> (String, u32) {
    strip_with(text, language, false)
}

/// [`strip`], and hesitations that open a sentence on their own go too
/// ("Euh, on se voit …"). For the text inserted when the model's cleanup was
/// not used: the model itself removes fillers more readily when it still
/// sees the hesitations (S-12a eval), so the request keeps them.
pub fn strip_all(text: &str, language: Option<&str>) -> (String, u32) {
    strip_with(text, language, true)
}

fn strip_with(text: &str, language: Option<&str>, opening_hesitations: bool) -> (String, u32) {
    let Some(lang) = language else {
        return (text.to_owned(), 0);
    };
    let primary = lang
        .split(['-', '_'])
        .next()
        .unwrap_or(lang)
        .to_ascii_lowercase();
    let list: Vec<Vec<String>> = leadins(&primary)
        .iter()
        .map(|p| markers::words(p).into_iter().map(|w| w.text).collect())
        .collect();
    if list.is_empty() {
        return (text.to_owned(), 0);
    }
    let table = markers::TABLES.for_language(Some(&primary));
    let ws = markers::words(text);
    // (start byte, end byte) of each removal.
    let mut cuts: Vec<(usize, usize)> = Vec::new();
    for s in sentence_starts(text, &ws) {
        let mut i = s;
        let mut removed_any = false;
        loop {
            let hit = list
                .iter()
                .find(|p| markers::matches_at(&ws, i, p))
                .map(|p| (p.len(), p.join(" ")))
                .filter(|(n, p)| ws.get(i + n).is_some_and(|next| !blocked_by(p, &next.text)));
            // A hesitation goes after a lead-in, or opening the sentence when
            // it is only a sound (not also a word: Spanish "este").
            let hes = markers::phrase_at(&ws, i, &table.hesitations).filter(|n| {
                removed_any
                    || opening_hesitations
                        && ws[i..i + n]
                            .iter()
                            .all(|w| !table.stopwords.contains(&w.text))
            });
            match (hit, hes) {
                (Some((n, _)), _) | (None, Some(n)) => {
                    // A real word must follow in the same sentence.
                    let after = i + n;
                    let Some(next) = ws.get(after) else { break };
                    let gap = &text[ws[after - 1].end..next.start];
                    if gap.contains('\n') || gap.contains(['.', '!', '?']) {
                        break;
                    }
                    i = after;
                    removed_any = true;
                }
                (None, None) => break,
            }
        }
        if removed_any {
            cuts.push((ws[s].start, ws[i].start));
        }
    }
    if cuts.is_empty() {
        return (text.to_owned(), 0);
    }
    let mut out = String::with_capacity(text.len());
    let mut last = 0;
    for (start, end) in &cuts {
        out.push_str(&text[last..*start]);
        // The next word now opens the sentence.
        let rest = &text[*end..*end + text[*end..].chars().next().map_or(0, char::len_utf8)];
        out.extend(rest.chars().flat_map(char::to_uppercase));
        last = *end + rest.len();
    }
    out.push_str(&text[last..]);
    let n = u32::try_from(cuts.len()).unwrap_or(u32::MAX);
    (out, n)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn s(text: &str, lang: &str) -> String {
        strip(text, Some(lang)).0
    }

    #[test]
    fn dutch_and_french_lead_ins_go() {
        assert_eq!(
            s("nou dus ik heb de offerte gisteren verstuurd", "nl"),
            "Ik heb de offerte gisteren verstuurd"
        );
        assert_eq!(
            s("nou eh ik weet het niet zeker", "nl"),
            "Ik weet het niet zeker"
        );
        assert_eq!(
            s("alors du coup j'ai envoyé le devis hier", "fr"),
            "J'ai envoyé le devis hier"
        );
        assert_eq!(s("bon ben on se voit lundi", "fr"), "On se voit lundi");
        assert_eq!(
            s("Alors, je voulais te demander un service.", "fr"),
            "Je voulais te demander un service."
        );
        assert_eq!(
            s("also ich glaube wir sind fertig", "de"),
            "Ich glaube wir sind fertig"
        );
        assert_eq!(
            s("so uh do we still need the projector", "en"),
            "Do we still need the projector"
        );
        assert_eq!(s("so the plan is um", "en"), "The plan is um");
        assert_eq!(
            s("Basically, the client wants a refund.", "en"),
            "The client wants a refund."
        );
        // Every sentence and line.
        assert_eq!(
            s("C'est fini. Bon, on y va.\nalors on mange", "fr"),
            "C'est fini. On y va.\nOn mange"
        );
    }

    #[test]
    fn real_phrases_and_other_words_stay() {
        for (t, l) in [
            ("alors que nous partions", "fr"),
            ("Bon appétit à tous.", "fr"),
            ("genre de problème", "fr"),
            ("Ja, dat klopt.", "nl"),
            ("Oui, c'est ça.", "fr"),
            ("C'est quoi le problème ?", "fr"),
            ("ik denk dus dat het werkt", "nl"),
            ("So far so good.", "en"),
            ("So do I.", "en"),
            ("so that we can ship", "en"),
            ("Also, we need milk.", "en"),
            ("Bueno es poco.", "es"),
        ] {
            assert_eq!(s(t, l), t, "{l}: {t}");
        }
        // A hesitation opening a sentence goes; a word that is also one stays.
        assert_eq!(s("Euh, on se voit mardi.", "fr"), "Euh, on se voit mardi.");
        assert_eq!(
            strip_all("Euh, on se voit mardi.", Some("fr")).0,
            "On se voit mardi."
        );
        assert_eq!(
            strip_all("este proyecto va bien", Some("es")).0,
            "este proyecto va bien"
        );
        // A lead-in alone, or ending its sentence, stays.
        assert_eq!(s("bon", "fr"), "bon");
        assert_eq!(s("Bon. On y va.", "fr"), "Bon. On y va.");
        // No language, no stripping.
        assert_eq!(strip("alors on y va", None).0, "alors on y va");
        assert_eq!(strip("nou dus ja", Some("nl")), ("Ja".into(), 1));
    }
}
