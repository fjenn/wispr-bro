// SPDX-License-Identifier: GPL-3.0-or-later
// Draws the keys and turns touches into engine calls.
//
// Ported from Hex (MIT, (c) 2026 Christopher Langton):
// ios/HEXShared/FullKeyboard/KeyboardKeyPlaneView.swift. See ../../NOTICE-hex.md.

import UIKit

/// Draws the keys and turns touches into `WBKeyEngine` calls. The globe key is a real
/// control laid over its slot by `WBFullKeyboardView`; this view only draws its keycap.
@MainActor
final class WBKeyPlaneView: UIView {
    let engine: WBKeyEngine
    var palette = WBKeyPalette(isDark: false) { didSet { if palette != oldValue { setNeedsDisplay() } } }
    var returnLabel = "return" { didSet { if returnLabel != oldValue { setNeedsDisplay(); rebuildAccessibility() } } }
    var returnIsAction = false { didSet { if returnIsAction != oldValue { setNeedsDisplay() } } }
    /// Called on every key going down, for clicks and haptics.
    var onKeyDown: ((WBKeyKind) -> Void)?

    private var touchIDs: [ObjectIdentifier: Int] = [:]
    private var nextTouchID = 0
    private var deleteTimer: Timer?
    private var deleteStartedAt: TimeInterval = 0
    private var holdTimers: [Int: (generation: Int, work: DispatchWorkItem)] = [:]

    /// What the last drawing showed, to redraw only the keys that changed.
    private struct Drawn: Equatable {
        var layout: WBKeyLayout
        var shift: WBShiftState
        var shiftHeld: Bool
        var uppercase: Bool
        var returnEnabled: Bool
    }
    private var drawn: Drawn?
    private var drawnPressed: Set<WBKeyKind> = []
    private var drawnPreview: WBKeyKind?

    init(engine: WBKeyEngine) {
        self.engine = engine
        super.init(frame: .zero)
        isOpaque = false
        backgroundColor = .clear
        isMultipleTouchEnabled = true
        isExclusiveTouch = false
        contentMode = .redraw
        isAccessibilityElement = false
    }

    @available(*, unavailable)
    required init?(coder: NSCoder) { fatalError() }

    override func point(inside point: CGPoint, with event: UIEvent?) -> Bool {
        engine.layout.keysAreaFrame.contains(point)
    }

    // MARK: - Drawing

    private var previewKind: WBKeyKind? {
        engine.layout.showsBalloons ? engine.previewKey?.kind : nil
    }

    /// Call after the engine changed: redraws the keys whose look changed, or all of them
    /// when the page, shift or the layout did.
    func refresh() {
        let now = Drawn(
            layout: engine.layout,
            shift: engine.shift.state,
            shiftHeld: engine.shift.isHeld,
            uppercase: engine.isUppercase,
            returnEnabled: engine.isReturnEnabled
        )
        let pressed = engine.pressedKinds
        let preview = previewKind
        if now != drawn {
            setNeedsDisplay()
            rebuildAccessibility()
        } else {
            var changed = pressed.symmetricDifference(drawnPressed)
            if preview != drawnPreview {
                if let preview { changed.insert(preview) }
                if let drawnPreview { changed.insert(drawnPreview) }
            }
            for kind in changed {
                if let key = now.layout.key(kind) {
                    setNeedsDisplay(key.frame.insetBy(dx: -2, dy: -3))
                }
            }
        }
        drawn = now
        drawnPressed = pressed
        drawnPreview = preview
    }

