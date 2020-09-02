# SixtyFPS-cpp

**A C++ UI toolkit**

[SixtyFPS](https://www.sixtyfps.io/) is a UI toolkit that supports different programming languages.
SixtyFPS.cpp is the C++ API to interact with a SixtyFPS UI from C++.

The complete C++ documentation can be viewed online at [https://www.sixtyfps.io/docs/cpp/](https://www.sixtyfps.io/docs/cpp/).

## Building

The C++ API comes as a CMake package with a library and header files. The build process consists of the following steps:

 1. Install Rust and Cargo (recommended via https://rustup.rs).
 2. Build the CMake package: `cargo xtask cmake`. If you'd like to install SixtyFPS.cpp into a specific directory, then
       you can pass the `prefix` and `install` parameters: `cargo xtask cmake --prefix /path/where/to/install --install`
 3. When running `CMake` make sure to point the `CMAKE_PREFIX_PATH` to the install directory. If you build SixtyFPS.cpp without `prefix`, then the `target/debug/` directory is your `CMAKE_PREFIX_PATH`.

## Usage via CMake

TODO

## Tutorial

Let's make a UI for a todo list application using the SixtyFPS UI description language.
Hopefully this should be self explainatory. Check out the documentation of the language for help

NOTE: this is not yet implemented as is.

```sixtyfps
// file: todoapp.60
TodoApp := MainWindow {
    signal todo_added(string);
    property<model> todo_model;

    ColumnLayout {
        RowLayout {
            text_edit := LineEdit {}
            Button {
                text: "Add Todo";
                clicked => {
                    todo_added(text_edit.text);
                    text_edit.text = "";
                }
            }
        }
        NativeListView {
            model: todo_model;
        }
    }
}
```

Now, we can generate the C++ code using the following command

```
sixtyfpscpp_compiler todoapp.sixtyfps -o todoapp.h
```

Note: You would usually not type this command yourself, this is done automatically by the build system
See the documentation for how to integrate with cmake

This will generate a todoapp.h header file. It basically contains the following code
(edited for briefty)

```C++
#include <sixtyfps>
struct TodoApp : sixtyfps::window {
    sixtyfps::signal<std::string_view> &todo_added();
    sixtyfps::property<std::shared_ptr<sixtyfps::data_model<
        sixtyfps::native_list_view_item>>> &todo_model();
    //...
}
```

We can then use this from out .cpp file

```C++
// include the generated file
#include "todoapp.h"

int main() {
    // Let's instantiate our window: this return a handle to it
    auto todo_app = sixtyfps::create_window<TodoApp>();

    // let's create a model: `simple_data_model` is a data model which is simply backed by
    // a vector behind the scene.
    auto model = std::make_shared<sixtyfps::simple_data_model<sixtyfps::native_list_view_item>>();
    model->push_back({"Write documentation", sixtyfps::native_list_view_item::checkable });
    todo_app->data_model().set(model);

    // let's connect our "add" button to add an item in the model
    todo_app->todo_added().connect([=](std::string_view data) {
        model->push_back({data, sixtyfps::native_list_view_item::checkable})
    });

    // Show the window
    todo_app->show();

    // Run the sixtyfps envent loop on this thread.
    sixtyfps::run();
}
```

That's it.

Check the rest of the documentation for the reference.

## Types

The types used for properties in `.60` design markup each translate to specific types in C++.
The follow table summarizes the entire mapping:

| `.60` Type | C++ Type | Note |
| --- | --- | --- |
| `int` | `int` | |
| `float` | `float` | |
| `string` | [`sixtyfps::SharedString`](api/structsixtyfps_1_1_shared_string.html) | A reference-counted string type that uses UTF-8 encoding and can be easily converted to a std::string_view or a const char *. |
| `color` | [`sixtyfps::Color`](api/classsixtyfps_1_1_color.html) | |
| `length` | `float` | The unit are physical pixels. |
| `logical_length` | `float` | At run-time, logical lengths are automatically translated to physical pixels using the device pixel ratio. |
| `duration` | `std::int64_t` | At run-time, durations are always represented as signed 64-bit integers with milisecond precision. |