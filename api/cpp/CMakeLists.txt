# Copyright © SixtyFPS GmbH <info@slint.dev>
# SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

cmake_minimum_required(VERSION 3.21)
# Select C++ and C as languages, as Corrosion needs ${CMAKE_C_COMPILER}
# for linking
project(Slint HOMEPAGE_URL "https://slint.dev/" LANGUAGES C CXX VERSION 1.13.0)

include(FeatureSummary)
include(CMakeDependentOption)

find_package(Corrosion QUIET 0.5)
if (NOT Corrosion_FOUND)
    include(FetchContent)
    FetchContent_Declare(
        Corrosion
        GIT_REPOSITORY https://github.com/corrosion-rs/corrosion.git
        GIT_TAG v0.5.2
    )

    FetchContent_MakeAvailable(Corrosion)
endif (NOT Corrosion_FOUND)

list(PREPEND CMAKE_MODULE_PATH ${Corrosion_SOURCE_DIR}/cmake)
find_package(Rust 1.85 REQUIRED MODULE)

option(BUILD_SHARED_LIBS "Build Slint as shared library" ON)
option(SLINT_FEATURE_COMPILER "Enable support for compiling .slint files to C++ ahead of time" ON)
add_feature_info(SLINT_FEATURE_COMPILER SLINT_FEATURE_COMPILER "Enable support for compiling .slint files to C++ ahead of time")
option(SLINT_BUILD_RUNTIME "Actually build the Slint runtime libraries (Disable that to only build the compiler)" ON)
add_feature_info(SLINT_BUILD_RUNTIME SLINT_BUILD_RUNTIME "Actually build the Slint runtime libraries (Disable that to only build the compiler)")
option(SLINT_FEATURE_SDF_FONTS "Enable support for pre-rendering fonts as Signed Distance Fields (for the software renderer)" OFF)
add_feature_info(SLINT_FEATURE_SDF_FONTS SLINT_FEATURE_SDF_FONTS "Enable support for pre-rendering fonts as Signed Distance Fields (for the software renderer)")

set(SLINT_COMPILER "" CACHE STRING "Path to the slint-compiler executable. When unset, it the compiler will be build as part of the build process. When set to 'download', the compiler will be downloaded from GitHub.")
set(SLINT_LIBRARY_CARGO_FLAGS "" CACHE STRING
    "Flags to pass to cargo when building the Slint runtime library")

function(define_cargo_feature cargo_feature description default)
    # turn foo-bar into SLINT_FEATURE_FOO_BAR
    string(TOUPPER "${cargo_feature}" cmake_option)
    string(REPLACE "-" "_" cmake_option "${cmake_option}")
    list(APPEND public_cmake_features ${cmake_option})
    set(cmake_option "SLINT_FEATURE_${cmake_option}")
    option("${cmake_option}" "${description}" ${default})

    if(${cmake_option})
        list(APPEND features ${cargo_feature})
    endif()

    set(features "${features}" PARENT_SCOPE)
    set(public_cmake_features "${public_cmake_features}" PARENT_SCOPE)
    add_feature_info(${cmake_option} ${cmake_option} ${description})
endfunction()

function(define_cargo_dependent_feature cargo_feature description default depends_condition)
    # turn foo-bar into SLINT_FEATURE_FOO_BAR
    string(TOUPPER "${cargo_feature}" cmake_option)
    string(REPLACE "-" "_" cmake_option "${cmake_option}")
    list(APPEND public_cmake_features ${cmake_option})
    set(cmake_option "SLINT_FEATURE_${cmake_option}")
    cmake_dependent_option("${cmake_option}" "${description}" ${default} ${depends_condition} OFF)

    if(${cmake_option})
        list(APPEND features ${cargo_feature})
    endif()

    set(features "${features}" PARENT_SCOPE)
    set(public_cmake_features "${public_cmake_features}" PARENT_SCOPE)
    add_feature_info(${cmake_option} ${cmake_option} ${description})
endfunction()

# Features that are mapped to features in the Rust crate. These and their
# defaults need to be kept in sync with the Rust bit (cpp/Cargo.toml and cbindgen.rs)

define_cargo_feature(freestanding "Enable use of freestanding environment. This is only for bare-metal systems. Most other features are incompatible with this one" OFF)

