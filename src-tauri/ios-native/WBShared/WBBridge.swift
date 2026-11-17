// SPDX-License-Identifier: GPL-3.0-or-later
// Wispr Bro for iPhone: the App Group bridge between the keyboard extension
// and the containing app.
//
// Ported from Hex (MIT, (c) 2026 Christopher Langton): ios/HEXShared/KeyboardBridge.swift.
// See ../NOTICE-hex.md. Changes: Wispr Bro naming, numeric take ids, a separate
// heartbeat key, Darwin notifications, a memory-mapped level channel, an
// exactly-once result claim and a pending start request.

import Foundation

// MARK: - Constants

enum WBBridgeConstants {
    static let appGroup = "group.com.wisprbro.ios"
    static let urlScheme = "wisprbro"

    /// Keyboard → app: a command or a start request was written.
    static let commandNotification = "com.wisprbro.ios.bridge.command"
    /// App → keyboard: the snapshot or the result changed.
    static let stateNotification = "com.wisprbro.ios.bridge.state"

    /// A heartbeat older than this means the session is gone.
    static let heartbeatLifetime: TimeInterval = 3.0
    /// Commands and start requests older than this are ignored.
    static let handoffWindow: TimeInterval = 10.0
    /// A finished text can still be inserted this long after it was produced.
    static let resultLifetime: TimeInterval = 120.0
    /// An error without a live session stays visible this long.
    static let errorLifetime: TimeInterval = 8.0

    static let levelFileName = "wb-levels.bin"
}

/// Keys in the App Group UserDefaults suite. The `wb.session.*` settings are
/// written by the app's Settings UI (Rust side) and read live here.
enum WBBridgeKey {
    static let timeoutSeconds = "wb.session.timeoutSeconds"   // Int; 0 = immediately, -1 = never, default 300
    static let autoReturn = "wb.session.autoReturn"           // Bool, default true
    static let returnAppID = "wb.session.returnAppID"         // String, optional (public return route)
    static let allowBluetoothMic = "wb.session.allowBluetoothMic" // Bool, default false

    static let command = "wb.bridge.command"
    static let consumedCommandID = "wb.bridge.consumedCommandID"
    static let snapshot = "wb.bridge.snapshot"
    static let heartbeat = "wb.bridge.heartbeat"
    static let result = "wb.bridge.result"
    static let insertedTakeID = "wb.bridge.insertedTakeID"
    static let startRequest = "wb.bridge.startRequest"
    static let fullAccess = "wb.bridge.fullAccess"            // Bool, written by the keyboard
    static let fullAccessAt = "wb.bridge.fullAccessAt"
}

// MARK: - Models

enum WBSessionState: String, Codable, Sendable {
    case noSession
    case idle
    case listening
    case transcribing
    case error
}

enum WBCommandKind: String, Codable, Sendable {
    case start
    case stop
    case cancel
}

struct WBCommand: Codable, Equatable, Sendable {
    let id: String
    let kind: WBCommandKind
    let takeID: UInt64
    let sentAt: TimeInterval
}

struct WBSnapshot: Codable, Equatable, Sendable {
    var state: WBSessionState
    var message: String?
    var takeID: UInt64?
    var takeStartedAt: TimeInterval?
    /// When the idle session ends by itself; nil = never.
    var sessionExpiresAt: TimeInterval?
    var updatedAt: TimeInterval

    static let noSession = WBSnapshot(state: .noSession, updatedAt: 0)

    init(
        state: WBSessionState,
        message: String? = nil,
        takeID: UInt64? = nil,
        takeStartedAt: TimeInterval? = nil,
        sessionExpiresAt: TimeInterval? = nil,
        updatedAt: TimeInterval
    ) {
        self.state = state
        self.message = message
        self.takeID = takeID
        self.takeStartedAt = takeStartedAt
        self.sessionExpiresAt = sessionExpiresAt
        self.updatedAt = updatedAt
    }
}

struct WBResult: Codable, Equatable, Sendable {
    let takeID: UInt64
    let text: String
    let createdAt: TimeInterval
}

/// "Please start a session", written by the keyboard right before it opens the app.
struct WBStartRequest: Codable, Equatable, Sendable {
    let id: String
    let requestedAt: TimeInterval
    /// When set, the app starts this take as soon as the session is up.
    let takeID: UInt64?
    /// The app the keyboard was typing in, when the keyboard could tell.
    let hostBundleID: String?

