# Copyright © SixtyFPS GmbH <info@slint-ui.com>
# SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-commercial

cmake_minimum_required(VERSION 3.21)
project(Slint HOMEPAGE_URL "https://slint-ui.com/" LANGUAGES CXX VERSION 1.0.0)

include(FeatureSummary)
include(CMakeDependentOption)

include(FetchContent)
FetchContent_Declare(
    Corrosion
    GIT_REPOSITORY https://github.com/corrosion-rs/corrosion.git
    GIT_TAG v0.4-beta1
)
FetchContent_MakeAvailable(Corrosion)

list(PREPEND CMAKE_MODULE_PATH ${Corrosion_SOURCE_DIR}/cmake)
find_package(Rust 1.66 REQUIRED MODULE)

option(SLINT_FEATURE_COMPILER "Enable support for compiling .slint files to C++ ahead of time" ON)

if(SLINT_FEATURE_COMPILER)
    set(slint_compiler_crate "slint-compiler")
endif()

corrosion_import_crate(MANIFEST_PATH "${CMAKE_CURRENT_SOURCE_DIR}/../../Cargo.toml"
    CRATES slint-cpp ${slint_compiler_crate})

# When doing "make install" package builds, set install_name to rpath, so that the installed
# binaries don't have a load-command pointing back to their build directory.
# Don't do this when Slint is used via FetchContent. While we could set CMAKE_BUILD_RPATH to
# include the binary dir and thus our examples would have the correct rpath set, binaries
# outside (i.e. applications using Slint via FetchContent) would not and the BUILD_RPATH
# target property doesn't propagate :(
if (APPLE AND SLINT_IS_TOPLEVEL_BUILD)
    # corrosion could provide the Cargo.toml package version as a CMake target property.
    corrosion_add_target_local_rustflags(slint-cpp -Clink-arg=-Wl,-install_name,@rpath/libslint_cpp.dylib,-current_version,${PROJECT_VERSION_MAJOR}.${PROJECT_VERSION_MINOR},-compatibility_version,${PROJECT_VERSION_MAJOR}.0)
    # Set this one to false again explicitely because Corrosion will starting setting this property to true by default.
    set_target_properties(slint-cpp-shared PROPERTIES IMPORTED_NO_SONAME 0)
    set_target_properties(slint-cpp-shared PROPERTIES IMPORTED_SONAME libslint_cpp.dylib)
endif()

set_property(
    TARGET slint-cpp
    APPEND
    PROPERTY CORROSION_ENVIRONMENT_VARIABLES
    "SLINT_GENERATED_INCLUDE_DIR=${CMAKE_CURRENT_BINARY_DIR}/generated_include/"
)

if(SLINT_FEATURE_COMPILER)
    set_property(
        TARGET slint-compiler
        PROPERTY CORROSION_USE_HOST_BUILD 1
    )
endif()

add_library(Slint INTERFACE)
add_library(Slint::Slint ALIAS Slint)
target_link_libraries(Slint INTERFACE slint-cpp)
target_compile_features(Slint INTERFACE cxx_std_20)
if (MSVC)
    target_compile_options(Slint INTERFACE /bigobj)
endif()

function(define_cargo_feature cargo-feature description default)
    # turn foo-bar into SLINT_FEATURE_FOO_BAR
    string(TOUPPER "${cargo-feature}" cmake_option)
    string(REPLACE "-" "_" cmake_option "${cmake_option}")
    set(cmake_option "SLINT_FEATURE_${cmake_option}")
    option("${cmake_option}" "${description}" ${default})

    if(${cmake_option})
        list(APPEND features ${cargo-feature})
    endif()

    set(features "${features}" PARENT_SCOPE)
    add_feature_info(${cmake_option} ${cmake_option} ${description})
endfunction()

