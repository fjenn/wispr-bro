// SPDX-License-Identifier: GPL-3.0-or-later
//! Vibe coding (S-36, owner's request 2026-11-04): the file and folder names
//! of the project open in the editor in front, so a spoken file name can
//! become the project's exact name and an `@` tag can name any file in it.
//!
//! 1. The root ([`resolve`]): the editor window's `AXDocument` (the active
//!    file; VS Code, Cursor and Windsurf set it through Electron's
//!    `representedFilename`) and its title's folder name. The nearest
//!    ancestor of the file named like a title segment is the workspace
//!    folder; else the git work tree around the file. Never the home folder
//!    or anything above it. A window without a document reuses a root
//!    listed before whose folder name its title shows.
//! 2. The list ([`list`]): names and relative paths only, never contents.
//!    Inside a git work tree `git ls-files --cached --others
//!    --exclude-standard` (what `.gitignore` keeps out stays out), run from
//!    a real git binary only (never `/usr/bin/git`, whose stub asks to
//!    install the developer tools); else a bounded walk that skips
//!    dependency, build and VCS folders. At most [`MAX_ENTRIES`] paths in
//!    [`LIST_BUDGET`].
//! 3. The cache ([`ensure`], [`names`]): per root, in memory only, listed on
//!    a worker thread at a take's start when missing or older than
//!    [`STALE_AFTER`] (a take uses the last list meanwhile). Never written
//!    to disk; paths and names are never logged above DEBUG.

use std::collections::{HashMap, HashSet};
use std::io::Read as _;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

/// Paths one list keeps at most.
pub const MAX_ENTRIES: usize = 50_000;
/// One list's time budget (git or the walk); what it found by then is kept.
pub const LIST_BUDGET: Duration = Duration::from_millis(500);
/// A list older than this is listed again at the next take's start.
pub const STALE_AFTER: Duration = Duration::from_secs(30);
/// Roots kept in memory at once (least recently used goes).
const MAX_ROOTS: usize = 6;
/// How far up from the active file the root may be.
const MAX_ANCESTORS: usize = 40;
/// How deep the walk goes below the root.
const MAX_WALK_DEPTH: usize = 16;

/// Folders the walk (no git) never enters: version control, dependencies,
/// build output and caches. Inside git, `.gitignore` decides instead.
const SKIPPED_DIRS: &[&str] = &[
    ".git",
    ".hg",
    ".svn",
    "node_modules",
    "bower_components",
    "target",
    "build",
    "dist",
    "out",
    ".next",
    ".nuxt",
    ".turbo",
    ".cache",
    ".gradle",
    ".venv",
    "venv",
    "__pycache__",
    "Pods",
    "DerivedData",
    "coverage",
];

/// What the editor window says about its project (from AX, in memory).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct WindowHint {
    /// The active file (`AXDocument`).
    pub document: Option<PathBuf>,
    /// The window title.
    pub title: Option<String>,
}

/// The local path of an `AXDocument` value: a `file://` URL (percent
/// escapes decoded) or an absolute path. Remote and other URLs are `None`.
pub fn document_path(value: &str) -> Option<PathBuf> {
    let value = value.trim();
    let path = if let Some(rest) = value.strip_prefix("file://") {
        // `file:///Users/...` or `file://localhost/Users/...`.
        let rest = rest.strip_prefix("localhost").unwrap_or(rest);
        if !rest.starts_with('/') {
            return None;
        }
        percent_decode(rest)?
    } else if value.starts_with('/') {
        value.to_owned()
    } else {
        return None;
    };
    let path = PathBuf::from(path.trim_end_matches('/'));
    path.is_absolute().then_some(path)
}

fn percent_decode(text: &str) -> Option<String> {
    let bytes = text.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%' {
            let hex = text.get(i + 1..i + 3)?;
            out.push(u8::from_str_radix(hex, 16).ok()?);
            i += 3;
        } else {
            out.push(bytes[i]);
            i += 1;
        }
    }
    String::from_utf8(out).ok()
}

