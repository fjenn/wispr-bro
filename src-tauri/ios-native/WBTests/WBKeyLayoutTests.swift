// SPDX-License-Identifier: GPL-3.0-or-later
// Ported from Hex (MIT, (c) 2026 Christopher Langton): ios/HEXMobileTests/KeyboardLayoutTests.swift.
// See ../NOTICE-hex.md.

import CoreGraphics
import XCTest
@testable import WBNativeHarness

final class WBKeyLayoutTests: XCTestCase {
    private let portraitWidths: [CGFloat] = [320, 375, 393, 402, 430, 440]
    private let layers: [WBKeyLayer] = [.letters, .numbers, .symbols]
    private let fields: [WBFieldKind] = [.text, .url, .email, .webSearch]

    private func everyPortraitLayout() -> [WBKeyLayout] {
        var layouts: [WBKeyLayout] = []
        for width in portraitWidths {
            for layer in layers {
                for field in fields {
                    for globe in [false, true] {
                        layouts.append(WBKeyLayout(width: width, layer: layer, field: field, showsGlobe: globe))
                    }
                }
            }
        }
        return layouts
    }

    func testPortraitIs270PointsOnEveryWidth() {
        for width in portraitWidths {
            XCTAssertEqual(WBKeyLayout(width: width).totalHeight, 270, "width \(width)")
        }
        XCTAssertEqual(WBKeyLayout.height(for: .portrait), 270)
        XCTAssertEqual(WBKeyLayout.height(for: .landscape), 190)
    }

    func testLetterWidthMatchesPlan() {
        XCTAssertEqual(WBKeyLayout(width: 393).letterWidth, (393 - 12 - 54) / 10, accuracy: 0.001)
        let q = WBKeyLayout(width: 393).key(.character("q"))!
        XCTAssertEqual(q.frame.minX, 6, accuracy: 0.001)
        XCTAssertEqual(q.frame.minY, 54 + 8, accuracy: 0.001)
        XCTAssertEqual(q.frame.height, 42)
    }

    func testHitTargetsTileTheKeyAreaWithNoDeadZonesOrOverlaps() {
        var generator = SystemRandomNumberGenerator()
        for layout in everyPortraitLayout() + landscapeLayouts() {
            let area = layout.keysAreaFrame
            // Rows stack edge to edge from the top of the key area to the bottom.
            var rowTop = area.minY
            for row in 0..<4 {
                let targets = layout.keys.filter { $0.row == row }.map(\.hitFrame).sorted { $0.minX < $1.minX }
                XCTAssertEqual(targets.first?.minX, 0, describe(layout))
                XCTAssertEqual(targets.last?.maxX, layout.width, describe(layout))
                for target in targets {
                    XCTAssertEqual(target.minY, rowTop, accuracy: 0.0001, "\(describe(layout)) row \(row)")
                }
                for (a, b) in zip(targets, targets.dropFirst()) {
                    XCTAssertEqual(a.maxX, b.minX, accuracy: 0.0001, "\(describe(layout)) row \(row)")
                }
                rowTop = targets[0].maxY
            }
            XCTAssertEqual(rowTop, area.maxY, accuracy: 0.0001, describe(layout))

            // And every sampled point lands on exactly one key.
            for _ in 0..<400 {
                let point = CGPoint(
                    x: CGFloat.random(in: 0..<layout.width, using: &generator),
                    y: CGFloat.random(in: area.minY..<area.maxY, using: &generator)
                )
                let hits = layout.keys.filter { $0.hitFrame.contains(point) }
                XCTAssertEqual(hits.count, 1, "\(describe(layout)) at \(point): \(hits.map(\.label))")
            }
            XCTAssertNil(layout.hitTest(CGPoint(x: 20, y: layout.stripFrame.midY)), "strip is not a key")
        }
    }

    func testKeycapsSitInsideTheirTargetsAndNeverOverlap() {
        for layout in everyPortraitLayout() + landscapeLayouts() {
            let bounds = CGRect(x: 0, y: 0, width: layout.width, height: layout.totalHeight)
            for key in layout.keys {
                XCTAssertTrue(key.hitFrame.contains(key.frame), "\(describe(layout)) \(key.label)")
                XCTAssertTrue(bounds.contains(key.frame), "\(describe(layout)) \(key.label)")
                XCTAssertGreaterThan(key.frame.width, 20, "\(describe(layout)) \(key.label)")
            }
            for (i, a) in layout.keys.enumerated() {
                for b in layout.keys[(i + 1)...] {
                    XCTAssertFalse(a.frame.intersects(b.frame), "\(describe(layout)) \(a.label) × \(b.label)")
                }
            }
        }
    }

