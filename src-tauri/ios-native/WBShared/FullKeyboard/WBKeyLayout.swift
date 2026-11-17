// SPDX-License-Identifier: GPL-3.0-or-later
// Pure geometry of the typing keys.
//
// Ported from Hex (MIT, (c) 2026 Christopher Langton):
// ios/HEXShared/FullKeyboard/KeyboardLayout.swift. See ../../NOTICE-hex.md.

import CoreGraphics

enum WBKeyLayer: Equatable, Hashable, Sendable {
    case letters
    case numbers
    case symbols
}

/// The kind of text field, as far as the key rows care.
enum WBFieldKind: Equatable, Sendable {
    case text
    /// Adds `/` and `.` beside space.
    case url
    /// Adds `@` and `.` beside space.
    case email
    /// Adds `.` right of space (search-or-address fields).
    case webSearch
}

enum WBKeyboardOrientation: Equatable, Sendable {
    case portrait
    case landscape
}

enum WBKeyKind: Equatable, Hashable, Sendable {
    /// Inserts its text (letters are stored lowercase; shift decides the case).
    case character(String)
    case shift
    case layerSwitch(WBKeyLayer)
    case delete
    case space
    case returnKey
    case globe
}

struct WBKey: Equatable, Sendable {
    let kind: WBKeyKind
    let row: Int
    /// The drawn keycap.
    let frame: CGRect
    /// The touch target. Hit frames tile the whole key area with no gaps.
    var hitFrame: CGRect
    let label: String

    var character: String? {
        if case .character(let text) = kind { return text }
        return nil
    }
}

/// The long-press pop-up of a key: its alternates (accents) in one row. Wispr Bro.
struct WBAlternatesMenu: Equatable, Sendable {
    static let padding: CGFloat = 5

    let keyFrame: CGRect
    let bubble: CGRect
    /// Left to right, as drawn.
    let options: [String]
    /// The option over the key: chosen when the finger never slides.
    let defaultIndex: Int
    /// Portrait: the bubble is joined to the key like a balloon. Landscape: it floats.
    let joined: Bool

    var cellWidth: CGFloat { (bubble.width - 2 * Self.padding) / CGFloat(max(1, options.count)) }

    func cellFrame(_ index: Int) -> CGRect {
        CGRect(
            x: bubble.minX + Self.padding + CGFloat(index) * cellWidth,
            y: bubble.minY + Self.padding,
            width: cellWidth,
            height: bubble.height - 2 * Self.padding
        )
    }

    /// The option under the finger. The row follows the finger sideways from anywhere
    /// near it; far above or below, nothing is chosen.
    func index(at point: CGPoint) -> Int? {
        guard point.y > bubble.minY - 60, point.y < keyFrame.maxY + 50 else { return nil }
        let raw = Int(((point.x - bubble.minX - Self.padding) / cellWidth).rounded(.down))
        return max(0, min(options.count - 1, raw))
    }

    var balloon: WBKeyBalloon { WBKeyBalloon(keyFrame: keyFrame, bubble: bubble) }
}

/// The pop-up balloon over a pressed key: a wider bubble joined to the key by a neck.
struct WBKeyBalloon: Equatable, Sendable {
    static let bubbleHeight: CGFloat = 52
    static let neck: CGFloat = 10
    static let extraWidth: CGFloat = 22

    let keyFrame: CGRect
    let bubble: CGRect

    /// Bubble centre minus key centre; nonzero when an edge key's bubble is pushed inward.
    var offsetX: CGFloat { bubble.midX - keyFrame.midX }
    /// The balloon's whole outline, bubble through key.
    var bounds: CGRect { bubble.union(keyFrame) }
}

struct WBKeyMetrics: Equatable, Sendable {
    let side: CGFloat
    let gap: CGFloat
    let rowGap: CGFloat
    let keyHeight: CGFloat
    let stripTop: CGFloat
    let stripHeight: CGFloat
    let stripBottom: CGFloat
    let keysTop: CGFloat
    let keysBottom: CGFloat
    let cornerRadius: CGFloat

