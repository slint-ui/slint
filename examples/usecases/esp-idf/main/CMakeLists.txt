# Copyright © SixtyFPS GmbH <info@slint.dev>
# SPDX-License-Identifier: MIT


idf_component_register(
    SRCS "main.cpp"
    INCLUDE_DIRS "."
    REQUIRES esp32_p4_function_ev_board_noglib slint
)

set(SLINT_STYLE "cosmic-light" CACHE STRING "Style for demo")
slint_target_sources(${COMPONENT_LIB} ../../ui/app.slint)
target_link_options(${COMPONENT_LIB} PUBLIC -Wl,--allow-multiple-definition)