# Compat options (must be declared after the STD feature, but before the other renderer features)
function(define_renderer_winit_compat_option renderer)
    string(TOUPPER "${renderer}" cmake_option)
    string(REPLACE "-" "_" cmake_option "${cmake_option}")
    cmake_dependent_option("SLINT_FEATURE_RENDERER_WINIT_${cmake_option}" "Compat option equivalent to SLINT_FEATURE_RENDERER_${cmake_option}" OFF "NOT SLINT_FEATURE_FREESTANDING" OFF)
    if(SLINT_FEATURE_RENDERER_WINIT_${cmake_option})
        set("SLINT_FEATURE_RENDERER_${cmake_option}" ON PARENT_SCOPE)
        message("SLINT_FEATURE_RENDERER_WINIT_${cmake_option} is deprecated, use SLINT_FEATURE_RENDERER_${cmake_option} instead")
    endif()
endfunction()
define_renderer_winit_compat_option(femtovg)
define_renderer_winit_compat_option(skia)
define_renderer_winit_compat_option(skia-opengl)
define_renderer_winit_compat_option(skia-vulkan)
define_renderer_winit_compat_option(software)

define_cargo_dependent_feature(interpreter "Enable support for the Slint interpreter to load .slint files at run-time" ON "NOT SLINT_FEATURE_FREESTANDING")
define_cargo_dependent_feature(live-preview "Enable support for the Slint live-preview to re-load changed .slint files at run-time" OFF "SLINT_FEATURE_INTERPRETER")

define_cargo_dependent_feature(backend-winit "Enable support for the winit crate to interaction with all windowing systems." ON "NOT SLINT_FEATURE_FREESTANDING")
define_cargo_dependent_feature(backend-winit-x11 "Enable support for the winit create to interact only with the X11 windowing system on Unix. Enable this option and turn off SLINT_FEATURE_BACKEND_WINIT for a smaller build with just X11 support on Unix." OFF "NOT SLINT_FEATURE_FREESTANDING")
define_cargo_dependent_feature(backend-winit-wayland "Enable support for the winit create to interact only with the wayland windowing system on Unix. Enable this option and turn off SLINT_FEATURE_BACKEND_WINIT for a smaller build with just wayland support." OFF "NOT SLINT_FEATURE_FREESTANDING")

define_cargo_dependent_feature(renderer-femtovg "Enable support for the OpenGL ES 2.0 based FemtoVG rendering engine." ON "NOT SLINT_FEATURE_FREESTANDING")
define_cargo_dependent_feature(renderer-femtovg-wgpu "Enable support for the WGPU based FemtoVG rendering engine." OFF "NOT SLINT_FEATURE_FREESTANDING")
define_cargo_dependent_feature(renderer-skia "Enable support for the Skia based rendering engine." OFF "NOT SLINT_FEATURE_FREESTANDING")
define_cargo_dependent_feature(renderer-skia-opengl "Enable support for the Skia based rendering engine with its OpenGL backend." OFF "NOT SLINT_FEATURE_FREESTANDING")
define_cargo_dependent_feature(renderer-skia-vulkan "Enable support for the Skia based rendering engine with its Vulkan backend." OFF "NOT SLINT_FEATURE_FREESTANDING")
define_cargo_feature(renderer-software "Enable support for the software renderer" OFF)

set(SLINT_BACKEND_QT_DEFAULT OFF)
if(${CMAKE_SYSTEM_NAME} MATCHES "Linux")
  set(SLINT_BACKEND_QT_DEFAULT ON)
endif()
define_cargo_dependent_feature(backend-qt "Enable Qt based rendering backend" ${SLINT_BACKEND_QT_DEFAULT} "NOT SLINT_FEATURE_FREESTANDING")

define_cargo_dependent_feature(backend-linuxkms "Enable support for the backend that renders a single window fullscreen on Linux. Requires libseat. If you don't have libseat, select `backend-linuxkms-noseat` instead. (Experimental)" OFF "NOT SLINT_FEATURE_FREESTANDING")
define_cargo_dependent_feature(backend-linuxkms-noseat "Enable support for the backend that renders a single window fullscreen on Linux (Experimental)" OFF "NOT SLINT_FEATURE_FREESTANDING")

