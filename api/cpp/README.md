# Slint-cpp

## A C++ UI toolkit

[Slint](https://slint-ui.com/) is a UI toolkit that supports different programming languages.
Slint.cpp is the C++ API to interact with a Slint UI from C++.

The complete C++ documentation can be viewed online at https://slint-ui.com/docs/cpp/.

If you are new to Slint, you might also consider going through our [Walk-through tutorial](https://slint-ui.com/docs/tutorial/cpp).

## Installing or Building Slint

Slint comes with a CMake integration that automates the compilation step of the `.slint` markup language files and
offers a CMake target for convenient linkage.

*Note*: We recommend using the Ninja generator of CMake for the most efficient build and `.slint` dependency tracking.
You can select the CMake Ninja backend by passing `-GNinja` or setting the `CMAKE_GENERATOR` environment variable to `Ninja`.

### Building from Sources

The recommended and most flexible way to use the C++ API is to build Slint from sources.

First you need to install the prerequisites:

* Install Rust by following the [Rust Getting Started Guide](https://www.rust-lang.org/learn/get-started). Once this is done,
  you should have the ```rustc``` compiler and the ```cargo``` build system installed in your path.
* **[cmake](https://cmake.org/download/)** (3.23 or newer)
* A C++ compiler that supports C++20 (e.g., **MSVC 2019 16.6** on Windows)

You can include Slint in your CMake project using CMake's `FetchContent` feature. Insert the following snippet into your
`CMakeLists.txt` to make CMake download the latest release, compile it and make the CMake integration available:

```cmake
include(FetchContent)
FetchContent_Declare(
    Slint
    GIT_REPOSITORY https://github.com/slint-ui/slint.git
    GIT_TAG release/1
    SOURCE_SUBDIR api/cpp
)
FetchContent_MakeAvailable(Slint)
```

If you prefer to treat Slint as an external CMake package, then you can also build Slint from source like a regular
CMake project, install it into a prefix directory of your choice and use `find_package(Slint)` in your `CMakeLists.txt`.

#### Cross-compiling

It is possible to cross-compile Slint to a different target architecture when building with CMake. In order to complete
that, you need to make sure that your CMake setup is ready for cross-compilation. You can find more information about
how to set this up in the [upstream CMake documentation](https://cmake.org/cmake/help/latest/manual/cmake-toolchains.7.html#cross-compiling).
If you are building against a Yocto SDK, it is sufficient to source the SDK's environment setup file.

Since Slint is implemented using the Rust programming language, you need to determine which Rust target
matches the target architecture that you're compiling to. Please consult the [upstream Rust documentation](https://doc.rust-lang.org/nightly/rustc/platform-support.html) to find the correct target name. Now you need to install the Rust toolchain:

```sh
rustup target add <target-name>
```

Then you're ready to invoke CMake and you need to add `-DRust_CARGO_TARGET=<target name>` to the CMake command line.
This ensures that the Slint library is built for the correct architecture.

For example if you are building against an embedded Linux Yocto SDK targeting an ARM64 board, the following commands
show how to compile:

Install the Rust targe toolchain once:

```sh
rustup target add aarch64-unknown-linux-gnu
```

Set up the environment and build:

```sh
. /path/to/yocto/sdk/environment-setup-cortexa53-crypto-poky-linux
cd slint
mkdir build
cd build
cmake -DRust_CARGO_TARGET=aarch64-unknown-linux-gnu -DCMAKE_INSTALL_PREFIX=/slint/install/path ..
cmake --build .
cmake --install .
```

### Binary Packages

We also provide binary packages of Slint for use with C++, which eliminates the need to have Rust installed in your development environment.

You can download one of our pre-built binaries for Linux or Windows on x86-64 architectures:

1. Open <https://github.com/slint-ui/slint/releases>
2. Click on the latest release
3. From "Assets" download either `slint-cpp-XXX-Linux-x86_64.tar.gz` for a Linux x86-64 archive
   or `slint-cpp-XXX-win64.exe` for a Windows x86-64 installer. ("XXX" refers to the version of the latest release)
4. Uncompress the downloaded archive or run the installer.


After extracting the artifact or running the installer, you can place the `lib` sub-directory into your `CMAKE_PREFIX_PATH` and `find_package(Slint)` should succeed in locating the package.

## Usage via CMake

A typical example looks like this:

```cmake
cmake_minimum_required(VERSION 3.21)
project(my_application LANGUAGES CXX)

# Note: Use find_package(Slint) instead of the following three commands, if you prefer the package
# approach.
include(FetchContent)
FetchContent_Declare(
    Slint
    GIT_REPOSITORY https://github.com/slint-ui/slint.git
    GIT_TAG release/1
    SOURCE_SUBDIR api/cpp
)
FetchContent_MakeAvailable(Slint)

add_executable(my_application main.cpp)
target_link_libraries(my_application PRIVATE Slint::Slint)
slint_target_sources(my_application my_application_ui.slint)
# On Windows, copy the Slint DLL next to the application binary so that it's found.
if (WIN32)
    add_custom_command(TARGET my_application POST_BUILD COMMAND ${CMAKE_COMMAND} -E copy $<TARGET_RUNTIME_DLLS:my_application> $<TARGET_FILE_DIR:my_application> COMMAND_EXPAND_LISTS)
endif()
```

The `slint_target_sources` cmake command allows you to add .slint files to your build. Finally it is
necessary to link your executable or library against the `Slint::Slint` target.

## Tutorial

Let's make a UI for a todo list application using the Slint UI description language.
Hopefully this should be self explanatory. Check out the documentation of the language for help

```slint
// file: my_application_ui.slint
import { CheckBox, Button, ListView, LineEdit } from "std-widgets.slint";

export struct TodoItem {
    title: string,
    checked: bool,
}

export component MainWindow {
    callback todo_added(string);
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
                            todo.checked = self.checked;
                        }
                    }
                }
            }
        }
    }
}
```

We can compile this code using the `slint-compiler` binary:

```sh
slint-compiler my_application_ui.slint > my_application_ui.h
```

Note: You would usually not type this command yourself, this is done automatically by the build system.
(that's what the `slint_target_sources` cmake function does)

This will generate a `my_application_ui.h` header file. It basically contains the following code
(edited for brevity)

```C++
#include <slint.h>

struct TodoItem {
    bool checked;
    slint::SharedString title;
};

struct MainWindow {
 public:
    inline auto create () -> slint::ComponentHandle<MainWindow>;

    inline auto get_todo_model () const -> std::shared_ptr<slint::Model<TodoItem>>;
    inline void set_todo_model (const std::shared_ptr<slint::Model<TodoItem>> &value) const;

    inline void invoke_todo_added (slint::SharedString arg_0) const;
    template<typename Functor> inline void on_todo_added (Functor && callback_handler) const;

    //...
}
```

We can then use this from out .cpp file

```C++
// include the generated file
#include "my_application_ui.h"

int main() {
    // Let's instantiate our window
    auto todo_app = MainWindow::create();

    // let's create a model:
    auto todo_model = std::make_shared<slint::VectorModel<TodoItem>>(std::vector {
        TodoItem { false, "Write documentation" },
    });
    // set the model as the model of our view
    todo_app->set_todo_model(todo_model);

    // let's connect our "add" button to add an item in the model
    todo_app->on_todo_added([todo_model](const slint::SharedString &s) {
         todo_model->push_back(TodoItem { false, s} );
    });

    // Show the window and run the event loop
    todo_app->run();
}
```

That's it.

For more details, check the [Online documentation](https://slint-ui.com/docs/cpp) and the full
  [Walk-through tutorial](https://slint-ui.com/docs/tutorial/cpp).
We also have a [Getting Started Template](https://github.com/slint-ui/slint-cpp-template) repository with
the code of a minimal C++ application using Slint that can be used as a starting point to your program.