/// The folder names a VS Code-family window title may show: its segments
/// split by " — " or " - " (`user_service.ts — my-app`, `● index.ts - api -
/// Cursor`), without a dirty mark or a `(Workspace)` suffix.
pub fn title_folders(title: &str) -> Vec<String> {
    title
        .split(" \u{2014} ")
        .flat_map(|part| part.split(" - "))
        .map(|part| {
            part.trim()
                .trim_start_matches(['\u{25cf}', '\u{2022}'])
                .trim()
                .trim_end_matches("(Workspace)")
                .trim()
                .to_owned()
        })
        .filter(|part| !part.is_empty() && !part.contains('/'))
        .collect()
}

fn home() -> Option<PathBuf> {
    std::env::var_os("HOME")
        .map(PathBuf::from)
        .filter(|h| h.is_absolute())
}

/// Whether `dir` may be a root: strictly inside the home folder when there
/// is one, never `/` or a top-level folder.
fn may_be_root(dir: &Path, home: Option<&Path>) -> bool {
    match home {
        Some(home) => dir.starts_with(home) && dir != home,
        None => dir.components().count() > 2,
    }
}

/// The git work tree that holds `dir` (its folder with a `.git` folder or
/// file), at or below the home folder's children.
pub fn git_root(dir: &Path, home: Option<&Path>) -> Option<PathBuf> {
    dir.ancestors()
        .take(MAX_ANCESTORS)
        .take_while(|a| may_be_root(a, home))
        .find(|a| a.join(".git").exists())
        .map(Path::to_path_buf)
}

/// The project root for an editor window (see the module docs). `home`:
/// the home folder (tests pass their own).
pub fn resolve_with(hint: &WindowHint, home: Option<&Path>) -> Option<PathBuf> {
    let folders = hint.title.as_deref().map(title_folders).unwrap_or_default();
    let Some(document) = hint.document.as_deref() else {
        return folders.iter().find_map(|f| last_root_named(f));
    };
    let dir = if document.is_dir() {
        document
    } else {
        document.parent()?
    };
    if !dir.is_dir() {
        return None;
    }
    let named = |exact: bool| {
        dir.ancestors()
            .take(MAX_ANCESTORS)
            .take_while(|a| may_be_root(a, home))
            .find(|a| {
                a.file_name().and_then(|n| n.to_str()).is_some_and(|name| {
                    folders.iter().any(|f| {
                        if exact {
                            f == name
                        } else {
                            f.eq_ignore_ascii_case(name)
                        }
                    })
                })
            })
            .map(Path::to_path_buf)
    };
    named(true)
        .or_else(|| named(false))
        .or_else(|| git_root(dir, home))
}

/// [`resolve_with`] for this user's home folder.
pub fn resolve(hint: &WindowHint) -> Option<PathBuf> {
    resolve_with(hint, home().as_deref())
}

// ---------------------------------------------------------------------------
// The list

/// How a list was made (for the DEBUG log).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Source {
    Git,
    Walk,
}

/// One project's names: relative paths (`/`-separated) and the unique file
/// and folder names in them, shallow first.
#[derive(Debug, Default)]
pub struct Project {
    paths: Vec<String>,
    names: Vec<String>,
}

impl Project {
    pub fn from_paths(mut paths: Vec<String>) -> Project {
        paths.retain(|p| !p.is_empty());
        let mut order: Vec<(usize, usize, &str)> = Vec::new();
        let mut seen = HashSet::new();
        for (i, path) in paths.iter().enumerate() {
            for (depth, part) in path.split('/').enumerate() {
                if !part.is_empty() && seen.insert(part) {
                    order.push((depth, i, part));
                }
            }
        }
        order.sort();
        let names = order.into_iter().map(|(.., n)| n.to_owned()).collect();
        Project { paths, names }
    }

    /// The unique file and folder names, shallow first.
    pub fn names(&self) -> &[String] {
        &self.names
    }

    pub fn len(&self) -> usize {
        self.paths.len()
    }

    pub fn is_empty(&self) -> bool {
        self.paths.is_empty()
    }

