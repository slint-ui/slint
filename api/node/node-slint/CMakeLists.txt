# Copyright © SixtyFPS GmbH <info@slint.dev>
# SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

cmake_minimum_required(VERSION 3.16)
project(node-slint LANGUAGES CXX)

set(CMAKE_CXX_STANDARD 17)
set(CMAKE_CXX_STANDARD_REQUIRED ON)

# --- Configuration ------------------------------------------------------------

# Which Node.js version to build from source when no prebuilt libnode is found.
set(NODE_VERSION "22.22.2" CACHE STRING "Node.js version to download and build")

# --- Locate libnode -----------------------------------------------------------
#
# Search order:
#   1. NODE_DIR (CMake variable or environment variable)
#   2. pkg-config (libnode-dev on Debian/Ubuntu)
#   3. Download Node.js source and build (automatic)
#
# Within NODE_DIR, static archives (.a) are preferred over shared libraries.
# A static build produces a self-contained binary with no LD_LIBRARY_PATH needed.

if(NOT DEFINED NODE_DIR AND DEFINED ENV{NODE_DIR})
    set(NODE_DIR $ENV{NODE_DIR})
endif()

if(DEFINED NODE_DIR)
    # --- Option 1: User-provided Node.js installation -------------------------

    find_path(NODE_INCLUDE_DIR
        NAMES node.h
        PATHS
            "${NODE_DIR}/include/node"
            "${NODE_DIR}/include"
            "${NODE_DIR}/src"
            "${NODE_DIR}/deps/v8/include"
        NO_DEFAULT_PATH
    )
    if(NOT NODE_INCLUDE_DIR)
        message(FATAL_ERROR
            "Cannot find node.h under NODE_DIR=${NODE_DIR}.\n"
            "Expected in include/node/, include/, or src/.")
    endif()

    # Collect include directories.
    set(NODE_INCLUDE_DIRS "${NODE_INCLUDE_DIR}")
    foreach(_subdir
        "${NODE_DIR}/deps/v8/include"
        "${NODE_DIR}/deps/uv/include"
        "${NODE_DIR}/deps/cares/include"
    )
        if(IS_DIRECTORY "${_subdir}")
            list(APPEND NODE_INCLUDE_DIRS "${_subdir}")
        endif()
    endforeach()

    # Check for static archives first.
    file(GLOB _node_static_libs "${NODE_DIR}/lib/*.a")
    if(_node_static_libs)
        set(NODE_STATIC TRUE)
        set(NODE_LIBRARIES ${_node_static_libs})
        message(STATUS "Node.js (static, user-provided)")
        message(STATUS "  includes:  ${NODE_INCLUDE_DIRS}")
        message(STATUS "  archives:  ${NODE_LIBRARIES}")
    else()
        # Fall back to shared library.
        set(NODE_STATIC FALSE)
        find_library(NODE_LIBRARY
            NAMES node libnode
            PATHS
                "${NODE_DIR}/lib"
                "${NODE_DIR}/out/Release/lib"
                "${NODE_DIR}/out/Release"
                "${NODE_DIR}/Release"
            NO_DEFAULT_PATH
        )
        if(NOT NODE_LIBRARY)
            file(GLOB _node_libs
                "${NODE_DIR}/lib/libnode${CMAKE_SHARED_LIBRARY_SUFFIX}.*"
                "${NODE_DIR}/out/Release/lib/libnode${CMAKE_SHARED_LIBRARY_SUFFIX}.*"
            )
            if(_node_libs)
                list(GET _node_libs 0 NODE_LIBRARY)
            endif()
        endif()
        if(NOT NODE_LIBRARY)
            message(FATAL_ERROR
                "Cannot find libnode under NODE_DIR=${NODE_DIR}.\n"
                "Run ./build-libnode.sh --prefix ${NODE_DIR}")
        endif()
        set(NODE_LIBRARIES ${NODE_LIBRARY})
        message(STATUS "Node.js (shared, user-provided)")
        message(STATUS "  includes: ${NODE_INCLUDE_DIRS}")
        message(STATUS "  library:  ${NODE_LIBRARIES}")
    endif()

elseif(NOT DEFINED NODE_DIR)
    # --- Option 2: pkg-config (system libnode-dev) ----------------------------
    find_package(PkgConfig QUIET)
    if(PkgConfig_FOUND)
        pkg_check_modules(NODE QUIET libnode)
    endif()

    if(NODE_FOUND)
        set(NODE_STATIC FALSE)
        set(NODE_INCLUDE_DIRS ${NODE_INCLUDE_DIRS})
        set(NODE_LIBRARIES ${NODE_LINK_LIBRARIES})
        message(STATUS "Node.js (pkg-config)")
        message(STATUS "  includes: ${NODE_INCLUDE_DIRS}")
        message(STATUS "  library:  ${NODE_LIBRARIES}")
    else()
        # --- Option 3: Download and build from source -------------------------
        message(STATUS "No prebuilt libnode found — building Node.js ${NODE_VERSION} from source.")
        message(STATUS "This will take a while on the first build.  Set NODE_DIR to skip.")

        include(ExternalProject)

        set(_node_install "${CMAKE_BINARY_DIR}/node-install")

        ExternalProject_Add(libnode
            URL "https://nodejs.org/dist/v${NODE_VERSION}/node-v${NODE_VERSION}.tar.gz"
            PREFIX "${CMAKE_BINARY_DIR}/node-build"
            BUILD_IN_SOURCE TRUE
            CONFIGURE_COMMAND
                ./configure
                    --prefix=<INSTALL_DIR>
            BUILD_COMMAND
                make -j$ENV{CMAKE_BUILD_PARALLEL_LEVEL}
            INSTALL_COMMAND
                make install
            INSTALL_DIR "${_node_install}"
            UPDATE_COMMAND ""
            LOG_CONFIGURE TRUE
            LOG_BUILD TRUE
            LOG_INSTALL TRUE
        )

        set(NODE_STATIC FALSE)
        set(NODE_INCLUDE_DIRS "${_node_install}/include/node")
        set(NODE_LIBRARIES "${_node_install}/lib/${CMAKE_SHARED_LIBRARY_PREFIX}node${CMAKE_SHARED_LIBRARY_SUFFIX}")
        set(_node_slint_depends libnode)
    endif()
endif()

# --- Build node-slint ---------------------------------------------------------

add_executable(node-slint main.cpp)

if(_node_slint_depends)
    add_dependencies(node-slint ${_node_slint_depends})
endif()

target_include_directories(node-slint PRIVATE ${NODE_INCLUDE_DIRS})
target_link_libraries(node-slint PRIVATE ${NODE_LIBRARIES})

if(NODE_STATIC)
    # Static archives need system libraries that Node.js normally links.
    find_package(Threads REQUIRED)
    target_link_libraries(node-slint PRIVATE
        Threads::Threads
        ${CMAKE_DL_LIBS}   # dlopen/dlsym
    )
    if(UNIX AND NOT APPLE)
        target_link_libraries(node-slint PRIVATE rt)
    endif()
    # V8 needs C++ standard library.
    target_link_libraries(node-slint PRIVATE stdc++)
endif()

# Export symbols so the NAPI addon (loaded via dlopen) can resolve them.
if(UNIX AND NOT APPLE)
    target_link_options(node-slint PRIVATE -rdynamic)
endif()

if(APPLE)
    target_link_options(node-slint PRIVATE -undefined dynamic_lookup)
endif()

install(TARGETS node-slint RUNTIME DESTINATION bin)
