// SPDX-License-Identifier: GPL-3.0-or-later
//! Move to Applications (S-29a) and startup location handling (alpha.3).
//! The move runs only by itself (with a clean destination), on the user's
//! press in the small prompt after a failed move, or from an old build's
//! button; it never overwrites another app. The path decisions are
//! unit-tested; the move itself runs in the Mac wave (MAC-TEST-SCRIPT S-29a).
//!
//! The idea follows LetsMove (move, else copy, then relaunch the copy);
//! nothing is ported. A plain `rename` works inside one volume; a copy is
//! needed from a disk image, another volume or a translocated (read-only)
//! launch, where `ditto` keeps the bundle's signature, symlinks and
//! extended attributes. The copy's quarantine flag is then cleared: this
//! app has already passed Gatekeeper to be running, and a flagged copy would
//! be translocated again on its next launch.
//!
//! At startup [`ensure_location`] settles where the app runs from before
//! onboarding gets far: already installed is a no-op; a free destination is
//! moved to and relaunched silently; an existing installed copy is opened
//! instead (never overwritten, so a newer build stays); only a permission or
//! copy failure asks, in one small native dialog.

use std::path::{Path, PathBuf};

use crate::fork::commands::onboarding::ForkOnboardingLocation;
use crate::fork::ForkRuntime;

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

/// What startup does about where the app runs from (alpha.3). Pure over the
/// bundle, the candidate folders and whether a destination exists, so it is
/// tested without touching the disk.
#[derive(Debug, PartialEq, Eq)]
pub enum EnsureDecision {
    /// Already in an Applications folder, or not a bundle (a dev build).
    Nothing,
    /// No installed copy: move or copy there and relaunch.
    Install,
    /// An installed copy is already there: open it instead of overwriting.
    OpenExisting(PathBuf),
}

