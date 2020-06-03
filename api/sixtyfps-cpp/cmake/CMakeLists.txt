cmake_minimum_required(VERSION 3.16)

project(SixtyFPS LANGUAGES CXX)

foreach(lib_path IN LISTS SIXTYFPS_INTERNAL_LIBS)
    get_filename_component(lib_name "${lib_path}" NAME_WE)

    add_library(internal_lib_${lib_name} OBJECT IMPORTED)
    set_property(TARGET internal_lib_${lib_name} PROPERTY IMPORTED_OBJECTS "${lib_path}")

    list(APPEND resolved_internal_libs "$<TARGET_OBJECTS:internal_lib_${lib_name}>")
endforeach()

add_library(SixtyFPS STATIC lib.cpp ${resolved_internal_libs})

# When synthesizing the final .a file, make sure that ar flattens instead of creating a .a that just contains a .a (won't be usable)
if (CMAKE_SYSTEM_NAME STREQUAL "Linux")
    set(CMAKE_CXX_ARCHIVE_CREATE "<CMAKE_AR> crT <TARGET> <LINK_FLAGS> <OBJECTS>")
    set(CMAKE_C_ARCHIVE_CREATE "<CMAKE_AR> cruT <TARGET> <LINK_FLAGS> <OBJECTS>")
    set(CMAKE_CXX_ARCHIVE_APPEND "<CMAKE_AR> ruT <TARGET> <LINK_FLAGS> <OBJECTS>")
    set(CMAKE_C_ARCHIVE_APPEND "<CMAKE_AR> ruT <TARGET> <LINK_FLAGS> <OBJECTS>")
endif()

if (DEFINED SIXTYFPS_EXTERNAL_LIBS)
    target_link_libraries(SixtyFPS PRIVATE ${SIXTYFPS_EXTERNAL_LIBS})
endif()

set(api_headers
    sixtyfps.h
    sixtyfps_properties.h
    sixtyfps_signals.h
    sixtyfps_string.h
    vtable.h
)
foreach(header IN LISTS api_headers)
    set_property(TARGET SixtyFPS APPEND PROPERTY PUBLIC_HEADER ../include/${header})
endforeach()

target_include_directories(SixtyFPS PUBLIC
    $<BUILD_INTERFACE:${CMAKE_BINARY_DIR}/include>
    $<BUILD_INTERFACE:${CMAKE_CURRENT_SOURCE_DIR}/../include>
    $<INSTALL_INTERFACE:include/sixtyfps>
)

export(TARGETS SixtyFPS NAMESPACE SixtyFPS:: FILE "${CMAKE_BINARY_DIR}/cmake/SixtyFPSTargets.cmake")

#install(EXPORT SixtyFPSTargets NAMESPACE SixtyFPS:: DESTINATION lib/cmake/SixtyFPS)
#
#install(TARGETS SixtyFPS
#    EXPORT SixtyFPSTargets
#    LIBRARY DESTINATION lib
#    PUBLIC_HEADER DESTINATION include/sixtyfps
#)
#
#include(CMakePackageConfigHelpers)
#
#configure_package_config_file("SixtyFPSConfig.cmake.in" "${CMAKE_CURRENT_BINARY_DIR}/SixtyFPSConfig.cmake" INSTALL_DESTINATION lib/cmake/SixtyFPS)
#
#write_basic_package_version_file(
#  ${CMAKE_CURRENT_BINARY_DIR}/SixtyFPSConfigVersion.cmake
#  VERSION 1.0.0
#  COMPATIBILITY SameMajorVersion
#)
#
#install(FILES
#    "${CMAKE_CURRENT_BINARY_DIR}/SixtyFPSConfig.cmake"
#    "${CMAKE_CURRENT_BINARY_DIR}/SixtyFPSConfigVersion.cmake"
#    DESTINATION lib/cmake/SixtyFPS
#)