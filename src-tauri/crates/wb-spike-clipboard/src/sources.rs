// SPDX-License-Identifier: GPL-3.0-or-later
//! What is on the clipboard before each paste (RS-05: plain text, RTF from
//! TextEdit, HTML from Safari, a file URL from Finder, an image, two files).
//!
//! Two kinds: synthetic (written by the tool, the same bytes every run) and
//! real copies (the tool opens the file it made in TextEdit, Safari or
//! Finder and presses ⌘A/⌘C there, so the clipboard holds exactly the types
//! those apps write). Real copies need permission to post key events; the
//! paste matrix uses a real copy where one was made.

use serde::Serialize;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use crate::clip::keys;
use crate::clip::nspb;
use crate::clip::snapshot::{Item, Snapshot};
use crate::macos;

pub const TEXT: &str = "public.utf8-plain-text";
/// Every source's text contains this, so a paste of the old clipboard
/// (instead of the transcript) is easy to spot.
pub const SOURCE_MARK: &str = "WB-SOURCE";

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Kind {
    Plain,
    Rtf,
    Html,
    FileUrl,
    Image,
    TwoFiles,
    /// Fidelity checks only: a 12 MB image, for snapshot timing.
    BigImage,
}

impl Kind {
    pub const MATRIX: [Kind; 6] = [
        Kind::Plain,
        Kind::Rtf,
        Kind::Html,
        Kind::FileUrl,
        Kind::Image,
        Kind::TwoFiles,
    ];

    pub fn label(&self) -> &'static str {
        match self {
            Kind::Plain => "plain text",
            Kind::Rtf => "rich text (RTF)",
            Kind::Html => "web page (HTML)",
            Kind::FileUrl => "a file",
            Kind::Image => "an image (PNG)",
            Kind::TwoFiles => "two files",
            Kind::BigImage => "a 12 MB image",
        }
    }
}

#[derive(Clone, Debug, Serialize)]
pub struct Source {
    pub kind: Kind,
    /// "synthetic" or the app it was copied from.
    pub origin: String,
    pub snapshot: Snapshot,
}

pub struct Files {
    pub file_a: PathBuf,
    pub file_b: PathBuf,
    pub rtf: PathBuf,
    pub html: PathBuf,
}

const RTF: &str = r"{\rtf1\ansi\ansicpg1252\cocoartf2822
{\fonttbl\f0\fswiss\fcharset0 Helvetica;}
{\colortbl;\red255\green255\blue255;\red200\green0\blue0;}
\pard\f0\fs28 WB-SOURCE rich text: \b bold\b0 , \i italic\i0  and \cf2 red\cf0 .}
";

const HTML: &str = "<!doctype html>\n<html><head><meta charset=\"utf-8\"><title>WB source page</title></head>\n\
<body style=\"font-size:28px;margin:0;padding:1em 1em 40em\"><p>WB-SOURCE web page: <b>bold</b>, <i>italic</i>, a list:</p><ul><li>one</li><li>two \u{2713}</li></ul></body></html>\n";

pub fn make_files(dir: &Path) -> Result<Files, String> {
    let dir = dir.join("sources");
    fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    let f = Files {
        file_a: dir.join("WB-SOURCE file A.txt"),
        file_b: dir.join("WB-SOURCE file B.txt"),
        rtf: dir.join("WB-SOURCE rich.rtf"),
        html: dir.join("WB-SOURCE page.html"),
    };
    let w = |p: &Path, b: &[u8]| fs::write(p, b).map_err(|e| format!("{}: {e}", p.display()));
    w(&f.file_a, b"WB-SOURCE file A\n")?;
    w(&f.file_b, b"WB-SOURCE file B\n")?;
    w(&f.rtf, RTF.as_bytes())?;
    w(&f.html, HTML.as_bytes())?;
    Ok(f)
}

/// `file:///…` with percent-encoding, as Finder writes it.
pub fn file_url(p: &Path) -> String {
    let mut s = String::from("file://");
    for b in p.to_string_lossy().bytes() {
        if b.is_ascii_alphanumeric() || b"/-._~".contains(&b) {
            s.push(b as char);
        } else {
            s.push_str(&format!("%{b:02X}"));
        }
    }
    s
}

pub fn png(width: u32, height: u32, noise: bool) -> Vec<u8> {
    let mut out = Vec::new();
    {
        let mut enc = png::Encoder::new(&mut out, width, height);
        enc.set_color(png::ColorType::Rgb);
        enc.set_depth(png::BitDepth::Eight);
        let mut w = enc.write_header().expect("png header");
        let mut seed: u32 = 0x2545_f491;
        let mut data = Vec::with_capacity((width * height * 3) as usize);
        for y in 0..height {
            for x in 0..width {
                if noise {
                    seed ^= seed << 13;
                    seed ^= seed >> 17;
                    seed ^= seed << 5;
                    data.extend_from_slice(&seed.to_le_bytes()[..3]);
                } else {
                    data.extend_from_slice(&[
                        (x * 255 / width.max(1)) as u8,
                        (y * 255 / height.max(1)) as u8,
                        160,
                    ]);
                }
            }
        }
        w.write_image_data(&data).expect("png data");
    }
    out
}

