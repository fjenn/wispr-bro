// SPDX-License-Identifier: GPL-3.0-or-later
//
// S-38 soak helper (optional since the Mac wave 2 fixes: soak.py feeds the
// app's test microphone, WISPRBRO_TEST_MIC_WAV, and needs no loopback
// driver): plays a WAV into a named audio *output* device and returns when
// it has finished. With a loopback driver (e.g. BlackHole 2ch, test machines
// only, never shipped) set as the app's microphone, the app hears the
// prerecorded take and nothing reaches the speakers (`soak.py --device`).
//
//   swift scripts/wb-soak/play.swift <device name> <file.wav>
//   swift scripts/wb-soak/play.swift --list
//
// Output: one JSON line {"played_ms": …, "device": …} or {"error": …}.

import AVFoundation
import CoreAudio
import Foundation

func out(_ d: [String: Any]) {
    if let data = try? JSONSerialization.data(withJSONObject: d, options: [.sortedKeys]),
       let s = String(data: data, encoding: .utf8) {
        print(s)
        fflush(stdout)
    }
}

func outputDevices() -> [(AudioDeviceID, String)] {
    var addr = AudioObjectPropertyAddress(
        mSelector: kAudioHardwarePropertyDevices,
        mScope: kAudioObjectPropertyScopeGlobal,
        mElement: kAudioObjectPropertyElementMain)
    var size: UInt32 = 0
    AudioObjectGetPropertyDataSize(AudioObjectID(kAudioObjectSystemObject), &addr, 0, nil, &size)
    var ids = [AudioDeviceID](repeating: 0, count: Int(size) / MemoryLayout<AudioDeviceID>.size)
    AudioObjectGetPropertyData(AudioObjectID(kAudioObjectSystemObject), &addr, 0, nil, &size, &ids)
    return ids.compactMap { id in
        var streams = AudioObjectPropertyAddress(
            mSelector: kAudioDevicePropertyStreams,
            mScope: kAudioDevicePropertyScopeOutput,
            mElement: kAudioObjectPropertyElementMain)
        var ssize: UInt32 = 0
        AudioObjectGetPropertyDataSize(id, &streams, 0, nil, &ssize)
        if ssize == 0 { return nil }
        var nameAddr = AudioObjectPropertyAddress(
            mSelector: kAudioObjectPropertyName,
            mScope: kAudioObjectPropertyScopeGlobal,
            mElement: kAudioObjectPropertyElementMain)
        var name: Unmanaged<CFString>?
        var nsize = UInt32(MemoryLayout<Unmanaged<CFString>?>.size)
        guard AudioObjectGetPropertyData(id, &nameAddr, 0, nil, &nsize, &name) == noErr,
              let n = name?.takeRetainedValue() else { return nil }
        return (id, n as String)
    }
}

let args = CommandLine.arguments
if args.count == 2 && args[1] == "--list" {
    out(["devices": outputDevices().map { $0.1 }])
    exit(0)
}
guard args.count == 3 else {
    out(["error": "usage: play.swift <device name> <file.wav> | --list"])
    exit(2)
}
guard let device = outputDevices().first(where: { $0.1 == args[1] }) else {
    out(["error": "no output device named \(args[1])"])
    exit(3)
}

let engine = AVAudioEngine()
let player = AVAudioPlayerNode()
do {
    let file = try AVAudioFile(forReading: URL(fileURLWithPath: args[2]))
    var id = device.0
    guard let unit = engine.outputNode.audioUnit,
          AudioUnitSetProperty(unit, kAudioOutputUnitProperty_CurrentDevice,
                               kAudioUnitScope_Global, 0, &id,
                               UInt32(MemoryLayout<AudioDeviceID>.size)) == noErr else {
        out(["error": "could not select \(args[1])"])
        exit(4)
    }
    engine.attach(player)
    engine.connect(player, to: engine.mainMixerNode, format: file.processingFormat)
    let done = DispatchSemaphore(value: 0)
    player.scheduleFile(file, at: nil, completionCallbackType: .dataPlayedBack) { _ in
        done.signal()
    }
    let started = Date()
    try engine.start()
    player.play()
    done.wait()
    engine.stop()
    out(["played_ms": Int(Date().timeIntervalSince(started) * 1000), "device": device.1])
} catch {
    out(["error": "\(error)"])
    exit(5)
}
