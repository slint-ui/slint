#pragma once
#include <string_view>
#include "sixtyfps_string_internal.h"

namespace sixtyfps {

struct SharedString
{
    SharedString() { internal::sixtyfps_shared_string_from_bytes(this, "", 0); }
    SharedString(std::string_view s)
    {
        internal::sixtyfps_shared_string_from_bytes(this, s.data(), s.size());
    }
    SharedString(const SharedString &other)
    {
        internal::sixtyfps_shared_string_clone(this, &other);
    }
    ~SharedString() { internal::sixtyfps_shared_string_drop(this); }
    SharedString &operator=(const SharedString &other)
    {
        internal::sixtyfps_shared_string_drop(this);
        internal::sixtyfps_shared_string_clone(this, &other);
    }
    SharedString &operator=(std::string_view s)
    {
        internal::sixtyfps_shared_string_drop(this);
        internal::sixtyfps_shared_string_from_bytes(this, s.data(), s.size());
    }
    SharedString &operator=(SharedString &&other) { std::swap(inner, other.inner); }

    operator std::string_view() const { return internal::sixtyfps_shared_string_bytes(this); }
    auto data() const -> const char * { return internal::sixtyfps_shared_string_bytes(this); }

    static SharedString from_number(double n) { return SharedString(n); }

private:
    /// Use SharedString::from_number
    explicit SharedString(double n) { internal::sixtyfps_shared_string_from_number(this, n); }
    void *inner; // opaque
};
}
