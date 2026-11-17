// SPDX-License-Identifier: GPL-3.0-or-later
// The warm microphone: one AVAudioSession + AVAudioEngine that stays up for
// the whole session and feeds 16 kHz mono Float32 to the Rust core while a
// take is active.
//
// Ported from Hex (MIT, (c) 2026 Christopher Langton): ios/HEXMobile/AudioBackend.swift
// (keepalive engine, interruption and route notifications). See ../NOTICE-hex.md.
// Changes: one engine for keepalive and capture (no AVAudioRecorder, no file),
// live conversion to 16 kHz, takes survive a route change.

import AVFAudio
import Foundation

enum WBAudioEvent: Equatable {
    case interruptionBegan
    case interruptionEnded(shouldResume: Bool)
    /// The input device or its format changed; the engine must be restarted.
    case inputChanged
    case mediaServicesReset
}

final class WBAudioEngine {
    /// Delivered on the main queue.
    var onEvent: ((WBAudioEvent) -> Void)?

    /// Samples per call to the core: about 33 ms, one waveform frame.
    private static let chunk = 533

    private var engine = AVAudioEngine()
    private var tapInstalled = false
    private var converter: AVAudioConverter?
    private let outputFormat = AVAudioFormat(
        commonFormat: .pcmFormatFloat32, sampleRate: 16_000, channels: 1, interleaved: false
    )!
    private var observers: [NSObjectProtocol] = []

    /// Guards `activeTake` and `levels`; held across each feed so `endTake`
    /// returns only after the last sample of the take was fed.
    private let lock = NSLock()
    private var activeTake: UInt64 = 0
    private var levels: [Float] = []

    init() {
        installNotifications()
    }

    deinit {
        observers.forEach(NotificationCenter.default.removeObserver)
    }

    var isRunning: Bool {
        #if targetEnvironment(simulator)
        if fakeTimer != nil { return true }
        #endif
        return engine.isRunning
    }

    #if targetEnvironment(simulator)
    // The simulator's audio input aborts the process when the Mac's
    // microphone is unavailable (headless runs). `--wb-fake-mic` swaps the
    // engine for a generated, speech-shaped tone so the rest of the session
    // can be driven end to end. Simulator builds only.
    //
    // `WB_FAKE_MIC_WAV=/path/to.wav` (environment, implies the fake mic)
    // plays that 16 kHz mono 16-bit WAV instead of the tone, from its start
    // at every take and silence after its end, so real speech can flow
    // session → core → text without a microphone. The file is read again at
    // every take (a test swaps it between takes), and the path is kept in
    // the App Group defaults so a launch by the keyboard's link, which has
    // no environment, uses the same file.
    private static let fakeWavPath: String? = {
        let defaults = UserDefaults(suiteName: WBBridgeConstants.appGroup)
        let key = "wb.debug.fakeMicWav"
        if let path = ProcessInfo.processInfo.environment["WB_FAKE_MIC_WAV"] {
            if path.isEmpty { defaults?.removeObject(forKey: key) } else { defaults?.set(path, forKey: key) }
            return path.isEmpty ? nil : path
        }
        return defaults?.string(forKey: key)
    }()
    private static func loadFakeWav() -> [Float]? {
        guard let path = fakeWavPath,
              let data = FileManager.default.contents(atPath: path),
              // The samples follow the "data" chunk's 8-byte header (afconvert
              // may put a filler chunk before it).
              let marker = data.range(of: Data("data".utf8)) else { return nil }
        let pcm = [UInt8](data.suffix(from: marker.upperBound + 4))
        return stride(from: 0, to: pcm.count - 1, by: 2).map { i in
            Float(Int16(bitPattern: UInt16(pcm[i]) | UInt16(pcm[i + 1]) << 8)) / 32768
        }
    }
    private static let fakeMic = ProcessInfo.processInfo.arguments.contains("--wb-fake-mic") || fakeWavPath != nil
    private var fakeWav: [Float]?
    private var fakeTimer: DispatchSourceTimer?
    private var fakeClock = 0
    private var fakeWavTake: UInt64 = 0
    private var fakeWavOffset = 0

    private func startFake() {
        let timer = DispatchSource.makeTimerSource(queue: DispatchQueue(label: "com.wisprbro.ios.fakemic"))
        timer.schedule(deadline: .now(), repeating: .milliseconds(33))
        timer.setEventHandler { [weak self] in
            guard let self else { return }
            var samples = [Float](repeating: 0, count: Self.chunk)
            for i in 0..<Self.chunk {
                let t = Double(self.fakeClock + i) / 16_000
                let envelope = pow(abs(sin(t * 2.6)), 2) * (0.6 + 0.4 * sin(t * 9.1))
                samples[i] = Float(0.2 * envelope * sin(2 * .pi * 220 * t))
            }
            self.fakeClock += Self.chunk
            self.lock.lock()
            if self.activeTake != self.fakeWavTake {
                self.fakeWavTake = self.activeTake
                self.fakeWavOffset = 0
                if self.activeTake != 0 { self.fakeWav = Self.loadFakeWav() }
            }
            if let wav = self.fakeWav {
                for i in 0..<Self.chunk {
                    let at = self.fakeWavOffset + i
                    samples[i] = at < wav.count ? wav[at] : 0
                }
                if self.activeTake != 0 { self.fakeWavOffset += Self.chunk }
            }
            if self.activeTake != 0 {
                self.levels.append(WBCoreClient.feed(self.activeTake, samples, samples.count))
            }
            self.lock.unlock()
        }
        timer.resume()
        fakeTimer = timer
    }
    #endif

