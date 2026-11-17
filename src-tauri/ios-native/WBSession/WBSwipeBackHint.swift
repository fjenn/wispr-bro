// SPDX-License-Identifier: GPL-3.0-or-later
// The full-screen "swipe back" hint shown when the app cannot send the user
// back to the app they were typing in.
//
// After Hex's home-indicator swipe cue (MIT, (c) 2026 Christopher Langton):
// ios/HEXShared/HomeSwipeCue.swift. See ../NOTICE-hex.md. Redrawn in SwiftUI
// (no Metal): a row of dots along the home indicator with a highlight that
// sweeps to the right.

import SwiftUI
import UIKit

struct WBSwipeBackHintView: View {
    var onDismiss: () -> Void = {}
    /// For still previews.
    var frozenPhase: Double? = nil

    @Environment(\.accessibilityReduceMotion) private var reduceMotion

    var body: some View {
        ZStack {
            Color.black.ignoresSafeArea()

            VStack(spacing: 0) {
                Spacer()
                WBBrandMark()
                    .fill(Color.white)
                    .frame(width: 44, height: 44)
                    .frame(width: 84, height: 84)
                    .background(Color(wbHex: 0x1B1B1B), in: Circle())
                    .padding(.bottom, 28)
                Text("Wispr Bro is listening for you")
                    .font(.system(size: 26, weight: .semibold))
                    .foregroundStyle(.white)
                    .multilineTextAlignment(.center)
                Text("Swipe back to keep dictating")
                    .font(.system(size: 17))
                    .foregroundStyle(Color.white.opacity(0.62))
                    .padding(.top, 8)
                Spacer()
                Button(action: onDismiss) {
                    Text("Stay in Wispr Bro")
                        .font(.system(size: 15, weight: .medium))
                        .foregroundStyle(.white)
                        .padding(.horizontal, 18)
                        .frame(height: 40)
                        .background(Color(wbHex: 0x30302F), in: Capsule())
                }
                .padding(.bottom, 34)

                Image(systemName: "arrow.right")
                    .font(.system(size: 15, weight: .semibold))
                    .foregroundStyle(Color.white.opacity(0.62))
                    .padding(.bottom, 10)
                sweep
                    .frame(height: 12)
                    .padding(.horizontal, 28)
                    .padding(.bottom, 6)
            }
            .padding(.horizontal, 24)
        }
        .preferredColorScheme(.dark)
        .accessibilityElement(children: .combine)
        .accessibilityLabel("Wispr Bro is listening for you. Swipe right along the bottom edge to go back and keep dictating.")
    }

    private var sweep: some View {
        TimelineView(.animation(paused: reduceMotion || frozenPhase != nil)) { timeline in
            Canvas { context, size in
                let period = 1.3
                let t = frozenPhase ?? (reduceMotion
                    ? 0.5
                    : timeline.date.timeIntervalSinceReferenceDate.truncatingRemainder(dividingBy: period) / period)
                let pitch: CGFloat = 9
                let count = max(8, Int(size.width / pitch))
                let head = -0.2 + 1.4 * t
                for i in 0..<count {
                    let x = (CGFloat(i) + 0.5) * size.width / CGFloat(count)
                    let position = Double(i) / Double(count - 1)
                    let behind = head - position
                    let glow = behind >= 0 ? max(0, 1 - behind / 0.35) : 0
                    let radius = 1.6 + 1.4 * glow
                    context.fill(
                        Path(ellipseIn: CGRect(x: x - radius, y: size.height / 2 - radius, width: radius * 2, height: radius * 2)),
                        with: .color(Color.white.opacity(0.22 + 0.78 * glow))
                    )
                }
            }
        }
        .accessibilityHidden(true)
    }
}

/// Shows the hint in its own window above the app (the app's UI is a web view
/// this code knows nothing about).
@MainActor
final class WBSwipeBackHintPresenter {
    private var window: UIWindow?

    var isVisible: Bool { window != nil }

    func show() {
        guard window == nil,
              let scene = UIApplication.shared.connectedScenes
                .compactMap({ $0 as? UIWindowScene })
                .first(where: { $0.activationState != .background }) ?? UIApplication.shared.connectedScenes
                .compactMap({ $0 as? UIWindowScene }).first
        else { return }
        let host = UIHostingController(rootView: WBSwipeBackHintView(onDismiss: { [weak self] in self?.hide() }))
        host.view.backgroundColor = .black
        let window = UIWindow(windowScene: scene)
        window.windowLevel = .alert + 1
        window.rootViewController = host
        window.alpha = 0
        window.isHidden = false
        UIView.animate(withDuration: 0.22) { window.alpha = 1 }
        self.window = window
    }

    func hide() {
        guard let window else { return }
        self.window = nil
        UIView.animate(withDuration: 0.14, animations: { window.alpha = 0 }) { _ in
            window.isHidden = true
        }
    }
}