fn plan_ensure(
    bundle: Option<PathBuf>,
    home: Option<&Path>,
    exists: impl Fn(&Path) -> bool,
) -> EnsureDecision {
    let Some(bundle) = bundle else {
        return EnsureDecision::Nothing;
    };
    if in_applications(&bundle, home) {
        return EnsureDecision::Nothing;
    }
    let Ok(candidates) = destinations(&bundle, home) else {
        return EnsureDecision::Nothing;
    };
    match candidates.into_iter().find(|candidate| exists(candidate)) {
        Some(existing) => EnsureDecision::OpenExisting(existing),
        None => EnsureDecision::Install,
    }
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

/// Startup location handling (alpha.3, macOS only), called from
/// `fork::on_ready`. Nothing to do is the common case and returns at once;
/// an existing installed copy is opened and this instance quits; otherwise
/// the move runs off the main thread (the copy can take a moment) and, if it
/// fails, one small native prompt offers to try again.
#[cfg(target_os = "macos")]
pub fn ensure_location<R: ForkRuntime>(app: &tauri::AppHandle<R>) {
    let decision = plan_ensure(
        std::env::current_exe()
            .ok()
            .and_then(|exe| bundle_of_executable(&exe)),
        home_dir().as_deref(),
        |candidate| candidate.exists(),
    );
    match decision {
        EnsureDecision::Nothing => {}
        EnsureDecision::OpenExisting(destination) => open_existing(app, &destination),
        EnsureDecision::Install => {
            let app = app.clone();
            let spawned = std::thread::Builder::new()
                .name("wb-install-location".into())
                .spawn(move || install_or_prompt(&app));
            if let Err(e) = spawned {
                log::warn!("onboarding: cannot start the move to Applications: {e}");
            }
        }
    }
}

#[cfg(not(target_os = "macos"))]
pub fn ensure_location<R: ForkRuntime>(_: &tauri::AppHandle<R>) {}

#[cfg(target_os = "macos")]
fn open_existing<R: ForkRuntime>(app: &tauri::AppHandle<R>, destination: &Path) {
    log::info!(
        "onboarding: an installed copy already exists at {}; opening it",
        destination.display()
    );
    match std::process::Command::new("/usr/bin/open")
        .arg("-n")
        .arg(destination)
        .spawn()
    {
        Ok(_) => app.exit(0),
        Err(e) => log::warn!(
            "onboarding: could not open the installed copy at {}: {e}",
            destination.display()
        ),
    }
}

#[cfg(target_os = "macos")]
fn install_or_prompt<R: ForkRuntime>(app: &tauri::AppHandle<R>) {
    match move_to_applications(app) {
        Ok(_) => {}
        Err(e) => {
            log::warn!("onboarding: automatic move to Applications failed: {e}");
            // Another launch may have installed a copy in the meantime: open
            // it rather than asking again.
            let decision = plan_ensure(
                std::env::current_exe()
                    .ok()
                    .and_then(|exe| bundle_of_executable(&exe)),
                home_dir().as_deref(),
                |candidate| candidate.exists(),
            );
            match decision {
                EnsureDecision::OpenExisting(destination) => open_existing(app, &destination),
                _ => prompt_move(app),
            }
        }
    }
}

/// The one small dialog after a failed automatic move. Not the onboarding
/// screen: Move retries the same move; Not Now keeps running from where it
/// is (the user can still drag it to Applications themselves).
#[cfg(target_os = "macos")]
fn prompt_move<R: ForkRuntime>(app: &tauri::AppHandle<R>) {
    use tauri_plugin_dialog::{DialogExt, MessageDialogButtons, MessageDialogKind};

    let locale = crate::fork::i18n::current_locale(R::handy_app_language(app).as_deref());
    let title = crate::fork::i18n::t(locale, "rust.onboarding.movePrompt.title", &[]);
    let body = crate::fork::i18n::t(locale, "rust.onboarding.movePrompt.body", &[]);
    let move_label = crate::fork::i18n::t(locale, "rust.onboarding.movePrompt.move", &[]);
    let not_now = crate::fork::i18n::t(locale, "rust.onboarding.movePrompt.notNow", &[]);
    let moved = app
        .dialog()
        .message(body)
        .title(title)
        .kind(MessageDialogKind::Info)
        .buttons(MessageDialogButtons::OkCancelCustom(move_label, not_now))
        .blocking_show();
    if moved {
        if let Err(e) = move_to_applications(app) {
            log::warn!("onboarding: the move requested from the prompt failed: {e}");
        }
    } else {
        log::info!("onboarding: staying outside an Applications folder for now");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn finds_the_bundle_and_recognizes_both_applications_folders() {
        let exe = Path::new("/Users/me/Downloads/Wispr Bro.app/Contents/MacOS/wispr-bro");
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
            "/private/var/folders/x/AppTranslocation/ABC/d/Wispr Bro.app/Contents/MacOS/wispr-bro",
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

    #[test]
    fn startup_installs_only_when_the_destination_is_free() {
        let home = Some(Path::new("/Users/me"));
        let bundle = Some(PathBuf::from("/Users/me/Downloads/Wispr Bro.app"));
        assert_eq!(
            plan_ensure(bundle.clone(), home, |_| false),
            EnsureDecision::Install
        );
        // The destination exists: open the installed copy, never overwrite it.
        assert_eq!(
            plan_ensure(bundle.clone(), home, |candidate| candidate
                == Path::new("/Applications/Wispr Bro.app")),
            EnsureDecision::OpenExisting(PathBuf::from("/Applications/Wispr Bro.app"))
        );
        // A standard account may have installed into ~/Applications.
        assert_eq!(
            plan_ensure(bundle, home, |candidate| candidate
                == Path::new("/Users/me/Applications/Wispr Bro.app")),
            EnsureDecision::OpenExisting(PathBuf::from("/Users/me/Applications/Wispr Bro.app"))
        );
    }

    #[test]
    fn startup_leaves_installed_and_unbundled_apps_alone() {
        let home = Some(Path::new("/Users/me"));
        for bundle in [
            "/Applications/Wispr Bro.app",
            "/Users/me/Applications/Wispr Bro.app",
        ] {
            assert_eq!(
                plan_ensure(Some(PathBuf::from(bundle)), home, |_| false),
                EnsureDecision::Nothing
            );
        }
        // A dev build is not a bundle: nothing to move.
        assert_eq!(plan_ensure(None, home, |_| false), EnsureDecision::Nothing);
    }
}
