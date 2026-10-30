// SPDX-License-Identifier: MIT AND GPL-3.0-or-later
// Copyright (c) 2026 Christopher Langton (hex)
// Modified by Wispr Bro contributors, 2026-10-30.
//
// Ported from hex `src/paste.rs` (`capture_clipboard`, `restore_clipboard`,
// `should_preserve_flavor` and the CoreFoundation helpers), anomalyco/hex on
// GitHub, branch dev, commit 3607595d92345af4be9f0eaa86ae3750c8851e39, MIT.
// Changes: errors are strings instead of eyre reports, tracing is dropped,
// the result is our `Snapshot` plus the Carbon flavor flags, and the R5 skip
// policy is applied after capture. MIT text: LICENSES/MIT-hex.txt. See
// docs/fork/PORTS.md.

//! The second snapshot backend RS-05 compares: hex's Carbon Pasteboard
//! Manager path. It sees which flavors macOS synthesised (flag bit 8) and
//! skips them when an authoritative flavor exists, and it restores the
//! flavor flags; the objc2 path (`nspb.rs`) cannot see either. R4 R-10 chose
//! objc2 to avoid the deprecated API; RS-05 checks whether that costs
//! fidelity.

use std::ffi::{c_char, c_void};
use std::ptr;

use super::snapshot::{apply_policy, Flavor, Item, Snapshot};

type PasteboardRef = *const c_void;
type PasteboardItemId = *mut c_void;
type CfTypeRef = *const c_void;
type CfStringRef = *const c_void;
type CfArrayRef = *const c_void;
type CfDataRef = *const c_void;

const CF_STRING_ENCODING_UTF8: u32 = 0x0800_0100;
const BAD_PASTEBOARD_FLAVOR: i32 = -25133;
const DUPLICATE_PASTEBOARD_FLAVOR: i32 = -25134;
const SYSTEM_TRANSLATED_FLAVOR: u32 = 1 << 8;

#[link(name = "ApplicationServices", kind = "framework")]
extern "C" {
    fn PasteboardCreate(name: CfStringRef, pasteboard: *mut PasteboardRef) -> i32;
    fn PasteboardSynchronize(pasteboard: PasteboardRef) -> u32;
    fn PasteboardClear(pasteboard: PasteboardRef) -> i32;
    fn PasteboardGetItemCount(pasteboard: PasteboardRef, count: *mut usize) -> i32;
    fn PasteboardGetItemIdentifier(
        pasteboard: PasteboardRef,
        index: isize,
        item: *mut PasteboardItemId,
    ) -> i32;
    fn PasteboardCopyItemFlavors(
        pasteboard: PasteboardRef,
        item: PasteboardItemId,
        flavors: *mut CfArrayRef,
    ) -> i32;
    fn PasteboardGetItemFlavorFlags(
        pasteboard: PasteboardRef,
        item: PasteboardItemId,
        flavor: CfStringRef,
        flags: *mut u32,
    ) -> i32;
    fn PasteboardCopyItemFlavorData(
        pasteboard: PasteboardRef,
        item: PasteboardItemId,
        flavor: CfStringRef,
        data: *mut CfDataRef,
    ) -> i32;
    fn PasteboardPutItemFlavor(
        pasteboard: PasteboardRef,
        item: PasteboardItemId,
        flavor: CfStringRef,
        data: CfDataRef,
        flags: u32,
    ) -> i32;
}

#[link(name = "CoreFoundation", kind = "framework")]
extern "C" {
    fn CFArrayGetCount(array: CfArrayRef) -> isize;
    fn CFArrayGetValueAtIndex(array: CfArrayRef, index: isize) -> *const c_void;
    fn CFDataCreate(allocator: *const c_void, bytes: *const u8, length: isize) -> CfDataRef;
    fn CFDataGetBytePtr(data: CfDataRef) -> *const u8;
    fn CFDataGetLength(data: CfDataRef) -> isize;
    fn CFStringGetLength(value: CfStringRef) -> isize;
    fn CFStringGetMaximumSizeForEncoding(length: isize, encoding: u32) -> isize;
    fn CFStringGetCString(
        value: CfStringRef,
        buffer: *mut c_char,
        buffer_size: isize,
        encoding: u32,
    ) -> bool;
    fn CFStringCreateWithBytes(
        allocator: *const c_void,
        bytes: *const u8,
        length: isize,
        encoding: u32,
        is_external_representation: u8,
    ) -> CfStringRef;
    fn CFRelease(value: CfTypeRef);
}

struct PasteboardHandle(PasteboardRef);

impl Drop for PasteboardHandle {
    fn drop(&mut self) {
        // SAFETY: created by PasteboardCreate (Create Rule).
        unsafe { CFRelease(self.0) };
    }
}

