# LICENSE BEGIN
# This file is part of the SixtyFPS Project -- https://sixtyfps.io
# Copyright (c) 2020 Olivier Goffart <olivier.goffart@sixtyfps.io>
# Copyright (c) 2020 Simon Hausmann <simon.hausmann@sixtyfps.io>
#
# SPDX-License-Identifier: GPL-3.0-only
# This file is also available under commercial licensing terms.
# Please contact info@sixtyfps.io for more information.
# LICENSE END
cmake_minimum_required(VERSION 3.14)
project(sixtyfps_cpptest LANGUAGES CXX)

set(CMAKE_CXX_STANDARD 17)

### BEGIN This should be moved in some file in api/sixtyfps-cpp/cmake


#FIXME: i guess this file need to be generated so it knows where to look
find_program(SIXTYFPS_COMPILER sixtyfps_compiler HINTS
    ${CMAKE_CURRENT_SOURCE_DIR}/../../../target/release
    ${CMAKE_CURRENT_SOURCE_DIR}/../../../target/debug )

# FIXME that's not where all the things are
get_filename_component(_SIXTYFPS_TARGET_DIR ${SIXTYFPS_COMPILER} DIRECTORY)

function(SIXTYFPS_TARGET_60_SOURCES target)
    foreach (it IN ITEMS ${ARGN})
        get_filename_component(_60_BASE_NAME ${it} NAME_WE)
        get_filename_component(_60_ABSOLUTE ${it} REALPATH BASE_DIR ${CMAKE_CURRENT_SOURCE_DIR})

        add_custom_command(
            OUTPUT ${CMAKE_CURRENT_BINARY_DIR}/${_60_BASE_NAME}.h
            COMMAND ${SIXTYFPS_COMPILER} ${_60_ABSOLUTE} > ${CMAKE_CURRENT_BINARY_DIR}/${_60_BASE_NAME}.h
            DEPENDS ${_60_ABSOLUTE} ${SIXTYFPS_COMPILER}
            COMMENT "Running sixtyfps_compiler on ${it}")
        target_sources(${target} PRIVATE ${CMAKE_CURRENT_BINARY_DIR}/${_60_BASE_NAME}.h)

    endforeach()
    # FIXME: DO WE NEED THIS HERE?
    target_include_directories(${target} PRIVATE ${CMAKE_CURRENT_BINARY_DIR})
endfunction()

find_package(SixtyFPS REQUIRED HINTS ${_SIXTYFPS_TARGET_DIR})

### END This should be moved in some file in api/sixtyfps-cpp/cmake

add_executable(todo main.cpp)
target_link_libraries(todo SixtyFPS::SixtyFPS)
sixtyfps_target_60_sources(todo ../ui/todo.60)
