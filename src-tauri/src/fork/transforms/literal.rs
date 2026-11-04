// SPDX-License-Identifier: GPL-3.0-or-later
//! Exact edits (OT2-D): replace, remove and case changes a transform asks
//! for, applied by code instead of the model. A small local model cannot be
//! trusted with character-level work ("replace every a with 0"), and the
//! guards rightly reject the new digits it would add, so these never reach it.
//!
//! Two sources, both language-independent in what they do:
//! - the editor's "Find and replace" rows ([`LiteralRule`]);
//! - sentences of the custom prompt that are wholly an exact edit, recognised
//!   in English, German, Spanish, French, Portuguese, Italian and Dutch, plus
//!   the symbolic `a → 0` form in any language.
//!
//! A sentence that is anything more ("make it formal and replace X with Y")
//! stays an instruction for the model; only whole sentences are taken over.

use once_cell::sync::Lazy;
use regex::Regex;
use serde::{Deserialize, Serialize};
use specta::Type;

/// One editor "Find and replace" row.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(default)]
pub struct LiteralRule {
    pub find: String,
    /// Empty removes what was found.
    pub replace: String,
    /// Off: letters and words match in any case, and the replacement follows
    /// the case of what it replaces.
    pub match_case: bool,
}

pub const MAX_RULES: usize = 20;
pub const MAX_RULE_CHARS: usize = 200;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename = "LiteralCase", rename_all = "snake_case")]
pub enum CaseMode {
    Upper,
    Lower,
    Title,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename = "LiteralCategory", rename_all = "snake_case")]
pub enum Category {
    Emoji,
    Email,
    Url,
    /// Every vowel letter, accented ones included (a, e, i, o, u, é, ü …).
    Vowels,
    /// Every other Latin letter (b, c, ß, ñ, ç …).
    Consonants,
}

impl Category {
    /// Vowels or consonants.
    pub fn is_letters(self) -> bool {
        matches!(self, Category::Vowels | Category::Consonants)
    }

    fn has(self, c: char) -> bool {
        match self {
            Category::Vowels => is_vowel(c),
            Category::Consonants => is_latin_letter(c) && !is_vowel(c),
            _ => false,
        }
    }
}

/// The vowels of the supported languages: a e i o u with their accented
/// forms (é, ü, ã, ô …), plus æ, ø and œ. "y" is not one.
fn is_vowel(c: char) -> bool {
    let mut lower = c.to_lowercase();
    let (Some(c), None) = (lower.next(), lower.next()) else {
        return false;
    };
    "aeiouàáâãäåāăąèéêëēĕėęěìíîïĩīĭįòóôõöøōŏőùúûüũūŭůűųæœ".contains(c)
}

/// A letter of the Latin script (Basic Latin to Latin Extended-B).
fn is_latin_letter(c: char) -> bool {
    c.is_alphabetic() && (c.is_ascii_alphabetic() || ('\u{C0}'..='\u{24F}').contains(&c))
}

/// What to look for.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(
    rename = "LiteralTarget",
    tag = "kind",
    content = "value",
    rename_all = "snake_case"
)]
pub enum Target {
    Text(String),
    Category(Category),
}

/// One exact edit, as the editor shows it.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename = "LiteralEdit", tag = "kind", rename_all = "snake_case")]
pub enum Edit {
    Replace {
        find: Target,
        with: String,
        match_case: bool,
    },
    Case {
        mode: CaseMode,
    },
}

/// A prompt split into exact edits and what is left for the model.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Plan {
    pub edits: Vec<Edit>,
    /// The prompt's other sentences, in order (empty when none).
    pub rest: String,
    /// Letters a sentence asked to replace with several replacements without
    /// saying which becomes which ("replace vowels with 1 and 0"). That
    /// sentence stays in `rest` for the model; these let the run check that
    /// the model did change them.
    pub unclear: Vec<Target>,
}

// --- Language tables -------------------------------------------------------

struct Lang {
    replace_verbs: &'static [&'static str],
    connectors: &'static [&'static str],
    remove_verbs: &'static [&'static str],
    and: &'static [&'static str],
    /// Quantifiers, articles and "the word(s)" style lead-ins, any order.
    fillers: &'static [&'static str],
    /// Trailing words such as "everywhere" or "respectively".
    trailers: &'static [&'static str],
    upper: &'static str,
    lower: &'static str,
    title: &'static str,
    emoji: &'static str,
    email: &'static str,
    url: &'static str,
    vowels: &'static str,
    consonants: &'static str,
    /// Punctuation named in words.
    marks: &'static [(&'static str, &'static str)],
    /// Words that point at the text itself or name a kind of thing; a bare
    /// one is never a literal find ("change it to Spanish", "replace names").
    vague: &'static [&'static str],
}

const EN: Lang = Lang {
    replace_verbs: &[
        "replace",
        "change",
        "swap",
        "substitute",
        "switch",
        "turn",
        "convert",
    ],
    connectors: &["with", "to", "into", "for", "by"],
    remove_verbs: &[
        "remove",
        "delete",
        "strip",
        "drop",
        "get rid of",
        "take out",
        "cut",
        "erase",
    ],
    and: &["and", "plus"],
    fillers: &[
        "all",
        "every",
        "each",
        "any",
        "the",
        "of",
        "occurrences",
        "occurrence",
        "instances",
        "instance",
        "word",
        "words",
        "letter",
        "letters",
        "character",
        "characters",
        "string",
        "strings",
        "single",
        "capital",
        "lowercase",
    ],
    trailers: &[
        "everywhere",
        "throughout",
        "respectively",
        "in the text",
        "in it",
        "please",
        "from the text",
        "wherever they appear",
        "wherever it appears",
        "each time",
    ],
    upper: r"^(?:(?:make|convert|change|turn|put|write|set|transform|format)\s+)?(?:(?:it|this|that|everything|all|all of it|all text|all letters|the text|the whole text|the selection|every letter|the letters)\s+)?(?:(?:to|into|in|as)\s+)?(?:all\s+)?(?:upper\s*-?\s*case|all\s+caps|caps(?:\s+lock)?|capital\s+letters|capitals|block\s+capitals)(?:\s+(?:letters|everything|only|please))?$|^(?:uppercase|upper-case|capitali[sz]e)\s+(?:everything|all|it|this|all letters|every letter|the (?:whole )?text)$|^shout(?: it)?$",
    lower: r"^(?:(?:make|convert|change|turn|put|write|set|transform|format)\s+)?(?:(?:it|this|that|everything|all|all of it|all text|all letters|the text|the whole text|the selection|every letter|the letters)\s+)?(?:(?:to|into|in|as)\s+)?(?:all\s+)?(?:lower\s*-?\s*case|small\s+letters|no\s+capitals)(?:\s+(?:letters|everything|only|please))?$|^(?:lowercase|lower-case|decapitali[sz]e)\s+(?:everything|all|it|this|all letters|every letter|the (?:whole )?text)$",
    title: r"^(?:(?:make|convert|change|turn|put|write|set|use)\s+)?(?:(?:it|this|everything|the text)\s+)?(?:(?:to|into|in|as)\s+)?title\s*-?\s*case$|^capitali[sz]e\s+(?:the\s+)?(?:first\s+letter\s+of\s+)?(?:every|each|all|all the)\s+words?$|^start\s+(?:every|each)\s+word\s+with\s+a\s+capital(?:\s+letter)?$",
    emoji: r"^(?:emojis?|emoticons?|emoji\s+characters|smileys?)$",
    email: r"^(?:e-?mail(?:\s+address(?:es)?)?s?|e-?mail\s+addresses)$",
    url: r"^(?:urls?|links?|web\s*links?|web\s+addresses|websites?)$",
    vowels: r"^vowels?$",
    consonants: r"^consonants?$",
    marks: &[
        ("exclamation marks", "!"),
        ("exclamation mark", "!"),
        ("exclamation points", "!"),
        ("exclamation point", "!"),
        ("exclamations", "!"),
        ("question marks", "?"),
        ("question mark", "?"),
        ("commas", ","),
        ("comma", ","),
        ("semicolons", ";"),
        ("semicolon", ";"),
        ("colons", ":"),
        ("colon", ":"),
        ("hashtags", "#"),
        ("hash signs", "#"),
        ("asterisks", "*"),
        ("asterisk", "*"),
        ("ampersands", "&"),
        ("quotation marks", "\""),
        ("double quotes", "\""),
        ("quotes", "\""),
        ("em dashes", "—"),
        ("em dash", "—"),
        ("ellipses", "…"),
    ],
    vague: &[
        "it",
        "this",
        "that",
        "them",
        "these",
        "those",
        "everything",
        "text",
        "all",
        "tone",
        "style",
        "name",
        "names",
        "email",
        "emails",
        "number",
        "numbers",
        "word",
        "words",
        "date",
        "dates",
        "url",
        "urls",
        "link",
        "links",
        "phone",
        "sentence",
        "sentences",
        "letter",
        "letters",
        "character",
        "characters",
        "emoji",
        "emojis",
        "jargon",
        "filler",
        "fillers",
        "typo",
        "typos",
        "mistake",
        "mistakes",
        "error",
        "errors",
        "language",
        "spanish",
        "english",
        "german",
        "french",
        "formal",
        "casual",
        "list",
        "bullets",
        "paragraph",
        "paragraphs",
        "slang",
        "swear",
        "swearing",
        "curse",
        "curses",
        "profanity",
        "abbreviations",
        "acronyms",
        "contractions",
        "passive",
        "voice",
        "spelling",
        "grammar",
        "punctuation",
        "me",
        "you",
        "us",
        "space",
        "spaces",
        "dash",
        "dashes",
        "hyphen",
        "hyphens",
        "period",
        "periods",
        "dot",
        "dots",
        "digit",
        "digits",
        "capitals",
        "line",
        "lines",
        "bullet",
        "point",
        "points",
        "symbols",
        "symbol",
        "marks",
        "quotes",
    ],
};

