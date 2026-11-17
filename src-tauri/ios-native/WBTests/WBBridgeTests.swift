// SPDX-License-Identifier: GPL-3.0-or-later
import XCTest
@testable import WBNativeHarness

final class WBBridgeTests: XCTestCase {
    private var suite = ""
    private var directory: URL!
    private var app: WBBridge!       // the containing app's end
    private var keyboard: WBBridge!  // the keyboard's end

    override func setUp() {
        suite = "wb.tests.\(UUID().uuidString)"
        directory = FileManager.default.temporaryDirectory.appendingPathComponent(suite)
        try? FileManager.default.createDirectory(at: directory, withIntermediateDirectories: true)
        app = WBBridge(defaults: UserDefaults(suiteName: suite), levelDirectory: directory)
        keyboard = WBBridge(defaults: UserDefaults(suiteName: suite), levelDirectory: directory)
    }

    override func tearDown() {
        UserDefaults().removePersistentDomain(forName: suite)
        try? FileManager.default.removeItem(at: directory)
    }

    // MARK: State

    func testNoSessionByDefault() {
        XCTAssertEqual(keyboard.snapshot(at: 100).state, .noSession)
        XCTAssertFalse(keyboard.isSessionAlive(at: 100))
    }

    func testSnapshotRoundTripWhileHeartbeatIsFresh() {
        app.beat(at: 100)
        app.publish(WBSnapshot(state: .listening, takeID: 42, takeStartedAt: 99, sessionExpiresAt: 400, updatedAt: 100))
        let seen = keyboard.snapshot(at: 101)
        XCTAssertEqual(seen.state, .listening)
        XCTAssertEqual(seen.takeID, 42)
        XCTAssertEqual(seen.sessionExpiresAt, 400)
    }

    func testEveryStateSurvivesTheBridge() {
        app.beat(at: 10)
        for state in [WBSessionState.idle, .listening, .transcribing, .error] {
            app.publish(WBSnapshot(state: state, message: state == .error ? "Oops" : nil, updatedAt: 10))
            XCTAssertEqual(keyboard.snapshot(at: 10.5).state, state)
        }
        XCTAssertEqual(keyboard.snapshot(at: 10.5).message, "Oops")
    }

    // MARK: Heartbeat

    func testHeartbeatExpiryTurnsALiveSessionIntoNoSession() {
        app.beat(at: 100)
        app.publish(WBSnapshot(state: .idle, updatedAt: 100))
        XCTAssertTrue(keyboard.isSessionAlive(at: 102.9))
        XCTAssertEqual(keyboard.snapshot(at: 102.9).state, .idle)
        // The app died without saying goodbye.
        XCTAssertFalse(keyboard.isSessionAlive(at: 103.1))
        XCTAssertEqual(keyboard.snapshot(at: 103.1).state, .noSession)
        // It comes back.
        app.beat(at: 200)
        XCTAssertEqual(keyboard.snapshot(at: 200.5).state, .idle)
    }

    func testClearedHeartbeatEndsTheSessionAtOnce() {
        app.beat(at: 100)
        app.publish(WBSnapshot(state: .idle, updatedAt: 100))
        app.clearHeartbeat()
        XCTAssertEqual(keyboard.snapshot(at: 100.1).state, .noSession)
    }

    func testRecentErrorShowsWithoutASession() {
        app.publish(WBSnapshot(state: .error, message: "Microphone access is off", updatedAt: 100))
        XCTAssertEqual(keyboard.snapshot(at: 103).state, .error)
        XCTAssertEqual(keyboard.snapshot(at: 103).message, "Microphone access is off")
        XCTAssertEqual(keyboard.snapshot(at: 100 + WBBridgeConstants.errorLifetime + 1).state, .noSession)
    }

    // MARK: Commands

    func testCommandIsConsumedOnce() {
        keyboard.send(.start, takeID: 7, at: 100)
        let first = app.consumeCommand(at: 100.1)
        XCTAssertEqual(first?.kind, .start)
        XCTAssertEqual(first?.takeID, 7)
        XCTAssertNil(app.consumeCommand(at: 100.2))

        keyboard.send(.stop, takeID: 7, at: 101)
        XCTAssertEqual(app.consumeCommand(at: 101.1)?.kind, .stop)
        keyboard.send(.cancel, takeID: 7, at: 102)
        XCTAssertEqual(app.consumeCommand(at: 102.1)?.kind, .cancel)
        XCTAssertNil(app.consumeCommand(at: 102.2))
    }

    func testStaleCommandIsIgnoredForGood() {
        keyboard.send(.start, takeID: 7, at: 100)
        XCTAssertNil(app.consumeCommand(at: 100 + WBBridgeConstants.handoffWindow + 1))
        XCTAssertNil(app.consumeCommand(at: 100.5), "a stale command must not come back")
    }

    func testAFreshAppProcessDoesNotReplayAnOldCommand() {
        keyboard.send(.start, takeID: 7, at: 100)
        XCTAssertNotNil(app.consumeCommand(at: 100.1))
        let relaunched = WBBridge(defaults: UserDefaults(suiteName: suite), levelDirectory: directory)
        XCTAssertNil(relaunched.consumeCommand(at: 100.2))
    }

