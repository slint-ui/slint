# SixtyFPS-cpp

**A C++ UI toolkit**

[SixtyFPS](https://www.sixtyfps.io/) is a UI toolkit that supports different programming languages.
SixtyFPS.cpp is the C++ API to interact with a SixtyFPS UI from C++.

The complete C++ documentation can be viewed online at https://www.sixtyfps.io/docs/cpp/.

**Warning: Pre-Alpha**
SixtyFPS is still in the early stages of development: APIs will change and important features are still being developed.

## Installing or Building SixtyFPS

SixtyFPS comes with a CMake integration that automates the compilation step of the `.60` markup language files and
offers a CMake target for convenient linkage.

### Building from Sources

The recommended and most flexible way to use the C++ API is to build SixtyFPS from sources.

First you need to install the prerequisites:

 * Install Rust by following the [Rust Getting Started Guide](https://www.rust-lang.org/learn/get-started). Once this is done,
   you should have the ```rustc``` compiler and the ```cargo``` build system installed in your path.
 * **cmake** (3.16 or newer)
 * A C++ compiler that supports C++17 (e.g., **MSVC 2019** on Windows)

You can include SixtyFPS in your CMake project using CMake's `FetchContent` feature. Insert the following snippet into your
`CMakeLists.txt` to make CMake download the latest release, compile it and make the CMake integration available:

```cmake
include(FetchContent)
FetchContent_Declare(
    SixtyFPS
    GIT_REPOSITORY https://github.com/sixtyfpsui/sixtyfps.git
    GIT_TAG v0.0.2
    SOURCE_SUBDIR api/sixtyfps-cpp
)
FetchContent_MakeAvailable(SixtyFPS)
```

If you prefer to treat SixtyFPS as an external CMake package, then you can also build SixtyFPS from source like a regular
CMake project, install it into a prefix directory of your choice and use `find_package(SixtyFPS)` in your `CMakeLists.txt`.

### Binary Packages

The SixtyFPS continuous integration system is building binary packages to use with C++ so that you do not need to install a rust compiler.
These binaries can be found by clicking on the last
[succesful build of the master branch](https://github.com/sixtyfpsui/sixtyfps/actions?query=workflow%3ACI+is%3Asuccess+branch%3Amaster)
and downloading the `cpp_bin` artifact.

After extracting the artifact you can place the `lib` directory into your `CMAKE_PREFIX_PATH` and `find_package(SixtyFPS)` should succeed
in locating the package.

## Usage via CMake

A typical example looks like this:

```cmake
cmake_minimum_required(VERSION 3.16)
project(my_application LANGUAGES CXX)

# Note: Use find_package(SixtyFPS) instead of the following three commands, if you prefer the package
# approach.
include(FetchContent)
FetchContent_Declare(
    SixtyFPS
    GIT_REPOSITORY https://github.com/sixtyfpsui/sixtyfps.git
    GIT_TAG v0.0.2
    SOURCE_SUBDIR api/sixtyfps-cpp
)
FetchContent_MakeAvailable(SixtyFPS)

add_executable(my_application main.cpp)
target_link_libraries(my_application SixtyFPS::SixtyFPS)
sixtyfps_target_60_sources(my_application my_application_ui.60)
```

The `sixtyfps_target_60_sources` cmake command allows you to add .60 files to your build. Finally it is
necessary to link your executable or library against the `SixtyFPS::SixtyFPS` target.

## Tutorial

Let's make a UI for a todo list application using the SixtyFPS UI description language.
Hopefully this should be self explainatory. Check out the documentation of the language for help

```60
// file: my_application_ui.60
import { CheckBox, Button, ListView, LineEdit } from "sixtyfps_widgets.60";

export TodoItem := {
    property <string> title;
    property <bool> checked;
}

export MainWindow := Window {
    signal todo_added(string);
    property <[TodoItem]> todo_model;

    GridLayout {
        Row {
            text_edit := LineEdit {
                accepted(text) => { todo_added(text); }
            }
            Button {
                text: "Add Todo";
                clicked => {
                    todo_added(text_edit.text);
                }
            }
        }
        list_view := ListView {
            rowspan: 2;
            row: 2;
            for todo in todo_model: Rectangle {
                height: 20px;
                GridLayout {
                    CheckBox {
                        text: todo.title;
                        checked: todo.checked;
                        toggled => {
                            todo.checked = checked;
                        }
                    }
                }
            }
        }
    }
}
```

We can compile this code using the `sixtyfps_compiler` binary:

```sh
sixtyfps_compiler my_application_ui.60 > my_application_ui.h
```

Note: You would usually not type this command yourself, this is done automatically by the build system.
(that's what the `sixtyfps_target_60_sources` cmake function does)


This will generate a `my_application_ui.h` header file. It basically contains the following code
(edited for briefty)

```C++
#include <sixtyfps>

struct TodoItem {
    bool checked;
    sixtyfps::SharedString title;
};

struct MainWindow {
 public:
    inline auto get_todo_model () -> std::shared_ptr<sixtyfps::Model<TodoItem>>;
    inline void set_todo_model (const std::shared_ptr<sixtyfps::Model<TodoItem>> &value);

    inline void emit_todo_added (sixtyfps::SharedString arg_0);
    template<typename Functor> inline void on_todo_added (Functor && signal_handler);

    //...
}
```

We can then use this from out .cpp file

```C++
// include the generated file
#include "my_application_ui.h"

int main() {
    // Let's instantiate our window
    auto todo_app = std::make_unique<MainWindow>();

    // let's create a model:
    auto todo_model = std::make_shared<sixtyfps::VectorModel<TodoItem>>(std::vector {
        TodoItem { false, "Write documentation" },
    });
    // set the model as the model of our view
    todo_app->set_todo_model(todo_model);

    // let's connect our "add" button to add an item in the model
    todo_app->on_todo_added([todo_model](const sixtyfps::SharedString &s) {
         todo_model->push_back(TodoItem { false, s} );
    });

    // Show the window and run the event loop
    todo_app->run();
}
```

That's it.

Check the rest of the documentation for the reference.
