// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

import Testing
import Slint

@Suite("SlintColor")
struct SlintColorTests {

    // MARK: - Initialisation & component access

    @Test func initStoresComponents() {
        let c = SlintColor(red: 10, green: 20, blue: 30, alpha: 200)
        #expect(c.red == 10)
        #expect(c.green == 20)
        #expect(c.blue == 30)
        #expect(c.alpha == 200)
    }

    @Test func defaultAlphaIs255() {
        let c = SlintColor(red: 100, green: 150, blue: 200)
        #expect(c.alpha == 255)
    }

    // MARK: - Named constants

    @Test func namedBlack() {
        #expect(SlintColor.black == SlintColor(red: 0, green: 0, blue: 0))
    }

    @Test func namedWhite() {
        #expect(SlintColor.white == SlintColor(red: 255, green: 255, blue: 255))
    }

    @Test func namedRed() {
        #expect(SlintColor.red == SlintColor(red: 255, green: 0, blue: 0))
    }

    @Test func namedTransparentHasZeroAlpha() {
        #expect(SlintColor.transparent.alpha == 0)
    }

    // MARK: - Equatable

    @Test func equalColors() {
        let a = SlintColor(red: 50, green: 100, blue: 150, alpha: 200)
        let b = SlintColor(red: 50, green: 100, blue: 150, alpha: 200)
        #expect(a == b)
    }

    @Test func unequalColors() {
        let a = SlintColor(red: 50, green: 100, blue: 150)
        let b = SlintColor(red: 51, green: 100, blue: 150)
        #expect(a != b)
    }

    // MARK: - Brightness / transparency adjustments

    @Test func brighterProducesBrighterColor() {
        let base = SlintColor(red: 100, green: 100, blue: 100)
        let bright = base.brighter(factor: 0.5)
        // At least one channel must increase
        #expect(bright.red > base.red || bright.green > base.green || bright.blue > base.blue)
    }

    @Test func darkerProducesDarkerColor() {
        let base = SlintColor(red: 200, green: 200, blue: 200)
        let dark = base.darker(factor: 0.5)
        #expect(dark.red < base.red || dark.green < base.green || dark.blue < base.blue)
    }

    @Test func transparentizeReducesAlpha() {
        let base = SlintColor(red: 100, green: 100, blue: 100, alpha: 200)
        let trans = base.transparentize(factor: 0.5)
        #expect(trans.alpha < base.alpha)
    }

    @Test func withAlphaZeroIsFullyTransparent() {
        let c = SlintColor(red: 100, green: 100, blue: 100).withAlpha(0.0)
        #expect(c.alpha == 0)
    }

    @Test func withAlphaOneIsFullyOpaque() {
        let c = SlintColor(red: 100, green: 100, blue: 100).withAlpha(1.0)
        #expect(c.alpha == 255)
    }

    @Test func withAlphaHalfIsApproxHalf() {
        let c = SlintColor(red: 100, green: 100, blue: 100).withAlpha(0.5)
        // 0.5 * 255 = 127.5 — allow rounding to either 127 or 128
        #expect(c.alpha == 127 || c.alpha == 128)
    }

    // MARK: - Mix

    @Test func mixFactorOneReturnsSelf() {
        let black = SlintColor.black
        let white = SlintColor.white
        #expect(black.mix(with: white, factor: 1.0) == black)
    }

    @Test func mixFactorZeroReturnsOther() {
        let black = SlintColor.black
        let white = SlintColor.white
        #expect(black.mix(with: white, factor: 0.0) == white)
    }

    @Test func mixSameColorReturnsSameColor() {
        let color = SlintColor(red: 80, green: 160, blue: 240)
        #expect(color.mix(with: color, factor: 0.5) == color)
    }

    // MARK: - HSVA round-trip

    @Test func hsvaRoundTripPureRed() {
        let red = SlintColor.red
        let hsva = red.toHSVA()
        // Pure red: full saturation, full value
        #expect(hsva.saturation > 0.99)
        #expect(hsva.value > 0.99)
        #expect(hsva.alpha > 0.99)
        let back = SlintColor.fromHSVA(hue: hsva.hue, saturation: hsva.saturation, value: hsva.value, alpha: hsva.alpha)
        #expect(back.red > 200)
        #expect(back.green < 10)
        #expect(back.blue < 10)
    }

    @Test func hsvaRoundTripBlack() {
        let black = SlintColor.black
        let hsva = black.toHSVA()
        #expect(hsva.value < 0.01)
        let back = SlintColor.fromHSVA(hue: hsva.hue, saturation: hsva.saturation, value: hsva.value, alpha: hsva.alpha)
        #expect(back.red < 5)
        #expect(back.green < 5)
        #expect(back.blue < 5)
    }

    // MARK: - OKLCh round-trip

    @Test func oklchRoundTrip() {
        let original = SlintColor(red: 100, green: 150, blue: 200)
        let oklch = original.toOKLCh()
        let back = SlintColor.fromOKLCh(lightness: oklch.lightness, chroma: oklch.chroma, hue: oklch.hue, alpha: oklch.alpha)
        // Allow ±2 for color-space rounding errors
        #expect(abs(Int(back.red) - Int(original.red)) <= 2)
        #expect(abs(Int(back.green) - Int(original.green)) <= 2)
        #expect(abs(Int(back.blue) - Int(original.blue)) <= 2)
    }

    @Test func oklchBlackHasZeroLightness() {
        let oklch = SlintColor.black.toOKLCh()
        #expect(oklch.lightness < 0.01)
    }

    // MARK: - CustomStringConvertible

    @Test func descriptionFormat() {
        let c = SlintColor(red: 10, green: 20, blue: 30, alpha: 40)
        #expect(c.description == "SlintColor(red: 10, green: 20, blue: 30, alpha: 40)")
    }
}
