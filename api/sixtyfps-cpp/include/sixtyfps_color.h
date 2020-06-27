#pragma once

#include "sixtyfps_color_internal.h"

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

}
