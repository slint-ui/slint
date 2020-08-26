/* LICENSE BEGIN
    This file is part of the SixtyFPS Project -- https://sixtyfps.io
    Copyright (c) 2020 Olivier Goffart <olivier.goffart@sixtyfps.io>
    Copyright (c) 2020 Simon Hausmann <simon.hausmann@sixtyfps.io>

    SPDX-License-Identifier: GPL-3.0-only
    This file is also available under commercial licensing terms.
    Please contact info@sixtyfps.io for more information.
LICENSE END */
#pragma once

#include "sixtyfps.h"

namespace sixtyfps::testing {
inline void mock_elapsed_time(int64_t time_in_ms)
{
    cbindgen_private::sixtyfps_mock_elapsed_time(time_in_ms);
}
template<typename Component>
inline void send_mouse_click(Component &component, float x, float y) {
    cbindgen_private::sixtyfps_send_mouse_click({&Component::component_type, &component}, x, y);
}
} // namespace sixtyfps
