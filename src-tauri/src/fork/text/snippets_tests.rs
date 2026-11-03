// SPDX-License-Identifier: GPL-3.0-or-later
//! S-17 acceptance: Wispr Flow's documented snippet rules as a table
//! (alone or inline; inline punctuation must match; spoken case ignored;
//! the saved casing kept; longest match wins; possessives expand, "n't" and
//! other contractions do not; ≤ 60 / ≤ 4,000 characters; one pass, never
//! rescanned), plus Unicode boundaries (scripts without spaces, combining
//! marks, NFC, Turkish i, Cyrillic, emoji).

use super::*;

const ADDRESS: &str = "Alex Morgan\n12 Harbour Lane\nKiel 24103";
const EMAIL: &str = "finn@example.com";
const SIGNATURE: &str = "Best wishes,\nFinn";
const SIG: &str = "Sent from my desk";
const CALENDAR: &str = "Calendar: Tuesdays 9-12";
const THANKS: &str = "Thank you very much.";

fn set() -> SnippetSet {
    let s = |id: i64, trigger: &str, body: &str| Snippet {
        id,
        trigger: trigger.into(),
        body: body.into(),
    };
    SnippetSet::new(vec![
        s(1, "my address", ADDRESS),
        s(2, "my email", EMAIL),
        s(3, "my email signature", SIGNATURE),
        s(4, "e-mail sig", SIG),
        s(5, "calendar link", CALENDAR),
        s(6, "link", "LINK"),
        s(7, "İmza", "Saygılarımla"),
        s(8, "can", "CAN bus"),
        s(9, "brien", "BRIEN"),
        s(10, "addr", "ADDR"),
        s(11, "我的地址", "上海市南京路1号"),
        s(12, "住所", "東京都"),
        s(13, "café menu", "Menu: soup"),
        s(14, "мой адрес", "Москва"),
        s(15, "loop", "loop loop link"),
        s(16, "my thanks", THANKS),
        s(17, "send address", "SA"),
        s(18, "address book", "AB"),
    ])
}

fn ex(text: &str) -> String {
    expand(&set(), text).0
}

