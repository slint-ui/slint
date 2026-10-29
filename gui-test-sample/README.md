# MCP Measurement Fixtures

These fixtures exercise the changes proposed in [issue #14546](https://github.com/slint-ui/slint/issues/14546).
They were used for the snapshot-size and scrolling measurements.

- `app_testable.slint`: a counter, text input, selectable cards and a short virtualized todo list.
- `large-list.slint`: 1,000 virtualized buttons and a popup.
  Clicking a row updates the `@testable` property `selected-row`.
- `mcp_runner.rs`: the interpreter runner used to open either fixture at 400×500 logical pixels.

## Run a Fixture

Run these commands from the repository root.
Copy the runner into the interpreter's examples directory so Cargo can build it with the repository's dependencies and lockfile:

```sh
mkdir -p internal/interpreter/examples
cp gui-test-sample/mcp_runner.rs internal/interpreter/examples/mcp_measure.rs
cargo build --release --locked -p slint-interpreter --example mcp_measure --features mcp
SLINT_BACKEND=headless-software SLINT_SCALE_FACTOR=1 SLINT_STYLE=fluent \
    SLINT_ENABLE_EXPERIMENTAL_FEATURES=testable SLINT_EMIT_DEBUG_INFO=1 \
    SLINT_MCP_PORT=8080 \
    target/release/examples/mcp_measure gui-test-sample/app_testable.slint
```

For the large-list fixture, replace the final argument with `gui-test-sample/large-list.slint`.
Stop the first process before starting another on the same port.
The server accepts JSON-RPC POST requests at `http://127.0.0.1:8080/mcp`.
The copied `internal/interpreter/examples/mcp_measure.rs` is temporary and can be removed after building.

## Repeat the Checks

Wait until consecutive `get_element_snapshot` results are stable before collecting sizes.
Use `list_windows` and `get_window_properties` to obtain the root element handle.
Compare `get_element_snapshot` and `get_element_tree` for that same root, with `maxElements: 1000` for the tree.
Check `truncated` and measure the UTF-8 bytes of each returned text block, excluding the JSON-RPC envelope.
A no-argument snapshot also includes a window header, so its size differs from the root-scoped result.

For the sample, query the button labeled `Increment`, then click it with `snapshot: true`.
The returned snapshot should show the changed counter and label.

For the list, query the button labeled `Row 999` before scrolling; it should have no matches.
Repeat the query with `scrollIntoView: true`, click the returned handle, and read the root's `selected-row` property to verify `999`.
To check popup inspection, query and click `Open popup` with `snapshot: true`.
The appended snapshot should contain `Popup content`.

## Measure the Editor

Build the editor with debug metadata and open the same sample:

```sh
SLINT_EMIT_DEBUG_INFO=1 cargo build --release --locked -p slint-editor --features slint/mcp
SLINT_BACKEND=headless-software SLINT_SCALE_FACTOR=1 SLINT_STYLE=fluent \
    SLINT_ENABLE_EXPERIMENTAL_FEATURES=testable SLINT_EMIT_DEBUG_INFO=1 \
    SLINT_MCP_PORT=8080 \
    target/release/slint-editor gui-test-sample/app_testable.slint
```

The original editor measurement used a 1360×860 window.
Its snapshot includes the project file tree, so directory contents can change the result size.
The original checkout also contained `app.slint` and generated-code directories beside the sample.
Record the window size and root handles, and account for duplicate roots before comparing totals.
