# Test Best Practices

- Default to `headless-skia` for local automated tests.
  Use visible windows only for explicit manual checks or native window behavior.
- Test the real `slint-editor` from the current checkout; rebuild after Rust or editor-shell changes.
- Wait for observable state, never fixed delays: `wait_for_source` before input, `wait_until` for UI state, and `SourceSnapshot.wait_for_applied` after edits.
  Bound waits and poll throughout negative-assertion observation periods.
- Reuse helpers in `ui_driver`, `canvas_interactions`, `gradient_interactions`, `inspector_interactions`, and `source_snapshot`.
  Never import another test module or duplicate interaction, synchronization, or coordinate logic.
- Share production validation and history paths for equivalent single-value, color, and batch edits.
  Cover stale targets and rejected edits.
- Use scoped, stable labels or IDs; reacquire replaced elements and check validity.
  Avoid layout-dependent screen coordinates.
- Exercise real pointer/keyboard input for gesture and focus contracts; assert observable behavior.
- Compare exact project source and applied preview; check atomic undo/redo and unchanged source after cancellation or rejection.
  Reuse save waits that ignore transient empty reads.
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
