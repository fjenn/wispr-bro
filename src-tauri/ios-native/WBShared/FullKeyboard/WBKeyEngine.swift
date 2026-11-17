// SPDX-License-Identifier: GPL-3.0-or-later
// Everything the keys do, apart from drawing.
//
// Ported from Hex (MIT, (c) 2026 Christopher Langton):
// ios/HEXShared/FullKeyboard/FullKeyboardEngine.swift. See ../../NOTICE-hex.md.

import QuartzCore
import UIKit

/// The text field the keys type into. `UITextDocumentProxy` in the keyboard; a fake in tests.
@MainActor
protocol WBTextTarget: AnyObject {
    var documentContextBeforeInput: String? { get }
    var hasText: Bool { get }
    var autocapitalizationType: UITextAutocapitalizationType? { get }
    var keyboardType: UIKeyboardType? { get }
    var returnKeyType: UIReturnKeyType? { get }
    var keyboardAppearance: UIKeyboardAppearance? { get }
    var enablesReturnKeyAutomatically: Bool? { get }
    func insertText(_ text: String)
    func deleteBackward()
}

@MainActor
final class WBDocumentProxyTarget: WBTextTarget {
    private let proxy: () -> UITextDocumentProxy?

    init(_ proxy: @escaping () -> UITextDocumentProxy?) {
        self.proxy = proxy
    }

    var documentContextBeforeInput: String? { proxy()?.documentContextBeforeInput }
    var hasText: Bool { proxy()?.hasText ?? false }
    var autocapitalizationType: UITextAutocapitalizationType? { proxy()?.autocapitalizationType }
    var keyboardType: UIKeyboardType? { proxy()?.keyboardType }
    var returnKeyType: UIReturnKeyType? { proxy()?.returnKeyType }
    var keyboardAppearance: UIKeyboardAppearance? { proxy()?.keyboardAppearance }
    var enablesReturnKeyAutomatically: Bool? { proxy()?.enablesReturnKeyAutomatically }
    func insertText(_ text: String) { proxy()?.insertText(text) }
    func deleteBackward() { proxy()?.deleteBackward() }
}

/// Everything the keys do, apart from drawing: multitouch with rollover, shift, pages,
/// double-space period, auto-capitalization, delete, and (Wispr Bro) long-press
/// alternates and the space owed after a dictation. Touches are identified by any
/// stable integer so tests can drive it without `UITouch`.
@MainActor
final class WBKeyEngine {
    struct ActiveTouch: Equatable {
        var key: WBKey
        var point: CGPoint
        /// A character that already went in because another key went down after it.
        var committed: Bool
        /// The page a 123/ABC press came from; releasing over a character returns there.
        var returnLayer: WBKeyLayer?
        /// Tap or long press, for character keys.
        var hold = WBKeyHoldGesture()
    }

    /// The open long-press row and the finger that owns it.
    struct ActiveAlternates: Equatable {
        let touchID: Int
        let menu: WBAlternatesMenu
        var selected: Int?
    }

    private(set) var shift = WBShiftStateMachine()
    private(set) var layerState = WBKeyLayerState()
    private(set) var field: WBFieldKind = .text
    private(set) var touches: [Int: ActiveTouch] = [:]
    /// Touch ids in the order they went down.
    private var touchOrder: [Int] = []
    private var lastSpaceAt: TimeInterval?
    private(set) var alternates: ActiveAlternates?
    /// The end of the dictation that was just inserted, while nothing was typed since.
    private var dictationTail: String?

    let target: WBTextTarget
    var width: CGFloat = 375 { didSet { if width != oldValue { rebuildLayout() } } }
    var orientation: WBKeyboardOrientation = .portrait { didSet { if orientation != oldValue { rebuildLayout() } } }
    var showsGlobe = false { didSet { if showsGlobe != oldValue { rebuildLayout() } } }
    var horizontalInset: CGFloat = 0 { didSet { if horizontalInset != oldValue { rebuildLayout() } } }
    private(set) var layout = WBKeyLayout(width: 375)

    var clock: () -> TimeInterval = { CACurrentMediaTime() }
    /// Called whenever something visible changed (page, shift, pressed keys).
    var onChange: (() -> Void)?
    /// Called after the keys typed or deleted anything.
    var onEdit: (() -> Void)?

    init(target: WBTextTarget) {
        self.target = target
        rebuildLayout()
    }

    var isUppercase: Bool { layerState.layer == .letters && shift.isUppercase }

    /// The key whose balloon shows: the newest character still held.
    var previewKey: WBKey? {
        for id in touchOrder.reversed() {
            if let touch = touches[id], touch.key.character != nil, !touch.committed {
                if alternates?.touchID == id { return nil }
                return touch.key
            }
        }
        return nil
    }

    var pressedKinds: Set<WBKeyKind> {
        Set(touches.values.map(\.key.kind))
    }

