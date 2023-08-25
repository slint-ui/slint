<!-- Copyright © SixtyFPS GmbH <info@slint.dev> ; SPDX-License-Identifier: MIT -->

# Examples

These examples demonstrate the main features of Slint and how to use them in different language environments.

### [`printerdemo`](./printerdemo)

A fictional user interface for the touch screen of a printer

| `.slint` Design | Rust Source | C++ Source | Node Source | Online wasm Preview | Open in SlintPad |
| --- | --- | --- | --- | --- | --- |
| [`ui.slint`](./printerdemo/ui/printerdemo.slint) | [`main.rs`](./printerdemo/rust/main.rs) | [`main.cpp`](./printerdemo/cpp/main.cpp) | [`main.js`](./printerdemo/node/main.js) | [Online simulation](https://slint.dev/snapshots/master/demos/printerdemo/) | [Preview in Online Code Editor](https://slint.dev/snapshots/master/editor?load_url=https://raw.githubusercontent.com/slint-ui/slint/master/examples/printerdemo/ui/printerdemo.slint) |

![Screenshot of the Printer Demo](https://slint.dev/resources/printerdemo_screenshot.png "Printer Demo")

### [`gallery`](./gallery)

A simple application showing the different widgets

| `.slint` Design | Rust Source | C++ Source | Online wasm Preview | Open in SlintPad |
| --- | --- | --- | --- | --- |
| [`gallery.slint`](./gallery/gallery.slint) | [`main.rs`](./gallery/main.rs) | [`main.cpp`](./gallery/main.cpp) | [Online simulation](https://slint.dev/snapshots/master/demos/gallery/) | [Preview in Online Code Editor](https://slint.dev/snapshots/master/editor?load_url=https://raw.githubusercontent.com/slint-ui/slint/master/examples/gallery/gallery.slint) |

![Screenshot of the Gallery on Windows](https://slint.dev/resources/gallery_win_screenshot.png "Gallery")

### [`energy-monitor`](./energy-monitor)

A fictional user interface of a device that monitors energy consumption in a building.

| `.slint` Design | Rust Source | Online wasm Preview | Open in SlintPad |
| --- | --- | --- | --- |
| [`desktop_window.slint`](./energy-monitor/ui/desktop_window.slint) | [`main.rs`](./energy-monitor/src/main.rs) | [Online simulation](https://slint.dev/snapshots/master/demos/energy-monitor/) | [Preview in Online Code Editor](https://slint.dev/snapshots/master/editor?load_url=https://raw.githubusercontent.com/slint-ui/slint/master/examples/energy-monitor/ui/desktop_window.slint) |

![Screenshot of the Energy-Monitor Demo](https://slint.dev/resources/energy-monitor-screenshot.png "Energy Monitor")

### [`todo`](./todo)

A simple todo mvc application

| `.slint` Design | Rust Source | C++ Source | NodeJS | Online wasm Preview | Open in SlintPad |
| --- | --- | --- | --- | --- | --- |
| [`todo.slint`](./todo/ui/todo.slint) | [`main.rs`](./todo/rust/main.rs) | [`main.cpp`](./todo/cpp/main.cpp) | [`main.js`](./todo/node/main.js) | [Online simulation](https://slint.dev/snapshots/master/demos/todo/) | [Preview in Online Code Editor](https://slint.dev/snapshots/master/editor?load_url=https://raw.githubusercontent.com/slint-ui/slint/master/examples/todo/ui/todo.slint) |

![Screenshot of the Todo Demo](https://slint.dev/resources/todo_screenshot.png "Todo Demo")

### [`carousel`](./carousel)

A custom carousel widget that can be controlled by touch, mouse and keyboard

The example can be run on desktop, wasm and mcu platforms

| `.slint` Design | Rust Source | C++ Source | Node Source | Online wasm Preview | Open in SlintPad |
| --- | --- | --- | --- | --- | --- |
| [`ui.slint`](./carousel/ui/carousel_demo.slint) | [`main.rs`](./carousel/rust/main.rs) | [`main.cpp`](./carousel/cpp/main.cpp) | [`main.js`](./carousel/node/main.js) | [Online simulation](https://slint.dev/snapshots/master/demos/carousel/) | [Preview in Online Code Editor](https://slint.dev/snapshots/master/editor?load_url=https://raw.githubusercontent.com/slint-ui/slint/master/examples/carousel/ui/carousel_demo.slint) |

![Screenshot of the Carousel Demo](https://user-images.githubusercontent.com/6715107/196679740-840a4b67-afaa-4d47-9a31-bfe643c7de48.png "Carousel Demo")

### [`slide_puzzle`](./slide_puzzle)

Puzzle game based on a Flutter example. See [Readme](./slide_puzzle)

| `.slint` Design | Rust Source | Online wasm Preview | Open in SlintPad |
| --- | --- | --- | --- |
| [`slide_puzzle.slint`](./slide_puzzle/slide_puzzle.slint) | [`main.rs`](./slide_puzzle/main.rs) | [Online simulation](https://slint.dev/snapshots/master/demos/slide_puzzle/) | [Preview in Online Code Editor](https://slint.dev/snapshots/master/editor?load_url=https://raw.githubusercontent.com/slint-ui/slint/master/examples/slide_puzzle/slide_puzzle.slint) |

![Screenshot of the Slide Puzzle](https://slint.dev/resources/puzzle_screenshot.png "Slide Puzzle")

### [`memory`](./memory)

A basic memory game used as an example the tutorial:

* [Memory Game Tutorial (Rust)](https://slint.dev/docs/tutorial/rust)
* [Memory Game Tutorial (C++)](https://slint.dev/docs/tutorial/cpp)

| `.slint` Design | Rust Source | C++ Source | Online wasm Preview | Open in SlintPad |
| --- | --- | --- | --- | --- |
| [`memory.slint`](./memory/memory.slint) | [`main.rs`](./memory/main.rs) | [`memory.cpp`](./memory/memory.cpp) | [Online simulation](https://slint.dev/snapshots/master/demos/memory/) | [Preview in Online Code Editor](https://slint.dev/snapshots/master/editor?load_url=https://raw.githubusercontent.com/slint-ui/slint/master/examples/memory/memory.slint) |

### [`iot-dashboard`](./iot-dashboard)

A clone of one demo from the [QSkinny framework](https://qskinny.github.io/).

Also show how a way to dynamically load widgets with the interpreter from C++.

| `.slint` Design | C++ Source | Online wasm Preview | Open in SlintPad |
| --- | --- | --- | --- |
| [`main.slint`](./iot-dashboard/main.slint) | [`main.cpp`](./iot-dashboard/main.cpp)   | [Online preview](https://slint.dev/snapshots/master/editor/preview.html?load_url=https://raw.githubusercontent.com/slint-ui/slint/master/examples/iot-dashboard/main.slint) | [Preview in Online Code Editor](https://slint.dev/snapshots/master/editor?load_url=https://raw.githubusercontent.com/slint-ui/slint/master/examples/iot-dashboard/main.slint) |

![Screenshot of the IOT Dashboard](https://slint.dev/resources/iot-dashboard_screenshot.png "IOT Dashboard")

### [`imagefilter`](./imagefilter)

A Rust-only example that shows how to use the Rust image crate to do image manipulations
and feed the result into Slint.

|  Source | Online wasm Preview |
| --- | --- |
| [`main.rs`](./imagefilter/main.rs)   | [Online simulation](https://slint.dev/snapshots/master/demos/imagefilter/) |

![Screenshot of the imagefilter example](https://slint.dev/resources/imagefilter_screenshot.png "Image Filter")

### [`plotter`](./plotter)

A Rust-only example that shows how to use the Rust plotters crate to do plot a
graph and integrate the result into Slint.

| `.slint` Design | Rust Source | Online wasm Preview |
| --- |  --- | --- |
| [`plotter.slint`](./plotter/plotter.slint) | [`main.rs`](./plotter/main.rs) | [Online simulation](https://slint.dev/snapshots/master/demos/plotter/) |

![Screenshot of the plotter example](https://slint.dev/resources/plotter_screenshot.png "Plotter")

### [`bash`](./bash)

Some examples of how to use the `slint-viewer` to add a GUI to shell scripts.

### [`opengl_underlay`](./opengl_underlay)

A Rust and C++ example that shows how render Slint on top of graphical effect rendered using custom OpenGL code. For more details check out the [Readme](./opengl_underlay).

| `.slint` Design | Rust Source | C++ Source | Online wasm Preview |
| --- | --- | --- | --- |
| [`scene.slint`](./opengl_underlay/scene.slint) | [`main.rs`](./opengl_underlay/main.rs) | [`main.cpp`](./opengl_underlay/main.cpp) | [Online simulation](https://slint.dev/snapshots/master/demos/opengl_underlay/) |

![Screenshot of the OpenGL Underlay Example on Windows](https://slint.dev/resources/opengl_underlay_screenshot.png "OpenGL Underlay")

### [`opengl_texture`](./opengl_texture)

A Rust and C++ example that shows how render a scene with custom OpenGL code intoa texture and render that texture within a Slint scene. For more details check out the [Readme](./opengl_texture).

| `.slint` Design | Rust Source | C++ Source |
| --- | --- | --- |
| [`scene.slint`](./opengl_texture/scene.slint) | [`main.rs`](./opengl_texture/main.rs) | [`main.cpp`](./opengl_texture/main.cpp) |

![Screenshot of the OpenGL Texture Example on macOS](https://github.com/slint-ui/slint/assets/1486/b9f1f6cf-3859-418e-9662-0c7170c3b1f2 "OpenGL Texture")

### [`ffmpeg`](./ffmpeg)

A Rust example that shows how render video frames with FFmpeg within a Slint scene. For more details check out the [Readme](./ffmpeg).

| `.slint` Design | Rust Source |
| --- | --- |
| [`scene.slint`](./ffmpeg/scene.slint) | [`main.rs`](./opengl_texture/main.rs) |

![Screenshot of the FFmpeg Example on macOS](https://github.com/slint-ui/slint/assets/1486/5a1fad32-611a-478e-ab8f-576b4b4bdaf3 "FFmpeg Example")

### [`virtual keyboard`](./virtual_keyboard)

| `.slint` Design | Rust Source | C++ Source |
| --- | --- | --- |
| [`main_window.slint`](./virtual_keyboard/ui/main_window.slint) | [`main.rs`](./virtual_keyboard/rust/main.rs) | [`main.cpp`](./virtual_keyboard/cpp/main.cpp) |

A Rust and C++ example that shows how to implement a custom virtual keyboard in Slint. For more details check out the [Readme](./virtual_keyboard).

![Screenshot of Virtual Keyboard Example on macOS](https://user-images.githubusercontent.com/6715107/231668373-23faedf8-b42a-401d-b3a2-845d5e61252b.png "Virtual Keyboard")


### [`7guis`](./7guis)

Our implementations of the ["7GUIs"](https://7guis.github.io/7guis/) Tasks.

![Composition of 7GUIs Screenshots](https://user-images.githubusercontent.com/22800467/169002497-5b90e63b-5717-4290-8ac7-c618d9e2a4f1.png "7GUIs")

### External examples

* [Cargo UI](https://github.com/slint-ui/cargo-ui): A rust application that makes use of threads in the background.

![Screenshot of Cargo UI](https://raw.githubusercontent.com/slint-ui/cargo-ui/master/screenshots/deptree.png "Cargo UI")

## Loading the example with the `viewer`

Simply load the .slint file with the viewer application

```sh
cargo run --release --bin slint-viewer -- examples/printerdemo/ui/printerdemo.slint
```

## Running the Rust Examples

You can run the examples either by going into the rust sub-folder and use `cargo run`, for example:

```sh
cd examples/printerdemo/rust
cargo run --release
```

or you can run them from anywhere in the Cargo workspace by name:

```sh
cargo run --release --bin printerdemo
```

### Wasm builds

In order to make the wasm build of the example, you first need to edit the Cargo.toml
files to uncomment the line starting with `#wasm#` (or use the `sed` line bellow)
You can then use wasm-pack (which you may need to obtain with `cargo install wasm-pack`).
This will generate the wasm in the `./pkg` directory, which the `index.html` file will open.
Since wasm files cannot be served from `file://` URL, you need to open a wab server to serve
the content

```sh
cd examples/printerdemo/rust
sed -i "s/^#wasm# //" Cargo.toml
wasm-pack build --release --target web
python3 -m http.server
```

## Running the C++ Examples

* **When compiling Slint from sources:** If you follow the [C++ build instructions](/docs/building.md#c-build), this will build the C++
examples as well by default
* **From [installed binary packages](/api/cpp/README.md#binary-packages):** Simply run cmake in one of the example directory containing a CMakeLists.txt

 ```sh
 mkdir build && cd build
 cmake -GNinja -DCMAKE_PREFIX_PATH="<path to installed>" ..
 cmake --build .
 ```

## Running the Node Examples

You can run the examples by going into the node sub-folder and use `npm`, for example:

```sh
cd examples/printerdemo/node
npm install
npm start
```
