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
These waits are scoped to the checkpoint or operation supplied by the test;
they do not infer completion from a quiet event loop.
Do not use an installed-source wait to prove that a failed source was processed.

Use `snapshot.wait_for_applied(expected, relative_path)` before an action that
depends on an edit's completed preview and history update. For an external
write whose failure or success phase matters, capture a checkpoint before the
write and use the revision-specific API:

```python
checkpoint = current_editor_sync.get().checkpoint()
source.write_bytes(broken)
current_editor_sync.get().wait_for_processed(
    source, broken, after=checkpoint, outcome="compile_error"
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
private settings store and records a copied binary, source revision, build
features, and handshake in the test artifacts.

Protocol version 4 also provides scoped scheduling gates. Use the context manager
so a failed assertion releases the gate:

```python
sync = current_editor_sync.get()
with sync.gate("publication", source) as gate:
    with sync.action() as edit:
        perform_input()
    gate.wait_for_reached()
    # Inspect the held stage without treating it as a completed edit.
edit.wait_for_settled(outcome="completed")
```

The `source` gate holds normal external-source processing. The `publication`
gate holds a compiled preview before factory assignment. The `factory` gate
holds creation after assignment, before invoking the component factory. Its
existing causal lease ends on installation or retirement; releasing a retired
factory cannot install it. The `acknowledgment` gate holds the real local edit
response and exposes its edit ID. Only a matching response and an installation
containing that edit's input revisions can complete a pending edit. Installation
abandonment cancels queued history and allows an acknowledged edit to settle. Ordinary source supersession is terminal only after a later instance is installed and, after the write acknowledgment, its edited-file inputs match current disk. This also covers watcher coalescing that skips the written revision. The disk comparison runs at edit completion, like history validation; test waits do not trigger a reload. Supersession cancels queued history and discards history based on the replaced source.

The observer keeps the last successful attempt for diagnostics separately from whether an instance is mounted. Image-mode switches and factory callbacks that return no component clear mounted state. Applied waits require a mounted instance, including while an external reload is held at the factory gate.

To prove that pointer-down owns work, leave the action context before checking
its pending state. An unsealed action is pending even without a gesture token.
After cancellation, await the canceled action and assert zero writes. Dispatch
a late release in a separate action and check it independently:

```python
with sync.action() as gesture:
    pointer_down()
# The operation is now sealed; its pending work must contain the gesture.
cancel_gesture()
gesture.wait_for_settled(outcome="canceled")
gesture.assert_no_source_writes()
with sync.action() as release:
    pointer_up()
release.assert_no_source_writes()
snapshot.assert_unchanged_now()
```

Operations record accepted edits, completed writes, and possible file mutations
separately. Failure opening an existing file has no mutation; attempted creation or
truncation is conservatively a possible mutation even if the final bytes match.
`assert_no_source_writes()` rejects any of these counters, including a completed
write later undone. A successful edit followed by undo therefore records two
writes even when disk content returns to the baseline.

The private `write_fault` request accepts `before_open`, `after_truncate`, or
`{"after_bytes": 5}` with a source URL. It is consumed by that file's next write.
Always send `clear_write_fault` in `finally` to remove an unconsumed fault.
Truncation and partial-write faults modify the real fixture file. Failure before
mutation preserves history; possible mutation invalidates history and rereads
source through the editor session. This is failure recovery, not a reload
triggered by a synchronization wait. Workspace writes are not transactional
across files; earlier successful writes remain when a later file fails.

### Audit disposition

| Finding | Disposition | Current state |
| --- | --- | --- |
| F01 | fixed | Broken-source tests acknowledge the exact failed revision before checking the retained preview. |
| F02 | fixed | Repair follows acknowledgment of the broken phase. |
| F03 | deferred | Deleted-root recovery remains skipped; no watcher repair is included. |
| F04 | fixed | Missing imports have an observed missing input and terminal failure before restoration. |
| F05 | fixed | Source changes cancel the active gesture; cancellation and late release have terminal no-write checks. |
| F06 | fixed | Undo during dragging and queued undo during pending publication have separate controlled tests. |
| F07 | fixed | Rotation release holds publication and inspects the retained oriented canvas frame before installation. No claim is made about every intermediate rendered frame. |
| F08 | fixed | Burst writes remain a coalescing test; separate publication and factory gates prove obsolete attempts cannot replace a newer instance. |
| F09 | fixed | Snapshot helper tests use a controlled polling callback instead of a thread timer. |
| F10 | fixed | Inspector rendering consumers await applied source before checking the element. |
| F11 | partial | Cancellation and rejection tests use terminal operation counters. Remaining navigation and held-gesture `assert_unchanged_now()` calls are point-in-time disk assertions, not settlement guarantees. |
| F12 | fixed | Redo has distinct held-source and already-applied external-edit variants. |
| F13 | fixed | Image switching covers held publication; lifecycle tests additionally cover an assigned factory with acknowledgment before and after abandonment. |
| F14 | partial | Lookups without waits and deadline-aware polling exist. These do not make every multi-probe UI read an atomic snapshot. |
| F15 | fixed | First-window acquisition uses a bounded condition wait; lifecycle requests also check process exit. |
| F16 | fixed | Session, cursor, actual attempt inputs, installed identity, edit IDs, and causal leases distinguish the milestones. |
| F17 | fixed | Each worker uses a copied binary with a streamed checksum cached for that immutable path, build revision, features, and required protocol handshake. Shell live preview is disabled. |
| F18 | partial | Handles are reacquired at migrated replacement boundaries. Direct multi-property reads still require a test-controlled stable boundary. |
| F19 | fixed | Every process gets a private settings directory and pinned defaults. |
| F20 | fixed | Initial broken-source recovery acknowledges the startup failure before repair. |
| F21 | fixed | Palette edits use operation completion before capturing source; overlap variants deliberately retain their gates. |

Disk-only `wait_for_exact()` and held-gesture `assert_unchanged_now()` checks are
intentional where that is the test contract. Neither proves future inactivity.
The observer covers work caused by the specified operation, not unrelated
future filesystem events. A coalesced revision that the editor never reads has
no invented attempt. History and event retention are bounded; an expired cursor
fails explicitly. Fault injection exercises controlled stages, not every
platform-specific filesystem error or crash durability.

### Validate lifecycle changes

Run helper tests before application tests:

```sh
uv run pytest tests/test_editor_sync.py tests/test_source_snapshot.py
uv run ruff check tests
uv run ruff format --check tests
uv run ty check tests
```

Run the affected lifecycle, transform, and history tests with `-n 1` and `-n 4`
against the same preserved binary using `SLINT_EDITOR_BINARY`. Run the complete
headless suite with `-n 3`. Keep replay pauses at zero. Failure artifacts include
source differences, screenshots, observer state, and `trace.jsonl` beside the
binary metadata. These scenarios require actual reached gates; repeated timing
runs alone are not evidence of the intended ordering.

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