    /// 54 pt strip area + 216 pt of keys = 270 pt on every portrait iPhone width.
    /// The strip area is exactly as tall as a top-row balloon (52 + 10 - 8). Keys and
    /// gaps are the system keyboard's (42 pt keys on a 54 pt pitch, 6 pt at the sides).
    static let portrait = WBKeyMetrics(
        side: 6, gap: 6, rowGap: 12, keyHeight: 42,
        stripTop: 6, stripHeight: 44, stripBottom: 4,
        keysTop: 8, keysBottom: 4, cornerRadius: 9
    )

    /// 40 pt strip area + 150 pt of keys = 190 pt.
    static let landscape = WBKeyMetrics(
        side: 3, gap: 6, rowGap: 7, keyHeight: 32,
        stripTop: 2, stripHeight: 36, stripBottom: 2,
        keysTop: 1, keysBottom: 0, cornerRadius: 7
    )

    static func `for`(_ orientation: WBKeyboardOrientation) -> WBKeyMetrics {
        orientation == .portrait ? .portrait : .landscape
    }

    var stripArea: CGFloat { stripTop + stripHeight + stripBottom }
    var keysArea: CGFloat { keysTop + 4 * keyHeight + 3 * rowGap + keysBottom }
    var totalHeight: CGFloat { stripArea + keysArea }
}

/// Pure geometry of the full keyboard: every key's frame and touch target for one width,
/// layer and field kind. Coordinates are in the keyboard view, strip included.
struct WBKeyLayout: Equatable, Sendable {
    static let modifierWidthRatio: CGFloat = 1.32
    static let returnWidthRatio: CGFloat = 2.55
    static let dotComWidthRatio: CGFloat = 1.7

    let width: CGFloat
    let orientation: WBKeyboardOrientation
    let layer: WBKeyLayer
    let field: WBFieldKind
    let showsGlobe: Bool
    let horizontalInset: CGFloat
    let metrics: WBKeyMetrics
    let keys: [WBKey]

    init(
        width: CGFloat,
        orientation: WBKeyboardOrientation = .portrait,
        layer: WBKeyLayer = .letters,
        field: WBFieldKind = .text,
        showsGlobe: Bool = false,
        horizontalInset: CGFloat = 0
    ) {
        self.width = width
        self.orientation = orientation
        self.layer = layer
        self.field = field
        self.showsGlobe = showsGlobe
        self.horizontalInset = max(0, horizontalInset)
        let metrics = WBKeyMetrics.for(orientation)
        self.metrics = metrics
        self.keys = Self.buildKeys(
            width: width,
            metrics: metrics,
            layer: layer,
            field: field,
            showsGlobe: showsGlobe,
            inset: self.horizontalInset
        )
    }

    static func height(for orientation: WBKeyboardOrientation) -> CGFloat {
        WBKeyMetrics.for(orientation).totalHeight
    }

    var totalHeight: CGFloat { metrics.totalHeight }

    /// Where the dictation strip sits.
    var stripFrame: CGRect {
        CGRect(x: 0, y: metrics.stripTop, width: width, height: metrics.stripHeight)
    }

    var keysAreaFrame: CGRect {
        CGRect(x: 0, y: metrics.stripArea, width: width, height: metrics.keysArea)
    }

    var letterWidth: CGFloat {
        Self.letterWidth(innerWidth: width - 2 * horizontalInset, metrics: metrics)
    }

    var showsBalloons: Bool { orientation == .portrait }

    func key(_ kind: WBKeyKind) -> WBKey? {
        keys.first { $0.kind == kind }
    }

    func hitTest(_ point: CGPoint) -> WBKey? {
        keys.first { $0.hitFrame.contains(point) }
    }

    /// The balloon for a character key, kept inside the keyboard's bounds. `nil` for keys
    /// without one (space, modifiers) and in landscape, where there is no room above row 1.
    func balloon(for key: WBKey) -> WBKeyBalloon? {
        guard showsBalloons, let character = key.character, character.count == 1 else { return nil }
        let bubbleWidth = key.frame.width + WBKeyBalloon.extraWidth
        let bottom = key.frame.minY - WBKeyBalloon.neck
        var x = key.frame.midX - bubbleWidth / 2
        x = max(0, min(width - bubbleWidth, x))
        let bubble = CGRect(
            x: x,
            y: bottom - WBKeyBalloon.bubbleHeight,
            width: bubbleWidth,
            height: WBKeyBalloon.bubbleHeight
        )
        return WBKeyBalloon(keyFrame: key.frame, bubble: bubble)
    }

