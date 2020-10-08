# SixtyFPS-cpp

**A C++ UI toolkit**

[SixtyFPS](https://www.sixtyfps.io/) is a UI toolkit that supports different programming languages.
SixtyFPS.cpp is the C++ API to interact with a SixtyFPS UI from C++.

The complete C++ documentation can be viewed online at https://www.sixtyfps.io/docs/cpp/.

**Warning: Pre-Alpha**
SixtyFPS is still in the early stages of development: APIs will change and important features are still being developed.

## Installing or Building SixtyFPS

### Building from sources

Follow the [C++ build instructions](/docs/building.md#c-build)

### Binary packages

The CI is building binary packages to use with C++ so that you do not need to install a rust compiler.
These binary can be found by clicking on the last [succesfull build of the master branch]
(https://github.com/sixtyfpsui/sixtyfps/actions?query=workflow%3ACI+is%3Asuccess+branch%3Amaster)
and downloading the `cpp_bin` artifact.

## Usage via CMake

While it should be possible to integrate SixftyFPS with any build system, we are provinding cmake integration.
Once SixtyFPS has been installed, it can simply be found using `find_package`

A typical example looks like this:

```cmake
cmake_minimum_required(VERSION 3.16)
project(my_application LANGUAGES CXX)
find_package(SixtyFPS REQUIRED)

add_executable(my_application main.cpp)
target_link_libraries(my_application SixtyFPS::SixtyFPS)
sixtyfps_target_60_sources(my_application my_application_ui.60)
```

The `sixtyfps_target_60_sources` cmake command allow to add .60 files to your build

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
                height: 20lx;
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
