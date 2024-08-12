<!-- Copyright © SixtyFPS GmbH <info@slint.dev> ; SPDX-License-Identifier: MIT -->

# Examples

These examples demonstrate the main features of Slint and how to use them in different language environments.

| Thumbnail | Description | Demo | 
| --- | --- | --- | 
| ![Printer demo image](https://github.com/user-attachments/assets/7e7400ad-283a-4404-b04a-8620ba4df452 "Printer demo image") |  A fictional user interface for the touch screen of a printer. [Project...](./printerdemo) | [Wasm Demo](https://slint.dev/snapshots/master/demos/printerdemo/) |
| ![Gallery demo image](https://github.com/user-attachments/assets/e37ad016-475a-4c01-8d1b-1326ee7aa733 "Gallery demo image") |  A simple application showing the different widgets. [Project...](./gallery) | [Wasm Demo](https://slint.dev/snapshots/master/demos/gallery/) |
| ![Energy meter demo image](https://github.com/user-attachments/assets/abfe03e3-ded6-4ddc-82b7-8303ee45515c "Energy meter demo image") |  A fictional user interface of a device that monitors energy consumption in a building. [Project...](./energy-monitor) | [Wasm Demo](https://slint.dev/snapshots/master/demos/energy-monitor/) |
| ![Todo demo image](https://github.com/user-attachments/assets/e534736b-3f64-4631-8b9a-80ccd985e9de "Todo demo image") |  A simple todo application. [Project...](./todo)<br><br>A simple todo application based on the [Model View Controller](https://en.wikipedia.org/wiki/Model%E2%80%93view%E2%80%93controller) pattern. [Project...](./todo-mvc) | [Wasm Demo](https://slint.dev/snapshots/master/demos/todo/)<br><br>[Wasm MVC Demo](https://slint.dev/snapshots/master/demos/todo-mvc/)   |
| ![Carousel demo image](https://user-images.githubusercontent.com/6715107/196679740-840a4b67-afaa-4d47-9a31-bfe643c7de48.png "Carousel demo image") |  A custom carousel widget that can be controlled by touch, mouse and keyboard. [Project...](./carousel) | [Wasm Demo](https://slint.dev/snapshots/master/demos/carousel/) |



### [`slide_puzzle`](./slide_puzzle)

Puzzle game based on a Flutter example. See [Readme](./slide_puzzle)

| `.slint` Design | Rust Source | Online wasm Preview | Open in SlintPad |
| --- | --- | --- | --- |
| [`slide_puzzle.slint`](./slide_puzzle/slide_puzzle.slint) | [`main.rs`](./slide_puzzle/main.rs) | [Online simulation](https://slint.dev/snapshots/master/demos/slide_puzzle/) | [Preview in Online Code Editor](https://slint.dev/snapshots/master/editor?load_url=https://raw.githubusercontent.com/slint-ui/slint/master/examples/slide_puzzle/slide_puzzle.slint) |

![Screenshot of the Slide Puzzle](https://slint.dev/resources/puzzle_screenshot.png "Slide Puzzle")

### [`memory`](./memory)

A basic memory game used as an example the tutorial:

* [Memory Game Tutorial (Rust)](https://slint.dev/docs/quickstart/rust)
* [Memory Game Tutorial (C++)](https://slint.dev/docs/quickstart/cpp)

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

| `.slint` Design |  Rust Source | TypeScript Source | Online wasm Preview |
| --- | --- | --- | --- |
| [`main.slint`](./imagefilter/ui/main.slint) | [`main.rs`](./imagefilter/rust/main.rs) | [`main.ts`](./imagefilter/node/main.ts)  | [Online simulation](https://slint.dev/snapshots/master/demos/imagefilter/) |

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

A Rust and C++ example that shows how render a scene with custom OpenGL code into a texture and render that texture within a Slint scene. For more details check out the [Readme](./opengl_texture).

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

### [`maps`](./maps)

A rust example that load image tiles asynchronously from OpenStreetMap server and allow panning and zooming

![Screenshot of the maps example](https://github.com/slint-ui/slint/assets/959326/f5e8cca6-dee1-4681-83da-88fec27f9a45 "Maps example")

### [`virtual keyboard`](./virtual_keyboard)

| `.slint` Design | Rust Source | C++ Source |
| --- | --- | --- |
| [`main_window.slint`](./virtual_keyboard/ui/main_window.slint) | [`main.rs`](./virtual_keyboard/rust/main.rs) | [`main.cpp`](./virtual_keyboard/cpp/main.cpp) |

A Rust and C++ example that shows how to implement a custom virtual keyboard in Slint. For more details check out the [Readme](./virtual_keyboard).

![Screenshot of Virtual Keyboard Example on macOS](https://user-images.githubusercontent.com/6715107/231668373-23faedf8-b42a-401d-b3a2-845d5e61252b.png "Virtual Keyboard")


### [`7guis`](./7guis)

Our implementations of the ["7GUIs"](https://7guis.github.io/7guis/) Tasks.

![Composition of 7GUIs Screenshots](https://user-images.githubusercontent.com/22800467/169002497-5b90e63b-5717-4290-8ac7-c618d9e2a4f1.png "7GUIs")

### [`weather-demo`](./weather-demo)

A simple, cross-platform (Desktop, Android, Wasm) weather application using real weather data from the [OpenWeather](https://openweathermap.org/) API.

| `.slint` Design | Rust Source (Desktop) | Rust Source (Android / Wasm) | Online wasm Preview | Open in SlintPad |
| --- | --- | --- | --- | --- |
| [`main.slint`](./weather-demo/ui/main.slint) | [`main.rs`](./weather-demo/src/main.rs) | [`lib.rs`](./weather-demo/src/lib.rs) | [Online simulation](https://slint.dev/snapshots/master/demos/weather-demo/) | [Preview in Online Code Editor](https://slint.dev/snapshots/master/editor?load_url=https://raw.githubusercontent.com/slint-ui/slint/master/examples/weather-demo/ui/main.slint) |

![Screenshot of the Weather Demo Desktop](./weather-demo/docs/img/desktop-preview.png "Weather Demo Desktop")

![Screenshot of the Weather Demo Desktop](./weather-demo/docs/img/android-preview.png "Weather Demo Android")

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
pushd api/node
npm install
npm run compile
popd
cd examples/printerdemo/node
npm install
npm start
```
