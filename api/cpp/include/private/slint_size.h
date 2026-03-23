// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

#pragma once

#include <cstdint>

namespace slint {

/// The Size structure is used to represent a two-dimensional size
/// with width and height.
template<typename T>
struct Size
{
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
template<typename T>
using Size2D = Size<T>;
}

/// A size given in logical pixels
struct LogicalSize : public Size<float>
{
    /// Explicitly convert a Size<float> to a LogicalSize
    explicit constexpr LogicalSize(const Size<float> s = { 0, 0 }) : Size<float>(s) { }
};
/// A size given in physical pixels.
struct PhysicalSize : public Size<uint32_t>
{
    /// Explicitly convert a Size<uint32_t> to a LogicalSize
    explicit constexpr PhysicalSize(const Size<uint32_t> s = { 0, 0 }) : Size<uint32_t>(s) { }
};

}