define_cargo_dependent_feature(gettext "Enable support of translations using gettext" OFF "NOT SLINT_FEATURE_FREESTANDING")
define_cargo_dependent_feature(accessibility "Enable integration with operating system provided accessibility APIs" ON "NOT SLINT_FEATURE_FREESTANDING")
define_cargo_dependent_feature(testing "Enable support for testing API (experimental)" ON "NOT SLINT_FEATURE_FREESTANDING")
define_cargo_feature(experimental "Enable experimental features. (No backward compatibility guarantees)" OFF)
define_cargo_dependent_feature(system-testing "Enable system testing support (experimental)" OFF "SLINT_FEATURE_EXPERIMENTAL AND NOT SLINT_FEATURE_FREESTANDING")

if (SLINT_BUILD_RUNTIME)
    if(SLINT_FEATURE_COMPILER AND NOT SLINT_COMPILER)
        set(slint_compiler_crate "slint-compiler")
    endif()

    if(BUILD_SHARED_LIBS)
        set(rustc_lib_type "cdylib")
        set(slint_cpp_impl "slint_cpp-shared")
        set(cmake_lib_type "SHARED")
    else()
        set(rustc_lib_type "staticlib")
        set(slint_cpp_impl "slint_cpp-static")
        set(cmake_lib_type "STATIC")
    endif()

    corrosion_import_crate(MANIFEST_PATH "${CMAKE_CURRENT_SOURCE_DIR}/../../Cargo.toml"
        CRATES slint-cpp ${slint_compiler_crate} CRATE_TYPES bin ${rustc_lib_type})
elseif(SLINT_FEATURE_COMPILER AND NOT SLINT_COMPILER)
    corrosion_import_crate(MANIFEST_PATH "${CMAKE_CURRENT_SOURCE_DIR}/../../Cargo.toml"
        CRATES slint-compiler CRATE_TYPES bin)
endif()

if(SLINT_FEATURE_COMPILER AND NOT SLINT_COMPILER)
    corrosion_set_hostbuild(slint-compiler)
    set_property(
        TARGET slint-compiler
        PROPERTY CORROSION_NO_DEFAULT_FEATURES
        ON
    )
    list(APPEND slint_compiler_features "software-renderer")
    if (SLINT_FEATURE_SDF_FONTS)
        list(APPEND slint_compiler_features "sdf-fonts")
    endif()
    # Enable jemalloc, except when cross-compiling. (#7463)
    if(NOT CMAKE_CROSSCOMPILING)
        list(APPEND slint_compiler_features "jemalloc")
    endif()
    set_property(
        TARGET slint-compiler
        PROPERTY CORROSION_FEATURES
        ${slint_compiler_features}
    )
endif()

