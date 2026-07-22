// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

#pragma once

#include <cstdint>

#if UINTPTR_MAX == 0xFFFFFFFF
#    define SLINT_TARGET_32
#elif UINTPTR_MAX == 0xFFFFFFFFFFFFFFFFu
#    define SLINT_TARGET_64
#endif

#if !defined(DOXYGEN)
#    if defined(_MSC_VER)
#        define SLINT_DLL_IMPORT __declspec(dllimport)
#    elif defined(__GNUC__)
#        if defined(_WIN32) || defined(_WIN64)
#            define SLINT_DLL_IMPORT __declspec(dllimport)
#        else
#            define SLINT_DLL_IMPORT __attribute__((visibility("default")))
#        endif
#    else
#        define SLINT_DLL_IMPORT
#    endif
#endif // !defined(DOXYGEN)

#if defined(__cpp_deleted_function) && __cpp_deleted_function >= 202403L && __cplusplus > 202302L
#    define SLINT_DELETED_FUNCTION(reason) delete (reason)
#else
#    define SLINT_DELETED_FUNCTION(reason) delete
#endif