    init(id: String = UUID().uuidString, requestedAt: TimeInterval, takeID: UInt64?, hostBundleID: String? = nil) {
        self.id = id
        self.requestedAt = requestedAt
        self.takeID = takeID
        self.hostBundleID = hostBundleID
    }

    /// `wisprbro://session/start?req=…&ts=…&take=…&host=…`
    var url: URL {
        var components = URLComponents()
        components.scheme = WBBridgeConstants.urlScheme
        components.host = "session"
        components.path = "/start"
        var items = [
            URLQueryItem(name: "req", value: id),
            URLQueryItem(name: "ts", value: String(format: "%.3f", requestedAt)),
        ]
        if let takeID { items.append(URLQueryItem(name: "take", value: String(takeID))) }
        if let hostBundleID, !hostBundleID.isEmpty { items.append(URLQueryItem(name: "host", value: hostBundleID)) }
        components.queryItems = items
        return components.url!
    }

    /// Parses a session start link. A bare `wisprbro://session/start` is valid.
    init?(url: URL, now: TimeInterval = Date().timeIntervalSince1970) {
        guard url.scheme?.lowercased() == WBBridgeConstants.urlScheme,
              url.host == "session", url.path == "/start" else { return nil }
        let items = URLComponents(url: url, resolvingAgainstBaseURL: false)?.queryItems ?? []
        func value(_ name: String) -> String? {
            guard let v = items.first(where: { $0.name == name })?.value, !v.isEmpty else { return nil }
            return v
        }
        self.id = value("req") ?? UUID().uuidString
        self.requestedAt = value("ts").flatMap(TimeInterval.init) ?? now
        self.takeID = value("take").flatMap { UInt64($0) }
        self.hostBundleID = value("host")
    }

    func isExpired(at now: TimeInterval) -> Bool {
        now - requestedAt > WBBridgeConstants.handoffWindow
    }
}

// MARK: - Take ids

enum WBTakeID {
    /// A fresh non-zero id (0 means "no take" in the C API).
    static func make() -> UInt64 {
        UInt64.random(in: 1...UInt64(Int64.max))
    }
}

// MARK: - Bridge

/// Both processes create their own `WBBridge`; all state lives in the App Group.
final class WBBridge {
    let defaults: UserDefaults?
    let levels: WBLevelChannel?
    private let encoder = JSONEncoder()
    private let decoder = JSONDecoder()

    /// The real bridge. `nil` storage (keyboard without Full Access) makes every call a no-op.
    convenience init() {
        let container = FileManager.default.containerURL(
            forSecurityApplicationGroupIdentifier: WBBridgeConstants.appGroup
        )
        self.init(
            defaults: UserDefaults(suiteName: WBBridgeConstants.appGroup),
            levelDirectory: container
        )
    }

    init(defaults: UserDefaults?, levelDirectory: URL?) {
        self.defaults = defaults
        self.levels = levelDirectory.flatMap {
            WBLevelChannel(fileURL: $0.appendingPathComponent(WBBridgeConstants.levelFileName))
        }
    }

    private func now() -> TimeInterval { Date().timeIntervalSince1970 }

    private func write<T: Encodable>(_ value: T, _ key: String) {
        guard let data = try? encoder.encode(value) else { return }
        defaults?.set(data, forKey: key)
    }

    private func read<T: Decodable>(_ type: T.Type, _ key: String) -> T? {
        guard let data = defaults?.data(forKey: key) else { return nil }
        return try? decoder.decode(type, from: data)
    }

    // MARK: Commands (keyboard → app)

    @discardableResult
    func send(_ kind: WBCommandKind, takeID: UInt64, at time: TimeInterval? = nil) -> WBCommand {
        let command = WBCommand(id: UUID().uuidString, kind: kind, takeID: takeID, sentAt: time ?? now())
        write(command, WBBridgeKey.command)
        WBDarwin.post(WBBridgeConstants.commandNotification)
        return command
    }

    func latestCommand() -> WBCommand? {
        read(WBCommand.self, WBBridgeKey.command)
    }

