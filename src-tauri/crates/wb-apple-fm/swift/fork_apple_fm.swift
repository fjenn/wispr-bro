// SPDX-License-Identifier: GPL-3.0-or-later
//
// Wispr Bro's Apple Foundation Models bridge (R5 S-12b). Ported from:
//
// - hex, https://github.com/anomalyco/hex, f316831775f63106d41200578c4a752fe8c3e174,
//   src/apple_intelligence.swift (`blocking(timeout:)` with `task.cancel()`,
//   the availability codes, `contextSize`, permissive guardrails,
//   `GenerationOptions`). Copyright (c) 2026 Christopher Langton. MIT
//   (LICENSES/MIT-hex.txt).
// - MacParakeet, https://github.com/moona3k/macparakeet, 3ec699152f78df8adccba0cbff5cb968c9790947,
//   Sources/MacParakeetCore/Services/LLM/FoundationModelsAppleIntelligenceGenerator.swift
//   (`AppleIntelligenceErrorMapper`: which `GenerationError` cases mean what).
//   Copyright (C) 2026 Daniel Moon. GPL-3.0.
// - TypeWhisper, https://github.com/TypeWhisper/typewhisper-mac, 4620bb59a804a2aeeaf5607cf10aa437c7589504,
//   TypeWhisper/Services/LLM/FoundationModelsProvider.swift (`prewarm()`
//   keeps the prewarmed session alive; permissive content transformations).
//   Copyright (c) 2026 TypeWhisper. GPL-3.0-or-later.
//
// Modified by Wispr Bro contributors, 2026-10-31: one cleanup call instead of
// hex's meeting notes; every symbol prefixed `wb_apple_fm_` so it never
// clashes with Handy's bridge; request ids so Rust can cancel a request it
// gave up on (Esc, deadline); the prewarmed session is used by the next
// request with the same instructions (TypeWhisper drops it); errors are
// numeric codes only, never the model's or the transcript's text; guided
// generation of `{text}` or a plain reply; greedy sampling; macOS 27's
// `LanguageModelError` mapped as well.

import Foundation
#if canImport(FoundationModels)
import FoundationModels
#endif

// Result codes (`wb-apple-fm` `ErrorCode`). Keep in sync with src/lib.rs.
private let codeOk: Int32 = 0
private let codeTimeout: Int32 = 1
private let codeCancelled: Int32 = 2
private let codeUnavailable: Int32 = 3
private let codeContextWindow: Int32 = 4
private let codeGuardrail: Int32 = 5
private let codeRefusal: Int32 = 6
private let codeRateLimited: Int32 = 7
private let codeConcurrent: Int32 = 8
private let codeUnsupportedLanguage: Int32 = 9
private let codeAssetsUnavailable: Int32 = 10
private let codeDecoding: Int32 = 11
private let codeOther: Int32 = 12

private struct BridgeFailure: Error {
    let code: Int32
}

// MARK: - Request registry (cancel from Rust)

private final class Registry: @unchecked Sendable {
    private let lock = NSLock()
    private var tasks: [UInt64: Task<Void, Never>] = [:]
    private var cancelledEarly: Set<UInt64> = []

    /// Registers `task`; cancels it at once if Rust already gave up on `id`.
    func register(_ id: UInt64, _ task: Task<Void, Never>) {
        let cancel: Bool = lock.withLock {
            if cancelledEarly.remove(id) != nil {
                return true
            }
            tasks[id] = task
            return false
        }
        if cancel {
            task.cancel()
        }
    }

    func finish(_ id: UInt64) {
        lock.withLock {
            tasks[id] = nil
            cancelledEarly.remove(id)
        }
    }

    func cancel(_ id: UInt64) {
        let task: Task<Void, Never>? = lock.withLock {
            if let t = tasks.removeValue(forKey: id) {
                return t
            }
            // Not registered yet (or already done): remember a few ids.
            if cancelledEarly.count > 64 {
                cancelledEarly.removeAll()
            }
            cancelledEarly.insert(id)
            return nil
        }
        task?.cancel()
    }
}

private let registry = Registry()

private final class Box: @unchecked Sendable {
    private let lock = NSLock()
    private var value: Result<String, BridgeFailure>?

