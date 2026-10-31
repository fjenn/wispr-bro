// SPDX-License-Identifier: GPL-3.0-or-later
//! Downloads a cleanup model from Hugging Face (S-10, DEC-34).
//!
//! Uses the same hf-hub crate and cache as Handy's model manager
//! (`HF_HOME/hub`, under the app data dir per DEC-39), so the file lands in
//! `models--<org>--<name>/snapshots/<revision>/<file>`. Handy's cache scan
//! ignores it: its GGUF probe only surfaces models transcribe-cpp can load,
//! and a Qwen/Muesli GGUF is not one (test `handy_does_not_list_the_llm`).
//!
//! Rules: the endpoint is fixed to `https://huggingface.co` (an `HF_ENDPOINT`
//! in the environment is ignored), no token is sent, the revision is the
//! pinned commit, and the file is sha256-checked before first use. A
//! verification marker (size + mtime + sha256, TypeWhisper's
//! `LlamaCppVerificationMarker` idea) avoids re-hashing 1.3 GB at every
//! launch while still noticing a changed file. A mismatch deletes the file.
//! A stalled transfer (no bytes for 30 s) is cancelled and retried with
//! backoff; hf-hub resumes from its `.sync.part`.

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant, SystemTime};

use hf_hub::api::tokio::{ApiBuilder, ApiError, CancellationToken, Progress};
use hf_hub::{Cache, Repo, RepoType};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use super::catalog::LlmModel;

/// The only download endpoint (R5 §1).
pub const HF_ENDPOINT: &str = "https://huggingface.co";
/// No bytes for this long cancels the attempt.
pub const STALL_TIMEOUT: Duration = Duration::from_secs(30);
/// Attempts before giving up (the user can retry from the AI engine panel).
pub const MAX_ATTEMPTS: u32 = 5;

/// The resolve URL of a pinned file (also what the allowlist test checks).
pub fn resolve_url(m: &LlmModel) -> String {
    format!("{HF_ENDPOINT}/{}/resolve/{}/{}", m.repo, m.revision, m.file)
}

/// Where hf-hub puts the file for `m` in the cache at `hub_dir`.
pub fn snapshot_path(hub_dir: &Path, m: &LlmModel) -> PathBuf {
    hub_dir
        .join(format!("models--{}", m.repo.replace('/', "--")))
        .join("snapshots")
        .join(m.revision)
        .join(m.file)
}

/// The hub cache dir hf-hub and Handy use (`$HF_HOME/hub`, else
/// `~/.cache/huggingface/hub`).
pub fn hub_dir() -> PathBuf {
    Cache::from_env().path().clone()
}

// ---------------------------------------------------------------------------
// Verification marker

#[derive(Debug, Serialize, Deserialize, PartialEq, Eq)]
struct Marker {
    sha256: String,
    size: u64,
    mtime_ns: u128,
}

fn file_stamp(path: &Path) -> Option<(u64, u128)> {
    // Follow the snapshot symlink to the blob.
    let meta = std::fs::metadata(path).ok()?;
    if !meta.is_file() {
        return None;
    }
    let mtime = meta
        .modified()
        .ok()?
        .duration_since(SystemTime::UNIX_EPOCH)
        .ok()?
        .as_nanos();
    Some((meta.len(), mtime))
}

fn marker_path(markers_dir: &Path, m: &LlmModel) -> PathBuf {
    markers_dir.join(format!("{}.verified.json", m.id))
}

/// `true` when the marker says `path` was hashed to `m.sha256` and the file
/// has not changed since (same size and mtime).
pub fn is_verified(markers_dir: &Path, path: &Path, m: &LlmModel) -> bool {
    let Some((size, mtime_ns)) = file_stamp(path) else {
        return false;
    };
    let Ok(raw) = std::fs::read(marker_path(markers_dir, m)) else {
        return false;
    };
    serde_json::from_slice::<Marker>(&raw).ok()
        == Some(Marker {
            sha256: m.sha256.to_string(),
            size,
            mtime_ns,
        })
        && size == m.size_bytes
}

