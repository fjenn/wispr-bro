// SPDX-License-Identifier: GPL-3.0-or-later
//
// The availability cache and the codes follow hex `src/apple_intelligence.rs`
// (https://github.com/anomalyco/hex, f316831775f63106d41200578c4a752fe8c3e174;
// Copyright (c) 2026 Christopher Langton; MIT, LICENSES/MIT-hex.txt).
// Modified by Wispr Bro contributors, 2026-10-31: `wb_apple_fm_` symbols, one
// cleanup call with a request id for cancelling, language support, prewarm,
// numeric failure codes, a Rust stub where the Swift part is not built.
//
//! Safe Rust over `swift/fork_apple_fm.swift` (R5 S-12b). Blocking calls:
//! run [`complete`] on a blocking thread (`spawn_blocking`). Where the Swift
//! part is not built (`cfg(wb_apple_fm_stub)`: not Apple Silicon, no macOS 26
//! SDK, no full Xcode) every call reports [`Availability::UnsupportedSystem`].

use std::ffi::{c_char, CStr, CString};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Mutex;
use std::time::{Duration, Instant};

/// `SystemLanguageModel.default.availability`, with hex's codes.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Availability {
    Available,
    DeviceNotEligible,
    AppleIntelligenceNotEnabled,
    ModelNotReady,
    /// macOS before 26, or a build without the Swift part.
    UnsupportedSystem,
    Unknown,
}

impl Availability {
    pub fn from_code(code: i32) -> Self {
        match code {
            0 => Self::Available,
            1 => Self::DeviceNotEligible,
            2 => Self::AppleIntelligenceNotEnabled,
            3 => Self::ModelNotReady,
            4 => Self::UnsupportedSystem,
            _ => Self::Unknown,
        }
    }

    pub fn is_available(self) -> bool {
        self == Self::Available
    }
}

/// Why a request returned no text (MacParakeet's `GenerationError` mapping).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum ErrorCode {
    Timeout,
    Cancelled,
    /// Apple Intelligence off, not ready, or no Swift part.
    Unavailable,
    ContextWindow,
    Guardrail,
    Refusal,
    /// The system throttles the app (for example while it is in the background).
    RateLimited,
    ConcurrentRequests,
    UnsupportedLanguage,
    AssetsUnavailable,
    /// Guided generation could not produce the structure.
    Decoding,
    Other,
}

impl ErrorCode {
    pub fn from_code(code: i32) -> Self {
        match code {
            1 => Self::Timeout,
            2 => Self::Cancelled,
            3 => Self::Unavailable,
            4 => Self::ContextWindow,
            5 => Self::Guardrail,
            6 => Self::Refusal,
            7 => Self::RateLimited,
            8 => Self::ConcurrentRequests,
            9 => Self::UnsupportedLanguage,
            10 => Self::AssetsUnavailable,
            11 => Self::Decoding,
            _ => Self::Other,
        }
    }

    /// Stable name for logs (never any text).
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Timeout => "timeout",
            Self::Cancelled => "cancelled",
            Self::Unavailable => "unavailable",
            Self::ContextWindow => "context_window",
            Self::Guardrail => "guardrail",
            Self::Refusal => "refusal",
            Self::RateLimited => "rate_limited",
            Self::ConcurrentRequests => "concurrent_requests",
            Self::UnsupportedLanguage => "unsupported_language",
            Self::AssetsUnavailable => "assets_unavailable",
            Self::Decoding => "decoding",
            Self::Other => "other",
        }
    }
}

/// Guardrails of the session (RS-08 compares both).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum Guardrails {
    Default,
    /// `.permissiveContentTransformations`: for rewriting text the user gave.
    #[default]
    PermissiveContentTransformations,
}

/// One generation.
#[derive(Clone, Debug)]
pub struct Generation<'a> {
    pub instructions: &'a str,
    pub prompt: &'a str,
    /// 0 for no limit.
    pub max_tokens: u32,
    pub timeout: Duration,
    pub guardrails: Guardrails,
    /// Guided generation of a single `text` field instead of a plain reply.
    pub guided: bool,
}

/// A request's id, for [`cancel`].
pub fn next_request_id() -> u64 {
    static NEXT: AtomicU64 = AtomicU64::new(1);
    NEXT.fetch_add(1, Ordering::Relaxed)
}

/// `true` when the Swift part is in this build.
pub const fn is_built() -> bool {
    !cfg!(wb_apple_fm_stub)
}

const AVAILABILITY_TTL: Duration = Duration::from_secs(10);

/// The system availability, refreshed at most every ten seconds (hex).
pub fn availability() -> Availability {
    static CACHE: Mutex<Option<(Instant, Availability)>> = Mutex::new(None);
    let mut cache = CACHE.lock().unwrap_or_else(|e| e.into_inner());
    if let Some((at, a)) = *cache {
        if at.elapsed() < AVAILABILITY_TTL {
            return a;
        }
    }
    let a = Availability::from_code(ffi::availability());
    *cache = Some((Instant::now(), a));
    a
}

