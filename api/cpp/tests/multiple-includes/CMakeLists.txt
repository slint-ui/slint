# Copyright © SixtyFPS GmbH <info@slint.dev>
# SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

add_executable(multiple-includes main.cpp logic.cpp)
target_link_libraries(multiple-includes PRIVATE Slint::Slint)
slint_target_sources(multiple-includes app-window.slint COMPILATION_UNITS 0)
slint_target_sources(multiple-includes another-window.slint NAMESPACE other COMPILATION_UNITS 0)

