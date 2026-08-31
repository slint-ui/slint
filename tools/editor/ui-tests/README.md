# Visual Editor UI Tests

These tests use the private Python `slint-testing` package to run the visual editor and control it out of process.

The suite covers visual editor startup, source lifecycle, navigation,
selection, deletion, palette, outline, canvas, and inspector behaviors as
independent pytest cases. Cases that depend on editor functionality not yet
available are kept collected with explicit skips.

Each behavior starts from a fresh source fixture and editor process.
This prevents one failed interaction from affecting later behavior checks.

## Set Up the Environment

Set `SLINT_TESTING_TOKEN` to the access token from your Slint license.
Then install the test dependencies without storing the token in this repository or `uv.lock`:

```sh
cd tools/editor/ui-tests
UV_INDEX="slint-private=https://testing.slint.dev/simple/" \
UV_INDEX_SLINT_PRIVATE_USERNAME=__token__ \
UV_INDEX_SLINT_PRIVATE_PASSWORD="$SLINT_TESTING_TOKEN" \
uv sync --locked
```

## Build and Run

Build the editor with debug information, the system-testing transport, and the
feature that provides the headless Skia backend:

```sh
SLINT_ENABLE_EXPERIMENTAL_FEATURES=1 \
SLINT_EMIT_DEBUG_INFO=1 \
cargo build -p slint-editor \
    --features system-testing,slint/mcp
```

Run the tests:

```sh
cd tools/editor/ui-tests
./run-tests.sh
```

The runner uses up to four workers and lets pytest report the result and total duration.
The tests use the headless Skia backend by default, so editor windows do not appear locally or in CI.

## Watch the Tests on a Desktop

Run the suite with native windows to see each interaction:

```sh
./run-tests.sh --visible
```

Visible mode uses the `winit-skia` backend and runs serially so test windows don't overlap.
Pass a normal pytest selection after `--visible` to watch specific cases:

```sh
./run-tests.sh --visible \
    tests/test_canvas.py::test_rotation_crosses_zero_with_exact_source
```

Set `SLINT_EDITOR_BINARY` to test a different editor binary.

## Rust-Dependent Cases

The suite retains skipped cases for behavior that needs changes in the Rust
preview implementation:

- resizing elements that have their own rotation
- moving rotated elements, including a rotated child under rotated ancestors
- persisting moves of selected Rectangle previews
- moving selected Text elements beyond the artboard bounds
- canceling transient preview overrides when the pointer exits
- inserting palette elements and exposing valid drop markers through the Rust drop path
- rejecting descendant cycles and component-root outline drops
- recovering a deleted root source file without relaunching
- editing a component-root property from the inspector
- switching shadow families as one atomic property edit
- changing both shadow-offset properties atomically through the angle control

Remove a skip when its Rust implementation lands.
