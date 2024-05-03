<!-- Copyright © SixtyFPS GmbH <info@slint.dev> ; SPDX-License-Identifier: MIT -->

# Running In A Browser Using WebAssembly

:::{warning}
Only Rust supports using Slint with WebAssembly.
:::

If you're using Rust, the tutorial so far used `cargo run` to build and run the code as a native application.
Native applications are the primary target of the Slint framework, but it also supports WebAssembly
for demonstration purposes. This section uses the standard rust tool `wasm-bindgen` and
`wasm-pack` to run the game in the browser. Read the [wasm-bindgen documentation](https://rustwasm.github.io/docs/wasm-bindgen/examples/without-a-bundler.html)
for more about using wasm and rust.

Install `wasm-pack` using cargo:

```sh
cargo install wasm-pack
```

Edit the `Cargo.toml` file to add the dependencies.

```toml
[target.'cfg(target_arch = "wasm32")'.dependencies]
wasm-bindgen = { version = "0.2" }
getrandom = { version = "0.2.2", features = ["js"] }
```

`'cfg(target_arch = "wasm32")'` ensures that these dependencies are only active
when compiling for the wasm32 architecture. Note that the `rand` dependency is now duplicated,
to enable the `"wasm-bindgen"` feature.

While you are editing the `Cargo.toml`, make one last change. To turn the binary into
a library by adding the following:

```toml
[lib]
path = "src/main.rs"
crate-type = ["cdylib"]
```

This is because wasm-pack requires Rust to generate a `"cdylib"`.

You also need to change `main.rs` by adding the `wasm_bindgen(start)`
attribute to the `main` function and export it with the `pub` keyword:

```rust,noplayground
#[cfg_attr(target_arch = "wasm32",
           wasm_bindgen::prelude::wasm_bindgen(start))]
pub fn main() {
    //...
}
```

Compile the program with `wasm-pack build --release --target web`. This
creates a `pkg` directory containing several files, including a `.js` file
named after the program name that you need to import into an HTML file.

Create a minimal `index.html` in the top level of the project that declares a `<canvas>` element for rendering and loads the generated wasm
file. The Slint runtime expects the `<canvas>` element to have the id `id = "canvas"`.
(Replace `memory.js` with the correct file name).

```html
<html>
    <body>
        <!-- canvas required by the Slint runtime -->
        <canvas id="canvas"></canvas>
        <script type="module">
            // import the generated file.
            import init from "./pkg/memory.js";
            init();
        </script>
    </body>
</html>
```

Unfortunately, loading ES modules isn't allowed for files on the file system when accessed from a
`file://` URL, so you can't load the `index.html`. Instead, you need to serve it through a web server.
For example, using Python, by running:

```sh
python3 -m http.server
```

Now you can access the game at [http://localhost:8000](http://localhost:8000/).
