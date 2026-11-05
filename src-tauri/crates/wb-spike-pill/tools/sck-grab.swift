// SPDX-License-Identifier: GPL-3.0-or-later
//
// RS-03 capture helper (test tool only, never shipped): records the main
// display with an SCStream, the ScreenCaptureKit path Zoom, OBS and Teams use
// on macOS 14+, and writes the 15th frame as a PNG.
//
//   sck-grab OUT.png

import CoreImage
import Foundation
import ScreenCaptureKit

final class Grabber: NSObject, SCStreamOutput {
    var frames = 0
    let out: URL
    let done = DispatchSemaphore(value: 0)
    init(out: URL) { self.out = out }

    func stream(_ stream: SCStream, didOutputSampleBuffer sb: CMSampleBuffer, of type: SCStreamOutputType) {
        guard type == .screen, let pb = sb.imageBuffer else { return }
        frames += 1
        if frames != 15 { return }
        let ci = CIImage(cvPixelBuffer: pb)
        let ctx = CIContext()
        if let cs = CGColorSpace(name: CGColorSpace.sRGB) {
            try? ctx.writePNGRepresentation(of: ci, to: out, format: .RGBA8, colorSpace: cs)
        }
        done.signal()
    }
}

let args = CommandLine.arguments
guard args.count >= 2 else {
    FileHandle.standardError.write("usage: sck-grab OUT.png\n".data(using: .utf8)!)
    exit(2)
}
let grabber = Grabber(out: URL(fileURLWithPath: args[1]))
Task {
    do {
        let content = try await SCShareableContent.excludingDesktopWindows(false, onScreenWindowsOnly: true)
        guard let display = content.displays.first(where: { $0.displayID == CGMainDisplayID() }) else {
            exit(1)
        }
        let filter = SCContentFilter(display: display, excludingWindows: [])
        let cfg = SCStreamConfiguration()
        let scale = Int(filter.pointPixelScale)
        cfg.width = display.width * scale
        cfg.height = display.height * scale
        cfg.minimumFrameInterval = CMTime(value: 1, timescale: 30)
        cfg.showsCursor = false
        let stream = SCStream(filter: filter, configuration: cfg, delegate: nil)
        try stream.addStreamOutput(grabber, type: .screen, sampleHandlerQueue: .global())
        try await stream.startCapture()
        // A static screen sends few frames; wait up to 10 s.
        _ = grabber.done.wait(timeout: .now() + 10)
        try await stream.stopCapture()
        exit(grabber.frames >= 15 ? 0 : 3)
    } catch {
        FileHandle.standardError.write("sck-grab: \(error)\n".data(using: .utf8)!)
        exit(1)
    }
}
dispatchMain()