    /// The names most likely spoken next to the active file `document`
    /// (relative to the root): its own, its folder's other entries, then
    /// the root's, at most `n`.
    pub fn near(&self, document: Option<&str>, n: usize) -> Vec<String> {
        let dir = document.and_then(|d| d.rsplit_once('/')).map(|(d, _)| d);
        let mut out: Vec<String> = Vec::new();
        let push = |name: &str, out: &mut Vec<String>| {
            if out.len() < n && !out.iter().any(|o| o == name) {
                out.push(name.to_owned());
            }
        };
        if let Some(own) = document.map(|d| d.rsplit('/').next().unwrap_or(d)) {
            push(own, &mut out);
        }
        for path in &self.paths {
            let (parent, name) = path.rsplit_once('/').unwrap_or(("", path));
            if Some(parent) == dir || (dir.is_none() && parent.is_empty()) {
                push(name, &mut out);
            }
        }
        for path in &self.paths {
            let top = path.split('/').next().unwrap_or(path);
            push(top, &mut out);
        }
        out
    }
}

/// A real git binary, never the `/usr/bin/git` stub (with no developer
/// tools installed it opens an install dialog).
fn git_binary() -> Option<PathBuf> {
    let mut candidates = vec![
        PathBuf::from("/opt/homebrew/bin/git"),
        PathBuf::from("/usr/local/bin/git"),
    ];
    if let Ok(dev) = std::fs::read_link("/var/db/xcode_select_link") {
        candidates.push(dev.join("usr/bin/git"));
    }
    candidates.push(PathBuf::from(
        "/Library/Developer/CommandLineTools/usr/bin/git",
    ));
    candidates.push(PathBuf::from(
        "/Applications/Xcode.app/Contents/Developer/usr/bin/git",
    ));
    candidates.into_iter().find(|p| p.is_file())
}

fn list_git(git: &Path, root: &Path, cap: usize, deadline: Instant) -> Option<Vec<String>> {
    use std::process::{Command, Stdio};
    let mut child = Command::new(git)
        .arg("-C")
        .arg(root)
        .args([
            "--no-optional-locks",
            "-c",
            "core.fsmonitor=false",
            "-c",
            "core.quotePath=false",
            "ls-files",
            "--cached",
            "--others",
            "--exclude-standard",
            "-z",
        ])
        .env_remove("GIT_DIR")
        .env_remove("GIT_WORK_TREE")
        .env_remove("GIT_INDEX_FILE")
        .env("GIT_OPTIONAL_LOCKS", "0")
        .env("GIT_TERMINAL_PROMPT", "0")
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .ok()?;
    let mut stdout = child.stdout.take()?;
    // A watchdog ends a git that runs past the budget; what it wrote is kept.
    let pid = child.id();
    let done = Arc::new(Mutex::new(false));
    let watchdog = {
        let done = done.clone();
        std::thread::Builder::new()
            .name("wb-workspace-git".into())
            .spawn(move || {
                while Instant::now() < deadline {
                    if *done.lock().unwrap_or_else(|e| e.into_inner()) {
                        return;
                    }
                    std::thread::sleep(Duration::from_millis(10));
                }
                let done = done.lock().unwrap_or_else(|e| e.into_inner());
                if !*done {
                    // SAFETY: a plain signal to our own child, which is not
                    // reaped yet: `done` is set, under this lock, before the
                    // wait.
                    #[cfg(unix)]
                    unsafe {
                        libc::kill(pid as i32, libc::SIGKILL);
                    }
                    #[cfg(not(unix))]
                    let _ = pid;
                }
            })
            .ok()
    };
    let mut paths = Vec::new();
    let mut pending = Vec::new();
    let mut buf = [0u8; 64 * 1024];
    'read: while let Ok(n) = stdout.read(&mut buf) {
        if n == 0 {
            break;
        }
        for &b in &buf[..n] {
            if b == 0 {
                if let Ok(path) = String::from_utf8(std::mem::take(&mut pending)) {
                    paths.push(path);
                }
                if paths.len() >= cap {
                    break 'read;
                }
            } else {
                pending.push(b);
            }
        }
    }
    drop(stdout);
    let status = {
        let mut done = done.lock().unwrap_or_else(|e| e.into_inner());
        *done = true;
        if paths.len() >= cap {
            let _ = child.kill();
        }
        drop(done);
        child.wait().ok()
    };
    if let Some(w) = watchdog {
        let _ = w.join();
    }
    // A git that failed outright (not a repo, unsafe directory) gave
    // nothing: the walk runs instead.
    if paths.is_empty() && !status.is_some_and(|s| s.success()) {
        return None;
    }
    Some(paths)
}

