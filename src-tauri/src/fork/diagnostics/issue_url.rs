// SPDX-License-Identifier: GPL-3.0-or-later
//! Prefilled GitHub issues for Help › Report a problem and Send feedback
//! (S-26b, R2 C-103).
//!
//! The browser opens `github.com/fjenn/wispr-bro/issues/new` with a title,
//! a body template and a label; the user reads and edits everything before
//! anything is sent, and GitHub is the only host involved (R5 §1).
//!
//! **What goes in:** only the fields of [`IssueContext`]: the app version,
//! the macOS version, the chip and RAM class, the app language and the
//! speech model id the app shipped. Nothing the user dictated, typed or
//! named: no transcripts, History, dictionary words, snippets, prompts,
//! file paths, device names or logs. Diagnostics (sanitized logs) are never
//! put into the URL; the body tells the user how to copy and paste them.
//! [`gather`] reads no database and no History, by construction.

use reqwest::Url;
use serde::{Deserialize, Serialize};
use specta::Type;

/// The public repository (the only host these links open).
pub const REPO_URL: &str = "https://github.com/fjenn/wispr-bro";
/// `issues/new` on [`REPO_URL`].
pub const NEW_ISSUE_URL: &str = "https://github.com/fjenn/wispr-bro/issues/new";

/// GitHub serves `issues/new` URLs up to about 8 KB; the template stays far
/// below, whatever the context strings are.
pub const MAX_URL_LEN: usize = 6000;
/// Longest context value kept (a model id, a version string).
const MAX_FIELD_CHARS: usize = 64;

/// Which form to open.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "snake_case")]
pub enum IssueKind {
    /// Help › Report a problem… (⌘/).
    Problem,
    /// Help › Send feedback….
    Feedback,
}

/// Everything an issue may say about this Mac and app. Every field is a
/// version, an id the app ships or a hardware class; adding a field here
/// is a privacy decision (see the module docs).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct IssueContext {
    pub app_version: String,
    /// `26.6.2`, when known.
    pub macos_version: Option<String>,
    /// `Apple M5 Max`, when known.
    pub chip: Option<String>,
    /// Installed RAM in whole GB, when known.
    pub ram_gb: Option<u64>,
    /// Handy's app-language setting (`en`, `de`, …).
    pub app_language: Option<String>,
    /// The speech model id (a catalog id, never a path).
    pub speech_model: Option<String>,
}

/// One line of the "About this Mac" block, trimmed and bounded; `None` when
/// empty or not a plain one-line value.
fn field(value: Option<&str>) -> Option<String> {
    let value = value?.trim();
    if value.is_empty() || value.chars().any(char::is_control) {
        return None;
    }
    Some(value.chars().take(MAX_FIELD_CHARS).collect())
}

fn context_block(ctx: &IssueContext) -> String {
    let unknown = || "unknown".to_string();
    let mac = match (field(ctx.chip.as_deref()), ctx.ram_gb) {
        (Some(chip), Some(gb)) => format!("{chip}, {gb} GB"),
        (Some(chip), None) => chip,
        (None, Some(gb)) => format!("{gb} GB"),
        (None, None) => unknown(),
    };
    format!(
        "- Wispr Bro: {}\n- macOS: {}\n- Mac: {}\n- App language: {}\n- Speech model: {}\n",
        field(Some(&ctx.app_version)).unwrap_or_else(unknown),
        field(ctx.macos_version.as_deref()).unwrap_or_else(unknown),
        mac,
        field(ctx.app_language.as_deref()).unwrap_or_else(unknown),
        field(ctx.speech_model.as_deref()).unwrap_or_else(unknown),
    )
}