    // MARK: Exactly-once text hand-off

    func testResultIsClaimedExactlyOnce() {
        app.publishResult(takeID: 42, text: "Hello there", at: 100)
        XCTAssertEqual(keyboard.claimResult(at: 100.1)?.text, "Hello there")
        XCTAssertNil(keyboard.claimResult(at: 100.2))
        // A second keyboard instance (another host app) gets nothing either.
        let other = WBBridge(defaults: UserDefaults(suiteName: suite), levelDirectory: directory)
        XCTAssertNil(other.claimResult(at: 100.3))
        // Republishing the same take does not insert it again.
        app.publishResult(takeID: 42, text: "Hello there", at: 101)
        XCTAssertNil(keyboard.claimResult(at: 101.1))
    }

    func testEachTakeIsDeliveredOnce() {
        app.publishResult(takeID: 1, text: "One", at: 100)
        XCTAssertEqual(keyboard.claimResult(at: 100.1)?.takeID, 1)
        app.publishResult(takeID: 2, text: "Two", at: 101)
        XCTAssertEqual(keyboard.claimResult(at: 101.1)?.text, "Two")
        XCTAssertNil(keyboard.claimResult(at: 101.2))
    }

    func testResultSurvivesTheSessionEndingButNotForever() {
        app.publishResult(takeID: 9, text: "Late", at: 100)
        app.clearHeartbeat()
        XCTAssertEqual(keyboard.pendingResult(at: 130)?.text, "Late")
        XCTAssertNil(keyboard.claimResult(at: 100 + WBBridgeConstants.resultLifetime + 1))
    }

    func testEmptyResultIsNeverDelivered() {
        app.publishResult(takeID: 3, text: "", at: 100)
        XCTAssertNil(keyboard.claimResult(at: 100.1))
    }

    // MARK: Start request

    func testStartRequestIsTakenOnceAndExpires() {
        let request = WBStartRequest(requestedAt: 100, takeID: 55, hostBundleID: "com.apple.MobileSMS")
        keyboard.requestSessionStart(request)
        XCTAssertEqual(app.takePendingStartRequest(at: 101), request)
        XCTAssertNil(app.takePendingStartRequest(at: 101))

        keyboard.requestSessionStart(WBStartRequest(requestedAt: 100, takeID: nil))
        XCTAssertNil(app.takePendingStartRequest(at: 100 + WBBridgeConstants.handoffWindow + 1))
    }

    func testStartRequestURLRoundTrip() throws {
        let request = WBStartRequest(id: "abc", requestedAt: 1234.5, takeID: 987_654_321, hostBundleID: "com.apple.mobilenotes")
        XCTAssertEqual(request.url.scheme, "wisprbro")
        let parsed = try XCTUnwrap(WBStartRequest(url: request.url, now: 1235))
        XCTAssertEqual(parsed, request)

        let bare = try XCTUnwrap(WBStartRequest(url: URL(string: "wisprbro://session/start")!, now: 50))
        XCTAssertNil(bare.takeID)
        XCTAssertEqual(bare.requestedAt, 50)
        XCTAssertNil(WBStartRequest(url: URL(string: "wisprbro://settings")!))
        XCTAssertNil(WBStartRequest(url: URL(string: "https://example.com/session/start")!))
    }

    // MARK: Levels, settings, Full Access

    func testLevelChannelCrossesTheBridgeAndGoesQuiet() throws {
        let writer = try XCTUnwrap(app.levels)
        let reader = try XCTUnwrap(keyboard.levels)
        writer.write(level: 0.7, at: 100)
        XCTAssertEqual(reader.currentLevel(at: 100.02), 0.7, accuracy: 0.0001)
        let first = reader.read().sequence
        writer.write(level: 2.0, at: 100.033)
        XCTAssertEqual(reader.read().level, 1.0, "levels are clamped to 0…1")
        XCTAssertEqual(reader.read().sequence, first &+ 1)
        XCTAssertEqual(reader.currentLevel(at: 101), 0, "a writer that stopped reads as silence")
    }

    func testSettingsDefaultsAndValues() {
        XCTAssertEqual(app.timeoutSeconds, 300)
        XCTAssertTrue(app.autoReturn)
        app.defaults?.set(0, forKey: WBBridgeKey.timeoutSeconds)
        XCTAssertEqual(keyboard.timeoutSeconds, 0)
        app.defaults?.set(-1, forKey: WBBridgeKey.timeoutSeconds)
        XCTAssertNil(keyboard.timeoutSeconds)
        app.defaults?.set(3600, forKey: WBBridgeKey.timeoutSeconds)
        XCTAssertEqual(keyboard.timeoutSeconds, 3600)
        app.defaults?.set(0, forKey: WBBridgeKey.autoReturn)
        XCTAssertFalse(keyboard.autoReturn)
    }

    func testFullAccessReport() {
        XCTAssertEqual(app.fullAccessKnown, -1)
        keyboard.noteFullAccess(true, at: 100)
        XCTAssertEqual(app.fullAccessKnown, 1)
        keyboard.noteFullAccess(false, at: 101)
        XCTAssertEqual(app.fullAccessKnown, 0)
    }