fn list_walk(root: &Path, cap: usize, deadline: Instant) -> Vec<String> {
    let mut paths = Vec::new();
    let mut queue = std::collections::VecDeque::from([(root.to_path_buf(), String::new(), 0)]);
    while let Some((dir, rel, depth)) = queue.pop_front() {
        let Ok(entries) = std::fs::read_dir(&dir) else {
            continue;
        };
        let mut entries: Vec<_> = entries.flatten().collect();
        entries.sort_by_key(|e| e.file_name());
        for entry in entries {
            if paths.len() >= cap || Instant::now() > deadline {
                return paths;
            }
            let Ok(name) = entry.file_name().into_string() else {
                continue;
            };
            // Never follow a link out of the project.
            let Ok(kind) = entry.file_type() else {
                continue;
            };
            let path = if rel.is_empty() {
                name.clone()
            } else {
                format!("{rel}/{name}")
            };
            if kind.is_dir() {
                if SKIPPED_DIRS.contains(&name.as_str()) {
                    continue;
                }
                if depth < MAX_WALK_DEPTH {
                    queue.push_back((entry.path(), path.clone(), depth + 1));
                }
                // A folder without files below still has a name.
                paths.push(format!("{path}/"));
            } else {
                paths.push(path);
            }
        }
    }
    paths
}

/// Lists the project at `root` (see the module docs).
pub fn list(root: &Path) -> (Project, Source) {
    list_with(root, git_binary().as_deref(), MAX_ENTRIES, LIST_BUDGET)
}

pub fn list_with(
    root: &Path,
    git: Option<&Path>,
    cap: usize,
    budget: Duration,
) -> (Project, Source) {
    let deadline = Instant::now() + budget;
    let in_git = git_root(root, None).is_some();
    if let (true, Some(git)) = (in_git, git) {
        if let Some(paths) = list_git(git, root, cap, deadline) {
            return (Project::from_paths(paths), Source::Git);
        }
    }
    let paths = list_walk(root, cap, deadline)
        .into_iter()
        .map(|p| p.trim_end_matches('/').to_owned())
        .collect();
    (Project::from_paths(paths), Source::Walk)
}

// ---------------------------------------------------------------------------
// The cache

#[derive(Default)]
struct Entry {
    project: Option<Arc<Project>>,
    listed_at: Option<Instant>,
    listing: bool,
    used_at: Option<Instant>,
}

static CACHE: Mutex<Option<HashMap<PathBuf, Entry>>> = Mutex::new(None);

fn cache() -> std::sync::MutexGuard<'static, Option<HashMap<PathBuf, Entry>>> {
    CACHE.lock().unwrap_or_else(|e| e.into_inner())
}

/// Makes sure `root` is listed: starts a list on a worker thread when there
/// is none or it is older than [`STALE_AFTER`]. Never waits.
pub fn ensure(root: &Path) {
    let start = {
        let mut guard = cache();
        let map = guard.get_or_insert_with(HashMap::new);
        if !map.contains_key(root) && map.len() >= MAX_ROOTS {
            if let Some(oldest) = map
                .iter()
                .filter(|(_, e)| !e.listing)
                .min_by_key(|(_, e)| e.used_at)
                .map(|(k, _)| k.clone())
            {
                map.remove(&oldest);
            }
        }
        let entry = map.entry(root.to_path_buf()).or_default();
        entry.used_at = Some(Instant::now());
        let stale = entry.listed_at.is_none_or(|t| t.elapsed() > STALE_AFTER);
        let start = stale && !entry.listing;
        if start {
            entry.listing = true;
        }
        start
    };
    if !start {
        return;
    }
    let root = root.to_path_buf();
    let spawned = std::thread::Builder::new()
        .name("wb-workspace-list".into())
        .spawn({
            let root = root.clone();
            move || {
                let began = Instant::now();
                let (project, source) = list(&root);
                log::debug!(
                    "workspace: listed {} path(s), {} name(s) by {:?} in {:.1} ms",
                    project.len(),
                    project.names().len(),
                    source,
                    began.elapsed().as_secs_f64() * 1000.0
                );
                store(&root, project);
            }
        });
    if spawned.is_err() {
        if let Some(e) = cache().as_mut().and_then(|m| m.get_mut(&root)) {
            e.listing = false;
        }
    }
}

