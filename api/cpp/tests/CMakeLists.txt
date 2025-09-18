# Copyright © SixtyFPS GmbH <info@slint.dev>
# SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

FetchContent_Declare(
    Catch2
    GIT_REPOSITORY https://github.com/catchorg/Catch2.git
    GIT_TAG v3.8.0
)

FetchContent_MakeAvailable(Catch2)

find_package(Threads REQUIRED)

macro(slint_test NAME)
    add_executable(test_${NAME} ${NAME}.cpp)
    target_link_libraries(test_${NAME} PRIVATE Slint Catch2::Catch2WithMain)
    target_compile_definitions(test_${NAME} PRIVATE
        SOURCE_DIR=\"${CMAKE_CURRENT_SOURCE_DIR}/\"
    )
    # Use debug version of run-time library to enable MSVC iterator debugging
    set_property(TARGET test_${NAME} PROPERTY MSVC_RUNTIME_LIBRARY MultiThreadedDebugDLL)
    add_test(NAME test_${NAME} COMMAND test_${NAME})

    if(MSVC)
        target_compile_options(test_${NAME} PRIVATE /W3)
    else()
        target_compile_options(test_${NAME} PRIVATE -Wall -Wextra -Werror)
    endif()

    if(CMAKE_CXX_COMPILER_ID STREQUAL GNU AND CMAKE_CXX_COMPILER_VERSION VERSION_LESS 14)
        # that warning has false positives in GCC 11  https://github.com/slint-ui/slint/issues/7358
        target_compile_options(test_${NAME} PRIVATE -Wno-maybe-uninitialized)
    endif()
endmacro(slint_test)

slint_test(datastructures)

if(SLINT_FEATURE_INTERPRETER)
    slint_test(interpreter)
    slint_test(window)
endif()

slint_test(properties)
if(NOT SLINT_FEATURE_FREESTANDING)
    slint_test(eventloop)
    target_link_libraries(test_eventloop PRIVATE Threads::Threads)
endif()
slint_test(models)

if(SLINT_FEATURE_EXPERIMENTAL AND SLINT_FEATURE_TESTING)
    slint_test(testing)
endif()

if(SLINT_FEATURE_COMPILER OR SLINT_COMPILER)
    add_subdirectory(multiple-includes)
    add_subdirectory(libraries)
endif()

slint_test(platform_eventloop)
target_link_libraries(test_platform_eventloop PRIVATE Threads::Threads)

