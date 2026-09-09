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

The test-only observer reports four different milestones. `observed` means the
normal source path read a revision (or observed a missing file), `processed`
means that exact input finished with `compiled`, `compile_error`, or another
terminal outcome, `applied` means a successful component instance was installed,
and `settled` covers the causal work for an action, including queued history.
Do not use an installed-source wait to prove that a failed source was processed.

Use `snapshot.wait_for_applied(expected, relative_path)` before an action that
depends on an edit's completed preview and history update. For an external
write whose failure or success phase matters, capture a checkpoint before the
write and use the revision-specific API:

```python
checkpoint = current_editor_sync.get().checkpoint()
source.write_bytes(broken)
current_editor_sync.get().wait_for_processed(
    source, broken, after=checkpoint["cursor"], outcome="compile_error"
)
```

`EditorSync.action()` captures the causal boundary without delaying input.
Use its write counter for canceled and rejected operations. Keyboard and
pointer helpers dispatch immediately; completion waits belong at call sites
that need them. Keep `wait_for_exact` for disk-only assertions and tests that
deliberately send input while compilation is pending.

The protocol has a session identity, version, monotonic event cursor, and a
bounded event history. A stale cursor or history overflow fails the wait with a
diagnostic instead of silently accepting an old event. The request path never
starts a reload. The editor process uses `SLINT_EDITOR_TEST_CONFIG_DIR` for a
private settings store and records the binary hash in its temporary sync
directory.

### Audit disposition

| Finding | Disposition | Current state |
| --- | --- | --- |
| F01 | fixed | Broken-source test waits for its exact compile error. |
| F02 | fixed | Repair waits for the broken phase before writing the repair. |
| F03 | defer | Root recovery remains explicitly skipped pending watcher repair. |
| F04 | defer | Import recovery still needs missing-input lifecycle coverage. |
| F05 | partial | External revision is processed before gesture release; terminal no-write proof remains local. |
| F06 | partial | Undo/release uses revision and write counters; full queued-history gate remains. |
| F07 | defer | Rotation publication gate is not yet exposed. |
| F08 | partial | Newest revision waits on processing and application; older-attempt scheduling gate remains. |
| F09 | fixed | Source snapshot uses an explicit thread handshake instead of a timer. |
| F10 | defer | Existing-element rendering callers need applied-generation migration. |
| F11 | defer | Negative-observation callers require per-action terminal outcomes. |
| F12 | defer | Redo overlap needs an external-observation gate. |
| F13 | defer | Palette publication overlap needs a preview gate. |
| F14 | defer | Nested UI probes still need a shared deadline and nonblocking variants. |
| F15 | defer | First-window startup should use a bounded readiness condition. |
| F16 | fixed | Revision-specific observer protocol covers observed, processed, and applied events. |
| F17 | partial | Binary identity is recorded; CI artifact pinning remains external to the fixture. |
| F18 | defer | Reload-sensitive handles still need generation-aware reacquisition. |
| F19 | fixed | Each editor process has a private settings directory. |
| F20 | defer | Initial failure occurs before the test checkpoint and needs startup lifecycle access. |
| F21 | partial | Exact applied waits replace byte-change probes where migrated; remaining canvas callers need migration. |

The remaining deferred cases require controlled source and preview publication
gates. They must not be “fixed” by increasing a timeout or adding a silence
window.

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
