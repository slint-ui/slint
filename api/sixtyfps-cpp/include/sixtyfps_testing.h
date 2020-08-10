#pragma once

#include "sixtyfps.h"

namespace sixtyfps::testing {
inline void mock_elapsed_time(int64_t time_in_ms)
{
    internal::sixtyfps_mock_elapsed_time(time_in_ms);
}
template<typename Component>
inline void send_mouse_click(Component &component, float x, float y) {
    internal::sixtyfps_send_mouse_click({&Component::component_type, &component}, x, y);
}
} // namespace sixtyfps
