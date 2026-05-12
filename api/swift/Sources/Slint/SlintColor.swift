// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

@preconcurrency import SlintCBridge

/// A color value with red, green, blue, and alpha components.
///
/// `SlintColor` wraps Slint's `Color` type, which is stored as 4 bytes (RGBA).
/// Color component values range from 0 to 255.
public struct SlintColor: Sendable, Equatable {
    var inner: SlintCBridge.SlintColor

    /// Creates a color from the C bridging type (internal use).
    init(inner: SlintCBridge.SlintColor) {
        self.inner = inner
    }

    /// Creates a color from RGBA components (0-255).
    public init(red: UInt8, green: UInt8, blue: UInt8, alpha: UInt8 = 255) {
        inner = SlintCBridge.SlintColor(red: red, green: green, blue: blue, alpha: alpha)
    }

    /// The red component (0-255).
    public var red: UInt8 { inner.red }

    /// The green component (0-255).
    public var green: UInt8 { inner.green }

    /// The blue component (0-255).
    public var blue: UInt8 { inner.blue }

    /// The alpha component (0-255).
    public var alpha: UInt8 { inner.alpha }

    /// Returns a brighter version of this color.
    ///
    /// The brightness is increased by the given `factor` (e.g., 0.2 for 20% brighter).
    /// Negative values make the color darker.
    public func brighter(factor: Float) -> SlintColor {
        var result = SlintCBridge.SlintColor(red: 0, green: 0, blue: 0, alpha: 0)
        withUnsafePointer(to: inner) { col in
            slint_color_brighter(col, factor, &result)
        }
        return SlintColor(inner: result)
    }

    /// Returns a darker version of this color.
    ///
    /// The brightness is decreased by the given `factor` (e.g., 0.3 for 30% darker).
    public func darker(factor: Float) -> SlintColor {
        var result = SlintCBridge.SlintColor(red: 0, green: 0, blue: 0, alpha: 0)
        withUnsafePointer(to: inner) { col in
            slint_color_darker(col, factor, &result)
        }
        return SlintColor(inner: result)
    }

    /// Returns a more transparent version of this color.
    ///
    /// The alpha is multiplied by `(1 - factor)`. For example, `transparentize(0.5)` halves
    /// the opacity. Negative values increase the opacity.
    public func transparentize(factor: Float) -> SlintColor {
        var result = SlintCBridge.SlintColor(red: 0, green: 0, blue: 0, alpha: 0)
        withUnsafePointer(to: inner) { col in
            slint_color_transparentize(col, factor, &result)
        }
        return SlintColor(inner: result)
    }

    /// Mixes this color with another color.
    ///
    /// `factor` is clamped to [0, 1]. A factor of 1.0 returns this color,
    /// 0.0 returns the other color, and 0.5 returns an equal mix.
    public func mix(with other: SlintColor, factor: Float) -> SlintColor {
        var result = SlintCBridge.SlintColor(red: 0, green: 0, blue: 0, alpha: 0)
        withUnsafePointer(to: inner) { col1 in
            withUnsafePointer(to: other.inner) { col2 in
                slint_color_mix(col1, col2, factor, &result)
            }
        }
        return SlintColor(inner: result)
    }

    /// Returns this color with a new alpha value.
    ///
    /// `alpha` is in the range [0.0, 1.0] where 0.0 is fully transparent and 1.0 is fully opaque.
    public func withAlpha(_ alpha: Float) -> SlintColor {
        var result = SlintCBridge.SlintColor(red: 0, green: 0, blue: 0, alpha: 0)
        withUnsafePointer(to: inner) { col in
            slint_color_with_alpha(col, alpha, &result)
        }
        return SlintColor(inner: result)
    }

    /// HSVA representation of a color.
    public struct HSVA: Sendable {
        /// Hue in [0, 1].
        public var hue: Float
        /// Saturation in [0, 1].
        public var saturation: Float
        /// Value (brightness) in [0, 1].
        public var value: Float
        /// Alpha in [0, 1].
        public var alpha: Float
    }

    /// Converts this color to HSVA.
    public func toHSVA() -> HSVA {
        var h: Float = 0
        var s: Float = 0
        var v: Float = 0
        var a: Float = 0
        withUnsafePointer(to: inner) { col in
            slint_color_to_hsva(col, &h, &s, &v, &a)
        }
        return HSVA(hue: h, saturation: s, value: v, alpha: a)
    }

    /// Creates a color from HSVA components (each in [0, 1]).
    public static func fromHSVA(hue: Float, saturation: Float, value: Float, alpha: Float = 1.0) -> SlintColor {
        let c = slint_color_from_hsva(hue, saturation, value, alpha)
        return SlintColor(inner: c)
    }

    /// OKLCh representation of a color.
    public struct OKLCh: Sendable {
        /// Lightness.
        public var lightness: Float
        /// Chroma.
        public var chroma: Float
        /// Hue.
        public var hue: Float
        /// Alpha in [0, 1].
        public var alpha: Float
    }

    /// Creates a color from OKLCh components.
    public static func fromOKLCh(lightness: Float, chroma: Float, hue: Float, alpha: Float = 1.0) -> SlintColor {
        let c = slint_color_from_oklch(lightness, chroma, hue, alpha)
        return SlintColor(inner: c)
    }

    /// Converts this color to OKLCh.
    public func toOKLCh() -> OKLCh {
        var l: Float = 0
        var c: Float = 0
        var h: Float = 0
        var a: Float = 0
        withUnsafePointer(to: inner) { col in
            slint_color_to_oklch(col, &l, &c, &h, &a)
        }
        return OKLCh(lightness: l, chroma: c, hue: h, alpha: a)
    }

    // MARK: - Named Color Constants

    /// Transparent (alpha = 0).
    public static let transparent = SlintColor(red: 0, green: 0, blue: 0, alpha: 0)

    /// Black.
    public static let black = SlintColor(red: 0, green: 0, blue: 0)

    /// White.
    public static let white = SlintColor(red: 255, green: 255, blue: 255)

    /// Red.
    public static let red = SlintColor(red: 255, green: 0, blue: 0)

    /// Green.
    public static let green = SlintColor(red: 0, green: 128, blue: 0)

    /// Blue.
    public static let blue = SlintColor(red: 0, green: 0, blue: 255)

    public static func == (lhs: SlintColor, rhs: SlintColor) -> Bool {
        lhs.inner.red == rhs.inner.red
            && lhs.inner.green == rhs.inner.green
            && lhs.inner.blue == rhs.inner.blue
            && lhs.inner.alpha == rhs.inner.alpha
    }
}

extension SlintColor: CustomStringConvertible {
    public var description: String {
        "SlintColor(red: \(red), green: \(green), blue: \(blue), alpha: \(alpha))"
    }
}