/// Issue text is English whatever the app language: the maintainers triage
/// in English, and GitHub shows the form in the browser's own language.
fn body(kind: IssueKind, ctx: &IssueContext) -> String {
    let privacy =
        "<!-- Please don't paste anything you dictated that you want to keep private. -->";
    let about = context_block(ctx);
    match kind {
        IssueKind::Problem => format!(
            "{privacy}\n\n\
             ### What happened?\n\n\n\
             ### What did you expect?\n\n\n\
             ### Steps to reproduce\n\n1. \n\n\
             ### Diagnostics (optional)\n\n\
             In Wispr Bro, open Help › Report a problem and click Copy diagnostics, then paste here. \
             Diagnostics are sanitized logs; they never contain what you dictated.\n\n\
             ### About this Mac\n\n{about}"
        ),
        IssueKind::Feedback => format!(
            "{privacy}\n\n\
             ### Your idea or feedback\n\n\n\
             ### What would it help you do?\n\n\n\
             ### About this Mac\n\n{about}"
        ),
    }
}

fn title(kind: IssueKind) -> &'static str {
    match kind {
        IssueKind::Problem => "Problem: ",
        IssueKind::Feedback => "Feedback: ",
    }
}

fn label(kind: IssueKind) -> &'static str {
    match kind {
        IssueKind::Problem => "bug",
        IssueKind::Feedback => "feedback",
    }
}

/// The `issues/new` URL for `kind`. GitHub applies `labels` only for users
/// who may label; for everyone else it is ignored, which is harmless.
pub fn issue_url(kind: IssueKind, ctx: &IssueContext) -> String {
    let url = Url::parse_with_params(
        NEW_ISSUE_URL,
        &[
            ("title", title(kind)),
            ("labels", label(kind)),
            ("body", body(kind, ctx).as_str()),
        ],
    )
    .map(String::from)
    // NEW_ISSUE_URL is a constant that parses (tested).
    .unwrap_or_else(|_| NEW_ISSUE_URL.to_string());
    if url.len() > MAX_URL_LEN {
        return NEW_ISSUE_URL.to_string();
    }
    url
}

/// Reads this Mac's versions and hardware class (sysinfo; no database).
pub fn probe_machine() -> (Option<String>, Option<String>, Option<u64>) {
    use sysinfo::{CpuRefreshKind, MemoryRefreshKind, RefreshKind, System};
    let sys = System::new_with_specifics(
        RefreshKind::nothing()
            .with_memory(MemoryRefreshKind::nothing().with_ram())
            .with_cpu(CpuRefreshKind::nothing()),
    );
    let chip = sys
        .cpus()
        .first()
        .map(|cpu| cpu.brand().trim().to_string())
        .filter(|brand| !brand.is_empty());
    let ram = sys.total_memory();
    let ram_gb = (ram > 0).then(|| (ram + (1 << 29)) >> 30);
    (System::os_version(), chip, ram_gb)
}

