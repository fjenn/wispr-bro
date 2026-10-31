// SPDX-License-Identifier: GPL-3.0-or-later
//
// The precedence (website before app) and the website pattern match are a
// port of BetterWispr's `WritingMode`:
//   Source: BetterWispr, https://github.com/opennookorg/betterwispr
//   Commit: 304335a073a98f7439e5f7b4fbc2c31b810b807e
//   File:   Sources/BetterWisprCore/Domain/WritingMode.swift
//           (`website(_:matches:)`, `address`, `[WritingMode].match`;
//           tests: Tests/BetterWisprCoreTests/WritingModeTests.swift)
//   That file adapts VoiceInk (https://github.com/Beingpax/VoiceInk,
//   c09cc1f677f40f2ee665843a61f07670210f012e) and is GPL-3.0 (BetterWispr docs/oss-reuse.md).
//   Modified by Wispr Bro contributors, 2026-10-31: ported to Rust; a rule
//   names one app or one site and the style group it belongs to (no modes,
//   no trigger words); the built-in app table (`categories.json`) is the
//   last tier.
//
//! Which style group an app belongs to, as the user arranged it on the
//! Style page (S-19a, R5 §4.3 `app_rules`), and whether it gets "Press
//! Enter after dictating".
//!
//! [`resolve`] decides, most specific first (BetterWispr's tiers):
//!
//! 1. a site rule whose pattern matches the page (S-19b supplies the page;
//!    host, subdomain and path-prefix match);
//! 2. the user's rule for the app (bundle id, case-insensitive);
//! 3. the built-in table ([`super::categories::classify`]).
//!
//! A rule with no group only carries "Press Enter" and keeps the built-in
//! group. Rules live in `fork.db` (`app_rules`, migration 0007); the take
//! path reads a cached copy ([`current`]), refreshed after every change.
//!
//! "Press Enter after dictating" (R2 D-61, RV-48) is armed per take by the
//! cleanup step ([`arm_auto_send`]) only when the cleanup did not fall back,
//! and the paste sends Return only after a confirmed paste (S-04's
//! `deliver::paste` with `submit_strict`): never after a guard fallback, an
//! unverified paste or a card.

use std::collections::VecDeque;
use std::sync::{Arc, Mutex, RwLock};

use rusqlite::{params, Connection, OptionalExtension};
use serde::{Deserialize, Serialize};
use specta::Type;

use super::categories::{self, StyleGroup};
use crate::fork::db::DbError;

/// What a rule names.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "snake_case")]
pub enum RuleKind {
    /// A bundle id.
    App,
    /// A site pattern ("mail.google.com", "github.com/org"), S-19b.
    Host,
}

impl RuleKind {
    fn as_sql(self) -> &'static str {
        match self {
            RuleKind::App => "app",
            RuleKind::Host => "host",
        }
    }

    fn from_sql(s: &str) -> RuleKind {
        if s == "host" {
            RuleKind::Host
        } else {
            RuleKind::App
        }
    }
}

/// One row of `app_rules`.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, Type)]
pub struct AppRule {
    /// The bundle id (as macOS reports it) or the site pattern.
    pub key: String,
    pub kind: RuleKind,
    /// `None`: the built-in group stays (the rule only sets auto-send).
    pub style_group: Option<StyleGroup>,
    pub auto_send: bool,
    /// The app's name when the rule was made (shown if it is uninstalled).
    pub app_name: Option<String>,
    /// Unix ms.
    pub updated_at: i64,
}

/// Which tier decided.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "snake_case")]
pub enum RuleTier {
    Site,
    App,
    Builtin,
}

/// The answer for one app (and page).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Resolved {
    pub group: StyleGroup,
    /// The built-in table's group, whatever the user chose.
    pub builtin_group: StyleGroup,
    pub auto_send: bool,
    pub tier: RuleTier,
}

