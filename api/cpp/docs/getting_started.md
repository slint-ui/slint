# Getting Started

Once Slint is built, you can use it in your CMake application or library
target in two steps:

1. Associate the `.slint` files that you'd like to use by calling the
   `slint_target_sources` cmake command. The first parameter is
   your application (or library) build target, and the parameters following are
   the names of the `.slint` files you want to include. This will compile
   the `.slint` files to C++ source code and included that into your
   built target.
2. The generated C++ source code needs the Slint run-time library. Use
   `target_link_libraries` to link your build target to `Slint::Slint`.

A minimal CMake `CMakeLists.txt` file looks like this:

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

then you can use the following code in you `main` function to show the [`Window`](../slint/builtin_elements.html#window)
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
header file. That generated code contains a C++ class that corresponds to the `HelloWorld` element and has API to create
the UI, read or write properties, and set callbacks. You can learn more about how this API looks like in general in the
[](generated_code.md) section.

## Tutorial

For an in-depth walk-through, read our <a href="../tutorial/cpp">Slint Memory Game Tutorial</a>.
It will guide you through the `.slint` mark-up language and the C++ API by building a simple memory
game.

## Template

You can clone the [Template Repository](https://github.com/slint-ui/slint-cpp-template) repository with
the code of a minimal C++ application using Slint. It provides a convenient starting point to a new program.

Of course you can also read on: We will cover some recipes to handle common
use-cases next.
