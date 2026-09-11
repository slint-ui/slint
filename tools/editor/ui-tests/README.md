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

## Wait for Edits

Use `snapshot.wait_for_applied(expected, relative_path)` before an action that depends on an edit's completed preview and history update.
It checks exact project source, then waits for matching source in the installed preview with no compilation, workspace edit, or history request pending.
For external file writes, use `editor_sync.wait_for_source(path, expected)` to wait for the installed source directly.

`launch_editor` creates a private synchronization directory for each editor process.
The `system-testing` build answers requests on its UI thread through `SLINT_EDITOR_TEST_SYNC`.
Request IDs prevent an earlier response from satisfying a later wait; source comparisons prevent an older compilation from satisfying it.
Timeout failures include the pending-work flag and documents that haven't reached the preview.

Keep `wait_for_exact` for disk-only assertions and tests that deliberately send input while compilation is pending.
Keyboard helpers dispatch immediately; put completion waits at the call sites that need them.
Don't use completion waits for invalid source or transient drag previews, which must not become installed document revisions.

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
