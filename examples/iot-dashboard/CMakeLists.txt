# Copyright © SixtyFPS GmbH <info@slint.dev>
# SPDX-License-Identifier: MIT

cmake_minimum_required(VERSION 3.21)

# for the use of std::format
set(CMAKE_OSX_DEPLOYMENT_TARGET 13)

project(slint_cpp_iot_dashboard LANGUAGES CXX)
include(FetchContent)

if (NOT TARGET Slint::Slint)
    find_package(Slint REQUIRED)
endif()

add_executable(iot_dashboard main.cpp dashboard.cpp)
target_compile_definitions(iot_dashboard PRIVATE
   SOURCE_DIR=\"${CMAKE_CURRENT_SOURCE_DIR}\"
)
target_link_libraries(iot_dashboard PRIVATE Slint::Slint)
