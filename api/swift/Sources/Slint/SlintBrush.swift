// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

/// A brush used for painting in Slint.
///
/// For Phase 1, only solid colors are supported. Gradient support
/// will be added in Phase 2 (requires SharedVector<GradientStop> FFI).
public enum SlintBrush: Sendable, Equatable {
    /// A solid color brush.
    case solidColor(SlintColor)

    /// Creates a solid color brush from RGBA components (0-255).
    public static func color(red: UInt8, green: UInt8, blue: UInt8, alpha: UInt8 = 255) -> SlintBrush {
        .solidColor(SlintColor(red: red, green: green, blue: blue, alpha: alpha))
    }

    /// A transparent brush.
    public static let transparent = SlintBrush.solidColor(.transparent)
}
