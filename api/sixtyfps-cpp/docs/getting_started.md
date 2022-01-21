# Getting Started

Once SixtyFPS is built, you can use it in your CMake application or library target in two steps:

1. Associate the `.60` files that you'd like to use by calling the `sixtyfps_target_60_sources` cmake command. The first parameter is
   your application (or library) CMake target, and the parameters following are the names of the `.60` files. This will result in the
   `.60` files to be compiled into C++ source code.
2. The generated C++ source code also needs the SixtyFPS run-time library. This dependency is satisfied by linking `SixtyFPS::SixtyFPS`
   into your target with the `target_link_libraries` command.

A typical example looks like this:

```cmake
cmake_minimum_required(VERSION 3.19)
project(my_application LANGUAGES CXX)

# Note: Use find_package(SixtyFPS) instead of the following three commands,
# if you prefer the package approach.
include(FetchContent)
FetchContent_Declare(
    SixtyFPS
    GIT_REPOSITORY https://github.com/sixtyfpsui/sixtyfps.git
    GIT_TAG v0.1.6
    SOURCE_SUBDIR api/sixtyfps-cpp
)
FetchContent_MakeAvailable(SixtyFPS)

add_executable(my_application main.cpp)
sixtyfps_target_60_sources(my_application my_application_ui.60)
target_link_libraries(my_application PRIVATE SixtyFPS::SixtyFPS)
```

Suppose `my_application_ui.60` was a "Hello World" like this:

```60,ignore
HelloWorld := Window {
    width: 400px;
    height: 400px;

    // Declare an alias that exposes the label's text property to C++
    property my_label <=> label.text;

    label := Text {
       y: parent.width / 2;
       x: parent.x + 200px;
       text: "Hello, world";
       color: blue;
    }
}
```

then you can use the following code in you `main` function to show the [`Window`](markdown/builtin_elements.md#window)
and change the text:

```cpp
#include "my_application_ui.h"

int main(int argc, char **argv)
{
    auto hello_world = HelloWorld::create();
    hello_world->set_my_label("Hello from C++");
    // Show the window and spin the event loop until the window is closed.
    hello_world->run();
    return 0;
}
```

This works because the SixtyFPS compiler translated `my_application_ui.60` to C++ code, in the `my_application_ui.h`
header file. That generated code has a C++ class that corresponds to the `HelloWorld` element and has API to create
the ui, read or write properties or set callbacks. You can learn more about how this API looks like in general in the
[](generated_code.md) section.

## Tutorial

For an in-depth walk-through, you may be interested in reading our walk-through <a href="../tutorial/cpp">SixtyFPS Memory Game Tutorial Tutorial</a>.
It will guide you through the `.60` mark-up language and the C++ API by building a little memory game.

## Template

You can clone the [Template Repository](https://github.com/sixtyfpsui/sixtyfps-cpp-template) repository with
the code of a minimal C++ application using SixtyFPS that can be used as a starting point to your program.