/// A Carbon capture: the snapshot plus each flavor's flags (same shape).
#[derive(Clone, Debug, Default)]
pub struct CarbonSnapshot {
    pub snapshot: Snapshot,
    pub flags: Vec<Vec<u32>>,
    /// Flavors left out because macOS synthesised them.
    pub synthesized_skipped: usize,
}

pub fn should_preserve_flavor(flags: u32, has_authoritative_flavor: bool) -> bool {
    !has_authoritative_flavor || flags & SYSTEM_TRANSLATED_FLAVOR == 0
}

fn general_name() -> CfStringRef {
    // "Apple CFPasteboard general" is NSPasteboardNameGeneral; the toll-free
    // bridged NSString works as a CFString.
    let name = objc2_app_kit::NSPasteboard::generalPasteboard().name();
    let ptr = (&*name as *const objc2_foundation::NSString).cast();
    // Keep the name alive: one retain of a framework constant per call
    // (a test tool calls this a few dozen times).
    std::mem::forget(name);
    ptr
}

fn create_pasteboard() -> Result<PasteboardHandle, String> {
    let mut pasteboard = ptr::null();
    check_status(
        // SAFETY: valid CFString name and out-pointer.
        unsafe { PasteboardCreate(general_name(), &mut pasteboard) },
        "open the clipboard",
    )?;
    if pasteboard.is_null() {
        return Err("could not open the clipboard".into());
    }
    Ok(PasteboardHandle(pasteboard))
}

pub fn capture() -> Result<CarbonSnapshot, String> {
    let pasteboard = create_pasteboard()?;
    // SAFETY: valid handle.
    unsafe { PasteboardSynchronize(pasteboard.0) };
    let mut item_count = 0;
    check_status(
        // SAFETY: valid handle and out-pointer.
        unsafe { PasteboardGetItemCount(pasteboard.0, &mut item_count) },
        "count clipboard items",
    )?;
    let mut out = CarbonSnapshot::default();
    for index in 1..=item_count {
        let mut item = ptr::null_mut();
        check_status(
            // SAFETY: index in 1..=count.
            unsafe { PasteboardGetItemIdentifier(pasteboard.0, index as isize, &mut item) },
            "identify a clipboard item",
        )?;
        let mut flavors = ptr::null();
        check_status(
            // SAFETY: valid item id.
            unsafe { PasteboardCopyItemFlavors(pasteboard.0, item, &mut flavors) },
            "list clipboard formats",
        )?;
        if flavors.is_null() {
            return Err("could not list clipboard formats".into());
        }
        let result = read_item(&pasteboard, item, flavors);
        // SAFETY: Copy Rule array.
        unsafe { CFRelease(flavors) };
        let (it, fl, skipped) = result?;
        out.snapshot.items.push(it);
        out.flags.push(fl);
        out.synthesized_skipped += skipped;
    }
    // R5 skip policy, keeping the flags aligned with the kept flavors.
    let (kept, _) = apply_policy(&out.snapshot);
    let mut flags = Vec::new();
    for (it, fl) in out.snapshot.items.iter().zip(&out.flags) {
        let k: Vec<u32> = it
            .flavors
            .iter()
            .zip(fl)
            .filter(|(f, _)| super::snapshot::skip_reason(&f.uti).is_none())
            .map(|(_, x)| *x)
            .collect();
        if !k.is_empty() {
            flags.push(k);
        }
    }
    out.snapshot = kept;
    out.flags = flags;
    Ok(out)
}

fn read_item(
    pasteboard: &PasteboardHandle,
    item: PasteboardItemId,
    flavors: CfArrayRef,
) -> Result<(Item, Vec<u32>, usize), String> {
    // SAFETY: valid array.
    let flavor_count = unsafe { CFArrayGetCount(flavors) };
    let mut entries = Vec::new();
    for flavor_index in 0..flavor_count {
        // SAFETY: index in range.
        let flavor = unsafe { CFArrayGetValueAtIndex(flavors, flavor_index) };
        if flavor.is_null() {
            return Err("clipboard format was unavailable".into());
        }
        let mut flags = 0;
        check_status(
            // SAFETY: valid item and flavor.
            unsafe { PasteboardGetItemFlavorFlags(pasteboard.0, item, flavor, &mut flags) },
            "inspect clipboard format",
        )?;
        entries.push((flavor, flags));
    }
    let has_authoritative_flavor = entries
        .iter()
        .any(|&(_, flags)| flags & SYSTEM_TRANSLATED_FLAVOR == 0);
    let mut it = Item::default();
    let mut fl = Vec::new();
    let mut skipped = 0;
    for (flavor, flags) in entries {
        let data_type = cf_string(flavor)?;
        if !should_preserve_flavor(flags, has_authoritative_flavor) {
            skipped += 1;
            continue;
        }
        let mut data = ptr::null();
        // SAFETY: valid item and flavor; Copy Rule data released below.
        let status = unsafe { PasteboardCopyItemFlavorData(pasteboard.0, item, flavor, &mut data) };
        if status == BAD_PASTEBOARD_FLAVOR {
            continue;
        }
        check_status(status, "preserve clipboard format")?;
        if data.is_null() {
            return Err("clipboard format data was unavailable".into());
        }
        let bytes = cf_data(data);
        // SAFETY: Copy Rule.
        unsafe { CFRelease(data) };
        it.flavors.push(Flavor {
            uti: data_type,
            data: bytes?,
        });
        fl.push(flags & 0x0f);
    }
    Ok((it, fl, skipped))
}

