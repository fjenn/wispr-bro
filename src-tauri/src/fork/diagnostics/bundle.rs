// SPDX-License-Identifier: GPL-3.0-or-later
//! Copyable and zipped diagnostics (S-27). Every log and crash report passes
//! through the F-08 sanitizer; databases, settings, History and recordings are
//! never read. Nothing is uploaded.

use std::io::{Read, Seek, Write};
use std::path::{Path, PathBuf};

use zip::write::SimpleFileOptions;

const MAX_FILES: usize = 20;
/// Tail of each file in the saved zip.
const MAX_BYTES_PER_FILE: u64 = 2 * 1024 * 1024;
/// Copy diagnostics goes to the clipboard: the newest few files, short tails.
const COPY_MAX_FILES: usize = 4;
const COPY_BYTES_PER_FILE: u64 = 48 * 1024;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BundleReport {
    pub files: u32,
    pub bytes: u64,
}

fn diagnostic_files(log_dir: &Path) -> Vec<PathBuf> {
    let mut files: Vec<PathBuf> = std::fs::read_dir(log_dir)
        .map(|entries| {
            entries
                .filter_map(|e| e.ok())
                .filter(|e| {
                    e.file_type().is_ok_and(|t| t.is_file() && !t.is_symlink())
                        && e.path()
                            .extension()
                            .and_then(|x| x.to_str())
                            .is_some_and(|x| {
                                matches!(x.to_ascii_lowercase().as_str(), "log" | "txt" | "ips")
                            })
                })
                .map(|e| e.path())
                .collect()
        })
        .unwrap_or_default();
    // Newest first, so the current log and the latest crash always fit.
    let modified = |p: &PathBuf| {
        std::fs::metadata(p)
            .and_then(|m| m.modified())
            .unwrap_or(std::time::UNIX_EPOCH)
    };
    files.sort_by(|a, b| modified(b).cmp(&modified(a)).then_with(|| a.cmp(b)));
    files.truncate(MAX_FILES);
    files
}

fn sanitized_tail(path: &Path, limit: u64) -> Result<String, String> {
    let mut file = std::fs::File::open(path).map_err(|e| e.to_string())?;
    let length = file.metadata().map_err(|e| e.to_string())?.len();
    if length > limit {
        file.seek(std::io::SeekFrom::Start(length - limit))
            .map_err(|e| e.to_string())?;
    }
    let mut bytes = Vec::new();
    file.read_to_end(&mut bytes).map_err(|e| e.to_string())?;
    Ok(super::sanitizer::sanitize_log_content(
        &String::from_utf8_lossy(&bytes),
    ))
}

/// Plain text for Copy diagnostics: the newest files, short tails.
pub fn copy_text(log_dir: &Path, app_version: &str) -> String {
    report(log_dir, app_version, COPY_MAX_FILES, COPY_BYTES_PER_FILE)
}

/// Plain text embedded in the saved zip as `diagnostics.txt`.
pub fn snapshot(log_dir: &Path, app_version: &str) -> String {
    report(log_dir, app_version, MAX_FILES, MAX_BYTES_PER_FILE)
}

fn report(log_dir: &Path, app_version: &str, max_files: usize, limit: u64) -> String {
    let mut out = format!(
        "Wispr Bro diagnostics\nversion: {app_version}\nos: {} {}\nlocal_only: true\n",
        std::env::consts::OS,
        std::env::consts::ARCH
    );
    for path in diagnostic_files(log_dir).into_iter().take(max_files) {
        let Some(name) = path.file_name().and_then(|n| n.to_str()) else {
            continue;
        };
        match sanitized_tail(&path, limit) {
            Ok(text) => {
                out.push_str(&format!("\n--- {name} ---\n{text}\n"));
            }
            Err(error) => {
                out.push_str(&format!("\n--- {name} ---\n[unreadable: {error}]\n"));
            }
        }
    }
    out
}

