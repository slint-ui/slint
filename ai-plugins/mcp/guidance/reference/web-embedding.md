# Live Previews in a Web Page

The WebAssembly interpreter compiles and runs `.slint` source in a browser, with no host-language logic.

## Get the Interpreter

Two files, kept in one directory: `slint_wasm_interpreter.js` (ES module) and `slint_wasm_interpreter_bg.wasm` (~9 MB).

- Latest release: `https://docs.slint.dev/latest/wasm-interpreter/`
- A pinned release: `https://releases.slint.dev/<version>/wasm-interpreter/`
- Unreleased code: `wasm-pack build --release --target web --out-dir <dir>` in `api/wasm-interpreter`

Both hosts allow any origin.
Under a strict Content Security Policy, such as Claude Artifacts, publish both files next to the page and serve the `.wasm` as `application/wasm`.

## Embed It

```html
<canvas id="preview"></canvas>
<script type="module">
  import init, * as slint from "./slint_wasm_interpreter.js";
  await init();
  slint.run_event_loop(); // once; create() and show() resolve only after it
  let instance = null;

  async function showPreview(source) {
    const { component, diagnostics } = await slint.compile_from_string(source, "main.slint");
    // diagnostics: { level (0 error, 1 warning, 2 note), lineNumber, columnNumber, message }
    if (!component) return;
    instance = instance
      ? await component.create_with_existing_window(instance) // consumes the old instance
      : await component.create("preview");
    await instance.show();
  }
</script>
```

- The canvas takes the root's fixed `width` and `height`, unless CSS sizes it.
  `data-slint-auto-resize-to-preferred="true"` on the canvas follows the preferred size.
- `std-widgets.slint` and a default font are built in.
  Page fonts don't reach the canvas: call `register_font_from_memory(bytes)` before compiling.
- `compile_from_string_with_style(source, url, "fluent")` picks a style.
- After the URL, or the style, both compile functions take optional loaders:
  - imports: `(url) => Promise<string | null>`, with `null` for a missing file
  - fonts: `(url) => Promise<Uint8Array>`
- The interpreter compiles with the [edition](https://docs.slint.dev/latest/docs/slint/guide/language/concepts/editions/) of its own release.
  The next argument of `compile_from_string_with_style`, such as `"1.18"`, selects another one.
- The last argument of both is the URL of the directory with the `slint-project.json` to apply, read through the import loader.
  Its `edition` takes precedence.
- Images don't go through the import loader.
  With a URL as the base, such as `https://…`, `@image-url` paths resolve to URLs that `<img>` elements load.
  A path from the root, such as `/img/logo.png`, resolves on the host of the base URL.
  With a file path as the base, the image doesn't load.

## Pitfalls

- Before 1.19, hiding the last instance ends the event loop, and later calls fail with "The event loop was already terminated".
  Reuse the window with `create_with_existing_window()`.
- A Rust panic leaves the module unusable, and its pending promises never settle.
  The panic hook logs it with `console.error`, starting with "panicked at".
  To recover without a reload, import the module again under a new URL fragment, such as `#2`, and give it a new canvas.
- A panic is a bug in Slint.
  Suggest reporting it at https://github.com/slint-ui/slint/issues with the panic message, the Slint version, and the smallest `.slint` source that reproduces it.
