# Copyright © SixtyFPS GmbH <info@slint-ui.com>
# SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-1.0 OR LicenseRef-Slint-commercial

add_executable(multiple-includes main.cpp logic.cpp)
target_link_libraries(multiple-includes PRIVATE Slint::Slint)
slint_target_sources(multiple-includes appwindow.slint)

