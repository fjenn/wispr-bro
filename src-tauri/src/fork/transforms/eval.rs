// SPDX-License-Identifier: GPL-3.0-or-later
//! The transforms eval (OT2-D): `eval/transforms/items.jsonl` holds realistic
//! custom transforms (created through the editor's validate + create path)
//! and built-in runs, each with code-checked pass criteria. No LLM judge.
//!
//! ```text
//! WB_LLAMA_SERVER=<sidecar> WB_LLM_MODEL=<gguf> WB_EVAL_OUT=<run.jsonl> \
//!   [WB_XFORM_RULES=new|legacy] \
//!   cargo test --lib fork::transforms::eval -- --ignored --nocapture
//! ```
//!
//! `WB_XFORM_RULES=new` (default) creates each custom transform with the
//! rules a new transform starts with today; `legacy` uses the rc3 defaults
//! (Shorter, Clearer wording, Better order and Keep my tone on), i.e. what a
//! transform made before OT2-D carries.

use serde_json::Value;

use crate::fork::transforms::store::TransformRules;

pub const ITEMS: &str = include_str!("../../../../eval/transforms/items.jsonl");

pub fn items() -> Vec<Value> {
    ITEMS
        .lines()
        .filter(|line| !line.trim().is_empty())
        .map(|line| serde_json::from_str(line).expect("eval item is JSON"))
        .collect()
}

/// The rules every transform got from the rc3 editor's "Create new".
pub fn legacy_rules() -> TransformRules {
    TransformRules::default()
}

pub fn is_emoji(c: char) -> bool {
    matches!(c as u32,
        0x1F300..=0x1FAFF | 0x2600..=0x27BF | 0x1F000..=0x1F2FF | 0xFE0F | 0x200D)
}

fn sentences(text: &str) -> usize {
    let re = regex::Regex::new(r"[.!?。！？]+(\s|$)").expect("static regex");
    re.split(text.trim())
        .filter(|part| part.chars().any(char::is_alphanumeric))
        .count()
}

fn bullets(text: &str) -> usize {
    let re = regex::Regex::new(r"^\s*([-*•–]|\d+[.)])\s+\S").expect("static regex");
    text.lines().filter(|line| re.is_match(line)).count()
}

/// The most likely of the eval's languages (no confidence gate: the items
/// are one sentence long, below the product detector's reliability bar).
fn language(text: &str) -> Option<&'static str> {
    use whatlang::Lang;
    const LANGS: [(Lang, &str); 7] = [
        (Lang::Eng, "en"),
        (Lang::Spa, "es"),
        (Lang::Deu, "de"),
        (Lang::Fra, "fr"),
        (Lang::Por, "pt"),
        (Lang::Ita, "it"),
        (Lang::Nld, "nl"),
    ];
    let detected = whatlang::Detector::with_allowlist(LANGS.iter().map(|(l, _)| *l).collect())
        .detect_lang(text)?;
    LANGS
        .iter()
        .find(|(l, _)| *l == detected)
        .map(|(_, code)| *code)
}

