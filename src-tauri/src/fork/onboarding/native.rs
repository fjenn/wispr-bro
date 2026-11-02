// SPDX-License-Identifier: GPL-3.0-or-later
//! Move to Applications (S-29a). Only on the user's press; never overwrites
//! another app. The path decisions are unit-tested; the move itself runs in
//! the Mac wave (MAC-TEST-SCRIPT S-29a).
//!
//! The idea follows LetsMove (move, else copy, then relaunch the copy);
//! nothing is ported. A plain `rename` works inside one volume; a copy is
//! needed from a disk image, another volume or a translocated (read-only)
//! launch, where `ditto` keeps the bundle's signature, symlinks and
//! extended attributes. The copy's quarantine flag is then cleared: this
//! app has already passed Gatekeeper to be running, and a flagged copy would
//! be translocated again on its next launch.

use std::path::{Path, PathBuf};

use crate::fork::commands::onboarding::ForkOnboardingLocation;

fn bundle_of_executable(exe: &Path) -> Option<PathBuf> {
    exe.ancestors()
        .skip(1)
        .find(|path| path.extension().is_some_and(|extension| extension == "app"))
        .map(Path::to_owned)
}

fn in_applications(bundle: &Path, home: Option<&Path>) -> bool {
    bundle.starts_with("/Applications")
        || home.is_some_and(|home| bundle.starts_with(home.join("Applications")))
}

fn location_of(bundle: Option<PathBuf>, home: Option<&Path>) -> ForkOnboardingLocation {
    match bundle {
        None => ForkOnboardingLocation::NotBundled,
        Some(bundle) if in_applications(&bundle, home) => {
            ForkOnboardingLocation::AlreadyInApplications
        }
        Some(_) => ForkOnboardingLocation::MoveNeeded,
    }
}

fn home_dir() -> Option<PathBuf> {
    std::env::var_os("HOME").map(PathBuf::from)
}

pub fn app_location() -> Result<ForkOnboardingLocation, String> {
    let exe = std::env::current_exe().map_err(|e| format!("cannot locate the app: {e}"))?;
    Ok(location_of(
        bundle_of_executable(&exe),
        home_dir().as_deref(),
    ))
}

/// How a bundle gets to its destination.
#[derive(Debug, PartialEq, Eq)]
enum Transfer {
    Renamed,
    /// `rename` cannot do it (another volume, a read-only image or a
    /// translocated launch): copy instead.
    CopyNeeded,
    /// No write access to the destination folder: try `~/Applications`.
    Denied,
    Failed(String),
}

fn classify(result: std::io::Result<()>) -> Transfer {
    use std::io::ErrorKind;
    match result {
        Ok(()) => Transfer::Renamed,
        Err(e) if e.kind() == ErrorKind::PermissionDenied => Transfer::Denied,
        // EXDEV (cross-device link), EROFS (read-only file system).
        Err(e) if matches!(e.raw_os_error(), Some(18) | Some(30)) => Transfer::CopyNeeded,
        Err(e) if e.kind() == ErrorKind::CrossesDevices => Transfer::CopyNeeded,
        Err(e) if e.kind() == ErrorKind::ReadOnlyFilesystem => Transfer::CopyNeeded,
        Err(e) => Transfer::Failed(e.to_string()),
    }
}

/// The folders tried, in order: `/Applications`, then `~/Applications` when
/// the first is not writable (a standard account).
fn destinations(bundle: &Path, home: Option<&Path>) -> Result<Vec<PathBuf>, String> {
    let name = bundle
        .file_name()
        .ok_or_else(|| "the app bundle has no name".to_owned())?;
    let mut out = vec![Path::new("/Applications").join(name)];
    if let Some(home) = home {
        out.push(home.join("Applications").join(name));
    }
    Ok(out)
}

#[cfg(target_os = "macos")]
fn copy_bundle(bundle: &Path, destination: &Path) -> Result<(), String> {
    let status = std::process::Command::new("/usr/bin/ditto")
        .arg(bundle)
        .arg(destination)
        .status()
        .map_err(|e| format!("cannot copy the app: {e}"))?;
    if !status.success() {
        let _ = std::fs::remove_dir_all(destination);
        return Err(format!("copying the app failed ({status})"));
    }
    // Best effort; a copy that keeps the flag still runs (translocated).
    let _ = std::process::Command::new("/usr/bin/xattr")
        .args(["-dr", "com.apple.quarantine"])
        .arg(destination)
        .status();
    Ok(())
}