/// The context for this app: versions and ids only (see [`IssueContext`]).
pub fn gather<R: super::super::ForkRuntime>(app: &tauri::AppHandle<R>) -> IssueContext {
    let (macos_version, chip, ram_gb) = probe_machine();
    let handy = R::handy_settings(app).map(|settings| settings.0);
    IssueContext {
        app_version: app.package_info().version.to_string(),
        macos_version,
        chip,
        ram_gb,
        app_language: handy.as_ref().map(|s| s.app_language.clone()),
        speech_model: handy
            .as_ref()
            .map(|s| s.selected_model.clone())
            .filter(|id| !id.is_empty()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn query(url: &str) -> Vec<(String, String)> {
        Url::parse(url)
            .unwrap()
            .query_pairs()
            .map(|(k, v)| (k.into_owned(), v.into_owned()))
            .collect()
    }

    fn sample() -> IssueContext {
        IssueContext {
            app_version: "1.0.0-alpha.1".into(),
            macos_version: Some("26.6.2".into()),
            chip: Some("Apple M5 Max".into()),
            ram_gb: Some(64),
            app_language: Some("de".into()),
            speech_model: Some("parakeet-tdt-0.6b-v3".into()),
        }
    }

    #[test]
    fn problem_url_opens_the_repo_issue_form_with_the_context() {
        let url = issue_url(IssueKind::Problem, &sample());
        assert!(url.starts_with("https://github.com/fjenn/wispr-bro/issues/new?"));
        let q = query(&url);
        let keys: Vec<&str> = q.iter().map(|(k, _)| k.as_str()).collect();
        assert_eq!(keys, ["title", "labels", "body"]);
        assert_eq!(q[0].1, "Problem: ");
        assert_eq!(q[1].1, "bug");
        let body = &q[2].1;
        for line in [
            "- Wispr Bro: 1.0.0-alpha.1",
            "- macOS: 26.6.2",
            "- Mac: Apple M5 Max, 64 GB",
            "- App language: de",
            "- Speech model: parakeet-tdt-0.6b-v3",
            "Copy diagnostics",
        ] {
            assert!(body.contains(line), "{line} missing: {body}");
        }
    }

    #[test]
    fn feedback_url_has_its_own_title_label_and_template() {
        let q = query(&issue_url(IssueKind::Feedback, &sample()));
        assert_eq!(q[0].1, "Feedback: ");
        assert_eq!(q[1].1, "feedback");
        assert!(q[2].1.contains("Your idea or feedback"));
        assert!(!q[2].1.contains("Steps to reproduce"));
    }

    #[test]
    fn the_body_has_no_transcript_fields() {
        // The context type is the whole input: every line of the "About this
        // Mac" block is one of its version/id/class fields, and nothing
        // else can reach the URL.
        for kind in [IssueKind::Problem, IssueKind::Feedback] {
            let body = query(&issue_url(kind, &sample()))[2].1.clone();
            let about = body.split("### About this Mac\n\n").nth(1).unwrap();
            let labels: Vec<&str> = about
                .lines()
                .filter(|l| !l.is_empty())
                .map(|l| l.split(':').next().unwrap())
                .collect();
            assert_eq!(
                labels,
                [
                    "- Wispr Bro",
                    "- macOS",
                    "- Mac",
                    "- App language",
                    "- Speech model"
                ]
            );
            for word in ["transcript", "History", "dictionary", "snippet", "log:"] {
                assert!(!about.contains(word), "{word} in {about}");
            }
        }
        // `gather` and the builder never read History, the database or
        // text: the module names none of them outside these tests.
        let src = include_str!("issue_url.rs");
        let production = src.split("#[cfg(test)]").next().unwrap();
        for forbidden in [
            "ForkDb",
            "history",
            "recovery::last",
            "custom_words",
            "transcription",
            "with_conn",
            "app_log_dir",
            "clipboard",
        ] {
            assert!(
                !production.contains(forbidden),
                "issue_url.rs production code names {forbidden}"
            );
        }
    }

    #[test]
    fn unknown_or_hostile_context_values_are_dropped_or_bounded() {
        let ctx = IssueContext {
            app_version: "1.0.0".into(),
            macos_version: Some("26.1\n- Injected: line".into()),
            chip: Some("  ".into()),
            ram_gb: None,
            app_language: None,
            speech_model: Some("m".repeat(500)),
        };
        let body = query(&issue_url(IssueKind::Problem, &ctx))[2].1.clone();
        assert!(body.contains("- macOS: unknown"));
        assert!(!body.contains("Injected"));
        assert!(body.contains("- Mac: unknown"));
        assert!(body.contains("- App language: unknown"));
        assert!(body.contains(&format!("- Speech model: {}\n", "m".repeat(64))));
        assert!(!body.contains(&"m".repeat(65)));
    }

    #[test]
    fn urls_stay_well_under_githubs_limit() {
        let mut ctx = sample();
        ctx.chip = Some("é".repeat(200));
        ctx.speech_model = Some("ü".repeat(200));
        for kind in [IssueKind::Problem, IssueKind::Feedback] {
            let url = issue_url(kind, &ctx);
            assert!(url.len() <= MAX_URL_LEN, "{}", url.len());
            assert!(url.contains("body="));
        }
        assert!(Url::parse(NEW_ISSUE_URL).is_ok());
    }

    #[test]
    fn the_probe_reads_this_mac_without_panicking() {
        let (os, _chip, ram) = probe_machine();
        if cfg!(target_os = "macos") {
            assert!(os.is_some_and(|v| v.chars().next().unwrap().is_ascii_digit()));
            assert!(ram.is_some_and(|gb| gb >= 4));
        }
    }
}
