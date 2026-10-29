// SPDX-License-Identifier: GPL-3.0-or-later
//! Panic hook (F-08, R5 §5.7, W-24).
//!
//! [`install`] (called by `fork::early_init`) chains a hook in front of the
//! previous one: it writes `crash-<timestamp>.txt` to the app's log folder,
//! sanitized by [`super::sanitizer`], then calls the previous hook so stderr
//! output and the process behaviour stay Handy's. Nothing is uploaded. S-27
//! adds the "quit unexpectedly" inbox entry that reads these files.

use std::fmt::Write as _;
use std::panic::PanicHookInfo;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

/// Crash files kept in the log folder (oldest removed first).
pub const MAX_CRASH_FILES: usize = 10;
/// Prefix of crash files.
pub const CRASH_FILE_PREFIX: &str = "crash-";

static CRASH_DIR: Mutex<Option<PathBuf>> = Mutex::new(None);

/// Sets (or replaces) the folder crash files go to. `early_init` sets the
/// macOS path computed before Tauri exists; the plugin `setup` replaces it
/// with Tauri's resolved (portable-aware) log dir.
pub fn set_crash_dir(dir: PathBuf) {
    if let Ok(mut slot) = CRASH_DIR.lock() {
        *slot = Some(dir);
    }
}

/// The configured crash folder.
pub fn crash_dir() -> Option<PathBuf> {
    CRASH_DIR.lock().ok().and_then(|d| d.clone())
}

