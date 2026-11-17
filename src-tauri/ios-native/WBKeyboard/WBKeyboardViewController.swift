// SPDX-License-Identifier: GPL-3.0-or-later
// The Wispr Bro keyboard: a mic-first dictation surface. It never touches the
// microphone or a model; it sends commands to the app over the App Group and
// inserts the finished text.
//
// Ported from Hex (MIT, (c) 2026 Christopher Langton):
// ios/HEXKeyboard/KeyboardViewController.swift. See ../NOTICE-hex.md.

import SwiftUI
import UIKit

@objc(WBKeyboardViewController)
final class WBKeyboardViewController: UIInputViewController {
    private let bridge = WBBridge()
    private let model = WBKeyboardModel()
    private var pollTimer: Timer?
    private var stateObserver: WBDarwinObserver?
    private var heightConstraint: NSLayoutConstraint?
    private var lastFullAccessNote: TimeInterval = 0
    private var isVisible = false

    // MARK: Lifecycle

    override func viewDidLoad() {
        super.viewDidLoad()
        view.backgroundColor = .clear
        wireModel()
        mountSurface()
        sync()
    }

    override func viewWillAppear(_ animated: Bool) {
        super.viewWillAppear(animated)
        isVisible = true
        if leaveNumericField() { return }
        stateObserver = WBDarwinObserver(WBBridgeConstants.stateNotification) { [weak self] in
            self?.sync()
        }
        // Darwin notifications carry state changes; the timer catches a missed
        // one and notices a session whose heartbeat stopped.
        pollTimer?.invalidate()
        pollTimer = Timer.scheduledTimer(withTimeInterval: 0.25, repeats: true) { [weak self] _ in
            self?.sync()
        }
        sync()
    }

    override func viewWillDisappear(_ animated: Bool) {
        super.viewWillDisappear(animated)
        isVisible = false
        pollTimer?.invalidate()
        pollTimer = nil
        stateObserver = nil
    }

    override func textDidChange(_ textInput: UITextInput?) {
        super.textDidChange(textInput)
        if leaveNumericField() { return }
        updateTraits()
    }

    // MARK: Setup

    private func wireModel() {
        model.inputViewController = self
        model.levelSource = { [weak self] in self?.bridge.levels?.currentLevel() ?? 0 }
        model.onMicTap = { [weak self] in self?.micTapped() }
        model.onFinishTap = { [weak self] in self?.finishTapped() }
        model.onCancelTap = { [weak self] in self?.cancelTapped() }
        model.onOpenApp = { [weak self] in self?.openApp(startingTake: false) }
        model.onSpace = { [weak self] in self?.textDocumentProxy.insertText(" ") }
        model.onDelete = { [weak self] in self?.textDocumentProxy.deleteBackward() }
        model.onReturn = { [weak self] in self?.textDocumentProxy.insertText("\n") }
    }

    private func mountSurface() {
        let hosting = UIHostingController(rootView: WBKeyboardSurfaceView(model: model))
        hosting.view.backgroundColor = .clear
        addChild(hosting)
        hosting.view.translatesAutoresizingMaskIntoConstraints = false
        view.addSubview(hosting.view)
        let height = view.heightAnchor.constraint(equalToConstant: WBKeyboardSurfaceView.preferredHeight)
        height.priority = UILayoutPriority(999)
        heightConstraint = height
        NSLayoutConstraint.activate([
            height,
            hosting.view.leadingAnchor.constraint(equalTo: view.leadingAnchor),
            hosting.view.trailingAnchor.constraint(equalTo: view.trailingAnchor),
            hosting.view.topAnchor.constraint(equalTo: view.topAnchor),
            hosting.view.bottomAnchor.constraint(equalTo: view.bottomAnchor),
        ])
        hosting.didMove(toParent: self)
    }

    /// Number, phone and decimal fields need digits, not dictation.
    private func leaveNumericField() -> Bool {
        switch textDocumentProxy.keyboardType {
        case .numberPad, .phonePad, .decimalPad, .asciiCapableNumberPad:
            advanceToNextInputMode()
            return true
        default:
            return false
        }
    }

