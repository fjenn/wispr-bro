// SPDX-License-Identifier: GPL-3.0-or-later
//! App taxonomies (R5 §4.3): which `style_group`, `usage_category` and
//! `is_ai_chat` an app has. The data is `categories.json` (OpenWhispr and
//! AudioWhisper data, MIT; see its `_sources`); the matching rule is
//! OpenWhispr's `toneAppCategories.js` (whole-token match of a signature in
//! the app's name and bundle id, so "mail" matches "com.apple.mail" and
//! "Mail" but not "Mailplane").
//!
//! An app has exactly one value of each. A host rule for a supported
//! browser's page (S-19b) overrides the browser's app rule.

use std::collections::HashMap;
use std::sync::OnceLock;

use serde::{Deserialize, Serialize};

/// Styles group (S-19a). Values match the `entry_meta.style_group` CHECK.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "snake_case")]
pub enum StyleGroup {
    Personal,
    Work,
    Email,
    Other,
}

impl StyleGroup {
    pub fn as_str(self) -> &'static str {
        match self {
            StyleGroup::Personal => "personal",
            StyleGroup::Work => "work",
            StyleGroup::Email => "email",
            StyleGroup::Other => "other",
        }
    }
}

/// Insights usage bucket (S-25). Values match the `usage_category` CHECKs.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "snake_case")]
pub enum UsageCategory {
    Documents,
    AiPrompts,
    Email,
    Work,
    Personal,
    Other,
}

impl UsageCategory {
    /// Every category, in the order Insights lists them (S-25).
    pub const ALL: [UsageCategory; 6] = [
        UsageCategory::Documents,
        UsageCategory::AiPrompts,
        UsageCategory::Email,
        UsageCategory::Work,
        UsageCategory::Personal,
        UsageCategory::Other,
    ];

    /// The category stored as `s`, if it is one.
    pub fn parse(s: &str) -> Option<UsageCategory> {
        Self::ALL.into_iter().find(|c| c.as_str() == s)
    }

    pub fn as_str(self) -> &'static str {
        match self {
            UsageCategory::Documents => "documents",
            UsageCategory::AiPrompts => "ai_prompts",
            UsageCategory::Email => "email",
            UsageCategory::Work => "work",
            UsageCategory::Personal => "personal",
            UsageCategory::Other => "other",
        }
    }

    /// The usage category an app matched only by its style group gets.
    fn for_group(group: StyleGroup) -> UsageCategory {
        match group {
            StyleGroup::Personal => UsageCategory::Personal,
            StyleGroup::Work => UsageCategory::Work,
            StyleGroup::Email => UsageCategory::Email,
            StyleGroup::Other => UsageCategory::Other,
        }
    }
}

/// What the taxonomy says about one app.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct AppClass {
    pub style_group: StyleGroup,
    pub usage_category: UsageCategory,
    pub is_ai_chat: bool,
    /// A web browser: its page decides (S-19b host rules).
    pub is_browser: bool,
    /// How the answer was found (tests, logs).
    pub matched_by: MatchedBy,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MatchedBy {
    BundleId,
    Signature,
    Browser,
    Default,
}

impl AppClass {
    pub const OTHER: AppClass = AppClass {
        style_group: StyleGroup::Other,
        usage_category: UsageCategory::Other,
        is_ai_chat: false,
        is_browser: false,
        matched_by: MatchedBy::Default,
    };
}

#[derive(Debug, Deserialize)]
struct AppRule {
    style_group: StyleGroup,
    usage_category: UsageCategory,
    #[serde(default)]
    is_ai_chat: bool,
}

#[derive(Debug, Deserialize)]
struct HostRule {
    name: String,
    style_group: StyleGroup,
    usage_category: UsageCategory,
    #[serde(default)]
    title_aliases: Vec<String>,
}

#[derive(Debug, Deserialize)]
struct Data {
    apps: HashMap<String, AppRule>,
    hosts: HashMap<String, HostRule>,
    signature_order: Vec<StyleGroup>,
    signatures: HashMap<StyleGroup, Vec<String>>,
    browsers: Vec<String>,
}

const DATA_JSON: &str = include_str!("categories.json");