fn store(root: &Path, project: Project) {
    let mut guard = cache();
    let map = guard.get_or_insert_with(HashMap::new);
    let entry = map.entry(root.to_path_buf()).or_default();
    entry.project = Some(Arc::new(project));
    entry.listed_at = Some(Instant::now());
    entry.listing = false;
}

/// The last list of `root`, if one is done.
pub fn names(root: &Path) -> Option<Arc<Project>> {
    cache().as_ref()?.get(root)?.project.clone()
}

/// Lists `root` now on this thread and caches it (tests, and a take whose
/// list is not there yet may wait for it with [`wait_for`]).
pub fn list_now(root: &Path) -> Arc<Project> {
    let (project, _) = list(root);
    store(root, project);
    names(root).unwrap_or_default()
}

/// Waits up to `timeout` for the list of `root` a worker is making.
pub fn wait_for(root: &Path, timeout: Duration) -> Option<Arc<Project>> {
    let until = Instant::now() + timeout;
    loop {
        let (project, listing) = {
            let guard = cache();
            match guard.as_ref().and_then(|m| m.get(root)) {
                Some(e) => (e.project.clone(), e.listing),
                None => (None, false),
            }
        };
        if project.is_some() || !listing || Instant::now() >= until {
            return project;
        }
        std::thread::sleep(Duration::from_millis(10));
    }
}

/// A root listed before whose folder is named `folder` (only one).
fn last_root_named(folder: &str) -> Option<PathBuf> {
    let guard = cache();
    let mut found = guard
        .as_ref()?
        .keys()
        .filter(|k| k.file_name().and_then(|n| n.to_str()) == Some(folder));
    let first = found.next()?.clone();
    found.next().is_none().then_some(first)
}

