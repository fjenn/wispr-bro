// SPDX-License-Identifier: GPL-3.0-or-later
//! Context awareness for one take (S-20, DEC-24): the hints of the take in
//! flight, in memory only, and when they go.
//!
//! 1. Record start ([`on_record_start`]): with the setting on, one read is
//!    queued on the AX thread ([`super::caret`]); it never waits. The read's
//!    hints land in the slot for this take only (a newer take's slot is
//!    never overwritten by an older read).
//! 2. Key-up ([`on_stopped`], from `fork::take_file_name`): the slot is
//!    armed. Only an armed slot gives hints, so a file transcription or a
//!    retry outside a dictation never sees any.
//! 3. The take uses them: the recognizer's vocabulary
//!    ([`vocabulary`], `managers/transcription.rs` hook), the spelling stage
//!    (`fork::text::process`), the cleanup prompt (`fork::llm::post_process`).
//!    Any use marks the take ([`note_used`]): the pill shows the context
//!    glyph and the history row says "Context used".
//! 4. The take ends ([`on_take_over`]: Handy hid its bar after the paste,
//!    an empty take, an error or a cancel): the slot is cleared. A slot left
//!    armed longer than [`ARMED_TTL`] is never used and is dropped.
//!
//! Nothing here is written to disk or logged: the log gets counts, times
//! and skip reasons only.

use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use super::caret::{Read, Skip};
use super::file_names::NameSet;
use super::workspace;

/// File names a take adds to the recognizer's vocabulary at most (open tabs
/// first, then the active file's folder and the project's top level).
pub const MAX_FILE_HINTS: usize = 8;
/// How long the text stage waits for a project's first list (a root seen
/// for the first time; later takes use the cached list at once).
pub const FIRST_LIST_WAIT: Duration = Duration::from_millis(300);

/// An armed slot older than this is dropped (a take that never ended).
pub const ARMED_TTL: Duration = Duration::from_secs(120);

#[derive(Debug, Default)]
struct Slot {
    generation: u64,
    /// `None` while the read runs (or when it found nothing).
    hints: Option<Vec<String>>,
    identifiers: Option<Vec<String>>,
    /// S-36: the file names in the editor's tabs and explorer.
    visible_files: Vec<String>,
    /// S-36: a set-up editor was in front at record start.
    editor: bool,
    /// S-36: the editor whose chat input had focus at record start.
    chat_editor: Option<super::ide::Editor>,
    /// S-36: the project root the editor window showed, and its active
    /// file relative to it (`src/user_service.ts`).
    root: Option<PathBuf>,
    document: Option<String>,
    /// S-36: the take's file names, built once (with the project's list
    /// when `.0`).
    names: Option<(bool, Arc<NameSet>)>,
    armed_at: Option<Instant>,
    used: bool,
    vocabulary_sent: bool,
}

static SLOT: Mutex<Option<Slot>> = Mutex::new(None);
static GENERATION: Mutex<u64> = Mutex::new(0);

fn slot() -> std::sync::MutexGuard<'static, Option<Slot>> {
    SLOT.lock().unwrap_or_else(|e| e.into_inner())
}

/// A new take: its slot replaces the last one. Returns its generation.
pub fn begin() -> u64 {
    let generation = {
        let mut g = GENERATION.lock().unwrap_or_else(|e| e.into_inner());
        *g += 1;
        *g
    };
    *slot() = Some(Slot {
        generation,
        ..Slot::default()
    });
    generation
}