const DE: Lang = Lang {
    replace_verbs: &[
        "ersetze",
        "ersetzen",
        "ersetz",
        "ändere",
        "ändern",
        "tausche",
        "tauschen",
        "wandle",
        "mache aus",
    ],
    connectors: &["durch", "mit", "zu", "in", "gegen"],
    remove_verbs: &[
        "entferne",
        "entfernen",
        "lösche",
        "löschen",
        "streiche",
        "streichen",
    ],
    and: &["und"],
    fillers: &[
        "alle",
        "jedes",
        "jeden",
        "jede",
        "die",
        "das",
        "den",
        "der",
        "buchstaben",
        "wort",
        "wörter",
        "zeichen",
        "vorkommen",
    ],
    trailers: &["überall", "jeweils", "im text", "bitte"],
    upper: r"^(?:(?:alles|den text|text)\s+)?(?:in\s+)?(?:großbuchstaben|grossbuchstaben|versalien)(?:\s+(?:schreiben|umwandeln|setzen))?$|^(?:schreibe?|wandle|setze|mache?)\s+(?:alles|den text|es)\s+(?:in\s+)?(?:großbuchstaben|grossbuchstaben|versalien)(?:\s+um)?$",
    lower: r"^(?:(?:alles|den text|text)\s+)?(?:in\s+)?kleinbuchstaben(?:\s+(?:schreiben|umwandeln|setzen))?$|^(?:schreibe?|wandle|setze|mache?)\s+(?:alles|den text|es)\s+(?:in\s+)?kleinbuchstaben(?:\s+um)?$",
    title: r"^(?:jedes|jeden)\s+wort\s+großschreiben$",
    emoji: r"^(?:emojis?|smileys?)$",
    email: r"^(?:e-?mail-?adressen|e-?mail-?adresse|e-?mails)$",
    url: r"^(?:urls?|links?|webadressen)$",
    vowels: r"^(?:vokale?|selbstlaute?)$",
    consonants: r"^(?:konsonanten?|mitlaute?)$",
    marks: &[
        ("ausrufezeichen", "!"),
        ("fragezeichen", "?"),
        ("kommas", ","),
        ("kommata", ","),
        ("komma", ","),
        ("semikolons", ";"),
        ("anführungszeichen", "\""),
    ],
    vague: &[
        "es", "das", "alles", "text", "namen", "name", "zahlen", "wörter", "ton", "stil",
        "deutsch", "englisch", "fehler", "sprache",
    ],
};

const ES: Lang = Lang {
    replace_verbs: &[
        "reemplaza",
        "reemplazar",
        "sustituye",
        "sustituir",
        "cambia",
        "cambiar",
        "convierte",
    ],
    connectors: &["por", "con", "a", "en"],
    remove_verbs: &[
        "elimina", "eliminar", "quita", "quitar", "borra", "borrar", "suprime",
    ],
    and: &["y", "e"],
    fillers: &[
        "todas",
        "todos",
        "cada",
        "las",
        "los",
        "la",
        "el",
        "letra",
        "letras",
        "palabra",
        "palabras",
        "caracteres",
        "carácter",
        "de",
    ],
    trailers: &["en todo el texto", "respectivamente", "por favor"],
    upper: r"^(?:(?:pon|escribe|convierte|pasa)(?:lo)?\s+)?(?:(?:todo|el texto)\s+)?(?:en|a)\s+mayúsculas$|^(?:todo\s+)?en\s+mayúsculas$",
    lower: r"^(?:(?:pon|escribe|convierte|pasa)(?:lo)?\s+)?(?:(?:todo|el texto)\s+)?(?:en|a)\s+minúsculas$|^(?:todo\s+)?en\s+minúsculas$",
    title: r"^(?:pon\s+)?(?:la\s+)?primera\s+letra\s+de\s+cada\s+palabra\s+en\s+mayúscula$",
    emoji: r"^(?:emojis?|emoticonos?)$",
    email: r"^(?:correos(?:\s+electrónicos)?|direcciones\s+de\s+correo(?:\s+electrónico)?|e-?mails?)$",
    url: r"^(?:urls?|enlaces|links?)$",
    vowels: r"^vocal(?:es)?$",
    consonants: r"^consonantes?$",
    marks: &[
        ("signos de exclamación", "!"),
        ("exclamaciones", "!"),
        ("comas", ","),
        ("signos de interrogación", "?"),
    ],
    vague: &[
        "lo", "esto", "eso", "todo", "texto", "nombres", "números", "palabras", "tono", "estilo",
        "español", "inglés", "errores", "idioma",
    ],
};

