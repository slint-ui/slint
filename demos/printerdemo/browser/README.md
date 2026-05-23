# Slint Printer Demo — Browser

Browser port of `demos/printerdemo/node/main.js`, running against
`slint-wasm-interpreter` instead of the Node.js NAPI bindings.

## Running

```sh
# 1. Build the WASM interpreter (~40 s on first build, then incremental)
cd ../../../api/wasm-interpreter
pnpm build:wasm
pnpm compile

# 2. Serve and open
cd ../../demos/printerdemo/browser
pnpm dev
# open http://localhost:5173/
```

## What's exercised

- `slint.loadSource(source, baseUrl, { fileLoader })` — compile inline source with imports resolved by `fetch`.
- `slint.ArrayModel` — JS-backed model passed into the Slint side via `setProperty`; mutations via `push`/`remove`/`setRowData` propagate to the UI.
- Brush parsing from CSS strings (`"#00ffff"`).
- Global property/callback access (`appWindow.PrinterQueue.start_job = …`).
- `appWindow.run()` — awaits until `slint.quitEventLoop()` is called.

## Known caveats

- Translations (`initTranslations`) are not wired in the browser bundle.
- Image / font asset loading from `.slint` (`@image-url(…)`, `import "./fonts/…"`) is not yet handled by the wasm interpreter's `fileLoader`; missing assets render blank or fall back to the default font.