/// The read for take `generation` finished.
pub fn land(generation: u64, read: Read) {
    log::info!("{}", log_line(&read));
    let mut s = slot();
    if let Some(s) = s.as_mut().filter(|s| s.generation == generation) {
        s.identifiers =
            (read.code_names || !read.identifiers.is_empty()).then_some(read.identifiers);
        s.visible_files = read.file_names;
        s.hints = (!read.hints.is_empty()).then_some(read.hints);
        s.editor = read.ide.is_some();
        s.chat_editor = read.ide.and_then(|(editor, chat)| chat.then_some(editor));
        if let Some(hint) = read.workspace.filter(|_| s.editor) {
            s.root = workspace::resolve(&hint);
            if let Some(root) = s.root.as_deref() {
                workspace::ensure(root);
                s.document = hint
                    .document
                    .as_deref()
                    .and_then(|d| d.strip_prefix(root).ok())
                    .and_then(|d| d.to_str())
                    .map(str::to_owned);
            }
            log::debug!(
                "context: project root {}",
                if s.root.is_some() {
                    "found"
                } else {
                    "not found"
                }
            );
        }
    }
}

/// The one log line of a read: the outcome, counts and time, never a hint,
/// code name, file name or any of the text. An editor read (S-36) adds the
/// editor's id, whether its chat input had focus, and its two counts.
pub fn log_line(read: &Read) -> String {
    let ide = match read.ide {
        Some((editor, chat)) => format!(
            ", {}{}: {} code name(s), {} file name(s)",
            editor.id(),
            if chat { " chat" } else { "" },
            read.identifiers.len(),
            read.file_names.len()
        ),
        None => String::new(),
    };
    format!(
        "context: read {} ({} char(s), {} hint(s){ide}, {:.1} ms)",
        read.skip.map(Skip::as_str).unwrap_or("ok"),
        read.chars,
        read.hints.len(),
        read.ms
    )
}

/// Key-up of the running take.
pub fn on_stopped() {
    if let Some(s) = slot().as_mut() {
        s.armed_at = Some(Instant::now());
    }
}

/// The take ended: its hints go.
pub fn on_take_over() {
    *slot() = None;
}

fn armed_value(select: impl FnOnce(&Slot) -> Option<Vec<String>>) -> Option<Vec<String>> {
    let mut guard = slot();
    let s = guard.as_ref()?;
    let armed = s.armed_at?;
    if armed.elapsed() > ARMED_TTL {
        *guard = None;
        return None;
    }
    select(s)
}

/// The armed take's identifiers (a copy), if any.
pub fn identifiers() -> Option<Vec<String>> {
    armed_value(|s| s.identifiers.clone())
}

/// The armed take's file names: the visible ones (and the dotfile names
/// among its code names) first, then the project's list when its root was
/// found. `None` outside a set-up editor or when no name is known.
fn take_names() -> Option<Arc<NameSet>> {
    let (visible, root, cached) = {
        let mut guard = slot();
        let s = guard.as_ref()?;
        let armed = s.armed_at?;
        if armed.elapsed() > ARMED_TTL {
            *guard = None;
            return None;
        }
        if !s.editor {
            return None;
        }
        let mut visible = s.visible_files.clone();
        visible.extend(
            s.identifiers
                .iter()
                .flatten()
                .filter(|id| id.starts_with('.'))
                .cloned(),
        );
        (visible, s.root.clone(), s.names.clone())
    };
    let project = root
        .as_deref()
        .and_then(|r| workspace::names(r).or_else(|| workspace::wait_for(r, FIRST_LIST_WAIT)));
    if let Some((with_project, names)) = cached {
        if with_project || project.is_none() {
            return (!names.is_empty()).then_some(names);
        }
    }
    let names = Arc::new(NameSet::build(
        visible.iter().map(String::as_str),
        project
            .iter()
            .flat_map(|p| p.names().iter().map(String::as_str)),
    ));
    if let Some(s) = slot().as_mut() {
        s.names = Some((project.is_some(), names.clone()));
    }
    (!names.is_empty()).then_some(names)
}

/// The armed chat take's file names (visible, then the project's), for its
/// `@` tags, if any.
pub fn file_names() -> Option<Arc<NameSet>> {
    chat_editor()?;
    take_names()
}