const FR: Lang = Lang {
    replace_verbs: &[
        "remplace",
        "remplacer",
        "change",
        "changer",
        "substitue",
        "transforme",
    ],
    connectors: &["par", "en", "avec"],
    remove_verbs: &[
        "supprime",
        "supprimer",
        "enlève",
        "enlever",
        "retire",
        "retirer",
        "efface",
    ],
    and: &["et"],
    fillers: &[
        "tous",
        "toutes",
        "chaque",
        "les",
        "le",
        "la",
        "l'",
        "lettre",
        "lettres",
        "mot",
        "mots",
        "caractères",
        "des",
        "de",
    ],
    trailers: &[
        "partout",
        "respectivement",
        "dans le texte",
        "s'il te plaît",
        "s'il vous plaît",
    ],
    upper: r"^(?:(?:mets|mettez|écris|écrivez|passe|passez|convertis)\s+)?(?:(?:tout|le texte)\s+)?en\s+majuscules?$|^tout\s+en\s+majuscules$",
    lower: r"^(?:(?:mets|mettez|écris|écrivez|passe|passez|convertis)\s+)?(?:(?:tout|le texte)\s+)?en\s+minuscules?$|^tout\s+en\s+minuscules$",
    title: r"^(?:mets\s+)?une\s+majuscule\s+à\s+chaque\s+mot$",
    emoji: r"^(?:emojis?|émojis?|émoticônes?|smileys?)$",
    email: r"^(?:adresses?\s+e-?mail|e-?mails?|courriels?|adresses?\s+électroniques?)$",
    url: r"^(?:urls?|liens?)$",
    vowels: r"^voyelles?$",
    consonants: r"^consonnes?$",
    marks: &[
        ("points d'exclamation", "!"),
        ("point d'exclamation", "!"),
        ("virgules", ","),
        ("points d'interrogation", "?"),
    ],
    vague: &[
        "le",
        "ça",
        "cela",
        "tout",
        "texte",
        "noms",
        "nombres",
        "mots",
        "ton",
        "style",
        "français",
        "anglais",
        "fautes",
        "langue",
    ],
};

const PT: Lang = Lang {
    replace_verbs: &[
        "substitua",
        "substituir",
        "troque",
        "trocar",
        "mude",
        "mudar",
        "altere",
        "converta",
    ],
    connectors: &["por", "com", "para", "em"],
    remove_verbs: &[
        "remova", "remover", "apague", "apagar", "tire", "tirar", "elimine", "exclua",
    ],
    and: &["e"],
    fillers: &[
        "todas",
        "todos",
        "cada",
        "as",
        "os",
        "a",
        "o",
        "letra",
        "letras",
        "palavra",
        "palavras",
        "caracteres",
        "de",
    ],
    trailers: &["em todo o texto", "respectivamente", "por favor"],
    upper: r"^(?:(?:coloque|escreva|converta|passe)\s+)?(?:(?:tudo|o texto)\s+)?(?:em|para)\s+maiúsculas$|^tudo\s+em\s+maiúsculas$",
    lower: r"^(?:(?:coloque|escreva|converta|passe)\s+)?(?:(?:tudo|o texto)\s+)?(?:em|para)\s+minúsculas$|^tudo\s+em\s+minúsculas$",
    title: r"^(?:coloque\s+)?a\s+primeira\s+letra\s+de\s+cada\s+palavra\s+em\s+maiúscula$",
    emoji: r"^(?:emojis?|emoticons?)$",
    email: r"^(?:e-?mails?|endereços\s+de\s+e-?mail)$",
    url: r"^(?:urls?|links?)$",
    vowels: r"^(?:vogal|vogais)$",
    consonants: r"^consoantes?$",
    marks: &[
        ("pontos de exclamação", "!"),
        ("vírgulas", ","),
        ("pontos de interrogação", "?"),
    ],
    vague: &[
        "isso",
        "isto",
        "tudo",
        "texto",
        "nomes",
        "números",
        "palavras",
        "tom",
        "estilo",
        "português",
        "inglês",
        "erros",
        "idioma",
    ],
};

const IT: Lang = Lang {
    replace_verbs: &[
        "sostituisci",
        "sostituire",
        "cambia",
        "cambiare",
        "rimpiazza",
        "converti",
    ],
    connectors: &["con", "in", "per"],
    remove_verbs: &[
        "rimuovi",
        "rimuovere",
        "elimina",
        "eliminare",
        "cancella",
        "togli",
        "togliere",
    ],
    and: &["e", "ed"],
    fillers: &[
        "tutte",
        "tutti",
        "ogni",
        "le",
        "gli",
        "i",
        "la",
        "il",
        "l'",
        "lettera",
        "lettere",
        "parola",
        "parole",
        "caratteri",
        "di",
        "delle",
        "dei",
    ],
    trailers: &["ovunque", "rispettivamente", "nel testo", "per favore"],
    upper: r"^(?:(?:metti|scrivi|converti)\s+)?(?:(?:tutto|il testo)\s+)?(?:in|a)\s+maiuscolo$|^tutto\s+(?:in\s+)?maiuscolo$",
    lower: r"^(?:(?:metti|scrivi|converti)\s+)?(?:(?:tutto|il testo)\s+)?(?:in|a)\s+minuscolo$|^tutto\s+(?:in\s+)?minuscolo$",
    title: r"^(?:metti\s+)?la\s+maiuscola\s+(?:a|all'inizio\s+di)\s+ogni\s+parola$",
    emoji: r"^(?:emoji|emoticon|faccine)$",
    email: r"^(?:e-?mail|indirizzi\s+e-?mail)$",
    url: r"^(?:url|link)$",
    vowels: r"^vocal[ei]$",
    consonants: r"^consonant[ei]$",
    marks: &[
        ("punti esclamativi", "!"),
        ("virgole", ","),
        ("punti interrogativi", "?"),
    ],
    vague: &[
        "lo", "questo", "tutto", "testo", "nomi", "numeri", "parole", "tono", "stile", "italiano",
        "inglese", "errori", "lingua",
    ],
};

const NL: Lang = Lang {
    replace_verbs: &[
        "vervang",
        "vervangen",
        "verander",
        "veranderen",
        "wijzig",
        "wissel",
    ],
    connectors: &["door", "met", "in", "naar", "voor"],
    remove_verbs: &["verwijder", "verwijderen", "haal weg", "schrap", "wis"],
    and: &["en"],
    fillers: &[
        "alle", "elke", "ieder", "iedere", "de", "het", "letter", "letters", "woord", "woorden",
        "tekens",
    ],
    trailers: &["overal", "respectievelijk", "in de tekst", "alsjeblieft"],
    upper: r"^(?:(?:zet|schrijf|maak)\s+)?(?:(?:alles|de tekst|het)\s+)?(?:in\s+)?hoofdletters$",
    lower: r"^(?:(?:zet|schrijf|maak)\s+)?(?:(?:alles|de tekst|het)\s+)?(?:in\s+)?kleine\s+letters$",
    title: r"^(?:elk|ieder)\s+woord\s+met\s+een\s+hoofdletter$",
    emoji: r"^(?:emoji'?s|emoji|smileys?)$",
    email: r"^(?:e-?mailadressen|e-?mails?)$",
    url: r"^(?:urls?|links?)$",
    vowels: r"^klinkers?$",
    consonants: r"^medeklinkers?$",
    marks: &[
        ("uitroeptekens", "!"),
        ("komma's", ","),
        ("vraagtekens", "?"),
    ],
    vague: &[
        "het",
        "dit",
        "dat",
        "alles",
        "tekst",
        "namen",
        "getallen",
        "woorden",
        "toon",
        "stijl",
        "nederlands",
        "engels",
        "fouten",
        "taal",
    ],
};

const LANGS: [&Lang; 7] = [&EN, &DE, &ES, &FR, &PT, &IT, &NL];

struct Compiled {
    lang: &'static Lang,
    upper: Regex,
    lower: Regex,
    title: Regex,
    emoji: Regex,
    email: Regex,
    url: Regex,
    vowels: Regex,
    consonants: Regex,
}