struct Table {
    /// Lower-cased bundle id → rule.
    apps: HashMap<String, AppRule>,
    /// Lower-cased host → rule, sorted for deterministic title fallback.
    hosts: Vec<(String, HostRule)>,
    signatures: Vec<(StyleGroup, Vec<String>)>,
    browsers: Vec<String>,
    /// Every bundle id of `apps` as written there, with its group, sorted.
    known: Vec<(String, StyleGroup)>,
    /// Every built-in site's host, display name and group (the Style page).
    known_hosts: Vec<(String, String, StyleGroup)>,
}

fn parse(json: &str) -> Result<Table, String> {
    let data: Data = serde_json::from_str(json).map_err(|e| e.to_string())?;
    let mut signatures = Vec::new();
    for group in data.signature_order {
        let list = data
            .signatures
            .get(&group)
            .ok_or_else(|| format!("signature_order names {group:?} with no signatures"))?;
        signatures.push((group, list.iter().map(|s| s.to_lowercase()).collect()));
    }
    let mut known: Vec<(String, StyleGroup)> = data
        .apps
        .iter()
        .map(|(k, v)| (k.clone(), v.style_group))
        .collect();
    known.sort_by(|a, b| a.0.cmp(&b.0));
    let mut hosts: Vec<(String, HostRule)> = data
        .hosts
        .into_iter()
        .map(|(host, rule)| (host.to_lowercase(), rule))
        .collect();
    hosts.sort_by(|a, b| a.0.cmp(&b.0));
    let known_hosts = hosts
        .iter()
        .map(|(host, rule)| (host.clone(), rule.name.clone(), rule.style_group))
        .collect();
    Ok(Table {
        known,
        known_hosts,
        hosts,
        apps: data
            .apps
            .into_iter()
            .map(|(k, v)| (k.to_lowercase(), v))
            .collect(),
        signatures,
        browsers: data.browsers.iter().map(|s| s.to_lowercase()).collect(),
    })
}

fn table() -> &'static Table {
    static TABLE: OnceLock<Table> = OnceLock::new();
    TABLE.get_or_init(|| {
        // A test (`shipped_table_parses`) proves this never fails.
        parse(DATA_JSON).expect("categories.json is valid")
    })
}

/// `entry` occurs in `signature` with no letter or digit on either side
/// (OpenWhispr `tokenPattern`).
fn has_token(signature: &str, entry: &str) -> bool {
    let bytes = signature.as_bytes();
    signature.match_indices(entry).any(|(at, _)| {
        let before = at.checked_sub(1).map(|i| bytes[i]);
        let after = bytes.get(at + entry.len()).copied();
        let boundary = |b: Option<u8>| b.is_none_or(|b| !b.is_ascii_alphanumeric());
        boundary(before) && boundary(after)
    })
}

/// Classifies an app by its bundle id and (localized) name. Either may be
/// missing; with neither the answer is [`AppClass::OTHER`].
pub fn classify(bundle_id: Option<&str>, app_name: Option<&str>) -> AppClass {
    classify_in(table(), bundle_id, app_name)
}

/// The built-in group and usage category of a URL's host or parent domain.
pub fn classify_host(url: &str) -> Option<AppClass> {
    let host = super::url::host(url)?;
    table().hosts.iter().find_map(|(pattern, rule)| {
        (host == *pattern || host.ends_with(&format!(".{pattern}"))).then_some(AppClass {
            style_group: rule.style_group,
            usage_category: rule.usage_category,
            is_ai_chat: false,
            is_browser: true,
            matched_by: MatchedBy::Browser,
        })
    })
}

/// A title fallback can name only a built-in host, never arbitrary title text.
pub fn host_from_title(title: &str) -> Option<String> {
    let title = title.to_lowercase();
    table().hosts.iter().find_map(|(host, rule)| {
        std::iter::once(host)
            .chain(rule.title_aliases.iter())
            .any(|alias| !alias.is_empty() && has_token(&title, &alias.to_lowercase()))
            .then(|| host.clone())
    })
}