    func testBridgeWithoutStorageIsInert() {
        let none = WBBridge(defaults: nil, levelDirectory: nil)
        none.send(.start, takeID: 1)
        none.publishResult(takeID: 1, text: "x")
        XCTAssertNil(none.claimResult())
        XCTAssertEqual(none.snapshot().state, .noSession)
    }
}

final class WBKeyboardLogicTests: XCTestCase {
    private func snapshot(_ state: WBSessionState, take: UInt64? = nil, at time: TimeInterval = 100) -> WBSnapshot {
        WBSnapshot(state: state, takeID: take, updatedAt: time)
    }

    func testFullAccessComesFirst() {
        let r = WBKeyboardModel.resolve(hasFullAccess: false, snapshot: snapshot(.idle), pending: nil, now: 100)
        XCTAssertEqual(r.phase, .needsFullAccess)
    }

    func testStartShowsListeningUntilTheAppConfirms() {
        let pending = WBKeyboardModel.Pending(kind: .start, takeID: 5, sentAt: 100)
        var r = WBKeyboardModel.resolve(hasFullAccess: true, snapshot: snapshot(.idle), pending: pending, now: 100.2)
        XCTAssertEqual(r.phase, .listening)
        XCTAssertFalse(r.pendingSettled)
        r = WBKeyboardModel.resolve(hasFullAccess: true, snapshot: snapshot(.listening, take: 5), pending: pending, now: 100.4)
        XCTAssertEqual(r.phase, .listening)
        XCTAssertTrue(r.pendingSettled)
        // The app never answered: fall back to what it says.
        r = WBKeyboardModel.resolve(hasFullAccess: true, snapshot: snapshot(.idle), pending: pending, now: 103)
        XCTAssertEqual(r.phase, .idle)
        XCTAssertTrue(r.pendingSettled)
    }

    func testStopAndCancelAreShownAtOnce() {
        let stop = WBKeyboardModel.Pending(kind: .stop, takeID: 5, sentAt: 100)
        XCTAssertEqual(
            WBKeyboardModel.resolve(hasFullAccess: true, snapshot: snapshot(.listening, take: 5), pending: stop, now: 100.1).phase,
            .transcribing
        )
        let cancel = WBKeyboardModel.Pending(kind: .cancel, takeID: 5, sentAt: 100)
        XCTAssertEqual(
            WBKeyboardModel.resolve(hasFullAccess: true, snapshot: snapshot(.transcribing, take: 5), pending: cancel, now: 100.1).phase,
            .idle
        )
    }

    func testErrorCarriesItsMessage() {
        var s = snapshot(.error)
        s.message = "Microphone access is off"
        XCTAssertEqual(
            WBKeyboardModel.resolve(hasFullAccess: true, snapshot: s, pending: nil, now: 100).phase,
            .error("Microphone access is off")
        )
    }

    func testLeadingSpace() {
        XCTAssertEqual(WBInsertion.text(for: "hello", before: "Hi there."), " hello")
        XCTAssertEqual(WBInsertion.text(for: "hello", before: "word"), " hello")
        XCTAssertEqual(WBInsertion.text(for: "hello", before: "word "), "hello")
        XCTAssertEqual(WBInsertion.text(for: "hello", before: "line\n"), "hello")
        XCTAssertEqual(WBInsertion.text(for: "hello", before: nil), "hello")
        XCTAssertEqual(WBInsertion.text(for: "hello", before: ""), "hello")
        XCTAssertEqual(WBInsertion.text(for: "hello", before: "say ("), "hello")
        XCTAssertEqual(WBInsertion.text(for: ", and more", before: "word"), ", and more")
        XCTAssertEqual(WBInsertion.text(for: "  spaced  ", before: "x"), " spaced")
        XCTAssertEqual(WBInsertion.text(for: "   ", before: "x"), "")
    }

    func testWaveformShape() {
        XCTAssertEqual(WBWaveformState.weights.count, 11)
        XCTAssertEqual(WBWaveformState.weights[5], 1, accuracy: 0.0001)
        XCTAssertEqual(WBWaveformState.weights[0], 0.55, accuracy: 0.0001)
        XCTAssertEqual(WBWaveformState.weights[10], 0.55, accuracy: 0.0001)

        let state = WBWaveformState()
        XCTAssertFalse(state.heard)
        var t = 0.0
        for _ in 0..<60 { t += 1.0 / 60; state.step(level: 1, now: t, reduceMotion: false) }
        XCTAssertTrue(state.heard)
        XCTAssertGreaterThan(state.shown[5], state.shown[0], "loudest in the middle")
        XCTAssertGreaterThan(state.shown[5], 0.7) // shimmer takes up to 28 %
    }

    func testHostCatalog() {
        XCTAssertEqual(WBHostAppCatalog.app(bundleID: "com.apple.MobileSMS")?.id, "messages")
        XCTAssertNil(WBHostAppCatalog.app(bundleID: "com.example.unknown"))
        XCTAssertNil(WBHostAppCatalog.app(id: nil))
    }
}