pub fn synthetic(kind: Kind, f: &Files) -> Source {
    let item = |v: &[(&str, &[u8])]| Item::new(v);
    let file_item = |p: &Path| {
        let name = p
            .file_name()
            .unwrap_or_default()
            .to_string_lossy()
            .into_owned();
        Item::new(&[
            ("public.file-url", file_url(p).as_bytes()),
            (TEXT, name.as_bytes()),
        ])
    };
    let items = match kind {
        Kind::Plain => vec![item(&[(
            TEXT,
            "WB-SOURCE plain text: café, naïve, 日本語, emoji 🙂\nsecond line\n".as_bytes(),
        )])],
        Kind::Rtf => vec![item(&[
            ("public.rtf", RTF.as_bytes()),
            (TEXT, b"WB-SOURCE rich text: bold, italic and red."),
        ])],
        Kind::Html => vec![item(&[
            ("public.html", HTML.as_bytes()),
            (
                TEXT,
                "WB-SOURCE web page: bold, italic, a list:\none\ntwo \u{2713}".as_bytes(),
            ),
        ])],
        Kind::FileUrl => vec![file_item(&f.file_a)],
        Kind::Image => vec![item(&[("public.png", &png(64, 48, false))])],
        Kind::TwoFiles => vec![file_item(&f.file_a), file_item(&f.file_b)],
        Kind::BigImage => vec![item(&[("public.png", &png(2560, 1600, true))])],
    };
    Source {
        kind,
        origin: "synthetic".into(),
        snapshot: Snapshot { items },
    }
}

// ---------------------------------------------------------------------------
// Real copies

/// `click`: click into the window first (Safari keeps key focus in the
/// address bar after `open`, so ⌘A/⌘C would copy the URL).
fn copy_in_front(
    bundle: &str,
    select_all: bool,
    click: bool,
    title_hint: Option<&str>,
) -> Result<Snapshot, String> {
    let front = macos::wait_front(bundle, Duration::from_secs(10))
        .ok_or_else(|| format!("{bundle} did not come to the front"))?;
    if let Some(hint) = title_hint {
        let end = Instant::now() + Duration::from_secs(5);
        loop {
            let t = macos::focus(front.pid).window_title.unwrap_or_default();
            if t.contains(hint) {
                break;
            }
            if Instant::now() > end {
                return Err(format!(
                    "the {hint} window did not open (front window: {t:?})"
                ));
            }
            macos::pump(Duration::from_millis(100));
        }
    }
    macos::pump(Duration::from_millis(600));
    let still = |what: &str| -> Result<(), String> {
        match macos::frontmost() {
            Some(f) if f.bundle_id == bundle => Ok(()),
            other => Err(format!(
                "{} came to the front before {what}; stopped",
                other.map(|f| f.name).unwrap_or_default()
            )),
        }
    };
    if click {
        still("the click")?;
        if !macos::click_in_window(front.pid, 0.6) {
            return Err("could not click into the window".into());
        }
        macos::pump(Duration::from_millis(300));
    }
    let cc = nspb::change_count();
    if select_all {
        still("⌘A")?;
        keys::post_command_letter(b'a')?;
        macos::pump(Duration::from_millis(300));
    }
    still("⌘C")?;
    keys::post_command_letter(b'c')?;
    let end = Instant::now() + Duration::from_secs(3);
    while nspb::change_count() == cc {
        if Instant::now() > end {
            return Err("⌘C did not change the clipboard".into());
        }
        macos::pump(Duration::from_millis(20));
    }
    macos::pump(Duration::from_millis(300));
    Ok(nspb::read().0)
}

/// Copies from the real apps. Each entry is the copy or why it failed.
pub fn real_copies(f: &Files) -> Vec<(Kind, Result<Source, String>)> {
    let mut out = Vec::new();
    let mut add = |kind: Kind, origin: &str, r: Result<Snapshot, String>| {
        out.push((
            kind,
            r.map(|snapshot| Source {
                kind,
                origin: origin.into(),
                snapshot,
            }),
        ));
    };
    let rtf = f.rtf.to_string_lossy().into_owned();
    let r = if macos::open(&["-a", "TextEdit", &rtf]) {
        copy_in_front("com.apple.TextEdit", true, false, Some("WB-SOURCE rich"))
    } else {
        Err("could not open TextEdit".into())
    };
    add(Kind::Rtf, "TextEdit", r);

    let html = f.html.to_string_lossy().into_owned();
    let r = if macos::open(&["-a", "Safari", &html]) {
        copy_in_front("com.apple.Safari", true, true, Some("WB source page"))
    } else {
        Err("could not open Safari".into())
    };
    add(Kind::Html, "Safari", r);

    macos::reveal_in_finder(std::slice::from_ref(&f.file_a));
    add(
        Kind::FileUrl,
        "Finder",
        copy_in_front("com.apple.finder", false, false, None),
    );
    macos::reveal_in_finder(&[f.file_a.clone(), f.file_b.clone()]);
    add(
        Kind::TwoFiles,
        "Finder",
        copy_in_front("com.apple.finder", false, false, None),
    );
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn file_urls_are_percent_encoded() {
        assert_eq!(
            file_url(Path::new("/Users/x/WB-SOURCE file A.txt")),
            "file:///Users/x/WB-SOURCE%20file%20A.txt"
        );
    }

    #[test]
    fn synthetic_sources_have_the_listed_types() {
        let dir = std::env::temp_dir().join(format!("wb-rs05-src-{}", std::process::id()));
        let f = make_files(&dir).unwrap();
        let s = synthetic(Kind::TwoFiles, &f);
        assert_eq!(s.snapshot.items.len(), 2);
        assert!(s.snapshot.items[1].has("public.file-url"));
        let img = synthetic(Kind::Image, &f);
        assert!(img.snapshot.items[0]
            .get("public.png")
            .unwrap()
            .starts_with(b"\x89PNG"));
        for k in Kind::MATRIX {
            let s = synthetic(k, &f);
            assert!(!s.snapshot.items.is_empty(), "{k:?}");
        }
        fs::remove_dir_all(&dir).unwrap();
    }
}