/// BetterWispr `address`: host and path of a pattern or URL, lower case,
/// without scheme, `www.`/`*.`, port, query or fragment.
fn address(text: &str) -> (String, String) {
    let mut t = text.trim().to_lowercase();
    if let Some(i) = t.find("://") {
        let scheme = &t[..i];
        let valid = scheme
            .chars()
            .next()
            .is_some_and(|c| c.is_ascii_lowercase())
            && scheme
                .chars()
                .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || "+.-".contains(c));
        if valid {
            t = t[i + 3..].to_string();
        }
    }
    for prefix in ["www.", "*."] {
        if let Some(rest) = t.strip_prefix(prefix) {
            t = rest.to_string();
            break;
        }
    }
    let location: &str = t.split(['?', '#']).next().unwrap_or_default();
    let host_end = location.find('/').unwrap_or(location.len());
    let host = location[..host_end].split(':').next().unwrap_or_default();
    let path = location[host_end..].trim_matches(|c| c == '/' || c == '*');
    (host.to_string(), path.to_string())
}

/// BetterWispr `website(_:matches:)`: the URL's host is the pattern's host
/// or a subdomain of it, and its path starts with the pattern's path
/// segments.
pub fn site_matches(pattern: &str, url: &str) -> bool {
    let (ph, pp) = address(pattern);
    let (uh, up) = address(url);
    if ph.is_empty() || !(uh == ph || uh.ends_with(&format!(".{ph}"))) {
        return false;
    }
    pp.is_empty() || up == pp || up.starts_with(&format!("{pp}/"))
}

/// The group and auto-send for an app (and its page, when known), most
/// specific rule first. Within a tier, the first rule in `rules` wins.
pub fn resolve(
    rules: &[AppRule],
    bundle_id: Option<&str>,
    app_name: Option<&str>,
    url: Option<&str>,
) -> Resolved {
    let builtin = categories::classify(bundle_id, app_name).style_group;
    let app_rule = bundle_id.and_then(|b| {
        rules
            .iter()
            .find(|r| r.kind == RuleKind::App && r.key.eq_ignore_ascii_case(b))
    });
    let auto_send = app_rule.is_some_and(|r| r.auto_send);
    if let Some(url) = url.filter(|u| !u.trim().is_empty()) {
        if let Some(site) = rules.iter().find(|r| {
            r.kind == RuleKind::Host && r.style_group.is_some() && site_matches(&r.key, url)
        }) {
            return Resolved {
                group: site.style_group.unwrap_or(builtin),
                builtin_group: builtin,
                auto_send: site.auto_send || auto_send,
                tier: RuleTier::Site,
            };
        }
    }
    match app_rule.and_then(|r| r.style_group) {
        Some(group) => Resolved {
            group,
            builtin_group: builtin,
            auto_send,
            tier: RuleTier::App,
        },
        None => Resolved {
            group: builtin,
            builtin_group: builtin,
            auto_send,
            tier: RuleTier::Builtin,
        },
    }
}

// ---------------------------------------------------------------------------
// Store (`fork.db` `app_rules`)

fn group_sql(g: StyleGroup) -> &'static str {
    g.as_str()
}

fn group_from_sql(s: &str) -> Option<StyleGroup> {
    Some(match s {
        "personal" => StyleGroup::Personal,
        "work" => StyleGroup::Work,
        "email" => StyleGroup::Email,
        "other" => StyleGroup::Other,
        _ => return None,
    })
}

/// Every rule, newest change first.
pub fn list(conn: &Connection) -> Result<Vec<AppRule>, DbError> {
    let mut stmt = conn.prepare(
        "SELECT \"match\", kind, style_group, auto_send, app_name, updated_at
         FROM app_rules ORDER BY updated_at DESC, \"match\"",
    )?;
    let rows = stmt.query_map([], |r| {
        Ok(AppRule {
            key: r.get(0)?,
            kind: RuleKind::from_sql(&r.get::<_, String>(1)?),
            style_group: r
                .get::<_, Option<String>>(2)?
                .as_deref()
                .and_then(group_from_sql),
            auto_send: r.get::<_, i64>(3)? != 0,
            app_name: r.get(4)?,
            updated_at: r.get(5)?,
        })
    })?;
    Ok(rows.collect::<Result<Vec<_>, _>>()?)
}