    func displayLabel(for key: WBKey) -> String {
        guard let character = key.character else { return key.label }
        return isUppercase && character.count == 1 ? character.uppercased() : character
    }

    /// False when the field wants text before return does anything.
    var isReturnEnabled: Bool {
        !(target.enablesReturnKeyAutomatically == true && !target.hasText)
    }

    // MARK: - Field

    /// Call when the keyboard appears or the field changes.
    func configureForField() {
        switch WBFieldRouting.route(for: target.keyboardType) {
        case .keys(let kind, let initialLayer):
            field = kind
            layerState.reset(to: initialLayer)
        case .handOff:
            field = .text
            layerState.reset(to: .numbers)
        }
        shift.reset()
        cancelAllTouches()
        rebuildLayout()
        refreshAutoCapitalization()
    }

    /// Call after the text or the cursor changed outside these keys.
    func textDidChange() {
        // iOS also reports this keyboard's own edits here, so nothing is reset: the
        // double space and the owed space check the text before the cursor themselves.
        refreshAutoCapitalization()
    }

    /// A dictation went in at the cursor. When it ends a sentence or clause, the next
    /// letter typed gets a space before it.
    func didInsertDictation(_ text: String) {
        lastSpaceAt = nil
        if let last = text.last, Self.spaceOwedAfter.contains(last) {
            dictationTail = String(text.suffix(12))
        } else {
            dictationTail = nil
        }
        refreshAutoCapitalization()
    }

    private static let spaceOwedAfter: Set<Character> = [".", ",", ";", ":", "!", "?", "…"]

    private var owesSpace: Bool {
        guard let dictationTail, let before = target.documentContextBeforeInput else { return false }
        return before.hasSuffix(dictationTail)
    }

    func refreshAutoCapitalization() {
        let before = shift
        shift.refreshAutoCapitalization(
            WBAutoCapitalization.shouldCapitalize(
                mode: WBAutoCapitalization.Mode(target.autocapitalizationType),
                before: owesSpace ? target.documentContextBeforeInput.map { $0 + " " } : target.documentContextBeforeInput,
                hasText: target.hasText
            )
        )
        if shift != before { onChange?() }
    }

    // MARK: - Touches

    func touchBegan(id: Int, at point: CGPoint) {
        guard let key = layout.hitTest(point) else { return }
        // Rollover: a new key going down commits any character still held.
        for other in touchOrder {
            guard var touch = touches[other], touch.key.character != nil, !touch.committed else { continue }
            if alternates?.touchID == other {
                commitAlternate()
            } else {
                commitCharacter(touch.key)
            }
            touch.hold.cancel()
            touch.committed = true
            touches[other] = touch
        }

        var touch = ActiveTouch(key: key, point: point, committed: false, returnLayer: nil)
        switch key.kind {
        case .shift:
            shift.shiftDown(now: clock())
        case .delete:
            deleteBackward()
        case .layerSwitch(let next):
            touch.returnLayer = layerState.layer
            layerState.switchTo(next)
            if next == .letters { shift.reset() }
            rebuildLayout()
            if next == .letters { refreshAutoCapitalization() }
        case .character:
            touch.hold.touchDown()
        case .space, .returnKey, .globe:
            break
        }
        touches[id] = touch
        touchOrder.append(id)
        onChange?()
    }

    func touchMoved(id: Int, to point: CGPoint) {
        guard var touch = touches[id] else { return }
        touch.point = point
        if var open = alternates, open.touchID == id {
            _ = touch.hold.touchMoved()
            touches[id] = touch
            let selected = open.menu.index(at: point)
            if selected != open.selected {
                open.selected = selected
                alternates = open
                onChange?()
            }
            return
        }
        if touch.key.character != nil, !touch.committed,
           let under = layout.hitTest(point), under.character != nil, under.kind != touch.key.kind {
            touch.key = under
            touch.hold.touchDown()
            touches[id] = touch
            onChange?()
            return
        }
        touches[id] = touch
    }

    func touchEnded(id: Int, at point: CGPoint) {
        guard var touch = touches.removeValue(forKey: id) else { return }
        touchOrder.removeAll { $0 == id }
        touch.point = point
        switch touch.key.kind {
        case .character:
            if !touch.committed {
                if touch.hold.touchUp() == .releaseMenu, var open = alternates, open.touchID == id {
                    open.selected = open.menu.index(at: point)
                    alternates = open
                    commitAlternate()
                } else {
                    commitCharacter(touch.key)
                }
            }
        case .shift:
            shift.shiftUp()
        case .space:
            typeSpace()
        case .returnKey:
            if isReturnEnabled { insert("\n") }
        case .layerSwitch:
            // Hold 123, slide to a key, let go: type it and go back.
            if let returnLayer = touch.returnLayer,
               let under = layout.hitTest(point),
               let character = under.character,
               under.kind != touch.key.kind {
                insert(character)
                layerState.switchTo(returnLayer)
                rebuildLayout()
                refreshAutoCapitalization()
            }
        case .delete, .globe:
            break
        }
        onChange?()
    }

