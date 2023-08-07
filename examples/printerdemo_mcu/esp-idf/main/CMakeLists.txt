# Copyright © SixtyFPS GmbH <info@slint.dev>
# SPDX-License-Identifier: MIT


idf_component_register(
    SRCS "main.cpp"
    INCLUDE_DIRS "."
    REQUIRES slint esp-box
)

slint_target_sources(${COMPONENT_LIB} ../../ui/printerdemo.slint)
target_link_options(${COMPONENT_LIB} PUBLIC -Wl,--allow-multiple-definition)
set_target_properties(${COMPONENT_LIB} PROPERTIES SLINT_EMBED_RESOURCES "embed-for-software-renderer")