/// The bundle ids of the built-in table (`apps`) with their style group
/// (the Style page lists the installed ones, S-19a).
pub fn known_apps() -> &'static [(String, StyleGroup)] {
    &table().known
}

/// Built-in sites with their display names and groups (S-19b).
pub fn known_hosts() -> &'static [(String, String, StyleGroup)] {
    &table().known_hosts
}

fn classify_in(t: &Table, bundle_id: Option<&str>, app_name: Option<&str>) -> AppClass {
    if let Some(rule) = bundle_id.and_then(|b| t.apps.get(&b.to_lowercase())) {
        return AppClass {
            style_group: rule.style_group,
            usage_category: rule.usage_category,
            is_ai_chat: rule.is_ai_chat,
            is_browser: false,
            matched_by: MatchedBy::BundleId,
        };
    }
    let signature = format!(
        "{} {}",
        app_name.unwrap_or_default(),
        bundle_id.unwrap_or_default()
    )
    .to_lowercase();
    if signature.trim().is_empty() {
        return AppClass::OTHER;
    }
    for (group, entries) in &t.signatures {
        if entries.iter().any(|e| has_token(&signature, e)) {
            return AppClass {
                style_group: *group,
                usage_category: UsageCategory::for_group(*group),
                is_ai_chat: false,
                is_browser: false,
                matched_by: MatchedBy::Signature,
            };
        }
    }
    if t.browsers.iter().any(|e| has_token(&signature, e)) {
        return AppClass {
            is_browser: true,
            matched_by: MatchedBy::Browser,
            ..AppClass::OTHER
        };
    }
    AppClass::OTHER
}

#[cfg(test)]
mod tests {
    use super::*;

    fn c(bundle: &str, name: &str) -> AppClass {
        classify(Some(bundle), Some(name))
    }

    #[test]
    fn shipped_table_parses_and_every_value_is_valid() {
        let t = parse(DATA_JSON).unwrap();
        assert!(t.apps.len() >= 40, "{} apps", t.apps.len());
        assert_eq!(t.signatures.len(), 3);
        // Every signature is lower case and non-empty (match is on lower case).
        for (_, list) in &t.signatures {
            for s in list {
                assert!(!s.trim().is_empty());
            }
        }
        // AI chat apps are always AI prompts.
        for (id, rule) in &t.apps {
            if rule.is_ai_chat {
                assert_eq!(rule.usage_category, UsageCategory::AiPrompts, "{id}");
            }
        }
    }

    #[test]
    fn serialized_names_match_the_db_checks() {
        let sql = include_str!("../db/migrations/0001_init.sql");
        for g in [
            StyleGroup::Personal,
            StyleGroup::Work,
            StyleGroup::Email,
            StyleGroup::Other,
        ] {
            assert!(sql.contains(&format!("'{}'", g.as_str())));
            assert_eq!(
                serde_json::to_value(g).unwrap(),
                serde_json::json!(g.as_str())
            );
        }
        for u in [
            UsageCategory::Documents,
            UsageCategory::AiPrompts,
            UsageCategory::Email,
            UsageCategory::Work,
            UsageCategory::Personal,
            UsageCategory::Other,
        ] {
            assert!(sql.contains(&format!("'{}'", u.as_str())));
            assert_eq!(
                serde_json::to_value(u).unwrap(),
                serde_json::json!(u.as_str())
            );
        }
    }

    #[test]
    fn known_apps_by_bundle_id() {
        let mail = c("com.apple.mail", "Mail");
        assert_eq!(mail.style_group, StyleGroup::Email);
        assert_eq!(mail.usage_category, UsageCategory::Email);
        assert_eq!(mail.matched_by, MatchedBy::BundleId);

        assert_eq!(
            c("com.tinyspeck.slackmacgap", "Slack").style_group,
            StyleGroup::Work
        );
        assert_eq!(
            c("com.apple.MobileSMS", "Messages").style_group,
            StyleGroup::Personal
        );
        let notes = c("com.apple.Notes", "Notes");
        assert_eq!(notes.style_group, StyleGroup::Other);
        assert_eq!(notes.usage_category, UsageCategory::Documents);

        // ChatGPT.app's bundle id is `com.openai.codex` in current builds
        // (seen on macOS 26.6, 2026-10-30); older builds use `com.openai.chat`.
        assert!(c("com.openai.codex", "ChatGPT").is_ai_chat);
        let chatgpt = c("com.openai.chat", "ChatGPT");
        assert!(chatgpt.is_ai_chat);
        assert_eq!(chatgpt.usage_category, UsageCategory::AiPrompts);
        let cursor = c("com.todesktop.230313mzl4w4u92", "Cursor");
        assert!(!cursor.is_ai_chat);
        assert_eq!(cursor.usage_category, UsageCategory::AiPrompts);
        // Bundle ids are case-insensitive.
        assert_eq!(
            classify(Some("COM.APPLE.MAIL"), None).style_group,
            StyleGroup::Email
        );
    }

