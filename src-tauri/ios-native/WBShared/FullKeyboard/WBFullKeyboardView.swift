// SPDX-License-Identifier: GPL-3.0-or-later
// The keyboard: the dictation strip over the typing keys.
//
// Ported from Hex (MIT, (c) 2026 Christopher Langton):
// ios/HEXShared/FullKeyboard/FullKeyboardView.swift (there the dictation key
// sits right of space and a panel replaces the keys while recording; here
// dictation lives in the strip and the keys stay). See ../../NOTICE-hex.md.

import Combine
import SwiftUI
import UIKit

/// The strip (`WBDictationStrip`, fixed height) and QWERTY keys below it. Dictation
/// state comes from `WBKeyboardModel`; the keys type through a `WBTextTarget`.
@MainActor
final class WBFullKeyboardView: UIView, UIInputViewAudioFeedback {
    let model: WBKeyboardModel
    let engine: WBKeyEngine
    weak var hostController: UIInputViewController? {
        didSet { configureHost() }
    }

    /// Previews force light or dark and portrait or landscape; the keyboard follows its field.
    var appearanceOverride: Bool? { didSet { applyAppearance() } }
    var orientationOverride: WBKeyboardOrientation? { didSet { setNeedsLayout() } }
    /// Haptics need Full Access.
    var allowsHaptics: () -> Bool = { false }
    /// Draws a stand-in backdrop where there is no system keyboard behind the view.
    var drawsBackdrop = false { didSet { applyAppearance() } }

    private let plane: WBKeyPlaneView
    private let callout = WBKeyCalloutView()
    private let globeButton = UIButton(type: .custom)
    private let strip: UIHostingController<WBDictationStrip>
    private var palette = WBKeyPalette(isDark: false)
    private var didApplyPalette = false
    private var subscriptions: Set<AnyCancellable> = []
    private let haptics = UIImpactFeedbackGenerator(style: .light)

    init(model: WBKeyboardModel, target: WBTextTarget) {
        self.model = model
        engine = WBKeyEngine(target: target)
        plane = WBKeyPlaneView(engine: engine)
        strip = UIHostingController(rootView: WBDictationStrip(model: model))
        super.init(frame: .zero)
        backgroundColor = .clear

        strip.view.backgroundColor = .clear
        strip.safeAreaRegions = []
        addSubview(strip.view)
        addSubview(plane)
        addSubview(globeButton)
        addSubview(callout)

        globeButton.accessibilityLabel = "Next keyboard"
        globeButton.isAccessibilityElement = true

        plane.onKeyDown = { [weak self] _ in self?.keyFeedback() }
        engine.onChange = { [weak self] in self?.engineChanged() }

        model.$needsGlobeKey
            .receive(on: RunLoop.main)
            .sink { [weak self] _ in self?.refreshFromModel() }
            .store(in: &subscriptions)

        registerForTraitChanges([UITraitUserInterfaceStyle.self, UITraitVerticalSizeClass.self]) { (view: WBFullKeyboardView, _) in
            view.applyAppearance()
            view.setNeedsLayout()
        }

        engine.configureForField()
        refreshReturnKey()
        applyAppearance()
        refreshFromModel()
    }

    @available(*, unavailable)
    required init?(coder: NSCoder) { fatalError() }

    // MARK: - Size

    static func orientation(for traits: UITraitCollection) -> WBKeyboardOrientation {
        traits.verticalSizeClass == .compact ? .landscape : .portrait
    }

    var orientation: WBKeyboardOrientation {
        orientationOverride ?? Self.orientation(for: traitCollection)
    }

    var preferredHeight: CGFloat {
        WBKeyLayout.height(for: orientation)
    }

    // MARK: - Host events

    var enableInputClicksWhenVisible: Bool { true }

    /// The field or its contents changed (also after this keyboard's own edits).
    func textDidChange() {
        applyAppearance()
        let route = WBFieldRouting.route(for: engine.target.keyboardType)
        let routeField: WBFieldKind = if case .keys(let kind, _) = route { kind } else { .text }
        if routeField != engine.field {
            plane.cancelAllTouches()
            engine.configureForField()
        } else {
            engine.textDidChange()
        }
        refreshReturnKey()
        plane.refresh()
    }

    /// The keyboard came on screen for a (possibly new) field.
    func prepareForField() {
        plane.cancelAllTouches()
        engine.configureForField()
        refreshReturnKey()
        applyAppearance()
        plane.refresh()
    }

    /// A dictation was inserted at the cursor.
    func didInsertDictation(_ text: String) {
        engine.didInsertDictation(text)
        plane.refresh()
    }

    /// Previews: show a key held down, or its alternates open.
    func debugPress(_ character: String, alternates: Bool = false, slideTo index: Int? = nil) {
        layoutIfNeeded()
        guard let key = engine.layout.key(.character(character)) else { return }
        engine.touchBegan(id: -1, at: CGPoint(x: key.frame.midX, y: key.frame.midY))
        if alternates, let touch = engine.touches[-1] {
            engine.holdElapsed(id: -1, generation: touch.hold.generation)
            if let index, let menu = engine.alternates?.menu {
                let cell = menu.cellFrame(index)
                engine.touchMoved(id: -1, to: CGPoint(x: cell.midX, y: cell.midY))
            }
        }
    }

