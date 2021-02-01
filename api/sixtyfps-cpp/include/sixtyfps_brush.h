/* LICENSE BEGIN
    This file is part of the SixtyFPS Project -- https://sixtyfps.io
    Copyright (c) 2020 Olivier Goffart <olivier.goffart@sixtyfps.io>
    Copyright (c) 2020 Simon Hausmann <simon.hausmann@sixtyfps.io>

    SPDX-License-Identifier: GPL-3.0-only
    This file is also available under commercial licensing terms.
    Please contact info@sixtyfps.io for more information.
LICENSE END */
#pragma once
#include <string_view>
#include "sixtyfps_color.h"
#include "sixtyfps_brush_internal.h"
#include "sixtyfps_string.h"

namespace sixtyfps {

using cbindgen_private::types::GradientStop;

class Brush
{
public:
    Brush() : data(Inner::NoBrush()) { }
    explicit Brush(const Color &color) : data(Inner::SolidColor(color.inner)) { }
    explicit Brush(float angle, const GradientStop *firstStop, int stopCount)
        : data(Inner::LinearGradient(make_linear_gradient(angle, firstStop, stopCount)))
    {
    }

    friend bool operator==(const Brush &a, const Brush &b) { return a.data == b.data; }
    friend bool operator!=(const Brush &a, const Brush &b) { return a.data != b.data; }

private:
    static SharedVector<GradientStop>
    make_linear_gradient(float angle, const GradientStop *firstStop, int stopCount)
    {
        SharedVector<GradientStop> gradient;
        gradient.push_back({ Color::from_argb_encoded(0).inner, angle });
        for (int i = 0; i < stopCount; ++i, ++firstStop)
            gradient.push_back(*firstStop);
        return gradient;
    }

    using Tag = cbindgen_private::types::Brush::Tag;
    using Inner = cbindgen_private::types::Brush;
    Inner data;
};

}
