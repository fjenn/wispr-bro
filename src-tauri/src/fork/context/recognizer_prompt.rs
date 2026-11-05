// SPDX-License-Identifier: GPL-3.0-or-later
//! How the take's words (the user's vocabulary and S-20's context hints)
//! reach the recognizer, and the guard against the recognizer echoing them
//! back (the stray "Glossary").
//!
//! transcribe-cpp 0.4.0 renders `RunOptions::vocabulary` for Whisper as
//! `" Glossary: a, b, c"` in the `<|startofprev|>` slot
//! (`transcribe-cpp-sys/src/arch/whisper/model.cpp`,
//! `whisper_generic_prompt_ids`). Whisper reads that slot as the previous
//! window's transcript, so it sometimes continues it: a take then starts or
//! ends with "Glossary" (or the list). For Whisper the words therefore go
//! through the crate's plain context prompt (`RunOptions::prompt`, the same
//! slot without a label) as a list that reads like dictated text, which is
//! how Whisper's prompt is meant to be primed. Other families keep the
//! crate's vocabulary (their formatting has no label in the transcript).
//!
//! [`strip_echo`] removes what is left: a standalone "Glossary" at the start
//! or end of a take, and the terms list repeated as a sentence of its own.
//! It runs only when the take's words were sent; "glossary" inside a normal
//! sentence is never touched.

use transcribe_cpp::RunOptions;

/// The context prompt's length cap. Whisper's prompt slot holds 223 tokens
/// and the crate keeps the last ones of an overlong context; names run at
/// two to four characters a token, so this stays well inside it.
pub const MAX_PROMPT_CHARS: usize = 400;

/// The label transcribe-cpp puts in front of a Whisper vocabulary.
const LABEL: &str = "glossary";

/// The `managers/transcription.rs` hook: the run options carrying the take's
/// `terms` (priority order) for a model of `arch`. Whisper gets them as its
/// context prompt ([`whisper_prompt`]), every other model as the crate's
/// vocabulary.
pub fn run_options(arch: &str, terms: Vec<String>) -> RunOptions {
    if arch.eq_ignore_ascii_case("whisper") {
        let prompt = whisper_prompt(&terms);
        log::debug!(
            "recognizer: {} term(s) as Whisper's context prompt ({} char(s))",
            terms.len(),
            prompt.as_deref().map_or(0, str::len)
        );
        return RunOptions {
            prompt,
            ..Default::default()
        };
    }
    RunOptions {
        vocabulary: terms,
        ..Default::default()
    }
}

/// Whisper's context prompt for `terms` (priority order): `"c, b, a."`,
/// lowest priority first, so an overlong prompt (whose start the crate
/// drops) loses the least important words, and Whisper reads the most
/// important ones last, nearest the audio. Terms that would not survive the
/// round trip (control tokens, commas, line breaks) are left out; terms
/// beyond [`MAX_PROMPT_CHARS`] are dropped from the end of the priority list.
/// `None` when nothing is left.
pub fn whisper_prompt(terms: &[String]) -> Option<String> {
    let mut kept: Vec<&str> = Vec::new();
    let mut len = 0;
    for term in terms {
        let term = term.trim();
        if term.is_empty()
            || term.contains("<|")
            || term.contains(',')
            || term.chars().any(char::is_control)
            || kept.iter().any(|k| k.eq_ignore_ascii_case(term))
        {
            continue;
        }
        let add = term.len() + if kept.is_empty() { 1 } else { 2 };
        if len + add > MAX_PROMPT_CHARS {
            break;
        }
        len += add;
        kept.push(term);
    }
    if kept.is_empty() {
        return None;
    }
    kept.reverse();
    Some(format!("{}.", kept.join(", ")))
}

/// The terms a run sent, in the order the model saw them (empty: none).
pub fn prompted_terms(options: &RunOptions) -> Vec<String> {
    if !options.vocabulary.is_empty() {
        return options.vocabulary.clone();
    }
    match options.prompt.as_deref() {
        Some(p) => p
            .trim()
            .trim_end_matches('.')
            .split(',')
            .map(|t| t.trim().to_owned())
            .filter(|t| !t.is_empty())
            .collect(),
        None => Vec::new(),
    }
}

/// The `finish_text` hook: `text` without a prompt echo, when `options`
/// sent terms. Logs that it removed one, never the text.
pub fn strip_echo(text: String, options: &RunOptions) -> String {
    let terms = prompted_terms(options);
    if terms.is_empty() {
        return text;
    }
    let out = strip_echo_with(&text, &terms);
    if out.len() == text.len() {
        return text;
    }
    log::info!("recognizer: removed an echo of the prompt from the take");
    out
}

