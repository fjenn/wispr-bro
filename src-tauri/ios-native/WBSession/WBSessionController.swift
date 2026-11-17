// SPDX-License-Identifier: GPL-3.0-or-later
// The dictation session inside the containing app: a warm microphone, takes
// run through the Rust core, state published to the keyboard, an idle timeout,
// a Live Activity, and the trip back to the app the user was typing in.
//
// Structure after Hex (MIT, (c) 2026 Christopher Langton):
// ios/HEXMobile/DictationController.swift (warm session loop, command
// handling, interruption rules, auto-return with a swipe fallback).
// See ../NOTICE-hex.md.

import AVFAudio
import UIKit

@MainActor
@objc(WBSessionController)
final class WBSessionController: NSObject {
    @objc static let shared = WBSessionController()

    enum Phase: String {
        case off, idle, listening, transcribing
    }

    /// A take shorter than this is dropped when the audio is interrupted.
    private static let minTake: TimeInterval = 0.3
    /// A forgotten take is finished after this long.
    private static let maxTake: TimeInterval = 15 * 60
    /// With the timeout set to "Immediately", how long a new session waits for its first take.
    private static let immediateGrace: TimeInterval = 30

    let bridge = WBBridge()
    private let audio = WBAudioEngine()
    private let liveActivity = WBLiveActivityCoordinator()
    private let hint = WBSwipeBackHintPresenter()
    private let workQueue = DispatchQueue(label: "com.wisprbro.ios.take", qos: .userInitiated)
    private let levelQueue = DispatchQueue(label: "com.wisprbro.ios.levels", qos: .userInteractive)

    private(set) var phase: Phase = .off
    private var bootstrapped = false
    private var starting = false
    private var interrupted = false
    private var endAfterTake = false
    private var activeTake: UInt64?
    private var takeGeneration = 0
    private var takeStartedAt: TimeInterval = 0
    private var takesDone = 0
    private var lastActivity: TimeInterval = 0
    private var errorMessage: String?
    private var errorUntil: TimeInterval = 0
    private var lastRequestID: String?
    private var lastPublishedExpiry: TimeInterval?
    private var lastBeat: TimeInterval = 0
    private var tickTimer: Timer?
    private var levelTimer: DispatchSourceTimer?
    private var restartPending = false
    private var commandObserver: WBDarwinObserver?

    private func now() -> TimeInterval { Date().timeIntervalSince1970 }

    // MARK: - Bootstrap

    /// Called by WBSessionBootstrap (+load → didFinishLaunching). Idempotent.
    @objc(bootstrapWithLaunchURL:)
    static func bootstrap(launchURL: URL?) {
        shared.bootstrap(launchURL: launchURL)
    }

    func bootstrap(launchURL: URL? = nil) {
        guard !bootstrapped else { return }
        bootstrapped = true

        WBCoreClient.initializeOnce()
        if bridge.autoReturn { WBHostAppObserver.shared.start() }

        // A fresh process has no session, whatever an earlier one left behind.
        bridge.clearHeartbeat()
        bridge.publish(.init(state: .noSession, updatedAt: now()))
        liveActivity.end()
        refreshStateCache()

        audio.onEvent = { [weak self] event in
            MainActor.assumeIsolated { self?.handleAudio(event) }
        }
        commandObserver = WBDarwinObserver(WBBridgeConstants.commandNotification) { [weak self] in
            MainActor.assumeIsolated { self?.bridgeSignalled() }
        }
        let center = NotificationCenter.default
        center.addObserver(forName: UIApplication.didBecomeActiveNotification, object: nil, queue: .main) { [weak self] _ in
            MainActor.assumeIsolated { self?.checkPendingStart() }
        }
        center.addObserver(forName: UIApplication.didEnterBackgroundNotification, object: nil, queue: .main) { [weak self] _ in
            MainActor.assumeIsolated { self?.hint.hide() }
        }
        center.addObserver(forName: UIApplication.willTerminateNotification, object: nil, queue: .main) { [weak self] _ in
            MainActor.assumeIsolated { self?.tearDown() }
        }
        Task {
            await WBActivityCommandBus.shared.register { [weak self] command in
                switch command {
                case .endSession: self?.endSession()
                }
            }
        }

        if let launchURL { handleOpen(url: launchURL) }
    }