fn get(conn: &Connection, key: &str, kind: RuleKind) -> Result<Option<AppRule>, DbError> {
    Ok(conn
        .query_row(
            "SELECT \"match\", style_group, auto_send, app_name, updated_at
         FROM app_rules WHERE \"match\" = ?1 AND kind = ?2",
            params![key, kind.as_sql()],
            |r| {
                Ok(AppRule {
                    key: r.get(0)?,
                    kind,
                    style_group: r
                        .get::<_, Option<String>>(1)?
                        .as_deref()
                        .and_then(group_from_sql),
                    auto_send: r.get::<_, i64>(2)? != 0,
                    app_name: r.get(3)?,
                    updated_at: r.get(4)?,
                })
            },
        )
        .optional()?)
}

/// Writes `rule`, or deletes it when it says nothing (no group, no
/// auto-send): the app is then back to its built-in group.
fn put(conn: &Connection, rule: &AppRule) -> Result<(), DbError> {
    if rule.style_group.is_none() && !rule.auto_send {
        conn.execute(
            "DELETE FROM app_rules WHERE \"match\" = ?1 AND kind = ?2",
            params![rule.key, rule.kind.as_sql()],
        )?;
        return Ok(());
    }
    conn.execute(
        "INSERT INTO app_rules (\"match\", kind, style_group, auto_send, source, app_name, updated_at)
         VALUES (?1, ?2, ?3, ?4, 'user', ?5, ?6)
         ON CONFLICT(\"match\") DO UPDATE SET
           kind = excluded.kind,
           style_group = excluded.style_group,
           auto_send = excluded.auto_send,
           app_name = COALESCE(excluded.app_name, app_rules.app_name),
           updated_at = excluded.updated_at",
        params![
            rule.key,
            rule.kind.as_sql(),
            rule.style_group.map(group_sql),
            i64::from(rule.auto_send),
            rule.app_name,
            rule.updated_at
        ],
    )?;
    Ok(())
}

fn existing_or_new(
    conn: &Connection,
    bundle_id: &str,
    app_name: Option<&str>,
    now: i64,
) -> Result<AppRule, DbError> {
    let mut rule = get(conn, bundle_id, RuleKind::App)?.unwrap_or(AppRule {
        key: bundle_id.to_string(),
        kind: RuleKind::App,
        style_group: None,
        auto_send: false,
        app_name: None,
        updated_at: now,
    });
    if let Some(n) = app_name.map(str::trim).filter(|n| !n.is_empty()) {
        rule.app_name = Some(n.to_string());
    }
    rule.updated_at = now;
    Ok(rule)
}

/// Puts an app in `group`; `None` returns it to its built-in group.
pub fn set_group(
    conn: &Connection,
    bundle_id: &str,
    app_name: Option<&str>,
    group: Option<StyleGroup>,
    now: i64,
) -> Result<(), DbError> {
    let mut rule = existing_or_new(conn, bundle_id, app_name, now)?;
    rule.style_group = group;
    put(conn, &rule)
}

/// Turns "Press Enter after dictating" on or off for an app.
pub fn set_auto_send(
    conn: &Connection,
    bundle_id: &str,
    app_name: Option<&str>,
    on: bool,
    now: i64,
) -> Result<(), DbError> {
    let mut rule = existing_or_new(conn, bundle_id, app_name, now)?;
    rule.auto_send = on;
    put(conn, &rule)
}

/// Takes an app out of `from`: it goes to Other ("all other apps"). An app
/// whose built-in group is Other then needs no group of its own.
pub fn remove_from_group(
    conn: &Connection,
    bundle_id: &str,
    app_name: Option<&str>,
    now: i64,
) -> Result<(), DbError> {
    let builtin = categories::classify(Some(bundle_id), app_name).style_group;
    let group = (builtin != StyleGroup::Other).then_some(StyleGroup::Other);
    set_group(conn, bundle_id, app_name, group, now)
}

// ---------------------------------------------------------------------------
// The cached copy for the take path

static RULES: RwLock<Option<Arc<Vec<AppRule>>>> = RwLock::new(None);

/// The rules as last loaded (empty before [`reload`] ran: built-in groups).
pub fn current() -> Arc<Vec<AppRule>> {
    RULES
        .read()
        .unwrap_or_else(|e| e.into_inner())
        .clone()
        .unwrap_or_default()
}

/// Replaces the cached rules (tests, and after a change).
pub fn set_current(rules: Vec<AppRule>) {
    *RULES.write().unwrap_or_else(|e| e.into_inner()) = Some(Arc::new(rules));
}

/// Reloads the rules from `fork.db` (at start and after every change).
pub fn reload(conn: &Connection) -> Result<Arc<Vec<AppRule>>, DbError> {
    let rules = list(conn)?;
    set_current(rules);
    Ok(current())
}

/// The group and auto-send of an app with the cached rules.
pub fn for_app(bundle_id: Option<&str>, app_name: Option<&str>) -> Resolved {
    resolve(&current(), bundle_id, app_name, None)
}

/// Whether the app in front now has "Press Enter after dictating" on (the
/// bar's "Sends automatically" hint).
pub fn frontmost_auto_send() -> bool {
    super::frontmost::frontmost()
        .is_some_and(|a| for_app(a.bundle_id.as_deref(), a.name.as_deref()).auto_send)
}

// ---------------------------------------------------------------------------
// Auto-send armed for one take

/// A take's text that may be followed by Return, and the app it was meant
/// for.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Armed {
    pub text: String,
    pub bundle_id: String,
}

