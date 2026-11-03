// SPDX-License-Identifier: GPL-3.0-or-later
//! The structured part of a macOS `.ips` crash report (S-27, R5 §5.7).
//!
//! [`super::crash::pickup_ips_reports`] copies our own `.ips` reports into
//! the log folder, where Copy/Save diagnostics read them. Redacting by
//! pattern was not enough (Mac wave 1, S-27.5): any body text without a
//! known prefix (an abort message, a panic quoting a transcript, a thread
//! name) went through verbatim. So the copy is rebuilt from an allow-list:
//! only the fields that locate a crash (versions, exception, termination
//! code, thread frames and the images they point into) are kept; everything
//! else, including `asi` (application-specific information), `vmSummary`,
//! file paths, user and device ids, is dropped. A body that is not the JSON
//! `.ips` format is dropped whole. The result still goes through the log
//! sanitizer.

use serde_json::{Map, Value};

/// What may be kept of a value.
enum Spec {
    /// A number or a boolean, or a short string from the system or a
    /// binary (versions, symbols, codes).
    Scalar,
    /// A short identifier-like string (thread and queue names), else dropped.
    Ident,
    Object(&'static [(&'static str, Spec)]),
    /// At most this many items.
    Array(&'static Spec, usize),
}

use Spec::{Array, Ident, Object, Scalar};

/// Longest string kept (symbols can be long; nothing else is).
const MAX_STRING: usize = 512;
/// Frames kept per thread, threads kept per report.
const MAX_FRAMES: usize = 128;
const MAX_THREADS: usize = 64;
const MAX_IMAGES: usize = 512;

const HEADER: Spec = Object(&[
    ("app_name", Scalar),
    ("app_version", Scalar),
    ("build_version", Scalar),
    ("bug_type", Scalar),
    ("bundleID", Scalar),
    ("incident_id", Scalar),
    ("name", Scalar),
    ("os_version", Scalar),
    ("platform", Scalar),
    ("slice_uuid", Scalar),
    ("timestamp", Scalar),
]);

const FRAME: Spec = Object(&[
    ("imageIndex", Scalar),
    ("imageOffset", Scalar),
    ("symbol", Scalar),
    ("symbolLocation", Scalar),
    ("inline", Scalar),
]);

const THREAD: Spec = Object(&[
    ("id", Scalar),
    ("triggered", Scalar),
    ("name", Ident),
    ("queue", Ident),
    ("frames", Array(&FRAME, MAX_FRAMES)),
]);

const IMAGE: Spec = Object(&[
    ("arch", Scalar),
    ("base", Scalar),
    ("size", Scalar),
    ("uuid", Scalar),
    ("name", Scalar),
    ("source", Scalar),
    ("CFBundleIdentifier", Scalar),
    ("CFBundleShortVersionString", Scalar),
    ("CFBundleVersion", Scalar),
]);

const BODY: Spec = Object(&[
    ("bug_type", Scalar),
    ("captureTime", Scalar),
    ("cpuType", Scalar),
    ("incident", Scalar),
    ("isCorpse", Scalar),
    ("modelCode", Scalar),
    ("procName", Scalar),
    ("procRole", Scalar),
    ("translated", Scalar),
    ("uptime", Scalar),
    ("version", Scalar),
    ("faultingThread", Scalar),
    (
        "osVersion",
        Object(&[
            ("train", Scalar),
            ("build", Scalar),
            ("releaseType", Scalar),
        ]),
    ),
    (
        "bundleInfo",
        Object(&[
            ("CFBundleIdentifier", Scalar),
            ("CFBundleShortVersionString", Scalar),
            ("CFBundleVersion", Scalar),
        ]),
    ),
    (
        "exception",
        Object(&[
            ("type", Scalar),
            ("signal", Scalar),
            ("subtype", Scalar),
            ("codes", Scalar),
            ("rawCodes", Array(&Scalar, 8)),
        ]),
    ),
    (
        "termination",
        Object(&[("code", Scalar), ("flags", Scalar), ("namespace", Scalar)]),
    ),
    ("threads", Array(&THREAD, MAX_THREADS)),
    ("lastExceptionBacktrace", Array(&FRAME, MAX_FRAMES)),
    ("usedImages", Array(&IMAGE, MAX_IMAGES)),
    (
        "sharedCache",
        Object(&[("base", Scalar), ("size", Scalar), ("uuid", Scalar)]),
    ),
]);

