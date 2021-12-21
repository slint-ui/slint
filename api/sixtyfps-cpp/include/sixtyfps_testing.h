// Copyright © SixtyFPS GmbH <info@sixtyfps.io>
// SPDX-License-Identifier: (GPL-3.0-only OR LicenseRef-SixtyFPS-commercial)

#pragma once
#include "sixtyfps.h"
#include <iostream>

namespace sixtyfps::testing {

inline void init()
{
    cbindgen_private::sixtyfps_testing_init_backend();
}

inline void mock_elapsed_time(int64_t time_in_ms)
{
    cbindgen_private::sixtyfps_mock_elapsed_time(time_in_ms);
}
template<typename Component>
inline void send_mouse_click(const Component *component, float x, float y)
{
    auto crc = *component->self_weak.into_dyn().lock();
    cbindgen_private::sixtyfps_send_mouse_click(&crc, x, y, &component->m_window.window_handle());
}

template<typename Component>
inline void send_keyboard_string_sequence(const Component *component,
                                          const sixtyfps::SharedString &str,
                                          cbindgen_private::KeyboardModifiers modifiers = {})
{
    cbindgen_private::send_keyboard_string_sequence(&str, modifiers,
                                                    &component->m_window.window_handle());
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