function(define_cargo_dependent_feature cargo-feature description default depends_condition)
    # turn foo-bar into SLINT_FEATURE_FOO_BAR
    string(TOUPPER "${cargo-feature}" cmake_option)
    string(REPLACE "-" "_" cmake_option "${cmake_option}")
    set(cmake_option "SLINT_FEATURE_${cmake_option}")
    cmake_dependent_option("${cmake_option}" "${description}" ${default} ${depends_condition} OFF)

    if(${cmake_option})
        list(APPEND features ${cargo-feature})
    endif()

    set(features "${features}" PARENT_SCOPE)
    add_feature_info(${cmake_option} ${cmake_option} ${description})
endfunction()

# Features that are mapped to features in the Rust crate. These and their
# defaults need to be kept in sync with the Rust bit.
define_cargo_feature(interpreter "Enable support for the Slint interpeter to load .slint files at run-time" ON)

define_cargo_feature(backend-winit "Enable support for the winit crate to interaction with all windowing systems." ON)
define_cargo_feature(backend-winit-x11 "Enable support for the winit create to interact only with the X11 windowing system on Unix. Enable this option and turn off SLINT_FEATURE_BACKEND_WINIT for a smaller build with just X11 support on Unix." OFF)
define_cargo_feature(backend-winit-wayland "Enable support for the winit create to interact only with the wayland windowing system on Unix. Enable this option and turn off SLINT_FEATURE_BACKEND_WINIT for a smaller build with just wayland support." OFF)

define_cargo_feature(renderer-winit-femtovg "Enable support for the OpenGL ES 2.0 based FemtoVG rendering engine." ON)
define_cargo_feature(renderer-winit-skia "Enable support for the Skia based rendering engine." OFF)
define_cargo_feature(renderer-winit-skia-opengl "Enable support for the Skia based rendering engine with its OpenGL backend." OFF)
define_cargo_feature(renderer-winit-skia-vulkan "Enable support for the Skia based rendering engine with its Vulkan backend." OFF)
define_cargo_feature(renderer-winit-software "Enable support for the software renderer with the winit backend" OFF)

define_cargo_feature(backend-qt "Enable Qt based rendering backend" ON)

define_cargo_feature(experimental "Enable experimental features (no compatibility guarantees)" OFF)

# Compat options
option(SLINT_FEATURE_BACKEND_GL_ALL "This feature is an alias for SLINT_FEATURE_BACKEND_WINIT and SLINT_FEATURE_RENDERER_WINIT_FEMTOVG." OFF)
option(SLINT_FEATURE_BACKEND_GL_X11 "This feature is an alias for SLINT_FEATURE_BACKEND_WINIT_X11 and SLINT_FEATURE_RENDERER_WINIT_FEMTOVG." OFF)
option(SLINT_FEATURE_BACKEND_GL_WAYLAND "This feature is an alias for SLINT_FEATURE_BACKEND_WINIT_WAYLAND and SLINT_FEATURE_RENDERER_WINIT_FEMTOVG." OFF)

if(SLINT_FEATURE_BACKEND_GL_ALL)
    set(SLINT_FEATURE_BACKEND_WINIT ON)
    set(SLINT_FEATURE_RENDERER_WINIT_FEMTOVG ON)
endif()

if(SLINT_FEATURE_BACKEND_GL_X11)
    set(SLINT_FEATURE_BACKEND_WINIT_X11 ON)
    set(SLINT_FEATURE_RENDERER_WINIT_FEMTOVG ON)
endif()

if(SLINT_FEATURE_BACKEND_GL_WAYLAND)
    set(SLINT_FEATURE_BACKEND_WINIT_WAYLAND ON)
    set(SLINT_FEATURE_RENDERER_WINIT_FEMTOVG ON)
endif()

set_property(
    TARGET slint-cpp
    PROPERTY CORROSION_FEATURES
    ${features}
)
set_property(
    TARGET slint-cpp
    PROPERTY CORROSION_NO_DEFAULT_FEATURES
    ${features}
)

