// SPDX-License-Identifier: MIT AND GPL-3.0-or-later
// Copyright (c) 2026 Christopher Langton (hex: Chromium version detection)
// Modified by Wispr Bro contributors, 2026-10-30.
//
// `chromium_major_versions`, `chromium_major_version` and the 155..=156 range
// are ported from hex `src/paste.rs` (anomalyco/hex on GitHub, branch dev,
// commit 3607595d92345af4be9f0eaa86ae3750c8851e39, MIT). Changes: they return
// the versions instead of a yes/no, Electron apps are recognised and their
// Chromium version is read from the framework binary (new, GPL-3.0-or-later),
// and the result is a `TargetClass`. MIT text: LICENSES/MIT-hex.txt. See
// docs/fork/PORTS.md.

//! Which kind of app a paste goes to (native, Chromium, Electron) and its
//! Chromium version: Chromium 155 and 156 drop a ⌘V that races their own
//! pasteboard notification, so hex types the text instead there.

use std::fs;
use std::io::Read;
use std::ops::RangeInclusive;
use std::path::Path;

use super::restore::TargetClass;

/// Chromium CL 8217390 (155) introduced the race, CL 8503629 (157) fixed it.
pub const CLIPBOARD_RACE_CHROMIUM: RangeInclusive<u32> = 155..=156;

pub fn has_clipboard_race(class: TargetClass) -> bool {
    class
        .chromium_major()
        .is_some_and(|m| CLIPBOARD_RACE_CHROMIUM.contains(&m))
}

pub fn classify(bundle: &Path) -> TargetClass {
    let frameworks = bundle.join("Contents/Frameworks");
    let electron = frameworks.join("Electron Framework.framework");
    if electron.exists() {
        return TargetClass::Electron {
            chromium_major: electron_chromium_major(&electron),
        };
    }
    let majors: Vec<u32> = chromium_major_versions(bundle).collect();
    if majors.is_empty() {
        TargetClass::Native
    } else {
        TargetClass::Chromium {
            major: majors.into_iter().max(),
        }
    }
}

/// Chromium browsers (Chrome, Brave, Edge, Chromium itself and most forks)
/// ship `Contents/Frameworks/<Name> Framework.framework/Versions/
/// <major>.<minor>.<build>.<patch>/`. Electron's `Versions/A` has no version.
pub fn chromium_major_versions(bundle: &Path) -> impl Iterator<Item = u32> {
    fs::read_dir(bundle.join("Contents/Frameworks"))
        .into_iter()
        .flatten()
        .flatten()
        .filter(|framework| {
            framework
                .file_name()
                .to_str()
                .is_some_and(|name| name.ends_with("Framework.framework"))
        })
        .flat_map(|framework| fs::read_dir(framework.path().join("Versions")).into_iter())
        .flatten()
        .flatten()
        .filter_map(|version| chromium_major_version(version.file_name().to_str()?))
}

pub fn chromium_major_version(version: &str) -> Option<u32> {
    let components = version
        .split('.')
        .map(str::parse::<u32>)
        .collect::<Result<Vec<_>, _>>()
        .ok()?;
    match components[..] {
        [major, _, _, _] => Some(major),
        _ => None,
    }
}

/// Electron embeds its Chromium version in the framework binary's user agent
/// ("Chrome/134.0.6998.205 Electron/35…"). Reads the binary in chunks.
fn electron_chromium_major(framework: &Path) -> Option<u32> {
    let bin = framework.join("Electron Framework");
    let bin = if bin.exists() {
        bin
    } else {
        framework.join("Versions/A/Electron Framework")
    };
    let mut f = fs::File::open(bin).ok()?;
    let mut buf = vec![0u8; 4 << 20];
    let mut carry: Vec<u8> = Vec::new();
    loop {
        let n = f.read(&mut buf).ok()?;
        if n == 0 {
            return None;
        }
        let mut hay = std::mem::take(&mut carry);
        hay.extend_from_slice(&buf[..n]);
        if let Some(m) = find_chrome_version(&hay) {
            return Some(m);
        }
        let keep = hay.len().min(64);
        carry = hay[hay.len() - keep..].to_vec();
    }
}

/// The major of the first "Chrome/<a>.<b>.<c>.<d>" in `hay`.
pub fn find_chrome_version(hay: &[u8]) -> Option<u32> {
    const NEEDLE: &[u8] = b"Chrome/";
    let mut i = 0;
    while i + NEEDLE.len() < hay.len() {
        if &hay[i..i + NEEDLE.len()] == NEEDLE {
            let rest = &hay[i + NEEDLE.len()..];
            let end = rest
                .iter()
                .position(|b| !(b.is_ascii_digit() || *b == b'.'))
                .unwrap_or(rest.len());
            if let Ok(s) = std::str::from_utf8(&rest[..end]) {
                if let Some(m) = chromium_major_version(s.trim_end_matches('.')) {
                    return Some(m);
                }
            }
        }
        i += 1;
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn version_parsing() {
        assert_eq!(chromium_major_version("150.0.7700.12"), Some(150));
        assert_eq!(chromium_major_version("A"), None);
        assert_eq!(chromium_major_version("150.0.1"), None);
        assert_eq!(
            find_chrome_version(b"Mozilla/5.0 Chrome/134.0.6998.205 Electron/35.1.0"),
            Some(134)
        );
        assert_eq!(find_chrome_version(b"Chrome/%s Chrome/1.2"), None);
    }

    #[test]
    fn race_range() {
        let c = |m| TargetClass::Chromium { major: Some(m) };
        assert!(!has_clipboard_race(c(154)));
        assert!(has_clipboard_race(c(155)));
        assert!(has_clipboard_race(c(156)));
        assert!(!has_clipboard_race(c(157)));
        assert!(has_clipboard_race(TargetClass::Electron {
            chromium_major: Some(156)
        }));
        assert!(!has_clipboard_race(TargetClass::Native));
    }

    #[test]
    fn classify_fake_bundles() {
        let root = std::env::temp_dir().join(format!("wb-rs05-target-{}", std::process::id()));
        let chrome = root.join("Chrome.app");
        fs::create_dir_all(
            chrome.join(
                "Contents/Frameworks/Google Chrome Framework.framework/Versions/150.0.7700.1",
            ),
        )
        .unwrap();
        assert_eq!(
            classify(&chrome),
            TargetClass::Chromium { major: Some(150) }
        );
        let el = root.join("El.app/Contents/Frameworks/Electron Framework.framework");
        fs::create_dir_all(&el).unwrap();
        fs::write(
            el.join("Electron Framework"),
            b"xx Chrome/138.0.7204.1 Electron/37",
        )
        .unwrap();
        assert_eq!(
            classify(&root.join("El.app")),
            TargetClass::Electron {
                chromium_major: Some(138)
            }
        );
        fs::create_dir_all(root.join("Native.app/Contents")).unwrap();
        assert_eq!(classify(&root.join("Native.app")), TargetClass::Native);
        fs::remove_dir_all(&root).unwrap();
    }
}