    // MARK: - Entry points

    /// Call from the app's URL handler. Returns true for Wispr Bro session links.
    @objc @discardableResult
    func handleOpen(url: URL) -> Bool {
        guard url.scheme?.lowercased() == WBBridgeConstants.urlScheme else { return false }
        bootstrap()
        guard let request = WBStartRequest(url: url, now: now()) else { return false }
        // The same request also sits in the bridge; drop that copy.
        _ = bridge.takePendingStartRequest()
        startSession(request: request, fromKeyboard: true)
        return true
    }

    /// Starts a session from inside the app.
    @objc func startSession() {
        bootstrap()
        startSession(request: nil, fromKeyboard: false)
    }

    /// A take started by the app itself (a Scratchpad note's Record button):
    /// the same take as one from the keyboard, but its text goes back to the
    /// app (`WBAppTake`) instead of to the keyboard. Starts the session
    /// first when none is running.
    func startAppTake(_ takeID: UInt64) {
        bootstrap()
        guard WBAppTake.shared.owns(takeID) else { return }
        switch phase {
        case .off:
            startSession(request: nil, fromKeyboard: false, appTake: takeID)
        case .idle:
            touch()
            startTake(takeID)
            if activeTake != takeID {
                WBAppTake.shared.fail(takeID, errorMessage ?? "The microphone could not start. Try again.")
            }
            publish()
        case .listening, .transcribing:
            WBAppTake.shared.fail(takeID, "Another dictation is still running")
        }
    }

    func stopAppTake(_ takeID: UInt64) {
        guard phase == .listening, activeTake == takeID else { return }
        finishTake()
    }

    func cancelAppTake(_ takeID: UInt64) {
        guard activeTake == takeID else {
            WBAppTake.shared.cancel(takeID)
            return
        }
        cancelTake()
    }

    /// Ends the session. A take that is listening is finished and delivered first.
    @objc func endSession() {
        switch phase {
        case .off:
            return
        case .listening:
            endAfterTake = true
            finishTake()
        case .transcribing:
            endAfterTake = true
        case .idle:
            tearDown()
        }
    }

    private func bridgeSignalled() {
        if UIApplication.shared.applicationState != .background {
            checkPendingStart()
        }
        consumeCommands()
    }

    private func checkPendingStart() {
        guard let request = bridge.takePendingStartRequest(at: now()) else { return }
        startSession(request: request, fromKeyboard: true)
    }

    // MARK: - Session

    private func startSession(request: WBStartRequest?, fromKeyboard: Bool, appTake: UInt64? = nil) {
        if let id = request?.id {
            guard id != lastRequestID else { return }
            lastRequestID = id
        }
        // A request that arrives while this app is already in front came from
        // the keyboard inside this app: there is nowhere to go back to.
        let fromKeyboard = fromKeyboard && UIApplication.shared.applicationState != .active
        let wantedTake = appTake ?? request.flatMap { $0.isExpired(at: now()) ? nil : $0.takeID }

        if phase != .off {
            touch()
            if let wantedTake, phase == .idle { startTake(wantedTake) }
            publish()
            if fromKeyboard { returnToHost(request) }
            return
        }
        guard !starting else {
            if let appTake { WBAppTake.shared.fail(appTake, "Wispr Bro is still starting. Try again.") }
            return
        }
        starting = true

        requestMicrophone { [weak self] granted in
            guard let self else { return }
            self.starting = false
            guard granted else {
                self.failStart("Microphone access is off. Turn it on in Settings.")
                if let appTake { WBAppTake.shared.fail(appTake, "Microphone access is off. Turn it on in Settings.") }
                return
            }
            do {
                try self.audio.start(allowBluetooth: self.bridge.allowBluetoothMic)
            } catch {
                self.audio.shutDown()
                self.failStart("The microphone could not start. Try again.")
                if let appTake { WBAppTake.shared.fail(appTake, "The microphone could not start. Try again.") }
                return
            }
            self.phase = .idle
            self.interrupted = false
            self.endAfterTake = false
            self.takesDone = 0
            self.errorMessage = nil
            self.touch()
            self.startTicking()
            self.bridge.beat()
            self.liveActivity.show(.ready)
            if let wantedTake { self.startTake(wantedTake) }
            if let appTake, self.activeTake != appTake {
                WBAppTake.shared.fail(appTake, self.errorMessage ?? "The microphone could not start. Try again.")
            }
            self.publish()
            if fromKeyboard { self.returnToHost(request) }
        }
    }

