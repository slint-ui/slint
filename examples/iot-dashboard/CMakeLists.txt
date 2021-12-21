# Copyright © SixtyFPS GmbH <info@sixtyfps.io>
# SPDX-License-Identifier: (GPL-3.0-only OR LicenseRef-SixtyFPS-commercial)

cmake_minimum_required(VERSION 3.14)
project(sixtyfps_cpp_iot_dashboard LANGUAGES CXX)

if (NOT TARGET SixtyFPS::SixtyFPS)
    find_package(SixtyFPS REQUIRED)
endif()

FetchContent_Declare(fmt
  GIT_REPOSITORY https://github.com/fmtlib/fmt.git
  GIT_TAG 7.1.3
)
FetchContent_MakeAvailable(fmt)

add_executable(iot_dashboard main.cpp dashboard.cpp)
target_compile_definitions(iot_dashboard PRIVATE
   SOURCE_DIR=\"${CMAKE_CURRENT_SOURCE_DIR}\"
)
target_link_libraries(iot_dashboard PRIVATE SixtyFPS::SixtyFPS fmt::fmt )
