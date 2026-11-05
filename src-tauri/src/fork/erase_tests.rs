// SPDX-License-Identifier: GPL-3.0-or-later
//! Tests for `fork::erase` (a separate file: they lay out a fake app data dir
//! with Handy's database file names on purpose).

use super::*;
use std::sync::{Arc, Mutex};

fn file(path: &Path, text: &str) {
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(path, text).unwrap();
}

fn names(path: &Path) -> Vec<String> {
    let mut names: Vec<String> = std::fs::read_dir(path)
        .unwrap()
        .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
        .collect();
    names.sort();
    names
}

#[test]
fn typed_confirmation_is_exact() {
    assert!(valid_confirmation("DELETE"));
    for wrong in ["", "delete", " DELETE", "DELETE ", "REMOVE"] {
        assert!(!valid_confirmation(wrong));
    }
}

#[test]
fn erase_leaves_nothing_but_kept_models_in_a_portable_data_dir() {
    let temp = tempfile::tempdir().unwrap();
    let data = temp.path().join("portable/Data");
    let logs = temp.path().join("app-logs");
    for name in [
        "history.db",
        "history.db-wal",
        "history.db-shm",
        "fork.db",
        "fork.db-wal",
        "fork.db-shm",
        "settings_store.json",
        "fork-settings.json",
        "recordings/private.wav",
        "logs/wispr-bro.log",
        "models/speech.gguf",
        "huggingface/hub/model.gguf",
        "llm/verified/qwen.verified.json",
        "llm/llm.sock",
        "WebKit/cache",
    ] {
        file(&data.join(name), "data");
    }
    file(&logs.join("crash.txt"), "crash");
    let outside = temp.path().join("outside.txt");
    file(&outside, "keep");

    erase_files(&data, &logs, false).unwrap();
    assert_eq!(names(&data), ["huggingface", "llm", "models"]);
    assert_eq!(names(&data.join("models")), ["speech.gguf"]);
    assert_eq!(names(&data.join("llm")), ["verified"]);
    assert!(!logs.exists());
    assert_eq!(std::fs::read_to_string(outside).unwrap(), "keep");

    erase_files(&data, &logs, true).unwrap();
    assert_eq!(names(&data), Vec::<String>::new());
}

#[test]
fn erase_unlinks_a_symlink_without_touching_its_target() {
    #[cfg(unix)]
    {
        let temp = tempfile::tempdir().unwrap();
        let data = temp.path().join("Data");
        std::fs::create_dir(&data).unwrap();
        let target = temp.path().join("personal.txt");
        file(&target, "personal");
        std::os::unix::fs::symlink(&target, data.join("link")).unwrap();
        erase_files(&data, &data.join("logs"), true).unwrap();
        assert_eq!(std::fs::read_to_string(target).unwrap(), "personal");
    }
}

#[test]
fn phases_stop_then_unload_then_erase_then_relaunch() {
    let phases = Arc::new(Mutex::new(Vec::new()));
    let phase = |name: &'static str, phases: &Arc<Mutex<Vec<&'static str>>>| {
        let phases = phases.clone();
        move || {
            phases.lock().unwrap().push(name);
            Ok::<_, ()>(())
        }
    };
    let relaunch = {
        let phases = phases.clone();
        move || phases.lock().unwrap().push("relaunch")
    };
    erase_in_order(
        phase("stop", &phases),
        phase("unload", &phases),
        phase("erase", &phases),
        relaunch,
    )
    .unwrap();
    assert_eq!(
        *phases.lock().unwrap(),
        ["stop", "unload", "erase", "relaunch"]
    );
}

#[test]
fn a_failed_phase_never_erases_or_relaunches() {
    let called = Arc::new(Mutex::new(Vec::new()));
    let later = {
        let called = called.clone();
        move || {
            called.lock().unwrap().push("erase");
            Ok::<_, &'static str>(())
        }
    };
    let relaunch = {
        let called = called.clone();
        move || called.lock().unwrap().push("relaunch")
    };
    assert!(erase_in_order(
        || -> Result<(), &'static str> { Err("stop failed") },
        || Ok(()),
        later,
        relaunch,
    )
    .is_err());
    assert!(called.lock().unwrap().is_empty());
}