    /// Previews: one whole tap on a key.
    func debugTap(_ kind: WBKeyKind) {
        layoutIfNeeded()
        guard let key = engine.layout.key(kind) else { return }
        let point = CGPoint(x: key.frame.midX, y: key.frame.midY)
        engine.touchBegan(id: -3, at: point)
        engine.touchEnded(id: -3, at: point)
    }

    /// Previews: show a page.
    func debugShow(_ layer: WBKeyLayer) {
        layoutIfNeeded()
        guard layer != .letters, let key = engine.layout.key(.layerSwitch(.numbers)) else { return }
        let point = CGPoint(x: key.frame.midX, y: key.frame.midY)
        engine.touchBegan(id: -2, at: point)
        engine.touchEnded(id: -2, at: point)
        if layer == .symbols, let next = engine.layout.key(.layerSwitch(.symbols)) {
            let point = CGPoint(x: next.frame.midX, y: next.frame.midY)
            engine.touchBegan(id: -2, at: point)
            engine.touchEnded(id: -2, at: point)
        }
    }

    // MARK: - Layout

    override func layoutSubviews() {
        super.layoutSubviews()
        let isLandscape = orientation == .landscape
        engine.orientation = orientation
        engine.horizontalInset = isLandscape ? max(safeAreaInsets.left, safeAreaInsets.right) : 0
        engine.width = bounds.width
        let layout = engine.layout

        // Both drawn views keep the keyboard's coordinates but are only as large as
        // what they draw: a full-size layer each would cost 4 MB of the extension's memory.
        place(plane, over: layout.keysAreaFrame)
        strip.view.frame = layout.stripFrame

        if let globe = layout.key(.globe) {
            globeButton.isHidden = false
            globeButton.frame = globe.hitFrame
        } else {
            globeButton.isHidden = true
        }
        plane.refresh()
        updateCallout()
    }

    // MARK: - State

    private func applyAppearance() {
        let isDark = appearanceOverride
            ?? WBKeyPalette.isDark(appearance: engine.target.keyboardAppearance, traits: traitCollection)
        let next = WBKeyPalette(isDark: isDark)
        backgroundColor = drawsBackdrop ? next.backdrop : .clear
        guard next != palette || !didApplyPalette else { return }
        didApplyPalette = true
        palette = next
        plane.palette = next
        callout.palette = next
        callout.setNeedsDisplay()
        strip.overrideUserInterfaceStyle = isDark ? .dark : .light
    }

    private func refreshReturnKey() {
        let type = engine.target.returnKeyType ?? .default
        plane.returnLabel = switch type {
        case .go: "go"
        case .google, .yahoo, .search: "search"
        case .join: "join"
        case .next: "next"
        case .route: "route"
        case .send: "send"
        case .done: "done"
        case .emergencyCall: "SOS"
        case .continue: "continue"
        default: "return"
        }
        plane.returnIsAction = switch type {
        case .go, .google, .yahoo, .search, .join, .route, .send, .done, .continue: true
        default: false
        }
    }

    private func refreshFromModel() {
        if engine.showsGlobe != model.needsGlobeKey {
            plane.cancelAllTouches()
            engine.showsGlobe = model.needsGlobeKey
            setNeedsLayout()
        }
    }

    private func engineChanged() {
        plane.refresh()
        updateCallout()
        if globeButton.isHidden != (engine.layout.key(.globe) == nil) {
            setNeedsLayout()
        }
    }

    private func updateCallout() {
        let layout = engine.layout
        if let open = engine.alternates {
            place(callout, over: open.menu.balloon.bounds.insetBy(dx: -8, dy: -8))
            callout.isHidden = false
            callout.show(open.menu, selected: open.selected)
        } else if let key = engine.previewKey, let balloon = layout.balloon(for: key) {
            place(callout, over: balloon.bounds.insetBy(dx: -8, dy: -8))
            callout.isHidden = false
            callout.show(balloon, label: engine.displayLabel(for: key))
        } else {
            callout.isHidden = true
            callout.show(nil, label: "")
        }
    }

    /// Puts `view` over `rect` of the keyboard, with the keyboard's coordinates inside it.
    private func place(_ view: UIView, over rect: CGRect) {
        let rect = rect.intersection(bounds.isEmpty ? rect : bounds)
        guard view.bounds != rect || view.center != CGPoint(x: rect.midX, y: rect.midY) else { return }
        view.bounds = rect
        view.center = CGPoint(x: rect.midX, y: rect.midY)
        view.setNeedsDisplay()
    }

    // MARK: - Actions

    private func keyFeedback() {
        UIDevice.current.playInputClick()
        if allowsHaptics() {
            haptics.impactOccurred(intensity: 0.6)
            haptics.prepare()
        }
    }

    private func configureHost() {
        globeButton.removeTarget(nil, action: nil, for: .allEvents)
        guard let controller = hostController else { return }
        globeButton.addTarget(
            controller,
            action: #selector(UIInputViewController.handleInputModeList(from:with:)),
            for: .allTouchEvents
        )
        if strip.parent !== controller {
            controller.addChild(strip)
            strip.didMove(toParent: controller)
        }
    }
}