if(SLINT_FEATURE_BACKEND_QT)
    # For the CMake build don't rely on qmake being in PATH but use CMake to locate Qt. This
    # means usually CMAKE_PREFIX_PATH is set.
    find_package(Qt6 6.0 QUIET COMPONENTS Core Widgets)

    if(NOT TARGET Qt::qmake)
        find_package(Qt5 5.15 QUIET COMPONENTS Core Widgets)
    endif()
endif(SLINT_FEATURE_BACKEND_QT)

if(TARGET Qt::qmake)
    set_property(
        TARGET slint-cpp
        APPEND
        PROPERTY CORROSION_ENVIRONMENT_VARIABLES
        QMAKE=$<TARGET_PROPERTY:Qt::qmake,LOCATION>
    )
    set(SLINT_STYLE_DEFAULT "native")
else()
    set_property(
        TARGET slint-cpp
        APPEND
        PROPERTY CORROSION_ENVIRONMENT_VARIABLES
        SLINT_NO_QT=1
    )
    set(SLINT_STYLE_DEFAULT "fluent")
endif()

set(SLINT_STYLE "" CACHE STRING "The Slint Widget Style")

if(SLINT_STYLE)
    set_property(GLOBAL PROPERTY SLINT_STYLE ${SLINT_STYLE})
else(SLINT_STYLE)
    set_property(GLOBAL PROPERTY SLINT_STYLE ${SLINT_STYLE_DEFAULT})
endif(SLINT_STYLE)

# The Skia cross-build requires a host C compiler (due to some build dependencies of rust-skia),
# so cc.rs will first look for CC_<triplet> and then HOST_CC.
# When cross-compiling, CMake doesn't really know what the host compiler is. Corrosion will set
# HOST_CC to $CC, which is a good bet. Unfortunately in Yocto environments, CC will be set to
# the cross-compiler. The same applies to CFLAGS, which may contain target specific options.
# So the hack to solve this is two-fold:
#  * We look for clang or gcc in PATH - unprefixed those are usually host compilers.
#  * Through corrosion we know the correct host value of CC_<triplet>.
# Finally, we set CC_<host triplet> to clang or gcc and empty CFLAGS_<host triplet>
if((SLINT_FEATURE_RENDERER_WINIT_SKIA OR SLINT_FEATURE_RENDERER_WINIT_SKIA_OPENGL OR SLINT_FEATURE_RENDERER_WINIT_SKIA_VULKAN) AND CMAKE_CROSSCOMPILING AND Rust_CARGO_HOST_TARGET)
    find_program(CLANG clang)
    if(CLANG)
        set(host_cc "${CLANG}")
    else()
        find_program(GCC gcc)
        if (GCC)
            set(host_cc "${GCC}")
        endif()
    endif()
    if(host_cc)
        string(REPLACE "-" "_" cargo_host_target_underscore "${Rust_CARGO_HOST_TARGET}")
        set_property(
            TARGET slint-cpp
            APPEND
            PROPERTY CORROSION_ENVIRONMENT_VARIABLES
            CC_${cargo_host_target_underscore}=${host_cc}
        )
        set_property(
            TARGET slint-cpp
            APPEND
            PROPERTY CORROSION_ENVIRONMENT_VARIABLES
            CFLAGS_${cargo_host_target_underscore}=
        )
    endif()
endif()

if(SLINT_FEATURE_EXPERIMENTAL)
    target_compile_definitions(Slint INTERFACE SLINT_FEATURE_EXPERIMENTAL)
endif()

file(GLOB api_headers RELATIVE "${CMAKE_CURRENT_SOURCE_DIR}/include/"
    "${CMAKE_CURRENT_SOURCE_DIR}/include/*.h")

foreach(header IN LISTS api_headers)
    set_property(TARGET Slint APPEND PROPERTY PUBLIC_HEADER include/${header})
endforeach()

