// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

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
// The Point types are expanded to the Point2D<...> type from the euclid crate which
// is binary compatible with Point<T>
template<typename T>
using Point2D = Point<T>;
}

/// A position in logical pixel coordinates
struct LogicalPosition : public Point<float>
{
    /// Explicitly convert a Point<float> to a LogicalPosition
    explicit LogicalPosition(const Point<float> p) : Point<float>(p) { };
    /// Default construct a LogicalPosition in the origin
    LogicalPosition() : Point<float> { 0., 0. } { };
};
/// A position in physical pixel coordinates
struct PhysicalPosition : public Point<int32_t>
{
    /// Explicitly convert a Point<int32_t> to a LogicalPosition
    explicit PhysicalPosition(const Point<int32_t> p) : Point<int32_t>(p) { };
    /// Default construct a PhysicalPosition in the origin
    PhysicalPosition() : Point<int32_t> { 0, 0 } { };
};

}
