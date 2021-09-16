# Getting Started

In this tutorial, we use C++ as the host programming language. We also support other programming languages like
[Rust](https://sixtyfps.io/docs/rust/sixtyfps/) or [JavaScript](https://sixtyfps.io/docs/node/).

You will need a development environment that can compile C++17 with CMake 3.19.
We do not provide binaries of SixtyFPS yet, so we will use the CMake integration that will automatically build
the tools and library from source. Since it is implemented in the Rust programming language, this means that
you also need to install a Rust compiler (1.48). You can easily install a Rust compiler
following the instruction from [the Rust website](https://www.rust-lang.org/learn/get-started).
We are going to use `cmake`'s builtin FetchContent module to fetch the source code of SixtyFPS.

In a new directory, we create a new `CMakeLists.txt` file.

```cmake
# CMakeLists.txt
cmake_minimum_required(VERSION 3.19)
project(memory LANGUAGES CXX)

include(FetchContent)
FetchContent_Declare(
    SixtyFPS
    GIT_REPOSITORY https://github.com/sixtyfpsui/sixtyfps.git
    GIT_TAG v0.1.2
    SOURCE_SUBDIR api/sixtyfps-cpp
)
FetchContent_MakeAvailable(SixtyFPS)

add_executable(memory_game main.cpp)
target_link_libraries(memory_game PRIVATE SixtyFPS::SixtyFPS)
sixtyfps_target_60_sources(memory_game memory.60)
```

This should look familiar to people familiar with CMake. We see that this CMakeLists.txt
references a `main.cpp`, which we will add later, and it also has a line
`sixtyfps_target_60_sources(memory_game memory.60)`, which is a SixtyFPS function used to
add the `memory.60` file to the target. We must then create, in the same directory,
the `memory.60` file. Let's just fill it with a hello world for now:

```60
{{#include memory.60:main_window}}
```

What's still missing is the `main.cpp`:

```cpp
{{#include main_initial.cpp:main}}
```

To recap, we now have a directory with a `CMakeLists.txt`, `memory.60` and `main.cpp`.

We can now compile and run this program:

```sh
cmake -GNinja .
cmake --build .
./memory_game
```

and a window will appear with the green "Hello World" greeting.

![Screenshot of initial tutorial app showing Hello World](https://sixtyfps.io/blog/memory-game-tutorial/getting-started.png "Hello World")

Feel free to use your favorite IDE for this purpose, or use out-of-tree build, or Ninja, ...
We just keep it simple here for the purpose of this blog.

*Note*: When configuring with CMake, the FetchContent module will fetch the source code of SixtyFPS via git.
this may take some time. When building for the first time, the first thing that need to be build
is the SixtyFPS runtime and compiler, this can take a few minutes.