    /// The app's side: returns each command once, and never a stale one.
    func consumeCommand(at time: TimeInterval? = nil) -> WBCommand? {
        guard let command = latestCommand(),
              defaults?.string(forKey: WBBridgeKey.consumedCommandID) != command.id else { return nil }
        defaults?.set(command.id, forKey: WBBridgeKey.consumedCommandID)
        guard (time ?? now()) - command.sentAt <= WBBridgeConstants.handoffWindow else { return nil }
        return command
    }

    // MARK: Start request (keyboard → app, across an app launch)

    func requestSessionStart(_ request: WBStartRequest) {
        write(request, WBBridgeKey.startRequest)
        WBDarwin.post(WBBridgeConstants.commandNotification)
    }

    /// The app's side: returns a fresh pending request once and clears it.
    func takePendingStartRequest(at time: TimeInterval? = nil) -> WBStartRequest? {
        guard let request = read(WBStartRequest.self, WBBridgeKey.startRequest) else { return nil }
        defaults?.removeObject(forKey: WBBridgeKey.startRequest)
        return request.isExpired(at: time ?? now()) ? nil : request
    }

    // MARK: Snapshot + heartbeat (app → keyboard)

    func publish(_ snapshot: WBSnapshot) {
        write(snapshot, WBBridgeKey.snapshot)
        WBDarwin.post(WBBridgeConstants.stateNotification)
    }

    func rawSnapshot() -> WBSnapshot {
        read(WBSnapshot.self, WBBridgeKey.snapshot) ?? .noSession
    }

    /// The app calls this about once a second while the session is alive.
    func beat(at time: TimeInterval? = nil) {
        defaults?.set(time ?? now(), forKey: WBBridgeKey.heartbeat)
    }

    func clearHeartbeat() {
        defaults?.set(0.0, forKey: WBBridgeKey.heartbeat)
    }

    func isSessionAlive(at time: TimeInterval? = nil) -> Bool {
        let beat = defaults?.double(forKey: WBBridgeKey.heartbeat) ?? 0
        let t = time ?? now()
        return beat > 0 && t - beat < WBBridgeConstants.heartbeatLifetime && beat - t < WBBridgeConstants.heartbeatLifetime
    }

    /// What the keyboard should show: the snapshot while the heartbeat is
    /// fresh, a recent error even without a session, otherwise "no session".
    func snapshot(at time: TimeInterval? = nil) -> WBSnapshot {
        let t = time ?? now()
        let snapshot = rawSnapshot()
        if isSessionAlive(at: t) {
            return snapshot
        }
        if snapshot.state == .error, t - snapshot.updatedAt < WBBridgeConstants.errorLifetime {
            return snapshot
        }
        return WBSnapshot(state: .noSession, updatedAt: snapshot.updatedAt)
    }

    // MARK: Result hand-off (app → keyboard, exactly once)

    func publishResult(takeID: UInt64, text: String, at time: TimeInterval? = nil) {
        write(WBResult(takeID: takeID, text: text, createdAt: time ?? now()), WBBridgeKey.result)
        WBDarwin.post(WBBridgeConstants.stateNotification)
    }

    func pendingResult(at time: TimeInterval? = nil) -> WBResult? {
        guard let result = read(WBResult.self, WBBridgeKey.result), !result.text.isEmpty,
              (time ?? now()) - result.createdAt <= WBBridgeConstants.resultLifetime,
              defaults?.string(forKey: WBBridgeKey.insertedTakeID) != String(result.takeID) else { return nil }
        return result
    }

    /// The keyboard's side: returns a finished text once. The take id is
    /// marked as inserted before the text is returned, so a second call (or a
    /// second keyboard instance) gets nothing.
    func claimResult(at time: TimeInterval? = nil) -> WBResult? {
        guard let result = pendingResult(at: time) else { return nil }
        defaults?.set(String(result.takeID), forKey: WBBridgeKey.insertedTakeID)
        return result
    }

    // MARK: Full Access (keyboard → app)

    func noteFullAccess(_ granted: Bool, at time: TimeInterval? = nil) {
        defaults?.set(granted, forKey: WBBridgeKey.fullAccess)
        defaults?.set(time ?? now(), forKey: WBBridgeKey.fullAccessAt)
    }

    /// 1 = on, 0 = off, -1 = the keyboard has not reported yet.
    var fullAccessKnown: Int {
        guard let defaults, defaults.object(forKey: WBBridgeKey.fullAccess) != nil else { return -1 }
        return defaults.bool(forKey: WBBridgeKey.fullAccess) ? 1 : 0
    }

