#pragma once

#include "sixtyfps.h"

namespace sixtyfps::testing {
    inline void ellapse_time(int64_t time_in_ms) {
        internal::sixtyfps_test_ellapse_time(time_in_ms);
    }
} // namespace sixtyfps