    /// The long-press row for a character key. `options` come most wanted first; that one
    /// sits over the key and the rest run toward the middle of the keyboard.
    func alternatesMenu(for key: WBKey, options: [String]) -> WBAlternatesMenu? {
        guard key.character != nil, !options.isEmpty else { return nil }
        let pad = WBAlternatesMenu.padding
        let cell = min(max(30, min(40, key.frame.width + metrics.gap)), (width - 2 * pad) / CGFloat(options.count))
        let bubbleWidth = CGFloat(options.count) * cell + 2 * pad
        let growsRight = key.frame.midX < width / 2
        var x = growsRight
            ? key.frame.midX - cell / 2 - pad
            : key.frame.midX + cell / 2 + pad - bubbleWidth
        x = max(0, min(width - bubbleWidth, x))
        let bubble: CGRect
        if showsBalloons {
            bubble = CGRect(
                x: x,
                y: key.frame.minY - WBKeyBalloon.neck - WBKeyBalloon.bubbleHeight,
                width: bubbleWidth,
                height: WBKeyBalloon.bubbleHeight
            )
        } else {
            let height = metrics.keyHeight + 6
            bubble = CGRect(x: x, y: max(0, key.frame.minY - 3 - height), width: bubbleWidth, height: height)
        }
        return WBAlternatesMenu(
            keyFrame: key.frame,
            bubble: bubble,
            options: growsRight ? options : options.reversed(),
            defaultIndex: growsRight ? 0 : options.count - 1,
            joined: showsBalloons
        )
    }

    // MARK: - Building

    static func letterWidth(innerWidth: CGFloat, metrics: WBKeyMetrics) -> CGFloat {
        (innerWidth - 2 * metrics.side - 9 * metrics.gap) / 10
    }

    static func characterRows(for layer: WBKeyLayer) -> [[String]] {
        let punctuation = [".", ",", "?", "!", "'"]
        switch layer {
        case .letters:
            return ["qwertyuiop", "asdfghjkl", "zxcvbnm"].map { $0.map(String.init) }
        case .numbers:
            return [
                "1234567890".map(String.init),
                ["-", "/", ":", ";", "(", ")", "$", "&", "@", "\""],
                punctuation,
            ]
        case .symbols:
            return [
                ["[", "]", "{", "}", "#", "%", "^", "*", "+", "="],
                ["_", "\\", "|", "~", "<", ">", "€", "£", "¥", "•"],
                punctuation,
            ]
        }
    }

