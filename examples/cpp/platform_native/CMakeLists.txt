# Copyright © SixtyFPS GmbH <info@slint.dev>
# SPDX-License-Identifier: MIT

if (WIN32)
add_executable(platform_native WIN32 main.cpp appview.cpp)
target_link_libraries(platform_native PRIVATE Slint::Slint)
slint_target_sources(platform_native appwindow.slint)
endif(WIN32)

