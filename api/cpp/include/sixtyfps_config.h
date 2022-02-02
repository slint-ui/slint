// Copyright © SixtyFPS GmbH <info@sixtyfps.io>
// SPDX-License-Identifier: (GPL-3.0-only OR LicenseRef-SixtyFPS-commercial)

#pragma once

#include <cstdint>

#if UINTPTR_MAX == 0xFFFFFFFF
#    define SLINT_TARGET_32
#elif UINTPTR_MAX == 0xFFFFFFFFFFFFFFFFu
#    define SLINT_TARGET_64
#endif

#if !defined(DOXYGEN)
#    if defined(_MSC_VER)
#        define SIXTYFPS_DLL_IMPORT __declspec(dllimport)
#    elif defined(__GNUC__)
#        if defined(_WIN32) || defined(_WIN64)
#            define SIXTYFPS_DLL_IMPORT __declspec(dllimport)
#        else
#            define SIXTYFPS_DLL_IMPORT __attribute__((visibility("default")))
#        endif
#    else
#        define SIXTYFPS_DLL_IMPORT
#    endif
#endif // !defined(DOXYGEN)