    func testBalloonsStayInsideTheKeyboard() {
        for layout in everyPortraitLayout() {
            let bounds = CGRect(x: 0, y: 0, width: layout.width, height: layout.totalHeight)
            for key in layout.keys where key.character?.count == 1 {
                guard let balloon = layout.balloon(for: key) else {
                    return XCTFail("\(describe(layout)) \(key.label) has no balloon")
                }
                XCTAssertTrue(
                    bounds.insetBy(dx: -0.001, dy: -0.001).contains(balloon.bounds),
                    "\(describe(layout)) \(key.label): \(balloon.bubble)"
                )
                XCTAssertGreaterThanOrEqual(balloon.bubble.width, key.frame.width)
            }
        }
    }

    func testEdgeBalloonsArePushedInward() {
        let layout = WBKeyLayout(width: 393)
        let q = layout.balloon(for: layout.key(.character("q"))!)!
        let p = layout.balloon(for: layout.key(.character("p"))!)!
        let g = layout.balloon(for: layout.key(.character("g"))!)!
        XCTAssertGreaterThan(q.offsetX, 0)
        XCTAssertLessThan(p.offsetX, 0)
        XCTAssertEqual(g.offsetX, 0, accuracy: 0.001)
        XCTAssertEqual(q.bubble.minY, 0, accuracy: 0.001)
    }

    func testNoBalloonsForModifiersOrInLandscape() {
        let portrait = WBKeyLayout(width: 393)
        XCTAssertNil(portrait.balloon(for: portrait.key(.space)!))
        XCTAssertNil(portrait.balloon(for: portrait.key(.shift)!))
        let landscape = WBKeyLayout(width: 852, orientation: .landscape)
        XCTAssertNil(landscape.balloon(for: landscape.key(.character("a"))!))
    }

    func testBottomRowFollowsTheField() {
        XCTAssertEqual(bottomRow(WBKeyLayout(width: 393)), [.layerSwitch(.numbers), .space, .returnKey])
        XCTAssertEqual(
            bottomRow(WBKeyLayout(width: 375, showsGlobe: true)),
            [.layerSwitch(.numbers), .globe, .space, .returnKey]
        )
        XCTAssertEqual(
            bottomRow(WBKeyLayout(width: 393, layer: .numbers, field: .email)),
            [.layerSwitch(.letters), .character("@"), .space, .character("."), .returnKey]
        )
        XCTAssertEqual(
            bottomRow(WBKeyLayout(width: 393, field: .url)),
            [.layerSwitch(.numbers), .character("/"), .space, .character("."), .character(".com"), .returnKey]
        )
        XCTAssertEqual(
            bottomRow(WBKeyLayout(width: 393, field: .webSearch)),
            [.layerSwitch(.numbers), .space, .character("."), .returnKey]
        )
    }

    func testWithoutAGlobeKey123IsAsWideAsReturn() {
        let plain = WBKeyLayout(width: 393)
        XCTAssertEqual(
            plain.key(.layerSwitch(.numbers))!.frame.width,
            plain.key(.returnKey)!.frame.width,
            accuracy: 0.001
        )
        let globe = WBKeyLayout(width: 393, showsGlobe: true)
        XCTAssertLessThan(globe.key(.layerSwitch(.numbers))!.frame.width, globe.key(.returnKey)!.frame.width)
        XCTAssertGreaterThan(WBKeyLayout(width: 320, field: .url).key(.space)!.frame.width, 40)
    }

    // MARK: Long-press rows (Wispr Bro)

    func testAlternatesStayInsideTheKeyboardOnEveryWidth() {
        let eight = ["à", "á", "â", "ä", "æ", "ã", "å", "ā"]
        for layout in everyPortraitLayout() + landscapeLayouts() {
            let bounds = CGRect(x: 0, y: 0, width: layout.width, height: layout.totalHeight)
            for key in layout.keys where key.character != nil {
                let menu = layout.alternatesMenu(for: key, options: eight)!
                XCTAssertTrue(
                    bounds.insetBy(dx: -0.001, dy: -0.001).contains(menu.bubble),
                    "\(describe(layout)) \(key.label): \(menu.bubble)"
                )
                XCTAssertEqual(menu.options.count, 8)
                XCTAssertGreaterThanOrEqual(menu.cellWidth, 30)
            }
        }
    }

