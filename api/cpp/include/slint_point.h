// Copyright © SixtyFPS GmbH <info@slint-ui.com>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-commercial

#pragma once

#include <cstdint>

namespace slint {

/// The Point structure is used to represent a two-dimensional point
/// with x and y coordinates.
template<typename T>
struct Point
{
    /// The x coordinate of the point
    T x;
    /// The y coordinate of the point
    T y;

    /// Compares with \a other and returns true if they are equal; false otherwise.
    bool operator==(const Point &other) const = default;
};

namespace cbindgen_private {
// The Size types are expanded to the Point2D<...> type from the euclid crate which
// is binary compatible with Point<T>
template<typename T>
using Point2D = Point<T>;
}

/// A position in logical pixel coordinates
using LogicalPosition = Point<float>;
/// A position in physical pixel coordinates
using PhysicalPosition = Point<int32_t>;

}
