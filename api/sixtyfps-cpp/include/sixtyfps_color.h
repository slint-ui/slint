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

class Color;

/// ARGBColor stores the red, green, blue and alpha components of a color
/// with the precision of the template parameter T. For example if T is float,
/// the values are normalized between 0 and 1. If T is uint8_t, they values range
/// is 0 to 255.
template<typename T>
struct ARGBColor
{
    /// The alpha component.
    T alpha;
    /// The red component.
    T red;
    /// The green component.
    T green;
    /// The blue component.
    T blue;

    /// Creates a new ARGBColor instance from a given color. This template function is
    /// specialized and thus implemented for T == uint8_t and T == float.
    ARGBColor(const Color &col);
};

/// Color represents a color in the SixtyFPS run-time, represented using 8-bit channels for
/// red, green, blue and the alpha (opacity).
class Color
{
public:
    /// Default constructs a new color that is entirely transparent.
    Color() { inner.red = inner.green = inner.blue = inner.alpha = 0; }
    Color(const ARGBColor<uint8_t> &col)
    {
        inner.red = col.red;
        inner.green = col.green;
        inner.blue = col.blue;
        inner.alpha = col.alpha;
    }
    Color(const ARGBColor<float> &col)
    {
        inner.red = col.red * 255;
        inner.green = col.green * 255;
        inner.blue = col.blue * 255;
        inner.alpha = col.alpha * 255;
    }

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

    /// Construct a color from the alpha, red, green and blue color channel parameters.
    static Color from_argb_uint8(uint8_t alpha, uint8_t red, uint8_t green, uint8_t blue)
    {
        Color col;
        col.inner.alpha = alpha;
        col.inner.red = red;
        col.inner.green = green;
        col.inner.blue = blue;
        return col;
    }

    /// Construct a color from the red, green and blue color channel parameters. The alpha
    /// channel will have the value 255.
    static Color from_rgb_uint8(uint8_t red, uint8_t green, uint8_t blue)
    {
        return from_argb_uint8(255, red, green, blue);
    }

    /// Construct a color from the alpha, red, green and blue color channel parameters.
    static Color from_argb_float(float alpha, float red, float green, float blue)
    {
        Color col;
        col.inner.alpha = alpha * 255;
        col.inner.red = red * 255;
        col.inner.green = green * 255;
        col.inner.blue = blue * 255;
        return col;
    }

    /// Construct a color from the red, green and blue color channel parameters. The alpha
    /// channel will have the value 255.
    static Color from_rgb_float(float red, float green, float blue)
    {
        return Color::from_argb_float(1.0, red, green, blue);
    }

    /// Converts this color to an ARGBColor struct for easy destructuring.
    inline ARGBColor<uint8_t> to_argb_uint() const;

    /// Converts this color to an ARGBColor struct for easy destructuring.
    inline ARGBColor<float> to_argb_float() const;

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

    /// Writes the \a color to the specified \a stream and returns a reference to the
    /// stream.
    friend std::ostream &operator<<(std::ostream &stream, const Color &color)
    {
        // Cast to uint32_t to avoid the components being interpreted as char.
        return stream << "argb(" << uint32_t(color.inner.alpha) << ", " << uint32_t(color.inner.red)
                      << ", " << uint32_t(color.inner.green) << ", " << uint32_t(color.inner.blue)
                      << ")";
    }

private:
    cbindgen_private::types::Color inner;
};

template<>
ARGBColor<uint8_t>::ARGBColor(const Color &color)
{
    red = color.red();
    green = color.green();
    blue = color.blue();
    alpha = color.alpha();
}

template<>
ARGBColor<float>::ARGBColor(const Color &color)
{
    red = float(color.red()) / 255.;
    green = float(color.green()) / 255.;
    blue = float(color.blue()) / 255.;
    alpha = float(color.alpha()) / 255.;
}

ARGBColor<uint8_t> Color::to_argb_uint() const
{
    return ARGBColor<uint8_t>(*this);
}

ARGBColor<float> Color::to_argb_float() const
{
    return ARGBColor<float>(*this);
}

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