    func testAlternatesGrowTowardTheMiddleWithTheFirstOverTheKey() {
        let layout = WBKeyLayout(width: 393)
        let e = layout.key(.character("e"))!
        let left = layout.alternatesMenu(for: e, options: ["é", "è", "ê"])!
        XCTAssertEqual(left.options, ["é", "è", "ê"])
        XCTAssertEqual(left.defaultIndex, 0)
        XCTAssertEqual(left.cellFrame(0).midX, e.frame.midX, accuracy: 0.001)

        let o = layout.key(.character("o"))!
        let right = layout.alternatesMenu(for: o, options: ["ó", "ö", "ô"])!
        XCTAssertEqual(right.options, ["ô", "ö", "ó"])
        XCTAssertEqual(right.defaultIndex, 2)
        XCTAssertEqual(right.cellFrame(2).midX, o.frame.midX, accuracy: 0.001)
    }

    func testAlternatesFollowTheFingerSideways() {
        let layout = WBKeyLayout(width: 393)
        let menu = layout.alternatesMenu(for: layout.key(.character("e"))!, options: ["é", "è", "ê"])!
        XCTAssertEqual(menu.index(at: CGPoint(x: menu.cellFrame(1).midX, y: menu.keyFrame.midY)), 1)
        XCTAssertEqual(menu.index(at: CGPoint(x: -40, y: menu.bubble.midY)), 0, "clamped")
        XCTAssertEqual(menu.index(at: CGPoint(x: 390, y: menu.bubble.midY)), 2, "clamped")
        XCTAssertNil(menu.index(at: CGPoint(x: menu.cellFrame(1).midX, y: menu.keyFrame.maxY + 80)), "far below: nothing")
        XCTAssertNil(layout.alternatesMenu(for: layout.key(.character("q"))!, options: []))
        XCTAssertNil(layout.alternatesMenu(for: layout.key(.space)!, options: ["x"]))
    }

    func testMultiCharacterKeysHaveNoBalloon() {
        let layout = WBKeyLayout(width: 393, field: .url)
        XCTAssertNil(layout.balloon(for: layout.key(.character(".com"))!))
    }

    func testRowThreeModifiersFollowTheLayer() {
        XCTAssertNotNil(WBKeyLayout(width: 393).key(.shift))
        XCTAssertEqual(WBKeyLayout(width: 393, layer: .numbers).key(.layerSwitch(.symbols))?.row, 2)
        XCTAssertEqual(WBKeyLayout(width: 393, layer: .symbols).key(.layerSwitch(.numbers))?.row, 2)
        let numbers = WBKeyLayout(width: 393, layer: .numbers)
        XCTAssertEqual(numbers.keys.filter { $0.row == 2 }.compactMap(\.character), [".", ",", "?", "!", "'"])
    }

    func testRowTwoIsCentred() {
        let layout = WBKeyLayout(width: 402)
        let a = layout.key(.character("a"))!.frame
        let l = layout.key(.character("l"))!.frame
        XCTAssertEqual(a.minX, layout.width - l.maxX, accuracy: 0.001)
        XCTAssertEqual(layout.key(.character("a"))!.hitFrame.minX, 0)
        XCTAssertEqual(layout.key(.character("l"))!.hitFrame.maxX, layout.width)
    }

    func testLandscapeKeepsInsetsAndHeight() {
        let layout = WBKeyLayout(width: 932, orientation: .landscape, horizontalInset: 60)
        XCTAssertEqual(layout.totalHeight, 190)
        XCTAssertEqual(layout.key(.character("q"))!.frame.minX, 63, accuracy: 0.001)
        XCTAssertEqual(layout.key(.character("q"))!.hitFrame.minX, 0)
    }

    // MARK: - Helpers

    private func landscapeLayouts() -> [WBKeyLayout] {
        [
            WBKeyLayout(width: 667, orientation: .landscape),
            WBKeyLayout(width: 852, orientation: .landscape, layer: .numbers),
            WBKeyLayout(width: 956, orientation: .landscape, field: .email, horizontalInset: 60),
        ]
    }

    private func bottomRow(_ layout: WBKeyLayout) -> [WBKeyKind] {
        layout.keys.filter { $0.row == 3 }.sorted { $0.frame.minX < $1.frame.minX }.map(\.kind)
    }

    private func describe(_ layout: WBKeyLayout) -> String {
        "\(layout.width)pt \(layout.orientation) \(layout.layer) \(layout.field) globe=\(layout.showsGlobe)"
    }
}