    func store(_ v: Result<String, BridgeFailure>) {
        lock.withLock { value = v }
    }

    func take() -> Result<String, BridgeFailure>? {
        lock.withLock { value }
    }
}

#if canImport(FoundationModels)

@available(macOS 26.0, *)
@Generable
private struct CleanedText {
    let text: String
}

@available(macOS 26.0, *)
private func model(permissive: Bool) -> SystemLanguageModel {
    SystemLanguageModel(
        useCase: .general,
        guardrails: permissive ? .permissiveContentTransformations : .default
    )
}

// MARK: - Prewarm (TypeWhisper)

@available(macOS 26.0, *)
private final class Prewarmed: @unchecked Sendable {
    private let lock = NSLock()
    private var session: LanguageModelSession?
    private var key: String?

    func store(_ session: LanguageModelSession, key: String) {
        lock.withLock {
            self.session = session
            self.key = key
        }
    }

    /// The prewarmed session for these instructions, once.
    func take(key: String) -> LanguageModelSession? {
        lock.withLock {
            guard self.key == key, let s = session else { return nil }
            session = nil
            self.key = nil
            return s
        }
    }
}

@available(macOS 26.0, *)
private let prewarmed = Prewarmed()

private func sessionKey(_ instructions: String, _ permissive: Bool) -> String {
    (permissive ? "p:" : "d:") + instructions
}

// MARK: - Errors (MacParakeet's mapping)

@available(macOS 26.0, *)
private func failureCode(_ error: Error) -> Int32 {
    if error is CancellationError {
        return codeCancelled
    }
    if let e = error as? LanguageModelSession.GenerationError {
        switch e {
        case .exceededContextWindowSize:
            return codeContextWindow
        case .assetsUnavailable:
            return codeAssetsUnavailable
        case .guardrailViolation:
            return codeGuardrail
        case .refusal:
            return codeRefusal
        case .rateLimited:
            return codeRateLimited
        case .concurrentRequests:
            return codeConcurrent
        case .unsupportedLanguageOrLocale:
            return codeUnsupportedLanguage
        case .unsupportedGuide, .decodingFailure:
            return codeDecoding
        @unknown default:
            return codeOther
        }
    }
    #if compiler(>=6.4)
    if #available(macOS 27.0, *) {
        if let e = error as? LanguageModelError {
            switch e {
            case .contextSizeExceeded:
                return codeContextWindow
            case .rateLimited:
                return codeRateLimited
            case .guardrailViolation:
                return codeGuardrail
            case .refusal:
                return codeRefusal
            case .unsupportedLanguageOrLocale:
                return codeUnsupportedLanguage
            case .timeout:
                return codeTimeout
            default:
                return codeOther
            }
        }
    }
    #endif
    return codeOther
}

@available(macOS 26.0, *)
private func generate(
    instructions: String,
    prompt: String,
    maxTokens: Int,
    permissive: Bool,
    guided: Bool
) async throws -> String {
    let session =
        prewarmed.take(key: sessionKey(instructions, permissive))
        ?? LanguageModelSession(model: model(permissive: permissive), instructions: instructions)
    let options = GenerationOptions(
        samplingMode: .greedy,
        maximumResponseTokens: maxTokens > 0 ? maxTokens : nil
    )
    if guided {
        return try await session.respond(
            to: prompt,
            generating: CleanedText.self,
            options: options
        ).content.text
    }
    return try await session.respond(to: prompt, options: options).content
}

#endif

private func availabilityCode() -> Int32 {
    #if canImport(FoundationModels)
    guard #available(macOS 26.0, *) else { return 4 }
    switch SystemLanguageModel.default.availability {
    case .available:
        return 0
    case .unavailable(.deviceNotEligible):
        return 1
    case .unavailable(.appleIntelligenceNotEnabled):
        return 2
    case .unavailable(.modelNotReady):
        return 3
    case .unavailable:
        return 5
    }
    #else
    return 4
    #endif
}

// MARK: - C entry points

/// 0 available, 1 device not eligible, 2 Apple Intelligence off, 3 model not
/// ready, 4 macOS before 26, 5 unknown reason.
@_cdecl("wb_apple_fm_availability")
public func wbAppleFmAvailability() -> Int32 {
    availabilityCode()
}