    override func draw(_ rect: CGRect) {
        guard let context = UIGraphicsGetCurrentContext() else { return }
        let layout = engine.layout
        let pressed = engine.pressedKinds
        let preview = previewKind
        let metrics = layout.metrics
        let radius = metrics.cornerRadius
        let small = layout.orientation == .landscape

        func cap(_ key: WBKey, _ fill: UIColor) {
            context.fillKeycap(key.frame, fill: fill, shadow: palette.keyShadow, radius: radius)
        }

        for key in layout.keys {
            guard key.frame.insetBy(dx: -2, dy: -3).intersects(rect) else { continue }
            switch key.kind {
            case .character(let character):
                // The pressed key is drawn by its balloon in portrait.
                if key.kind == preview { continue }
                cap(key, pressed.contains(key.kind) ? palette.keyPressedFill : palette.keyFill)
                let label = engine.displayLabel(for: key)
                let size: CGFloat
                if character.count > 1 {
                    size = 16
                } else if layout.layer != .letters || key.row == 3 {
                    size = small ? 19 : 21
                } else if engine.isUppercase {
                    size = small ? 19 : 22
                } else {
                    size = small ? 20 : 24
                }
                // Lowercase letters sit a little high in their line box.
                let lift: CGFloat = layout.layer == .letters && !engine.isUppercase && character.count == 1 && key.row < 3 ? -1.5 : 0
                drawText(label, in: key.frame, font: .systemFont(ofSize: size, weight: .regular), color: palette.textBase, yOffset: lift)
            case .space:
                cap(key, pressed.contains(.space) ? palette.keyPressedFill : palette.keyFill)
            case .returnKey:
                let isPressed = pressed.contains(.returnKey)
                let enabled = engine.isReturnEnabled
                let fill: UIColor
                let ink: UIColor
                if returnIsAction && enabled {
                    fill = isPressed ? palette.actionFill.withAlphaComponent(0.7) : palette.actionFill
                    ink = palette.actionText
                } else {
                    fill = isPressed && enabled ? palette.functionLitFill : palette.functionFill
                    ink = enabled ? palette.textBase : palette.textDisabled
                }
                cap(key, fill)
                if returnLabel == "return" {
                    drawSymbol("return", in: key.frame, color: ink, pointSize: small ? 15 : 17, weight: .medium)
                } else {
                    drawText(returnLabel, in: key.frame, font: .systemFont(ofSize: 16, weight: returnIsAction ? .medium : .regular), color: ink)
                }
            case .shift:
                let state = engine.shift.state
                let lit = state != .off || engine.shift.isHeld
                cap(key, palette.flat ? (engine.shift.isHeld ? palette.functionLitFill : palette.functionFill) : (lit ? palette.keyFill : palette.functionFill))
                let symbol = switch state {
                case .off: engine.shift.isHeld ? "shift.fill" : "shift"
                case .on: "shift.fill"
                case .locked: "capslock.fill"
                }
                drawSymbol(symbol, in: key.frame, color: palette.textBase, pointSize: small ? 16 : 18)
            case .delete:
                let lit = pressed.contains(.delete)
                cap(key, lit ? palette.functionLitFill : palette.functionFill)
                drawSymbol(lit ? "delete.left.fill" : "delete.left", in: key.frame, color: palette.textBase, pointSize: small ? 16 : 18)
            case .layerSwitch:
                let lit = pressed.contains(key.kind)
                cap(key, lit ? palette.functionLitFill : palette.functionFill)
                drawText(key.label, in: key.frame, font: .systemFont(ofSize: palette.flat ? 18 : 16), color: palette.textBase)
            case .globe:
                cap(key, palette.functionFill)
                drawSymbol("globe", in: key.frame, color: palette.textBase, pointSize: small ? 16 : 18)
            }
        }
    }

    private func drawText(_ text: String, in frame: CGRect, font: UIFont, color: UIColor, yOffset: CGFloat = 0) {
        let attributes: [NSAttributedString.Key: Any] = [.font: font, .foregroundColor: color]
        let size = (text as NSString).size(withAttributes: attributes)
        let origin = CGPoint(
            x: frame.midX - size.width / 2,
            y: frame.midY - size.height / 2 + yOffset
        )
        (text as NSString).draw(at: origin, withAttributes: attributes)
    }

    private func drawSymbol(_ name: String, in frame: CGRect, color: UIColor, pointSize: CGFloat = 18, weight: UIImage.SymbolWeight = .regular) {
        let config = UIImage.SymbolConfiguration(pointSize: pointSize, weight: weight)
        guard let image = UIImage(systemName: name, withConfiguration: config)?
            .withTintColor(color, renderingMode: .alwaysOriginal) else { return }
        let size = image.size
        image.draw(in: CGRect(
            x: frame.midX - size.width / 2,
            y: frame.midY - size.height / 2,
            width: size.width,
            height: size.height
        ))
    }