fn write_marker(markers_dir: &Path, path: &Path, m: &LlmModel) -> std::io::Result<()> {
    let (size, mtime_ns) =
        file_stamp(path).ok_or_else(|| std::io::Error::other("model file vanished"))?;
    std::fs::create_dir_all(markers_dir)?;
    let marker = Marker {
        sha256: m.sha256.to_string(),
        size,
        mtime_ns,
    };
    let tmp = marker_path(markers_dir, m).with_extension("tmp");
    std::fs::write(&tmp, serde_json::to_vec(&marker)?)?;
    std::fs::rename(tmp, marker_path(markers_dir, m))
}

/// Lower-case hex sha256 of a file, reading in 4 MiB chunks; stops early
/// when `cancel` fires.
pub fn sha256_file(path: &Path, cancel: &CancellationToken) -> std::io::Result<String> {
    use std::io::Read;
    let mut f = std::fs::File::open(path)?;
    let mut hasher = Sha256::new();
    let mut buf = vec![0u8; 4 * 1024 * 1024];
    loop {
        if cancel.is_cancelled() {
            return Err(std::io::Error::new(
                std::io::ErrorKind::Interrupted,
                "cancelled",
            ));
        }
        let n = f.read(&mut buf)?;
        if n == 0 {
            break;
        }
        hasher.update(&buf[..n]);
    }
    Ok(hasher
        .finalize()
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect())
}

/// Result of checking a file on disk.
#[derive(Debug, PartialEq, Eq)]
pub enum Check {
    /// Present and verified (now or by an earlier marker).
    Ok,
    /// Not on disk.
    Missing,
    /// On disk with the wrong size or hash; it was deleted.
    Corrupt,
}

/// Verifies the cached file for `m` (hashing only when no valid marker
/// exists) and deletes it on mismatch.
pub fn check_cached(
    hub_dir: &Path,
    markers_dir: &Path,
    m: &LlmModel,
    cancel: &CancellationToken,
) -> std::io::Result<Check> {
    let path = snapshot_path(hub_dir, m);
    let Some((size, _)) = file_stamp(&path) else {
        return Ok(Check::Missing);
    };
    if is_verified(markers_dir, &path, m) {
        return Ok(Check::Ok);
    }
    if size == m.size_bytes && sha256_file(&path, cancel)? == m.sha256 {
        write_marker(markers_dir, &path, m)?;
        return Ok(Check::Ok);
    }
    remove_cached(hub_dir, m);
    let _ = std::fs::remove_file(marker_path(markers_dir, m));
    Ok(Check::Corrupt)
}

/// Deletes the snapshot entry and the blob it points to.
pub fn remove_cached(hub_dir: &Path, m: &LlmModel) {
    let path = snapshot_path(hub_dir, m);
    if let Ok(target) = std::fs::canonicalize(&path) {
        let _ = std::fs::remove_file(&target);
    }
    let _ = std::fs::remove_file(&path);
}

// ---------------------------------------------------------------------------
// Download

/// Progress shared with the status reporter.
#[derive(Debug, Default)]
pub struct ProgressCounter {
    pub done: AtomicU64,
    pub total: AtomicU64,
    last_activity_ms: AtomicU64,
}

impl ProgressCounter {
    fn touch(&self, epoch: Instant) {
        let ms = u64::try_from(epoch.elapsed().as_millis()).unwrap_or(u64::MAX);
        self.last_activity_ms.store(ms, Ordering::Relaxed);
    }

    fn idle_for(&self, epoch: Instant) -> Duration {
        let last = Duration::from_millis(self.last_activity_ms.load(Ordering::Relaxed));
        epoch.elapsed().saturating_sub(last)
    }
}

#[derive(Clone)]
struct HfProgress {
    counter: Arc<ProgressCounter>,
    epoch: Instant,
}