/// The armed take's file names for respelling spoken ones, when
/// "Recognize code names" applied to its read.
pub fn code_file_names() -> Option<Arc<NameSet>> {
    if !slot().as_ref().is_some_and(|s| s.identifiers.is_some()) {
        return None;
    }
    take_names()
}

/// The file names the recognizer is told about for the armed take: open
/// tabs and explorer rows first, then the active file's folder and the
/// project's top level, at most [`MAX_FILE_HINTS`].
fn file_hints() -> Vec<String> {
    let (mut out, root, document) = {
        let guard = slot();
        let Some(s) = guard.as_ref().filter(|s| s.editor && s.armed_at.is_some()) else {
            return Vec::new();
        };
        let visible: Vec<String> = s
            .visible_files
            .iter()
            .take(MAX_FILE_HINTS)
            .cloned()
            .collect();
        (visible, s.root.clone(), s.document.clone())
    };
    if let Some(project) = root.as_deref().and_then(workspace::names) {
        for name in project.near(document.as_deref(), MAX_FILE_HINTS) {
            if out.len() >= MAX_FILE_HINTS {
                break;
            }
            if !out.contains(&name) {
                out.push(name);
            }
        }
    }
    out
}

/// S-36: the editor whose chat input the armed take goes to, if the read
/// found one (the delivery types its file tags through the editor's own
/// `@` picker).
pub fn chat_editor() -> Option<super::ide::Editor> {
    let mut guard = slot();
    let s = guard.as_ref()?;
    let armed = s.armed_at?;
    if armed.elapsed() > ARMED_TTL {
        *guard = None;
        return None;
    }
    s.chat_editor
}

/// The armed take's hints (a copy), if any. IDE identifiers lead, followed by
/// S-20's names, within the same per-take cap.
pub fn hints() -> Option<Vec<String>> {
    armed_value(|s| {
        let mut out = s.identifiers.clone().unwrap_or_default();
        for hint in s.hints.clone().unwrap_or_default() {
            if out.len() == super::hints::MAX_HINTS {
                break;
            }
            if !out.contains(&hint) {
                out.push(hint);
            }
        }
        (!out.is_empty()).then_some(out)
    })
}

/// The armed take used its hints: the pill shows the glyph (once).
pub fn note_used() {
    let first = {
        let mut g = slot();
        match g.as_mut() {
            Some(s) if !s.used => {
                s.used = true;
                true
            }
            _ => false,
        }
    };
    if first {
        log::info!("context: this take used its hints");
        crate::fork::pill::context_used();
    }
}

/// Whether the armed take used its hints so far.
pub fn used() -> bool {
    slot().as_ref().is_some_and(|s| s.used)
}

/// Whether the recognizer got hints for this take.
pub fn vocabulary_sent() -> bool {
    slot().as_ref().is_some_and(|s| s.vocabulary_sent)
}

/// The `managers/transcription.rs` hook: the vocabulary for a model that
/// takes one (`supports`), with the armed take's hints after the user's
/// words. Unchanged when the model takes none or there are no hints.
pub fn vocabulary(supports: bool, custom_words: Vec<String>) -> Vec<String> {
    if !supports {
        return custom_words;
    }
    // Code names and S-20's names lead; a few file names always fit.
    let files = file_hints();
    let mut terms = hints().unwrap_or_default();
    terms
        .truncate(super::hints::MAX_VOCABULARY.saturating_sub(files.len().min(MAX_FILE_HINTS / 2)));
    for name in files {
        if !terms.contains(&name) {
            terms.push(name);
        }
    }
    if terms.is_empty() {
        return custom_words;
    }
    let out = super::hints::vocabulary_terms(&custom_words, &terms);
    if out.len() > custom_words.len() {
        if let Some(s) = slot().as_mut() {
            s.vocabulary_sent = true;
        }
        note_used();
    }
    out
}