const ARMED_KEEP: usize = 4;
static ARMED: Mutex<VecDeque<Armed>> = Mutex::new(VecDeque::new());

/// The cleanup step allows Return after `text` in `bundle_id` (called only
/// when the cleanup did not fall back).
pub fn arm_auto_send(text: &str, bundle_id: &str) {
    let mut q = ARMED.lock().unwrap_or_else(|e| e.into_inner());
    q.retain(|a| a.text != text);
    if q.len() == ARMED_KEEP {
        q.pop_front();
    }
    q.push_back(Armed {
        text: text.to_string(),
        bundle_id: bundle_id.to_string(),
    });
}

/// The delivery of `text` takes its arming, if any (consumed).
pub fn take_armed(text: &str) -> Option<Armed> {
    let mut q = ARMED.lock().unwrap_or_else(|e| e.into_inner());
    let at = q.iter().rposition(|a| a.text == text)?;
    q.remove(at)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rule(key: &str, kind: RuleKind, group: Option<StyleGroup>, auto_send: bool) -> AppRule {
        AppRule {
            key: key.into(),
            kind,
            style_group: group,
            auto_send,
            app_name: None,
            updated_at: 0,
        }
    }

    const SLACK: &str = "com.tinyspeck.slackmacgap";

    // BetterWispr `modeResolutionPrefersTriggerWordThenWebsiteThenApp`
    // (without trigger words) and our built-in tier.
    #[test]
    fn precedence_site_then_app_then_builtin() {
        let rules = vec![
            rule(SLACK, RuleKind::App, Some(StyleGroup::Personal), false),
            rule("github.com", RuleKind::Host, Some(StyleGroup::Work), false),
        ];
        let r = resolve(
            &rules,
            Some(SLACK),
            Some("Slack"),
            Some("https://github.com/pulls"),
        );
        assert_eq!((r.group, r.tier), (StyleGroup::Work, RuleTier::Site));
        let r = resolve(
            &rules,
            Some(SLACK),
            Some("Slack"),
            Some("https://example.com"),
        );
        assert_eq!((r.group, r.tier), (StyleGroup::Personal, RuleTier::App));
        assert_eq!(r.builtin_group, StyleGroup::Work);
        let r = resolve(&rules, Some(SLACK), None, None);
        assert_eq!(r.group, StyleGroup::Personal);
        let r = resolve(
            &rules,
            Some("com.apple.mail"),
            Some("Mail"),
            Some("https://example.com"),
        );
        assert_eq!((r.group, r.tier), (StyleGroup::Email, RuleTier::Builtin));
        let r = resolve(&rules, None, None, None);
        assert_eq!((r.group, r.tier), (StyleGroup::Other, RuleTier::Builtin));
        // Bundle ids are case-insensitive.
        let r = resolve(&rules, Some("COM.TINYSPECK.SLACKMACGAP"), None, None);
        assert_eq!(r.group, StyleGroup::Personal);
    }

    // `modeResolutionUsesListOrderWithinATier`.
    #[test]
    fn first_rule_wins_within_a_tier() {
        let a = rule("github.com", RuleKind::Host, Some(StyleGroup::Work), false);
        let b = rule(
            "github.com/org",
            RuleKind::Host,
            Some(StyleGroup::Email),
            false,
        );
        let url = Some("https://github.com/org/repo");
        assert_eq!(
            resolve(&[a.clone(), b.clone()], None, None, url).group,
            StyleGroup::Work
        );
        assert_eq!(resolve(&[b, a], None, None, url).group, StyleGroup::Email);
    }

    #[test]
    fn a_rule_without_a_group_keeps_the_builtin_group() {
        let rules = vec![rule(SLACK, RuleKind::App, None, true)];
        let r = resolve(&rules, Some(SLACK), Some("Slack"), None);
        assert_eq!(r.group, StyleGroup::Work);
        assert_eq!(r.tier, RuleTier::Builtin);
        assert!(r.auto_send);
        assert!(!resolve(&rules, Some("com.apple.mail"), None, None).auto_send);
    }

    // `websitePatternsMatchHostsSubdomainsAndPathPrefixes`.
    #[test]
    // BetterWispr's cases with its real sites replaced by RFC 2606/6761
    // names (the hosts guard allows only those in test code).
    fn site_patterns() {
        assert!(site_matches("example.com", "https://example.com/team"));
        assert!(site_matches(
            "example.com",
            "https://gist.example.com/someone"
        ));
        assert!(site_matches(
            "https://www.Example.com/",
            "http://example.com"
        ));
        assert!(site_matches("www.example.com", "example.com:443/x"));
        assert!(site_matches(
            "mail.example.com/mail",
            "https://mail.example.com/mail/u/0/#inbox"
        ));
        assert!(site_matches(
            "video.example/watch",
            "https://www.video.example/watch?v=abc"
        ));
        assert!(site_matches("x.example", "https://x.example/home"));
        assert!(site_matches("mail.example.com", "mail.example.com"));

        assert!(!site_matches("x.example", "https://www.files.example/home"));
        assert!(!site_matches("hub.example", "https://nothub.example"));
        assert!(!site_matches(
            "example.com",
            "https://example.com.evil.example/"
        ));
        assert!(!site_matches(
            "mail.example.com/mail",
            "https://mail.example.com/chat"
        ));
        assert!(!site_matches(
            "example.com/team",
            "https://example.com/teams/claude"
        ));
        assert!(!site_matches(
            "search.example/mail",
            "https://search.example/?q=/mail"
        ));
        assert!(site_matches(
            "*.example.com",
            "https://gist.example.com/someone"
        ));
        assert!(site_matches(
            "https://example.com/team/*",
            "https://example.com/team/claude"
        ));
        assert!(!site_matches(
            "https://example.com/team/*",
            "https://example.com/apple"
        ));
        assert!(!site_matches(" ", "https://example.com"));
        assert!(!site_matches(" ", "file:///Users/me/page.html"));
    }

    fn db() -> (tempfile::TempDir, crate::fork::db::ForkDb) {
        let dir = tempfile::tempdir().unwrap();
        let db = crate::fork::db::ForkDb::in_dir(dir.path());
        (dir, db)
    }

    #[test]
    fn store_moves_removes_and_resets() {
        let (_d, db) = db();
        db.with_conn(|c| {
            // Move Slack (built-in Work) to Personal.
            set_group(
                c,
                "com.tinyspeck.slackmacgap",
                Some("Slack"),
                Some(StyleGroup::Personal),
                1,
            )?;
            let r = list(c)?;
            assert_eq!(r.len(), 1);
            assert_eq!(r[0].style_group, Some(StyleGroup::Personal));
            assert_eq!(r[0].app_name.as_deref(), Some("Slack"));
            assert_eq!(
                resolve(&r, Some(SLACK), None, None).group,
                StyleGroup::Personal
            );
            // Keys are case-insensitive: the same row.
            set_auto_send(c, "COM.TINYSPECK.SLACKMACGAP", None, true, 2)?;
            let r = list(c)?;
            assert_eq!(r.len(), 1);
            assert!(r[0].auto_send);
            assert_eq!(r[0].app_name.as_deref(), Some("Slack"));
            // Removed from Personal: Slack goes to Other (not back to Work).
            remove_from_group(c, SLACK, Some("Slack"), 3)?;
            assert_eq!(list(c)?[0].style_group, Some(StyleGroup::Other));
            // Default group again: the row keeps only auto-send.
            set_group(c, SLACK, None, None, 4)?;
            let r = list(c)?;
            assert_eq!((r[0].style_group, r[0].auto_send), (None, true));
            assert_eq!(resolve(&r, Some(SLACK), None, None).group, StyleGroup::Work);
            // Nothing left to say: the row goes.
            set_auto_send(c, SLACK, None, false, 5)?;
            assert!(list(c)?.is_empty());
            // An Other app removed from a group it was added to: no row.
            set_group(
                c,
                "com.example.tool",
                Some("Tool"),
                Some(StyleGroup::Email),
                6,
            )?;
            remove_from_group(c, "com.example.tool", Some("Tool"), 7)?;
            assert!(list(c)?.is_empty());
            // The table refuses a row that says nothing.
            let bad = c.execute(
                "INSERT INTO app_rules (\"match\", style_group, auto_send) VALUES ('x', NULL, 0)",
                [],
            );
            assert!(bad.is_err());
            // The cache follows a reload.
            // (A made-up app: other tests read the cache in parallel.)
            let id = "com.example.wb-cache-test";
            set_group(c, id, Some("Cache Test"), Some(StyleGroup::Work), 8)?;
            reload(c)?;
            assert_eq!(for_app(Some(id), None).group, StyleGroup::Work);
            set_group(c, id, None, None, 9)?;
            reload(c)?;
            assert_eq!(for_app(Some(id), None).group, StyleGroup::Other);
            Ok(())
        })
        .unwrap();
    }

    #[test]
    fn migration_0007_rebuilds_the_empty_v1_table() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("fork.db");
        let old =
            crate::fork::db::ForkDb::with_migrations(&path, crate::fork::db::test_migrations(6));
        old.with_conn(|c| {
            // v1's table refused a NULL group.
            let r = c.execute(
                "INSERT INTO app_rules (\"match\", style_group, auto_send) VALUES ('a', NULL, 1)",
                [],
            );
            assert!(r.is_err());
            Ok(())
        })
        .unwrap();
        old.close();
        let db = crate::fork::db::ForkDb::in_dir(dir.path());
        db.with_conn(|c| {
            set_auto_send(c, "com.example.a", None, true, 1)?;
            let cols: Vec<String> = c
                .prepare("SELECT name FROM pragma_table_info('app_rules')")?
                .query_map([], |r| r.get(0))?
                .collect::<Result<_, _>>()?;
            assert_eq!(
                cols,
                [
                    "match",
                    "kind",
                    "style_group",
                    "auto_send",
                    "source",
                    "app_name",
                    "updated_at"
                ]
            );
            Ok(())
        })
        .unwrap();
    }

    #[test]
    fn arming_is_per_text_and_consumed() {
        arm_auto_send("hello there", SLACK);
        arm_auto_send("other text", "com.apple.MobileSMS");
        assert_eq!(take_armed("nope"), None);
        assert_eq!(take_armed("hello there").unwrap().bundle_id, SLACK);
        assert_eq!(take_armed("hello there"), None);
        assert!(take_armed("other text").is_some());
        for i in 0..10 {
            arm_auto_send(&format!("t{i}"), SLACK);
        }
        assert_eq!(take_armed("t0"), None);
        assert!(take_armed("t9").is_some());
    }
}