    private func failStart(_ message: String) {
        bridge.clearHeartbeat()
        bridge.publish(.init(state: .error, message: message, updatedAt: now()))
        refreshStateCache(message: message)
    }

    private func tearDown() {
        guard phase != .off else { return }
        if let take = activeTake {
            takeGeneration += 1
            audio.endTake()
            WBCoreClient.cancel(take)
            WBAppTake.shared.cancel(take)
            activeTake = nil
        }
        stopLevelPump()
        tickTimer?.invalidate()
        tickTimer = nil
        audio.shutDown()
        phase = .off
        endAfterTake = false
        errorMessage = nil
        hint.hide()
        liveActivity.end()
        bridge.clearHeartbeat()
        publish()
    }

    private func touch() {
        lastActivity = now()
    }

    /// When the idle session ends by itself; nil = never. Read live, so a
    /// changed setting applies to the running session.
    private var expiresAt: TimeInterval? {
        guard let timeout = bridge.timeoutSeconds else { return nil }
        if timeout <= 0 {
            return lastActivity + (takesDone > 0 ? 0 : Self.immediateGrace)
        }
        return lastActivity + timeout
    }

    private func startTicking() {
        tickTimer?.invalidate()
        let timer = Timer(timeInterval: 0.25, repeats: true) { [weak self] _ in
            MainActor.assumeIsolated { self?.tick() }
        }
        RunLoop.main.add(timer, forMode: .common)
        tickTimer = timer
    }

    private func tick() {
        guard phase != .off else { return }
        let t = now()
        consumeCommands()

        switch phase {
        case .listening:
            // Never cut by the idle timeout; only by the safety cap.
            if t - takeStartedAt > Self.maxTake { finishTake() }
        case .idle:
            if let expiresAt, t >= expiresAt {
                tearDown()
                return
            }
            if errorMessage != nil, t >= errorUntil {
                errorMessage = nil
                publish()
            }
            if !audio.isRunning, !interrupted {
                try? audio.start(allowBluetooth: bridge.allowBluetoothMic)
            }
            if expiresAt != lastPublishedExpiry { publish() }
        case .transcribing, .off:
            break
        }

        if t - lastBeat >= 1 {
            lastBeat = t
            bridge.beat(at: t)
        }
    }

    // MARK: - Commands

    private func consumeCommands() {
        guard phase != .off, let command = bridge.consumeCommand(at: now()) else { return }
        switch command.kind {
        case .start:
            startTake(command.takeID)
        case .stop:
            guard phase == .listening, activeTake == command.takeID else { return }
            finishTake()
        case .cancel:
            guard activeTake == command.takeID else { return }
            cancelTake()
        }
    }

    // MARK: - Takes

    private func startTake(_ takeID: UInt64) {
        guard phase == .idle, takeID != 0 else { return }
        let status = WBCoreClient.status()
        guard status.ready else {
            showError(status.message.isEmpty ? "Wispr Bro is still setting up. Open the app to finish." : status.message)
            return
        }
        if !audio.isRunning {
            do {
                try audio.start(allowBluetooth: bridge.allowBluetoothMic)
                interrupted = false
            } catch {
                showError("The microphone is busy right now")
                return
            }
        }
        errorMessage = nil
        WBCoreClient.start(takeID)
        audio.beginTake(takeID)
        activeTake = takeID
        takeStartedAt = now()
        phase = .listening
        WBAppTake.shared.set(takeID, phase: .listening)
        startLevelPump()
        liveActivity.show(.listening)
        publish()
    }