/// Record start (`fork::on_record_start`): queues the read when context
/// awareness is on (`enabled`). `languages`: the user's spoken languages;
/// `vibe`: the vibe-coding settings (S-36), which only add to a read context
/// awareness allows.
pub fn on_record_start(
    enabled: bool,
    vibe: crate::fork::settings::VibeCodingSettings,
    languages: Vec<String>,
) {
    let generation = begin();
    if !enabled {
        land(generation, Read::skipped(Skip::Off, 0.0));
        return;
    }
    #[cfg(target_os = "macos")]
    {
        if !crate::fork::ax::accessibility_trusted() {
            land(generation, Read::skipped(Skip::NoAccess, 0.0));
            return;
        }
        read_on_ax_thread(generation, languages, vibe, 0);
    }
    #[cfg(not(target_os = "macos"))]
    {
        let _ = (languages, vibe);
        land(generation, Read::skipped(Skip::NoAccess, 0.0));
    }
}

/// S-36: a read that found a Chromium/Electron tree still building (our
/// lease switched it on just now) is repeated this often, at most
/// [`WARM_RETRIES`] times, while its take is the current one. Each read
/// keeps its own budget; the take only uses the result after key-up.
pub const WARM_RETRY_GAP: Duration = Duration::from_millis(400);
pub const WARM_RETRIES: u32 = 5;

/// Whether a read that came back `warming` is worth repeating.
pub fn retry_warming(read: &Read, attempt: u32, generation: u64) -> bool {
    read.warming && attempt < WARM_RETRIES && is_current(generation)
}

/// Take `generation` is the one in flight.
pub fn is_current(generation: u64) -> bool {
    slot().as_ref().is_some_and(|s| s.generation == generation)
}

