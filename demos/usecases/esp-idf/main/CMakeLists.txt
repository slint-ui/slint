# Copyright © SixtyFPS GmbH <info@slint.dev>
# SPDX-License-Identifier: MIT


idf_component_register(
    SRCS "main.cpp"
    INCLUDE_DIRS "."
    REQUIRES esp32_p4_function_ev_board_noglib slint
)

slint_target_sources(${COMPONENT_LIB} ../../ui/app.slint)
set_target_properties(${COMPONENT_LIB} PROPERTIES
    SLINT_TRANSLATION_DOMAIN usecases
    SLINT_BUNDLE_TRANSLATIONS "${CMAKE_CURRENT_SOURCE_DIR}/../../lang"
)
target_link_options(${COMPONENT_LIB} PUBLIC -Wl,--allow-multiple-definition)
