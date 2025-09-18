# Copyright © SixtyFPS GmbH <info@slint.dev>
# SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0
#
# FindSlint
# ---------
#
# This modules attempts to locate an installation of Slint, as follows:
#
# 1. First `find_package(Slint ... CONFIG ...)` is called, to locate any packages in the `CMAKE_PREFIX_PATH`.
# 2. If that failed and if `find_package` was called with a `VERSION`, then this module will attempt to download
#    a pre-compiled binary package for the specified Slint release, extract it into `${CMAKE_BINARY_DIR}/slint-prebuilt`,
#    and make it available. If version is unset, download the nightly release.
#
# The following variables may be set to affect the behaviour:
#
# `SLINT_TARGET_ARCHITECTURE`: Set this to the desired target architecture. The format of this string is matched against
# the `Slint-cpp-*-$SLINT_TARGET_ARCHITECTURE.tar.gz` pre-built assets on the GitHub releases. For example, if you're targeting
# STM32 ARM architectures, you'd set this to `thumbv7em-none-eabihf`. If not set, this module will attempt to detect if compilation
# is happening in an ESP-IDF cross-compilation environment and detect the architecture accordingly, otherwise
# `${CMAKE_SYSTEM_NAME}-${CMAKE_SYSTEM_PROCESSOR}` is used.

find_package(Slint ${Slint_FIND_VERSION} QUIET CONFIG)
if (TARGET Slint::Slint)
    return()
endif()

if (NOT SLINT_TARGET_ARCHITECTURE)
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
        set(SLINT_TARGET_ARCHITECTURE "${CPACK_SYSTEM_NAME}${compiler_suffix}-${CMAKE_SYSTEM_PROCESSOR}")
    elseif (CONFIG_IDF_TARGET_ARCH_XTENSA)
        set(SLINT_TARGET_ARCHITECTURE "xtensa-${IDF_TARGET}-none-elf")
    elseif(CONFIG_IDF_TARGET_ARCH_RISCV)
        if (CONFIG_IDF_TARGET_ESP32C6 OR CONFIG_IDF_TARGET_ESP32C5 OR CONFIG_IDF_TARGET_ESP32H2)
            set(SLINT_TARGET_ARCHITECTURE "riscv32imac-esp-espidf")
        elseif (CONFIG_IDF_TARGET_ESP32P4)
            set(SLINT_TARGET_ARCHITECTURE "riscv32imafc-esp-espidf")
        else ()
            set(SLINT_TARGET_ARCHITECTURE "riscv32imc-esp-espidf")
        endif()
    else()
        set(SLINT_TARGET_ARCHITECTURE "${CMAKE_SYSTEM_NAME}-${CMAKE_SYSTEM_PROCESSOR}")
    endif()
endif()

if (NOT DEFINED Slint_FIND_VERSION)
    # Set this to instruct the slint-compiler download to use the same release
    set(SLINT_GITHUB_RELEASE "nightly" CACHE STRING "")
    set(github_release "nightly")
    set(github_filename_infix "nightly")
else()
    set(github_release "v${Slint_FIND_VERSION}")
    set(github_filename_infix "${Slint_FIND_VERSION}")
endif()

set(prebuilt_archive_filename "Slint-cpp-${github_filename_infix}-${SLINT_TARGET_ARCHITECTURE}.tar.gz")
set(download_target_path "${CMAKE_BINARY_DIR}/slint-prebuilt/")
set(download_url "https://github.com/slint-ui/slint/releases/download/${github_release}/${prebuilt_archive_filename}")

file(MAKE_DIRECTORY "${download_target_path}")
message(STATUS "Downloading pre-built Slint binary ${download_url}")
file(DOWNLOAD "${download_url}" "${download_target_path}/${prebuilt_archive_filename}" STATUS download_status)
list(GET download_status 0 download_code)
if (NOT download_code EQUAL 0)
    list(GET download_status 1 download_message)
    message(STATUS "Download of Slint binary package failed: ${download_message}")
    return()
endif()

file(ARCHIVE_EXTRACT INPUT "${download_target_path}/${prebuilt_archive_filename}" DESTINATION "${download_target_path}")
list(PREPEND CMAKE_PREFIX_PATH "${download_target_path}")
find_package(Slint CONFIG)
