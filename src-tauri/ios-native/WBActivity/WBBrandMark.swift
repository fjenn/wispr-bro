// SPDX-License-Identifier: GPL-3.0-or-later
// The Wispr Bro mark: the five bars of the app icon
// (src/assets/fork/art/brand/mark.svg), drawn as a SwiftUI shape.

import SwiftUI

struct WBBrandMark: Shape {
    /// x, y, height in the mark's 32 × 32 box; every bar is 3.12 wide.
    private static let bars: [(CGFloat, CGFloat, CGFloat)] = [
        (4.04, 10.37, 11.27),
        (9.24, 5.82, 20.37),
        (14.44, 3.00, 26.00),
        (19.64, 7.77, 16.47),
        (24.84, 11.23, 9.53),
    ]

    func path(in rect: CGRect) -> Path {
        let unit = min(rect.width, rect.height) / 32
        let origin = CGPoint(x: rect.midX - 16 * unit, y: rect.midY - 16 * unit)
        var path = Path()
        for (x, y, height) in Self.bars {
            path.addRoundedRect(
                in: CGRect(x: origin.x + x * unit, y: origin.y + y * unit, width: 3.12 * unit, height: height * unit),
                cornerSize: CGSize(width: 1.56 * unit, height: 1.56 * unit)
            )
        }
        return path
    }
}
