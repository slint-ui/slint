# Copyright © SixtyFPS GmbH <info@slint.dev>
# SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-1.1 OR LicenseRef-Slint-commercial

FetchContent_Declare(
    Catch2
    GIT_REPOSITORY https://github.com/catchorg/Catch2.git
    GIT_TAG v2.13.8
)

FetchContent_MakeAvailable(Catch2)

find_package(Threads REQUIRED)

macro(slint_test NAME)
    add_executable(test_${NAME} ${NAME}.cpp)
    target_link_libraries(test_${NAME} PRIVATE Slint Catch2::Catch2)
    target_compile_definitions(test_${NAME} PRIVATE
        SOURCE_DIR=\"${CMAKE_CURRENT_SOURCE_DIR}/\"
    )
    add_test(NAME test_${NAME} COMMAND test_${NAME})

    if(MSVC)
        target_compile_options(test_${NAME} PRIVATE /W3)
    else()
        target_compile_options(test_${NAME} PRIVATE -Wall -Wextra -Werror)
    endif()

    if(CMAKE_CXX_COMPILER_ID STREQUAL GNU)
        # that warning has false positives
        target_compile_options(test_${NAME} PRIVATE -Wno-maybe-uninitialized)
    endif()
endmacro(slint_test)

slint_test(datastructures)

if(SLINT_FEATURE_INTERPRETER)
    slint_test(interpreter)
    slint_test(window)
endif()

slint_test(properties)
if(SLINT_FEATURE_STD)
    slint_test(eventloop)
    target_link_libraries(test_eventloop PRIVATE Threads::Threads)
endif()
slint_test(models)

add_subdirectory(multiple-includes)

if(SLINT_FEATURE_EXPERIMENTAL)
    slint_test(platform_eventloop)
    target_link_libraries(test_platform_eventloop PRIVATE Threads::Threads)
    if(Qt6_FOUND)
        add_subdirectory(manual/platform_qt)
    endif(Qt6_FOUND)
    add_subdirectory(manual/platform_native)
endif(SLINT_FEATURE_EXPERIMENTAL)
