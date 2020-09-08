/* LICENSE BEGIN
    This file is part of the SixtyFPS Project -- https://sixtyfps.io
    Copyright (c) 2020 Olivier Goffart <olivier.goffart@sixtyfps.io>
    Copyright (c) 2020 Simon Hausmann <simon.hausmann@sixtyfps.io>

    SPDX-License-Identifier: GPL-3.0-only
    This file is also available under commercial licensing terms.
    Please contact info@sixtyfps.io for more information.
LICENSE END */
#pragma once

#include "sixtyfps_color_internal.h"
#include "sixtyfps_properties.h"

#include <stdint.h>

namespace sixtyfps {

/// Color represents a color in the SixtyFPS run-time, represented using 8-bit channels for
/// red, green, blue and the alpha (opacity).
class Color
{
public:
    /// Default constructs a new color that is entirely transparent.
    Color() { inner.red = inner.green = inner.blue = inner.alpha = 0; }

    /// Construct a color from an integer encoded as `0xAARRGGBB`
    static Color from_argb_encoded(uint32_t argb_encoded)
    {
        Color col;
        col.inner.red = (argb_encoded >> 16) & 0xff;
        col.inner.green = (argb_encoded >> 8) & 0xff;
        col.inner.blue = argb_encoded & 0xff;
        col.inner.alpha = (argb_encoded >> 24) & 0xff;
        return col;
    }

    /// Returns `(alpha, red, green, blue)` encoded as uint32_t.
    uint32_t as_argb_encoded() const
    {
        return (uint32_t(inner.red) << 16) | (uint32_t(inner.green) << 8) | uint32_t(inner.blue)
                | (uint32_t(inner.alpha) << 24);
    }

    /// Returns the red channel of the color as u8 in the range 0..255.
    uint8_t red() const { return inner.red; }

    /// Returns the green channel of the color as u8 in the range 0..255.
    uint8_t green() const { return inner.green; }

    /// Returns the blue channel of the color as u8 in the range 0..255.
    uint8_t blue() const { return inner.blue; }

    /// Returns the alpha channel of the color as u8 in the range 0..255.
    uint8_t alpha() const { return inner.alpha; }

    /// Returns true if \a lhs has the same values for the individual color channels as \rhs; false
    /// otherwise.
    friend bool operator==(const Color &lhs, const Color &rhs)
    {
        return lhs.inner.red == rhs.inner.red && lhs.inner.green == rhs.inner.green
                && lhs.inner.blue == rhs.inner.blue && lhs.inner.alpha == rhs.inner.alpha;
    }

    /// Returns true if \a lhs has any different values for the individual color channels as \rhs;
    /// false otherwise.
    friend bool operator!=(const Color &lhs, const Color &rhs) { return !(lhs == rhs); }

private:
    cbindgen_private::types::Color inner;
};

template<>
void Property<Color>::set_animated_value(const Color &new_value,
                                         const cbindgen_private::PropertyAnimation &animation_data)
{
    cbindgen_private::sixtyfps_property_set_animated_value_color(&inner, value, new_value,
                                                                 &animation_data);
}

template<>
template<typename F>
void Property<Color>::set_animated_binding(
        F binding, const cbindgen_private::PropertyAnimation &animation_data)
{
    cbindgen_private::sixtyfps_property_set_animated_binding_color(
            &inner,
            [](void *user_data, Color *value) {
                *reinterpret_cast<Color *>(value) = (*reinterpret_cast<F *>(user_data))();
            },
            new F(binding), [](void *user_data) { delete reinterpret_cast<F *>(user_data); },
            &animation_data);
}

}