    private static func buildKeys(
        width: CGFloat,
        metrics m: WBKeyMetrics,
        layer: WBKeyLayer,
        field: WBFieldKind,
        showsGlobe: Bool,
        inset: CGFloat
    ) -> [WBKey] {
        let innerWidth = width - 2 * inset
        let lw = letterWidth(innerWidth: innerWidth, metrics: m)
        let modW = lw * modifierWidthRatio
        let rows = characterRows(for: layer)
        let left = inset + m.side
        let right = width - inset - m.side

        func rowY(_ row: Int) -> CGFloat {
            m.stripArea + m.keysTop + CGFloat(row) * (m.keyHeight + m.rowGap)
        }

        var drawn: [(WBKeyKind, Int, CGRect, String)] = []

        func add(_ kind: WBKeyKind, row: Int, x: CGFloat, w: CGFloat, label: String) {
            drawn.append((kind, row, CGRect(x: x, y: rowY(row), width: w, height: m.keyHeight), label))
        }

        func addCharacters(_ chars: [String], row: Int, startX: CGFloat, keyWidth: CGFloat) {
            var x = startX
            for ch in chars {
                add(.character(ch), row: row, x: x, w: keyWidth, label: ch)
                x += keyWidth + m.gap
            }
        }

        func centredStart(count: Int, keyWidth: CGFloat) -> CGFloat {
            let total = CGFloat(count) * keyWidth + CGFloat(count - 1) * m.gap
            return inset + (innerWidth - total) / 2
        }

        // Rows 1 and 2.
        for row in 0..<2 {
            let chars = rows[row]
            addCharacters(chars, row: row, startX: centredStart(count: chars.count, keyWidth: lw), keyWidth: lw)
        }

        // Row 3: shift (or the symbol-page switch), the middle keys, delete.
        switch layer {
        case .letters:
            add(.shift, row: 2, x: left, w: modW, label: "shift")
        case .numbers:
            add(.layerSwitch(.symbols), row: 2, x: left, w: modW, label: "#+=")
        case .symbols:
            add(.layerSwitch(.numbers), row: 2, x: left, w: modW, label: "123")
        }
        let middle = rows[2]
        if layer == .letters {
            addCharacters(middle, row: 2, startX: centredStart(count: middle.count, keyWidth: lw), keyWidth: lw)
        } else {
            // Apple's punctuation row: five wide keys between the modifiers.
            let edgeGap = 1.5 * (lw + m.gap) - modW
            let count = CGFloat(middle.count)
            let middleW = (innerWidth - 2 * m.side - 2 * modW - 2 * edgeGap - (count - 1) * m.gap) / count
            addCharacters(middle, row: 2, startX: left + modW + edgeGap, keyWidth: middleW)
        }
        add(.delete, row: 2, x: right - modW, w: modW, label: "delete")

        // Row 4: 123/ABC, globe?, field extras, space, return. With nothing else in
        // the row, 123 is as wide as return (Apple's row without an emoji key).
        let returnW = lw * returnWidthRatio + m.gap
        let plainRow = field == .text && !showsGlobe
        var leading: [(WBKeyKind, CGFloat, String)] = [
            layer == .letters
                ? (.layerSwitch(.numbers), plainRow ? returnW : modW, "123")
                : (.layerSwitch(.letters), plainRow ? returnW : modW, "ABC"),
        ]
        if showsGlobe {
            leading.append((.globe, modW, "globe"))
        }
        var trailing: [(WBKeyKind, CGFloat, String)] = []
        switch field {
        case .text:
            break
        case .url:
            leading.append((.character("/"), lw, "/"))
            trailing.append((.character("."), lw, "."))
            trailing.append((.character(".com"), lw * dotComWidthRatio, ".com"))
        case .email:
            leading.append((.character("@"), lw, "@"))
            trailing.append((.character("."), lw, "."))
        case .webSearch:
            trailing.append((.character("."), lw, "."))
        }
        trailing.append((.returnKey, returnW, "return"))

        let fixed = (leading + trailing).reduce(0) { $0 + $1.1 }
        let itemCount = CGFloat(leading.count + trailing.count + 1)
        let spaceW = max(lw, (right - left) - fixed - (itemCount - 1) * m.gap)
        var x = left
        for (kind, w, label) in leading {
            add(kind, row: 3, x: x, w: w, label: label)
            x += w + m.gap
        }
        add(.space, row: 3, x: x, w: spaceW, label: "space")
        x += spaceW + m.gap
        for (kind, w, label) in trailing {
            add(kind, row: 3, x: x, w: w, label: label)
            x += w + m.gap
        }

        // Touch targets: split every gap down the middle; outer keys reach the edges.
        let top = m.stripArea
        let bottom = m.totalHeight
        var keys: [WBKey] = []
        for row in 0..<4 {
            let rowKeys = drawn.filter { $0.1 == row }.sorted { $0.2.minX < $1.2.minX }
            let y0 = row == 0 ? top : rowY(row) - m.rowGap / 2
            let y1 = row == 3 ? bottom : rowY(row + 1) - m.rowGap / 2
            for (i, item) in rowKeys.enumerated() {
                let x0 = i == 0 ? 0 : (rowKeys[i - 1].2.maxX + item.2.minX) / 2
                let x1 = i == rowKeys.count - 1 ? width : (item.2.maxX + rowKeys[i + 1].2.minX) / 2
                keys.append(WBKey(
                    kind: item.0,
                    row: row,
                    frame: item.2,
                    hitFrame: CGRect(x: x0, y: y0, width: x1 - x0, height: y1 - y0),
                    label: item.3
                ))
            }
        }
        return keys
    }
}
