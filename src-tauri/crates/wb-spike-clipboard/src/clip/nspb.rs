// SPDX-License-Identifier: GPL-3.0-or-later
//! The general pasteboard through objc2 `NSPasteboard` (R4 R-10: own
//! snapshot code on objc2, following Hex legacy `PasteboardSnapshot`'s
//! pattern: items → types → data; one `NSPasteboardItem` per saved item on
//! restore).
//!
//! The transcript is written as one item: the three markers as eager empty
//! data and the text as a lazy promise (`setDataProvider:forTypes:`), so the
//! provider callback is the read receipt (Handy's reliable paste does the
//! same with `declareTypes:owner:`). Callbacks arrive on the main run loop:
//! everything here runs on the main thread, and waits pump the run loop
//! (`crate::macos::pump`).

use objc2::rc::Retained;
use objc2::runtime::ProtocolObject;
use objc2::{define_class, msg_send, AnyThread, DefinedClass};
use objc2_app_kit::{
    NSPasteboard, NSPasteboardItem, NSPasteboardItemDataProvider, NSPasteboardType,
    NSPasteboardWriting,
};
use objc2_foundation::{NSArray, NSData, NSObject, NSObjectProtocol, NSString};
use std::sync::{Arc, Mutex};
use std::time::Instant;

use super::markers;
use super::snapshot::{Flavor, Item, Snapshot};

fn general() -> Retained<NSPasteboard> {
    NSPasteboard::generalPasteboard()
}

pub fn change_count() -> i64 {
    general().changeCount() as i64
}

/// Everything on the clipboard. Types that list no data (a promise that
/// fails) are returned in the second value and left out of the snapshot.
pub fn read() -> (Snapshot, Vec<String>) {
    let pb = general();
    let mut snap = Snapshot::default();
    let mut unreadable = Vec::new();
    let Some(items) = pb.pasteboardItems() else {
        return (snap, unreadable);
    };
    for item in items.iter() {
        let mut out = Item::default();
        for ty in item.types().iter() {
            match item.dataForType(&ty) {
                Some(data) => out.flavors.push(Flavor {
                    uti: ty.to_string(),
                    data: data.to_vec(),
                }),
                None => unreadable.push(ty.to_string()),
            }
        }
        snap.items.push(out);
    }
    (snap, unreadable)
}

/// Type names per item, without asking for any data (a clipboard manager's
/// first look; does not fire our lazy text).
pub fn types() -> Vec<Vec<String>> {
    let pb = general();
    pb.pasteboardItems()
        .map(|items| {
            items
                .iter()
                .map(|i| i.types().iter().map(|t| t.to_string()).collect())
                .collect()
        })
        .unwrap_or_default()
}

/// Replaces the clipboard with `snap` (one item per saved item, every type
/// with its bytes). Returns the new change count.
pub fn write(snap: &Snapshot) -> Result<i64, String> {
    let pb = general();
    let items: Vec<Retained<NSPasteboardItem>> = snap
        .items
        .iter()
        .map(|it| {
            let item = NSPasteboardItem::new();
            for f in &it.flavors {
                let data = NSData::with_bytes(&f.data);
                if !item.setData_forType(&data, &NSString::from_str(&f.uti)) {
                    return Err(format!("could not set {}", f.uti));
                }
            }
            Ok(item)
        })
        .collect::<Result<_, _>>()?;
    pb.clearContents();
    if !items.is_empty() {
        write_items(&pb, &items)?;
    }
    Ok(pb.changeCount() as i64)
}

fn write_items(pb: &NSPasteboard, items: &[Retained<NSPasteboardItem>]) -> Result<(), String> {
    let objects: Vec<&ProtocolObject<dyn NSPasteboardWriting>> = items
        .iter()
        .map(|i| ProtocolObject::from_ref(&**i))
        .collect();
    if pb.writeObjects(&NSArray::from_slice(&objects)) {
        Ok(())
    } else {
        Err("could not write the clipboard".into())
    }
}

// ---------------------------------------------------------------------------
// The transcript (lazy text + markers)

#[derive(Default)]
pub struct ProviderState {
    pub reads: Vec<Instant>,
    /// AppKit is done with the provider: someone else wrote, or every
    /// promised type has been provided.
    pub finished: bool,
}

pub struct ProviderIvars {
    text: String,
    state: Arc<Mutex<ProviderState>>,
}

define_class!(
    // SAFETY: NSObject has no subclassing requirements; the ivars are plain
    // Rust values behind a Mutex.
    #[unsafe(super(NSObject))]
    #[name = "WbRs05TranscriptProvider"]
    #[ivars = ProviderIvars]
    pub struct TranscriptProvider;

    unsafe impl NSObjectProtocol for TranscriptProvider {}

    unsafe impl NSPasteboardItemDataProvider for TranscriptProvider {
        #[unsafe(method(pasteboard:item:provideDataForType:))]
        fn provide(
            &self,
            _pb: Option<&NSPasteboard>,
            item: &NSPasteboardItem,
            ty: &NSPasteboardType,
        ) {
            let iv = self.ivars();
            if let Ok(mut s) = iv.state.lock() {
                s.reads.push(Instant::now());
            }
            item.setString_forType(&NSString::from_str(&iv.text), ty);
        }

        #[unsafe(method(pasteboardFinishedWithDataProvider:))]
        fn finished(&self, _pb: &NSPasteboard) {
            if let Ok(mut s) = self.ivars().state.lock() {
                s.finished = true;
            }
        }
    }
);

/// Our transcript on the clipboard. Keep it alive until the restore.
pub struct Published {
    pub change_count: i64,
    pub state: Arc<Mutex<ProviderState>>,
    _provider: Option<Retained<TranscriptProvider>>,
}

impl Published {
    pub fn reads(&self) -> Vec<Instant> {
        self.state
            .lock()
            .map(|s| s.reads.clone())
            .unwrap_or_default()
    }

    pub fn finished(&self) -> bool {
        self.state.lock().map(|s| s.finished).unwrap_or(false)
    }
}

/// Writes `text` with the three markers in one item, by a single write, so a
/// polling clipboard manager never sees the text without them. `lazy`: the
/// text is a promise (read receipts); otherwise it is eager data.
pub fn publish_transcript(text: &str, lazy: bool) -> Result<Published, String> {
    let pb = general();
    let item = NSPasteboardItem::new();
    let empty = NSData::new();
    for m in markers::ALL {
        if !item.setData_forType(&empty, &NSString::from_str(m)) {
            return Err(format!("could not set {m}"));
        }
    }
    let state = Arc::new(Mutex::new(ProviderState::default()));
    let text_type = NSString::from_str(markers::TEXT);
    let provider = if lazy {
        let p = TranscriptProvider::alloc().set_ivars(ProviderIvars {
            text: text.to_string(),
            state: state.clone(),
        });
        // SAFETY: NSObject's init.
        let p: Retained<TranscriptProvider> = unsafe { msg_send![super(p), init] };
        let types = NSArray::from_slice(&[&*text_type]);
        if !item.setDataProvider_forTypes(ProtocolObject::from_ref(&*p), &types) {
            return Err("could not set the text promise".into());
        }
        Some(p)
    } else {
        if !item.setString_forType(&NSString::from_str(text), &text_type) {
            return Err("could not set the text".into());
        }
        None
    };
    pb.clearContents();
    write_items(&pb, &[item])?;
    Ok(Published {
        change_count: pb.changeCount() as i64,
        state,
        _provider: provider,
    })
}