    // MARK: - Touches

    private func id(for touch: UITouch, creating: Bool) -> Int? {
        let key = ObjectIdentifier(touch)
        if let id = touchIDs[key] { return id }
        guard creating else { return nil }
        nextTouchID += 1
        touchIDs[key] = nextTouchID
        return nextTouchID
    }

    override func touchesBegan(_ touches: Set<UITouch>, with event: UIEvent?) {
        for touch in touches.sorted(by: { $0.timestamp < $1.timestamp }) {
            guard let id = id(for: touch, creating: true) else { continue }
            let point = touch.location(in: self)
            engine.touchBegan(id: id, at: point)
            if let kind = engine.touches[id]?.key.kind {
                onKeyDown?(kind)
                if kind == .delete { startDeleteRepeat() }
            }
        }
        syncHoldTimers()
    }

    override func touchesMoved(_ touches: Set<UITouch>, with event: UIEvent?) {
        for touch in touches {
            guard let id = id(for: touch, creating: false) else { continue }
            engine.touchMoved(id: id, to: touch.location(in: self))
        }
        syncHoldTimers()
    }

    override func touchesEnded(_ touches: Set<UITouch>, with event: UIEvent?) {
        for touch in touches.sorted(by: { $0.timestamp < $1.timestamp }) {
            guard let id = touchIDs.removeValue(forKey: ObjectIdentifier(touch)) else { continue }
            if engine.touches[id]?.key.kind == .delete { stopDeleteRepeat() }
            engine.touchEnded(id: id, at: touch.location(in: self))
        }
        syncHoldTimers()
    }

    override func touchesCancelled(_ touches: Set<UITouch>, with event: UIEvent?) {
        for touch in touches {
            guard let id = touchIDs.removeValue(forKey: ObjectIdentifier(touch)) else { continue }
            if engine.touches[id]?.key.kind == .delete { stopDeleteRepeat() }
            engine.touchCancelled(id: id)
        }
        syncHoldTimers()
    }

    func cancelAllTouches() {
        touchIDs.removeAll()
        stopDeleteRepeat()
        engine.cancelAllTouches()
        syncHoldTimers()
    }

    // MARK: - Long press

    /// One timer per finger resting on a character key; restarted when it slides to another.
    private func syncHoldTimers() {
        for (id, timer) in holdTimers {
            let touch = engine.touches[id]
            if touch == nil || touch?.committed == true || touch?.hold.state != .pressing
                || touch?.hold.generation != timer.generation {
                timer.work.cancel()
                holdTimers[id] = nil
            }
        }
        for (id, touch) in engine.touches where holdTimers[id] == nil {
            guard touch.key.character != nil, !touch.committed, touch.hold.state == .pressing else { continue }
            let generation = touch.hold.generation
            let work = DispatchWorkItem { [weak self] in
                guard let self else { return }
                self.holdTimers[id] = nil
                self.engine.holdElapsed(id: id, generation: generation)
            }
            holdTimers[id] = (generation, work)
            DispatchQueue.main.asyncAfter(deadline: .now() + WBKeyHoldGesture.holdDuration, execute: work)
        }
    }

    // MARK: - Delete repeat

    private func startDeleteRepeat() {
        stopDeleteRepeat()
        deleteStartedAt = CACurrentMediaTime()
        scheduleDeleteRepeat(after: WBKeyEngine.repeatDelay)
    }

    private func scheduleDeleteRepeat(after delay: TimeInterval) {
        let timer = Timer(timeInterval: delay, repeats: false) { [weak self] _ in
            MainActor.assumeIsolated {
                guard let self, self.deleteTimer != nil else { return }
                let held = CACurrentMediaTime() - self.deleteStartedAt
                self.engine.repeatDelete(heldFor: held)
                self.onKeyDown?(.delete)
                self.scheduleDeleteRepeat(after: WBKeyEngine.repeatInterval(heldFor: held))
            }
        }
        RunLoop.main.add(timer, forMode: .common)
        deleteTimer = timer
    }

    private func stopDeleteRepeat() {
        deleteTimer?.invalidate()
        deleteTimer = nil
    }

    // MARK: - Accessibility