    /// The hold timer of touch `id`, started when its press had `generation`, ran out:
    /// opens the key's alternates if the finger is still on that key. Returns whether it did.
    @discardableResult
    func holdElapsed(id: Int, generation: Int) -> Bool {
        guard var touch = touches[id], !touch.committed, let character = touch.key.character else { return false }
        let options = WBKeyAlternates.options(for: character, uppercase: isUppercase)
        let menu = layout.alternatesMenu(for: touch.key, options: options)
        guard touch.hold.holdElapsed(generation: generation, hasAlternates: menu != nil) == .openMenu,
              let menu else { return false }
        touches[id] = touch
        alternates = ActiveAlternates(touchID: id, menu: menu, selected: menu.defaultIndex)
        onChange?()
        return true
    }

    func touchCancelled(id: Int) {
        guard touches.removeValue(forKey: id) != nil else { return }
        if alternates?.touchID == id { alternates = nil }
        touchOrder.removeAll { $0 == id }
        onChange?()
    }

    func cancelAllTouches() {
        let hadTouches = !touches.isEmpty
        touches.removeAll()
        touchOrder.removeAll()
        alternates = nil
        if shift.isHeld { shift.shiftUp() }
        if hadTouches { onChange?() }
    }

    // MARK: - Delete

    /// The delete key repeats while held, faster and faster; after `wordDeleteAfter` it
    /// removes whole words at a calmer pace.
    static let repeatDelay: TimeInterval = 0.38
    static let wordDeleteAfter: TimeInterval = 1.9

    /// The wait before the next repeat, for a key held this long.
    static func repeatInterval(heldFor: TimeInterval) -> TimeInterval {
        if heldFor >= wordDeleteAfter { return 0.2 }
        let progress = min(1, max(0, (heldFor - repeatDelay) / 1.0))
        return 0.1 - 0.055 * progress
    }

    func repeatDelete(heldFor: TimeInterval) {
        if heldFor >= Self.wordDeleteAfter {
            deleteWordBackward()
        } else {
            deleteBackward()
        }
    }

    func deleteBackward() {
        target.deleteBackward()
        lastSpaceAt = nil
        dictationTail = nil
        didEdit()
    }

    func deleteWordBackward() {
        guard let before = target.documentContextBeforeInput, !before.isEmpty else {
            deleteBackward()
            return
        }
        var count = 0
        var index = before.endIndex
        while index > before.startIndex, before[before.index(before: index)].isWhitespace {
            index = before.index(before: index)
            count += 1
        }
        while index > before.startIndex, !before[before.index(before: index)].isWhitespace {
            index = before.index(before: index)
            count += 1
        }
        for _ in 0..<max(1, count) {
            target.deleteBackward()
        }
        lastSpaceAt = nil
        dictationTail = nil
        didEdit()
    }

    // MARK: - Typing

    private func commitCharacter(_ key: WBKey) {
        guard let character = key.character else { return }
        var text = isUppercase && character.count == 1 ? character.uppercased() : character
        if owesSpace, let first = character.first, first.isLetter || first.isNumber {
            text = " " + text
        }
        if layerState.layer == .letters {
            shift.didType()
        }
        insert(text)
    }

    /// Types the chosen alternate (none chosen: nothing) and closes the row.
    private func commitAlternate() {
        guard let open = alternates else { return }
        alternates = nil
        guard let index = open.selected, open.menu.options.indices.contains(index) else { return }
        var text = open.menu.options[index]
        if owesSpace, let first = text.first, first.isLetter || first.isNumber {
            text = " " + text
        }
        if layerState.layer == .letters {
            shift.didType()
        }
        insert(text)
    }

    private func typeSpace() {
        let now = clock()
        if WBDoubleSpacePeriod.shouldReplace(before: target.documentContextBeforeInput, lastSpaceAt: lastSpaceAt, now: now) {
            target.deleteBackward()
            target.insertText(". ")
            lastSpaceAt = nil
            dictationTail = nil
            layerState.didInsert(" ")
            rebuildLayoutIfNeeded()
            didEdit()
            return
        }
        insert(" ")
        lastSpaceAt = now
    }

    private func insert(_ text: String) {
        target.insertText(text)
        lastSpaceAt = nil
        dictationTail = nil
        layerState.didInsert(text)
        rebuildLayoutIfNeeded()
        didEdit()
    }

    private func didEdit() {
        refreshAutoCapitalization()
        onEdit?()
    }

    // MARK: - Layout

    private func rebuildLayoutIfNeeded() {
        if layout.layer != layerState.layer {
            rebuildLayout()
        }
    }

    private func rebuildLayout() {
        layout = WBKeyLayout(
            width: width,
            orientation: orientation,
            layer: layerState.layer,
            field: field,
            showsGlobe: showsGlobe,
            horizontalInset: horizontalInset
        )
    }
}
