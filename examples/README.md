# Examples

These examples demonstrate the main features of SixtyFPS and how to use them in different language environments.


### `printerdemo`

A fictional user interface for the touch screen of a printer

`.60` Design | Rust Source | C++ Source | Node Source | Online wasm Preview  | Open in code editor
--- | --- | --- | --- | --- | ---
[`ui.60`](./printerdemo/ui/printerdemo.60) | [`main.rs`](./printerdemo/rust/main.rs) | [`main.cpp`](./printerdemo/cpp/main.cpp) | [`main.js`](./printerdemo/node/main.js)| [Online simulation](https://sixtyfps.io/demos/printerdemo/) | [Preview in Online Code Editor](https://sixtyfps.io/editor?load_url=https://raw.githubusercontent.com/sixtyfpsui/sixtyfps/master/examples/printerdemo/ui/printerdemo.60)

![Screenshot of the Printer Demo](https://sixtyfps.io/resources/printerdemo_screenshot.png "Printer Demo")


### `gallery`

A simple application showing the different widgets

`.60` Design | Rust Source | C++ Source | Online wasm Preview  | Open in code editor
--- | --- | --- | --- | ---
[`gallery.60`](./gallery/gallery.60) | [`main.rs`](./gallery/main.rs) | [`main.cpp`](./gallery/main.cpp)| [Online simulation](https://sixtyfps.io/demos/gallery/) | [Preview in Online Code Editor](https://sixtyfps.io/editor?load_url=https://raw.githubusercontent.com/sixtyfpsui/sixtyfps/master/examples/gallery/gallery.60)

![Screenshot of the Gallery on Windows](https://sixtyfps.io/resources/gallery_win_screenshot.png "Gallery")

### `todo`

A simple todo mvc application

`.60` Design | Rust Source | C++ Source | Node Source | Online wasm Preview  | Open in code editor
--- | --- | --- | --- | --- | ---
[`todo.60`](./todo/ui/todo.60) | [`main.rs`](./todo/rust/main.rs) | [`main.cpp`](./todo/cpp/main.cpp) | [`main.js`](./todo/node/main.js) | [Online simulation](https://sixtyfps.io/demos/todo/) | [Preview in Online Code Editor](https://sixtyfps.io/editor?load_url=https://raw.githubusercontent.com/sixtyfpsui/sixtyfps/master/examples/todo/ui/todo.60)

### `slide_puzzle`

Puzzle game based on a Flutter example. See [Readme](./slide_puzzle)

`.60` Design | Rust Source | C++ Source | Node Source | Online wasm Preview  | Open in code editor
--- | --- | --- | --- | --- | ---
[`slide_puzzle.60`](./slide_puzzle/slide_puzzle.60) | [`main.rs`](./todo/rust/main.rs) | ❌ | ❌ | [Online simulation](https://sixtyfps.io/demos/slide_puzzle/) | [Preview in Online Code Editor](https://sixtyfps.io/editor?load_url=https://raw.githubusercontent.com/sixtyfpsui/sixtyfps/master/examples/slide_puzzle/slide_puzzle.60)

![Screenshot of the Slide Puzzle](https://sixtyfps.io/resources/puzzle_screenshot.png "Slide Puzzle")

## Loading the example with the `viewer`

Simply load the .60 file with the viewer application

```sh
cargo run --release --bin viewer -- examples/printerdemo/ui/printerdemo.60
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

## Running the C++ Examples

 * **When compiling SixtyFPS from sources:** If you follow the [C++ build instructions](/docs/building.md#c-build), this will build the C++
examples as well by default

 * **From [installed binary packages](/api/sixtyfps-cpp/README.md#binary-packages):** Simply run cmake in one of the example directory containing a CMakeLists.txt

 ```sh
 mkdir build && cd build
 cmake -DCMAKE_PREFIX_PATH="<path to installed>" ..
 cmake --build .
 ```

## Running the Node Examples

You can run the examples by going into the node sub-folder and use `npm`, for example:

```sh
cd examples/printerdemo/node
npm install
npm start
```
