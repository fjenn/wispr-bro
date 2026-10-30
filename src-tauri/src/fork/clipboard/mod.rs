// SPDX-License-Identifier: GPL-3.0-or-later
//! Clipboard layer (R5 §4.7, W-07, S-04).
//!
//! Lifted from the RS-05 spike (`crates/wb-spike-clipboard/src/clip/`), which
//! now compiles these same files through `#[path]`, so the RS-05 tool and the
//! S-04 soak run exactly the product code:
//!
//! - [`snapshot`]: a clipboard snapshot as data, the skip policy, byte
//!   comparison (pure);
//! - [`restore`]: the restore guard and the wait rule (pure; ports hex and
//!   Handy);
//! - [`markers`]: the nspasteboard.org markers on our transcript (pure);
//! - [`target`]: native / Chromium / Electron and the 155/156 check (hex);
//! - [`keys`]: ⌘V, typing, the modifier-release wait (hex and Handy; macOS);
//! - [`nspb`]: objc2 `NSPasteboard` read and write and the lazy transcript
//!   (macOS).
//!
//! RS-05 settled the backend: the objc2 `NSPasteboardItem` path restores
//! every type byte for byte; hex's Carbon path (kept only in the spike as
//! `carbon.rs`) adds two text types on restore. `arboard`'s
//! `exclude_from_history()` is not needed: it writes the same concealed
//! marker [`nspb::publish_transcript`] writes.
//!
//! The paste itself (snapshot → transcript → ⌘V → verify → restore) is
//! `fork::insert`.

pub mod markers;
pub mod restore;
pub mod snapshot;
pub mod target;

#[cfg(target_os = "macos")]
pub mod keys;
#[cfg(target_os = "macos")]
pub mod nspb;

pub use snapshot::{Item, Snapshot};

/// One `NSPasteboardItem`: every type it carries with its bytes, in order.
/// The name the F-10 test contract uses for [`snapshot::Item`].
pub type PasteboardItem = Item;

/// The general pasteboard. Moved here from `fork::testutil` (F-10 contract,
/// unchanged in shape); [`MacPasteboard`] is the real one,
/// `testutil::FakePasteboard` the test double.
pub trait Pasteboard: Send + Sync {
    /// `NSPasteboard.changeCount`: bumps on every write by anyone.
    fn change_count(&self) -> i64;
    /// All items with all their types.
    fn read(&self) -> Vec<PasteboardItem>;
    /// Clears the pasteboard and writes `items`; returns the new change count.
    fn write(&self, items: Vec<PasteboardItem>) -> i64;
}

impl<T: Pasteboard + ?Sized> Pasteboard for std::sync::Arc<T> {
    fn change_count(&self) -> i64 {
        (**self).change_count()
    }
    fn read(&self) -> Vec<PasteboardItem> {
        (**self).read()
    }
    fn write(&self, items: Vec<PasteboardItem>) -> i64 {
        (**self).write(items)
    }
}

/// The real general pasteboard through [`nspb`].
#[cfg(target_os = "macos")]
#[derive(Clone, Copy, Debug, Default)]
pub struct MacPasteboard;

#[cfg(target_os = "macos")]
impl Pasteboard for MacPasteboard {
    fn change_count(&self) -> i64 {
        nspb::change_count()
    }

    fn read(&self) -> Vec<PasteboardItem> {
        nspb::read().0.items
    }

    /// Returns the change count after the write; on a failed write, the
    /// current count (the caller's `changeCount` guard then sees no write
    /// of ours and leaves the clipboard alone).
    fn write(&self, items: Vec<PasteboardItem>) -> i64 {
        match nspb::write(&Snapshot { items }) {
            Ok(count) => count,
            Err(e) => {
                log::warn!("clipboard: write failed: {e}");
                nspb::change_count()
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fork::testutil::FakePasteboard;

    #[test]
    fn pasteboard_item_helpers() {
        let item = PasteboardItem::text("hi").with("public.rtf", b"{\\rtf1}");
        assert_eq!(item.get(markers::TEXT), Some(&b"hi"[..]));
        assert!(item.has("public.rtf"));
        assert!(!item.has("public.png"));
        assert_eq!(
            Snapshot {
                items: vec![item.clone()]
            }
            .types(),
            vec![vec![markers::TEXT.to_string(), "public.rtf".to_string()]]
        );
    }

    #[test]
    fn fake_is_a_pasteboard_through_arc() {
        let pb: std::sync::Arc<dyn Pasteboard> = std::sync::Arc::new(FakePasteboard::new());
        assert_eq!(pb.write(vec![PasteboardItem::text("x")]), 1);
        assert_eq!(pb.change_count(), 1);
        assert_eq!(pb.read(), vec![PasteboardItem::text("x")]);
    }
}
