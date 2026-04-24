// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

import Testing
import Slint

@Suite("SlintBrush")
struct SlintBrushTests {

    @Test func solidColorWrapsColor() {
        let color = SlintColor(red: 100, green: 150, blue: 200)
        let brush = SlintBrush.solidColor(color)
        #expect(brush == .solidColor(color))
    }

    @Test func transparentIsZeroAlphaSolidColor() {
        if case .solidColor(let c) = SlintBrush.transparent {
            #expect(c.alpha == 0)
        } else {
            Issue.record("SlintBrush.transparent is not a .solidColor")
        }
    }

    @Test func colorFactoryDefaultAlpha() {
        let brush = SlintBrush.color(red: 255, green: 128, blue: 0)
        #expect(brush == .solidColor(SlintColor(red: 255, green: 128, blue: 0, alpha: 255)))
    }

    @Test func colorFactoryExplicitAlpha() {
        let brush = SlintBrush.color(red: 255, green: 0, blue: 0, alpha: 128)
        #expect(brush == .solidColor(SlintColor(red: 255, green: 0, blue: 0, alpha: 128)))
    }

    @Test func equalBrushes() {
        let a = SlintBrush.color(red: 100, green: 200, blue: 50)
        let b = SlintBrush.color(red: 100, green: 200, blue: 50)
        #expect(a == b)
    }

    @Test func unequalBrushes() {
        let a = SlintBrush.color(red: 100, green: 200, blue: 50)
        let b = SlintBrush.color(red: 101, green: 200, blue: 50)
        #expect(a != b)
    }
}
