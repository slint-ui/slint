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
#include <iostream>

namespace sixtyfps::testing {
inline void mock_elapsed_time(int64_t time_in_ms)
{
    cbindgen_private::sixtyfps_mock_elapsed_time(time_in_ms);
}
template<typename Component>
inline void send_mouse_click(const ComponentHandle<Component> *component, float x, float y)
{
    cbindgen_private::sixtyfps_send_mouse_click(
        reinterpret_cast<const vtable::VRc<private_api::ComponentVTable>*>(component),
        x, y, &(*component)->window);
}

template<typename Component>
inline void send_keyboard_string_sequence(const Component &component,
                                          const sixtyfps::SharedString &str)
{
    cbindgen_private::send_keyboard_string_sequence(&str, {}, &component.window);
}

#define assert_eq(A, B)                                                                            \
    sixtyfps::testing::private_api::assert_eq_impl(A, B, #A, #B, __FILE__, __LINE__)

namespace private_api {
template<typename A, typename B>
void assert_eq_impl(const A &a, const B &b, const char *a_str, const char *b_str, const char *file,
                    int line)
{
    if (a != b) {
        std::cerr << file << ":" << line << ": assert_eq FAILED!\n"
                  << a_str << ": " << a << "\n"
                  << b_str << ": " << b << std::endl;
        std::abort();
    }
}
}

} // namespace sixtyfps