type Hook = Box<dyn Fn(&PanicHookInfo<'_>) + Sync + Send + 'static>;

/// Installs the crash hook, chained to the current hook.
pub fn install() {
    let previous = std::panic::take_hook();
    std::panic::set_hook(chained_hook(crash_dir, previous));
}

/// A hook that writes a crash file into `dir()` (if any) and then calls
/// `previous`. Never panics itself: every failure is ignored, because a panic
/// inside a panic hook aborts the process.
pub fn chained_hook(dir: fn() -> Option<PathBuf>, previous: Hook) -> Hook {
    Box::new(move |info| {
        if let Some(dir) = dir() {
            let message = panic_message(info);
            let location = info
                .location()
                .map(|l| format!("{}:{}:{}", l.file(), l.line(), l.column()))
                .unwrap_or_else(|| "unknown".into());
            let backtrace = std::backtrace::Backtrace::force_capture().to_string();
            let _ = write_crash_report(&dir, &message, &location, &backtrace);
        }
        previous(info);
    })
}

fn panic_message(info: &PanicHookInfo<'_>) -> String {
    if let Some(s) = info.payload().downcast_ref::<&str>() {
        (*s).to_string()
    } else if let Some(s) = info.payload().downcast_ref::<String>() {
        s.clone()
    } else {
        "non-string panic payload".into()
    }
}

/// Text of a crash file, before sanitizing.
pub fn crash_report_text(message: &str, location: &str, backtrace: &str) -> String {
    let thread = std::thread::current();
    let mut out = String::new();
    let _ = writeln!(out, "Wispr Bro crash report (local only, never uploaded)");
    let _ = writeln!(out, "version: {}", env!("CARGO_PKG_VERSION"));
    let _ = writeln!(
        out,
        "build: {}",
        if cfg!(debug_assertions) {
            "debug"
        } else {
            "release"
        }
    );
    let _ = writeln!(
        out,
        "os: {} {}",
        std::env::consts::OS,
        std::env::consts::ARCH
    );
    let _ = writeln!(out, "time: {}", chrono::Local::now().to_rfc3339());
    let _ = writeln!(out, "thread: {}", thread.name().unwrap_or("<unnamed>"));
    let _ = writeln!(out, "location: {location}");
    let _ = writeln!(out, "message: {message}");
    let _ = writeln!(out, "backtrace:\n{backtrace}");
    out
}

/// Writes a sanitized crash file into `dir` and prunes old ones. Returns the
/// file path.
pub fn write_crash_report(
    dir: &Path,
    message: &str,
    location: &str,
    backtrace: &str,
) -> std::io::Result<PathBuf> {
    std::fs::create_dir_all(dir)?;
    let text =
        super::sanitizer::sanitize_log_content(&crash_report_text(message, location, backtrace));
    let stamp = chrono::Local::now().format("%Y%m%d-%H%M%S%.3f");
    let path = dir.join(format!(
        "{CRASH_FILE_PREFIX}{stamp}-{}.txt",
        std::process::id()
    ));
    std::fs::write(&path, text)?;
    prune(dir);
    Ok(path)
}

fn prune(dir: &Path) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    let mut files: Vec<PathBuf> = entries
        .filter_map(|e| e.ok().map(|e| e.path()))
        .filter(|p| {
            p.file_name()
                .and_then(|n| n.to_str())
                .is_some_and(|n| n.starts_with(CRASH_FILE_PREFIX) && n.ends_with(".txt"))
        })
        .collect();
    if files.len() <= MAX_CRASH_FILES {
        return;
    }
    files.sort();
    for old in &files[..files.len() - MAX_CRASH_FILES] {
        let _ = std::fs::remove_file(old);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::sync::Arc;

    static TEST_DIR: Mutex<Option<PathBuf>> = Mutex::new(None);
    fn test_dir() -> Option<PathBuf> {
        // Only the test's own thread writes into the temp dir, so other
        // tests that panic concurrently are unaffected.
        (std::thread::current().name() == Some("wb-crash-test"))
            .then(|| TEST_DIR.lock().unwrap().clone())
            .flatten()
    }

    #[test]
    fn panic_hook_writes_a_sanitized_file_and_chains() {
        let tmp = tempfile::tempdir().unwrap();
        *TEST_DIR.lock().unwrap() = Some(tmp.path().to_path_buf());
        let home = std::env::var("HOME").unwrap_or_else(|_| "/Users/nobody".into());

        let chained = Arc::new(AtomicBool::new(false));
        let chained_flag = chained.clone();
        let original = std::panic::take_hook();
        let original = Arc::new(original);
        let original_for_prev = original.clone();
        let previous: Hook = Box::new(move |info| {
            if std::thread::current().name() == Some("wb-crash-test") {
                chained_flag.store(true, Ordering::SeqCst);
            } else {
                (*original_for_prev)(info);
            }
        });
        std::panic::set_hook(chained_hook(test_dir, previous));

        let secret = format!(
            "transcript: my secret diagnosis\nfile {home}/Documents/x.txt\nmail a.b@example.com"
        );
        let joined = std::thread::Builder::new()
            .name("wb-crash-test".into())
            .spawn(move || panic!("{secret}"))
            .unwrap()
            .join();
        assert!(joined.is_err());

        // Restore a plain default hook for the rest of the test run.
        let _ = std::panic::take_hook();
        assert!(chained.load(Ordering::SeqCst), "previous hook not called");

        let files: Vec<_> = std::fs::read_dir(tmp.path())
            .unwrap()
            .map(|e| e.unwrap().path())
            .collect();
        assert_eq!(files.len(), 1, "{files:?}");
        let name = files[0].file_name().unwrap().to_str().unwrap().to_string();
        assert!(
            name.starts_with("crash-") && name.ends_with(".txt"),
            "{name}"
        );
        let text = std::fs::read_to_string(&files[0]).unwrap();
        assert!(text.contains("Wispr Bro crash report"), "{text}");
        assert!(text.contains("thread: wb-crash-test"));
        assert!(text.contains("location: "));
        assert!(!text.contains("secret diagnosis"), "{text}");
        assert!(!text.contains("a.b@example.com"), "{text}");
        assert!(!text.contains(&format!("{home}/Documents")), "{text}");
        assert!(text.contains("~/Documents/x.txt"), "{text}");
        drop(original);
    }

    #[test]
    fn old_crash_files_are_pruned() {
        let tmp = tempfile::tempdir().unwrap();
        for i in 0..(MAX_CRASH_FILES + 3) {
            std::fs::write(tmp.path().join(format!("crash-2020010{i:02}.txt")), "x").unwrap();
        }
        std::fs::write(tmp.path().join("wispr-bro.log"), "log").unwrap();
        write_crash_report(tmp.path(), "m", "l", "b").unwrap();
        let count = std::fs::read_dir(tmp.path())
            .unwrap()
            .filter(|e| {
                let n = e.as_ref().unwrap().file_name();
                n.to_string_lossy().starts_with("crash-")
            })
            .count();
        assert_eq!(count, MAX_CRASH_FILES);
        assert!(tmp.path().join("wispr-bro.log").exists());
    }
}