    private func finishTake() {
        guard phase == .listening, let takeID = activeTake else { return }
        audio.endTake()
        stopLevelPump()
        phase = .transcribing
        WBAppTake.shared.set(takeID, phase: .transcribing)
        liveActivity.show(.transcribing)
        publish()

        let generation = takeGeneration
        let background = UIApplication.shared.beginBackgroundTask(withName: "wb.take.finish")
        workQueue.async { [weak self] in
            let result = WBCoreClient.finish(takeID)
            DispatchQueue.main.async {
                MainActor.assumeIsolated {
                    self?.takeFinished(takeID, generation: generation, result: result)
                    UIApplication.shared.endBackgroundTask(background)
                }
            }
        }
    }

    private func takeFinished(_ takeID: UInt64, generation: Int, result: WBCoreClient.TakeResult) {
        // A cancelled take's result is dropped.
        guard generation == takeGeneration, activeTake == takeID, phase == .transcribing else { return }
        activeTake = nil
        phase = .idle
        takesDone += 1
        touch()
        if WBAppTake.shared.owns(takeID) {
            // The app's own take: the text goes to the note, not the keyboard.
            switch result {
            case .text(let text):
                let trimmed = text.trimmingCharacters(in: .whitespacesAndNewlines)
                if trimmed.isEmpty {
                    WBAppTake.shared.fail(takeID, "Didn't hear anything")
                } else {
                    WBAppTake.shared.finish(takeID, text: trimmed)
                }
            case .failure(let message):
                WBAppTake.shared.fail(takeID, message)
            }
            liveActivity.show(.ready)
            publish()
            if endAfterTake { tearDown() }
            return
        }
        switch result {
        case .text(let text):
            let trimmed = text.trimmingCharacters(in: .whitespacesAndNewlines)
            if trimmed.isEmpty {
                showError("Didn't hear anything", seconds: 3, publishNow: false)
            } else {
                bridge.publishResult(takeID: takeID, text: trimmed)
            }
        case .failure(let message):
            showError(message, publishNow: false)
        }
        liveActivity.show(.ready)
        publish()
        if endAfterTake { tearDown() }
    }

    private func cancelTake() {
        guard let takeID = activeTake else { return }
        takeGeneration += 1
        if phase == .listening {
            audio.endTake()
            stopLevelPump()
        }
        WBCoreClient.cancel(takeID)
        WBAppTake.shared.cancel(takeID)
        activeTake = nil
        phase = .idle
        touch()
        liveActivity.show(.ready)
        publish()
        if endAfterTake { tearDown() }
    }

    private func showError(_ message: String, seconds: TimeInterval = 5, publishNow: Bool = true) {
        errorMessage = message
        errorUntil = now() + seconds
        if publishNow { publish() }
    }

    // MARK: - Levels

    /// Writes one waveform level about 30 times a second while listening.
    private func startLevelPump() {
        stopLevelPump()
        let timer = DispatchSource.makeTimerSource(queue: levelQueue)
        let audio = self.audio
        let channel = bridge.levels
        var last: Float = 0
        timer.schedule(deadline: .now(), repeating: .milliseconds(33), leeway: .milliseconds(4))
        timer.setEventHandler {
            if let level = audio.popLevel() { last = level }
            channel?.write(level: last)
            WBAppTake.shared.level = last
        }
        timer.resume()
        levelTimer = timer
    }

    private func stopLevelPump() {
        levelTimer?.cancel()
        levelTimer = nil
        let channel = bridge.levels
        levelQueue.async { channel?.write(level: 0) }
    }

    // MARK: - Audio events

    private func handleAudio(_ event: WBAudioEvent) {
        guard phase != .off else { return }
        switch event {
        case .interruptionBegan:
            // A call or Siri took the microphone: keep the words spoken so far.
            interrupted = true
            if phase == .listening {
                if now() - takeStartedAt >= Self.minTake { finishTake() } else { cancelTake() }
            }
            audio.stopEngine()

        case .interruptionEnded(let shouldResume):
            interrupted = false
            guard shouldResume || UIApplication.shared.applicationState == .active else {
                // iOS will not let a background app take the microphone back.
                if phase == .idle { tearDown() } else { endAfterTake = true }
                return
            }
            restartAudio()

        case .inputChanged:
            // AirPods in or out, or a new format: rebuild the tap. An active
            // take keeps going on the new input.
            guard !interrupted, !restartPending else { return }
            restartPending = true
            DispatchQueue.main.asyncAfter(deadline: .now() + 0.2) { [weak self] in
                MainActor.assumeIsolated {
                    guard let self else { return }
                    self.restartPending = false
                    guard self.phase != .off, !self.interrupted else { return }
                    self.restartAudio()
                }
            }

        case .mediaServicesReset:
            restartAudio()
        }
    }