/// [`strip_echo`] over the prompted `terms`.
pub fn strip_echo_with(text: &str, terms: &[String]) -> String {
    let mut sentences = split_sentences(text);
    // From the end: echo sentences, then a trailing clause "…, Glossary."
    while let Some(last) = sentences.last() {
        if is_echo(last, terms) {
            sentences.pop();
        } else {
            break;
        }
    }
    while let Some(first) = sentences.first() {
        if is_echo(first, terms) {
            sentences.remove(0);
        } else {
            break;
        }
    }
    if let Some(first) = sentences.first_mut() {
        if let Some(rest) = strip_leading_label(first) {
            *first = rest;
        }
    }
    if let Some(last) = sentences.last_mut() {
        if let Some(rest) = strip_trailing_label_clause(last) {
            *last = rest;
        }
    }
    let joined: String = sentences.concat();
    if joined.len() == text.len() {
        return text.to_owned();
    }
    joined.trim().to_owned()
}

/// `text` cut after each sentence end (`.`, `!`, `?`, `…` and the closing
/// whitespace), so the pieces concatenate back to `text`.
fn split_sentences(text: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut cur = String::new();
    let mut ended = false;
    for c in text.chars() {
        if ended && !c.is_whitespace() && !is_end(c) {
            out.push(std::mem::take(&mut cur));
            ended = false;
        }
        cur.push(c);
        if is_end(c) {
            ended = true;
        }
    }
    if !cur.is_empty() {
        out.push(cur);
    }
    out
}

fn is_end(c: char) -> bool {
    matches!(c, '.' | '!' | '?' | '…' | '。')
}

fn trim_punct(s: &str) -> &str {
    s.trim_matches(|c: char| c.is_whitespace() || c.is_ascii_punctuation() || is_end(c))
}

/// A sentence that is only the label, or the label and/or the terms list.
fn is_echo(sentence: &str, terms: &[String]) -> bool {
    let core = trim_punct(sentence);
    if core.is_empty() {
        return false;
    }
    if core.eq_ignore_ascii_case(LABEL) {
        return true;
    }
    let (labelled, list) = match label_prefix(core) {
        Some(rest) => (true, rest),
        None => (false, core),
    };
    let mut parts: Vec<&str> = list
        .split(',')
        .map(trim_punct)
        .filter(|p| !p.is_empty())
        .collect();
    // Whisper repeats a term while it echoes ("Tauri, Tauri, Kubernetes").
    parts.dedup_by(|a, b| a.eq_ignore_ascii_case(b));
    // Unlabelled, a list must be long enough not to be a sentence the user
    // said: three terms (both, when only two were sent).
    let need = if labelled { 1 } else { terms.len().clamp(2, 3) };
    parts.len() >= need && is_window(&parts, terms)
}

/// `parts` is a run of consecutive `terms`, in order (case-insensitive).
fn is_window(parts: &[&str], terms: &[String]) -> bool {
    if parts.len() > terms.len() {
        return false;
    }
    (0..=terms.len() - parts.len()).any(|start| {
        parts
            .iter()
            .zip(&terms[start..])
            .all(|(p, t)| p.to_lowercase() == trim_punct(t).to_lowercase())
    })
}

/// The rest of `core` after a leading "Glossary:" (the label form).
fn label_prefix(core: &str) -> Option<&str> {
    let head = core.get(..LABEL.len())?;
    if !head.eq_ignore_ascii_case(LABEL) {
        return None;
    }
    let rest = core[LABEL.len()..].trim_start();
    rest.strip_prefix(':').map(str::trim_start)
}

/// The take's first sentence after a leading "Glossary:" / "Glossary."
/// label (punctuation required: "Glossary terms are…" stays).
fn strip_leading_label(sentence: &str) -> Option<String> {
    let lead = sentence.len() - sentence.trim_start().len();
    let s = &sentence[lead..];
    let head = s.get(..LABEL.len())?;
    if !head.eq_ignore_ascii_case(LABEL) {
        return None;
    }
    let rest = s[LABEL.len()..].trim_start();
    let rest = rest.strip_prefix([':', '.', '!', '?'])?;
    let rest = rest.trim_start();
    if rest.is_empty() {
        return None; // A sentence of its own: is_echo's case.
    }
    Some(rest.to_owned())
}