/// (input, expected) — every row is one acceptance case.
const TABLE: &[(&str, &str)] = &[
    // --- spoken alone: punctuation, symbols, spacing and case ignored
    ("my address", ADDRESS),
    ("My address.", ADDRESS),
    ("MY ADDRESS!", ADDRESS),
    ("  my address  ", ADDRESS),
    ("My, address?", ADDRESS),
    ("my\naddress", ADDRESS),
    ("e-mail sig", SIG),
    ("Email sig.", SIG),
    ("e mail sig", SIG),
    ("My email-signature.", SIGNATURE),
    ("İMZA", "Saygılarımla"),
    ("imza.", "Saygılarımla"),
    // --- inline: as written, any case, whole words
    (
        "Please send it to my address today.",
        "Please send it to Alex Morgan\n12 Harbour Lane\nKiel 24103 today.",
    ),
    (
        "Send it to My Address.",
        "Send it to Alex Morgan\n12 Harbour Lane\nKiel 24103.",
    ),
    (
        "my address, please",
        "Alex Morgan\n12 Harbour Lane\nKiel 24103, please",
    ),
    ("Write to my email.", "Write to finn@example.com."),
    ("write MY EMAIL now", "write finn@example.com now"),
    ("Use the e-mail sig here", "Use the Sent from my desk here"),
    ("Use the E-MAIL SIG here", "Use the Sent from my desk here"),
    // inline, internal punctuation must match
    ("Add the email sig please", "Add the email sig please"),
    ("Add the e mail sig please", "Add the e mail sig please"),
    // --- boundaries: letters, digits, underscores
    ("these are my addresses", "these are my addresses"),
    ("see my address2 now", "see my address2 now"),
    ("see my address_old now", "see my address_old now"),
    ("remy address", "remy address"),
    ("the address is here", "the address is here"),
    (
        "see (my address)",
        "see (Alex Morgan\n12 Harbour Lane\nKiel 24103)",
    ),
    ("mail “my email”", "mail “finn@example.com”"),
    // --- possessives expand, contractions do not
    (
        "Is my address's format right?",
        "Is Alex Morgan\n12 Harbour Lane\nKiel 24103's format right?",
    ),
    ("my email’s inbox", "finn@example.com’s inbox"),
    ("We can go", "We CAN bus go"),
    ("I can't go", "I can't go"),
    ("I can’t go", "I can’t go"),
    ("the can's lid", "the CAN bus's lid"),
    ("can, maybe", "CAN bus, maybe"),
    ("Mr. O'Brien called", "Mr. O'Brien called"),
    // --- longest match wins
    ("my email signature is long", "Best wishes,\nFinn is long"),
    (
        "my email and my email signature",
        "finn@example.com and Best wishes,\nFinn",
    ),
    ("open the calendar link", "open the Calendar: Tuesdays 9-12"),
    ("a link here", "a LINK here"),
    // equal length: the earlier one
    ("send address book", "SA book"),
    // --- single pass, never rescanned; saved casing kept
    ("loop", "loop loop link"),
    ("a loop b", "a loop loop link b"),
    ("a link and a loop", "a LINK and a loop loop link"),
    // --- a sentence mark is not doubled
    ("And my thanks.", "And Thank you very much."),
    ("my thanks to all", "Thank you very much. to all"),
    // --- lines
    (
        "Hi Anna,\nmy address\nThanks",
        "Hi Anna,\nAlex Morgan\n12 Harbour Lane\nKiel 24103\nThanks",
    ),
    // --- nothing to do
    ("", ""),
    ("...", "..."),
    ("nothing here at all", "nothing here at all"),
];

#[test]
fn acceptance_table() {
    assert!(TABLE.len() >= 40, "{} cases", TABLE.len());
    let set = set();
    let mut failures = Vec::new();
    for (input, expected) in TABLE {
        let (out, _) = expand(&set, input);
        if out != *expected {
            failures.push(format!("{input:?}\n   got {out:?}\n  want {expected:?}"));
        }
    }
    assert!(failures.is_empty(), "\n{}", failures.join("\n"));
}

#[test]
fn hits_name_the_snippet_and_whether_it_was_alone() {
    let set = set();
    let (_, hits) = expand(&set, "My address.");
    assert_eq!(
        hits,
        vec![SnippetHit {
            snippet_id: 1,
            matched: "My address.".into(),
            alone: true,
        }]
    );
    let (_, hits) = expand(&set, "my email and My Email signature");
    assert_eq!(
        hits.iter()
            .map(|h| (h.snippet_id, h.matched.as_str(), h.alone))
            .collect::<Vec<_>>(),
        [(2, "my email", false), (3, "My Email signature", false)]
    );
    assert!(expand(&set, "nothing").1.is_empty());
}

// ------------------------------------------------------------ Unicode

#[test]
fn scripts_without_spaces_need_no_boundary() {
    assert_eq!(ex("请寄到我的地址谢谢"), "请寄到上海市南京路1号谢谢");
    assert_eq!(ex("私の住所です"), "私の東京都です");
    assert_eq!(ex("住所"), "東京都");
}

#[test]
fn combining_marks_are_part_of_the_word() {
    // A mark after the trigger belongs to its last letter.
    assert_eq!(
        ex("see my address\u{0301} now"),
        "see my address\u{0301} now"
    );
    // A decomposed "é" in the text meets the composed trigger (NFC).
    assert_eq!(ex("the cafe\u{0301} menu"), "the Menu: soup");
    assert_eq!(ex("CAFÉ MENU"), "Menu: soup");
    // A decomposed trigger meets the composed text.
    let set = SnippetSet::new(vec![Snippet {
        id: 1,
        trigger: "re\u{0301}sume\u{0301}".into(),
        body: "CV".into(),
    }]);
    assert_eq!(expand(&set, "my résumé please").0, "my CV please");
}