    private func restartAudio() {
        do {
            try audio.start(allowBluetooth: bridge.allowBluetoothMic)
        } catch {
            if phase == .listening {
                // Keep what was said before the microphone went away.
                if now() - takeStartedAt >= Self.minTake { finishTake() } else { cancelTake() }
                endAfterTake = true
            } else if phase == .idle {
                tearDown()
            } else {
                endAfterTake = true
            }
        }
    }

    // MARK: - Publishing

    private func publish() {
        let t = now()
        let state: WBSessionState
        switch phase {
        case .off: state = .noSession
        case .idle: state = errorMessage == nil ? .idle : .error
        case .listening: state = .listening
        case .transcribing: state = .transcribing
        }
        let expiry = phase == .off ? nil : expiresAt
        lastPublishedExpiry = expiry
        bridge.publish(WBSnapshot(
            state: state,
            message: errorMessage,
            takeID: activeTake,
            takeStartedAt: phase == .listening ? takeStartedAt : nil,
            sessionExpiresAt: expiry,
            updatedAt: t
        ))
        refreshStateCache(state: state, message: errorMessage, expiry: expiry)
    }

    private func refreshStateCache(state: WBSessionState = .noSession, message: String? = nil, expiry: TimeInterval? = nil) {
        WBSessionStateCache.shared.set([
            "active": phase != .off,
            "state": message != nil && phase == .off ? WBSessionState.error.rawValue : state.rawValue,
            "message": message ?? NSNull(),
            "takeId": activeTake.map { String($0) as Any } ?? NSNull(),
            "expiresAt": expiry ?? NSNull(),
            "returnHintVisible": hint.isVisible,
        ])
    }

    // MARK: - Microphone permission

    private func requestMicrophone(_ completion: @escaping @MainActor (Bool) -> Void) {
        switch AVAudioApplication.shared.recordPermission {
        case .granted:
            completion(true)
        case .denied:
            completion(false)
        default:
            AVAudioApplication.requestRecordPermission { granted in
                DispatchQueue.main.async {
                    MainActor.assumeIsolated { completion(granted) }
                }
            }
        }
    }

    // MARK: - Back to the app the user was typing in

    /// Route 1 (flag `wb.session.autoReturn`, on by default): the host app is
    /// detected (named by the keyboard, or observed by WBHostAppObserver) and
    /// opened through its URL scheme, or by bundle id when it has none.
    /// Route 2 (public): the return app chosen in settings (`wb.session.returnAppID`).
    /// Otherwise the swipe-back hint stays up.
    private func returnToHost(_ request: WBStartRequest?) {
        // Something to look at while the host is found, and the fallback.
        hint.show()
        refreshVisibleHint()

        guard bridge.autoReturn else {
            NSLog("WBReturn: automatic return is off")
            openChosenReturnApp()
            return
        }
        if let named = request?.hostBundleID, !WBHostAppObserver.isOwnBundle(named) {
            NSLog("WBReturn: host named by the keyboard: %@", named)
            open(hostBundleID: named)
            return
        }
        WBHostAppObserver.shared.start()
        NSLog("WBReturn: waiting for the host (observer available: %d)", WBHostAppObserver.shared.isAvailable ? 1 : 0)
        WBHostAppObserver.shared.waitForHost(since: Date().addingTimeInterval(-5), timeout: 2) { [weak self] bundle in
            MainActor.assumeIsolated {
                guard let self, self.hint.isVisible else { return }
                if let bundle {
                    NSLog("WBReturn: host observed: %@", bundle)
                    self.open(hostBundleID: bundle)
                } else {
                    NSLog("WBReturn: no host observed; the swipe hint stays")
                    self.openChosenReturnApp()
                }
            }
        }
    }