if (SLINT_BUILD_RUNTIME)

    # When doing "make install" package builds, set install_name to rpath, so that the installed
    # binaries don't have a load-command pointing back to their build directory.
    # Don't do this when Slint is used via FetchContent. While we could set CMAKE_BUILD_RPATH to
    # include the binary dir and thus our examples would have the correct rpath set, binaries
    # outside (i.e. applications using Slint via FetchContent) would not and the BUILD_RPATH
    # target property doesn't propagate :(
    if (APPLE AND SLINT_IS_TOPLEVEL_BUILD AND BUILD_SHARED_LIBS)
        # corrosion could provide the Cargo.toml package version as a CMake target property.
        corrosion_add_target_local_rustflags(slint_cpp -Clink-arg=-Wl,-install_name,@rpath/libslint_cpp.dylib,-current_version,${PROJECT_VERSION_MAJOR}.${PROJECT_VERSION_MINOR},-compatibility_version,${PROJECT_VERSION_MAJOR}.0)
        # Set this one to false again explicitly because Corrosion will starting setting this property to true by default.
        set_target_properties(slint_cpp-shared PROPERTIES IMPORTED_NO_SONAME 0)
        set_target_properties(slint_cpp-shared PROPERTIES IMPORTED_SONAME libslint_cpp.dylib)
    endif()

    add_library(Slint INTERFACE)
    add_library(Slint::Slint ALIAS Slint)
    target_link_libraries(Slint INTERFACE slint_cpp)
    target_compile_features(Slint INTERFACE cxx_std_20)
    if (WIN32)
        target_compile_options(Slint INTERFACE
            $<$<CXX_COMPILER_ID:MSVC>:/bigobj>
            $<$<CXX_COMPILER_ID:Clang>:-Wa,-mbig-obj>
            $<$<CXX_COMPILER_ID:GNU>:-Wa,-mbig-obj>
        )
    endif()

    foreach(feature ${public_cmake_features})
        set(cmake_option "SLINT_FEATURE_${feature}")
        if(${cmake_option})
            list(APPEND enabled_features ${feature})
        else()
            list(APPEND disabled_features ${feature})
        endif()
    endforeach()
    set_property(TARGET Slint APPEND PROPERTY SLINT_ENABLED_FEATURES "${enabled_features}")
    set_property(TARGET Slint APPEND PROPERTY SLINT_DISABLED_FEATURES "${disabled_features}")
    set_property(TARGET Slint APPEND PROPERTY EXPORT_PROPERTIES "SLINT_ENABLED_FEATURES")
    set_property(TARGET Slint APPEND PROPERTY EXPORT_PROPERTIES "SLINT_DISABLED_FEATURES")

    if (SLINT_FEATURE_FREESTANDING)
        if (ESP_PLATFORM AND NOT IDF_TARGET STREQUAL "esp32p4")
            list(APPEND features "esp-backtrace/${IDF_TARGET}")
            list(APPEND features "esp-println/${IDF_TARGET}")
        endif()
    else (SLINT_FEATURE_FREESTANDING)
        list(APPEND features std)
    endif()


    set_property(
        TARGET slint_cpp
        APPEND
        PROPERTY CORROSION_ENVIRONMENT_VARIABLES
        "SLINT_GENERATED_INCLUDE_DIR=${CMAKE_CURRENT_BINARY_DIR}/generated_include/"
    )

    set_property(
        TARGET slint_cpp
        PROPERTY CORROSION_FEATURES
        ${features}
    )
    set_property(
        TARGET slint_cpp
        PROPERTY CORROSION_NO_DEFAULT_FEATURES
        ON
    )

    if(SLINT_LIBRARY_CARGO_FLAGS)
        corrosion_set_cargo_flags(slint_cpp ${SLINT_LIBRARY_CARGO_FLAGS})
    endif()

    if(SLINT_FEATURE_BACKEND_QT)
        # For the CMake build don't rely on qmake being in PATH but use CMake to locate Qt. This
        # means usually CMAKE_PREFIX_PATH is set.
        find_package(Qt6 6.2 QUIET COMPONENTS Core Widgets)

        if(NOT TARGET Qt::qmake)
            find_package(Qt5 5.15 QUIET COMPONENTS Core Widgets)
        endif()
    endif(SLINT_FEATURE_BACKEND_QT)

    if(SLINT_FEATURE_BACKEND_QT AND TARGET Qt::qmake)
        set_property(
            TARGET slint_cpp
            APPEND
            PROPERTY CORROSION_ENVIRONMENT_VARIABLES
            QMAKE=$<TARGET_PROPERTY:Qt::qmake,LOCATION>
        )
    else()
        set_property(
            TARGET slint_cpp
            APPEND
            PROPERTY CORROSION_ENVIRONMENT_VARIABLES
            SLINT_NO_QT=1
        )
    endif()

    if(SLINT_FEATURE_FREESTANDING)
        set(SLINT_STYLE_DEFAULT "fluent")
    elseif(ANDROID)
        set(SLINT_STYLE_DEFAULT "material")
    elseif(WIN32)
        set(SLINT_STYLE_DEFAULT "fluent")
    elseif(APPLE)
        set(SLINT_STYLE_DEFAULT "cupertino")
    elseif (SLINT_FEATURE_BACKEND_QT AND TARGET Qt::qmake)
        set(SLINT_STYLE_DEFAULT "qt")
    else ()
        set(SLINT_STYLE_DEFAULT "fluent")
    endif()

    set(SLINT_STYLE "" CACHE STRING "The Slint Widget Style")

    if(SLINT_STYLE AND NOT SLINT_STYLE STREQUAL "native")
        set_property(GLOBAL PROPERTY SLINT_STYLE ${SLINT_STYLE})
    else()
        set_property(GLOBAL PROPERTY SLINT_STYLE ${SLINT_STYLE_DEFAULT})
    endif()

    # Build environments such as buildroot set PKG_CONFIG_SYSROOT_DIR in their toolchain file
    # and thus it's only available at cmake configure time. Our build dependencies that use pkg-config
    # however invoke pkg-config in build.rs, which means we need to defer/propagate the environment variable
    # to build time.
    if(DEFINED ENV{PKG_CONFIG_SYSROOT_DIR})
        set_property(
            TARGET slint_cpp
            APPEND
            PROPERTY CORROSION_ENVIRONMENT_VARIABLES
            "PKG_CONFIG_SYSROOT_DIR=$ENV{PKG_CONFIG_SYSROOT_DIR}"
        )
    endif()

    if(SLINT_FEATURE_RENDERER_SKIA OR SLINT_FEATURE_RENDERER_SKIA_OPENGL OR SLINT_FEATURE_RENDERER_SKIA_VULKAN)
        find_program(CLANGCC clang)
        find_program(CLANGCXX clang++)
        if(CLANGCC AND NOT DEFINED ENV{CLANGCC})
            set_property(
                TARGET slint_cpp
                APPEND
                PROPERTY CORROSION_ENVIRONMENT_VARIABLES
                CLANGCC=${CLANGCC}
            )
        endif()
        if(CLANGCXX AND NOT DEFINED ENV{CLANGCXX})
            set_property(
                TARGET slint_cpp
                APPEND
                PROPERTY CORROSION_ENVIRONMENT_VARIABLES
                CLANGCXX=${CLANGCXX}
            )
        endif()

        # The Skia cross-build requires a host C compiler (due to some build dependencies of rust-skia),
        # so cc.rs will first look for CC_<triplet> and then HOST_CC.
        # When cross-compiling, CMake doesn't really know what the host compiler is. Corrosion will set
        # HOST_CC to $CC, which is a good bet. Unfortunately in Yocto environments, CC will be set to
        # the cross-compiler. The same applies to CFLAGS, which may contain target specific options.
        # So the hack to solve this is two-fold:
        #  * We look for clang or gcc in PATH - unprefixed those are usually host compilers.
        #  * Through corrosion we know the correct host value of CC_<triplet>.
        # Finally, we set CC_<host triplet> to clang or gcc and empty CFLAGS_<host triplet>
        if(CMAKE_CROSSCOMPILING AND Rust_CARGO_HOST_TARGET)
            if(CLANGCC)
                set(host_cc "${CLANGCC}")
            else()
                find_program(GCC gcc)
                if (GCC)
                    set(host_cc "${GCC}")
                endif()
            endif()
            if(host_cc)
                string(REPLACE "-" "_" cargo_host_target_underscore "${Rust_CARGO_HOST_TARGET}")
                set_property(
                    TARGET slint_cpp
                    APPEND
                    PROPERTY CORROSION_ENVIRONMENT_VARIABLES
                    CC_${cargo_host_target_underscore}=${host_cc}
                )
                set_property(
                    TARGET slint_cpp
                    APPEND
                    PROPERTY CORROSION_ENVIRONMENT_VARIABLES
                    CFLAGS_${cargo_host_target_underscore}=
                )
            endif()
        endif()
    endif()

    file(GLOB api_headers RELATIVE "${CMAKE_CURRENT_SOURCE_DIR}/include/"
        "${CMAKE_CURRENT_SOURCE_DIR}/include/*.h")

    foreach(header IN LISTS api_headers)
        set_property(TARGET Slint APPEND PROPERTY PUBLIC_HEADER include/${header})
    endforeach()

    set(generated_headers
        ${CMAKE_CURRENT_BINARY_DIR}/generated_include/slint_brush_internal.h
        ${CMAKE_CURRENT_BINARY_DIR}/generated_include/slint_builtin_structs.h
        ${CMAKE_CURRENT_BINARY_DIR}/generated_include/slint_builtin_structs_internal.h
        ${CMAKE_CURRENT_BINARY_DIR}/generated_include/slint_color_internal.h
        ${CMAKE_CURRENT_BINARY_DIR}/generated_include/slint_enums.h
        ${CMAKE_CURRENT_BINARY_DIR}/generated_include/slint_enums_internal.h
        ${CMAKE_CURRENT_BINARY_DIR}/generated_include/slint_generated_public.h
        ${CMAKE_CURRENT_BINARY_DIR}/generated_include/slint_image_internal.h
        ${CMAKE_CURRENT_BINARY_DIR}/generated_include/slint_internal.h
        ${CMAKE_CURRENT_BINARY_DIR}/generated_include/slint_pathdata_internal.h
        ${CMAKE_CURRENT_BINARY_DIR}/generated_include/slint_platform_internal.h
        ${CMAKE_CURRENT_BINARY_DIR}/generated_include/slint_properties_internal.h
        ${CMAKE_CURRENT_BINARY_DIR}/generated_include/slint_qt_internal.h
        ${CMAKE_CURRENT_BINARY_DIR}/generated_include/slint_sharedvector_internal.h
        ${CMAKE_CURRENT_BINARY_DIR}/generated_include/slint_string_internal.h
        ${CMAKE_CURRENT_BINARY_DIR}/generated_include/slint_timer_internal.h
        ${CMAKE_CURRENT_BINARY_DIR}/generated_include/slint_events_internal.h
    )

    if(SLINT_FEATURE_INTERPRETER)
        list(APPEND generated_headers
            ${CMAKE_CURRENT_BINARY_DIR}/generated_include/slint_interpreter_internal.h
            ${CMAKE_CURRENT_BINARY_DIR}/generated_include/slint_interpreter_generated_public.h
        )
    endif()


    foreach(header IN LISTS generated_headers)
        set_property(TARGET Slint APPEND PROPERTY PUBLIC_HEADER ${header})
    endforeach()

    target_include_directories(Slint INTERFACE
        $<BUILD_INTERFACE:${CMAKE_CURRENT_BINARY_DIR}/generated_include>
        $<BUILD_INTERFACE:${CMAKE_CURRENT_SOURCE_DIR}/include>
        $<INSTALL_INTERFACE:include/slint>
    )


    if(SLINT_FEATURE_COMPILER AND NOT SLINT_COMPILER)
        add_executable(Slint::slint-compiler ALIAS slint-compiler)
        include(${CMAKE_CURRENT_LIST_DIR}/cmake/SlintMacro.cmake)
    endif()

    export(TARGETS Slint slint_cpp
        NAMESPACE Slint:: FILE "${CMAKE_BINARY_DIR}/lib/cmake/Slint/SlintTargets.cmake")
    install(EXPORT SlintTargets NAMESPACE Slint:: DESTINATION lib/cmake/Slint)
    install(TARGETS Slint slint_cpp
        EXPORT SlintTargets LIBRARY DESTINATION lib PUBLIC_HEADER DESTINATION include/slint)

    install(FILES $<TARGET_FILE:${slint_cpp_impl}> TYPE LIB)

    if(WIN32)
        install(FILES $<TARGET_LINKER_FILE:${slint_cpp_impl}> TYPE LIB)
    endif()

