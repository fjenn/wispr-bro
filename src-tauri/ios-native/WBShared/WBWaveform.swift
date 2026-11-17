// SPDX-License-Identifier: GPL-3.0-or-later
// The pill's waveform and spinner, after the Mac app's
// src/fork/pill/states/Waveform.tsx and Working.tsx.

import SwiftUI

/// Eases 11 bars toward one display level. The middle bar follows the newest
/// level, outer bars follow slightly older ones, so speech ripples outward.
final class WBWaveformState {
    static let bars = 11
    static let edge: Double = 0.55
    static let attack: Double = 0.030
    static let release: Double = 0.110

    /// Height weight per bar: 1 in the middle, `edge` at the ends.
    static let weights: [Double] = (0..<bars).map { i in
        let mid = Double(bars - 1) / 2
        let d = abs(Double(i) - mid) / mid
        return 1 - (1 - edge) * pow(d, 1.4)
    }

    private(set) var shown = [Double](repeating: 0, count: bars)
    private(set) var heard = false
    private var history = [Double](repeating: 0, count: 8)
    private var lastStep: TimeInterval = 0
    private var lastHistory: TimeInterval = 0

    func reset() {
        shown = [Double](repeating: 0, count: Self.bars)
        history = [Double](repeating: 0, count: history.count)
        heard = false
        lastStep = 0
    }

    func step(level: Double, now: TimeInterval, reduceMotion: Bool) {
        let dt = lastStep == 0 ? 0.016 : min(1, max(0, now - lastStep))
        lastStep = now
        if level > 0.03 { heard = true }

        // One history slot per ~22 ms.
        if now - lastHistory >= 0.022 {
            lastHistory = now
            history.removeLast()
            history.insert(level, at: 0)
        } else {
            history[0] = max(history[0], level)
        }

        let mid = (Self.bars - 1) / 2
        for i in 0..<Self.bars {
            let distance = abs(i - mid)
            var target = history[min(distance, history.count - 1)] * Self.weights[i]
            if reduceMotion {
                shown[i] = target
                continue
            }
            if target > 0.03 {
                target *= 0.86 + 0.14 * sin(now * 1000 / 160 + Double(i) * 1.9 + Double(i % 3))
            }
            let tau = target > shown[i] ? Self.attack : Self.release
            shown[i] += (target - shown[i]) * (1 - exp(-dt / tau))
        }
    }
}

/// 11 bars, 2 pt wide with a 2.5 pt gap, 2–18 pt tall, times `scale`.
struct WBWaveformView: View {
    /// Returns the current display level, 0…1.
    let level: () -> Float
    var scale: CGFloat = 1
    var color: Color = WBTheme.pillInk
    /// For still previews: fixed bar levels instead of live audio.
    var frozen: [Double]? = nil

    @Environment(\.accessibilityReduceMotion) private var reduceMotion
    @State private var state = WBWaveformState()

    private var barWidth: CGFloat { 2 * scale }
    private var gap: CGFloat { 2.5 * scale }
    private var minHeight: CGFloat { 2 * scale }
    private var maxHeight: CGFloat { 18 * scale }
    private var width: CGFloat {
        CGFloat(WBWaveformState.bars) * barWidth + CGFloat(WBWaveformState.bars - 1) * gap
    }

    var body: some View {
        Group {
            if let frozen {
                Canvas { context, size in
                    draw(context, size, levels: frozen, heard: frozen.contains { $0 > 0.03 })
                }
            } else {
                TimelineView(.animation) { timeline in
                    Canvas { context, size in
                        state.step(
                            level: Double(level()),
                            now: timeline.date.timeIntervalSinceReferenceDate,
                            reduceMotion: reduceMotion
                        )
                        draw(context, size, levels: state.shown, heard: state.heard)
                    }
                }
            }
        }
        .frame(width: width, height: maxHeight)
        .accessibilityHidden(true)
    }

    private func draw(_ context: GraphicsContext, _ size: CGSize, levels: [Double], heard: Bool) {
        var context = context
        context.opacity = heard ? 1 : 0.55
        for i in 0..<WBWaveformState.bars {
            let value = i < levels.count ? min(1, max(0, levels[i])) : 0
            let height = minHeight + CGFloat(value) * (maxHeight - minHeight)
            let rect = CGRect(
                x: CGFloat(i) * (barWidth + gap),
                y: (size.height - height) / 2,
                width: barWidth,
                height: height
            )
            context.fill(Path(roundedRect: rect, cornerRadius: barWidth / 2), with: .color(color))
        }
    }
}

/// Eight short spokes, the brightest leads; steps once per spoke.
struct WBSpinner: View {
    var size: CGFloat = 18
    var color: Color = WBTheme.pillInk
    var accent: Color = Color(wbHex: 0xC3A0EE)

    @Environment(\.accessibilityReduceMotion) private var reduceMotion

    var body: some View {
        TimelineView(.periodic(from: .now, by: reduceMotion ? 3600 : 0.9 / 8)) { timeline in
            let step = reduceMotion ? 0 : Int(timeline.date.timeIntervalSinceReferenceDate / (0.9 / 8)) % 8
            Canvas { context, canvas in
                let unit = canvas.width / 16
                let center = CGPoint(x: canvas.width / 2, y: canvas.height / 2)
                for i in 0..<8 {
                    var spoke = Path()
                    spoke.move(to: CGPoint(x: center.x, y: 2 * unit))
                    spoke.addLine(to: CGPoint(x: center.x, y: 4.6 * unit))
                    let angle = Angle.degrees(Double((i + step) % 8) * 45)
                    let transform = CGAffineTransform(translationX: center.x, y: center.y)
                        .rotated(by: angle.radians)
                        .translatedBy(x: -center.x, y: -center.y)
                    context.stroke(
                        spoke.applying(transform),
                        with: .color((i % 2 == 1 ? accent : color).opacity(0.25 + 0.75 * Double(i) / 7)),
                        style: StrokeStyle(lineWidth: 1.7 * unit, lineCap: .round)
                    )
                }
            }
        }
        .frame(width: size, height: size)
        .accessibilityHidden(true)
    }
}
