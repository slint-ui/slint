# Copyright © SixtyFPS GmbH <info@sixtyfps.io>
# SPDX-License-Identifier: (GPL-3.0-only OR LicenseRef-SixtyFPS-commercial)

cmake_minimum_required(VERSION 3.14)
project(sixtyfps_cpp_iot_dashboard LANGUAGES CXX)

if (NOT "cxx_std_20" IN_LIST CMAKE_CXX_COMPILE_FEATURES)
    message(STATUS "The C++ iot dashboard demo requires C++ >= 20. The compiler you're using does not support that, so the build of this example is skipped.")
    return()
endif()

set(CMAKE_CXX_STANDARD 20)
set(CMAKE_CXX_STANDARD_REQUIRED ON)

if (NOT TARGET SixtyFPS::SixtyFPS)
    find_package(SixtyFPS REQUIRED)
endif()

add_executable(iot_dashboard main.cpp dashboard.cpp)
target_compile_definitions(iot_dashboard PRIVATE
   SOURCE_DIR=\"${CMAKE_CURRENT_SOURCE_DIR}\"
)
target_link_libraries(iot_dashboard PRIVATE SixtyFPS::SixtyFPS)