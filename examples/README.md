# Examples

These examples demonstrate the main features of SixtyFPS and how to use them in different language environments.

### [`printerdemo`](./printerdemo)

A fictional user interface for the touch screen of a printer

| `.slint` Design | Rust Source | C++ Source | Node Source | Online wasm Preview | Open in code editor |
| --- | --- | --- | --- | --- | --- |
| [`ui.slint`](./printerdemo/ui/printerdemo.slint) | [`main.rs`](./printerdemo/rust/main.rs) | [`main.cpp`](./printerdemo/cpp/main.cpp) | [`main.js`](./printerdemo/node/main.js) | [Online simulation](https://sixtyfps.io/snapshots/master/demos/printerdemo/) | [Preview in Online Code Editor](https://sixtyfps.io/snapshots/master/editor?load_url=https://raw.githubusercontent.com/sixtyfpsui/sixtyfps/master/examples/printerdemo/ui/printerdemo.slint) |

![Screenshot of the Printer Demo](https://sixtyfps.io/resources/printerdemo_screenshot.png "Printer Demo")

### [`gallery`](./gallery)

A simple application showing the different widgets

| `.slint` Design | Rust Source | C++ Source | Online wasm Preview | Open in code editor |
| --- | --- | --- | --- | --- |
| [`gallery.slint`](./gallery/gallery.slint) | [`main.rs`](./gallery/main.rs) | [`main.cpp`](./gallery/main.cpp) | [Online simulation](https://sixtyfps.io/snapshots/master/demos/gallery/) | [Preview in Online Code Editor](https://sixtyfps.io/snapshots/master/editor?load_url=https://raw.githubusercontent.com/sixtyfpsui/sixtyfps/master/examples/gallery/gallery.slint) |

![Screenshot of the Gallery on Windows](https://sixtyfps.io/resources/gallery_win_screenshot.png "Gallery")

### [`todo`](./todo)

A simple todo mvc application

| `.slint` Design | Rust Source | C++ Source | NodeJS | Online wasm Preview | Open in code editor |
| --- | --- | --- | --- | --- | --- |
| [`todo.slint`](./todo/ui/todo.slint) | [`main.rs`](./todo/rust/main.rs) | [`main.cpp`](./todo/cpp/main.cpp) | [`main.js`](./todo/node/main.js) | [Online simulation](https://sixtyfps.io/snapshots/master/demos/todo/) | [Preview in Online Code Editor](https://sixtyfps.io/snapshots/master/editor?load_url=https://raw.githubusercontent.com/sixtyfpsui/sixtyfps/master/examples/todo/ui/todo.slint) |

![Screenshot of the Todo Demo](https://sixtyfps.io/resources/todo_screenshot.png "Todo Demo")

### [`slide_puzzle`](./slide_puzzle)

Puzzle game based on a Flutter example. See [Readme](./slide_puzzle)

| `.slint` Design | Rust Source | Online wasm Preview | Open in code editor |
| --- | --- | --- | --- |
| [`slide_puzzle.slint`](./slide_puzzle/slide_puzzle.slint) | [`main.rs`](./todo/rust/main.rs) | [Online simulation](https://sixtyfps.io/snapshots/master/demos/slide_puzzle/) | [Preview in Online Code Editor](https://sixtyfps.io/snapshots/master/editor?load_url=https://raw.githubusercontent.com/sixtyfpsui/sixtyfps/master/examples/slide_puzzle/slide_puzzle.slint) |

![Screenshot of the Slide Puzzle](https://sixtyfps.io/resources/puzzle_screenshot.png "Slide Puzzle")

### [`memory`](./memory)

A basic memory game used as an example the tutorial:

* [Memory Game Tutorial (Rust)](https://sixtyfps.io/docs/tutorial/rust)
* [Memory Game Tutorial (C++)](https://sixtyfps.io/docs/tutorial/cpp)

| `.slint` Design | Rust Source | C++ Source | Online wasm Preview | Open in code editor |
| --- | --- | --- | --- | --- |
| [`memory.slint`](./memory/memory.slint) | [`main.rs`](./memory/main.rs) | [`memory.cpp`](./memory/memory.cpp) | [Online simulation](https://sixtyfps.io/snapshots/master/demos/memory/) | [Preview in Online Code Editor](https://sixtyfps.io/snapshots/master/editor?load_url=https://raw.githubusercontent.com/sixtyfpsui/sixtyfps/master/examples/memory/memory.slint) |

### [`iot-dashboard`](./iot-dashboard)

A clone of one demo from the [QSkinny framework](https://qskinny.github.io/).

Also show how a way to dynamically load widgets with the interpreter from C++.

| `.slint` Design | C++ Source | Online wasm Preview | Open in code editor |
| --- | --- | --- | --- |
| [`main.slint`](./iot-dashboard/main.slint) | [`main.cpp`](./iot-dashboard/main.cpp)   | [Online preview](https://sixtyfps.io/snapshots/master/editor/preview.html?load_url=https://raw.githubusercontent.com/sixtyfpsui/sixtyfps/master/examples/iot-dashboard/main.slint) | [Preview in Online Code Editor](https://sixtyfps.io/snapshots/master/editor?load_url=https://raw.githubusercontent.com/sixtyfpsui/sixtyfps/master/examples/iot-dashboard/main.slint) |

![Screenshot of the IOT Dashboard](https://sixtyfps.io/resources/iot-dashboard_screenshot.png "IOT Dashboard")

### [`imagefilter`](./imagefilter)

A Rust-only example that shows how to use the Rust image crate to do image manipulations
and feed the result into SixtyFPS.

|  Source | Online wasm Preview |
| --- | --- |
| [`main.rs`](./imagefilter/main.rs)   | [Online simulation](https://sixtyfps.io/snapshots/master/demos/imagefilter/) |

![Screenshot of the imagefilter example](https://sixtyfps.io/resources/imagefilter_screenshot.png "Image Filter")

### [`plotter`](./plotter)

A Rust-only example that shows how to use the Rust plotters crate to do plot a
graph and integrate the result into SixtyFPS.

| `.slint` Design | Rust Source | Online wasm Preview |
| --- |  --- | --- |
| [`plotter.slint`](./plotter/plotter.slint) | [`main.rs`](./plotter/main.rs) | [Online simulation](https://sixtyfps.io/snapshots/master/demos/plotter/) |

![Screenshot of the plotter example](https://sixtyfps.io/resources/plotter_screenshot.png "Plotter")

### [`bash`](./bash)

Some examples of how to use the `slint-viewer` to add a GUI to shell scripts.

### External examples

* [Cargo UI](https://github.com/sixtyfpsui/cargo-ui): A rust application that makes use of threads in the background.

![Screenshot of Cargo UI](https://raw.githubusercontent.com/sixtyfpsui/cargo-ui/master/screenshots/deptree.png "Cargo UI")

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

* **When compiling SixtyFPS from sources:** If you follow the [C++ build instructions](/docs/building.md#c-build), this will build the C++
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
