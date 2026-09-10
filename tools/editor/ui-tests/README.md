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

## Review Radial Canvas Editing

From the repository root, open the centered 200 × 200 Rectangle fixture:

```sh
SLINT_LIVE_PREVIEW=1 SLINT_BACKEND=winit-skia \
cargo run --release -p slint-editor --features slint/live-preview -- \
    tools/editor/ui-tests/fixtures/radial-gradient.slint
```

Select the Rectangle and open its background color picker.
Keep the existing gradient ramp and stop list visible beside the canvas.
Drag the center or guide to translate the gradient, and drag the outer endpoint to change its radius.
Rotating the guide without changing its length doesn't change the circular gradient or rewrite the source.

Double-click the guide to add a stop.
Drag a pointed stop marker along the guide, or focus it and use arrow keys, Delete, or Backspace.
Shift increases keyboard steps tenfold; at least two stops remain.
Escape cancels the current drag first, or cancels the whole session when no drag is active.

The radial canvas and session tests use the headless backend with the rest of the suite.
They cover rotated rectangles, picker synchronization, geometry, stop identity, cancellation, source reload, and history.

## Review Conic Canvas Editing

Open `tools/editor/ui-tests/fixtures/conic-gradient.slint` with the same release command.
Select the Rectangle and open its background color picker.
The existing picker stays beside the circular canvas guide.

Drag the center to translate the gradient.
Drag the white ray or its outer endpoint to rotate it, including across the zero-degree seam.
The circle's radius is an editing aid, not a brush property.
Drag pointed stop markers around the circle; their tips stay on the edited position.
The zero-degree stop sits outside the circle, and the 360-degree stop sits inside it.

Double-click the circle to insert a stop.
Focus a stop and press Delete or Backspace to remove it; at least two stops remain.
Arrow keys move the center by one logical pixel, or rotate the ray and stops by one degree.
Shift multiplies these steps by ten.
Escape restores an active gesture, then cancels the session on a second press.
Closing accepts one edit in undo history; opening and closing unchanged leaves source untouched.

Conic tests use the headless backend with the rest of the suite.
They cover seam crossing, rotated rectangles, insertion, deletion, keyboard input, picker synchronization, cancellation, external edits, and history.

## Verify Shared Stop Editing

Run the gradient suites with four headless workers:

```sh
SLINT_EDITOR_UI_TEST_BACKEND=headless-skia ./run-tests.sh -n 4 --dist=worksteal \
    tests/test_gradient_geometry.py tests/test_linear_gradient_canvas.py \
    tests/test_radial_gradient_canvas.py tests/test_radial_gradient_session.py \
    tests/test_conic_gradient_canvas.py
```

The tests create independent scenes; editing the demonstration fixtures doesn't change their expected geometry.
Crossing tests send several pointer moves before release, including reversals across neighboring stops.
A single pointer move doesn't expose loss of capture when a preview replaces the stop model.

Picker rows follow position order, while selection and canvas handles refer to stable session slots during movement.
The generated source and rendered brush use sorted copies.
Check duplicate positions, color identity, focused deletion, and row ordering when changing this mapping.
Text-gradient tests cover numeric geometry controls without canvas handles.

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
