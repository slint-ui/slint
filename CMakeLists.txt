cmake_minimum_required(VERSION 3.16)

project(SixtyFPS LANGUAGES CXX)

include(ExternalProject)
# Set default ExternalProject root directory
set_directory_properties(PROPERTIES EP_PREFIX ${CMAKE_BINARY_DIR}/Rust)
ExternalProject_Add(
    SixtyFPS
    DOWNLOAD_COMMAND ""
    CONFIGURE_COMMAND ""
    BUILD_COMMAND cargo xtask cmake
    INSTALL_COMMAND ""
    BINARY_DIR "${CMAKE_SOURCE_DIR}"
    LOG_BUILD ON)

add_subdirectory(examples/cpptest/)
add_subdirectory(examples/printerdemo/cpp/)
