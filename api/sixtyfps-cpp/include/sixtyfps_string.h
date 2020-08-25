/* LICENSE BEGIN

    This file is part of the Sixty FPS Project

    Copyright (c) 2020 Olivier Goffart <olivier.goffart@sixtyfps.io>
    Copyright (c) 2020 Simon Hausmann <simon.hausmann@sixtyfps.io>

    SPDX-License-Identifier: GPL-3.0-only

LICENSE END */
#pragma once
#include <string_view>
#include "sixtyfps_string_internal.h"

namespace sixtyfps {

struct SharedString
{
    SharedString() { cbindgen_private::sixtyfps_shared_string_from_bytes(this, "", 0); }
    SharedString(std::string_view s)
    {
        cbindgen_private::sixtyfps_shared_string_from_bytes(this, s.data(), s.size());
    }
    SharedString(const char *s) : SharedString(std::string_view(s)) { }
    SharedString(const SharedString &other)
    {
        cbindgen_private::sixtyfps_shared_string_clone(this, &other);
    }
    ~SharedString() { cbindgen_private::sixtyfps_shared_string_drop(this); }
    SharedString &operator=(const SharedString &other)
    {
        cbindgen_private::sixtyfps_shared_string_drop(this);
        cbindgen_private::sixtyfps_shared_string_clone(this, &other);
        return *this;
    }
    SharedString &operator=(std::string_view s)
    {
        cbindgen_private::sixtyfps_shared_string_drop(this);
        cbindgen_private::sixtyfps_shared_string_from_bytes(this, s.data(), s.size());
        return *this;
    }
    SharedString &operator=(SharedString &&other)
    {
        std::swap(inner, other.inner);
        return *this;
    }

    operator std::string_view() const { return cbindgen_private::sixtyfps_shared_string_bytes(this); }
    auto data() const -> const char * { return cbindgen_private::sixtyfps_shared_string_bytes(this); }

    static SharedString from_number(double n) { return SharedString(n); }

    friend bool operator==(const SharedString &a, const SharedString &b)
    {
        return std::string_view(a) == std::string_view(b);
    }
    friend bool operator!=(const SharedString &a, const SharedString &b)
    {
        return std::string_view(a) != std::string_view(b);
    }

private:
    /// Use SharedString::from_number
    explicit SharedString(double n) { cbindgen_private::sixtyfps_shared_string_from_number(this, n); }
    void *inner; // opaque
};
}
