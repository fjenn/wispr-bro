// SPDX-License-Identifier: GPL-3.0-or-later
//! The apps on this Mac, for the Style page (S-19a): which are installed
//! (the "+" picker and the group lists), their names and their icons.
//!
//! Everything comes from the file system and NSWorkspace (objc2, in tree):
//! the `.app` bundles in the usual folders plus the apps running now, and
//! each app's Finder icon rendered at 64 px as a PNG `data:` URL (the Hub's
//! CSP allows `img-src data:`). Nothing leaves the Mac. Results are cached:
//! the folder scan for [`SCAN_TTL`], icons for the process lifetime.

use std::collections::HashMap;
use std::sync::Mutex;
use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};
use specta::Type;

/// One installed app.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, Type)]
pub struct InstalledApp {
    pub bundle_id: String,
    /// The name Finder shows.
    pub name: String,
    pub path: String,
}

/// How long a folder scan is reused.
pub const SCAN_TTL: Duration = Duration::from_secs(60);

/// Icon edge in points (rendered at 2x for Retina).
pub const ICON_POINTS: f64 = 32.0;

static SCAN: Mutex<Option<(Instant, Vec<InstalledApp>)>> = Mutex::new(None);
static ICONS: Mutex<Option<HashMap<String, Option<String>>>> = Mutex::new(None);

/// Every installed app, by name (each bundle id once).
pub fn installed() -> Vec<InstalledApp> {
    let mut cache = SCAN.lock().unwrap_or_else(|e| e.into_inner());
    if let Some((at, apps)) = cache.as_ref() {
        if at.elapsed() < SCAN_TTL {
            return apps.clone();
        }
    }
    let apps = sorted_unique(platform::scan());
    *cache = Some((Instant::now(), apps.clone()));
    apps
}

/// Drops the cached scan (after the user installed an app, tests).
pub fn forget_scan() {
    *SCAN.lock().unwrap_or_else(|e| e.into_inner()) = None;
}

/// The installed app with this bundle id, if any.
pub fn locate(bundle_id: &str) -> Option<InstalledApp> {
    if let Some(a) = installed()
        .into_iter()
        .find(|a| a.bundle_id.eq_ignore_ascii_case(bundle_id))
    {
        return Some(a);
    }
    platform::locate(bundle_id)
}

/// The app's icon as a PNG `data:` URL (cached; `None` when the app is not
/// installed or the icon could not be drawn).
pub fn icon_data_url(bundle_id: &str) -> Option<String> {
    let key = bundle_id.to_lowercase();
    if let Some(hit) = ICONS
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .as_ref()
        .and_then(|m| m.get(&key).cloned())
    {
        return hit;
    }
    let url = locate(bundle_id)
        .and_then(|a| platform::icon_png(&a.path))
        .map(|png| format!("data:image/png;base64,{}", base64(&png)));
    ICONS
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .get_or_insert_with(HashMap::new)
        .insert(key, url.clone());
    url
}

/// One entry per bundle id (the first path wins), sorted by name.
pub fn sorted_unique(mut apps: Vec<InstalledApp>) -> Vec<InstalledApp> {
    let mut seen = std::collections::HashSet::new();
    apps.retain(|a| !a.bundle_id.is_empty() && seen.insert(a.bundle_id.to_lowercase()));
    apps.sort_by(|a, b| {
        a.name
            .to_lowercase()
            .cmp(&b.name.to_lowercase())
            .then_with(|| a.bundle_id.cmp(&b.bundle_id))
    });
    apps
}

/// Standard base64 with padding (RFC 4648), for the icon URLs.
pub fn base64(bytes: &[u8]) -> String {
    const T: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::with_capacity(bytes.len().div_ceil(3) * 4);
    for chunk in bytes.chunks(3) {
        let b = [
            chunk[0],
            chunk.get(1).copied().unwrap_or(0),
            chunk.get(2).copied().unwrap_or(0),
        ];
        let n = (u32::from(b[0]) << 16) | (u32::from(b[1]) << 8) | u32::from(b[2]);
        out.push(T[(n >> 18) as usize & 63] as char);
        out.push(T[(n >> 12) as usize & 63] as char);
        out.push(if chunk.len() > 1 {
            T[(n >> 6) as usize & 63] as char
        } else {
            '='
        });
        out.push(if chunk.len() > 2 {
            T[n as usize & 63] as char
        } else {
            '='
        });
    }
    out
}

/// A display name without its `.app` extension.
pub fn display_name(raw: &str) -> String {
    raw.strip_suffix(".app").unwrap_or(raw).trim().to_string()
}

#[cfg(target_os = "macos")]
mod platform {
    use super::{display_name, InstalledApp, ICON_POINTS};
    use objc2::rc::Retained;
    use objc2::AnyThread;
    use objc2_app_kit::{
        NSApplicationActivationPolicy, NSBitmapImageFileType, NSBitmapImageRep, NSWorkspace,
    };
    use objc2_foundation::{
        NSBundle, NSDictionary, NSFileManager, NSPoint, NSRect, NSSize, NSString,
    };
    use std::path::{Path, PathBuf};

    fn folders() -> Vec<PathBuf> {
        let mut v: Vec<PathBuf> = [
            "/Applications",
            "/Applications/Utilities",
            "/System/Applications",
            "/System/Applications/Utilities",
            "/System/Library/CoreServices/Applications",
        ]
        .iter()
        .map(PathBuf::from)
        .collect();
        if let Some(home) = std::env::var_os("HOME") {
            v.push(PathBuf::from(home).join("Applications"));
        }
        v
    }

