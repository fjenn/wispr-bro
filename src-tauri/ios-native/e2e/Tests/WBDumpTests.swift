// SPDX-License-Identifier: GPL-3.0-or-later
import XCTest

/// Writes the accessibility tree of the running Wispr Bro app (a debugging aid).
final class WBDumpTests: WBE2ECase {
    func testDump() {
        wispr.activate()
        sleep(3)
        shot("dump")
        note("dump", wispr.debugDescription)
    }
}
