// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

#pragma once

#include "private/slint_builtin_functions_internal.h"

namespace slint::private_api {
// Calls a `slint_builtin_*` function, which take the result first, then pointers to the arguments.
// Each argument converts to its parameter type, such as `float` to `double`.
template<typename R, typename... P, typename... A>
inline R call_builtin(void (*f)(R *, const P *...), const A &...args)
{
    R result {};
    f(&result, &static_cast<const P &>(args)...);
    return result;
}

}
