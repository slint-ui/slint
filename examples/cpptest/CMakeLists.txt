cmake_minimum_required(VERSION 3.14)
project(sixtyfps_cpptest LANGUAGES CXX)

set(CMAKE_CXX_STANDARD 17)

### BEGIN This should be moce in some file in api/sixtyfps-cpp/cmake


#FIXME: i guess this file need to be generated so it knows where to look
find_program(SIXTYFPS_COMPILER sixtyfps_compiler HINTS
    ${CMAKE_CURRENT_SOURCE_DIR}/../../target/release
    ${CMAKE_CURRENT_SOURCE_DIR}/../../target/debug )

# FIXME that's not where all the things are
get_filename_component(_SIXTYFPS_TARGET_DIR ${SIXTYFPS_COMPILER} DIRECTORY)


function(SIXTYFPS_TARGET_60_SOURCES target)
    foreach (it IN ITEMS ${ARGN})
        get_filename_component(_60_BASE_NAME ${it} NAME_WE)
        get_filename_component(_60_ABSOLUTE ${it} REALPATH BASE_DIR ${CMAKE_CURRENT_SOURCE_DIR})

        add_custom_command(
            OUTPUT ${CMAKE_CURRENT_BINARY_DIR}/${_60_BASE_NAME}.cpp
            COMMAND ${SIXTYFPS_COMPILER} ${_60_ABSOLUTE} > ${CMAKE_CURRENT_BINARY_DIR}/${_60_BASE_NAME}.cpp
            DEPENDS ${it} ${SIXTYFPS_COMPILER}
            COMMENT "Running sixtyfps_compiler on ${it}")
        target_sources(${target} PRIVATE ${CMAKE_CURRENT_BINARY_DIR}/${_60_BASE_NAME}.cpp)
    endforeach()
endfunction()

#FIXME: we need sixtyfps_internal.h somewhere else
file(GLOB _SIXTYFPS_INTERNAL_HEADER "${_SIXTYFPS_TARGET_DIR}/build/corelib-*/out")

add_library(sixtyfps SHARED IMPORTED)
set_target_properties(sixtyfps PROPERTIES
  INTERFACE_INCLUDE_DIRECTORIES
    "${CMAKE_CURRENT_SOURCE_DIR}/../../api/sixtyfps-cpp/include;${_SIXTYFPS_INTERNAL_HEADER}"
  IMPORTED_LOCATION
    "${_SIXTYFPS_TARGET_DIR}/libcorelib${CMAKE_SHARED_LIBRARY_SUFFIX}")


### END This should be moce in some file in api/sixtyfps-cpp/cmake

add_executable(hello)
target_link_libraries(hello sixtyfps)
sixtyfps_target_60_sources(hello hello.60)