static COMPILED: Lazy<Vec<Compiled>> = Lazy::new(|| {
    LANGS
        .iter()
        .map(|lang| Compiled {
            lang,
            upper: Regex::new(lang.upper).expect("static regex"),
            lower: Regex::new(lang.lower).expect("static regex"),
            title: Regex::new(lang.title).expect("static regex"),
            emoji: Regex::new(lang.emoji).expect("static regex"),
            email: Regex::new(lang.email).expect("static regex"),
            url: Regex::new(lang.url).expect("static regex"),
            vowels: Regex::new(lang.vowels).expect("static regex"),
            consonants: Regex::new(lang.consonants).expect("static regex"),
        })
        .collect()
});

/// "Replace X with nothing" removes X.
const NOTHING: &[&str] = &[
    "nothing", "nichts", "nada", "rien", "niente", "niets", "empty",
];

// --- Tokens ----------------------------------------------------------------

#[derive(Clone, Debug, PartialEq, Eq)]
enum Tok {
    /// A quoted string, verbatim.
    Quoted(String),
    /// A bare word or symbol run, as typed.
    Bare(String),
    /// `→`, `->` or `=>`.
    Arrow,
    Comma,
}

fn closing(open: char) -> Option<&'static [char]> {
    Some(match open {
        '"' => &['"'],
        '\'' => &['\''],
        '“' => &['”', '“'],
        '‘' => &['’'],
        '„' => &['“', '”'],
        '«' => &['»'],
        '‹' => &['›'],
        '「' => &['」'],
        _ => return None,
    })
}

fn tokenize(sentence: &str) -> Option<Vec<Tok>> {
    let chars: Vec<char> = sentence.chars().collect();
    let mut toks = Vec::new();
    let mut i = 0;
    while i < chars.len() {
        let c = chars[i];
        if c.is_whitespace() {
            i += 1;
            continue;
        }
        if let Some(ends) = closing(c) {
            // An apostrophe inside a word ("don't") is not a quote.
            let word_before = i > 0 && chars[i - 1].is_alphanumeric();
            if !(c == '\'' && word_before) {
                if let Some(len) = chars[i + 1..].iter().position(|ch| ends.contains(ch)) {
                    toks.push(Tok::Quoted(chars[i + 1..i + 1 + len].iter().collect()));
                    i += len + 2;
                    continue;
                }
                if c == '"' || c == '“' || c == '„' || c == '«' {
                    return None; // unbalanced quote: not ours to guess
                }
            }
        }
        if c == '→' || c == '⇒' {
            toks.push(Tok::Arrow);
            i += 1;
            continue;
        }
        if (c == '-' || c == '=') && chars.get(i + 1) == Some(&'>') {
            toks.push(Tok::Arrow);
            i += 2;
            continue;
        }
        if c == ',' || c == '，' {
            toks.push(Tok::Comma);
            i += 1;
            continue;
        }
        let start = i;
        while i < chars.len()
            && !chars[i].is_whitespace()
            && chars[i] != ','
            && chars[i] != '→'
            && !((chars[i] == '-' || chars[i] == '=') && chars.get(i + 1) == Some(&'>'))
        {
            i += 1;
        }
        toks.push(Tok::Bare(chars[start..i].iter().collect()));
    }
    Some(toks)
}

fn lower(tok: &Tok) -> Option<String> {
    match tok {
        Tok::Bare(word) => Some(word.to_lowercase()),
        _ => None,
    }
}

/// Matches a (possibly multi-word) phrase at `toks[i..]`; returns its length.
fn phrase_at(toks: &[Tok], i: usize, phrase: &str) -> Option<usize> {
    let words: Vec<&str> = phrase.split_whitespace().collect();
    for (k, word) in words.iter().enumerate() {
        if lower(toks.get(i + k)?)?.as_str() != *word {
            return None;
        }
    }
    Some(words.len())
}

fn any_phrase_at(toks: &[Tok], i: usize, phrases: &[&str]) -> Option<usize> {
    phrases
        .iter()
        .filter_map(|phrase| phrase_at(toks, i, phrase))
        .max()
}

/// A run of bare words joined with spaces, lower-cased.
fn joined(toks: &[Tok]) -> Option<String> {
    toks.iter()
        .map(lower)
        .collect::<Option<Vec<_>>>()
        .map(|words| words.join(" "))
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Side {
    Find,
    With,
}

/// An item list ("A and E", "\"colour\"", "commas"). `None` if any piece is
/// not an exact, literal item.
fn items(c: &Compiled, toks: &[Tok], side: Side, removing: bool) -> Option<Vec<Target>> {
    let lang = c.lang;
    if let [single] = toks {
        return Some(vec![item(c, std::slice::from_ref(single), side, removing)?]);
    }
    // Split on commas and "and" words.
    let mut groups: Vec<&[Tok]> = Vec::new();
    let mut start = 0;
    let mut i = 0;
    while i < toks.len() {
        // An "and" word right after a separator is an item ("A y E": the
        // Spanish "e" is both).
        let sep = match &toks[i] {
            Tok::Comma => Some(1),
            Tok::Bare(_) if i > start => any_phrase_at(toks, i, lang.and),
            _ => None,
        };
        if let Some(len) = sep {
            let comma = toks[i] == Tok::Comma;
            groups.push(&toks[start..i]);
            i += len;
            // ", and"
            if comma {
                if let Some(len) = any_phrase_at(toks, i, lang.and) {
                    i += len;
                }
            }
            start = i;
        } else {
            i += 1;
        }
    }
    groups.push(&toks[start..]);
    let mut out = Vec::new();
    for group in groups {
        if group.is_empty() {
            return None;
        }
        out.push(item(c, group, side, removing)?);
    }
    Some(out)
}

fn strip_fillers<'t>(lang: &Lang, mut group: &'t [Tok]) -> (&'t [Tok], bool) {
    let mut named_word = false;
    while group.len() > 1 {
        let Some(word) = lower(&group[0]) else { break };
        if !lang.fillers.contains(&word.as_str()) && word != "l'" {
            break;
        }
        if matches!(
            word.as_str(),
            "word"
                | "words"
                | "letter"
                | "letters"
                | "character"
                | "characters"
                | "string"
                | "strings"
        ) {
            named_word = true;
        }
        group = &group[1..];
    }
    (group, named_word)
}

fn item(c: &Compiled, group: &[Tok], side: Side, removing: bool) -> Option<Target> {
    let lang = c.lang;
    let (group, named) = if side == Side::Find {
        strip_fillers(lang, group)
    } else {
        (group, false)
    };
    if let [Tok::Quoted(text)] = group {
        return (!text.is_empty() || side == Side::With).then(|| Target::Text(text.clone()));
    }
    let phrase = joined(group)?;
    if let Some((_, mark)) = lang.marks.iter().find(|(name, _)| *name == phrase) {
        return Some(Target::Text((*mark).to_owned()));
    }
    if side == Side::With && NOTHING.contains(&phrase.as_str()) {
        return Some(Target::Text(String::new()));
    }
    if side == Side::Find {
        if c.emoji.is_match(&phrase) {
            return Some(Target::Category(Category::Emoji));
        }
        if c.email.is_match(&phrase) {
            return Some(Target::Category(Category::Email));
        }
        if c.url.is_match(&phrase) {
            return Some(Target::Category(Category::Url));
        }
        if c.vowels.is_match(&phrase) {
            return Some(Target::Category(Category::Vowels));
        }
        if c.consonants.is_match(&phrase) {
            return Some(Target::Category(Category::Consonants));
        }
    }
    // A bare item is one token.
    let [Tok::Bare(word)] = group else {
        return None;
    };
    let lowered = word.to_lowercase();
    if side == Side::Find && LANGS.iter().any(|l| l.vague.contains(&lowered.as_str())) && !named {
        return None;
    }
    let single = word.chars().count() == 1;
    // Removing a bare multi-letter word needs "the word …" or quotes, so that
    // "remove fillers" or "remove jargon" stays an instruction.
    if side == Side::Find && removing && !single && !named {
        return None;
    }
    // A trailing full stop belongs to the sentence, not to the item.
    let trimmed = if single {
        word.as_str()
    } else {
        word.trim_end_matches(['.', '!', '?', ':', ';'])
    };
    (!trimmed.is_empty()).then(|| Target::Text(trimmed.to_owned()))
}

