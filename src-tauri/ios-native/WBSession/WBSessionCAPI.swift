// SPDX-License-Identifier: GPL-3.0-or-later
// C-callable helpers for Rust / JS. Declarations: wb_ios.h.

import AVFAudio
import UIKit

private let wbKeyboardBundleSuffix = ".keyboard"

private func wbOnMain(_ work: @escaping @MainActor () -> Void) {
    DispatchQueue.main.async { MainActor.assumeIsolated(work) }
}

private func wbGroupDefaults() -> UserDefaults? {
    UserDefaults(suiteName: WBBridgeConstants.appGroup)
}

enum WBSystemState {
    /// Is the Wispr Bro keyboard among the enabled keyboards?
    static var keyboardEnabled: Bool {
        guard let own = Bundle.main.bundleIdentifier else { return false }
        let keyboardID = own + wbKeyboardBundleSuffix
        if let keyboards = UserDefaults.standard.object(forKey: "AppleKeyboards") as? [String] {
            return keyboards.contains { $0 == keyboardID || $0.hasPrefix(keyboardID) }
        }
        return false
    }

    /// 0 undetermined, 1 granted, 2 denied.
    static var micPermission: Int32 {
        switch AVAudioApplication.shared.recordPermission {
        case .granted: 1
        case .denied: 2
        default: 0
        }
    }

    static func stateJSON() -> String {
        let bridge = WBBridge(defaults: wbGroupDefaults(), levelDirectory: nil)
        var object = WBSessionStateCache.shared.get()
        object["timeoutSeconds"] = bridge.timeoutSeconds.map { Int($0) } ?? -1
        object["autoReturn"] = bridge.autoReturn
        object["keyboardEnabled"] = keyboardEnabled
        object["fullAccess"] = bridge.fullAccessKnown
        object["mic"] = ["undetermined", "granted", "denied"][Int(micPermission)]
        guard let data = try? JSONSerialization.data(withJSONObject: object, options: [.sortedKeys]),
              let json = String(data: data, encoding: .utf8) else { return "{}" }
        return json
    }
}

@_cdecl("wb_ios_session_state_json")
public func wb_ios_session_state_json() -> UnsafeMutablePointer<CChar>? {
    strdup(WBSystemState.stateJSON())
}

@_cdecl("wb_ios_string_free")
public func wb_ios_string_free(_ pointer: UnsafeMutablePointer<CChar>?) {
    free(pointer)
}

@_cdecl("wb_ios_start_session")
public func wb_ios_start_session() {
    wbOnMain { WBSessionController.shared.startSession() }
}

@_cdecl("wb_ios_end_session")
public func wb_ios_end_session() {
    wbOnMain { WBSessionController.shared.endSession() }
}

@_cdecl("wb_ios_keyboard_enabled")
public func wb_ios_keyboard_enabled() -> Int32 {
    WBSystemState.keyboardEnabled ? 1 : 0
}

@_cdecl("wb_ios_full_access_known")
public func wb_ios_full_access_known() -> Int32 {
    Int32(WBBridge(defaults: wbGroupDefaults(), levelDirectory: nil).fullAccessKnown)
}

@_cdecl("wb_ios_mic_permission")
public func wb_ios_mic_permission() -> Int32 {
    WBSystemState.micPermission
}

@_cdecl("wb_ios_request_mic_permission")
public func wb_ios_request_mic_permission() {
    guard AVAudioApplication.shared.recordPermission == .undetermined else { return }
    wbOnMain {
        AVAudioApplication.requestRecordPermission { _ in }
    }
}

@_cdecl("wb_ios_open_settings")
public func wb_ios_open_settings() {
    wbOnMain {
        guard let url = URL(string: UIApplication.openSettingsURLString) else { return }
        UIApplication.shared.open(url, options: [:], completionHandler: nil)
    }
}

@_cdecl("wb_ios_defaults_get_int")
public func wb_ios_defaults_get_int(_ key: UnsafePointer<CChar>?, _ fallback: Int64) -> Int64 {
    guard let key, let defaults = wbGroupDefaults() else { return fallback }
    let name = String(cString: key)
    guard let number = defaults.object(forKey: name) as? NSNumber else { return fallback }
    return number.int64Value
}

@_cdecl("wb_ios_defaults_set_int")
public func wb_ios_defaults_set_int(_ key: UnsafePointer<CChar>?, _ value: Int64) {
    guard let key, let defaults = wbGroupDefaults() else { return }
    defaults.set(NSNumber(value: value), forKey: String(cString: key))
}
