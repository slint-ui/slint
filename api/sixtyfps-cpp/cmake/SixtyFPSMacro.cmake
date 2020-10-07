# LICENSE BEGIN
# This file is part of the SixtyFPS Project -- https://sixtyfps.io
# Copyright (c) 2020 Olivier Goffart <olivier.goffart@sixtyfps.io>
# Copyright (c) 2020 Simon Hausmann <simon.hausmann@sixtyfps.io>
#
# SPDX-License-Identifier: GPL-3.0-only
# This file is also available under commercial licensing terms.
# Please contact info@sixtyfps.io for more information.
# LICENSE END

function(SIXTYFPS_TARGET_60_SOURCES target)
    foreach (it IN ITEMS ${ARGN})
        get_filename_component(_60_BASE_NAME ${it} NAME_WE)
        get_filename_component(_60_ABSOLUTE ${it} REALPATH BASE_DIR ${CMAKE_CURRENT_SOURCE_DIR})

        add_custom_command(
            OUTPUT ${CMAKE_CURRENT_BINARY_DIR}/${_60_BASE_NAME}.h
            COMMAND SixtyFPS::sixtyfps_compiler ${_60_ABSOLUTE} > ${CMAKE_CURRENT_BINARY_DIR}/${_60_BASE_NAME}.h
            DEPENDS ${_60_ABSOLUTE}
            COMMENT "Running sixtyfps_compiler on ${it}")
        target_sources(${target} PRIVATE ${CMAKE_CURRENT_BINARY_DIR}/${_60_BASE_NAME}.h)
    endforeach()
    # FIXME: DO WE NEED THIS HERE?
    target_include_directories(${target} PRIVATE ${CMAKE_CURRENT_BINARY_DIR})
endfunction()