endif(SLINT_BUILD_RUNTIME)

include(CMakePackageConfigHelpers)
include(GNUInstallDirs)


if(SLINT_FEATURE_COMPILER AND NOT SLINT_COMPILER)
    install(PROGRAMS $<TARGET_FILE:slint-compiler> TYPE BIN)
endif()

if(SLINT_BUILD_RUNTIME)

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
            get_target_property(value ${slint_cpp_impl} ${prop})

            if(value)
                get_filename_component(value ${value} NAME)
                list(APPEND SLINT_LIB_PROPERTIES ${prop} "\${_IMPORT_PREFIX}/${CMAKE_INSTALL_LIBDIR}/${value}")
            endif()
        endforeach()

        # Corrosion sets IMPORTED_NO_SONAME and for macOS we reset that
        # and set IMPORTED_SONAME. Make sure to propagate these locally
        # set properties also to the installed cmake target.
        foreach(prop IMPORTED_NO_SONAME IMPORTED_SONAME)
            get_target_property(value ${slint_cpp_impl} ${prop})
            if(value)
                list(APPEND SLINT_LIB_PROPERTIES ${prop} ${value})
            endif()
        endforeach()

        get_property(_SLINT_STYLE GLOBAL PROPERTY SLINT_STYLE)
        if (TARGET slint-compiler)
            get_target_property(_slint_compiler_location slint-compiler IMPORTED_LOCATION)
            cmake_path(GET _slint_compiler_location FILENAME SLINT_COMPILER_FILE_NAME)
        endif()

        configure_package_config_file("cmake/SlintConfig.cmake.in" "${CMAKE_CURRENT_BINARY_DIR}/lib/cmake/Slint/SlintConfig.cmake" INSTALL_DESTINATION lib/cmake/Slint)
    endfunction()

    cmake_language(DEFER CALL _slint_write_configure_file)

    write_basic_package_version_file(
        ${CMAKE_CURRENT_BINARY_DIR}/lib/cmake/Slint/SlintConfigVersion.cmake
        VERSION ${PROJECT_VERSION}
        COMPATIBILITY SameMajorVersion
    )

    install(FILES
        "${CMAKE_CURRENT_BINARY_DIR}/lib/cmake/Slint/SlintConfig.cmake"
        "${CMAKE_CURRENT_BINARY_DIR}/lib/cmake/Slint/SlintConfigVersion.cmake"
        "${CMAKE_CURRENT_LIST_DIR}/cmake/SlintMacro.cmake"
        DESTINATION lib/cmake/Slint
    )