    #[test]
    fn unknown_apps_by_whole_token_signature() {
        // Not in `apps`: matched by name or bundle id token.
        let a = c("com.example.whatsapp-beta", "WhatsApp Beta");
        assert_eq!(a.style_group, StyleGroup::Personal);
        assert_eq!(a.matched_by, MatchedBy::Signature);
        assert_eq!(
            c("com.example.foo", "Mattermost").usage_category,
            UsageCategory::Work
        );
        // A Chrome web app named after its site.
        assert_eq!(
            c(
                "com.google.Chrome.app.fmgjjmmmlfnkbppncabfkddbjimcfncm",
                "Gmail"
            )
            .style_group,
            StyleGroup::Email
        );
        // Whole tokens only.
        assert_eq!(
            c("com.example.mailplane", "Mailplane").style_group,
            StyleGroup::Other
        );
        assert_eq!(
            c("com.example.slacker", "Slacker").style_group,
            StyleGroup::Other
        );
        // Multi-word signature.
        assert_eq!(
            classify(None, Some("Google Chat")).style_group,
            StyleGroup::Work
        );
    }

    #[test]
    fn email_wins_over_work_and_personal() {
        // "Outlook" (email) and "Teams" (work) in one signature.
        assert_eq!(
            classify(None, Some("Outlook Teams helper")).style_group,
            StyleGroup::Email
        );
        assert_eq!(
            classify(None, Some("Slack Messages bridge")).style_group,
            StyleGroup::Work
        );
    }

    #[test]
    fn browsers_and_everything_else_are_other() {
        let chrome = c("com.google.Chrome", "Google Chrome");
        assert_eq!(chrome.style_group, StyleGroup::Other);
        assert!(chrome.is_browser);
        assert!(c("company.thebrowser.Browser", "Arc").is_browser);
        assert!(c("com.apple.Safari", "Safari").is_browser);
        let term = c("com.apple.Terminal", "Terminal");
        assert_eq!(term, AppClass::OTHER);
        assert_eq!(classify(None, None), AppClass::OTHER);
        assert_eq!(classify(Some(""), Some("  ")), AppClass::OTHER);
    }

    #[test]
    fn builtin_hosts_and_title_aliases_are_host_only() {
        let gmail = classify_host("mail.google.com/mail/u/0").unwrap();
        assert_eq!(gmail.style_group, StyleGroup::Email);
        assert_eq!(gmail.usage_category, UsageCategory::Email);
        let slack = classify_host("app.slack.com/client/team").unwrap();
        assert_eq!(slack.style_group, StyleGroup::Work);
        assert_eq!(classify_host("example.com"), None);
        assert_eq!(
            host_from_title("Inbox - Gmail").as_deref(),
            Some("mail.google.com")
        );
        assert_eq!(
            host_from_title("Team | Slack").as_deref(),
            Some("app.slack.com")
        );
        assert_eq!(host_from_title("A private title"), None);
        assert_eq!(known_hosts().len(), 2);
    }

    #[test]
    fn token_boundaries() {
        assert!(has_token("mail com.apple.mail", "mail"));
        assert!(has_token("zoom.us us.zoom.xos", "zoom.us"));
        assert!(!has_token("gmailer", "mail"));
        assert!(!has_token("hotmail", "mail"));
        assert!(has_token("hey", "hey"));
        assert!(!has_token("heyday", "hey"));
    }
}