/// Forgets every list (erase, tests).
pub fn clear() {
    *cache() = None;
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    fn touch(root: &Path, rel: &str) {
        let path = root.join(rel);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, b"x").unwrap();
    }

    fn git(root: &Path, args: &[&str]) -> bool {
        let Some(bin) = git_binary() else {
            return false;
        };
        std::process::Command::new(bin)
            .arg("-C")
            .arg(root)
            .args(args)
            .env("GIT_CONFIG_GLOBAL", "/dev/null")
            .env("GIT_CONFIG_NOSYSTEM", "1")
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .status()
            .is_ok_and(|s| s.success())
    }

    #[test]
    fn document_urls_become_paths() {
        assert_eq!(
            document_path("file:///Users/a/my%20app/src/user_service.ts"),
            Some(PathBuf::from("/Users/a/my app/src/user_service.ts"))
        );
        assert_eq!(
            document_path("file://localhost/tmp/x/"),
            Some(PathBuf::from("/tmp/x"))
        );
        assert_eq!(document_path("/tmp/a.ts"), Some(PathBuf::from("/tmp/a.ts")));
        for remote in [
            "vscode-remote://ssh-remote+box/home/a.ts",
            "https://example.com/a.ts",
            "untitled:Untitled-1",
            "file://server/share/a.ts",
            "file:///bad%zz",
            "",
        ] {
            assert_eq!(document_path(remote), None, "{remote}");
        }
    }

    #[test]
    fn title_segments_name_folders() {
        assert_eq!(
            title_folders("user_service.ts \u{2014} my-app"),
            vec!["user_service.ts", "my-app"]
        );
        assert_eq!(
            title_folders("\u{25cf} index.ts - api - Cursor"),
            vec!["index.ts", "api", "Cursor"]
        );
        assert_eq!(
            title_folders("mono (Workspace) \u{2014} Cursor"),
            vec!["mono", "Cursor"]
        );
        assert!(title_folders("").is_empty());
    }

    #[test]
    fn the_root_is_the_title_folder_else_the_git_tree() {
        let home = tempfile::tempdir().unwrap();
        let home = home.path();
        // A git repo with a package opened as the workspace.
        let repo = home.join("code/mono");
        touch(&repo, "packages/api/src/user_service.ts");
        fs::create_dir_all(repo.join(".git")).unwrap();
        let doc = repo.join("packages/api/src/user_service.ts");
        let hint = |title: &str| WindowHint {
            document: Some(doc.clone()),
            title: Some(title.into()),
        };
        assert_eq!(
            resolve_with(&hint("user_service.ts \u{2014} api"), Some(home)),
            Some(repo.join("packages/api"))
        );
        // Case differs (a title shows the folder as typed).
        assert_eq!(
            resolve_with(&hint("user_service.ts \u{2014} API"), Some(home)),
            Some(repo.join("packages/api"))
        );
        // The title names no ancestor: the git work tree.
        assert_eq!(
            resolve_with(&hint("user_service.ts \u{2014} Cursor"), Some(home)),
            Some(repo.clone())
        );
        // A worktree's `.git` is a file.
        let wt = home.join("code/wt");
        touch(&wt, "a/b.rs");
        fs::write(wt.join(".git"), "gitdir: elsewhere").unwrap();
        let hint_wt = WindowHint {
            document: Some(wt.join("a/b.rs")),
            title: None,
        };
        assert_eq!(resolve_with(&hint_wt, Some(home)), Some(wt.clone()));
        // No git, no title folder: no root (visible names only).
        let loose = home.join("notes");
        touch(&loose, "todo.md");
        let hint_loose = WindowHint {
            document: Some(loose.join("todo.md")),
            title: Some("todo.md".into()),
        };
        assert_eq!(resolve_with(&hint_loose, Some(home)), None);
        // Never the home folder, even when it is a git repo (dotfiles).
        fs::create_dir_all(home.join(".git")).unwrap();
        assert_eq!(resolve_with(&hint_loose, Some(home)), None);
        let home_name = home.file_name().unwrap().to_str().unwrap().to_owned();
        let hint_home = WindowHint {
            document: Some(loose.join("todo.md")),
            title: Some(format!("todo.md \u{2014} {home_name}")),
        };
        assert_eq!(resolve_with(&hint_home, Some(home)), None);
        // A missing file or a remote document: none.
        let gone = WindowHint {
            document: Some(home.join("gone/x.ts")),
            title: None,
        };
        assert_eq!(resolve_with(&gone, Some(home)), None);
    }

    #[test]
    fn a_window_without_a_document_reuses_a_listed_root() {
        let _l = crate::fork::context::awareness::tests::lock();
        clear();
        let home = tempfile::tempdir().unwrap();
        let root = home.path().join("code/shop-api");
        touch(&root, "README.md");
        list_now(&root);
        let hint = WindowHint {
            document: None,
            title: Some("Cursor \u{2014} shop-api".into()),
        };
        assert_eq!(resolve_with(&hint, Some(home.path())), Some(root));
        let other = WindowHint {
            document: None,
            title: Some("Welcome \u{2014} other".into()),
        };
        assert_eq!(resolve_with(&other, Some(home.path())), None);
        clear();
    }

    #[test]
    fn git_lists_tracked_and_untracked_files_but_not_ignored_ones() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        if !git(root, &["init", "-q"]) {
            eprintln!("no git binary: skipped");
            return;
        }
        touch(root, ".gitattributes");
        touch(root, ".gitignore");
        fs::write(root.join(".gitignore"), "node_modules/\n*.log\n.env\n").unwrap();
        touch(root, "src/user_service.ts");
        touch(root, "src/components/UserCard.tsx");
        touch(root, "node_modules/left-pad/index.js");
        touch(root, "debug.log");
        touch(root, ".env");
        touch(root, "untracked/new_file.rs");
        assert!(git(root, &["add", ".gitattributes", ".gitignore", "src"]));
        let (project, source) = list(root);
        assert_eq!(source, Source::Git);
        let names: HashSet<&str> = project.names().iter().map(String::as_str).collect();
        for name in [
            ".gitattributes",
            ".gitignore",
            "src",
            "user_service.ts",
            "components",
            "UserCard.tsx",
            "untracked",
            "new_file.rs",
        ] {
            assert!(names.contains(name), "{name} in {names:?}");
        }
        for ignored in ["node_modules", "left-pad", "index.js", "debug.log", ".env"] {
            assert!(!names.contains(ignored), "{ignored} listed");
        }
        // Shallow names first.
        let at = |n: &str| project.names().iter().position(|x| x == n).unwrap();
        assert!(at("src") < at("user_service.ts"));
        assert!(at("user_service.ts") < at("UserCard.tsx"));
        // A subfolder opened as the workspace lists only itself.
        let (sub, _) = list(&root.join("src"));
        assert!(sub.names().contains(&"user_service.ts".to_string()));
        assert!(!sub.names().contains(&".gitignore".to_string()));
    }

    #[test]
    fn the_walk_skips_dependency_and_build_folders_and_stops_at_its_cap() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        touch(root, ".editorconfig");
        touch(root, "src/main.py");
        touch(root, "node_modules/x/index.js");
        touch(root, "target/debug/app");
        touch(root, ".git/HEAD");
        fs::create_dir_all(root.join("empty_dir")).unwrap();
        let (project, source) = list_with(root, None, MAX_ENTRIES, LIST_BUDGET);
        assert_eq!(source, Source::Walk);
        let names = project.names();
        for name in [".editorconfig", "src", "main.py", "empty_dir"] {
            assert!(names.contains(&name.to_string()), "{name} in {names:?}");
        }
        for skipped in ["node_modules", "index.js", "target", "app", ".git", "HEAD"] {
            assert!(!names.contains(&skipped.to_string()), "{skipped} listed");
        }
        for i in 0..50 {
            touch(root, &format!("many/file_{i}.txt"));
        }
        let (capped, _) = list_with(root, None, 20, LIST_BUDGET);
        assert_eq!(capped.len(), 20);
    }

    #[test]
    fn a_big_repo_lists_within_its_budget() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        if !git(root, &["init", "-q"]) {
            return;
        }
        for d in 0..60 {
            for f in 0..100 {
                touch(root, &format!("pkg_{d}/mod_{f}.rs"));
            }
        }
        let began = Instant::now();
        let (project, _) = list(root);
        assert_eq!(project.len(), 6_000);
        // The budget plus room for a loaded machine's process start.
        assert!(began.elapsed() < LIST_BUDGET + Duration::from_secs(2));
        let (capped, _) = list_with(root, git_binary().as_deref(), 1_000, LIST_BUDGET);
        assert_eq!(capped.len(), 1_000);
    }

    #[test]
    fn names_near_the_active_file_come_first() {
        let project = Project::from_paths(
            [
                ".gitignore",
                "README.md",
                "src/index.ts",
                "src/user_service.ts",
                "src/deep/util.ts",
                "docs/guide.md",
            ]
            .iter()
            .map(|s| s.to_string())
            .collect(),
        );
        assert_eq!(
            project.near(Some("src/user_service.ts"), 4),
            vec!["user_service.ts", "index.ts", ".gitignore", "README.md"]
        );
        assert_eq!(
            project.near(None, 3),
            vec![".gitignore", "README.md", "src"]
        );
    }

    #[test]
    fn the_cache_lists_once_and_serves_the_list() {
        let _l = crate::fork::context::awareness::tests::lock();
        clear();
        let dir = tempfile::tempdir().unwrap();
        touch(dir.path(), "a/b.ts");
        ensure(dir.path());
        let project = wait_for(dir.path(), Duration::from_secs(10)).expect("listed");
        assert!(project.names().contains(&"b.ts".to_string()));
        // Fresh: a second take start does not list again.
        ensure(dir.path());
        assert!(!cache().as_ref().unwrap()[dir.path()].listing);
        clear();
        assert!(names(dir.path()).is_none());
    }
}