set(generated_headers
    ${CMAKE_CURRENT_BINARY_DIR}/generated_include/slint_internal.h
    ${CMAKE_CURRENT_BINARY_DIR}/generated_include/slint_enums_internal.h
    ${CMAKE_CURRENT_BINARY_DIR}/generated_include/slint_string_internal.h
    ${CMAKE_CURRENT_BINARY_DIR}/generated_include/slint_brush_internal.h
    ${CMAKE_CURRENT_BINARY_DIR}/generated_include/slint_sharedvector_internal.h
    ${CMAKE_CURRENT_BINARY_DIR}/generated_include/slint_properties_internal.h
    ${CMAKE_CURRENT_BINARY_DIR}/generated_include/slint_image_internal.h
    ${CMAKE_CURRENT_BINARY_DIR}/generated_include/slint_color_internal.h
    ${CMAKE_CURRENT_BINARY_DIR}/generated_include/slint_pathdata_internal.h
    ${CMAKE_CURRENT_BINARY_DIR}/generated_include/slint_qt_internal.h
    ${CMAKE_CURRENT_BINARY_DIR}/generated_include/slint_backend_internal.h
    ${CMAKE_CURRENT_BINARY_DIR}/generated_include/slint_generated_public.h
    ${CMAKE_CURRENT_BINARY_DIR}/generated_include/slint_interpreter_internal.h
    ${CMAKE_CURRENT_BINARY_DIR}/generated_include/slint_interpreter_generated_public.h
)

foreach(header IN LISTS generated_headers)
    set_property(TARGET Slint APPEND PROPERTY PUBLIC_HEADER ${header})
endforeach()

target_include_directories(Slint INTERFACE
    $<BUILD_INTERFACE:${CMAKE_CURRENT_BINARY_DIR}/generated_include>
    $<BUILD_INTERFACE:${CMAKE_CURRENT_SOURCE_DIR}/include>
    $<INSTALL_INTERFACE:include/slint>
)

if(SLINT_FEATURE_COMPILER)
    add_executable(Slint::slint-compiler ALIAS slint-compiler)
    include(${CMAKE_CURRENT_LIST_DIR}/cmake/SlintMacro.cmake)
endif()

export(TARGETS Slint slint-cpp
    NAMESPACE Slint:: FILE "${CMAKE_BINARY_DIR}/lib/cmake/Slint/SlintTargets.cmake")
install(EXPORT SlintTargets NAMESPACE Slint:: DESTINATION lib/cmake/Slint)
install(TARGETS Slint slint-cpp
    EXPORT SlintTargets LIBRARY DESTINATION lib PUBLIC_HEADER DESTINATION include/slint)

include(CMakePackageConfigHelpers)
include(GNUInstallDirs)

install(FILES $<TARGET_FILE:slint-cpp-shared> TYPE LIB)

if(WIN32)
    install(FILES $<TARGET_LINKER_FILE:slint-cpp-shared> TYPE LIB)
endif()

if(SLINT_FEATURE_COMPILER)
    install(PROGRAMS $<TARGET_FILE:slint-compiler> TYPE BIN)
endif()

set(SLINT_LIB_PROPERTIES "IMPORTED_NO_SONAME;1")

# Corrosion sets the `IMPORTED` locations late to allow us to set OUTPUT_DIRECTORY
# target properties. This function must be deferred until after Corrosion set the
# locations. Since we are writing to a config file Generator expressions are not
# an option.
function(_slint_write_configure_file)
    foreach(prop
        IMPORTED_LOCATION IMPORTED_LOCATION_DEBUG IMPORTED_LOCATION_RELEASE
        IMPORTED_LOCATION_RELWITHDEBINFO IMPORTED_LOCATION_MINSIZEREL
        IMPORTED_IMPLIB IMPORTED_IMPLIB_DEBUG IMPORTED_IMPLIB_RELEASE
        IMPORTED_IMPLIB_RELWITHDEBINFO IMPORTED_IMPLIB_MINSIZEREL)
        get_target_property(value slint-cpp-shared ${prop})

        if(value)
            get_filename_component(value ${value} NAME)
            list(APPEND SLINT_LIB_PROPERTIES ${prop} "\${_IMPORT_PREFIX}/${CMAKE_INSTALL_LIBDIR}/${value}")
        endif()
    endforeach()

    get_property(_SLINT_STYLE GLOBAL PROPERTY SLINT_STYLE)
    configure_package_config_file("cmake/SlintConfig.cmake.in" "${CMAKE_CURRENT_BINARY_DIR}/lib/cmake/Slint/SlintConfig.cmake" INSTALL_DESTINATION lib/cmake/Slint)
