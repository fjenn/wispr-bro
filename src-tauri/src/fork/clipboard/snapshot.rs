// SPDX-License-Identifier: GPL-3.0-or-later
//! A clipboard snapshot as data: every `NSPasteboardItem` with every type it
//! carries and that type's bytes, in pasteboard order (R5 §4 W-07). Pure: no
//! AppKit here, so the fake-pasteboard tests use it directly
//! ([`super::PasteboardItem`] is [`Item`]). Written for the RS-05 spike and
//! lifted unchanged in behaviour by S-04.
//!
//! Also here: the skip policy (Wispr's documented rule: file contents, PDF,
//! RTFD and audio are not restored, R3 §clipboard), the byte-level comparison
//! RS-05 and the S-04 soak judge restores with, and a small binary format for
//! a clipboard backup file (the test tools keep the owner's clipboard in one
//! while they run).

use serde::Serialize;

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct Flavor {
    pub uti: String,
    #[serde(skip)]
    pub data: Vec<u8>,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize)]
pub struct Item {
    pub flavors: Vec<Flavor>,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize)]
pub struct Snapshot {
    pub items: Vec<Item>,
}

impl Item {
    pub fn new(flavors: &[(&str, &[u8])]) -> Item {
        Item {
            flavors: flavors
                .iter()
                .map(|(u, d)| Flavor {
                    uti: (*u).to_string(),
                    data: d.to_vec(),
                })
                .collect(),
        }
    }

    /// One plain-text item (`public.utf8-plain-text`).
    pub fn text(s: &str) -> Item {
        Item::default().with(super::markers::TEXT, s.as_bytes())
    }

    /// Adds a type (builder style, for tests and sources).
    pub fn with(mut self, uti: &str, data: &[u8]) -> Item {
        self.flavors.push(Flavor {
            uti: uti.to_string(),
            data: data.to_vec(),
        });
        self
    }

    pub fn get(&self, uti: &str) -> Option<&[u8]> {
        self.flavors
            .iter()
            .find(|f| f.uti == uti)
            .map(|f| f.data.as_slice())
    }

    pub fn has(&self, uti: &str) -> bool {
        self.get(uti).is_some()
    }
}

impl Snapshot {
    pub fn bytes(&self) -> usize {
        self.items
            .iter()
            .flat_map(|i| i.flavors.iter())
            .map(|f| f.data.len())
            .sum()
    }

    pub fn type_count(&self) -> usize {
        self.items.iter().map(|i| i.flavors.len()).sum()
    }

    /// Type names per item (what a clipboard manager sees without reading data).
    pub fn types(&self) -> Vec<Vec<String>> {
        self.items
            .iter()
            .map(|i| i.flavors.iter().map(|f| f.uti.clone()).collect())
            .collect()
    }
}

// ---------------------------------------------------------------------------
// Skip policy

/// Why a type is left out of the snapshot (and so not restored).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Skip {
    FileContents,
    Pdf,
    Rtfd,
    Audio,
}

const PDF: &[&str] = &[
    "com.adobe.pdf",
    "Apple PDF pasteboard type",
    "NSPDFPboardType",
];
const RTFD: &[&str] = &[
    "com.apple.flat-rtfd",
    "com.apple.rtfd",
    "NeXT RTFD pasteboard type",
    "NSRTFDPboardType",
];
const AUDIO: &[&str] = &[
    "public.audio",
    "public.aiff-audio",
    "public.aifc-audio",
    "public.mp3",
    "public.mpeg-4-audio",
    "public.ulaw-audio",
    "com.apple.m4a-audio",
    "com.apple.coreaudio-format",
    "com.microsoft.waveform-audio",
    "NeXT sound pasteboard type",
    "NSSoundPboardType",
];
/// File *contents* and file promises (the bytes of a file, or a promise to
/// write one), as opposed to a file URL, which is kept.
const FILE_CONTENTS: &[&str] = &[
    "NSFileContentsPboardType",
    "NSPromiseContentsPboardType",
    "Apple files promise pasteboard type",
    "com.apple.NSFilePromiseItemMetaData",
    "com.apple.pasteboard.promised-file-content-type",
    "com.apple.pasteboard.promised-file-url",
    "com.apple.pasteboard.promised-file-name",
    "com.apple.pasteboard.promised-suggested-file-name",
];