/// The model's context window in tokens (0 when unavailable).
pub fn context_size() -> usize {
    usize::try_from(ffi::context_size()).unwrap_or(0)
}

/// `true` when the model supports `language` (ISO 639-1, e.g. "nl").
pub fn supports_language(language: &str) -> bool {
    let Ok(c) = CString::new(language) else {
        return false;
    };
    ffi::supports_language(&c)
}

/// Loads the model and caches `instructions` for the next request with the
/// same instructions and guardrails. Returns at once.
pub fn prewarm(instructions: &str, guardrails: Guardrails) {
    if let Ok(c) = CString::new(instructions) {
        ffi::prewarm(&c, guardrails == Guardrails::PermissiveContentTransformations);
    }
}

/// Cancels request `id`, running or not yet started.
pub fn cancel(id: u64) {
    ffi::cancel(id);
}

/// Runs one generation; blocks for at most `g.timeout`.
pub fn complete(id: u64, g: &Generation<'_>) -> Result<String, ErrorCode> {
    let (Ok(instructions), Ok(prompt)) = (CString::new(g.instructions), CString::new(g.prompt))
    else {
        return Err(ErrorCode::Other);
    };
    ffi::complete(
        id,
        &instructions,
        &prompt,
        i32::try_from(g.max_tokens).unwrap_or(i32::MAX),
        u64::try_from(g.timeout.as_millis()).unwrap_or(u64::MAX),
        g.guardrails == Guardrails::PermissiveContentTransformations,
        g.guided,
    )
    .map_err(ErrorCode::from_code)
}

#[cfg(not(wb_apple_fm_stub))]
mod ffi {
    use super::*;

    extern "C" {
        fn wb_apple_fm_availability() -> i32;
        fn wb_apple_fm_context_size() -> isize;
        fn wb_apple_fm_supports_language(code: *const c_char) -> i32;
        fn wb_apple_fm_prewarm(instructions: *const c_char, permissive: bool);
        fn wb_apple_fm_cancel(id: u64);
        fn wb_apple_fm_complete(
            id: u64,
            instructions: *const c_char,
            prompt: *const c_char,
            max_tokens: i32,
            timeout_ms: u64,
            permissive: bool,
            guided: bool,
            code: *mut i32,
        ) -> *mut c_char;
        fn wb_apple_fm_free(s: *mut c_char);
    }

    pub fn availability() -> i32 {
        // SAFETY: a property read without arguments.
        unsafe { wb_apple_fm_availability() }
    }

    pub fn context_size() -> isize {
        // SAFETY: a property read without arguments.
        unsafe { wb_apple_fm_context_size() }
    }

    pub fn supports_language(code: &CStr) -> bool {
        // SAFETY: `code` is a valid C string for the duration of the call.
        unsafe { wb_apple_fm_supports_language(code.as_ptr()) == 1 }
    }

    pub fn prewarm(instructions: &CStr, permissive: bool) {
        // SAFETY: the bridge copies the string before returning.
        unsafe { wb_apple_fm_prewarm(instructions.as_ptr(), permissive) }
    }

    pub fn cancel(id: u64) {
        // SAFETY: plain value argument.
        unsafe { wb_apple_fm_cancel(id) }
    }

    pub fn complete(
        id: u64,
        instructions: &CStr,
        prompt: &CStr,
        max_tokens: i32,
        timeout_ms: u64,
        permissive: bool,
        guided: bool,
    ) -> Result<String, i32> {
        let mut code: i32 = 12;
        // SAFETY: both strings outlive the call (the bridge copies them);
        // `code` is a valid out pointer; a non-null result is a `strdup`
        // string we free with the bridge's `free`.
        unsafe {
            let p = wb_apple_fm_complete(
                id,
                instructions.as_ptr(),
                prompt.as_ptr(),
                max_tokens,
                timeout_ms,
                permissive,
                guided,
                &mut code,
            );
            if p.is_null() {
                return Err(if code == 0 { 12 } else { code });
            }
            let s = CStr::from_ptr(p).to_string_lossy().into_owned();
            wb_apple_fm_free(p);
            Ok(s)
        }
    }
}

#[cfg_attr(not(wb_apple_fm_stub), allow(dead_code))]
mod stub {
    use super::*;

    pub fn availability() -> i32 {
        4
    }
    pub fn context_size() -> isize {
        0
    }
    pub fn supports_language(_: &CStr) -> bool {
        false
    }
    pub fn prewarm(_: &CStr, _: bool) {}
    pub fn cancel(_: u64) {}
    pub fn complete(
        _: u64,
        _: &CStr,
        _: &CStr,
        _: i32,
        _: u64,
        _: bool,
        _: bool,
    ) -> Result<String, i32> {
        Err(3)
    }
}

