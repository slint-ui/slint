// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

import Testing
import Slint

@Suite("SlintImage")
struct SlintImageTests {

    // MARK: - Default image

    @Test func defaultImageHasZeroSize() {
        let image = SlintImage()
        #expect(image.size.width == 0)
        #expect(image.size.height == 0)
    }

    @Test func defaultImageHasNoPath() {
        let image = SlintImage()
        #expect(image.path == nil)
    }

    // MARK: - Cloning

    @Test func cloneHasSameSize() {
        let original = SlintImage()
        let clone = SlintImage(cloning: original)
        #expect(clone.size.width == original.size.width)
        #expect(clone.size.height == original.size.height)
    }

    @Test func cloneHasSamePath() {
        let original = SlintImage()
        let clone = SlintImage(cloning: original)
        #expect(clone.path == original.path)
    }
}
