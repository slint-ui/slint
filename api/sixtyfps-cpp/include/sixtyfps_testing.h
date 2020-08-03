#pragma once

#include "sixtyfps.h"

namespace sixtyfps::testing {
inline void mock_elapsed_time(int64_t time_in_ms)
{
    internal::sixtyfps_mock_elapsed_time(time_in_ms);
}
} // namespace sixtyfps
