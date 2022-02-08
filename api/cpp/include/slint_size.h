// Copyright © SixtyFPS GmbH <info@sixtyfps.io>
// SPDX-License-Identifier: (GPL-3.0-only OR LicenseRef-SixtyFPS-commercial)

#pragma once


namespace slint {

/// The Size structure is used to represent a two-dimensional size
/// with width and height.
template <typename T>
struct Size {
    /// The width of the size
    T width;
    /// The height of the size
    T height;

    /// Compares with \a other and returns true if they are equal; false otherwise.
    bool operator==(const Size &other) const = default;
};

namespace cbindgen_private {
    // The Size types are expanded to the Size2D<...> type from the euclid crate which
    // is binary compatible with Size<T>
    template <typename T> using Size2D = Size<T>;
}


}
