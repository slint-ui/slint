# Copyright © SixtyFPS GmbH <info@slint.dev>
# SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-1.0 OR LicenseRef-Slint-commercial

if (WIN32)
add_executable(platform_native WIN32 main.cpp appview.cpp)
target_link_libraries(platform_native PRIVATE Slint::Slint)
slint_target_sources(platform_native appwindow.slint)
endif(WIN32)

