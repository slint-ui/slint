# Test Best Practices

- Default to `headless-skia` for local automated tests.
  Use visible windows only for explicit manual checks or native window behavior.
- Test the real `slint-editor` from the current checkout; rebuild after Rust or editor-shell changes.
- Wait for observable state, never fixed delays: `wait_for_source` before input, `expect` for asserted state, and `SourceSnapshot.wait_for_applied` after edits.
  Bound waits and poll throughout negative-assertion observation periods.
- Reuse helpers in `ui_driver`, `canvas_interactions`, `gradient_interactions`, `inspector_interactions`, and `source_snapshot`.
  Never import another test module or duplicate interaction, synchronization, or coordinate logic.
- Share production validation and history paths for equivalent single-value, color, and batch edits.
  Cover stale targets and rejected edits.
- Use scoped, stable labels or IDs.
  Look elements up with `element()`: each read or action of the returned element runs its query again, so it follows replaced elements.
  Avoid layout-dependent screen coordinates.
- Exercise real pointer/keyboard input for gesture and focus contracts; assert observable behavior.
- Compare exact project source and applied preview; check atomic undo/redo and unchanged source after cancellation or rejection.
  Use `wait_for_source_change` when the saved bytes are not known in advance.
  It waits for preview application because in-place writes can produce non-empty torn reads.
- Isolate fixtures for independent, parallel runs; parameterize genuine variants and delete redundant setup or wrappers.
- Reproduce failures; never weaken assertions, add skips, or replace screenshot references merely to pass.
- Inspect renders for visual changes and account for logical-to-physical pixel scaling.
- Run focused tests during development, then all non-skipped editor UI/Rust tests before committing code changes.
  Run `mise run ci:autofix:fix` and CI's strict Clippy check.
  Report failures and remaining skips.

Build the UI test binary from the repository root:

```sh
SLINT_EMIT_DEBUG_INFO=1 SLINT_ENABLE_EXPERIMENTAL_FEATURES=1 \
  cargo build --locked -p slint-editor --all-features --features slint/mcp
```

From `tools/editor/ui-tests`, use its installed test environment:

```sh
SLINT_EDITOR_UI_TEST_BACKEND=headless-skia ./run-tests.sh
```

Set `SLINT_EDITOR_BINARY` when using a different build output directory.
Use [CI](../../.github/workflows/ci.yaml) for current Rust test and lint commands.

## Retrying Assertions

Use element assertions for asynchronous element state:

```python
field = element(window, "Width", role=slint_testing.AccessibleRole.TextInput)
expect(field).to_have_value("240")
expect(query(window, "Progress")).to_be_hidden()
```

Each attempt reads the element again, and for a `query()`, looks up its only match again.
This lets the assertion survive an item-tree rebuild when the replacement has the same role, name, or ID.
Counting assertions (`to_be_visible()`, `to_be_hidden()`) need a `query()`, since an element can't count matches.

Use `expect.poll()` for state outside the element tree or state composed from several observations:

```python
expect.poll(
    lambda: target.is_file() and not source.exists(),
    message="renamed file exists and original is gone",
).to_equal(True)
```

Use `wait_until()` when the operation must return a value for later steps.
It's `slint_testing.wait_until()`: it calls the function until it returns a true value, and returns that value.
A result like `0`, `""`, or `[]` counts as not met yet, so compare such values instead of returning them.

Assertions and `wait_until()` retry with growing pauses of up to 200ms.
Don't wait for a state that lasts shorter than that, such as an animation in progress; wait for the state that follows it, or act while the earlier state holds.
Keep an ordinary `assert` when the result must be correct immediately.
An eventual assertion can hide a transient wrong value when immediacy is part of the contract.

`to_be_hidden()` proves that the testing query eventually has no match.
It doesn't prove that an element never appeared.
To observe a value throughout a time interval, assert it in a loop until the interval ends.

Visibility means discoverability through the testing query.
It doesn't prove that the element can receive pointer input or that another item doesn't cover it.

Assertion timeouts use seconds, matching `element()` and the existing UI-test helpers.
Failures distinguish a mismatched observation from a target that never became available.
