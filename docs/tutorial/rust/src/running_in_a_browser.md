<!-- Copyright © SixtyFPS GmbH <info@slint.dev> ; SPDX-License-Identifier: MIT -->
# Running In A Browser Using WebAssembly

Right now, we used `cargo run` to build and run our program as a native application.
Native applications are the primary target of the Slint framework, but we also support WebAssembly
for demonstration purposes. So in this section we'll use the standard rust tool `wasm-bindgen` and
`wasm-pack` to run the game in the browser. The [wasm-bindgen](https://rustwasm.github.io/docs/wasm-bindgen/examples/without-a-bundler.html)
documentation explains all you need to know about using wasm and rust.

Make sure to have `wasm-pack` installed using

```sh
cargo install wasm-pack
```

You'll need to edit your `Cargo.toml` to add the dependencies.

```toml
[target.'cfg(target_arch = "wasm32")'.dependencies]
wasm-bindgen = { version = "0.2" }
getrandom = { version = "0.2.2", features = ["js"] }
```

The `'cfg(target_arch = "wasm32")'` ensures that these dependencies will only be active
when compiling for the wasm32 architecture. Note that the `rand` dependency is now duplicated,
in order to enable the `"wasm-bindgen"` feature.

While you are editing the Cargo.toml, one last change is needed: you need to turn the binary into
a library by adding the following:

```toml
[lib]
path = "src/main.rs"
crate-type = ["cdylib"]
```

This is required because wasm-pack require rust to generate a `"cdylib"`.

You also need to modify the `main.rs` by adding the `wasm_bindgen(start)`
attribute to the main function and export it with the `pub` keyword:

```rust,noplayground
#[cfg_attr(target_arch = "wasm32",
           wasm_bindgen::prelude::wasm_bindgen(start))]
pub fn main() {
    //...
}
```

Now, we can compile our program with `wasm-pack build --release --target web`. This
will create a `pkg` directory containing a few files, including a `.js` file
named after your program name. We just have to import that from a HTML file. So let's create a minimal
`index.html` that declares a `<canvas>` element for rendering and loads our generated wasm
file. The Slint runtime expects the `<canvas>` element to have the id `id = "canvas"`.
(Replace `memory.js` by the correct file name).

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
`file://` URL, so we can't simply open the index.html. Instead we need to serve it through a web server.
For example, using Python, it's as simple as running

```sh
python3 -m http.server
```

and then you can now access the game on [http://localhost:8000](http://localhost:8000/).
