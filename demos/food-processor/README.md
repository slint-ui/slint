# Food Processor Demo

A fictional touch screen user interface for a food processor, designed for a 512×300 display.
Browse recipes, look at their ingredients and preparation steps, and start a cooking timer.

| `.slint` Design | Rust Source | Online wasm Preview | Open in SlintPad |
| --- | --- | --- | --- |
| [`demo.slint`](./ui/demo.slint) | [`lib.rs`](./rust/lib.rs) | [Online simulation](https://slint.dev/snapshots/master/demos/food-processor/) | [Preview in Online Code Editor](https://slint.dev/snapshots/master/editor?load_url=https://raw.githubusercontent.com/slint-ui/slint/master/demos/food-processor/ui/demo.slint) |

## Running

Run the desktop version:

```sh
cargo run -p food-processor
```

Build the WebAssembly version in the `rust` directory and serve it with any web server:

```sh
wasm-pack build --release --target web
python3 -m http.server
```
