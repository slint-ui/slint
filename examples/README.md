# Examples

These examples demonstrate the main features of SixtyFPS and how to use them in different language environments.

Demo | `.60` Design | Rust Source | C++ Source | Description
--- | --- | --- | --- | ---
`printerdemo`| [`ui.60`](./printerdemo/ui/printerdemo.60) | [`main.rs`](./printerdemo/rust/main.rs) | [`main.cpp`](./printerdemo/cpp/main.cpp) | A fictional user interface for the touch screen of a printer
`gallery` |[`gallery.60`](./gallery/gallery.60) | [`main.rs`](./gallery/main.rs) | [`main.cpp`](./gallery/main.cpp) | A gallery of different widgets


Example | Source File | `.60` Design | Description
--- | --- | --- | ---
`cpptest` | [`main.cpp`](./cpptest/main.cpp)  | [`hello.60`](./cpptest/hello.60) | A minimal example to show a `.60` design in a window.
`rusttest2` | [`main.rs`](./rusttest2/src/main.rs) | [`hello.60`](./cpptest/hello.60) | A minimal example to show a `.60` design in a window.

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

 * **When compiling SifxtyFPS from sources:** If you follow the [C++ build instructions](/docs/building.md#c-build), this will build the C++
examples as well by default

 * **From [installed binary packages](/api/sixtyfps-cpp/README.md#binary-packages):** Simply run cmake in one of the example directory containing a CMakeLists.txt

 ```sh
 mkdir build && cd build
 cmake -DCMAKE_PREFIX_PATH="<path to installed>" ..
 cmake --build .
 ```

