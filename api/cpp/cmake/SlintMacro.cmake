# Copyright © SixtyFPS GmbH <info@slint.dev>
# SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

# Set up machinery to handle SLINT_EMBED_RESOURCES target property
set(DEFAULT_SLINT_EMBED_RESOURCES as-absolute-path CACHE STRING
    "The default resource embedding option to pass to the Slint compiler")
set_property(CACHE DEFAULT_SLINT_EMBED_RESOURCES PROPERTY STRINGS
    "as-absolute-path" "embed-files" "embed-for-software-renderer" "embed-for-software-renderer-with-sdf")
## This requires CMake 3.23 and does not work in 3.26 AFAICT.
# define_property(TARGET PROPERTY SLINT_EMBED_RESOURCES
#     INITIALIZE_FROM_VARIABLE DEFAULT_SLINT_EMBED_RESOURCES)

function(SLINT_TARGET_SOURCES target)
    # Parse the NAMESPACE argument
    cmake_parse_arguments(SLINT_TARGET_SOURCES "" "NAMESPACE;COMPILATION_UNITS" "LIBRARY_PATHS" ${ARGN})

    get_target_property(enabled_features Slint::Slint SLINT_ENABLED_FEATURES)
    if (("EXPERIMENTAL" IN_LIST enabled_features) AND ("SYSTEM_TESTING" IN_LIST enabled_features))
        set(SLINT_COMPILER_ENV ${CMAKE_COMMAND} -E env)
        set(SLINT_COMPILER_ENV ${SLINT_COMPILER_ENV} SLINT_EMIT_DEBUG_INFO=1)
    endif()

    if (DEFINED SLINT_TARGET_SOURCES_NAMESPACE)
        # Remove the NAMESPACE argument from the list
        list(FIND ARGN "NAMESPACE" _index)
        list(REMOVE_AT ARGN ${_index})
        list(FIND ARGN "${SLINT_TARGET_SOURCES_NAMESPACE}" _index)
        list(REMOVE_AT ARGN ${_index})
        # If the namespace is not empty, add the --cpp-namespace argument
        set(_SLINT_CPP_NAMESPACE_ARG "--cpp-namespace=${SLINT_TARGET_SOURCES_NAMESPACE}")
    endif()

    if (DEFINED SLINT_TARGET_SOURCES_COMPILATION_UNITS)
        if (NOT SLINT_TARGET_SOURCES_COMPILATION_UNITS MATCHES "^[0-9]+$")
            message(FATAL_ERROR "Expected number, got '${SLINT_TARGET_SOURCES_COMPILATION_UNITS}' for COMPILATION_UNITS argument")
        endif()
        set(compilation_units ${SLINT_TARGET_SOURCES_COMPILATION_UNITS})
    else()
        set(compilation_units 1)
    endif()

    while (SLINT_TARGET_SOURCES_LIBRARY_PATHS)
        list(POP_FRONT SLINT_TARGET_SOURCES_LIBRARY_PATHS name_and_path)
        list(APPEND _SLINT_CPP_LIBRARY_PATHS_ARG "-L")
        list(APPEND _SLINT_CPP_LIBRARY_PATHS_ARG "${name_and_path}")
    endwhile()

    foreach (it IN ITEMS ${SLINT_TARGET_SOURCES_UNPARSED_ARGUMENTS})
        get_filename_component(_SLINT_BASE_NAME ${it} NAME_WE)
        get_filename_component(_SLINT_ABSOLUTE ${it} REALPATH BASE_DIR ${CMAKE_CURRENT_SOURCE_DIR})
        get_property(_SLINT_STYLE GLOBAL PROPERTY SLINT_STYLE)

        set(t_prop "$<TARGET_GENEX_EVAL:${target},$<TARGET_PROPERTY:${target},SLINT_EMBED_RESOURCES>>")
        set(global_fallback "${DEFAULT_SLINT_EMBED_RESOURCES}")
        set(embed "$<IF:$<STREQUAL:${t_prop},>,${global_fallback},${t_prop}>")

        set(scale_factor_target_prop "$<TARGET_GENEX_EVAL:${target},$<TARGET_PROPERTY:${target},SLINT_SCALE_FACTOR>>")
        set(scale_factor_arg "$<IF:$<STREQUAL:${scale_factor_target_prop},>,,--scale-factor=${scale_factor_target_prop}>")

        set(bundle_translations_prop "$<TARGET_GENEX_EVAL:${target},$<TARGET_PROPERTY:${target},SLINT_BUNDLE_TRANSLATIONS>>")
        set(bundle_translations_arg "$<IF:$<STREQUAL:${bundle_translations_prop},>,,--bundle-translations=${bundle_translations_prop}>")

        set(translation_domain_prop "$<TARGET_GENEX_EVAL:${target},$<TARGET_PROPERTY:${target},SLINT_TRANSLATION_DOMAIN>>")
        set(translation_domain_arg "$<IF:$<STREQUAL:${translation_domain_prop},>,${target},${translation_domain_prop}>")

        if (compilation_units GREATER 0)
            foreach(cpp_num RANGE 1 ${compilation_units})
                list(APPEND cpp_files "${CMAKE_CURRENT_BINARY_DIR}/slint_generated_${_SLINT_BASE_NAME}_${cpp_num}.cpp")
            endforeach()
            list(TRANSFORM cpp_files PREPEND "--cpp-file=" OUTPUT_VARIABLE cpp_files_arg)
        endif()

        add_custom_command(
            OUTPUT ${CMAKE_CURRENT_BINARY_DIR}/${_SLINT_BASE_NAME}.h ${cpp_files}
            COMMAND ${SLINT_COMPILER_ENV} $<TARGET_FILE:Slint::slint-compiler> ${_SLINT_ABSOLUTE}
                -o ${CMAKE_CURRENT_BINARY_DIR}/${_SLINT_BASE_NAME}.h
                --depfile ${CMAKE_CURRENT_BINARY_DIR}/${_SLINT_BASE_NAME}.d
                --style ${_SLINT_STYLE}
                --embed-resources=${embed}
                --translation-domain=${translation_domain_arg}
                ${_SLINT_CPP_NAMESPACE_ARG}
                ${_SLINT_CPP_LIBRARY_PATHS_ARG}
                ${scale_factor_arg}
                ${bundle_translations_arg}
                ${cpp_files_arg}
            DEPENDS Slint::slint-compiler ${_SLINT_ABSOLUTE}
            COMMENT "Generating ${_SLINT_BASE_NAME}.h"
            DEPFILE ${CMAKE_CURRENT_BINARY_DIR}/${_SLINT_BASE_NAME}.d
            WORKING_DIRECTORY ${CMAKE_BINARY_DIR}
        )

        target_sources(${target} PRIVATE ${CMAKE_CURRENT_BINARY_DIR}/${_SLINT_BASE_NAME}.h ${cpp_files})
    endforeach()
    target_include_directories(${target} PUBLIC ${CMAKE_CURRENT_BINARY_DIR})
endfunction()
