/* LICENSE BEGIN

    This file is part of the Sixty FPS Project

    Copyright (c) 2020 Olivier Goffart <olivier.goffart@sixtyfps.io>
    Copyright (c) 2020 Simon Hausmann <simon.hausmann@sixtyfps.io>

    SPDX-License-Identifier: GPL-3.0-only

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
    explicit Color(uint32_t argb_encoded)
    {
        inner.red = (argb_encoded >> 16) & 0xff;
        inner.green = (argb_encoded >> 8) & 0xff;
        inner.blue = argb_encoded & 0xff;
        inner.alpha = (argb_encoded >> 24) & 0xff;
    }

    friend bool operator==(const Color &lhs, const Color &rhs)
    {
        return lhs.inner.red == rhs.inner.red && lhs.inner.green == rhs.inner.green
                && lhs.inner.blue == rhs.inner.blue && lhs.inner.alpha == rhs.inner.alpha;
    }

    friend bool operator!=(const Color &lhs, const Color &rhs) { return !(lhs == rhs); }

private:
    internal::types::Color inner;
};

template<>
void Property<Color>::set_animated_value(const Color &new_value,
                                         const internal::PropertyAnimation &animation_data)
{
    internal::sixtyfps_property_set_animated_value_color(&inner, value, new_value, &animation_data);
}

template<>
template<typename F>
void Property<Color>::set_animated_binding(F binding,
                                           const internal::PropertyAnimation &animation_data)
{
    internal::sixtyfps_property_set_animated_binding_color(
            &inner,
            [](void *user_data, Color *value) {
                *reinterpret_cast<Color *>(value) = (*reinterpret_cast<F *>(user_data))();
            },
            new F(binding), [](void *user_data) { delete reinterpret_cast<F *>(user_data); },
            &animation_data);
}

}