/// The model's context window in tokens, 0 when unavailable.
@_cdecl("wb_apple_fm_context_size")
public func wbAppleFmContextSize() -> Int {
    #if canImport(FoundationModels)
    guard #available(macOS 26.0, *) else { return 0 }
    return SystemLanguageModel.default.contextSize
    #else
    return 0
    #endif
}

/// 1 when the model supports the language `code` (BCP 47, e.g. "nl"), else 0.
@_cdecl("wb_apple_fm_supports_language")
public func wbAppleFmSupportsLanguage(_ code: UnsafePointer<CChar>?) -> Int32 {
    #if canImport(FoundationModels)
    guard #available(macOS 26.0, *), let code else { return 0 }
    let wanted = Locale.Language(identifier: String(cString: code))
    let ok = SystemLanguageModel.default.supportedLanguages.contains {
        $0.languageCode == wanted.languageCode
    }
    return ok ? 1 : 0
    #else
    return 0
    #endif
}

/// Loads the model and caches `instructions` in a session the next request
/// with the same instructions uses (TypeWhisper: the system unloads the model
/// seconds after a request).
@_cdecl("wb_apple_fm_prewarm")
public func wbAppleFmPrewarm(_ instructions: UnsafePointer<CChar>?, _ permissive: Bool) {
    #if canImport(FoundationModels)
    guard #available(macOS 26.0, *), let instructions, availabilityCode() == 0 else { return }
    let text = String(cString: instructions)
    let session = LanguageModelSession(model: model(permissive: permissive), instructions: text)
    session.prewarm()
    prewarmed.store(session, key: sessionKey(text, permissive))
    #endif
}

/// Cancels request `id` (running, or not started yet).
@_cdecl("wb_apple_fm_cancel")
public func wbAppleFmCancel(_ id: UInt64) {
    registry.cancel(id)
}

/// One generation. Blocks the calling thread for at most `timeoutMs`; on
/// timeout the request is cancelled. Returns the text (free it with
/// `wb_apple_fm_free`) and writes 0 to `code`, or returns nil and writes
/// the failure code.
@_cdecl("wb_apple_fm_complete")
public func wbAppleFmComplete(
    _ id: UInt64,
    _ instructions: UnsafePointer<CChar>?,
    _ prompt: UnsafePointer<CChar>?,
    _ maxTokens: Int32,
    _ timeoutMs: UInt64,
    _ permissive: Bool,
    _ guided: Bool,
    _ code: UnsafeMutablePointer<Int32>?
) -> UnsafeMutablePointer<CChar>? {
    func fail(_ c: Int32) -> UnsafeMutablePointer<CChar>? {
        code?.pointee = c
        return nil
    }
    #if canImport(FoundationModels)
    guard #available(macOS 26.0, *), let instructions, let prompt else {
        return fail(codeUnavailable)
    }
    guard availabilityCode() == 0 else { return fail(codeUnavailable) }
    let instructionText = String(cString: instructions)
    let promptText = String(cString: prompt)
    let tokens = Int(maxTokens)
    let semaphore = DispatchSemaphore(value: 0)
    let box = Box()
    let task = Task.detached(priority: .userInitiated) {
        do {
            let text = try await generate(
                instructions: instructionText,
                prompt: promptText,
                maxTokens: tokens,
                permissive: permissive,
                guided: guided
            )
            try Task.checkCancellation()
            box.store(.success(text))
        } catch {
            box.store(.failure(BridgeFailure(code: failureCode(error))))
        }
        semaphore.signal()
    }
    registry.register(id, task)
    defer { registry.finish(id) }
    guard semaphore.wait(timeout: .now() + .milliseconds(Int(min(timeoutMs, 600_000)))) == .success
    else {
        task.cancel()
        return fail(codeTimeout)
    }
    switch box.take() {
    case .success(let text):
        code?.pointee = codeOk
        return strdup(text)
    case .failure(let f):
        return fail(f.code)
    case nil:
        return fail(codeOther)
    }
    #else
    return fail(codeUnavailable)
    #endif
}

@_cdecl("wb_apple_fm_free")
public func wbAppleFmFree(_ s: UnsafeMutablePointer<CChar>?) {
    free(s)
}