pub fn skip_reason(uti: &str) -> Option<Skip> {
    if PDF.contains(&uti) {
        Some(Skip::Pdf)
    } else if RTFD.contains(&uti) {
        Some(Skip::Rtfd)
    } else if AUDIO.contains(&uti) {
        Some(Skip::Audio)
    } else if FILE_CONTENTS.contains(&uti) {
        Some(Skip::FileContents)
    } else {
        None
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct Skipped {
    pub item: usize,
    pub uti: String,
    pub reason: Skip,
    pub bytes: usize,
}

/// The snapshot to keep for a restore: every type except the skipped ones.
/// An item left with no types is dropped (restoring an empty item would add
/// an item the user never had).
pub fn apply_policy(full: &Snapshot) -> (Snapshot, Vec<Skipped>) {
    let mut skipped = Vec::new();
    let mut items = Vec::new();
    for (i, item) in full.items.iter().enumerate() {
        let mut kept = Item::default();
        for f in &item.flavors {
            match skip_reason(&f.uti) {
                Some(reason) => skipped.push(Skipped {
                    item: i,
                    uti: f.uti.clone(),
                    reason,
                    bytes: f.data.len(),
                }),
                None => kept.flavors.push(f.clone()),
            }
        }
        if !kept.flavors.is_empty() {
            items.push(kept);
        }
    }
    (Snapshot { items }, skipped)
}

// ---------------------------------------------------------------------------
// Comparison

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case", tag = "outcome")]
pub enum TypeOutcome {
    Identical,
    /// Left out on purpose (the skip policy).
    Skipped {
        reason: Skip,
    },
    /// Was there before, missing after, and not skipped on purpose.
    Lost,
    Changed {
        before_bytes: usize,
        after_bytes: usize,
    },
    /// Not there before.
    Added,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct TypeDiff {
    pub item: usize,
    pub uti: String,
    pub bytes: usize,
    #[serde(flatten)]
    pub outcome: TypeOutcome,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct Fidelity {
    pub items_before: usize,
    pub items_after: usize,
    pub types: Vec<TypeDiff>,
    /// Same type order within every item.
    pub order_kept: bool,
}

impl Fidelity {
    /// Every kept type came back with the same bytes, nothing was added, and
    /// the item count matches (items the policy emptied excepted).
    pub fn byte_identical(&self) -> bool {
        self.problems() == 0
    }

    pub fn problems(&self) -> usize {
        self.types
            .iter()
            .filter(|d| {
                matches!(
                    d.outcome,
                    TypeOutcome::Lost | TypeOutcome::Changed { .. } | TypeOutcome::Added
                )
            })
            .count()
    }

    pub fn skipped(&self) -> usize {
        self.types
            .iter()
            .filter(|d| matches!(d.outcome, TypeOutcome::Skipped { .. }))
            .count()
    }

    /// One line, e.g. "identical (7 types)" or "2 problems: lost public.rtf …".
    pub fn describe(&self) -> String {
        let n = self.types.len();
        let skipped = self.skipped();
        let extra = if skipped > 0 {
            format!(", {skipped} skipped by policy")
        } else {
            String::new()
        };
        if self.byte_identical() {
            return format!("identical ({} types{extra})", n - skipped);
        }
        let bad: Vec<String> = self
            .types
            .iter()
            .filter_map(|d| match &d.outcome {
                TypeOutcome::Lost => Some(format!("lost {}", d.uti)),
                TypeOutcome::Changed { .. } => Some(format!("changed {}", d.uti)),
                TypeOutcome::Added => Some(format!("added {}", d.uti)),
                _ => None,
            })
            .collect();
        format!("{} problem(s): {}{extra}", self.problems(), bad.join(", "))
    }
}

/// Compares the clipboard before the take with the clipboard after the
/// restore, type by type. Items the policy empties are expected to vanish,
/// so the remaining items are paired in order.
pub fn compare(before: &Snapshot, after: &Snapshot) -> Fidelity {
    let mut types = Vec::new();
    let mut order_kept = true;
    let mut after_items = after.items.iter().enumerate();
    let mut paired_after = 0;
    for (bi, item) in before.items.iter().enumerate() {
        let survives = item.flavors.iter().any(|f| skip_reason(&f.uti).is_none());
        let partner = if survives {
            after_items.next().map(|(_, it)| it)
        } else {
            None
        };
        if partner.is_some() {
            paired_after += 1;
        }
        for f in &item.flavors {
            let outcome = match (skip_reason(&f.uti), partner.and_then(|p| p.get(&f.uti))) {
                (_, Some(d)) if d == f.data.as_slice() => TypeOutcome::Identical,
                (_, Some(d)) => TypeOutcome::Changed {
                    before_bytes: f.data.len(),
                    after_bytes: d.len(),
                },
                (Some(reason), None) => TypeOutcome::Skipped { reason },
                (None, None) => TypeOutcome::Lost,
            };
            types.push(TypeDiff {
                item: bi,
                uti: f.uti.clone(),
                bytes: f.data.len(),
                outcome,
            });
        }
        if let Some(p) = partner {
            for f in &p.flavors {
                if !item.has(&f.uti) {
                    types.push(TypeDiff {
                        item: bi,
                        uti: f.uti.clone(),
                        bytes: f.data.len(),
                        outcome: TypeOutcome::Added,
                    });
                }
            }
            let kept_before: Vec<&str> = item
                .flavors
                .iter()
                .map(|f| f.uti.as_str())
                .filter(|u| p.has(u))
                .collect();
            let kept_after: Vec<&str> = p
                .flavors
                .iter()
                .map(|f| f.uti.as_str())
                .filter(|u| item.has(u))
                .collect();
            if kept_before != kept_after {
                order_kept = false;
            }
        }
    }
    // Extra items after the restore that pair with nothing.
    for (ai, item) in after_items {
        for f in &item.flavors {
            types.push(TypeDiff {
                item: ai,
                uti: f.uti.clone(),
                bytes: f.data.len(),
                outcome: TypeOutcome::Added,
            });
        }
        paired_after += 1;
    }
    Fidelity {
        items_before: before.items.len(),
        items_after: after.items.len().max(paired_after),
        types,
        order_kept,
    }
}

// ---------------------------------------------------------------------------
// Backup file format (the owner's own clipboard, kept until it is restored)

const MAGIC: &[u8] = b"WBCLIP1\n";

pub fn encode(s: &Snapshot) -> Vec<u8> {
    let mut out = MAGIC.to_vec();
    let put = |out: &mut Vec<u8>, n: usize| out.extend_from_slice(&(n as u64).to_le_bytes());
    put(&mut out, s.items.len());
    for item in &s.items {
        put(&mut out, item.flavors.len());
        for f in &item.flavors {
            put(&mut out, f.uti.len());
            out.extend_from_slice(f.uti.as_bytes());
            put(&mut out, f.data.len());
            out.extend_from_slice(&f.data);
        }
    }
    out
}

pub fn decode(bytes: &[u8]) -> Result<Snapshot, String> {
    let rest = bytes
        .strip_prefix(MAGIC)
        .ok_or("not a Wispr Bro clipboard backup")?;
    let mut pos = 0usize;
    let mut take = |n: usize| -> Result<&[u8], String> {
        let end = pos.checked_add(n).filter(|&e| e <= rest.len());
        let end = end.ok_or("backup file is truncated")?;
        let s = &rest[pos..end];
        pos = end;
        Ok(s)
    };
    fn num(b: &[u8]) -> usize {
        let mut a = [0u8; 8];
        a.copy_from_slice(b);
        u64::from_le_bytes(a) as usize
    }
    let items = num(take(8)?);
    let mut s = Snapshot::default();
    for _ in 0..items {
        let n = num(take(8)?);
        let mut item = Item::default();
        for _ in 0..n {
            let ul = num(take(8)?);
            let uti = String::from_utf8(take(ul)?.to_vec()).map_err(|e| e.to_string())?;
            let dl = num(take(8)?);
            let data = take(dl)?.to_vec();
            item.flavors.push(Flavor { uti, data });
        }
        s.items.push(item);
    }
    Ok(s)
}

#[cfg(test)]
mod tests {
    use super::*;

    const TXT: &str = "public.utf8-plain-text";

    fn snap(items: &[&[(&str, &[u8])]]) -> Snapshot {
        Snapshot {
            items: items.iter().map(|f| Item::new(f)).collect(),
        }
    }

    #[test]
    fn skip_policy_matches_wispr_list() {
        assert_eq!(skip_reason("com.adobe.pdf"), Some(Skip::Pdf));
        assert_eq!(skip_reason("com.apple.flat-rtfd"), Some(Skip::Rtfd));
        assert_eq!(skip_reason("com.apple.coreaudio-format"), Some(Skip::Audio));
        assert_eq!(
            skip_reason("com.apple.pasteboard.promised-file-url"),
            Some(Skip::FileContents)
        );
        // Kept: text, rich text, HTML, file URLs, images.
        for keep in [
            TXT,
            "public.rtf",
            "public.html",
            "public.file-url",
            "public.png",
            "public.tiff",
            "com.apple.webarchive",
        ] {
            assert_eq!(skip_reason(keep), None, "{keep}");
        }
    }

    #[test]
    fn policy_drops_emptied_items_and_reports_them() {
        let full = snap(&[
            &[(TXT, b"a"), ("com.apple.flat-rtfd", b"rtfd")],
            &[("com.adobe.pdf", b"%PDF")],
            &[("public.png", b"png")],
        ]);
        let (kept, skipped) = apply_policy(&full);
        assert_eq!(kept, snap(&[&[(TXT, b"a")], &[("public.png", b"png")]]));
        assert_eq!(skipped.len(), 2);
        assert_eq!(skipped[1].item, 1);
        assert_eq!(skipped[1].reason, Skip::Pdf);
        assert_eq!(skipped[0].bytes, 4);
    }

    #[test]
    fn identical_restore() {
        let a = snap(&[&[(TXT, b"x"), ("public.rtf", b"{\\rtf1}")], &[(TXT, b"y")]]);
        let f = compare(&a, &a.clone());
        assert!(f.byte_identical());
        assert!(f.order_kept);
        assert_eq!(f.describe(), "identical (3 types)");
    }

    #[test]
    fn lost_changed_and_added_types_are_problems() {
        let before = snap(&[&[(TXT, b"x"), ("public.rtf", b"r"), ("public.html", b"h")]]);
        let after = snap(&[&[(TXT, b"x"), ("public.html", b"H!"), ("public.png", b"p")]]);
        let f = compare(&before, &after);
        assert!(!f.byte_identical());
        assert_eq!(f.problems(), 3);
        let by = |u: &str| f.types.iter().find(|d| d.uti == u).unwrap().outcome.clone();
        assert_eq!(by("public.rtf"), TypeOutcome::Lost);
        assert_eq!(
            by("public.html"),
            TypeOutcome::Changed {
                before_bytes: 1,
                after_bytes: 2
            }
        );
        assert_eq!(by("public.png"), TypeOutcome::Added);
    }

    #[test]
    fn skipped_types_and_emptied_items_are_not_problems() {
        let before = snap(&[
            &[("com.adobe.pdf", b"%PDF")],
            &[(TXT, b"t"), ("com.apple.flat-rtfd", b"d")],
        ]);
        let (kept, _) = apply_policy(&before);
        let f = compare(&before, &kept);
        assert!(f.byte_identical(), "{f:?}");
        assert_eq!(f.skipped(), 2);
        assert_eq!(f.describe(), "identical (1 types, 2 skipped by policy)");
    }

    #[test]
    fn missing_and_extra_items() {
        let two = snap(&[&[(TXT, b"a")], &[(TXT, b"b")]]);
        let one = snap(&[&[(TXT, b"a")]]);
        assert_eq!(compare(&two, &one).problems(), 1);
        let f = compare(&one, &two);
        assert_eq!(f.problems(), 1);
        assert_eq!(f.items_after, 2);
        // An empty clipboard restored as empty.
        assert!(compare(&Snapshot::default(), &Snapshot::default()).byte_identical());
    }

    #[test]
    fn order_change_is_reported_but_not_a_problem() {
        let a = snap(&[&[(TXT, b"x"), ("public.rtf", b"r")]]);
        let b = snap(&[&[("public.rtf", b"r"), (TXT, b"x")]]);
        let f = compare(&a, &b);
        assert!(f.byte_identical());
        assert!(!f.order_kept);
    }

    #[test]
    fn backup_round_trip() {
        let s = snap(&[
            &[(TXT, "café ✓".as_bytes()), ("public.png", &[0, 1, 2, 255])],
            &[],
            &[("public.file-url", b"file:///tmp/a")],
        ]);
        assert_eq!(decode(&encode(&s)).unwrap(), s);
        assert_eq!(
            decode(&encode(&Snapshot::default())).unwrap(),
            Snapshot::default()
        );
        let mut cut = encode(&s);
        cut.truncate(cut.len() - 3);
        assert!(decode(&cut).is_err());
        assert!(decode(b"nope").is_err());
    }
}