    private func updateTraits() {
        let globe = needsInputModeSwitchKey
        if model.needsGlobeKey != globe { model.needsGlobeKey = globe }
        // Fields that ask for a dark keyboard get one even in light mode.
        overrideUserInterfaceStyle = textDocumentProxy.keyboardAppearance == .dark ? .dark : .unspecified
        let label: String? = switch textDocumentProxy.returnKeyType {
        case .send: "Send"
        case .search, .google, .yahoo: "Search"
        case .go: "Go"
        case .done: "Done"
        case .next: "Next"
        case .join: "Join"
        default: nil
        }
        if model.returnLabel != label { model.returnLabel = label }
    }

    private var effectiveFullAccess: Bool {
        #if targetEnvironment(simulator)
        // The simulator shares the App Group without the Full Access switch.
        return true
        #else
        return hasFullAccess
        #endif
    }

    // MARK: Sync

    private func sync() {
        let now = Date().timeIntervalSince1970
        updateTraits()

        let fullAccess = effectiveFullAccess
        if now - lastFullAccessNote >= 5 {
            lastFullAccessNote = now
            // Without Full Access this write stays in the extension's own
            // sandbox, so the app only ever sees `true` or nothing.
            bridge.noteFullAccess(fullAccess, at: now)
        }
        guard fullAccess else {
            model.apply(hasFullAccess: false, snapshot: .noSession, now: now)
            return
        }

        if isVisible, let result = bridge.claimResult(at: now) {
            let text = WBInsertion.text(for: result.text, before: textDocumentProxy.documentContextBeforeInput)
            if !text.isEmpty {
                textDocumentProxy.insertText(text)
                haptic()
            }
            model.pending = nil
        }

        withAnimation(WBTheme.quick) {
            model.apply(hasFullAccess: true, snapshot: bridge.snapshot(at: now), now: now)
        }
    }

    // MARK: Actions

    private func micTapped() {
        let now = Date().timeIntervalSince1970
        guard bridge.isSessionAlive(at: now) else {
            openApp(startingTake: true)
            return
        }
        let takeID = WBTakeID.make()
        model.pending = .init(kind: .start, takeID: takeID, sentAt: now)
        bridge.send(.start, takeID: takeID, at: now)
        haptic()
        sync()
    }

    private func finishTapped() {
        guard let takeID = bridge.rawSnapshot().takeID ?? model.pending?.takeID else { return }
        let now = Date().timeIntervalSince1970
        model.pending = .init(kind: .stop, takeID: takeID, sentAt: now)
        bridge.send(.stop, takeID: takeID, at: now)
        haptic()
        sync()
    }

    private func cancelTapped() {
        guard let takeID = bridge.rawSnapshot().takeID ?? model.pending?.takeID else { return }
        let now = Date().timeIntervalSince1970
        model.pending = .init(kind: .cancel, takeID: takeID, sentAt: now)
        bridge.send(.cancel, takeID: takeID, at: now)
        haptic()
        sync()
    }

    /// No live session: hand over to the app, which starts one (and the take)
    /// and sends the user back here.
    private func openApp(startingTake: Bool) {
        let request = WBStartRequest(
            requestedAt: Date().timeIntervalSince1970,
            takeID: startingTake ? WBTakeID.make() : nil,
            hostBundleID: hostBundleID()
        )
        if effectiveFullAccess {
            bridge.requestSessionStart(request)
        }
        haptic()
        WBContainingAppOpener.open(request.url, from: self)
    }

    /// The app this keyboard is typing in. Private and best effort: iOS 26.4+
    /// no longer tells the extension (the app then detects it itself).
    private func hostBundleID() -> String? {
        guard bridge.autoReturn, let parent else { return nil }
        let selector = NSSelectorFromString("_hostBundleID")
        guard parent.responds(to: selector),
              let value = parent.perform(selector)?.takeUnretainedValue() as? String,
              !value.isEmpty, value != "<null>" else { return nil }
        return value
    }

    private func haptic() {
        // Haptics from a keyboard need Full Access.
        guard hasFullAccess else { return }
        UIImpactFeedbackGenerator(style: .light).impactOccurred()
    }
}