/// Strips trailing courtesy words ("everywhere", "please").
fn strip_trailers<'t>(lang: &Lang, mut toks: &'t [Tok]) -> (&'t [Tok], bool) {
    let mut respectively = false;
    loop {
        let mut cut = false;
        for trailer in lang.trailers {
            let words = trailer.split_whitespace().count();
            if toks.len() > words && phrase_at(toks, toks.len() - words, trailer).is_some() {
                respectively |= matches!(
                    *trailer,
                    "respectively"
                        | "jeweils"
                        | "respectivamente"
                        | "respectivement"
                        | "rispettivamente"
                        | "respectievelijk"
                );
                toks = &toks[..toks.len() - words];
                cut = true;
                break;
            }
        }
        if !cut {
            return (toks, respectively);
        }
    }
}

fn pair(finds: Vec<Target>, withs: Vec<String>) -> Option<Vec<Edit>> {
    let edit = |find: Target, with: String| Edit::Replace {
        find,
        with,
        match_case: false,
    };
    if finds.len() == withs.len() {
        Some(
            finds
                .into_iter()
                .zip(withs)
                .map(|(f, w)| edit(f, w))
                .collect(),
        )
    } else if withs.len() == 1 {
        Some(
            finds
                .into_iter()
                .map(|f| edit(f, withs[0].clone()))
                .collect(),
        )
    } else {
        None
    }
}

fn with_texts(targets: Vec<Target>) -> Option<Vec<String>> {
    targets
        .into_iter()
        .map(|t| match t {
            Target::Text(text) => Some(text),
            Target::Category(_) => None,
        })
        .collect()
}

/// What one sentence is.
enum Parsed {
    Edits(Vec<Edit>),
    /// Letters with several replacements and no mapping (see [`Plan::unclear`]).
    Unclear(Vec<Target>),
}

/// Single letters or letter classes only: "which letter becomes which" is a
/// question the user can answer.
fn all_letters(finds: &[Target]) -> bool {
    finds.iter().all(|find| match find {
        Target::Text(text) => {
            let mut chars = text.chars();
            chars.next().is_some_and(char::is_alphabetic) && chars.next().is_none()
        }
        Target::Category(category) => category.is_letters(),
    })
}

fn parse_replace(c: &Compiled, toks: &[Tok]) -> Option<Parsed> {
    let lang = c.lang;
    let verb = any_phrase_at(toks, 0, lang.replace_verbs)?;
    let verb_word = lower(&toks[0])?;
    let (body, _) = strip_trailers(lang, &toks[verb..]);
    // "turn"/"convert" take only quoted items: "turn this into a list" is an
    // instruction.
    let quoted_only = matches!(
        verb_word.as_str(),
        "turn" | "convert" | "converti" | "convierte" | "converta" | "transforme" | "wandle"
    );
    if quoted_only && body.iter().any(|t| matches!(t, Tok::Bare(w) if !lang.connectors.contains(&w.to_lowercase().as_str()) && !lang.and.contains(&w.to_lowercase().as_str()) && !lang.fillers.contains(&w.to_lowercase().as_str())))
    {
        return None;
    }
    // Pair list: "A with 0 and E with 1".
    if let Some(edits) = pair_list(c, body) {
        return Some(Parsed::Edits(edits));
    }
    // "FINDS with WITHS": try every connector position, last one wins
    // ("change to to too" is rare; the last connector keeps "with" items).
    for (i, tok) in body.iter().enumerate().rev() {
        let Some(word) = lower(tok) else { continue };
        if !lang.connectors.contains(&word.as_str()) || i == 0 || i + 1 >= body.len() {
            continue;
        }
        let Some(finds) = items(c, &body[..i], Side::Find, false) else {
            continue;
        };
        let Some(withs) = items(c, &body[i + 1..], Side::With, false).and_then(with_texts) else {
            continue;
        };
        let several = withs.len() > 1;
        return match pair(finds.clone(), withs) {
            Some(edits) => Some(Parsed::Edits(edits)),
            // "vowels with 1 and 0": which vowel gets which is a guess.
            None => (several && all_letters(&finds)).then_some(Parsed::Unclear(finds)),
        };
    }
    None
}

/// "a with 0, e with 1" or "a to 0 and e to 1".
fn pair_list(c: &Compiled, body: &[Tok]) -> Option<Vec<Edit>> {
    let lang = c.lang;
    let mut edits = Vec::new();
    let mut rest = body;
    loop {
        let conn = rest
            .iter()
            .position(|t| lower(t).is_some_and(|w| lang.connectors.contains(&w.as_str())))?;
        if conn == 0 {
            return None;
        }
        let find = item(c, &rest[..conn], Side::Find, false)?;
        // The replacement is one token, then a separator or the end.
        let with = match rest.get(conn + 1)? {
            Tok::Quoted(text) => text.clone(),
            Tok::Bare(word) => {
                let word = word.trim_end_matches(['.', '!', '?', ';', ':']);
                if word.is_empty() {
                    return None;
                }
                word.to_owned()
            }
            _ => return None,
        };
        edits.push(Edit::Replace {
            find,
            with,
            match_case: false,
        });
        rest = &rest[conn + 2..];
        if rest.is_empty() {
            return (edits.len() > 1).then_some(edits);
        }
        let mut skip = 0;
        if rest[0] == Tok::Comma {
            skip = 1;
        }
        if let Some(len) = any_phrase_at(rest, skip, lang.and) {
            skip += len;
        }
        if skip == 0 {
            return None;
        }
        rest = &rest[skip..];
        if let Some(len) = any_phrase_at(rest, 0, lang.replace_verbs) {
            rest = &rest[len..];
        }
    }
}

fn parse_remove(c: &Compiled, toks: &[Tok]) -> Option<Vec<Edit>> {
    let lang = c.lang;
    let verb = any_phrase_at(toks, 0, lang.remove_verbs)?;
    let (body, _) = strip_trailers(lang, &toks[verb..]);
    let finds = items(c, body, Side::Find, true)?;
    Some(
        finds
            .into_iter()
            .map(|find| Edit::Replace {
                find,
                with: String::new(),
                match_case: false,
            })
            .collect(),
    )
}

/// `a → 0, e → 1` in any language.
fn parse_arrows(toks: &[Tok]) -> Option<Vec<Edit>> {
    if !toks.contains(&Tok::Arrow) {
        return None;
    }
    let mut edits = Vec::new();
    for part in toks.split(|t| *t == Tok::Comma) {
        let [find, Tok::Arrow, rest @ ..] = part else {
            return None;
        };
        let text = |tok: &Tok| match tok {
            Tok::Quoted(text) => Some(text.clone()),
            Tok::Bare(word) => Some(word.clone()),
            _ => None,
        };
        let find = text(find).filter(|f| !f.is_empty())?;
        let with = match rest {
            [] => String::new(),
            [tok] => text(tok)?,
            _ => return None,
        };
        edits.push(Edit::Replace {
            find: Target::Text(find),
            with,
            match_case: false,
        });
    }
    Some(edits)
}

