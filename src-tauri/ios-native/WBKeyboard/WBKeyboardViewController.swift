// SPDX-License-Identifier: GPL-3.0-or-later
// The Wispr Bro keyboard: typing keys under a dictation strip. It never
// touches the microphone or a model; it sends commands to the app over the
// App Group and inserts the finished text.
//
// Ported from Hex (MIT, (c) 2026 Christopher Langton):
// ios/HEXKeyboard/KeyboardViewController.swift. See ../NOTICE-hex.md.

import SwiftUI
import UIKit

/// The keyboard's root view; adopting the protocol lets the keys click.
final class WBKeyboardInputView: UIInputView, UIInputViewAudioFeedback {
    var enableInputClicksWhenVisible: Bool { true }
}

@objc(WBKeyboardViewController)
final class WBKeyboardViewController: UIInputViewController {
    private let bridge = WBBridge()
    private let model = WBKeyboardModel()
    private var pollTimer: Timer?
    private var stateObserver: WBDarwinObserver?
    private var heightConstraint: NSLayoutConstraint?
    private var lastFullAccessNote: TimeInterval = 0
    private var isVisible = false
    private var keyboard: WBFullKeyboardView?
    private static let takesStartedKey = "wb.keyboard.takesStarted"

    override func loadView() {
        let root = WBKeyboardInputView(frame: .zero, inputViewStyle: .keyboard)
        root.allowsSelfSizing = true
        view = root
    }

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
        keyboard?.prepareForField()
        updateHeight()
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

    override func viewDidAppear(_ animated: Bool) {
        super.viewDidAppear(animated)
        // The window's edge gestures hold touches back for a moment, which makes the
        // keys at the screen edges (q, a, p, 123) feel late. Keys must answer at once.
        view.window?.gestureRecognizers?.forEach { $0.delaysTouchesBegan = false }
    }

    override func viewWillLayoutSubviews() {
        super.viewWillLayoutSubviews()
        updateHeight()
    }

    override func textDidChange(_ textInput: UITextInput?) {
        super.textDidChange(textInput)
        if leaveNumericField() { return }
        updateTraits()
        keyboard?.textDidChange()
    }

    // MARK: Setup

    private func wireModel() {
        model.showsTalkHint = UserDefaults.standard.integer(forKey: Self.takesStartedKey) < 3
        model.levelSource = { [weak self] in self?.bridge.levels?.currentLevel() ?? 0 }
        model.onMicTap = { [weak self] in self?.micTapped() }
        model.onFinishTap = { [weak self] in self?.finishTapped() }
        model.onCancelTap = { [weak self] in self?.cancelTapped() }
        model.onOpenApp = { [weak self] in self?.openApp(startingTake: false) }
    }

    private func mountSurface() {
        let target = WBDocumentProxyTarget { [weak self] in self?.textDocumentProxy }
        let keyboard = WBFullKeyboardView(model: model, target: target)
        keyboard.hostController = self
        keyboard.allowsHaptics = { [weak self] in self?.hasFullAccess ?? false }
        keyboard.translatesAutoresizingMaskIntoConstraints = false
        view.addSubview(keyboard)
        let height = view.heightAnchor.constraint(equalToConstant: keyboard.preferredHeight)
        height.priority = UILayoutPriority(999)
        heightConstraint = height
        NSLayoutConstraint.activate([
            height,
            keyboard.leadingAnchor.constraint(equalTo: view.leadingAnchor),
            keyboard.trailingAnchor.constraint(equalTo: view.trailingAnchor),
            keyboard.topAnchor.constraint(equalTo: view.topAnchor),
            keyboard.bottomAnchor.constraint(equalTo: view.bottomAnchor),
        ])
        self.keyboard = keyboard
    }

    /// Portrait and landscape have their own fixed heights; the strip never changes them.
    private func updateHeight() {
        guard let keyboard, heightConstraint?.constant != keyboard.preferredHeight else { return }
        heightConstraint?.constant = keyboard.preferredHeight
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
                keyboard?.didInsertDictation(text)
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
        noteTakeStarted()
        haptic()
        sync()
    }

    /// The "Tap to talk" hint goes away after the first few takes.
    private func noteTakeStarted() {
        let defaults = UserDefaults.standard
        let count = defaults.integer(forKey: Self.takesStartedKey) + 1
        defaults.set(count, forKey: Self.takesStartedKey)
        if count >= 3, model.showsTalkHint { model.showsTalkHint = false }
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
