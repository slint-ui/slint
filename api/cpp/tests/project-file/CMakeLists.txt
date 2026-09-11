# Copyright © SixtyFPS GmbH <info@slint.dev>
# SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

add_executable(project_file main.cpp)
target_link_libraries(project_file PRIVATE Slint::Slint)
slint_target_sources(project_file app-window.slint)