    /// Every key as an element, so VoiceOver (and UI tests) can find and press it.
    private func rebuildAccessibility() {
        do {
            let layout = engine.layout
            let elements: [UIAccessibilityElement] = layout.keys.compactMap { key in
                if key.kind == .globe { return nil }
                let element = WBKeyElement(accessibilityContainer: self)
                element.accessibilityFrameInContainerSpace = key.frame
                element.accessibilityTraits = .keyboardKey
                element.onActivate = { [weak self] in self?.press(key) }
                switch key.kind {
                case .character(let character):
                    element.accessibilityLabel = engine.displayLabel(for: key)
                    element.accessibilityIdentifier = "wb.key.\(character)"
                case .shift:
                    element.accessibilityLabel = engine.shift.state == .locked ? "caps lock" : "shift"
                    element.accessibilityIdentifier = "wb.key.shift"
                case .delete:
                    element.accessibilityLabel = "delete"
                    element.accessibilityIdentifier = "wb.key.delete"
                case .space:
                    element.accessibilityLabel = "space"
                    element.accessibilityIdentifier = "wb.key.space"
                case .returnKey:
                    element.accessibilityLabel = returnLabel
                    element.accessibilityIdentifier = "wb.key.return"
                case .layerSwitch(let layer):
                    element.accessibilityLabel = switch layer {
                    case .letters: "letters"
                    case .numbers: "numbers"
                    case .symbols: "symbols"
                    }
                    element.accessibilityIdentifier = "wb.key.\(key.label)"
                case .globe:
                    break
                }
                return element
            }
            accessibilityElements = elements
        }
    }

    /// A whole tap, for VoiceOver's activation.
    private func press(_ key: WBKey) {
        let point = CGPoint(x: key.frame.midX, y: key.frame.midY)
        engine.touchBegan(id: -100, at: point)
        onKeyDown?(key.kind)
        engine.touchEnded(id: -100, at: point)
    }
}

private final class WBKeyElement: UIAccessibilityElement {
    var onActivate: (() -> Void)?

    override func accessibilityActivate() -> Bool {
        guard let onActivate else { return false }
        onActivate()
        return true
    }
}

/// The pop-up over a pressed key: its balloon in portrait, or the row of alternates
/// after a long press.
@MainActor
final class WBKeyCalloutView: UIView {
    var palette = WBKeyPalette(isDark: false)
    private var balloon: WBKeyBalloon?
    private var label = ""
    private var menu: WBAlternatesMenu?
    private var selected: Int?

    override init(frame: CGRect) {
        super.init(frame: frame)
        isOpaque = false
        backgroundColor = .clear
        isUserInteractionEnabled = false
        contentMode = .redraw
        isAccessibilityElement = false
    }

    @available(*, unavailable)
    required init?(coder: NSCoder) { fatalError() }

    func show(_ balloon: WBKeyBalloon?, label: String) {
        guard balloon != self.balloon || label != self.label || menu != nil else { return }
        self.balloon = balloon
        self.label = label
        menu = nil
        setNeedsDisplay()
    }

    func show(_ menu: WBAlternatesMenu, selected: Int?) {
        guard menu != self.menu || selected != self.selected else { return }
        self.menu = menu
        self.selected = selected
        balloon = nil
        setNeedsDisplay()
    }

    override func draw(_ rect: CGRect) {
        guard let context = UIGraphicsGetCurrentContext() else { return }
        if let menu {
            drawMenu(menu, in: context)
        } else if let balloon {
            fill(Self.path(for: balloon), in: context)
            draw(label, in: balloon.bubble, size: palette.flat ? 34 : 36, weight: palette.flat ? .regular : .light, color: palette.textBase)
        }
    }

    private func fill(_ path: CGPath, in context: CGContext) {
        context.saveGState()
        context.setShadow(offset: CGSize(width: 0, height: 1), blur: 4, color: palette.popupShadow.cgColor)
        context.addPath(path)
        context.setFillColor(palette.popupFill.cgColor)
        context.fillPath()
        context.restoreGState()
    }