#[test]
fn turkish_dotted_and_dotless_i_meet() {
    assert_eq!(ex("imza ekle"), "Saygılarımla ekle");
    assert_eq!(ex("İmza ekle"), "Saygılarımla ekle");
    assert_eq!(ex("ımza ekle"), "Saygılarımla ekle");
    assert_eq!(ex("IMZA ekle"), "Saygılarımla ekle");
}

#[test]
fn other_cases_and_neighbours() {
    assert_eq!(ex("МОЙ АДРЕС, пожалуйста"), "Москва, пожалуйста");
    assert_eq!(ex("мой адресат"), "мой адресат");
    assert_eq!(
        ex("see my address🙂"),
        "see Alex Morgan\n12 Harbour Lane\nKiel 24103🙂"
    );
    assert_eq!(
        ex("my address#1"),
        "Alex Morgan\n12 Harbour Lane\nKiel 24103#1"
    );
    // A no-break space or a dash is not a letter.
    assert_eq!(ex("to\u{00A0}my email"), "to\u{00A0}finn@example.com");
    assert_eq!(ex("my email—thanks"), "finn@example.com—thanks");
    // Greek and Arabic letters are letters.
    assert_eq!(ex("linkα"), "linkα");
    assert_eq!(ex("linkب"), "linkب");
}

// ------------------------------------------------------------ limits

#[test]
fn triggers_and_bodies_past_the_limits_never_match() {
    let s = |id, trigger: String, body: String| Snippet { id, trigger, body };
    let sixty = "a".repeat(59) + "b";
    let set = SnippetSet::new(vec![
        s(1, sixty.clone(), "60".into()),
        s(2, "c".repeat(61), "61".into()),
        s(3, "ok body".into(), "x".repeat(MAX_BODY_CHARS)),
        s(4, "long body".into(), "x".repeat(MAX_BODY_CHARS + 1)),
        s(5, "!!!".into(), "punctuation only".into()),
        s(6, "   ".into(), "blank".into()),
        s(7, "no body".into(), String::new()),
    ]);
    assert_eq!(
        set.snippets().iter().map(|s| s.id).collect::<Vec<_>>(),
        [1, 3]
    );
    assert_eq!(expand(&set, &sixty).0, "60");
    assert_eq!(expand(&set, "ok body").0.chars().count(), MAX_BODY_CHARS);
    assert_eq!(expand(&set, "long body").0, "long body");
}

#[test]
fn an_older_snippet_wins_a_shared_alone_key() {
    let set = SnippetSet::new(vec![
        Snippet {
            id: 2,
            trigger: "email".into(),
            body: "newer".into(),
        },
        Snippet {
            id: 1,
            trigger: "e-mail".into(),
            body: "older".into(),
        },
    ]);
    assert_eq!(expand(&set, "Email.").0, "older");
    // Inline the trigger as written decides.
    assert_eq!(expand(&set, "my email now").0, "my newer now");
    assert_eq!(expand(&set, "my e-mail now").0, "my older now");
}

#[test]
fn the_matcher_is_fast_with_a_thousand_snippets() {
    let set = SnippetSet::new(
        (0..1000)
            .map(|i| Snippet {
                id: i,
                trigger: format!("snippet number {i}"),
                body: format!("Body {i}\nwith two lines"),
            })
            .collect(),
    );
    let sentence = "so I said snippet number 512 to them, and we agreed on it. ";
    let text = sentence.repeat(20);
    let started = crate::fork::testutil::CpuTimer::start();
    let (out, hits) = expand(&set, &text);
    let took = started.elapsed();
    assert_eq!(hits.len(), 20);
    assert!(out.contains("Body 512\nwith two lines"));
    assert!(took < std::time::Duration::from_millis(50), "{took:?}");
}