    // MARK: Engine

    func start(allowBluetooth: Bool) throws {
        stopEngine()
        #if targetEnvironment(simulator)
        if Self.fakeMic {
            startFake()
            return
        }
        #endif
        let session = AVAudioSession.sharedInstance()
        // Record next to whatever is playing; AirPods keep their full-quality
        // output unless the user chose them as the microphone.
        var options: AVAudioSession.CategoryOptions = [.mixWithOthers, .defaultToSpeaker, .allowBluetoothA2DP]
        if allowBluetooth { options.insert(.allowBluetoothHFP) }
        try session.setCategory(.playAndRecord, mode: .default, options: options)
        try session.setActive(true)

        let input = engine.inputNode
        let format = input.outputFormat(forBus: 0)
        guard format.sampleRate > 0, format.channelCount > 0,
              let converter = AVAudioConverter(from: format, to: outputFormat) else {
            throw CocoaError(.coderInvalidValue)
        }
        self.converter = converter
        input.installTap(onBus: 0, bufferSize: 1_024, format: format) { [weak self] buffer, _ in
            self?.process(buffer)
        }
        tapInstalled = true
        engine.prepare()
        try engine.start()
    }

    /// Stops capturing but keeps the audio session (used during an interruption).
    func stopEngine() {
        #if targetEnvironment(simulator)
        fakeTimer?.cancel()
        fakeTimer = nil
        #endif
        if tapInstalled {
            engine.inputNode.removeTap(onBus: 0)
            tapInstalled = false
        }
        if engine.isRunning { engine.stop() }
        engine.reset()
    }

    func shutDown() {
        stopEngine()
        try? AVAudioSession.sharedInstance().setActive(false, options: .notifyOthersOnDeactivation)
    }

    // MARK: Takes

    func beginTake(_ takeID: UInt64) {
        lock.lock()
        activeTake = takeID
        levels.removeAll(keepingCapacity: true)
        lock.unlock()
    }

    func endTake() {
        lock.lock()
        activeTake = 0
        levels.removeAll(keepingCapacity: true)
        lock.unlock()
    }

    /// The next waveform level, in order; nil when none is waiting.
    func popLevel() -> Float? {
        lock.lock()
        defer { lock.unlock() }
        // Never fall more than ~100 ms behind the voice.
        if levels.count > 3 { levels.removeFirst(levels.count - 3) }
        return levels.isEmpty ? nil : levels.removeFirst()
    }

    private func process(_ buffer: AVAudioPCMBuffer) {
        lock.lock()
        defer { lock.unlock() }
        guard activeTake != 0, let converter, buffer.frameLength > 0 else { return }

        let ratio = outputFormat.sampleRate / buffer.format.sampleRate
        let capacity = AVAudioFrameCount(Double(buffer.frameLength) * ratio) + 32
        guard let output = AVAudioPCMBuffer(pcmFormat: outputFormat, frameCapacity: capacity) else { return }

        var consumed = false
        var error: NSError?
        converter.convert(to: output, error: &error) { _, status in
            if consumed {
                status.pointee = .noDataNow
                return nil
            }
            consumed = true
            status.pointee = .haveData
            return buffer
        }
        guard error == nil, let samples = output.floatChannelData?[0] else { return }

        let total = Int(output.frameLength)
        var offset = 0
        while offset < total {
            var count = min(Self.chunk, total - offset)
            // Fold a short tail into this chunk.
            if total - offset - count < Self.chunk / 4 { count = total - offset }
            levels.append(WBCoreClient.feed(activeTake, samples + offset, count))
            offset += count
        }
    }

    // MARK: Notifications

    private func installNotifications() {
        let center = NotificationCenter.default
        let session = AVAudioSession.sharedInstance()

        observers.append(center.addObserver(
            forName: AVAudioSession.interruptionNotification, object: session, queue: .main
        ) { [weak self] note in
            guard let raw = note.userInfo?[AVAudioSessionInterruptionTypeKey] as? UInt,
                  let type = AVAudioSession.InterruptionType(rawValue: raw) else { return }
            switch type {
            case .began:
                self?.onEvent?(.interruptionBegan)
            case .ended:
                let options = AVAudioSession.InterruptionOptions(
                    rawValue: note.userInfo?[AVAudioSessionInterruptionOptionKey] as? UInt ?? 0
                )
                self?.onEvent?(.interruptionEnded(shouldResume: options.contains(.shouldResume)))
            @unknown default:
                break
            }
        })

        observers.append(center.addObserver(
            forName: AVAudioSession.routeChangeNotification, object: session, queue: .main
        ) { [weak self] note in
            guard let raw = note.userInfo?[AVAudioSessionRouteChangeReasonKey] as? UInt,
                  let reason = AVAudioSession.RouteChangeReason(rawValue: raw) else { return }
            if reason == .oldDeviceUnavailable || reason == .newDeviceAvailable {
                self?.onEvent?(.inputChanged)
            }
        })

        observers.append(center.addObserver(
            forName: .AVAudioEngineConfigurationChange, object: nil, queue: .main
        ) { [weak self] note in
            guard let self, (note.object as AnyObject?) === self.engine else { return }
            self.onEvent?(.inputChanged)
        })

        observers.append(center.addObserver(
            forName: AVAudioSession.mediaServicesWereResetNotification, object: session, queue: .main
        ) { [weak self] _ in
            guard let self else { return }
            self.tapInstalled = false
            self.engine = AVAudioEngine()
            self.onEvent?(.mediaServicesReset)
        })
    }
}