#[cfg(wb_apple_fm_stub)]
use stub as ffi;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn availability_codes_are_hexs() {
        assert_eq!(Availability::from_code(0), Availability::Available);
        assert_eq!(Availability::from_code(1), Availability::DeviceNotEligible);
        assert_eq!(
            Availability::from_code(2),
            Availability::AppleIntelligenceNotEnabled
        );
        assert_eq!(Availability::from_code(3), Availability::ModelNotReady);
        assert_eq!(Availability::from_code(4), Availability::UnsupportedSystem);
        assert_eq!(Availability::from_code(5), Availability::Unknown);
        assert_eq!(Availability::from_code(-1), Availability::Unknown);
        assert!(Availability::Available.is_available());
        assert!(!Availability::ModelNotReady.is_available());
    }

    #[test]
    fn error_codes_match_the_swift_constants() {
        let swift = include_str!("../swift/fork_apple_fm.swift");
        for (name, code, e) in [
            ("codeTimeout", 1, ErrorCode::Timeout),
            ("codeCancelled", 2, ErrorCode::Cancelled),
            ("codeUnavailable", 3, ErrorCode::Unavailable),
            ("codeContextWindow", 4, ErrorCode::ContextWindow),
            ("codeGuardrail", 5, ErrorCode::Guardrail),
            ("codeRefusal", 6, ErrorCode::Refusal),
            ("codeRateLimited", 7, ErrorCode::RateLimited),
            ("codeConcurrent", 8, ErrorCode::ConcurrentRequests),
            ("codeUnsupportedLanguage", 9, ErrorCode::UnsupportedLanguage),
            ("codeAssetsUnavailable", 10, ErrorCode::AssetsUnavailable),
            ("codeDecoding", 11, ErrorCode::Decoding),
            ("codeOther", 12, ErrorCode::Other),
        ] {
            assert!(
                swift.contains(&format!("private let {name}: Int32 = {code}\n")),
                "{name}"
            );
            assert_eq!(ErrorCode::from_code(code), e);
        }
        assert_eq!(ErrorCode::from_code(99), ErrorCode::Other);
    }

    #[test]
    fn symbols_never_clash_with_handys_bridge() {
        let swift = include_str!("../swift/fork_apple_fm.swift");
        let handy = include_str!("../../../swift/apple_intelligence.swift");
        let names = |s: &str| -> Vec<String> {
            s.split("@_cdecl(\"")
                .skip(1)
                .filter_map(|r| r.split('"').next().map(str::to_owned))
                .collect()
        };
        let ours = names(swift);
        assert_eq!(ours.len(), 7);
        assert!(ours.iter().all(|n| n.starts_with("wb_apple_fm_")));
        for n in names(handy) {
            assert!(!ours.contains(&n), "{n}");
        }
    }

    #[test]
    fn the_stub_reports_an_unsupported_system() {
        let c = CString::new("x").unwrap();
        assert_eq!(
            Availability::from_code(stub::availability()),
            Availability::UnsupportedSystem
        );
        assert_eq!(stub::context_size(), 0);
        assert!(!stub::supports_language(&c));
        assert_eq!(
            stub::complete(1, &c, &c, 0, 10, true, true).map_err(ErrorCode::from_code),
            Err(ErrorCode::Unavailable)
        );
    }

    /// On a Mac with Apple Intelligence on: one guided and one plain request,
    /// a cancel before the start, a timeout.
    #[test]
    #[ignore = "needs macOS 26 with Apple Intelligence on"]
    fn live_smoke() {
        assert!(is_built(), "built with the stub");
        assert_eq!(availability(), Availability::Available);
        assert!(context_size() >= 4096);
        assert!(supports_language("en") && supports_language("nl"));
        let mut g = Generation {
            instructions: "Repeat the user's text with correct punctuation and capitals. Output only that text.",
            prompt: "hello there how are you",
            max_tokens: 40,
            timeout: Duration::from_secs(20),
            guardrails: Guardrails::PermissiveContentTransformations,
            guided: true,
        };
        prewarm(g.instructions, g.guardrails);
        let t = Instant::now();
        let guided = complete(next_request_id(), &g).expect("guided");
        eprintln!("guided {:?}: {guided:?}", t.elapsed());
        g.guided = false;
        let plain = complete(next_request_id(), &g).expect("plain");
        eprintln!("plain: {plain:?}");
        let id = next_request_id();
        cancel(id);
        assert_eq!(complete(id, &g), Err(ErrorCode::Cancelled));
        g.timeout = Duration::from_millis(1);
        assert_eq!(complete(next_request_id(), &g), Err(ErrorCode::Timeout));
    }

    #[test]
    fn interior_nul_is_refused_without_calling_swift() {
        let g = Generation {
            instructions: "a\0b",
            prompt: "x",
            max_tokens: 1,
            timeout: Duration::from_millis(1),
            guardrails: Guardrails::Default,
            guided: false,
        };
        assert_eq!(complete(next_request_id(), &g), Err(ErrorCode::Other));
        assert!(!supports_language("n\0l"));
    }
}
