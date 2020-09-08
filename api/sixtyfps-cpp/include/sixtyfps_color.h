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

class Color
{
public:
    Color() { inner.red = inner.green = inner.blue = inner.alpha = 0; }
    static Color from_argb_encoded(uint32_t argb_encoded)
    {
        Color col;
        col.inner.red = (argb_encoded >> 16) & 0xff;
        col.inner.green = (argb_encoded >> 8) & 0xff;
        col.inner.blue = argb_encoded & 0xff;
        col.inner.alpha = (argb_encoded >> 24) & 0xff;
        return col;
    }

    uint32_t as_argb_encoded() const
    {
        return (uint32_t(inner.red) << 16) | (uint32_t(inner.green) << 8) | uint32_t(inner.blue)
                | (uint32_t(inner.alpha) << 24);
    }

    friend bool operator==(const Color &lhs, const Color &rhs)
    {
        return lhs.inner.red == rhs.inner.red && lhs.inner.green == rhs.inner.green
                && lhs.inner.blue == rhs.inner.blue && lhs.inner.alpha == rhs.inner.alpha;
    }

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
