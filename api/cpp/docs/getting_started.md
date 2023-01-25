# Getting Started

Once Slint is built, you can use it in your CMake application or library target in two steps:

1. Associate the `.slint` files that you'd like to use by calling the `slint_target_sources` cmake command. The first parameter is
   your application (or library) CMake target, and the parameters following are the names of the `.slint` files. This will result in the
   `.slint` files to be compiled into C++ source code.
2. The generated C++ source code also needs the Slint run-time library. This dependency is satisfied by linking `Slint::Slint`
   into your target with the `target_link_libraries` command.

A typical example looks like this:

```cmake
cmake_minimum_required(VERSION 3.19)
project(my_application LANGUAGES CXX)

# Note: Use find_package(Slint) instead of the following three commands,
# if you prefer the package approach.
include(FetchContent)
FetchContent_Declare(
    Slint
    GIT_REPOSITORY https://github.com/slint-ui/slint.git
    GIT_TAG release/0.3
    SOURCE_SUBDIR api/cpp
)
FetchContent_MakeAvailable(Slint)

add_executable(my_application main.cpp)
slint_target_sources(my_application my_application_ui.slint)
target_link_libraries(my_application PRIVATE Slint::Slint)
# On Windows, copy the Slint DLL next to the application binary so that it's found.
if (WIN32)
    add_custom_command(TARGET my_application POST_BUILD COMMAND ${CMAKE_COMMAND} -E copy $<TARGET_RUNTIME_DLLS:my_application> $<TARGET_FILE_DIR:my_application> COMMAND_EXPAND_LISTS)
endif()
```

Suppose `my_application_ui.slint` was a "Hello World" like this:

```slint,ignore
export component HelloWorld inherits Window {
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

This works because the Slint compiler translated `my_application_ui.slint` to C++ code, in the `my_application_ui.h`
header file. That generated code has a C++ class that corresponds to the `HelloWorld` element and has API to create
the ui, read or write properties or set callbacks. You can learn more about how this API looks like in general in the
[](generated_code.md) section.

## Tutorial

For an in-depth walk-through, you may be interested in reading our walk-through <a href="../tutorial/cpp">Slint Memory Game Tutorial Tutorial</a>.
It will guide you through the `.slint` mark-up language and the C++ API by building a little memory game.

## Template

You can clone the [Template Repository](https://github.com/slint-ui/slint-cpp-template) repository with
the code of a minimal C++ application using Slint that can be used as a starting point to your program.