fn normalized(sentence: &str) -> String {
    sentence
        .trim()
        .trim_end_matches(['.', '!', '。'])
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .to_lowercase()
}

/// One sentence as exact edits, or `None`.
pub fn parse_sentence(sentence: &str) -> Option<Vec<Edit>> {
    match parse(sentence)? {
        Parsed::Edits(edits) => Some(edits),
        Parsed::Unclear(_) => None,
    }
}

fn parse(sentence: &str) -> Option<Parsed> {
    let text = sentence.trim();
    if text.is_empty() {
        return None;
    }
    let norm = normalized(text);
    for c in COMPILED.iter() {
        for (re, mode) in [
            (&c.upper, CaseMode::Upper),
            (&c.lower, CaseMode::Lower),
            (&c.title, CaseMode::Title),
        ] {
            if re.is_match(&norm) {
                return Some(Parsed::Edits(vec![Edit::Case { mode }]));
            }
        }
        if let Some(rest) = norm
            .strip_prefix("no ")
            .or_else(|| norm.strip_suffix("-free"))
        {
            let rest = rest.trim_start_matches("make it ").trim();
            if c.emoji.is_match(rest) {
                return Some(Parsed::Edits(vec![Edit::Replace {
                    find: Target::Category(Category::Emoji),
                    with: String::new(),
                    match_case: false,
                }]));
            }
        }
    }
    // The sentence's own full stop is not part of the last item.
    let body = text.strip_suffix('.').unwrap_or(text);
    let toks = tokenize(body)?;
    if let Some(edits) = parse_arrows(&toks) {
        return Some(Parsed::Edits(edits));
    }
    // A clear reading in any language wins over an unclear one.
    let mut unclear = None;
    for c in COMPILED.iter() {
        match parse_replace(c, &toks) {
            Some(Parsed::Unclear(finds)) => {
                unclear.get_or_insert(finds);
            }
            Some(edits) => return Some(edits),
            None => {}
        }
        if let Some(edits) = parse_remove(c, &toks) {
            return Some(Parsed::Edits(edits));
        }
    }
    unclear.map(Parsed::Unclear)
}

/// Splits a prompt into sentences: lines, then `. `/`; `/`! `/`? ` outside
/// quotes.
fn sentences(prompt: &str) -> Vec<String> {
    let mut out = Vec::new();
    for line in prompt.lines() {
        let chars: Vec<char> = line.chars().collect();
        let mut current = String::new();
        let mut quote: Option<&'static [char]> = None;
        for (i, &ch) in chars.iter().enumerate() {
            current.push(ch);
            if let Some(ends) = quote {
                if ends.contains(&ch) {
                    quote = None;
                }
                continue;
            }
            let word_before = i > 0 && chars[i - 1].is_alphanumeric();
            if !(ch == '\'' && word_before) {
                if let Some(ends) = closing(ch) {
                    if chars[i + 1..].iter().any(|c| ends.contains(c)) {
                        quote = Some(ends);
                        continue;
                    }
                }
            }
            if matches!(ch, '.' | ';' | '!' | '?' | '。')
                && chars.get(i + 1).is_none_or(|c| c.is_whitespace())
            {
                out.push(std::mem::take(&mut current));
            }
        }
        out.push(current);
    }
    out.into_iter()
        .map(|s| s.trim().to_owned())
        .filter(|s| !s.is_empty())
        .collect()
}

/// Splits `prompt` into exact edits and the rest.
pub fn plan(prompt: &str) -> Plan {
    let mut plan = Plan::default();
    let mut rest = Vec::new();
    for sentence in sentences(prompt) {
        match parse(&sentence) {
            Some(Parsed::Edits(edits)) => plan.edits.extend(edits),
            Some(Parsed::Unclear(finds)) => {
                plan.unclear.extend(finds);
                rest.push(sentence);
            }
            None => rest.push(sentence),
        }
    }
    plan.rest = rest.join(" ");
    plan
}

/// The editor rows as edits (blank finds skipped).
pub fn rule_edits(rules: &[LiteralRule]) -> Vec<Edit> {
    rules
        .iter()
        .filter(|rule| !rule.find.is_empty())
        .map(|rule| Edit::Replace {
            find: Target::Text(rule.find.clone()),
            with: rule.replace.clone(),
            match_case: rule.match_case,
        })
        .collect()
}

pub fn validate_rules(rules: &[LiteralRule]) -> Result<(), String> {
    if rules.len() > MAX_RULES {
        return Err(format!("at most {MAX_RULES} find and replace rows"));
    }
    if rules.iter().any(|rule| {
        rule.find.chars().count() > MAX_RULE_CHARS || rule.replace.chars().count() > MAX_RULE_CHARS
    }) {
        return Err(format!(
            "find and replace text is limited to {MAX_RULE_CHARS} characters"
        ));
    }
    Ok(())
}

// --- Applying --------------------------------------------------------------

static EMAIL: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r"[\p{L}\p{N}._%+-]+@[\p{L}\p{N}.-]+\.\p{L}{2,}").expect("static regex")
});
static URL: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r#"(?i)\b(?:https?://|www\.)[^\s<>"']*[^\s<>"'.,;:!?)\]]"#).expect("static regex")
});

fn is_emoji(c: char) -> bool {
    matches!(c as u32,
        0x1F000..=0x1FAFF | 0x2600..=0x27BF | 0x2B00..=0x2BFF
            | 0xFE0F | 0x200D | 0x20E3 | 0xE0020..=0xE007F)
}

/// The replacement in the case of what it replaces (only when the
/// replacement has letters and the match was case-insensitive).
fn follow_case(found: &str, with: &str) -> String {
    let letters: Vec<char> = found.chars().filter(|c| c.is_alphabetic()).collect();
    if letters.is_empty() || !with.chars().any(char::is_alphabetic) {
        return with.to_owned();
    }
    let all_upper = letters.iter().all(|c| c.is_uppercase());
    if all_upper && (letters.len() > 1 || with.chars().filter(|c| c.is_alphabetic()).count() == 1) {
        return with.to_uppercase();
    }
    let first_upper = letters[0].is_uppercase();
    if first_upper {
        let mut chars = with.chars();
        return match chars.next() {
            Some(first) => first.to_uppercase().chain(chars).collect(),
            None => String::new(),
        };
    }
    if letters.iter().all(|c| c.is_lowercase()) && with.chars().any(char::is_uppercase) {
        // The user typed the replacement's case on purpose ("jon → John").
        return with.to_owned();
    }
    with.to_owned()
}

fn replace_letters(text: &str, class: Category, with: &str) -> (String, usize) {
    let mut out = String::with_capacity(text.len());
    let mut count = 0;
    let mut buf = [0u8; 4];
    for c in text.chars() {
        if class.has(c) {
            count += 1;
            out.push_str(&follow_case(c.encode_utf8(&mut buf), with));
        } else {
            out.push(c);
        }
    }
    (out, count)
}

/// How often `target` occurs in `text`, letters in any case.
fn occurrences(target: &Target, text: &str) -> usize {
    match target {
        Target::Category(category) if category.is_letters() => {
            text.chars().filter(|c| category.has(*c)).count()
        }
        Target::Text(find) if !find.is_empty() => replace_text(text, find, "", false).1,
        _ => 0,
    }
}