fn keep(value: &Value, spec: &Spec) -> Option<Value> {
    match (spec, value) {
        (Scalar, Value::Number(_) | Value::Bool(_)) => Some(value.clone()),
        (Scalar, Value::String(s)) => Some(Value::String(s.chars().take(MAX_STRING).collect())),
        (Ident, Value::String(s)) => is_ident(s).then(|| value.clone()),
        (Object(fields), Value::Object(map)) => {
            let mut out = Map::new();
            for (name, inner) in fields.iter() {
                if let Some(v) = map.get(*name).and_then(|v| keep(v, inner)) {
                    out.insert((*name).to_string(), v);
                }
            }
            Some(Value::Object(out))
        }
        (Array(inner, limit), Value::Array(items)) => Some(Value::Array(
            items
                .iter()
                .take(*limit)
                .filter_map(|v| keep(v, inner))
                .collect(),
        )),
        _ => None,
    }
}

/// Thread and queue names our code or the system set (`paste`,
/// `com.apple.main-thread`, `tokio-runtime-worker`): no spaces, short.
fn is_ident(s: &str) -> bool {
    !s.is_empty()
        && s.len() <= 64
        && s.chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '-' | '_' | ':'))
}

/// The allow-listed fields of an `.ips` report (`raw` is the whole file:
/// one JSON header line, then the JSON body), as pretty JSON. A header or
/// body that is not JSON is replaced by a note.
pub fn structured(raw: &str) -> String {
    let (header, body) = raw.split_once('\n').unwrap_or((raw, ""));
    let header = serde_json::from_str::<Value>(header.trim())
        .ok()
        .and_then(|v| keep(&v, &HEADER));
    let body = serde_json::from_str::<Value>(body.trim())
        .ok()
        .and_then(|v| keep(&v, &BODY));
    let mut out = String::new();
    match header {
        Some(h) => out.push_str(&h.to_string()),
        None => out.push_str("(header omitted: not JSON)"),
    }
    out.push('\n');
    match body.and_then(|b| serde_json::to_string_pretty(&b).ok()) {
        Some(b) => out.push_str(&b),
        None => out.push_str("(report body omitted: not the structured .ips format)"),
    }
    out.push('\n');
    out
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;

    /// A trimmed real-shaped macOS 26 `.ips` (abort in a Rust panic) with
    /// user text planted in every free-text place a report can hold it.
    pub(crate) fn realistic_ips(id: &str, marker: &str) -> String {
        let header = serde_json::json!({
            "app_name": "wispr-bro",
            "timestamp": "2026-11-03 00:21:21.00 +0100",
            "app_version": "1.0.0-alpha.1",
            "slice_uuid": "8c1d2f2e-1111-2222-3333-444455556666",
            "build_version": "1.0.0-alpha.1",
            "platform": 1,
            "bundleID": id,
            "share_with_app_devs": 0,
            "is_first_party": 0,
            "bug_type": "309",
            "os_version": "macOS 26.6.2 (25G83)",
            "roots_installed": 0,
            "name": "wispr-bro",
            "incident_id": "0E3B9C7A-AAAA-BBBB-CCCC-DDDDEEEEFFFF"
        });
        let body = serde_json::json!({
            "uptime": 90_000,
            "procRole": "Background",
            "version": 2,
            "userID": 501,
            "deployVersion": 210,
            "modelCode": "Mac17,6",
            "coalitionID": 1234,
            "osVersion": {"train": "macOS 26.6.2", "build": "25G83", "releaseType": "User"},
            "captureTime": "2026-11-03 00:21:20.9 +0100",
            "incident": "0E3B9C7A-AAAA-BBBB-CCCC-DDDDEEEEFFFF",
            "pid": 4242,
            "cpuType": "ARM-64",
            "procName": "wispr-bro",
            "procPath": "/Users/someone/Applications/Wispr Bro.app/Contents/MacOS/wispr-bro",
            "bundleInfo": {"CFBundleShortVersionString": "1.0.0-alpha.1", "CFBundleVersion": "1", "CFBundleIdentifier": id},
            "parentProc": "launchd",
            "crashReporterKey": "A1B2C3D4-device-key",
            "exception": {"codes": "0x0000000000000000, 0x0000000000000000", "rawCodes": [0, 0], "type": "EXC_CRASH", "signal": "SIGABRT", "message": format!("panic: {marker}")},
            "termination": {"flags": 0, "code": 6, "namespace": "SIGNAL", "indicator": format!("Abort trap: 6 {marker}"), "reasons": [marker]},
            "asi": {"libsystem_c.dylib": [format!("abort() called"), format!("thread 'paste' panicked: {marker}")]},
            "faultingThread": 3,
            "threads": [
                {"id": 1, "queue": "com.apple.main-thread", "frames": [{"imageOffset": 4321, "symbol": "mach_msg2_trap", "symbolLocation": 8, "imageIndex": 0}]},
                {"id": 2, "name": format!("{marker} note"), "frames": []},
                {"triggered": true, "id": 3, "name": "paste", "threadState": {"x": [{"value": 0}], "pc": {"value": 1}},
                 "frames": [{"imageOffset": 1234, "symbol": "__pthread_kill", "symbolLocation": 8, "imageIndex": 0, "sourceFile": format!("/Users/someone/{marker}.rs"), "sourceLine": 3}]}
            ],
            "usedImages": [
                {"source": "P", "arch": "arm64e", "base": 4096, "size": 65536, "uuid": "11111111-2222-3333-4444-555555555555", "path": "/usr/lib/system/libsystem_kernel.dylib", "name": "libsystem_kernel.dylib"},
                {"source": "P", "arch": "arm64", "base": 8192, "size": 65536, "uuid": "66666666-2222-3333-4444-555555555555", "path": "/Users/someone/Applications/Wispr Bro.app/Contents/MacOS/wispr-bro", "name": "wispr-bro", "CFBundleIdentifier": id}
            ],
            "sharedCache": {"base": 1, "size": 2, "uuid": "77777777-2222-3333-4444-555555555555"},
            "vmSummary": format!("ReadOnly portion of Libraries: Total=1.2G\n{marker}"),
            "legacyInfo": {"threadTriggered": {"queue": marker}},
            "logWritingSignature": "abc",
            "trialInfo": {"rollouts": [{"rolloutId": marker}]},
            "reportNotes": [marker],
            "note": marker
        });
        format!(
            "{header}\n{}\n",
            serde_json::to_string_pretty(&body).unwrap()
        )
    }

    #[test]
    fn only_allow_listed_crash_fields_survive() {
        let marker = "zebra lantern quartz crash body";
        let out = structured(&realistic_ips("com.wisprbro.s27check", marker));
        assert!(!out.contains("zebra"), "{out}");
        assert!(!out.contains("someone"), "no paths: {out}");
        assert!(!out.contains("device-key"), "{out}");
        assert!(!out.contains("\"userID\""), "{out}");
        // What locates the crash is kept.
        for kept in [
            "\"bundleID\":\"com.wisprbro.s27check\"",
            "\"app_version\":\"1.0.0-alpha.1\"",
            "EXC_CRASH",
            "SIGABRT",
            "\"faultingThread\": 3",
            "__pthread_kill",
            "\"imageOffset\": 1234",
            "\"name\": \"paste\"",
            "com.apple.main-thread",
            "66666666-2222-3333-4444-555555555555",
            "\"procName\": \"wispr-bro\"",
        ] {
            assert!(out.contains(kept), "missing {kept}: {out}");
        }
    }

    #[test]
    fn a_body_that_is_not_structured_is_dropped_whole() {
        let out = structured(
            "{\"bundleID\":\"x\"}\nThread 0 Crashed:\nzebra lantern quartz\n{\"note\":\"zebra\"}",
        );
        assert!(!out.contains("zebra"), "{out}");
        assert!(out.contains("report body omitted"));
        // The Mac wave 1 test file: a JSON body with a free-text field.
        let out =
            structured("{\"bundleID\":\"x\"}\n{\"note\":\"zebra lantern quartz crash body\"}");
        assert!(!out.contains("zebra"), "{out}");
        let out = structured("not json\n{}");
        assert!(out.contains("header omitted"));
    }

    #[test]
    fn names_with_spaces_are_not_identifiers() {
        assert!(is_ident("tokio-runtime-worker"));
        assert!(is_ident("com.apple.main-thread"));
        assert!(!is_ident("hello there"));
        assert!(!is_ident(""));
        assert!(!is_ident(&"a".repeat(65)));
    }
}
