// SPDX-License-Identifier: GPL-3.0-or-later
//! Insights commands (S-25): the Usage and Voice tabs, and "Save as image".
//! All computed on this Mac from `fork.db`; nothing leaves it.

use std::path::Path;

use base64::Engine as _;
use tauri::{AppHandle, Manager};

use super::ForkError;
use crate::fork::db::{insights, DbError, ForkDb};
use crate::fork::settings::ForkSettingsHandle;
use crate::fork::stats::insights::UsageInsights;
use crate::fork::stats::voice::{self, VoiceProfile};
use crate::fork::ForkRuntime;

/// Largest image "Save as image" writes (a 2× render of the page is a few
/// hundred KB).
pub const MAX_IMAGE_BYTES: usize = 25 * 1024 * 1024;

const PNG_MAGIC: &[u8] = b"\x89PNG\r\n\x1a\n";

fn db<R: tauri::Runtime>(app: &AppHandle<R>) -> Result<tauri::State<'_, ForkDb>, ForkError> {
    app.try_state::<ForkDb>().ok_or(ForkError::NotReady)
}

fn internal(e: DbError) -> ForkError {
    ForkError::Internal(e.to_string())
}

/// The typing test result (S-29b), if the user has one.
fn typing_wpm<R: tauri::Runtime>(app: &AppHandle<R>) -> Option<u32> {
    app.try_state::<ForkSettingsHandle>()?
        .get(app)
        .ok()?
        .typing_wpm
}

/// The Usage tab: speed, fixes, totals, categories and the streak heatmap.
#[tauri::command]
#[specta::specta]
pub fn fork_insights_usage<R: ForkRuntime>(app: AppHandle<R>) -> Result<UsageInsights, ForkError> {
    let today = chrono::Local::now().date_naive();
    let (totals, days, cats) = db(&app)?
        .with_conn(|c| {
            Ok((
                insights::totals(c)?,
                insights::days(c)?,
                insights::categories(c)?,
            ))
        })
        .map_err(internal)?;
    Ok(crate::fork::stats::insights::usage(
        &totals,
        &days,
        &cats,
        typing_wpm(&app),
        today,
    ))
}

/// The Voice tab: locked below 1,700 words, else the four cards. The rows
/// are read under the database lock; the cards are computed off it.
#[tauri::command]
#[specta::specta]
pub async fn fork_insights_voice<R: ForkRuntime>(
    app: AppHandle<R>,
) -> Result<VoiceProfile, ForkError> {
    let (total, entries, logged) = db(&app)?
        .with_conn(|c| {
            let total = insights::totals(c)?.words;
            if total < voice::VOICE_PROFILE_WORDS {
                return Ok((total, Vec::new(), Vec::new()));
            }
            Ok((
                total,
                insights::voice_entries(c)?,
                insights::logged_corrections(c)?,
            ))
        })
        .map_err(internal)?;
    tauri::async_runtime::spawn_blocking(move || {
        let started = std::time::Instant::now();
        let profile = voice::profile(total, &entries, &logged, &chrono::Local);
        log::debug!(
            "insights: voice profile from {} dictation(s) in {} ms",
            profile.dictations_read,
            started.elapsed().as_millis()
        );
        profile
    })
    .await
    .map_err(|e| ForkError::Internal(e.to_string()))
}

/// Why an image was not written.
fn check_image(path: &Path, png: &[u8]) -> Result<(), String> {
    let png_ext = path
        .extension()
        .and_then(|e| e.to_str())
        .is_some_and(|e| e.eq_ignore_ascii_case("png"));
    if !png_ext {
        return Err("the file name must end in .png".into());
    }
    if png.len() > MAX_IMAGE_BYTES {
        return Err(format!("image too large ({} bytes)", png.len()));
    }
    if !png.starts_with(PNG_MAGIC) {
        return Err("not a PNG image".into());
    }
    Ok(())
}

/// Writes the PNG the page rendered ("Save as image", C-56) to `path`,
/// chosen by the user in the save panel. `png_base64` is the image without
/// a `data:` prefix. Returns the bytes written.
pub fn save_image(path: &Path, png_base64: &str) -> Result<u32, String> {
    let png = base64::engine::general_purpose::STANDARD
        .decode(png_base64.trim())
        .map_err(|e| format!("bad image data: {e}"))?;
    check_image(path, &png)?;
    std::fs::write(path, &png).map_err(|e| e.to_string())?;
    Ok(u32::try_from(png.len()).unwrap_or(u32::MAX))
}

/// "Save as image" (C-56): writes the rendered Insights to `path`.
#[tauri::command]
#[specta::specta]
pub fn fork_insights_save_image(path: String, png_base64: String) -> Result<u32, ForkError> {
    let n = save_image(Path::new(&path), &png_base64).map_err(|e| {
        log::warn!("insights: save as image failed: {e}");
        ForkError::Internal(e)
    })?;
    log::info!("insights: saved an image ({n} bytes)");
    Ok(n)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A 1×1 PNG.
    const PIXEL: &str = "iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAQAAAC1HAwCAAAAC0lEQVR42mNkYAAAAAYAAjCB0C8AAAAASUVORK5CYII=";

    #[test]
    fn writes_a_png_only() {
        let dir = tempfile::tempdir().unwrap();
        let ok = dir.path().join("insights.PNG");
        assert_eq!(save_image(&ok, PIXEL).unwrap(), 68);
        assert!(std::fs::read(&ok).unwrap().starts_with(PNG_MAGIC));

        let txt = dir.path().join("insights.txt");
        assert!(save_image(&txt, PIXEL).unwrap_err().contains(".png"));
        assert!(!txt.exists());

        let not_png = base64::engine::general_purpose::STANDARD.encode(b"GIF89a....");
        let bad = dir.path().join("bad.png");
        assert!(save_image(&bad, &not_png).unwrap_err().contains("PNG"));
        assert!(save_image(&bad, "%%%").unwrap_err().contains("bad image"));
        assert!(!bad.exists());
    }

    #[test]
    fn refuses_huge_images() {
        let mut big = PNG_MAGIC.to_vec();
        big.resize(MAX_IMAGE_BYTES + 1, 0);
        assert!(check_image(Path::new("a.png"), &big)
            .unwrap_err()
            .contains("too large"));
    }
}
