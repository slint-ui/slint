<!-- Copyright © SixtyFPS GmbH <info@slint.dev> ; SPDX-License-Identifier: MIT -->

# Getting Started

This tutorial uses C++ as the host programming language. Slint also supports other programming languages like
[Rust](https://slint.dev/docs/rust/slint/) or [JavaScript](https://slint.dev/docs/node/).

Slint has an application template you can use to create a project with dependencies already set up that follows recommended best practices.

Before using the template, you need a C++ compiler that supports C++ 20 and to install [CMake](https://cmake.org/download/) 3.21 or newer.

Clone or download template repository:

```sh
git clone https://github.com/slint-ui/slint-cpp-template memory
cd memory
```

Change all the references to `my_application` in the `CMakeLists.txt` file to `memory`.

The `CMakeLists.txt` uses the line `add_executable(memory src/main.cpp)` to set `src/main.cpp` as the main C++ code file.

```cpp
{{#include main_initial.cpp:main}}
```

Also in `CMakeLists.txt` the line
`slint_target_sources(memory ui/appwindow.slint)` is a Slint function used to
add the `appwindow.slint` file to the target.

Change the contents of `appwindow.slint` to the following:

```slint
{{#include appwindow.slint:main_window}}
```

Configure with CMake:

```sh
mkdir build
cmake -B build
```

Build with CMake:

```sh
cmake --build build
```

Run the application binary on Linux or macOS:

```sh
./build/memory
```

Windows:

```sh
build\my_application.exe
```

This opens a window with a green "Hello World" greeting.

If you are stepping through this tutorial on a Windows machine, you can run it with

```sh
memory
```

![Screenshot of initial tutorial app showing Hello World](https://slint.dev/blog/memory-game-tutorial/getting-started.png "Hello World")

_Note_: When configuring with CMake, the FetchContent module fetches the source code of Slint via git.
This may take some time when building for the first time, as the process needs to build
the Slint runtime and compiler.
