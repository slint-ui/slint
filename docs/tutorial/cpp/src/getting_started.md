<!-- Copyright © SixtyFPS GmbH <info@slint.dev> ; SPDX-License-Identifier: MIT -->
# Getting Started

In this tutorial, we use C++ as the host programming language. We also support other programming languages like
[Rust](https://slint.dev/docs/rust/slint/) or [JavaScript](https://slint.dev/docs/node/).

You will need a development environment that can compile C++20, [CMake 3.21](https://cmake.org/download/),
and we recommend [Ninja](https://ninja-build.org) for `-GNinja`.
We don't provide binaries of Slint yet, so we will use the CMake integration that will automatically build
the tools and library from source. Since it's implemented in the Rust programming language, this means that
you also need to install a Rust compiler (1.66 or newer). You can easily install a Rust compiler
following the instruction from [the Rust website](https://www.rust-lang.org/learn/get-started).
We're going to use `cmake`'s builtin FetchContent module to fetch the source code of Slint.

In a new directory, we create a new `CMakeLists.txt` file.

```cmake
# CMakeLists.txt
cmake_minimum_required(VERSION 3.21)
project(memory LANGUAGES CXX)

include(FetchContent)
FetchContent_Declare(
    Slint
    GIT_REPOSITORY https://github.com/slint-ui/slint.git
    # `release/1` will auto-upgrade to the latest Slint >= 1.0.0 and < 2.0.0
    # `release/1.0` will auto-upgrade to the latest Slint >= 1.0.0 and < 1.1.0
    GIT_TAG release/1
    SOURCE_SUBDIR api/cpp
)
FetchContent_MakeAvailable(Slint)

add_executable(memory_game main.cpp)
target_link_libraries(memory_game PRIVATE Slint::Slint)
slint_target_sources(memory_game memory.slint)
# On Windows, copy the Slint DLL next to the application binary so that it's found.
if (WIN32)
    add_custom_command(TARGET memory_game POST_BUILD COMMAND ${CMAKE_COMMAND} -E copy $<TARGET_RUNTIME_DLLS:memory_game> $<TARGET_FILE_DIR:memory_game> COMMAND_EXPAND_LISTS)
endif()
```

This should look familiar to people familiar with CMake. We see that this CMakeLists.txt
references a `main.cpp`, which we will add later, and it also has a line
`slint_target_sources(memory_game memory.slint)`, which is a Slint function used to
add the `memory.slint` file to the target. We must then create, in the same directory,
the `memory.slint` file. Let's just fill it with a hello world for now:

```slint
{{#include memory.slint:main_window}}
```

What's still missing is the `main.cpp`:

```cpp
{{#include main_initial.cpp:main}}
```

To recap, we now have a directory with a `CMakeLists.txt`, `memory.slint` and `main.cpp`.

We can now compile the program in a terminal:

```sh
cmake -GNinja .
cmake --build .
```

If you are on Linux or macOS, you can run the program:

```sh
./memory_game
```

and a window will appear with the green "Hello World" greeting.

If you are stepping through this tutorial on a Windows machine, you can run it with

```sh
memory_game
```

![Screenshot of initial tutorial app showing Hello World](https://slint.dev/blog/memory-game-tutorial/getting-started.png "Hello World")

Feel free to use your favorite IDE for this purpose, or use out-of-tree build, or Ninja, ...
We just keep it simple here for the purpose of this blog.

_Note_: When configuring with CMake, the FetchContent module will fetch the source code of Slint via git.
this may take some time. When building for the first time, the first thing that need to be build
is the Slint runtime and compiler, this can take a few minutes.