pub fn restore(snap: &CarbonSnapshot) -> Result<(), String> {
    let pasteboard = create_pasteboard()?;
    // SAFETY: valid handle.
    unsafe { PasteboardSynchronize(pasteboard.0) };
    check_status(
        // SAFETY: valid handle.
        unsafe { PasteboardClear(pasteboard.0) },
        "clear the clipboard",
    )?;
    for (item_index, (item, flags)) in snap.snapshot.items.iter().zip(&snap.flags).enumerate() {
        let item_id = (item_index + 1) as PasteboardItemId;
        for (flavor, flag) in item.flavors.iter().zip(flags) {
            let data_type = cf_string_create(&flavor.uti)?;
            // SAFETY: valid byte slice.
            let data = unsafe {
                CFDataCreate(
                    ptr::null(),
                    flavor.data.as_ptr(),
                    flavor.data.len() as isize,
                )
            };
            if data.is_null() {
                // SAFETY: Create Rule.
                unsafe { CFRelease(data_type) };
                return Err("could not encode a clipboard format".into());
            }
            // SAFETY: valid handle, CF objects; the pasteboard copies them.
            let status =
                unsafe { PasteboardPutItemFlavor(pasteboard.0, item_id, data_type, data, *flag) };
            // SAFETY: Create Rule.
            unsafe {
                CFRelease(data);
                CFRelease(data_type);
            }
            if status == DUPLICATE_PASTEBOARD_FLAVOR {
                continue;
            }
            check_status(status, "restore clipboard format")?;
        }
    }
    Ok(())
}

fn check_status(status: i32, action: &str) -> Result<(), String> {
    if status == 0 {
        Ok(())
    } else {
        Err(format!("could not {action} (status {status})"))
    }
}

fn cf_string_create(value: &str) -> Result<CfStringRef, String> {
    // SAFETY: valid UTF-8 bytes.
    let value = unsafe {
        CFStringCreateWithBytes(
            ptr::null(),
            value.as_ptr(),
            value.len() as isize,
            CF_STRING_ENCODING_UTF8,
            0,
        )
    };
    if value.is_null() {
        Err("could not encode a clipboard format name".into())
    } else {
        Ok(value)
    }
}

fn cf_string(value: CfStringRef) -> Result<String, String> {
    // SAFETY: valid CFString.
    let length = unsafe { CFStringGetLength(value) };
    // SAFETY: pure computation.
    let capacity = unsafe { CFStringGetMaximumSizeForEncoding(length, CF_STRING_ENCODING_UTF8) }
        .saturating_add(1);
    let mut bytes = vec![0u8; capacity as usize];
    // SAFETY: the buffer has `capacity` bytes.
    if !unsafe {
        CFStringGetCString(
            value,
            bytes.as_mut_ptr().cast(),
            capacity,
            CF_STRING_ENCODING_UTF8,
        )
    } {
        return Err("could not decode a clipboard format name".into());
    }
    let length = bytes.iter().position(|b| *b == 0).unwrap_or(bytes.len());
    bytes.truncate(length);
    String::from_utf8(bytes).map_err(|e| e.to_string())
}

fn cf_data(value: CfDataRef) -> Result<Vec<u8>, String> {
    // SAFETY: valid CFData.
    let length = unsafe { CFDataGetLength(value) };
    if length < 0 {
        return Err("clipboard format had an invalid length".into());
    }
    if length == 0 {
        return Ok(Vec::new());
    }
    // SAFETY: valid CFData.
    let bytes = unsafe { CFDataGetBytePtr(value) };
    if bytes.is_null() {
        return Err("clipboard format data was unavailable".into());
    }
    // SAFETY: `length` bytes live while `value` does.
    Ok(unsafe { std::slice::from_raw_parts(bytes, length as usize) }.to_vec())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn synthesized_flavors_are_kept_only_without_an_authoritative_one() {
        assert!(should_preserve_flavor(0, true));
        assert!(!should_preserve_flavor(SYSTEM_TRANSLATED_FLAVOR, true));
        assert!(should_preserve_flavor(SYSTEM_TRANSLATED_FLAVOR, false));
    }
}
