// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

#pragma once
#include "slint-testing.h"
#include <iostream>

// this file contains function useful for internal testing

namespace slint::private_api::testing {

inline void mock_elapsed_time(int64_t time_in_ms)
{
    cbindgen_private::slint_mock_elapsed_time(time_in_ms);
}

inline uint64_t get_mocked_time()
{
    return cbindgen_private::slint_get_mocked_time();
}

template<typename Component>
inline void send_mouse_click(const Component *component, float x, float y)
{
    cbindgen_private::slint_send_mouse_click(x, y, &component->window().window_handle());
}

template<typename Component>
inline void send_keyboard_char(const Component *component, const slint::SharedString &str,
                               bool pressed)
{
    cbindgen_private::slint_send_keyboard_char(&str, pressed, &component->window().window_handle());
}

template<typename Component>
inline void send_keyboard_string_sequence(const Component *component,
                                          const slint::SharedString &str)
{
    cbindgen_private::send_keyboard_string_sequence(&str, &component->window().window_handle());
}

#define assert_eq(A, B)                                                                            \
    slint::private_api::testing::assert_eq_impl(A, B, #A, #B, __FILE__, __LINE__)

template<typename A, std::equality_comparable_with<A> B>
void assert_eq_impl(const A &a, const B &b, const char *a_str, const char *b_str, const char *file,
                    int line)
{
    bool nok = true;
    if constexpr (std::is_integral_v<A> && std::is_integral_v<B>) {
        // Do a cast to the common type to avoid warning about signed vs. unsigned compare
        using T = std::common_type_t<A, B>;
        nok = T(a) != T(b);
    } else if constexpr (std::is_floating_point_v<A> && std::is_floating_point_v<B>) {
        const double dEpsilon = 0.000001; // or some other small number
        nok = fabs(a - b) > dEpsilon * fabs(a);
    } else {
        nok = a != b;
    }
    if (nok) {
        std::cerr << file << ":" << line << ": assert_eq FAILED!\n"
                  << a_str << ": " << a << "\n"
                  << b_str << ": " << b << std::endl;
        std::abort();
    }
}

} // namespace slint