endif(SLINT_BUILD_RUNTIME)

set(CPACK_PACKAGE_NAME "Slint-cpp")
set(CPACK_PACKAGE_VENDOR "Slint")
set(CPACK_VERBATIM_VARIABLES true)
set(CPACK_PACKAGE_VERSION_MAJOR ${PROJECT_VERSION_MAJOR})
set(CPACK_PACKAGE_VERSION_MINOR ${PROJECT_VERSION_MINOR})
set(CPACK_PACKAGE_VERSION_PATCH ${PROJECT_VERSION_PATCH})
set(CPACK_PACKAGE_HOMEPAGE_URL "https://slint.dev")
set(CPACK_RESOURCE_FILE_LICENSE "${CMAKE_CURRENT_LIST_DIR}/../../LICENSE.md")
set(CPACK_RESOURCE_FILE_README "${CMAKE_CURRENT_LIST_DIR}/README.md")
set(CPACK_STRIP_FILES ON)
set(CPACK_NSIS_DEFINES "ManifestDPIAware true")

# Include a third-party license attribution folder, if generated by the CI.
if(EXISTS "${CMAKE_BINARY_DIR}/licenses")
    list(APPEND CPACK_INSTALLED_DIRECTORIES "${CMAKE_BINARY_DIR}/licenses" licenses)