#[cfg(target_os = "macos")]
fn read_on_ax_thread(
    generation: u64,
    languages: Vec<String>,
    vibe: crate::fork::settings::VibeCodingSettings,
    attempt: u32,
) {
    crate::fork::ax::thread::post(move |ctx| {
        let read = super::caret::mac::read(ctx, &languages, &vibe);
        if retry_warming(&read, attempt, generation) {
            log::debug!(
                "context: the app's tree is still building; reading again ({})",
                attempt + 1
            );
            let spawned = std::thread::Builder::new()
                .name("wb-context-retry".into())
                .spawn(move || {
                    std::thread::sleep(WARM_RETRY_GAP);
                    read_on_ax_thread(generation, languages, vibe, attempt + 1);
                });
            if spawned.is_ok() {
                return;
            }
        }
        land(generation, read);
    });
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;

    /// The slot is process-wide; tests that use it hold this.
    pub(crate) static LOCK: Mutex<()> = Mutex::new(());

    pub(crate) fn lock() -> std::sync::MutexGuard<'static, ()> {
        LOCK.lock().unwrap_or_else(|e| e.into_inner())
    }

    fn read(hints: &[&str]) -> Read {
        Read {
            hints: hints.iter().map(|h| h.to_string()).collect(),
            chars: 120,
            ms: 3.0,
            ..Read::default()
        }
    }

    /// A take whose read found `hints`, stopped.
    pub(crate) fn armed_take(hints: &[&str]) {
        let g = begin();
        land(g, read(hints));
        on_stopped();
    }

    /// An editor take (S-36) whose read found `identifiers` and the
    /// visible `file_names`, stopped.
    pub(crate) fn armed_editor_take(identifiers: &[&str], file_names: &[&str]) {
        let g = begin();
        let mut r = read(&[]);
        r.identifiers = identifiers.iter().map(|s| s.to_string()).collect();
        r.file_names = file_names.iter().map(|s| s.to_string()).collect();
        r.code_names = true;
        r.ide = Some((super::super::ide::Editor::Cursor, true));
        land(g, r);
        on_stopped();
    }

    #[test]
    fn a_warming_read_is_repeated_only_for_the_current_take() {
        let _l = lock();
        let g = begin();
        let mut r = read(&[]);
        assert!(!retry_warming(&r, 0, g), "a built tree is read once");
        r.warming = true;
        assert!(retry_warming(&r, 0, g));
        assert!(retry_warming(&r, WARM_RETRIES - 1, g));
        assert!(!retry_warming(&r, WARM_RETRIES, g), "bounded");
        let newer = begin();
        assert!(!retry_warming(&r, 0, g), "a newer take replaced it");
        assert!(retry_warming(&r, 0, newer));
        on_take_over();
        assert!(!retry_warming(&r, 0, newer), "the take ended");
    }

    #[test]
    fn the_chat_editor_is_kept_only_for_a_chat_read() {
        let _l = lock();
        let g = begin();
        let mut r = read(&[]);
        r.ide = Some((super::super::ide::Editor::Cursor, true));
        land(g, r.clone());
        assert_eq!(chat_editor(), None, "not armed yet");
        on_stopped();
        assert_eq!(chat_editor(), Some(super::super::ide::Editor::Cursor));
        let g = begin();
        r.ide = Some((super::super::ide::Editor::Cursor, false));
        land(g, r);
        on_stopped();
        assert_eq!(chat_editor(), None, "the editor buffer is no chat");
        on_take_over();
    }

    #[test]
    fn hints_are_given_only_between_key_up_and_the_end_of_the_take() {
        let _l = lock();
        let g = begin();
        land(g, read(&["Siobhán"]));
        // Not armed yet: a retry or a file job during the recording sees
        // nothing.
        assert_eq!(hints(), None);
        on_stopped();
        assert_eq!(hints(), Some(vec!["Siobhán".to_string()]));
        on_take_over();
        assert_eq!(hints(), None);
        assert!(!used());
    }

    #[test]
    fn an_older_read_never_fills_a_newer_take() {
        let _l = lock();
        let old = begin();
        let new = begin();
        land(old, read(&["Old"]));
        on_stopped();
        assert_eq!(hints(), None);
        land(new, read(&["New"]));
        assert_eq!(hints(), Some(vec!["New".to_string()]));
        on_take_over();
    }

    #[test]
    fn a_skipped_or_disabled_read_gives_nothing() {
        let _l = lock();
        on_record_start(false, Default::default(), Vec::new());
        on_stopped();
        assert_eq!(hints(), None);
        let g = begin();
        land(g, Read::skipped(Skip::Timeout, 140.0));
        on_stopped();
        assert_eq!(hints(), None);
        on_take_over();
    }

    #[test]
    fn vocabulary_only_for_models_that_take_one() {
        let _l = lock();
        armed_take(&["Kowalczyk"]);
        let words = vec!["Wispr Bro".to_string()];
        assert_eq!(vocabulary(false, words.clone()), words);
        assert!(!used());
        assert_eq!(vocabulary(true, words.clone()), ["Wispr Bro", "Kowalczyk"]);
        assert!(used() && vocabulary_sent());
        on_take_over();
        // No take: Handy's words unchanged.
        assert_eq!(vocabulary(true, words.clone()), words);
    }

    #[test]
    fn the_log_line_has_counts_only() {
        let line = log_line(&read(&["Siobhán", "Kowalczyk"]));
        assert_eq!(line, "context: read ok (120 char(s), 2 hint(s), 3.0 ms)");
        assert_eq!(
            log_line(&Read::skipped(Skip::PasswordManager, 0.4)),
            "context: read password_manager (0 char(s), 0 hint(s), 0.4 ms)"
        );
    }

    #[test]
    fn an_editor_read_logs_its_editor_and_counts_never_names() {
        let mut r = read(&["Siobhán"]);
        r.identifiers = vec!["getUserById".into()];
        r.file_names = vec!["index.ts".into(), "app.tsx".into()];
        r.ide = Some((super::super::ide::Editor::Cursor, true));
        let line = log_line(&r);
        assert_eq!(
            line,
            "context: read ok (120 char(s), 1 hint(s), cursor chat: 1 code name(s), 2 file name(s), 3.0 ms)"
        );
        assert!(!line.contains("getUserById") && !line.contains("index.ts"));
    }

    #[test]
    fn code_names_lead_the_hints_and_file_names_stay_apart() {
        let _l = lock();
        let g = begin();
        let mut r = read(&["Priya", "getUserById"]);
        r.identifiers = vec!["getUserById".into(), "user_id".into()];
        r.file_names = vec!["index.ts".into()];
        r.ide = Some((super::super::ide::Editor::Cursor, true));
        land(g, r);
        assert_eq!(identifiers(), None, "not armed yet");
        assert!(file_names().is_none(), "not armed yet");
        on_stopped();
        assert_eq!(
            hints(),
            Some(vec!["getUserById".into(), "user_id".into(), "Priya".into()])
        );
        let names = file_names().expect("chat take");
        assert_eq!(names.exact("index.ts"), Some("index.ts"));
        on_take_over();
        assert_eq!(identifiers(), None);
        assert!(file_names().is_none());
    }

    #[test]
    fn tags_need_the_chat_and_respelling_needs_code_names() {
        let _l = lock();
        // The editor buffer (no chat): names to respell, no tags.
        let g = begin();
        let mut r = read(&[]);
        r.file_names = vec!["index.ts".into()];
        r.code_names = true;
        r.ide = Some((super::super::ide::Editor::Cursor, false));
        land(g, r);
        on_stopped();
        assert!(file_names().is_none());
        assert!(code_file_names().is_some_and(|n| n.exact("index.ts").is_some()));
        // Not an editor: neither.
        armed_take(&["Priya"]);
        assert!(file_names().is_none() && code_file_names().is_none());
        on_take_over();
    }

    #[test]
    fn the_project_list_joins_the_visible_names() {
        let _l = lock();
        workspace::clear();
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().to_path_buf();
        for rel in [
            "src/user_service.ts",
            "src/index.ts",
            ".gitattributes",
            "README.md",
        ] {
            let path = root.join(rel);
            std::fs::create_dir_all(path.parent().unwrap()).unwrap();
            std::fs::write(path, b"").unwrap();
        }
        let g = begin();
        let mut r = read(&[]);
        r.file_names = vec!["index.ts".into()];
        r.identifiers = vec!["getUserById".into()];
        r.code_names = true;
        r.ide = Some((super::super::ide::Editor::Cursor, true));
        land(g, r);
        // The read's root (resolved from AX in the app; set here).
        if let Some(s) = slot().as_mut() {
            s.root = Some(root.clone());
            s.document = Some("src/user_service.ts".into());
        }
        // The list runs on its worker; the text stage waits up to
        // FIRST_LIST_WAIT for a first one, a loaded test machine longer.
        workspace::ensure(&root);
        assert!(workspace::wait_for(&root, Duration::from_secs(20)).is_some());
        on_stopped();
        let names = file_names().expect("names");
        assert_eq!(names.exact("user_service.ts"), Some("user_service.ts"));
        assert_eq!(names.spoken(".gitattributes"), Some(".gitattributes"));
        assert!(code_file_names().is_some());
        // The recognizer hears the open tab first, then the active file's
        // folder; code names lead.
        assert_eq!(
            vocabulary(true, vec!["Wispr".into()]),
            [
                "Wispr",
                "getUserById",
                "index.ts",
                "user_service.ts",
                ".gitattributes",
                "README.md",
                "src"
            ]
        );
        on_take_over();
        workspace::clear();
    }

    #[test]
    fn a_stale_armed_slot_is_dropped() {
        let _l = lock();
        armed_take(&["Kowalczyk"]);
        if let Some(s) = slot().as_mut() {
            s.armed_at = Instant::now().checked_sub(ARMED_TTL + Duration::from_secs(1));
        }
        assert_eq!(hints(), None);
        assert!(slot().is_none(), "the slot is gone, not just hidden");
    }
}
