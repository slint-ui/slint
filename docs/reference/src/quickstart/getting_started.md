<!-- Copyright © SixtyFPS GmbH <info@slint.dev> ; SPDX-License-Identifier: MIT -->

# Getting started

This tutorial shows you how to use the languages that Slint supports as the host programming language.

We recommend using [our editor integrations for Slint](https://github.com/slint-ui/slint/tree/master/editors) for following this tutorial.

Slint has application templates you can use to create a project with dependencies already set up that follows recommended best practices.

## Prerequisites

:::::{tab-set}

::::{tab-item} C++
:sync: cpp

Before using the template, you need a C++ compiler that supports C++ 20 and to install [CMake](https://cmake.org/download/) 3.21 or newer.

Clone or download the template repository:

```sh
git clone https://github.com/slint-ui/slint-cpp-template memory
cd memory
```

### Configure the project

The `CMakeLists.txt` uses the line `add_executable(my_application src/main.cpp)` to set `src/main.cpp` as the main C++ code file.

Replace the content of `src/main.cpp` with the following:

:::{literalinclude} main_initial.cpp
:lines: 9-13
:::

Also in `CMakeLists.txt` the line
`slint_target_sources(my_application ui/appwindow.slint)` is a Slint function used to
add the `appwindow.slint` file to the target.

Replace the contents of `ui/appwindow.slint` with the following:

:::{literalinclude} appwindow.slint
:language: slint,no-preview
:lines: 6-11
:::

Configure with CMake:

```sh
cmake -B build
```

:::{tip}
When configuring with CMake, the FetchContent module fetches the source code of Slint via git.
This may take some time when building for the first time, as the process needs to build
the Slint runtime and compiler.
:::

Build with CMake:

```sh
cmake --build build
```

### Run the application

Run the application binary on Linux or macOS:

```sh
./build/my_application
```

Or on Windows:

```sh
build\my_application.exe
```

This opens a window with a green "Hello World" greeting.

If you are stepping through this tutorial on a Windows machine, you can run the application at each step with:

```sh
my_application
```

::::

::::{tab-item} NodeJS
:sync: nodejs

Clone or download the template repository:

```sh
git clone https://github.com/slint-ui/slint-nodejs-template memory
cd memory
```

Install dependencies with npm:

```sh
npm install
```

### Configure the project

The `package.json` file references `src/main.js` as the entry point for the application and `src/main.js` references `memory.slint` as the UI file.

Replace the contents of `src/main.js` with the following:

:::{literalinclude} main_initial.js
:lines: 6-10
:::

The `slint.loadFile` method resolves files from the process's current working directory, so from the `package.json` file's location.

Replace the contents of `ui/appwindow.slint` with the following:

:::{literalinclude} memory.slint
:language: slint,no-preview
:lines: 6-11
:::

### Run the application

Run the example with `npm start` and a window appears with the green "Hello World" greeting.

::::

::::{tab-item} Rust
:sync: rust
:selected: true

We recommend using [rust-analyzer](https://rust-analyzer.github.io) and [our editor integrations for Slint](https://github.com/slint-ui/slint/tree/master/editors) for following this tutorial.

Install the [template](https://github.com/slint-ui/slint-rust-template) with the following commands:

```sh
cargo install cargo-generate
cargo generate --git https://github.com/slint-ui/slint-rust-template --name memory
cd memory
```

### Configure the project

Replace the contents of `src/main.rs` with the following:

```rust
slint::include_modules!();

fn main() -> Result<(), slint::PlatformError> {
    let main_window = MainWindow::new()?;

    main_window.run()
}
```

Replace the contents of `ui/appwindow.slint` with the following:

:::{literalinclude} memory.slint
:language: slint,no-preview
:lines: 6-11
:::

### Run the application

Run the example with `cargo run` and a window appears with the green "Hello World" greeting.

::::

:::::

![Screenshot of initial tutorial app showing Hello World](https://slint.dev/blog/memory-game-tutorial/getting-started.png "Hello World")
