<!-- Copyright © SixtyFPS GmbH <info@slint.dev> -->
<!-- SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0 -->

# Visual Editor Component Gallery

Browse production components with small, disposable sample values.
The gallery contains foundations, basic controls, inspector controls, the element palette, the fill picker, and the outline.
Palette and outline drops report what happened; they do not edit a document.
The picker changes one swatch, and each control owns its own value.
Reset restores the current example.

```sh
cargo run -p slint-editor --features gallery --bin slint-editor-gallery
cargo run -p slint-editor --features gallery --bin slint-editor-gallery -- \
  --page picker --scenario Linear --theme dark
```

Use `--list` for pages and scenarios, and `--width`, `--height`, and `--scale-factor` for window sizing.
Search and theme controls are in the sidebar.

## Headless Tests

```sh
SLINT_EMIT_DEBUG_INFO=1 cargo build --locked -p slint-editor --all-features --features slint/mcp
SLINT_BACKEND=headless-skia cargo test --locked -p slint-editor --all-features --features slint/mcp
cd tools/editor/ui-tests
SLINT_EDITOR_UI_TEST_BACKEND=headless-skia ./run-tests.sh tests/test_gallery.py
```

For a separate Cargo target directory, set `SLINT_EDITOR_BINARY` and `SLINT_GALLERY_BINARY` to the absolute binary paths.
Set `SLINT_GALLERY_SCREENSHOT_DIR` to retain renders.
Gallery tests always use `headless-skia`.

## Adding an Example

Register the page in `catalog.rs` and instantiate the production component.
Keep values local to the example; use Rust only for callbacks that need host data or shared production helpers.
Do not add document editing, source evaluation, history, or application services.
Cover the component's visible interactions in `test_gallery.py`.
