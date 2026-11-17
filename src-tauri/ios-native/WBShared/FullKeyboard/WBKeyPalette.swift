// SPDX-License-Identifier: GPL-3.0-or-later
// The typing keys' colours, light and dark.
//
// Ported from Hex (MIT, (c) 2026 Christopher Langton):
// ios/HEXShared/FullKeyboard/FullKeyboardPalette.swift. See ../../NOTICE-hex.md.

import UIKit

/// The typing keys' colours, light and dark, after the system keyboard. Dark is chosen by the field's
/// `keyboardAppearance` first, then the host's interface style.
struct WBKeyPalette: Equatable {
    let isDark: Bool
    /// iOS 26 and later: every key has the same flat fill and there are no key shadows.
    /// Earlier systems keep grey function keys and the one-point shadow.
    var flat: Bool = {
        if #available(iOS 26, *) { return true }
        return false
    }()

    init(isDark: Bool) {
        self.isDark = isDark
    }

    static func isDark(appearance: UIKeyboardAppearance?, traits: UITraitCollection) -> Bool {
        switch appearance {
        case .dark?:
            return true
        case .light?:
            return false
        default:
            return traits.userInterfaceStyle == .dark
        }
    }

    private static func rgb(_ hex: UInt32, alpha: CGFloat = 1) -> UIColor {
        UIColor(
            red: CGFloat((hex >> 16) & 0xFF) / 255,
            green: CGFloat((hex >> 8) & 0xFF) / 255,
            blue: CGFloat(hex & 0xFF) / 255,
            alpha: alpha
        )
    }

    /// Stand-in for the system keyboard backdrop, used only where no system backdrop exists.
    var backdrop: UIColor { isDark ? Self.rgb(0x1D1E21) : Self.rgb(0xD3D5DB) }

    var textBase: UIColor { isDark ? UIColor(white: 1, alpha: 0.96) : (flat ? .black : Self.rgb(0x14161B)) }
    var textMuted: UIColor { isDark ? UIColor(white: 1, alpha: 0.56) : Self.rgb(0x4E5461) }
    var textDisabled: UIColor { isDark ? UIColor(white: 1, alpha: 0.3) : UIColor(white: 0, alpha: 0.3) }

    var keyFill: UIColor {
        if flat { return isDark ? UIColor(white: 1, alpha: 0.19) : .white }
        return isDark ? UIColor(white: 1, alpha: 0.135) : .white
    }
    /// Shift, delete, 123, return at rest.
    var functionFill: UIColor {
        if flat { return keyFill }
        return isDark ? UIColor(white: 1, alpha: 0.06) : UIColor(white: 0, alpha: 0.11)
    }
    /// A function key under a finger, and shift while on.
    var functionLitFill: UIColor {
        if flat { return keyPressedFill }
        return keyFill
    }
    /// A character key under a finger where no balloon shows (landscape, space).
    var keyPressedFill: UIColor {
        if flat { return isDark ? UIColor(white: 1, alpha: 0.4) : UIColor(white: 0.74, alpha: 1) }
        return isDark ? UIColor(white: 1, alpha: 0.06) : UIColor(white: 0, alpha: 0.11)
    }
    var keyShadow: UIColor { flat ? .clear : UIColor(white: 0, alpha: isDark ? 0.30 : 0.16) }

    /// Opaque so it can float over the strip and neighbouring keys.
    var popupFill: UIColor { isDark ? Self.rgb(0x45474C) : .white }
    var popupShadow: UIColor { UIColor(white: 0, alpha: isDark ? 0.45 : 0.22) }

    /// The action return key (Go, Search, Send, Done): Wispr Bro's accent.
    var actionFill: UIColor { isDark ? Self.rgb(0xC3A0EE) : Self.rgb(0x6A37A0) }
    var actionText: UIColor { isDark ? Self.rgb(0x1A1026) : .white }
    /// The chosen alternate in a long-press row.
    var selectFill: UIColor { actionFill }
    var selectText: UIColor { actionText }

    static let cornerRadius: CGFloat = 8.5
}

extension CGContext {
    /// A keycap: rounded rectangle with Apple's one-point drop shadow.
    func fillKeycap(_ rect: CGRect, fill: UIColor, shadow: UIColor, radius: CGFloat = WBKeyPalette.cornerRadius) {
        let path = UIBezierPath(roundedRect: rect, cornerRadius: radius).cgPath
        guard shadow.cgColor.alpha > 0 else {
            saveGState()
            addPath(path)
            setFillColor(fill.cgColor)
            fillPath()
            restoreGState()
            return
        }
        saveGState()
        setShadow(offset: CGSize(width: 0, height: 1), blur: 0.5, color: shadow.cgColor)
        // A translucent fill would let its own shadow show through; draw the shadow
        // from an opaque copy clipped to outside the key, then the fill on top.
        beginTransparencyLayer(auxiliaryInfo: nil)
        addPath(path)
        setFillColor(UIColor.black.cgColor)
        fillPath()
        endTransparencyLayer()
        restoreGState()

        saveGState()
        addPath(path)
        setBlendMode(.copy)
        setFillColor(UIColor.clear.cgColor)
        fillPath()
        restoreGState()

        saveGState()
        addPath(path)
        setFillColor(fill.cgColor)
        fillPath()
        restoreGState()
    }
}