/// True when the model left every one of the `unclear` letters as it was:
/// it cleaned the text up instead of doing the edit (see [`Plan::unclear`]).
pub fn untouched(unclear: &[Target], source: &str, output: &str) -> bool {
    let before: usize = unclear.iter().map(|t| occurrences(t, source)).sum();
    let after: usize = unclear.iter().map(|t| occurrences(t, output)).sum();
    before > 0 && after >= before
}

fn word_like(find: &str) -> bool {
    find.chars().count() > 1
        && find
            .chars()
            .all(|c| c.is_alphanumeric() || c == '\'' || c == '’' || c == '-')
}

fn replace_text(text: &str, find: &str, with: &str, match_case: bool) -> (String, usize) {
    let escaped = regex::escape(find);
    let letters = find.chars().any(char::is_alphabetic);
    let pattern = if word_like(find) {
        // Whole words only: "Jon" is not in "Jonathan".
        format!(r"(?:^|\b){escaped}(?:\b|$)")
    } else {
        escaped
    };
    let pattern = if !match_case && letters {
        format!("(?i){pattern}")
    } else {
        pattern
    };
    let Ok(re) = Regex::new(&pattern) else {
        return (text.to_owned(), 0);
    };
    let mut count = 0;
    let out = re.replace_all(text, |caps: &regex::Captures<'_>| {
        count += 1;
        let found = &caps[0];
        if match_case {
            with.to_owned()
        } else {
            follow_case(found, with)
        }
    });
    (out.into_owned(), count)
}

/// Removing a sentence's `!` or `?` leaves a full stop, not a run-on.
fn remove_end_mark(text: &str, mark: char) -> (String, usize) {
    let chars: Vec<char> = text.chars().collect();
    let mut out = String::with_capacity(text.len());
    let mut count = 0;
    let mut i = 0;
    while i < chars.len() {
        if chars[i] == mark {
            let mut j = i;
            while j < chars.len() && (chars[j] == '!' || chars[j] == '?') {
                j += 1;
            }
            count += j - i;
            let after_word = out.chars().last().is_some_and(char::is_alphanumeric);
            let ends = chars
                .get(j)
                .is_none_or(|c| c.is_whitespace() || *c == '"' || *c == '”');
            if after_word && ends {
                out.push('.');
            }
            i = j;
            continue;
        }
        out.push(chars[i]);
        i += 1;
    }
    (out, count)
}

/// Spaces left by removals: no doubles, none before punctuation, none at a
/// line's edges. Lines and blank lines are kept.
fn tidy(text: &str) -> String {
    static SPACES: Lazy<Regex> = Lazy::new(|| Regex::new(r"[ \t]{2,}").expect("static regex"));
    static BEFORE_PUNCT: Lazy<Regex> =
        Lazy::new(|| Regex::new(r"[ \t]+([,.;:!?…])").expect("static regex"));
    static DOUBLE_COMMA: Lazy<Regex> = Lazy::new(|| Regex::new(r",\s*,").expect("static regex"));
    text.lines()
        .map(|line| {
            let line = SPACES.replace_all(line, " ");
            let line = BEFORE_PUNCT.replace_all(&line, "$1");
            let line = DOUBLE_COMMA.replace_all(&line, ",");
            line.trim().to_owned()
        })
        .collect::<Vec<_>>()
        .join("\n")
        + if text.ends_with('\n') { "\n" } else { "" }
}

fn title_case(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut at_start = true;
    for c in text.chars() {
        if at_start && c.is_alphabetic() {
            out.extend(c.to_uppercase());
            at_start = false;
        } else {
            out.push(c);
            if c.is_whitespace() || c == '-' || c == '/' || c == '(' || c == '"' || c == '“' {
                at_start = true;
            } else if c.is_alphanumeric() {
                at_start = false;
            }
        }
    }
    out
}

/// The result of applying exact edits.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Applied {
    pub text: String,
    /// How many places changed (0: nothing matched).
    pub changes: usize,
}

