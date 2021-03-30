/* LICENSE BEGIN
    This file is part of the SixtyFPS Project -- https://sixtyfps.io
    Copyright (c) 2020 Olivier Goffart <olivier.goffart@sixtyfps.io>
    Copyright (c) 2020 Simon Hausmann <simon.hausmann@sixtyfps.io>

    SPDX-License-Identifier: GPL-3.0-only
    This file is also available under commercial licensing terms.
    Please contact info@sixtyfps.io for more information.
LICENSE END */
#pragma once

#include <cstdint>

#if UINTPTR_MAX == 0xFFFFFFFF
#    define SIXTYFPS_TARGET_32
#elif UINTPTR_MAX == 0xFFFFFFFFFFFFFFFFu
#    define SIXTYFPS_TARGET_64
#endif
