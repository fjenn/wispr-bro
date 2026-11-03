// SPDX-License-Identifier: GPL-3.0-or-later
//! Rust-side fork strings (DEC-35).
//!
//! Tray, app-menu, window, notification and VoiceOver strings live in the same
//! `src/i18n/locales/<lng>/fork.json` files as the React strings, under the
//! top-level `rust` object. All 26 files are embedded at compile time with
//! `include_str!`, so a missing locale file is a build error.
//!
//! Locale: Handy's app-language setting (`AppSettings::app_language`, which
//! itself defaults to the OS locale), then the OS locale, then English. A key
//! missing from a locale falls back to English (`wb-translate-missing.ts`
//! keeps the files in parity, so that only happens mid-edit).
//!
//! Plurals (S-37b): a string that shows a count is a group of keys with
//! i18next's suffixes, `<base>_one`/`<base>_other` in English and each
//! locale's CLDR cardinal categories elsewhere (`_zero`, `_two`, `_few`,
//! `_many` too). [`t_plural`] picks the form with ICU4X's CLDR rules, the
//! same data the frontend gets from `Intl.PluralRules`.

use std::collections::HashMap;

use icu_locale_core::Locale;
use icu_plurals::{PluralCategory, PluralRules};
use once_cell::sync::Lazy;
use serde_json::Value;

use crate::chinese_script::chinese_script_for_locale;
use crate::settings::ChineseScript;

/// English, the source locale.
pub const SOURCE_LOCALE: &str = "en";

macro_rules! embed_locales {
    ($($code:literal),+ $(,)?) => {
        &[$(
            (
                $code,
                include_str!(concat!(
                    env!("CARGO_MANIFEST_DIR"),
                    "/../src/i18n/locales/",
                    $code,
                    "/fork.json"
                )),
            ),
        )+]
    };
}

/// `(locale code, fork.json source)` for all 26 locales.
pub(crate) static EMBEDDED: &[(&str, &str)] = embed_locales!(
    "ar", "bg", "ca", "cs", "da", "de", "en", "es", "fr", "he", "hi", "id", "it", "ja", "ko", "ne",
    "nl", "pl", "pt", "ru", "sv", "tr", "uk", "vi", "zh", "zh-TW",
);

/// Parsed `rust` objects by locale code.
static STRINGS: Lazy<HashMap<&'static str, Value>> = Lazy::new(|| {
    EMBEDDED
        .iter()
        .map(|(code, src)| {
            let rust = serde_json::from_str::<Value>(src)
                .ok()
                .and_then(|v| v.get("rust").cloned())
                .unwrap_or(Value::Null);
            if rust.is_null() {
                log::error!("fork i18n: {code}/fork.json has no valid `rust` object");
            }
            (*code, rust)
        })
        .collect()
});

/// Supported locale codes.
pub fn locales() -> impl Iterator<Item = &'static str> {
    EMBEDDED.iter().map(|(code, _)| *code)
}

/// Maps a BCP-47-ish tag (`de-AT`, `zh_Hant_TW`, `pt-BR`) to a supported
/// locale code, or `None`. Same rules as Handy's tray (`tray_i18n.rs`): exact
/// match, then the Chinese script/region fallback, then the language subtag.
pub fn supported_locale(tag: &str) -> Option<&'static str> {
    let normalized = tag.trim().to_lowercase().replace('_', "-");
    if normalized.is_empty() {
        return None;
    }
    if let Some(code) = locales().find(|c| c.eq_ignore_ascii_case(&normalized)) {
        return Some(code);
    }
    let fallback = match chinese_script_for_locale(&normalized) {
        Some(ChineseScript::TraditionalTw | ChineseScript::TraditionalHk) => "zh-TW",
        Some(_) => "zh",
        None => normalized.split('-').next().unwrap_or_default(),
    };
    locales().find(|c| c.eq_ignore_ascii_case(fallback))
}

/// Resolves the fork locale: Handy's app language, then the OS locale, then
/// English.
pub fn resolve_locale(app_language: Option<&str>, os_locale: Option<&str>) -> &'static str {
    app_language
        .and_then(supported_locale)
        .or_else(|| os_locale.and_then(supported_locale))
        .unwrap_or(SOURCE_LOCALE)
}