/// Moves (or copies) the bundle into an Applications folder, opens the new
/// copy and quits. Returns where it went.
#[cfg(target_os = "macos")]
pub fn move_to_applications<R: tauri::Runtime>(
    app: &tauri::AppHandle<R>,
) -> Result<PathBuf, String> {
    let exe = std::env::current_exe().map_err(|e| format!("cannot locate the app: {e}"))?;
    let bundle = bundle_of_executable(&exe)
        .ok_or_else(|| "Move to Applications is available only in a packaged app".to_owned())?;
    let home = home_dir();
    if in_applications(&bundle, home.as_deref()) {
        return Ok(bundle);
    }
    let mut last = String::from("no Applications folder");
    let mut moved = None;
    for destination in destinations(&bundle, home.as_deref())? {
        if destination.exists() {
            return Err(format!(
                "{} already exists; move or remove it in Finder first",
                destination.display()
            ));
        }
        if let Some(parent) = destination.parent() {
            // `~/Applications` may not exist yet; `/Applications` always does.
            let _ = std::fs::create_dir_all(parent);
        }
        match classify(std::fs::rename(&bundle, &destination)) {
            Transfer::Renamed => {
                moved = Some(destination);
                break;
            }
            Transfer::CopyNeeded => match copy_bundle(&bundle, &destination) {
                Ok(()) => {
                    moved = Some(destination);
                    break;
                }
                Err(e) => last = e,
            },
            Transfer::Denied => {
                last = format!("no permission to write to {}", destination.display());
            }
            Transfer::Failed(e) => {
                return Err(format!(
                    "could not move the app to {}; move it in Finder instead: {e}",
                    destination.display()
                ))
            }
        }
    }
    let destination = moved.ok_or_else(|| format!("could not move the app: {last}"))?;
    log::info!("onboarding: the app moved to an Applications folder; relaunching");
    std::process::Command::new("/usr/bin/open")
        .arg("-n")
        .arg(&destination)
        .spawn()
        .map_err(|e| format!("the app moved, but could not reopen it: {e}"))?;
    app.exit(0);
    Ok(destination)
}

#[cfg(not(target_os = "macos"))]
pub fn move_to_applications<R: tauri::Runtime>(
    _app: &tauri::AppHandle<R>,
) -> Result<PathBuf, String> {
    Err("Move to Applications is available only on macOS".into())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn finds_the_bundle_and_recognizes_both_applications_folders() {
        let exe = Path::new("/Users/me/Downloads/Wispr Bro.app/Contents/MacOS/handy");
        let bundle = bundle_of_executable(exe);
        assert_eq!(
            bundle.as_deref(),
            Some(Path::new("/Users/me/Downloads/Wispr Bro.app"))
        );
        assert_eq!(
            location_of(bundle, Some(Path::new("/Users/me"))),
            ForkOnboardingLocation::MoveNeeded
        );
        assert_eq!(
            location_of(
                Some(PathBuf::from("/Applications/Wispr Bro.app")),
                Some(Path::new("/Users/me"))
            ),
            ForkOnboardingLocation::AlreadyInApplications
        );
        assert_eq!(
            location_of(
                Some(PathBuf::from("/Users/me/Applications/Wispr Bro.app")),
                Some(Path::new("/Users/me"))
            ),
            ForkOnboardingLocation::AlreadyInApplications
        );
        assert_eq!(location_of(None, None), ForkOnboardingLocation::NotBundled);
        // A dev build is not a bundle; a look-alike folder name is not one.
        assert_eq!(
            bundle_of_executable(Path::new("/repo/target/debug/handy")),
            None
        );
        assert_eq!(
            location_of(
                Some(PathBuf::from("/Applications Old/Wispr Bro.app")),
                Some(Path::new("/Users/me"))
            ),
            ForkOnboardingLocation::MoveNeeded
        );
    }

    #[test]
    fn translocated_and_disk_image_launches_need_a_copy() {
        let translocated = Path::new(
            "/private/var/folders/x/AppTranslocation/ABC/d/Wispr Bro.app/Contents/MacOS/handy",
        );
        let bundle = bundle_of_executable(translocated).unwrap();
        assert_eq!(
            location_of(Some(bundle.clone()), Some(Path::new("/Users/me"))),
            ForkOnboardingLocation::MoveNeeded
        );
        assert_eq!(
            destinations(&bundle, Some(Path::new("/Users/me"))).unwrap(),
            [
                PathBuf::from("/Applications/Wispr Bro.app"),
                PathBuf::from("/Users/me/Applications/Wispr Bro.app"),
            ]
        );
        let exdev = std::io::Error::from_raw_os_error(18);
        let erofs = std::io::Error::from_raw_os_error(30);
        let eacces = std::io::Error::from_raw_os_error(13);
        assert_eq!(classify(Err(exdev)), Transfer::CopyNeeded);
        assert_eq!(classify(Err(erofs)), Transfer::CopyNeeded);
        assert_eq!(classify(Err(eacces)), Transfer::Denied);
        assert_eq!(classify(Ok(())), Transfer::Renamed);
        assert!(matches!(
            classify(Err(std::io::Error::from_raw_os_error(2))),
            Transfer::Failed(_)
        ));
    }
}