    fn app_at(path: &Path) -> Option<InstalledApp> {
        let p = NSString::from_str(&path.to_string_lossy());
        let bundle: Retained<NSBundle> = NSBundle::bundleWithPath(&p)?;
        let bundle_id = bundle.bundleIdentifier()?.to_string();
        let name = display_name(
            &NSFileManager::defaultManager()
                .displayNameAtPath(&p)
                .to_string(),
        );
        Some(InstalledApp {
            bundle_id,
            name,
            path: path.to_string_lossy().into_owned(),
        })
    }

    /// The `.app` bundles in [`folders`] and one folder level below
    /// `/Applications` ("Microsoft Office", vendor folders), plus the
    /// regular apps running now.
    pub fn scan() -> Vec<InstalledApp> {
        let mut out = Vec::new();
        for dir in folders() {
            let Ok(entries) = std::fs::read_dir(&dir) else {
                continue;
            };
            for e in entries.flatten() {
                let path = e.path();
                if path.extension().is_some_and(|x| x == "app") {
                    out.extend(app_at(&path));
                } else if dir == Path::new("/Applications")
                    && path.is_dir()
                    && path != Path::new("/Applications/Utilities")
                {
                    if let Ok(inner) = std::fs::read_dir(&path) {
                        for i in inner.flatten() {
                            let p = i.path();
                            if p.extension().is_some_and(|x| x == "app") {
                                out.extend(app_at(&p));
                            }
                        }
                    }
                }
            }
        }
        let ws = NSWorkspace::sharedWorkspace();
        for app in ws.runningApplications().iter() {
            if app.activationPolicy() != NSApplicationActivationPolicy::Regular {
                continue;
            }
            if let Some(path) = app.bundleURL().and_then(|u| u.path()) {
                out.extend(app_at(Path::new(&path.to_string())));
            }
        }
        out
    }

    pub fn locate(bundle_id: &str) -> Option<InstalledApp> {
        let url = NSWorkspace::sharedWorkspace()
            .URLForApplicationWithBundleIdentifier(&NSString::from_str(bundle_id))?;
        let path = url.path()?.to_string();
        app_at(Path::new(&path))
    }

    /// The Finder icon of the bundle at `path`, as PNG bytes at 2x.
    pub fn icon_png(path: &str) -> Option<Vec<u8>> {
        let image = NSWorkspace::sharedWorkspace().iconForFile(&NSString::from_str(path));
        let px = ICON_POINTS * 2.0;
        image.setSize(NSSize::new(px, px));
        let mut rect = NSRect::new(NSPoint::new(0.0, 0.0), NSSize::new(px, px));
        // SAFETY: a valid rect pointer, no context, no hints.
        let cg = unsafe { image.CGImageForProposedRect_context_hints(&mut rect, None, None) }?;
        let rep = NSBitmapImageRep::initWithCGImage(NSBitmapImageRep::alloc(), &cg);
        // SAFETY: an empty properties dictionary is valid for PNG.
        let data = unsafe {
            rep.representationUsingType_properties(NSBitmapImageFileType::PNG, &NSDictionary::new())
        }?;
        Some(data.to_vec())
    }
}

#[cfg(not(target_os = "macos"))]
mod platform {
    use super::InstalledApp;

    pub fn scan() -> Vec<InstalledApp> {
        Vec::new()
    }

    pub fn locate(_bundle_id: &str) -> Option<InstalledApp> {
        None
    }

    pub fn icon_png(_path: &str) -> Option<Vec<u8>> {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn base64_matches_rfc4648() {
        assert_eq!(base64(b""), "");
        assert_eq!(base64(b"f"), "Zg==");
        assert_eq!(base64(b"fo"), "Zm8=");
        assert_eq!(base64(b"foo"), "Zm9v");
        assert_eq!(base64(b"foobar"), "Zm9vYmFy");
        assert_eq!(base64(&[0xff, 0xfe, 0x00]), "//4A");
    }

    #[test]
    fn names_and_dedupe() {
        assert_eq!(display_name("Slack.app"), "Slack");
        assert_eq!(display_name("Mail"), "Mail");
        let a = |id: &str, n: &str| InstalledApp {
            bundle_id: id.into(),
            name: n.into(),
            path: format!("/Applications/{n}.app"),
        };
        let out = sorted_unique(vec![
            a("com.b", "beta"),
            a("com.a", "Alpha"),
            a("COM.A", "Alpha copy"),
            a("", "No id"),
        ]);
        assert_eq!(
            out.iter().map(|x| x.name.as_str()).collect::<Vec<_>>(),
            ["Alpha", "beta"]
        );
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn finds_a_system_app_and_draws_its_icon() {
        // TextEdit ships with every macOS.
        let app = locate("com.apple.TextEdit").expect("TextEdit is installed");
        assert!(app.path.ends_with("TextEdit.app"), "{}", app.path);
        let url = icon_data_url("com.apple.TextEdit").expect("an icon");
        assert!(
            url.starts_with("data:image/png;base64,iVBOR"),
            "{}",
            &url[..40]
        );
        assert!(installed()
            .iter()
            .any(|a| a.bundle_id == "com.apple.TextEdit"));
        assert_eq!(icon_data_url("com.example.not-installed-wb"), None);
    }
}