/// [`resolve_locale`] with the live OS locale.
pub fn current_locale(app_language: Option<&str>) -> &'static str {
    let os = tauri_plugin_os::locale();
    resolve_locale(app_language, os.as_deref())
}

fn lookup<'a>(root: &'a Value, key: &str) -> Option<&'a str> {
    let path = key.strip_prefix("rust.").unwrap_or(key);
    path.split('.')
        .try_fold(root, |node, part| node.get(part))
        .and_then(Value::as_str)
        .filter(|s| !s.trim().is_empty())
}

/// Raw string for `key` (`"rust.tray.version"` or `"tray.version"`) in
/// `locale`, falling back to English. `None` only when English lacks the key.
pub fn get(locale: &str, key: &str) -> Option<&'static str> {
    let code = supported_locale(locale).unwrap_or(SOURCE_LOCALE);
    STRINGS
        .get(code)
        .and_then(|root| lookup(root, key))
        .or_else(|| STRINGS.get(SOURCE_LOCALE).and_then(|r| lookup(r, key)))
}

/// Translates `key` and fills `{{name}}` placeholders from `vars`. Returns the
/// key itself when even English lacks it (so a typo is visible, not blank).
pub fn t(locale: &str, key: &str, vars: &[(&str, &str)]) -> String {
    let Some(template) = get(locale, key) else {
        log::warn!("fork i18n: missing key {key}");
        return key.to_string();
    };
    interpolate(template, vars)
}

/// i18next-style `{{name}}` interpolation (whitespace inside braces allowed).
pub fn interpolate(template: &str, vars: &[(&str, &str)]) -> String {
    let mut out = String::with_capacity(template.len());
    let mut rest = template;
    while let Some(start) = rest.find("{{") {
        out.push_str(&rest[..start]);
        let after = &rest[start + 2..];
        let Some(end) = after.find("}}") else {
            out.push_str(&rest[start..]);
            return out;
        };
        let name = after[..end].trim();
        match vars.iter().find(|(k, _)| *k == name) {
            Some((_, v)) => out.push_str(v),
            None => out.push_str(&rest[start..start + 2 + end + 2]),
        }
        rest = &after[end + 2..];
    }
    out.push_str(rest);
    out
}

/// i18next's plural suffixes, in CLDR order.
pub const PLURAL_SUFFIXES: [&str; 6] = ["zero", "one", "two", "few", "many", "other"];

/// Cardinal plural rules for a supported locale code (CLDR data compiled
/// into ICU4X; building them is a cheap lookup in static data, and the
/// type is not `Sync`, so they are not cached in a static).
fn plural_rules(code: &str) -> Option<PluralRules> {
    let locale = Locale::try_from_str(code).ok()?;
    PluralRules::try_new_cardinal((&locale).into()).ok()
}

fn category_suffix(category: PluralCategory) -> &'static str {
    match category {
        PluralCategory::Zero => "zero",
        PluralCategory::One => "one",
        PluralCategory::Two => "two",
        PluralCategory::Few => "few",
        PluralCategory::Many => "many",
        PluralCategory::Other => "other",
    }
}

/// The CLDR cardinal category (`"one"`, `"few"`, …) of `count` in `locale`;
/// `"other"` for a locale without rules.
pub fn plural_category(locale: &str, count: u64) -> &'static str {
    let code = supported_locale(locale).unwrap_or(SOURCE_LOCALE);
    plural_rules(code)
        .map(|rules| category_suffix(rules.category_for(count)))
        .unwrap_or("other")
}

/// `key` without its plural suffix (`"a.b_few"` → `Some("a.b")`).
pub fn plural_base(key: &str) -> Option<&str> {
    let (base, suffix) = key.rsplit_once('_')?;
    PLURAL_SUFFIXES.contains(&suffix).then_some(base)
}

/// Translates the plural group `base` for `count`: the locale's own form
/// for its CLDR category, else English's form for English's category (a
/// locale mid-edit), with `{{count}}` and `vars` filled.
pub fn t_plural(locale: &str, base: &str, count: u64, vars: &[(&str, &str)]) -> String {
    let code = supported_locale(locale).unwrap_or(SOURCE_LOCALE);
    let local = STRINGS
        .get(code)
        .and_then(|root| lookup(root, &format!("{base}_{}", plural_category(code, count))));
    let count_text = count.to_string();
    let mut all: Vec<(&str, &str)> = vec![("count", count_text.as_str())];
    all.extend_from_slice(vars);
    match local {
        Some(template) => interpolate(template, &all),
        None => t(
            SOURCE_LOCALE,
            &format!("{base}_{}", plural_category(SOURCE_LOCALE, count)),
            &all,
        ),
    }
}