/// Saves a sanitized diagnostics zip to the user-selected path.
pub fn save_bundle(
    log_dir: &Path,
    app_version: &str,
    output: &Path,
) -> Result<BundleReport, String> {
    if !output.is_absolute()
        || !output
            .extension()
            .and_then(|e| e.to_str())
            .is_some_and(|e| e.eq_ignore_ascii_case("zip"))
    {
        return Err("choose an absolute diagnostics file name ending in .zip".into());
    }
    let file = std::fs::File::create(output).map_err(|e| e.to_string())?;
    let mut zip = zip::ZipWriter::new(file);
    let options = SimpleFileOptions::default().compression_method(zip::CompressionMethod::Deflated);
    zip.start_file("diagnostics.txt", options)
        .map_err(|e| e.to_string())?;
    zip.write_all(snapshot(log_dir, app_version).as_bytes())
        .map_err(|e| e.to_string())?;
    let mut files = 1;
    for path in diagnostic_files(log_dir) {
        let Some(name) = path.file_name().and_then(|n| n.to_str()) else {
            continue;
        };
        let text = sanitized_tail(&path, MAX_BYTES_PER_FILE)?;
        zip.start_file(format!("logs/{name}"), options)
            .map_err(|e| e.to_string())?;
        zip.write_all(text.as_bytes()).map_err(|e| e.to_string())?;
        files += 1;
    }
    let file = zip.finish().map_err(|e| e.to_string())?;
    let bytes = file.metadata().map_err(|e| e.to_string())?.len();
    Ok(BundleReport { files, bytes })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn copy_and_zip_are_sanitized_and_contain_no_data_files() {
        let temp = tempfile::tempdir().unwrap();
        let logs = temp.path().join("logs");
        std::fs::create_dir(&logs).unwrap();
        let home = std::env::var("HOME").unwrap_or_else(|_| "/Users/test".into());
        std::fs::write(
            logs.join("wispr-bro.log"),
            format!("Transcript: private diagnosis\npath {home}/Documents/private.txt\n"),
        )
        .unwrap();
        std::fs::write(logs.join("crash-1.txt"), "Prompt: my secret prompt").unwrap();
        std::fs::write(logs.join("stray.db"), "private database").unwrap();

        let text = snapshot(&logs, "1.2.3");
        assert!(text.contains("version: 1.2.3"));
        assert!(!text.contains("private diagnosis"), "{text}");
        assert!(!text.contains("secret prompt"), "{text}");
        assert!(!text.contains(&format!("{home}/Documents")), "{text}");
        assert!(!text.contains("private database"));

        let output = temp.path().join("diagnostics.zip");
        let report = save_bundle(&logs, "1.2.3", &output).unwrap();
        assert_eq!(report.files, 3);
        let mut archive = zip::ZipArchive::new(std::fs::File::open(output).unwrap()).unwrap();
        let mut names: Vec<String> = (0..archive.len())
            .map(|i| archive.by_index(i).unwrap().name().to_owned())
            .collect();
        names.sort();
        assert_eq!(
            names,
            ["diagnostics.txt", "logs/crash-1.txt", "logs/wispr-bro.log"]
        );
        for name in names {
            let mut text = String::new();
            archive
                .by_name(&name)
                .unwrap()
                .read_to_string(&mut text)
                .unwrap();
            assert!(!text.contains("private diagnosis"));
            assert!(!text.contains("secret prompt"));
        }
    }

    #[test]
    fn copy_keeps_the_newest_tail_short_enough_for_the_clipboard() {
        let temp = tempfile::tempdir().unwrap();
        let big = "x".repeat(200 * 1024) + "\nthe last line";
        std::fs::write(temp.path().join("wispr-bro.log"), &big).unwrap();
        let text = copy_text(temp.path(), "1");
        assert!(text.contains("the last line"));
        assert!(
            text.len() < (COPY_BYTES_PER_FILE as usize) + 1024,
            "{}",
            text.len()
        );
        assert!(snapshot(temp.path(), "1").len() > 200 * 1024);
    }

    #[test]
    fn diagnostics_requires_a_zip_path() {
        let temp = tempfile::tempdir().unwrap();
        assert!(save_bundle(temp.path(), "1", &temp.path().join("x.txt")).is_err());
    }
}