pub fn apply(edits: &[Edit], text: &str) -> Applied {
    let mut current = text.to_owned();
    let mut changes = 0;
    let mut removed = false;
    let mut removed_words = false;
    for edit in edits {
        match edit {
            Edit::Case { mode } => {
                let next = match mode {
                    CaseMode::Upper => current.to_uppercase(),
                    CaseMode::Lower => current.to_lowercase(),
                    CaseMode::Title => title_case(&current),
                };
                if next != current {
                    changes += 1;
                }
                current = next;
            }
            Edit::Replace {
                find,
                with,
                match_case,
            } => {
                let (next, count) = match find {
                    Target::Category(Category::Emoji) => {
                        let count = current.chars().filter(|c| is_emoji(*c)).count();
                        (current.chars().filter(|c| !is_emoji(*c)).collect(), count)
                    }
                    Target::Category(Category::Email) => {
                        let count = EMAIL.find_iter(&current).count();
                        (
                            EMAIL.replace_all(&current, with.as_str()).into_owned(),
                            count,
                        )
                    }
                    Target::Category(Category::Url) => {
                        let count = URL.find_iter(&current).count();
                        (URL.replace_all(&current, with.as_str()).into_owned(), count)
                    }
                    Target::Category(class @ (Category::Vowels | Category::Consonants)) => {
                        replace_letters(&current, *class, with)
                    }
                    Target::Text(find) if with.is_empty() && (find == "!" || find == "?") => {
                        remove_end_mark(&current, find.chars().next().unwrap_or('!'))
                    }
                    Target::Text(find) => replace_text(&current, find, with, *match_case),
                };
                removed |= with.is_empty() && count > 0;
                // Letters taken out of a word leave no word to re-capitalise.
                removed_words |= with.is_empty()
                    && count > 0
                    && !matches!(find, Target::Category(class) if class.is_letters());
                changes += count;
                current = next;
            }
        }
    }
    if removed {
        current = tidy(&current);
        // A removed first word leaves the sentence starting lower-case.
        if removed_words && text.chars().next().is_some_and(char::is_uppercase) {
            let mut chars = current.chars();
            if let Some(first) = chars.next() {
                current = first.to_uppercase().chain(chars).collect();
            }
        }
    }
    Applied {
        text: current,
        changes,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn run(prompt: &str, text: &str) -> String {
        let plan = plan(prompt);
        assert!(plan.rest.is_empty(), "{prompt:?} left {:?}", plan.rest);
        apply(&plan.edits, text).text
    }

    fn not_exact(prompt: &str) {
        let plan = plan(prompt);
        assert!(plan.edits.is_empty(), "{prompt:?} became {:?}", plan.edits);
    }

    #[test]
    fn the_owners_transform_replaces_both_letters_in_any_case() {
        assert_eq!(
            run(
                "Replace all A and E with 0 and 1",
                "Please send the agenda before the meeting tomorrow. EASY!"
            ),
            "Pl10s1 s1nd th1 0g1nd0 b1for1 th1 m11ting tomorrow. 10SY!"
        );
        for prompt in [
            "replace a and e with 0 and 1",
            "Replace A with 0 and E with 1.",
            "Replace every \"a\" with \"0\", and every \"e\" with \"1\"",
            "Change all the letters A and E to 0 and 1 respectively",
            "a → 0, e → 1",
            "A -> 0, E -> 1",
            "Ersetze alle A und E durch 0 und 1",
            "Reemplaza las A y E por 0 y 1",
            "Remplace tous les A et E par 0 et 1",
            "Vervang alle A en E door 0 en 1",
        ] {
            assert_eq!(run(prompt, "Ate"), "0t1", "{prompt}");
        }
    }

    #[test]
    fn words_match_whole_and_keep_their_case() {
        assert_eq!(
            run(
                "Change Jon to John everywhere",
                "Jon met Jonathan. jon left."
            ),
            "John met Jonathan. John left."
        );
        assert_eq!(
            run(
                "Replace \"colour\" with \"color\"",
                "Colour and colour and COLOUR."
            ),
            "Color and color and COLOR."
        );
        assert_eq!(
            run("Swap Monday for Tuesday", "Monday, then Monday afternoon."),
            "Tuesday, then Tuesday afternoon."
        );
    }

    #[test]
    fn removals_tidy_spaces_and_keep_sentences() {
        assert_eq!(
            run(
                "Remove all exclamation marks",
                "Thanks so much! This is perfect!! Ok"
            ),
            "Thanks so much. This is perfect. Ok"
        );
        assert_eq!(
            run("Remove all commas", "First, open it, then save."),
            "First open it then save."
        );
        assert_eq!(
            run(
                "Remove the word \"basically\"",
                "Basically the plan is basically fine."
            ),
            "The plan is fine."
        );
        assert_eq!(
            run("Remove the word basically", "it is basically done"),
            "it is done"
        );
        assert_eq!(
            run("Remove all emojis", "Great 🎉 launch 🚀👍🏽 team"),
            "Great launch team"
        );
        assert_eq!(run("No emojis", "Ok 👋"), "Ok");
        assert_eq!(
            run(
                "Replace email addresses with [email]",
                "Mail anna.berg@example.com or a@b.org."
            ),
            "Mail [email] or [email]."
        );
        assert_eq!(
            run("Remove links", "See https://example.com/a, then www.x.org."),
            "See, then."
        );
    }

    #[test]
    fn case_changes_in_several_languages() {
        for prompt in [
            "Convert everything to uppercase",
            "Make it all caps",
            "ALL CAPS",
            "uppercase everything",
            "Alles in Großbuchstaben",
            "Todo en mayúsculas",
            "Tout en majuscules",
        ] {
            assert_eq!(run(prompt, "Meet at noon."), "MEET AT NOON.", "{prompt}");
        }
        assert_eq!(
            run("Make everything lowercase", "Thanks For It"),
            "thanks for it"
        );
        assert_eq!(
            run(
                "Capitalize the first letter of every word",
                "quarterly notes for the team"
            ),
            "Quarterly Notes For The Team"
        );
    }

    #[test]
    fn instructions_that_are_more_than_an_exact_edit_stay_with_the_model() {
        for prompt in [
            "Turn this into a bullet list",
            "Translate into Spanish",
            "Make this more formal",
            "Change it to Spanish",
            "Change the tone to formal",
            "Replace jargon with plain words",
            "Replace every name with [name]",
            "Remove filler words like um, uh and you know",
            "Remove jargon",
            "Convert the steps to a numbered list",
            "Make it shorter and replace Bob with Robert",
            "Rewrite the selected text while preserving its meaning, facts and language.",
            "Polish this text. Fix grammar, spelling, punctuation and awkward wording while preserving its meaning, tone, language, formatting and approximate length. Return the same text when no change is needed.",
            "Use American spelling",
            "Write numbers as digits",
            "Remove hedging like \"I think\" and \"maybe\"",
        ] {
            not_exact(prompt);
        }
    }

    #[test]
    fn mixed_prompts_keep_the_rest_for_the_model() {
        let plan = plan("Make it friendlier. Replace Bob with Robert.\nKeep it short");
        assert_eq!(plan.rest, "Make it friendlier. Keep it short");
        assert_eq!(
            plan.edits,
            vec![Edit::Replace {
                find: Target::Text("Bob".into()),
                with: "Robert".into(),
                match_case: false
            }]
        );
    }

    #[test]
    fn editor_rows_match_case_only_when_asked() {
        let rows = [
            LiteralRule {
                find: "Hi".into(),
                replace: "Hello".into(),
                match_case: true,
            },
            LiteralRule {
                find: String::new(),
                replace: "ignored".into(),
                match_case: false,
            },
        ];
        let edits = rule_edits(&rows);
        assert_eq!(edits.len(), 1);
        assert_eq!(apply(&edits, "Hi hi HI").text, "Hello hi HI");
        let applied = apply(&edits, "nothing here");
        assert_eq!(applied.changes, 0);
        assert_eq!(applied.text, "nothing here");
        assert!(validate_rules(&vec![LiteralRule::default(); MAX_RULES + 1]).is_err());
    }

    #[test]
    fn newlines_survive_every_edit() {
        let text = "Line one, a\n\n- item, e";
        assert_eq!(run("Remove all commas", text), "Line one a\n\n- item e");
        assert_eq!(run("a → 0", text), "Line one, 0\n\n- item, e");
    }

    #[test]
    fn vowels_and_consonants_are_letter_classes() {
        assert_eq!(
            run("Replace vowels with 1", "Area UNIT, Ölberg"),
            "1r11 1N1T, 1lb1rg"
        );
        assert_eq!(
            run("Replace all the vowels with x", "Area UNIT"),
            "Xrxx XNXT"
        );
        assert_eq!(run("Remove vowels", "Apple pie, I said."), "ppl p, sd.");
        assert_eq!(run("remove all consonants", "Strasse ñandú"), "ae aú");
        assert_eq!(run("Replace consonants with *", "Hey you"), "*e* *ou");
        for (prompt, text, want) in [
            ("Ersetze alle Vokale durch 1", "Bäume", "B11m1"),
            ("Reemplaza las vocales por 1", "canción", "c1nc11n"),
            ("Remplace toutes les voyelles par 1", "été", "1t1"),
            ("Substitua as vogais por 1", "pão", "p11"),
            ("Sostituisci le vocali con 1", "città", "c1tt1"),
            ("Vervang alle klinkers door 1", "boek", "b11k"),
            ("Entferne alle Konsonanten", "Straße", "ae"),
        ] {
            assert_eq!(run(prompt, text), want, "{prompt}");
        }
        // A clear mapping is still exact.
        assert_eq!(run("Replace vowels with 1 and y with 2", "yay"), "212");
    }

    #[test]
    fn letters_with_several_replacements_and_no_mapping_are_not_guessed() {
        for prompt in [
            "Replace vowels with 1 and 0",
            "replace the vowels with 1, 0 and 2",
            "Ersetze alle Vokale durch 1 und 0",
            "Replace a, e and i with 1 and 0",
        ] {
            let plan = plan(prompt);
            assert!(plan.edits.is_empty(), "{prompt}");
            assert!(!plan.unclear.is_empty(), "{prompt}");
            assert_eq!(plan.rest, prompt);
        }
        let unclear = plan("Replace vowels with 1 and 0").unclear;
        assert_eq!(unclear, vec![Target::Category(Category::Vowels)]);
        assert!(untouched(&unclear, "hello there", "Hello there."));
        assert!(!untouched(&unclear, "hello there", "h1ll0 th1r0"));
        assert!(!untouched(&unclear, "rhythm", "rhythm"));
        // Words, not letters: the model may do it; nothing to ask.
        assert!(plan("Replace Bob and Jim with X, Y and Z")
            .unclear
            .is_empty());
        assert!(plan("Make it formal").unclear.is_empty());
    }

    #[test]
    fn symbols_and_digits_are_literal() {
        assert_eq!(run("Replace semicolons with commas", "a; b"), "a, b");
        assert_eq!(
            run("Replace \"very\" with nothing", "very very good"),
            "good"
        );
        not_exact("Change all dashes to commas");
        assert_eq!(run("Replace & with and", "R&D & ops"), "RandD and ops");
        assert_eq!(run("\"0\" → \"zero\"", "10 and 0"), "1zero and zero");
    }
}