/// Every `rust.*` key of the English source, dotted (`rust.tray.version`).
pub fn english_keys() -> Vec<String> {
    fn walk(prefix: &str, v: &Value, out: &mut Vec<String>) {
        match v {
            Value::Object(map) => {
                for (k, child) in map {
                    walk(&format!("{prefix}.{k}"), child, out);
                }
            }
            Value::String(_) => out.push(prefix.to_string()),
            _ => {}
        }
    }
    let mut out = Vec::new();
    if let Some(root) = STRINGS.get(SOURCE_LOCALE) {
        walk("rust", root, &mut out);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn placeholders(s: &str) -> Vec<String> {
        let mut out = Vec::new();
        let mut rest = s;
        while let Some(start) = rest.find("{{") {
            let after = &rest[start + 2..];
            let Some(end) = after.find("}}") else { break };
            out.push(after[..end].trim().to_string());
            rest = &after[end + 2..];
        }
        out.sort();
        out
    }

    #[test]
    fn embeds_all_26_locale_directories() {
        let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../src/i18n/locales");
        let mut on_disk: Vec<String> = std::fs::read_dir(dir)
            .unwrap()
            .filter_map(|e| e.ok())
            .filter(|e| e.path().join("fork.json").exists())
            .map(|e| e.file_name().to_string_lossy().into_owned())
            .collect();
        on_disk.sort();
        let mut embedded: Vec<String> = locales().map(String::from).collect();
        embedded.sort();
        assert_eq!(embedded.len(), 26);
        assert_eq!(embedded, on_disk);
    }

    #[test]
    fn every_rust_key_resolves_in_every_locale() {
        let keys = english_keys();
        assert!(!keys.is_empty(), "en/fork.json has no rust.* keys");
        for code in locales() {
            let root = STRINGS.get(code).unwrap();
            assert!(!root.is_null(), "{code}: no rust object");
            // Plural forms differ per language; the plural test covers them.
            for key in keys.iter().filter(|k| plural_base(k).is_none()) {
                let local =
                    lookup(root, key).unwrap_or_else(|| panic!("{code}: {key} missing or empty"));
                let english = get(SOURCE_LOCALE, key).unwrap();
                assert_eq!(
                    placeholders(local),
                    placeholders(english),
                    "{code}: {key} placeholders differ from English"
                );
            }
        }
    }

    #[test]
    fn plural_categories_follow_cldr() {
        let cases: &[(&str, &[(u64, &str)])] = &[
            (
                "en",
                &[(0, "other"), (1, "one"), (2, "other"), (21, "other")],
            ),
            ("de", &[(1, "one"), (5, "other")]),
            ("fr", &[(0, "one"), (1, "one"), (2, "other")]),
            ("ja", &[(1, "other"), (5, "other")]),
            ("zh-TW", &[(1, "other")]),
            (
                "ru",
                &[
                    (1, "one"),
                    (2, "few"),
                    (5, "many"),
                    (11, "many"),
                    (21, "one"),
                    (22, "few"),
                ],
            ),
            ("uk", &[(1, "one"), (3, "few"), (12, "many")]),
            (
                "pl",
                &[
                    (1, "one"),
                    (2, "few"),
                    (5, "many"),
                    (21, "many"),
                    (22, "few"),
                ],
            ),
            ("cs", &[(1, "one"), (3, "few"), (5, "other")]),
            ("he", &[(1, "one"), (2, "two"), (3, "other")]),
            (
                "ar",
                &[
                    (0, "zero"),
                    (1, "one"),
                    (2, "two"),
                    (3, "few"),
                    (11, "many"),
                    (100, "other"),
                ],
            ),
            ("pt-BR", &[(0, "one"), (2, "other")]),
        ];
        for (locale, expected) in cases {
            for (n, category) in *expected {
                assert_eq!(plural_category(locale, *n), *category, "{locale} {n}");
            }
        }
    }

    #[test]
    fn plural_base_strips_only_plural_suffixes() {
        assert_eq!(plural_base("rust.a.title_few"), Some("rust.a.title"));
        assert_eq!(plural_base("rust.a.title_other"), Some("rust.a.title"));
        assert_eq!(plural_base("rust.a.title"), None);
        assert_eq!(plural_base("rust.a.read_only"), None);
    }

    /// Every `rust.*` plural group has, in every locale, exactly the forms
    /// ICU4X's CLDR data names for that language, so `t_plural` never falls
    /// back. `wb-translate-missing.ts` writes them from the browser's
    /// `Intl.PluralRules`; this also catches the two CLDR copies disagreeing.
    #[test]
    fn every_rust_plural_group_has_the_locale_forms() {
        let bases: std::collections::BTreeSet<String> = english_keys()
            .iter()
            .filter_map(|k| plural_base(k).map(String::from))
            .collect();
        assert!(bases.contains("rust.menu.wordsDictated"));
        for code in locales() {
            let root = STRINGS.get(code).unwrap();
            let wanted: Vec<&str> = if code == SOURCE_LOCALE {
                vec!["one", "other"]
            } else {
                let rules = plural_rules(code).expect("plural rules");
                PLURAL_SUFFIXES
                    .iter()
                    .copied()
                    .filter(|s| rules.categories().any(|c| category_suffix(c) == *s))
                    .collect()
            };
            for base in &bases {
                for suffix in PLURAL_SUFFIXES {
                    let key = format!("{base}_{suffix}");
                    let has = lookup(root, &key).is_some();
                    assert_eq!(
                        has,
                        wanted.contains(&suffix),
                        "{code}: {key} {}",
                        if has { "is extra" } else { "is missing" }
                    );
                }
            }
        }
    }

    #[test]
    fn t_plural_picks_the_locale_form_and_fills_count() {
        assert_eq!(
            t_plural("en", "rust.menu.wordsDictated", 1, &[]),
            "1 word dictated"
        );
        assert_eq!(
            t_plural("en", "rust.menu.wordsDictated", 1234, &[]),
            "1234 words dictated"
        );
        for n in [1, 2, 5, 21] {
            let ru = t_plural("ru", "rust.menu.wordsDictated", n, &[]);
            let form = get(
                "ru",
                &format!("rust.menu.wordsDictated_{}", plural_category("ru", n)),
            )
            .unwrap();
            assert_eq!(ru, interpolate(form, &[("count", &n.to_string())]));
        }
        // Unknown locale → English rules and strings.
        assert_eq!(
            t_plural("xx", "rust.menu.snippetCount", 2, &[]),
            "2 snippets"
        );
    }

    #[test]
    fn no_rust_string_names_handy() {
        for code in locales() {
            for key in english_keys() {
                let Some(s) = get(code, &key) else { continue };
                assert!(!s.contains("Handy"), "{code}: {key} = {s:?}");
            }
        }
    }

    #[test]
    fn locale_resolution_order() {
        assert_eq!(resolve_locale(Some("de"), Some("fr-FR")), "de");
        assert_eq!(resolve_locale(Some("de-AT"), None), "de");
        assert_eq!(resolve_locale(Some("zh-Hant-TW"), None), "zh-TW");
        assert_eq!(resolve_locale(Some("zh_CN"), None), "zh");
        assert_eq!(resolve_locale(Some("pt-BR"), None), "pt");
        // Unsupported app language → OS locale → English.
        assert_eq!(resolve_locale(Some("xx"), Some("ja-JP")), "ja");
        assert_eq!(resolve_locale(None, Some("xx")), "en");
        assert_eq!(resolve_locale(Some(""), None), "en");
    }

    #[test]
    fn interpolation() {
        assert_eq!(
            interpolate("A v{{version}}", &[("version", "1.0")]),
            "A v1.0"
        );
        assert_eq!(
            interpolate("{{ a }}-{{b}}", &[("a", "x"), ("b", "y")]),
            "x-y"
        );
        assert_eq!(interpolate("keep {{missing}}", &[]), "keep {{missing}}");
        assert_eq!(interpolate("open {{", &[]), "open {{");
        assert_eq!(t("en", "rust.no.such.key", &[]), "rust.no.such.key");
    }
}