endfunction()

cmake_language(DEFER CALL _slint_write_configure_file)

write_basic_package_version_file(
    ${CMAKE_CURRENT_BINARY_DIR}/lib/cmake/Slint/SlintConfigVersion.cmake
    VERSION 1.0.3
    COMPATIBILITY SameMinorVersion
)

install(FILES
    "${CMAKE_CURRENT_BINARY_DIR}/lib/cmake/Slint/SlintConfig.cmake"
    "${CMAKE_CURRENT_BINARY_DIR}/lib/cmake/Slint/SlintConfigVersion.cmake"
    DESTINATION lib/cmake/Slint
)

if(SLINT_FEATURE_COMPILER)
    install(FILES
        "${CMAKE_CURRENT_LIST_DIR}/cmake/SlintMacro.cmake"
        DESTINATION lib/cmake/Slint
    )
endif()

option(SLINT_PACKAGE_BUNDLE_QT "Internal setting to install Qt binary in the packages" OFF)

if(SLINT_PACKAGE_BUNDLE_QT)
    if(WIN32)
        find_package(Qt6 6.0 COMPONENTS Core Gui Widgets Svg)
        install(
            FILES
            $<TARGET_FILE:Qt6::Core>
            $<TARGET_FILE:Qt6::Gui>
            $<TARGET_FILE:Qt6::Widgets>
            $<TARGET_FILE:Qt6::Svg>
            TYPE LIB)

        install(
            FILES ${Qt6_DIR}/../../../plugins/platforms/qwindows.dll
            DESTINATION plugins/platforms)
        install(
            FILES ${Qt6_DIR}/../../../plugins/styles/qwindowsvistastyle.dll
            DESTINATION plugins/styles)
        install(
            FILES ${Qt6_DIR}/../../../plugins/imageformats/qsvg.dll
            DESTINATION plugins/imageformats)

        install(DIRECTORY ${CMAKE_CURRENT_SOURCE_DIR}/../../licenses/ DESTINATION LICENSES)

        set(CMAKE_INSTALL_SYSTEM_RUNTIME_LIBS_SKIP TRUE)
        include(InstallRequiredSystemLibraries)
        install(FILES ${CMAKE_INSTALL_SYSTEM_RUNTIME_LIBS} TYPE LIB)
    endif()
endif(SLINT_PACKAGE_BUNDLE_QT)

set(CPACK_PACKAGE_NAME "Slint-cpp")
set(CPACK_PACKAGE_VENDOR "Slint")
set(CPACK_VERBATIM_VARIABLES true)
set(CPACK_PACKAGE_VERSION_MAJOR 1)
set(CPACK_PACKAGE_VERSION_MINOR 0)
set(CPACK_PACKAGE_VERSION_PATCH 3)
set(CPACK_PACKAGE_HOMEPAGE_URL "https://slint-ui.com")
set(CPACK_RESOURCE_FILE_LICENSE "${CMAKE_CURRENT_LIST_DIR}/../../LICENSE.md")
set(CPACK_RESOURCE_FILE_README "${CMAKE_CURRENT_LIST_DIR}/README.md")
set(CPACK_STRIP_FILES ON)
set(CPACK_NSIS_DEFINES "ManifestDPIAware true")

if(NOT WIN32)
    set(CPACK_GENERATOR "TGZ")
    set(CPACK_SYSTEM_NAME "${CMAKE_SYSTEM_NAME}-${CMAKE_SYSTEM_PROCESSOR}")
endif(NOT WIN32)

include(CPack)

if(SLINT_BUILD_TESTING)
    add_subdirectory(tests)
endif()
