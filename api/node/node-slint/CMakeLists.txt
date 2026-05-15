# Copyright © SixtyFPS GmbH <info@slint.dev>
# SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

cmake_minimum_required(VERSION 3.16)
project(node-slint LANGUAGES CXX)

set(CMAKE_CXX_STANDARD 17)
set(CMAKE_CXX_STANDARD_REQUIRED ON)

# --- Configuration ------------------------------------------------------------

# Which Node.js version to build from source when no prebuilt libnode is found.
set(NODE_VERSION "22.22.2" CACHE STRING "Node.js version to download and build")

# --- Locate or build libnode --------------------------------------------------
#
# Search order:
#   1. NODE_DIR (CMake variable or environment variable)
#   2. pkg-config (libnode-dev on Debian/Ubuntu)
#   3. Download Node.js source and build with --shared (automatic)

if(NOT DEFINED NODE_DIR AND DEFINED ENV{NODE_DIR})
    set(NODE_DIR $ENV{NODE_DIR})
endif()

if(DEFINED NODE_DIR)
    # --- Option 1: User-provided Node.js installation -------------------------

    # Look for headers in several common locations.
    find_path(NODE_INCLUDE_DIR
        NAMES node.h
        PATHS
            "${NODE_DIR}/include/node"
            "${NODE_DIR}/include"
            "${NODE_DIR}/src"            # in-source-tree builds
            "${NODE_DIR}/deps/v8/include" # fallback for V8 headers
        NO_DEFAULT_PATH
    )
    if(NOT NODE_INCLUDE_DIR)
        message(FATAL_ERROR
            "Cannot find node.h under NODE_DIR=${NODE_DIR}.\n"
            "Expected in include/node/, include/, or src/.")
    endif()

    # Look for the library.  Node.js installs a versioned name like
    # libnode.so.127 without a plain libnode.so symlink, so we also glob
    # for the versioned file.
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
        # Fallback: find the versioned .so (e.g. libnode.so.127).
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
            "Build Node.js with: ./configure --shared && make -j$$(nproc)")
    endif()

    # Collect all include directories needed for embedding.
    set(NODE_INCLUDE_DIRS "${NODE_INCLUDE_DIR}")

    # When pointing at a Node.js source tree, V8 and libuv headers live in
    # separate directories.
    foreach(_subdir
        "${NODE_DIR}/deps/v8/include"
        "${NODE_DIR}/deps/uv/include"
        "${NODE_DIR}/deps/cares/include"
    )
        if(IS_DIRECTORY "${_subdir}")
            list(APPEND NODE_INCLUDE_DIRS "${_subdir}")
        endif()
    endforeach()

    message(STATUS "Node.js (user-provided)")
    message(STATUS "  includes: ${NODE_INCLUDE_DIRS}")
    message(STATUS "  library:  ${NODE_LIBRARY}")

elseif(NOT DEFINED NODE_DIR)
    # --- Option 2: pkg-config (system libnode-dev) ----------------------------
    find_package(PkgConfig QUIET)
    if(PkgConfig_FOUND)
        pkg_check_modules(NODE QUIET libnode)
    endif()

    if(NODE_FOUND)
        set(NODE_INCLUDE_DIRS ${NODE_INCLUDE_DIRS})
        set(NODE_LIBRARY ${NODE_LINK_LIBRARIES})
        message(STATUS "Node.js (pkg-config)")
        message(STATUS "  includes: ${NODE_INCLUDE_DIRS}")
        message(STATUS "  library:  ${NODE_LIBRARY}")
    else()
        # --- Option 3: Download and build from source -------------------------
        message(STATUS "No prebuilt libnode found — building Node.js ${NODE_VERSION} from source.")
        message(STATUS "This will take a while on the first build.  Set NODE_DIR to skip.")

        include(ExternalProject)

        set(_node_install "${CMAKE_BINARY_DIR}/node-install")

        ExternalProject_Add(libnode
            URL "https://nodejs.org/dist/v${NODE_VERSION}/node-v${NODE_VERSION}.tar.gz"
            PREFIX "${CMAKE_BINARY_DIR}/node-build"
            # Build in the source tree (Node.js doesn't support out-of-tree).
            BUILD_IN_SOURCE TRUE
            CONFIGURE_COMMAND
                ./configure
                    --shared
                    --prefix=<INSTALL_DIR>
            BUILD_COMMAND
                make -j$ENV{CMAKE_BUILD_PARALLEL_LEVEL}
            INSTALL_COMMAND
                make install
            INSTALL_DIR "${_node_install}"
            # Only re-run configure when the URL changes.
            UPDATE_COMMAND ""
            LOG_CONFIGURE TRUE
            LOG_BUILD TRUE
            LOG_INSTALL TRUE
        )

        # Headers and library will be at these paths after the build.
        set(NODE_INCLUDE_DIRS "${_node_install}/include/node")
        set(NODE_LIBRARY "${_node_install}/lib/${CMAKE_SHARED_LIBRARY_PREFIX}node${CMAKE_SHARED_LIBRARY_SUFFIX}")

        # Make sure node-slint waits for the ExternalProject build.
        set(_node_slint_depends libnode)
    endif()
endif()

# --- Build node-slint ---------------------------------------------------------

add_executable(node-slint main.cpp)

if(_node_slint_depends)
    add_dependencies(node-slint ${_node_slint_depends})
endif()

target_include_directories(node-slint PRIVATE ${NODE_INCLUDE_DIRS})
target_link_libraries(node-slint PRIVATE ${NODE_LIBRARY})

# On Linux, export symbols from the executable so that the NAPI addon
# (a shared library loaded via dlopen) can resolve them.
if(UNIX AND NOT APPLE)
    target_link_options(node-slint PRIVATE -rdynamic)
endif()

# On macOS, allow undefined symbols at link time (they are resolved when
# the addon is loaded into the process).
if(APPLE)
    target_link_options(node-slint PRIVATE -undefined dynamic_lookup)
endif()

install(TARGETS node-slint RUNTIME DESTINATION bin)