impl Progress for HfProgress {
    async fn init(&mut self, size: usize, _filename: &str) {
        self.counter.total.store(size as u64, Ordering::Relaxed);
        self.counter.done.store(0, Ordering::Relaxed);
        self.counter.touch(self.epoch);
    }
    async fn update(&mut self, size: usize) {
        self.counter.done.fetch_add(size as u64, Ordering::Relaxed);
        self.counter.touch(self.epoch);
    }
    async fn finish(&mut self) {
        self.counter.touch(self.epoch);
    }
}

/// Why a download ended without a verified file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DownloadError {
    Cancelled,
    /// Downloaded, but the hash or size is wrong (file deleted).
    Corrupt,
    /// Network or disk error after all attempts (message for logs).
    Failed(String),
}

impl std::fmt::Display for DownloadError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            DownloadError::Cancelled => write!(f, "cancelled"),
            DownloadError::Corrupt => write!(f, "checksum mismatch"),
            DownloadError::Failed(m) => write!(f, "{m}"),
        }
    }
}

/// Backoff before attempt `n` (1-based): 2, 4, 8, 16 s.
pub fn backoff(attempt: u32) -> Duration {
    Duration::from_secs(1u64 << attempt.clamp(1, 4))
}

/// Makes sure the verified file for `m` is on disk, downloading it if
/// needed. Returns its path.
pub async fn ensure(
    m: &'static LlmModel,
    hub_dir: PathBuf,
    markers_dir: PathBuf,
    counter: Arc<ProgressCounter>,
    cancel: CancellationToken,
) -> Result<PathBuf, DownloadError> {
    let check = {
        let (h, d, c) = (hub_dir.clone(), markers_dir.clone(), cancel.clone());
        tokio::task::spawn_blocking(move || check_cached(&h, &d, m, &c))
            .await
            .map_err(|e| DownloadError::Failed(e.to_string()))?
            .map_err(|e| io_error(e, &cancel))?
    };
    if check == Check::Ok {
        return Ok(snapshot_path(&hub_dir, m));
    }
    if check == Check::Corrupt {
        log::warn!(
            "llm: cached {} failed verification, downloading again",
            m.id
        );
    }
    if !crate::fork::net::is_allowed_host(&resolve_url(m)) {
        return Err(DownloadError::Failed("download host not allowed".into()));
    }

    let api = ApiBuilder::from_cache(Cache::new(hub_dir.clone()))
        .with_endpoint(HF_ENDPOINT.to_string())
        .with_token(None)
        .with_progress(false)
        .build()
        .map_err(|e| DownloadError::Failed(format!("hf-hub init: {e}")))?;
    let repo = api.repo(Repo::with_revision(
        m.repo.to_string(),
        RepoType::Model,
        m.revision.to_string(),
    ));

    let mut last_error = String::new();
    for attempt in 1..=MAX_ATTEMPTS {
        if cancel.is_cancelled() {
            return Err(DownloadError::Cancelled);
        }
        let epoch = Instant::now();
        counter.touch(epoch);
        let attempt_token = cancel.child_token();
        let progress = HfProgress {
            counter: counter.clone(),
            epoch,
        };
        let watchdog = tokio::spawn({
            let counter = counter.clone();
            let token = attempt_token.clone();
            async move {
                loop {
                    tokio::time::sleep(Duration::from_secs(1)).await;
                    if token.is_cancelled() {
                        break;
                    }
                    if counter.idle_for(epoch) > STALL_TIMEOUT {
                        token.cancel();
                        break;
                    }
                }
            }
        });
        let mut fut = std::pin::pin!(repo.download_with_progress_cancellable(
            m.file,
            progress,
            attempt_token.clone()
        ));
        let result = tokio::select! {
            r = &mut fut => r,
            _ = attempt_token.cancelled() => {
                // Let hf-hub commit its resume offset, then give up on it.
                match tokio::time::timeout(Duration::from_secs(5), &mut fut).await {
                    Ok(r) => r,
                    Err(_) => Err(ApiError::Cancelled),
                }
            }
        };
        watchdog.abort();
        match result {
            Ok(_) => {
                let (h, d, c) = (hub_dir.clone(), markers_dir.clone(), cancel.clone());
                let check = tokio::task::spawn_blocking(move || check_cached(&h, &d, m, &c))
                    .await
                    .map_err(|e| DownloadError::Failed(e.to_string()))?
                    .map_err(|e| io_error(e, &cancel))?;
                return match check {
                    Check::Ok => Ok(snapshot_path(&hub_dir, m)),
                    Check::Corrupt => Err(DownloadError::Corrupt),
                    Check::Missing => Err(DownloadError::Failed(
                        "download finished but the file is missing".into(),
                    )),
                };
            }
            Err(ApiError::Cancelled) if cancel.is_cancelled() => {
                return Err(DownloadError::Cancelled);
            }
            Err(e) => {
                // `{e}` of hf-hub/reqwest errors carries no user data.
                last_error = if matches!(e, ApiError::Cancelled) {
                    format!("stalled for {} s", STALL_TIMEOUT.as_secs())
                } else {
                    e.to_string()
                };
                log::warn!(
                    "llm: download of {} attempt {attempt}/{MAX_ATTEMPTS} failed: {last_error}",
                    m.id
                );
            }
        }
        if attempt < MAX_ATTEMPTS {
            tokio::select! {
                _ = tokio::time::sleep(backoff(attempt)) => {}
                _ = cancel.cancelled() => return Err(DownloadError::Cancelled),
            }
        }
    }
    Err(DownloadError::Failed(last_error))
}

