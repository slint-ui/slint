<!-- Copyright © SixtyFPS GmbH <info@slint.dev> -->
<!-- SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0 -->

# Visual Editor Component Gallery

Browse the editor's production components with interactive, resettable fixtures.
The gallery runs independently of projects, the preview server, remote devices, and the updater.

From the repository root:

```sh
cargo run -p slint-editor --features gallery --bin slint-editor-gallery
```

Open a specific example:

```sh
cargo run -p slint-editor --features gallery --bin slint-editor-gallery -- \
  --page picker --scenario "Hard edge" --theme dark
```

Use `--list` to list pages and their scenarios.
Use `--width`, `--height`, and `--scale-factor` to reproduce window sizes and display scaling.
Choose System, Light, or Dark in the header.
The sidebar searches page names; each playground has named scenarios and a reset button.
Preview dimensions are logical pixels, with zero filling the available space.
Expand Preview hides the navigation and options to give large panels more room.

Palette drops, outline changes, inspector edits, and canvas gestures update an in-memory scene.
The fill picker uses the production fill session, gradient math, and recent-fill handling.
File operations, project opening, running, and update actions stay local and appear in the event log.
Reset and page changes discard pending edits and restore the example.
The gallery demonstrates component behavior; editor integration tests cover actual source changes.

## Headless Tests

Build the application binaries with the testing transport:

```sh
cargo build --locked -p slint-editor --all-features --features slint/mcp
```

Run the Rust and UI tests without opening windows:

```sh
SLINT_BACKEND=headless-skia cargo test --locked -p slint-editor --all-features --features slint/mcp
cd tools/editor/ui-tests
SLINT_EDITOR_UI_TEST_BACKEND=headless-skia ./run-tests.sh tests/test_gallery.py
```

For a separate Cargo target directory, set `SLINT_EDITOR_BINARY` and `SLINT_GALLERY_BINARY` to the absolute binary paths.
Set `SLINT_GALLERY_SCREENSHOT_DIR` to retain scenario renders in a chosen directory.
The gallery tests always select `headless-skia`, including when other editor tests use a visible backend.

## Adding an Example

Register its ID, category, title, description, and scenarios in `catalog.rs`.
Import the existing editor component into a gallery page and wire its callbacks in `controller.rs`.
Keep sample hierarchy and editing state in `model.rs`.
Share pure component helpers through `component_support`; keep project and service behavior in the editor.
Add the page ID to the gallery render test and cover any new fixture interaction.
