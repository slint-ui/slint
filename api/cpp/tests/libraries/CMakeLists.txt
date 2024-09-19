# Copyright © SixtyFPS GmbH <info@slint.dev>
# SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

add_executable(libraries main.cpp)
target_link_libraries(libraries PRIVATE Slint::Slint)
slint_target_sources(libraries app-window.slint
    LIBRARY_PATHS
       helper_components=${CMAKE_CURRENT_SOURCE_DIR}/../../../../tests/helper_components/
       helper_buttons=${CMAKE_CURRENT_SOURCE_DIR}/../../../../tests/helper_components/test_button.slint
)