fn io_error(e: std::io::Error, cancel: &CancellationToken) -> DownloadError {
    if cancel.is_cancelled() {
        DownloadError::Cancelled
    } else {
        DownloadError::Failed(e.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fork::llm::catalog::MUESLI_2B_V12;

    const TINY: LlmModel = LlmModel {
        id: "tiny",
        display_name: "Tiny",
        repo: "org/tiny-gguf",
        revision: "0123456789abcdef0123456789abcdef01234567",
        file: "tiny.gguf",
        // sha256("GGUFabcd")
        sha256: "",
        size_bytes: 8,
        license: "Apache-2.0",
        resident_mib: 1,
        prompt_style: crate::fork::llm::catalog::PromptStyle::ChatJsonSchema,
    };

    fn tiny_with_hash() -> LlmModel {
        let mut h = Sha256::new();
        h.update(b"GGUFabcd");
        let hex: String = h.finalize().iter().map(|b| format!("{b:02x}")).collect();
        LlmModel {
            sha256: Box::leak(hex.into_boxed_str()),
            ..TINY
        }
    }

    fn place(hub: &Path, m: &LlmModel, bytes: &[u8]) -> PathBuf {
        let p = snapshot_path(hub, m);
        std::fs::create_dir_all(p.parent().unwrap()).unwrap();
        std::fs::write(&p, bytes).unwrap();
        p
    }

    #[test]
    fn snapshot_path_is_hf_hubs_layout() {
        assert_eq!(
            snapshot_path(Path::new("/h/hub"), &MUESLI_2B_V12),
            PathBuf::from(
                "/h/hub/models--Muesli-HQ--qwen35-2b-muesli-cleanup-v12-gguf/snapshots/e35af097be4f17d3673515fb321ce19aa300faf6/qwen35-2b-muesli-v12-Q4_K_M.gguf"
            )
        );
        assert_eq!(
            resolve_url(&MUESLI_2B_V12),
            format!(
                "{HF_ENDPOINT}/Muesli-HQ/qwen35-2b-muesli-cleanup-v12-gguf/resolve/e35af097be4f17d3673515fb321ce19aa300faf6/qwen35-2b-muesli-v12-Q4_K_M.gguf"
            )
        );
    }

    #[test]
    fn verification_marker_notices_changed_files() {
        let dir = tempfile::tempdir().unwrap();
        let (hub, markers) = (dir.path().join("hub"), dir.path().join("markers"));
        let m = tiny_with_hash();
        let cancel = CancellationToken::new();
        assert_eq!(
            check_cached(&hub, &markers, &m, &cancel).unwrap(),
            Check::Missing
        );
        let p = place(&hub, &m, b"GGUFabcd");
        assert!(!is_verified(&markers, &p, &m));
        assert_eq!(
            check_cached(&hub, &markers, &m, &cancel).unwrap(),
            Check::Ok
        );
        assert!(is_verified(&markers, &p, &m));
        // A marker for another hash does not count.
        let other = LlmModel { sha256: "ab", ..m };
        assert!(!is_verified(&markers, &p, &other));
        // Same size, different bytes, new mtime: not verified any more, and
        // the full check deletes it.
        std::thread::sleep(Duration::from_millis(20));
        std::fs::write(&p, b"GGUFabce").unwrap();
        assert!(!is_verified(&markers, &p, &m));
        assert_eq!(
            check_cached(&hub, &markers, &m, &cancel).unwrap(),
            Check::Corrupt
        );
        assert!(!p.exists());
    }

    #[test]
    fn wrong_size_is_corrupt_without_hashing() {
        let dir = tempfile::tempdir().unwrap();
        let (hub, markers) = (dir.path().join("hub"), dir.path().join("markers"));
        let m = tiny_with_hash();
        let p = place(&hub, &m, b"GGUF");
        let cancel = CancellationToken::new();
        cancel.cancel(); // hashing would fail; size check must come first
        assert_eq!(
            check_cached(&hub, &markers, &m, &cancel).unwrap(),
            Check::Corrupt
        );
        assert!(!p.exists());
    }

    #[test]
    fn hashing_stops_when_cancelled() {
        let dir = tempfile::tempdir().unwrap();
        let p = dir.path().join("f");
        std::fs::write(&p, b"x").unwrap();
        let c = CancellationToken::new();
        c.cancel();
        assert_eq!(
            sha256_file(&p, &c).unwrap_err().kind(),
            std::io::ErrorKind::Interrupted
        );
    }

    #[test]
    fn backoff_grows_and_caps() {
        assert_eq!(backoff(1), Duration::from_secs(2));
        assert_eq!(backoff(2), Duration::from_secs(4));
        assert_eq!(backoff(4), Duration::from_secs(16));
        assert_eq!(backoff(9), Duration::from_secs(16));
    }

    #[test]
    fn hf_endpoint_env_is_ignored() {
        let src = include_str!("download.rs");
        let code = src.split("#[cfg(test)]").next().unwrap();
        assert!(code.contains(".with_endpoint(HF_ENDPOINT.to_string())"));
        assert!(code.contains(".with_token(None)"));
        assert!(!code.contains("ApiBuilder::from_env"));
    }

    /// Real download of the default model into a temp HF cache (1.27 GB).
    /// `cargo test -- --ignored real_download` with network.
    #[tokio::test]
    #[ignore = "network: downloads 1.27 GB from Hugging Face"]
    async fn real_download_of_the_default_model() {
        let dir = tempfile::tempdir().unwrap();
        let counter = Arc::new(ProgressCounter::default());
        let t0 = Instant::now();
        let path = ensure(
            &MUESLI_2B_V12,
            dir.path().join("hub"),
            dir.path().join("markers"),
            counter.clone(),
            CancellationToken::new(),
        )
        .await
        .unwrap();
        eprintln!(
            "downloaded {} bytes in {:?} to {}",
            counter.done.load(Ordering::Relaxed),
            t0.elapsed(),
            path.display()
        );
        assert!(is_verified(
            &dir.path().join("markers"),
            &path,
            &MUESLI_2B_V12
        ));
    }
}
