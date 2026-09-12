// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

#pragma once
#include <cstdint>
#include "private/slint_generated_public.h"

namespace slint {

/// A 16bit pixel that has 5 red bits, 6 green bits and 5 blue bits
struct Rgb565Pixel
{
    /// The blue component, encoded in 5 bits.
    uint16_t b : 5;
    /// The green component, encoded in 6 bits.
    uint16_t g : 6;
    /// The red component, encoded in 5 bits.
    uint16_t r : 5;

    /// Default constructor.
    constexpr Rgb565Pixel() : b(0), g(0), r(0) { }

    /// \brief Constructor that constructs from an Rgb8Pixel.
    explicit constexpr Rgb565Pixel(const Rgb8Pixel &pixel)
        : b(pixel.b >> 3), g(pixel.g >> 2), r(pixel.r >> 3)
    {
    }

    /// \brief Get the red component as an 8-bit value.
    ///
    /// The bits are shifted so that the result is between 0 and 255.
    /// \return The red component as an 8-bit value.
    constexpr uint8_t red() const { return (r << 3) | (r >> 2); }

    /// \brief Get the green component as an 8-bit value.
    ///
    /// The bits are shifted so that the result is between 0 and 255.
    /// \return The green component as an 8-bit value.
    constexpr uint8_t green() const { return (g << 2) | (g >> 4); }

    /// \brief Get the blue component as an 8-bit value.
    ///
    /// The bits are shifted so that the result is between 0 and 255.
    /// \return The blue component as an 8-bit value.
    constexpr uint8_t blue() const { return (b << 3) | (b >> 2); }

    /// \brief Convert to Rgb8Pixel.
    constexpr operator Rgb8Pixel() const { return { red(), green(), blue() }; }

    /// Returns true if \a lhs \a rhs are pixels with identical colors.
    friend bool operator==(const Rgb565Pixel &lhs, const Rgb565Pixel &rhs) = default;
};

}