/// The take's last sentence without a trailing clause that is only
/// "Glossary" (capitalised, as the crate writes it) after a comma or
/// semicolon: "Thanks, Glossary." → "Thanks." ("update the glossary"
/// has no clause of its own and stays).
fn strip_trailing_label_clause(sentence: &str) -> Option<String> {
    let at = sentence.rfind([',', ';'])?;
    let clause = trim_punct(&sentence[at + 1..]);
    if clause != "Glossary" {
        return None;
    }
    let head = sentence[..at].trim_end();
    if trim_punct(head).is_empty() {
        return None;
    }
    let end: String = sentence[at + 1..]
        .trim_end()
        .chars()
        .rev()
        .take_while(|c| is_end(*c))
        .collect();
    let end = if end.is_empty() {
        String::new()
    } else {
        end.chars().rev().collect()
    };
    Some(format!("{head}{end}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn terms() -> Vec<String> {
        ["Wispr Bro", "Kubernetes", "Tauri", "Finn"]
            .map(String::from)
            .to_vec()
    }

    #[test]
    fn whisper_gets_a_plain_prompt_without_the_label() {
        let o = run_options("whisper", terms());
        assert!(o.vocabulary.is_empty());
        let p = o.prompt.as_deref().unwrap();
        assert_eq!(p, "Finn, Tauri, Kubernetes, Wispr Bro.");
        assert!(!p.to_lowercase().contains("glossary"));
        assert_eq!(
            prompted_terms(&o),
            ["Finn", "Tauri", "Kubernetes", "Wispr Bro"]
        );
    }

    #[test]
    fn other_models_keep_the_crate_vocabulary() {
        let o = run_options("parakeet", terms());
        assert_eq!(o.vocabulary, terms());
        assert!(o.prompt.is_none());
        assert!(run_options("whisper", Vec::new()).prompt.is_none());
    }

    #[test]
    fn the_prompt_drops_unsafe_terms_and_the_lowest_priority_overflow() {
        let mut t = vec![
            "Alpha".to_string(),
            "<|en|>".into(),
            "a, b".into(),
            "alpha".into(),
            " ".into(),
        ];
        assert_eq!(whisper_prompt(&t).as_deref(), Some("Alpha."));
        t = (0..200).map(|i| format!("Term{i:03}")).collect();
        let p = whisper_prompt(&t).unwrap();
        assert!(p.len() <= MAX_PROMPT_CHARS + 1);
        // The first (highest priority) term is kept, last in the prompt.
        assert!(p.ends_with("Term000."));
        assert!(!p.contains("Term199"));
    }

    #[test]
    fn a_standalone_label_at_either_end_is_removed() {
        let t = terms();
        for (input, want) in [
            ("Glossary: Let's meet at noon.", "Let's meet at noon."),
            ("glossary. Let's meet at noon.", "Let's meet at noon."),
            ("Let's meet at noon. Glossary.", "Let's meet at noon."),
            ("Let's meet at noon. Glossary:", "Let's meet at noon."),
            ("Let's meet at noon! GLOSSARY", "Let's meet at noon!"),
            ("Let's meet at noon, Glossary.", "Let's meet at noon."),
            ("Glossary.", ""),
            (" Glossary: Tauri, Finn.", ""),
        ] {
            assert_eq!(strip_echo_with(input, &t), want, "{input:?}");
        }
    }

    #[test]
    fn a_trailing_echo_of_the_terms_list_is_removed() {
        let t: Vec<String> = ["Finn", "Tauri", "Kubernetes", "Wispr Bro"]
            .map(String::from)
            .to_vec();
        for (input, want) in [
            (
                "Ship it today. Finn, Tauri, Kubernetes, Wispr Bro.",
                "Ship it today.",
            ),
            (
                "Ship it today. Tauri, Kubernetes, Wispr Bro.",
                "Ship it today.",
            ),
            (
                "Ship it today. Glossary: Kubernetes, Wispr Bro.",
                "Ship it today.",
            ),
            ("Finn, Tauri, Kubernetes, Wispr Bro.", ""),
            ("Finn, Tauri, Kubernetes. Ship it today.", "Ship it today."),
            // Whisper small on noise, with the plain prompt.
            ("Tauri, Tauri, Kubernetes, Wispr Bro.", ""),
        ] {
            assert_eq!(strip_echo_with(input, &t), want, "{input:?}");
        }
    }

    #[test]
    fn glossary_inside_a_sentence_and_short_lists_are_kept() {
        let t = terms();
        for input in [
            "Please update the glossary.",
            "Please update the glossary",
            "The glossary: it needs a new entry.",
            "Glossary terms are in the doc.",
            "Add it to the glossary. Then ship it.",
            "We need an index, glossary.",
            "Here is the glossary:",
            // Two of four terms on their own: could be speech.
            "Ship it. Kubernetes, Tauri.",
            // Terms out of the prompt's order.
            "Ship it. Tauri, Finn, Kubernetes.",
            "",
        ] {
            assert_eq!(strip_echo_with(input, &t), input, "{input:?}");
        }
    }

    #[test]
    fn nothing_is_stripped_when_no_terms_were_sent() {
        let o = RunOptions::default();
        assert_eq!(strip_echo("Glossary.".into(), &o), "Glossary.");
        let o = run_options("whisper", terms());
        assert_eq!(strip_echo("Ship it. Glossary.".into(), &o), "Ship it.");
    }
}

/// The echo reproduction (fix/gloss): a Whisper model run on short noise
/// clips and, optionally, WAVs (`WB_ECHO_AUDIO`, 16 kHz mono), once with
/// the crate's vocabulary ("Glossary: …") and once with [`run_options`].
/// Prints how many outputs carry "glossary" or the terms list, before and
/// after [`strip_echo`]. `WB_ASR_MODEL=<whisper gguf> cargo test --lib
/// echo_repro -- --ignored --nocapture`.
#[cfg(test)]
mod echo_repro {
    use super::*;

    fn noise(seed: u64, secs: f32, amp: f32) -> Vec<f32> {
        let mut x = seed
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1442695040888963407);
        let mut last = 0.0f32;
        (0..(16_000.0 * secs) as usize)
            .map(|_| {
                x = x
                    .wrapping_mul(6364136223846793005)
                    .wrapping_add(1442695040888963407);
                let w = ((x >> 33) as f32 / (1u64 << 31) as f32) * 2.0 - 1.0;
                // Some low-passed, some white.
                last = if seed.is_multiple_of(2) {
                    0.9 * last + 0.1 * w
                } else {
                    w
                };
                last * amp
            })
            .collect()
    }

    /// Independent of the guard: the label anywhere, or a sentence of two
    /// or more comma-separated parts that are all terms, in any order.
    fn echoed(text: &str, terms: &[String]) -> bool {
        let lower: Vec<String> = terms.iter().map(|t| t.to_lowercase()).collect();
        text.to_lowercase().contains(LABEL)
            || split_sentences(text).iter().any(|s| {
                let parts: Vec<String> = trim_punct(s)
                    .split(',')
                    .map(|p| trim_punct(p).to_lowercase())
                    .filter(|p| !p.is_empty())
                    .collect();
                parts.len() >= 2 && parts.iter().all(|p| lower.contains(p))
            })
    }

    #[test]
    #[ignore = "needs WB_ASR_MODEL (a Whisper GGUF)"]
    fn echo_repro() {
        let Some(model) = std::env::var_os("WB_ASR_MODEL") else {
            return;
        };
        transcribe_cpp::init_backends_default().expect("backends");
        let model = transcribe_cpp::Model::load(model).expect("model");
        let mut session = model.session().expect("session");
        let terms: Vec<String> = [
            "Wispr Bro",
            "Kubernetes",
            "Tauri",
            "Siobhán",
            "PostgreSQL",
            "Constor",
            "Finn Jennen",
        ]
        .map(String::from)
        .to_vec();
        let mut clips: Vec<(String, Vec<f32>)> = Vec::new();
        clips.push(("silence".into(), vec![0.0; 32_000]));
        for seed in 0..24u64 {
            let amp = [0.002, 0.01, 0.03, 0.08][(seed % 4) as usize];
            clips.push((
                format!("noise{seed}"),
                noise(seed, 1.0 + (seed % 3) as f32, amp),
            ));
        }
        if let Some(dir) = std::env::var_os("WB_ECHO_AUDIO") {
            let mut paths: Vec<_> = std::fs::read_dir(dir)
                .unwrap()
                .filter_map(|e| e.ok().map(|e| e.path()))
                .filter(|p| p.extension().is_some_and(|x| x == "wav"))
                .collect();
            paths.sort();
            for p in paths {
                let pcm = crate::audio_toolkit::read_wav_samples(&p).expect("wav");
                clips.push((p.file_name().unwrap().to_string_lossy().into(), pcm));
            }
        }
        let old = RunOptions {
            vocabulary: terms.clone(),
            ..Default::default()
        };
        let new = run_options("whisper", terms.clone());
        let (mut old_hits, mut new_hits, mut old_left, mut new_left) = (0, 0, 0, 0);
        for (name, pcm) in &clips {
            let a = session.run(pcm, &old).map(|t| t.text).unwrap_or_default();
            let b = session.run(pcm, &new).map(|t| t.text).unwrap_or_default();
            let (sa, sb) = (strip_echo(a.clone(), &old), strip_echo(b.clone(), &new));
            old_hits += echoed(&a, &terms) as u32;
            new_hits += echoed(&b, &terms) as u32;
            old_left += echoed(&sa, &terms) as u32;
            new_left += echoed(&sb, &terms) as u32;
            println!("{name}\n  glossary: {a:?} -> {sa:?}\n  prompt:   {b:?} -> {sb:?}");
        }
        println!(
            "echo_repro: {} clips; label/list echo with Glossary vocabulary {old_hits} ({old_left} after the guard), with the plain prompt {new_hits} ({new_left} after the guard)",
            clips.len()
        );
    }
}
