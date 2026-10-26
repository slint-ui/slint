# Food Processor Demo

<!-- cspell:ignore Eiliv Aceron -->

A fictional touch screen user interface for a food processor, designed for a 1024×600 display.
Browse twelve recipes in a vertically scrolling grid, look at their ingredients and preparation steps, and start a cooking timer.
The photographs are by Eiliv Aceron on Unsplash; [REUSE.toml](../../REUSE.toml) records each source and license.

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
