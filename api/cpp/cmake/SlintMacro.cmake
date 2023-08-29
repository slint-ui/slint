# Copyright © SixtyFPS GmbH <info@slint.dev>
# SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-1.1 OR LicenseRef-Slint-commercial

# Set up machinery to handle SLINT_EMBED_RESOURCES target property
set(DEFAULT_SLINT_EMBED_RESOURCES as-absolute-path CACHE STRING
    "The default resource embedding option to pass to the Slint compiler")
set_property(CACHE DEFAULT_SLINT_EMBED_RESOURCES PROPERTY STRINGS
    "as-absolute-path" "embed-files" "embed-for-software-renderer")
## This requires CMake 3.23 and does not work in 3.26 AFAICT.
# define_property(TARGET PROPERTY SLINT_EMBED_RESOURCES
#     INITIALIZE_FROM_VARIABLE DEFAULT_SLINT_EMBED_RESOURCES)

function(SLINT_TARGET_SOURCES target)
    foreach (it IN ITEMS ${ARGN})
        get_filename_component(_SLINT_BASE_NAME ${it} NAME_WE)
        get_filename_component(_SLINT_ABSOLUTE ${it} REALPATH BASE_DIR ${CMAKE_CURRENT_SOURCE_DIR})
        get_property(_SLINT_STYLE GLOBAL PROPERTY SLINT_STYLE)

        set(t_prop "$<TARGET_PROPERTY:${target},SLINT_EMBED_RESOURCES>")
        set(global_fallback "${DEFAULT_SLINT_EMBED_RESOURCES}")
        set(embed "$<IF:$<STREQUAL:${t_prop},>,${global_fallback},${t_prop}>")

        add_custom_command(
            OUTPUT ${CMAKE_CURRENT_BINARY_DIR}/${_SLINT_BASE_NAME}.h
            COMMAND Slint::slint-compiler ${_SLINT_ABSOLUTE}
                -o ${CMAKE_CURRENT_BINARY_DIR}/${_SLINT_BASE_NAME}.h
                --depfile ${CMAKE_CURRENT_BINARY_DIR}/${_SLINT_BASE_NAME}.d
                --style ${_SLINT_STYLE}
                --embed-resources=${embed}
                --translation-domain="${target}"
            DEPENDS Slint::slint-compiler ${_SLINT_ABSOLUTE}
            COMMENT "Generating ${_SLINT_BASE_NAME}.h"
            DEPFILE ${CMAKE_CURRENT_BINARY_DIR}/${_SLINT_BASE_NAME}.d
            WORKING_DIRECTORY ${CMAKE_BINARY_DIR}
        )

        target_sources(${target} PRIVATE ${CMAKE_CURRENT_BINARY_DIR}/${_SLINT_BASE_NAME}.h)
    endforeach()
    target_include_directories(${target} PRIVATE ${CMAKE_CURRENT_BINARY_DIR})
endfunction()