endif()

if(WIN32)
    if(MSVC)
        set(compiler_suffix "-MSVC")
    elseif(MINGW)
        set(compiler_suffix "-MinGW")
    endif()
    if(CMAKE_SIZEOF_VOID_P EQUAL 8)
        set(CPACK_SYSTEM_NAME win64)
    else()
        set(CPACK_SYSTEM_NAME win32)
    endif()
    set(CPACK_SYSTEM_NAME "${CPACK_SYSTEM_NAME}${compiler_suffix}-${CMAKE_SYSTEM_PROCESSOR}")
else()
    set(CPACK_GENERATOR "TGZ")
    set(CPACK_SYSTEM_NAME "${CMAKE_SYSTEM_NAME}-${CMAKE_SYSTEM_PROCESSOR}")
endif()

if (NOT Rust_CARGO_TARGET STREQUAL Rust_CARGO_HOST_TARGET)
    # If the package contains host dependent content (the slint-compiler), append the target suffix.
    # For cross-builds that target bare-metal, strip the host suffix
    if (TARGET Slint::slint-compiler)
        string(APPEND CPACK_SYSTEM_NAME "-${Rust_CARGO_TARGET}")
    elseif(SLINT_FEATURE_FREESTANDING)
        set(CPACK_SYSTEM_NAME "${Rust_CARGO_TARGET}")
    endif()
endif()

include(CPack)

if(SLINT_BUILD_TESTING)
    add_subdirectory(tests)
endif()
