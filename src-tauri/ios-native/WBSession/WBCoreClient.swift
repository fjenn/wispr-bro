// SPDX-License-Identifier: GPL-3.0-or-later
// A thin Swift face on the Rust core's C API (WBCore/wb_core.h).

import Foundation
import WBCore

enum WBCoreClient {
    struct Status {
        var ready = false
        var model = ""
        var downloading = false
        var progress: Double = 0
        var message = ""
    }

    enum TakeResult {
        case text(String)
        case failure(String)
    }

    private static var initialized = false

    /// Main thread, once.
    static func initializeOnce() {
        guard !initialized else { return }
        initialized = true
        let fm = FileManager.default
        let support = (try? fm.url(for: .applicationSupportDirectory, in: .userDomainMask, appropriateFor: nil, create: true))?.path ?? NSTemporaryDirectory()
        let group = fm.containerURL(forSecurityApplicationGroupIdentifier: WBBridgeConstants.appGroup)?.path ?? ""
        wb_core_init(support, group)
    }

    private static func takeString(_ pointer: UnsafeMutablePointer<CChar>?) -> String? {
        guard let pointer else { return nil }
        defer { wb_string_free(pointer) }
        return String(cString: pointer)
    }

    private static func object(_ json: String?) -> [String: Any] {
        guard let data = json?.data(using: .utf8),
              let object = try? JSONSerialization.jsonObject(with: data) as? [String: Any] else { return [:] }
        return object
    }

    static func status() -> Status {
        let json = object(takeString(wb_core_status_json()))
        return Status(
            ready: json["ready"] as? Bool ?? false,
            model: json["model"] as? String ?? "",
            downloading: json["downloading"] as? Bool ?? false,
            progress: (json["progress"] as? NSNumber)?.doubleValue ?? 0,
            message: json["message"] as? String ?? ""
        )
    }

    static func start(_ takeID: UInt64) {
        wb_take_start(takeID)
    }

    /// Audio thread. Returns the display level 0…1.
    static func feed(_ takeID: UInt64, _ samples: UnsafePointer<Float>, _ count: Int) -> Float {
        wb_take_feed(takeID, samples, count)
    }

    /// Blocking; never on the main thread.
    static func finish(_ takeID: UInt64) -> TakeResult {
        let json = object(takeString(wb_take_finish(takeID)))
        if json["ok"] as? Bool == true {
            return .text(json["text"] as? String ?? "")
        }
        let error = json["error"] as? String ?? ""
        return .failure(error.isEmpty ? "That take could not be transcribed" : error)
    }

    static func cancel(_ takeID: UInt64) {
        wb_take_cancel(takeID)
    }
}
