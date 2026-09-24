# Copyright © SixtyFPS GmbH <info@slint.dev>
# SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

add_executable(project_file_entry main.cpp)
target_link_libraries(project_file_entry PRIVATE Slint::Slint)
slint_target_sources(project_file_entry slint-project.json)
