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
            OUTPUT ${CMAKE_CURRENT_BINARY_DIR}/${_60_BASE_NAME}.h
            COMMAND ${SIXTYFPS_COMPILER} ${_60_ABSOLUTE} > ${CMAKE_CURRENT_BINARY_DIR}/${_60_BASE_NAME}.h
            DEPENDS ${_60_ABSOLUTE} ${SIXTYFPS_COMPILER}
            COMMENT "Running sixtyfps_compiler on ${it}")
        target_sources(${target} PRIVATE ${CMAKE_CURRENT_BINARY_DIR}/${_60_BASE_NAME}.h)

    endforeach()
    # FIXME: DO WE NEED THIS HERE?
    target_include_directories(${target} PRIVATE ${CMAKE_CURRENT_BINARY_DIR})
endfunction()

add_library(sixtyfps SHARED IMPORTED)
set_target_properties(sixtyfps PROPERTIES
  INTERFACE_INCLUDE_DIRECTORIES
    "${CMAKE_CURRENT_SOURCE_DIR}/../../api/sixtyfps-cpp/include;${_SIXTYFPS_TARGET_DIR}/include"
  IMPORTED_LOCATION
    "${_SIXTYFPS_TARGET_DIR}/libgl${CMAKE_STATIC_LIBRARY_SUFFIX}")

#FIXME: remove this once we automate cmake target generation.
target_link_libraries(sixtyfps INTERFACE -lfontconfig -lfreetype -lexpat -lfreetype -ldl -ldl -lutil -ldl -lutil -ldl -lrt -lpthread -lgcc_s -lc -lm -lrt -lpthread -lutil -lutil)

add_custom_target(ensure_sixtyfps_runtime_up_to_date
    COMMAND cargo build --lib
    WORKING_DIRECTORY ${CMAKE_CURRENT_SOURCE_DIR}
)
add_dependencies(sixtyfps ensure_sixtyfps_runtime_up_to_date)

### END This should be moce in some file in api/sixtyfps-cpp/cmake

add_executable(hello main.cpp)
target_link_libraries(hello sixtyfps)
sixtyfps_target_60_sources(hello hello.60)