    private func open(hostBundleID: String) {
        let byBundleID = { [weak self] in
            WBHostAppObserver.openApplication(bundleID: hostBundleID) { opened in
                MainActor.assumeIsolated {
                    NSLog("WBReturn: opened %@ by bundle id: %d", hostBundleID, opened ? 1 : 0)
                    if !opened { self?.openChosenReturnApp() }
                }
            }
        }
        guard let url = WBHostAppCatalog.app(bundleID: hostBundleID)?.url else {
            byBundleID()
            return
        }
        UIApplication.shared.open(url, options: [:]) { opened in
            MainActor.assumeIsolated {
                NSLog("WBReturn: opened %@ by its link: %d", hostBundleID, opened ? 1 : 0)
                if !opened { byBundleID() }
            }
        }
    }

    private func openChosenReturnApp() {
        guard let url = WBHostAppCatalog.app(id: bridge.returnAppID)?.url else { return }
        UIApplication.shared.open(url, options: [:], completionHandler: nil)
    }

    private func refreshVisibleHint() {
        WBSessionStateCache.shared.merge(["returnHintVisible": hint.isVisible])
    }
}

/// The last published state, readable from any thread (for the C API).
final class WBSessionStateCache: @unchecked Sendable {
    static let shared = WBSessionStateCache()
    private let lock = NSLock()
    private var values: [String: Any] = ["active": false, "state": WBSessionState.noSession.rawValue]

    func set(_ new: [String: Any]) {
        lock.lock()
        values = new
        lock.unlock()
    }

    func merge(_ extra: [String: Any]) {
        lock.lock()
        values.merge(extra) { _, new in new }
        lock.unlock()
    }

    func get() -> [String: Any] {
        lock.lock()
        defer { lock.unlock() }
        return values
    }
}

/// The app's own take (a note's Record button), readable from any thread
/// (for the C API): where it is, its level while listening, and its text.
final class WBAppTake: @unchecked Sendable {
    enum Phase: String {
        case none, starting, listening, transcribing, done, failed, cancelled
    }

    static let shared = WBAppTake()
    private let lock = NSLock()
    private var id: UInt64 = 0
    private var phase: Phase = .none
    private var text: String?
    private var error: String?
    private var currentLevel: Float = 0

    var level: Float {
        get { lock.lock(); defer { lock.unlock() }; return currentLevel }
        set { lock.lock(); currentLevel = newValue; lock.unlock() }
    }

    func begin(_ takeID: UInt64) {
        lock.lock()
        id = takeID
        phase = .starting
        text = nil
        error = nil
        currentLevel = 0
        lock.unlock()
    }

    func owns(_ takeID: UInt64) -> Bool {
        lock.lock()
        defer { lock.unlock() }
        return id == takeID && takeID != 0
    }

    /// Moves the take on; ignored for a take that is not the app's.
    func set(_ takeID: UInt64, phase next: Phase) {
        lock.lock()
        if id == takeID { phase = next }
        lock.unlock()
    }

    func finish(_ takeID: UInt64, text result: String) {
        lock.lock()
        if id == takeID {
            phase = .done
            text = result
            currentLevel = 0
        }
        lock.unlock()
    }

    func fail(_ takeID: UInt64, _ message: String) {
        lock.lock()
        if id == takeID {
            phase = .failed
            error = message
            currentLevel = 0
        }
        lock.unlock()
    }

    func cancel(_ takeID: UInt64) {
        lock.lock()
        if id == takeID, phase != .done, phase != .failed {
            phase = .cancelled
            currentLevel = 0
        }
        lock.unlock()
    }

    /// {"phase":"…","level":0…1,"text":string|null,"error":string|null}
    func json(_ takeID: UInt64) -> String {
        lock.lock()
        let mine = id == takeID
        let object: [String: Any] = [
            "phase": (mine ? phase : .none).rawValue,
            "level": mine && phase == .listening ? Double(currentLevel) : 0,
            "text": (mine ? text : nil) ?? NSNull(),
            "error": (mine ? error : nil) ?? NSNull(),
        ]
        lock.unlock()
        guard let data = try? JSONSerialization.data(withJSONObject: object, options: [.sortedKeys]),
              let json = String(data: data, encoding: .utf8) else { return "{}" }
        return json
    }
}