/// Every failed check of one item, empty when the output passes.
pub fn failures(item: &Value, output: &str) -> Vec<String> {
    let input = item["input"].as_str().unwrap_or_default();
    let out = output.trim();
    let lower = out.to_lowercase();
    let mut failed = Vec::new();
    for check in item["checks"].as_array().into_iter().flatten() {
        let value = check["value"].as_str().unwrap_or_default();
        let values = || {
            check["values"]
                .as_array()
                .into_iter()
                .flatten()
                .filter_map(Value::as_str)
                .map(str::to_owned)
                .collect::<Vec<_>>()
        };
        let kind = check["type"].as_str().unwrap_or_default();
        let ok = match kind {
            "equals" => out == value.trim(),
            "contains" => out.contains(value),
            "not_contains" => !out.contains(value),
            "contains_ci" => lower.contains(&value.to_lowercase()),
            "not_contains_ci" => !lower.contains(&value.to_lowercase()),
            "regex" => regex::Regex::new(value).expect("check regex").is_match(out),
            "not_regex" => !regex::Regex::new(value).expect("check regex").is_match(out),
            "keeps" => values().iter().all(|word| out.contains(word.as_str())),
            "keeps_ci" => values()
                .iter()
                .all(|word| lower.contains(&word.to_lowercase())),
            "changed" => out != input.trim(),
            "no_emoji" => !out.chars().any(is_emoji),
            "has_emoji" => out.chars().any(is_emoji),
            "bullets" => bullets(out) >= check["min"].as_u64().unwrap_or(2) as usize,
            "max_sentences" => sentences(out) <= check["value"].as_u64().unwrap_or(1) as usize,
            "ratio" => {
                let ratio = out.chars().count() as f64 / input.chars().count().max(1) as f64;
                ratio >= check["min"].as_f64().unwrap_or(0.0)
                    && ratio <= check["max"].as_f64().unwrap_or(f64::MAX)
            }
            "language" => language(out) == Some(value),
            other => panic!("unknown eval check {other}"),
        };
        if !ok {
            failed.push(match check.get("value").or_else(|| check.get("values")) {
                Some(v) => format!("{kind} {v}"),
                None => kind.to_owned(),
            });
        }
    }
    failed
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;

    #[test]
    fn items_are_well_formed_and_their_checks_discriminate() {
        let items = items();
        assert!(items.len() >= 30, "{} items", items.len());
        let ids: HashSet<_> = items.iter().map(|i| i["id"].as_str().unwrap()).collect();
        assert_eq!(ids.len(), items.len(), "duplicate ids");
        assert!(items.iter().any(|i| i["kind"] == "builtin"));
        for item in &items {
            let id = item["id"].as_str().unwrap();
            let reference = item["reference"].as_str().unwrap();
            assert_eq!(
                failures(item, reference),
                Vec::<String>::new(),
                "{id}: its reference must pass"
            );
            let input = item["input"].as_str().unwrap();
            // Polish on clean text may legitimately return its input.
            if id != "polish-clean" && id != "polish-entities" {
                assert!(
                    !failures(item, input).is_empty(),
                    "{id}: returning the input unchanged must fail"
                );
            }
        }
    }

    #[test]
    fn checks_count_sentences_bullets_and_emoji() {
        assert_eq!(sentences("One. Two! Three?"), 3);
        assert_eq!(sentences("Version 1.2 ships today."), 1);
        assert_eq!(bullets("- a\n* b\n1. c\nplain"), 3);
        assert!(is_emoji('🎉') && !is_emoji('a'));
    }

    #[tokio::test(flavor = "multi_thread")]
    #[ignore = "needs WB_LLAMA_SERVER, WB_LLM_MODEL and WB_EVAL_OUT"]
    async fn transforms_eval_through_the_shipped_path() {
        use crate::fork::db::ForkDb;
        use crate::fork::llm::chat::CancellationToken;
        use crate::fork::llm::post_process::LocalEngine;
        use crate::fork::llm::supervisor::{Config, ProcessLauncher, Supervisor};
        use crate::fork::transforms::{run, store, TransformKind};
        use std::io::Write;
        use std::sync::Arc;
        use std::time::Instant;

        let (Ok(bin), Ok(model), Ok(out)) = (
            std::env::var("WB_LLAMA_SERVER"),
            std::env::var("WB_LLM_MODEL"),
            std::env::var("WB_EVAL_OUT"),
        ) else {
            eprintln!("transforms eval: set WB_LLAMA_SERVER, WB_LLM_MODEL and WB_EVAL_OUT");
            return;
        };
        let legacy = std::env::var("WB_XFORM_RULES").as_deref() == Ok("legacy");
        let dir = tempfile::tempdir().unwrap();
        let mut config = Config::new(bin.into(), dir.path().join("llm"));
        config.model = Some(model.clone().into());
        let supervisor = Arc::new(Supervisor::new(Arc::new(ProcessLauncher), config));
        supervisor.ensure_ready().await.expect("sidecar starts");
        let engine = LocalEngine {
            supervisor: &supervisor,
            pill: false,
        };
        let db = ForkDb::in_dir(dir.path());
        let mut file = std::fs::File::create(out).unwrap();
        let items = items();
        let mut passed = (0usize, 0usize, 0usize, 0usize);
        for item in &items {
            let id = item["id"].as_str().unwrap();
            let source = item["input"].as_str().unwrap();
            let definition = db
                .with_conn(|conn| {
                    if item["kind"] == "builtin" {
                        let kind = TransformKind::ALL
                            .into_iter()
                            .find(|kind| item["builtin"] == kind.builtin_key())
                            .unwrap();
                        return store::definition(conn, kind);
                    }
                    let mut input = store::TransformInput {
                        name: id.to_owned(),
                        description: String::new(),
                        rules: store::new_transform_rules(),
                        prompt: item["prompt"].as_str().unwrap().to_owned(),
                        examples: Vec::new(),
                        replacements: Vec::new(),
                    };
                    if legacy {
                        input.rules = legacy_rules();
                    }
                    let input = store::validate(&input).expect("valid transform");
                    let entry = store::create(conn, &input, 0)?;
                    store::definition_by_id(conn, entry.id)
                })
                .unwrap()
                .unwrap();
            let started = Instant::now();
            let result = run::generate_definition(
                Some(&engine),
                &definition,
                source,
                &CancellationToken::new(),
            )
            .await;
            let ms = started.elapsed().as_millis() as u64;
            let (output, error) = match &result {
                Ok(generated) => (Some(generated.text.clone()), None),
                Err(error) => (None, Some(format!("{error:?}"))),
            };
            // Polish on text that needs nothing may say so instead.
            let unchanged_ok = item["unchanged_ok"].as_bool().unwrap_or(false)
                && error.as_deref() == Some("Unchanged");
            let failed = match &output {
                Some(text) => failures(item, text),
                None if unchanged_ok => Vec::new(),
                None => vec![error.clone().unwrap_or_default()],
            };
            let pass = failed.is_empty();
            let builtin = item["kind"] == "builtin";
            if builtin {
                passed.2 += 1;
                passed.3 += usize::from(pass);
            } else {
                passed.0 += 1;
                passed.1 += usize::from(pass);
            }
            let row = serde_json::json!({
                "id": id, "kind": item["kind"], "input": source, "output": output,
                "error": error, "pass": pass, "failed": failed, "ms": ms,
            });
            writeln!(file, "{row}").unwrap();
            eprintln!(
                "transforms eval: {:<20} {} {ms:>5} ms {}",
                id,
                if pass { "PASS" } else { "FAIL" },
                if pass {
                    String::new()
                } else {
                    format!("{failed:?} -> {output:?}")
                }
            );
        }
        eprintln!(
            "transforms eval ({}): custom {}/{}, built-in {}/{}, total {}/{}",
            if legacy { "legacy rules" } else { "new rules" },
            passed.1,
            passed.0,
            passed.3,
            passed.2,
            passed.1 + passed.3,
            passed.0 + passed.2
        );
    }
}
