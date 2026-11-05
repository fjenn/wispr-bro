// SPDX-License-Identifier: GPL-3.0-or-later
//! The editor's old placeholders (rc6 and earlier), in every locale.
//!
//! Until rc6 a new transform was created with a generic instruction
//! ("Rewrite the selected text while preserving…") and a separate
//! description field; people typed what they wanted into the description,
//! which the model never saw. [`super::store`] reads such rows as if the
//! description were the instruction, and [`super::literal::plan`] skips these
//! sentences, so those transforms work without being re-created (XF2).

/// The old default instruction of a new transform.
pub const PROMPTS: &[&str] = &[
    "أعد صياغة النص المحدد مع الحفاظ على معناه وحقائقه ولغته.",
    "Пренапиши избрания текст, като запазиш смисъла, фактите и езика му.",
    "Reescriu el text seleccionat conservant-ne el significat, els fets i l'idioma.",
    "Přepiš vybraný text a zachovej jeho význam, fakta a jazyk.",
    "Omskriv den valgte tekst, og bevar dens betydning, fakta og sprog.",
    "Schreibe den ausgewählten Text um und bewahre dabei seine Bedeutung, die Fakten und die Sprache.",
    "Rewrite the selected text while preserving its meaning, facts and language.",
    "Reescribe el texto seleccionado conservando su significado, sus datos y su idioma.",
    "Réécris le texte sélectionné en conservant son sens, ses faits et sa langue.",
    "כתוב מחדש את הטקסט שנבחר ושמור על המשמעות, העובדות והשפה שלו.",
    "चुने गए टेक्स्ट का अर्थ, तथ्य और भाषा बनाए रखते हुए उसे दोबारा लिखें।",
    "Tulis ulang teks yang dipilih dengan tetap mempertahankan makna, fakta, dan bahasanya.",
    "Riscrivi il testo selezionato mantenendone significato, fatti e lingua.",
    "選択したテキストの意味、事実、言語を保ったまま書き換えてください。",
    "선택한 텍스트의 의미, 사실, 언어를 유지하면서 다시 쓰세요.",
    "चयन गरिएको टेक्स्टको अर्थ, तथ्य र भाषा कायम राख्दै फेरि लेख्नुहोस्।",
    "Herschrijf de geselecteerde tekst en behoud de betekenis, feiten en taal.",
    "Przepisz zaznaczony tekst, zachowując jego znaczenie, fakty i język.",
    "Reescreva o texto selecionado preservando o significado, os fatos e o idioma.",
    "Перепиши выделенный текст, сохранив его смысл, факты и язык.",
    "Skriv om den markerade texten och bevara dess betydelse, fakta och språk.",
    "Seçili metni anlamını, bilgilerini ve dilini koruyarak yeniden yaz.",
    "Перепиши виділений текст, зберігши його зміст, факти й мову.",
    "Viết lại văn bản đã chọn, giữ nguyên ý nghĩa, dữ kiện và ngôn ngữ.",
    "改寫所選文字，同時保留其含義、事實和語言。",
    "改写所选文本，同时保留其含义、事实和语言。",
];

/// The old default description of a new transform.
pub const DESCRIPTIONS: &[&str] = &[
    "أعد صياغة النص بالطريقة التي أريدها.",
    "Пренаписва текста така, както искам.",
    "Reescriu el text com vull jo.",
    "Přepíše text tak, jak chci.",
    "Omskriv tekst, som jeg vil have det.",
    "Text so umschreiben, wie ich es möchte.",
    "Rewrite text the way I want.",
    "Reescribir el texto como yo quiero.",
    "Réécrit le texte à ma façon.",
    "כתוב מחדש טקסט כמו שאני רוצה.",
    "टेक्स्ट को मेरे मनचाहे तरीके से दोबारा लिखें।",
    "Tulis ulang teks sesuai keinginan saya.",
    "Riscrive il testo come voglio io.",
    "テキストを思いどおりに書き換えます。",
    "원하는 방식으로 텍스트를 다시 씁니다.",
    "टेक्स्टलाई म चाहेजस्तै फेरि लेख्नुहोस्।",
    "Herschrijf tekst zoals ik dat wil.",
    "Przepisz tekst tak, jak chcę.",
    "Reescreve o texto do jeito que eu quero.",
    "Переписывает текст так, как мне нужно.",
    "Skriv om text som jag vill.",
    "Metni istediğim gibi yeniden yaz.",
    "Переписує текст так, як мені потрібно.",
    "Viết lại văn bản theo cách tôi muốn.",
    "依我想要的方式改寫文字。",
    "按我想要的方式改写文本。",
];

fn norm(text: &str) -> String {
    text.trim()
        .trim_end_matches(['.', '!', '。', '।'])
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .to_lowercase()
}

/// Whether `sentence` is one of the old placeholders and says nothing.
pub fn is_placeholder(sentence: &str) -> bool {
    let sentence = norm(sentence);
    !sentence.is_empty()
        && PROMPTS
            .iter()
            .chain(DESCRIPTIONS)
            .any(|text| norm(text) == sentence)
}

/// `description` without a leading old default description.
pub fn strip_description(description: &str) -> &str {
    let text = description.trim();
    for default in DESCRIPTIONS {
        if let Some(rest) = text.strip_prefix(default) {
            return rest.trim();
        }
    }
    text
}