    private func draw(_ text: String, in rect: CGRect, size: CGFloat, weight: UIFont.Weight, color: UIColor) {
        let attributes: [NSAttributedString.Key: Any] = [
            .font: UIFont.systemFont(ofSize: size, weight: weight),
            .foregroundColor: color,
        ]
        let bounds = (text as NSString).size(withAttributes: attributes)
        (text as NSString).draw(
            at: CGPoint(x: rect.midX - bounds.width / 2, y: rect.midY - bounds.height / 2),
            withAttributes: attributes
        )
    }

    private func drawMenu(_ menu: WBAlternatesMenu, in context: CGContext) {
        if menu.joined {
            fill(Self.path(for: menu.balloon), in: context)
        } else {
            fill(UIBezierPath(roundedRect: menu.bubble, cornerRadius: 10).cgPath, in: context)
        }
        for (index, option) in menu.options.enumerated() {
            let cell = menu.cellFrame(index)
            let chosen = index == selected
            if chosen {
                palette.selectFill.setFill()
                UIBezierPath(roundedRect: cell, cornerRadius: 7).fill()
            }
            draw(
                option,
                in: cell,
                size: menu.joined ? 24 : 20,
                weight: .regular,
                color: chosen ? palette.selectText : palette.textBase
            )
        }
    }

    /// The mockup's balloon: the key at the bottom, a wider bubble above, joined by a neck.
    static func path(for balloon: WBKeyBalloon) -> CGPath {
        let kr = WBKeyPalette.cornerRadius
        let br: CGFloat = 11
        let neck = WBKeyBalloon.neck
        let kl = balloon.keyFrame.minX
        let krt = balloon.keyFrame.maxX
        let kb = balloon.keyFrame.maxY
        let kt = balloon.keyFrame.minY
        let bb = balloon.bubble.maxY
        let bt = balloon.bubble.minY
        let bl = balloon.bubble.minX
        let brt = balloon.bubble.maxX

        let p = CGMutablePath()
        p.move(to: CGPoint(x: bl, y: bt + br))
        p.addQuadCurve(to: CGPoint(x: bl + br, y: bt), control: CGPoint(x: bl, y: bt))
        p.addLine(to: CGPoint(x: brt - br, y: bt))
        p.addQuadCurve(to: CGPoint(x: brt, y: bt + br), control: CGPoint(x: brt, y: bt))
        // Each side: a rounded shoulder into the neck when the bubble overhangs the key,
        // or a straight taper when an edge key's bubble is pushed flush with it.
        if brt - br >= krt + 2 {
            p.addLine(to: CGPoint(x: brt, y: bb - br))
            p.addQuadCurve(to: CGPoint(x: brt - br, y: bb), control: CGPoint(x: brt, y: bb))
            p.addLine(to: CGPoint(x: min(brt - br, krt + 6), y: bb))
            p.addCurve(
                to: CGPoint(x: krt, y: kt + 6),
                control1: CGPoint(x: krt + 1, y: bb),
                control2: CGPoint(x: krt, y: bb + neck * 0.4)
            )
        } else {
            p.addLine(to: CGPoint(x: brt, y: bb))
            p.addLine(to: CGPoint(x: krt, y: kt + 4))
        }
        p.addLine(to: CGPoint(x: krt, y: kb - kr))
        p.addQuadCurve(to: CGPoint(x: krt - kr, y: kb), control: CGPoint(x: krt, y: kb))
        p.addLine(to: CGPoint(x: kl + kr, y: kb))
        p.addQuadCurve(to: CGPoint(x: kl, y: kb - kr), control: CGPoint(x: kl, y: kb))
        if bl + br <= kl - 2 {
            p.addLine(to: CGPoint(x: kl, y: kt + 6))
            p.addCurve(
                to: CGPoint(x: max(bl + br, kl - 6), y: bb),
                control1: CGPoint(x: kl, y: bb + neck * 0.4),
                control2: CGPoint(x: kl - 1, y: bb)
            )
            p.addLine(to: CGPoint(x: bl + br, y: bb))
            p.addQuadCurve(to: CGPoint(x: bl, y: bb - br), control: CGPoint(x: bl, y: bb))
        } else {
            p.addLine(to: CGPoint(x: kl, y: kt + 4))
            p.addLine(to: CGPoint(x: bl, y: bb))
        }
        p.closeSubpath()
        return p
    }
}
