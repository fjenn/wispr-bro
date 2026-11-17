// SPDX-License-Identifier: GPL-3.0-or-later
// Wispr Bro's design tokens for iOS (the Mac app's pill and palette).

import SwiftUI
import UIKit

extension Color {
    init(wbHex hex: UInt32, opacity: Double = 1) {
        self.init(
            .sRGB,
            red: Double((hex >> 16) & 0xFF) / 255,
            green: Double((hex >> 8) & 0xFF) / 255,
            blue: Double(hex & 0xFF) / 255,
            opacity: opacity
        )
    }

    init(wbLight light: UInt32, dark: UInt32) {
        self.init(UIColor { traits in
            UIColor(wbHex: traits.userInterfaceStyle == .dark ? dark : light)
        })
    }
}

extension UIColor {
    convenience init(wbHex hex: UInt32, alpha: CGFloat = 1) {
        self.init(
            red: CGFloat((hex >> 16) & 0xFF) / 255,
            green: CGFloat((hex >> 8) & 0xFF) / 255,
            blue: CGFloat(hex & 0xFF) / 255,
            alpha: alpha
        )
    }
}

enum WBTheme {
    // The listening capsule (same in light and dark).
    static let pill = Color.black
    static let pillInk = Color.white
    static let pillControl = Color(wbHex: 0x30302F)
    static let pillCard = Color(wbHex: 0x1B1B1B)

    // Surfaces.
    static let ink = Color(wbLight: 0x1A1A1A, dark: 0xF3F2EE)
    static let panel = Color(wbLight: 0xFCFCFA, dark: 0x161615)
    static let chrome = Color(wbLight: 0xF5F4F0, dark: 0x1C1C1B)
    static let cream = Color(wbHex: 0xF0EDE6)
    static let accent = Color(wbLight: 0x6A37A0, dark: 0xC3A0EE)

    // Radii.
    static let r1: CGFloat = 5
    static let r2: CGFloat = 8
    static let r3: CGFloat = 12
    static let r4: CGFloat = 16
    static let r5: CGFloat = 22

    // Motion.
    static func easeOut(_ duration: Double) -> Animation {
        .timingCurve(0.22, 1, 0.36, 1, duration: duration)
    }
    static let fast = easeOut(0.09)
    static let quick = easeOut(0.14)
    static let medium = easeOut(0.22)
    static let slow = easeOut(0.32)
    /// Size changes of the capsule.
    static let spring = Animation.interpolatingSpring(mass: 0.8, stiffness: 600, damping: 38)

    static let pillFont = Font.system(size: 15, weight: .medium)
}