    // MARK: Settings

    /// Idle timeout in seconds: 0 = end right after a take, nil = never.
    var timeoutSeconds: TimeInterval? {
        guard let defaults, defaults.object(forKey: WBBridgeKey.timeoutSeconds) != nil else { return 300 }
        let value = defaults.integer(forKey: WBBridgeKey.timeoutSeconds)
        return value < 0 ? nil : TimeInterval(value)
    }

    var autoReturn: Bool {
        guard let defaults, defaults.object(forKey: WBBridgeKey.autoReturn) != nil else { return true }
        return defaults.bool(forKey: WBBridgeKey.autoReturn)
    }

    var returnAppID: String? {
        let value = defaults?.string(forKey: WBBridgeKey.returnAppID)
        return value?.isEmpty == false ? value : nil
    }

    var allowBluetoothMic: Bool {
        defaults?.bool(forKey: WBBridgeKey.allowBluetoothMic) ?? false
    }
}

// MARK: - Level channel

/// A 16-byte memory-mapped file in the App Group container: the app writes
/// the current display level (0…1) about 30 times a second, the keyboard reads
/// it every frame. No syscalls and no preference churn on either side.
///
/// Layout: UInt32 sequence | Float32 level | Float64 unix time.
final class WBLevelChannel: @unchecked Sendable {
    private static let size = 16
    private let pointer: UnsafeMutableRawPointer
    private var sequence: UInt32 = 0

    init?(fileURL: URL) {
        let fd = open(fileURL.path, O_RDWR | O_CREAT, 0o644)
        guard fd >= 0 else { return nil }
        defer { close(fd) }
        guard ftruncate(fd, off_t(Self.size)) == 0,
              let mapped = mmap(nil, Self.size, PROT_READ | PROT_WRITE, MAP_SHARED, fd, 0),
              mapped != MAP_FAILED else { return nil }
        pointer = mapped
        sequence = pointer.load(as: UInt32.self)
    }

    deinit {
        munmap(pointer, Self.size)
    }

    func write(level: Float, at time: TimeInterval = Date().timeIntervalSince1970) {
        sequence &+= 1
        pointer.storeBytes(of: max(0, min(1, level)), toByteOffset: 4, as: Float32.self)
        pointer.storeBytes(of: time, toByteOffset: 8, as: Float64.self)
        pointer.storeBytes(of: sequence, toByteOffset: 0, as: UInt32.self)
    }

    func read() -> (sequence: UInt32, level: Float, time: TimeInterval) {
        (
            pointer.load(as: UInt32.self),
            pointer.load(fromByteOffset: 4, as: Float32.self),
            pointer.load(fromByteOffset: 8, as: Float64.self)
        )
    }

    /// The level to draw now: 0 when the writer has gone quiet.
    func currentLevel(at time: TimeInterval = Date().timeIntervalSince1970) -> Float {
        let sample = read()
        return abs(time - sample.time) < 0.5 ? sample.level : 0
    }
}

// MARK: - Darwin notifications

enum WBDarwin {
    static func post(_ name: String) {
        CFNotificationCenterPostNotification(
            CFNotificationCenterGetDarwinNotifyCenter(),
            CFNotificationName(name as CFString),
            nil, nil, true
        )
    }
}

/// Calls `handler` on the main queue whenever the named Darwin notification fires.
final class WBDarwinObserver {
    private let name: String
    private let handler: () -> Void

    init(_ name: String, handler: @escaping () -> Void) {
        self.name = name
        self.handler = handler
        CFNotificationCenterAddObserver(
            CFNotificationCenterGetDarwinNotifyCenter(),
            Unmanaged.passUnretained(self).toOpaque(),
            { _, observer, _, _, _ in
                guard let observer else { return }
                let me = Unmanaged<WBDarwinObserver>.fromOpaque(observer).takeUnretainedValue()
                let handler = me.handler
                DispatchQueue.main.async(execute: handler)
            },
            name as CFString,
            nil,
            .deliverImmediately
        )
    }

    deinit {
        CFNotificationCenterRemoveEveryObserver(
            CFNotificationCenterGetDarwinNotifyCenter(),
            Unmanaged.passUnretained(self).toOpaque()
        )
    }
}
