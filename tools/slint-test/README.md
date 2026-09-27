# Slint Test: Readable Python Tests

`slint_test` is a synchronous Python API over `slint_testing==0.3`.
It depends on neither Visual Editor nor Test Studio.
Tests use ordinary pytest fixtures, parameterization, assertions, and context managers.
This is an experimental API validated on macOS with headless Skia.

## Run Without Studio

Use an existing Python environment containing pytest and `slint_testing==0.3`.
From the repository root, load the library and its optional pytest fixtures:

```sh
PYTHONPATH=tools/slint-test python -m pytest -p slint_test.pytest_plugin \
  /path/to/tests --slint-command /path/to/application
```

The application must be compiled with Slint system-testing support and emit debug information for element IDs.
Set `SLINT_BACKEND=headless-skia` explicitly for headless execution.
Neither this library nor Studio installs packages or builds applications.
For applications with command-line arguments, prefer an explicit fixture:

```python
import pytest
from slint_test import launch, expect


@pytest.fixture
def app():
    with launch(["/path/to/application", "--example-option"]) as application:
        yield application


@pytest.fixture
def window(app):
    return app.window()


def test_edit_name(window):
    name = window.get_by_role("text-input", name="Name")
    name.fill("Alice")
    window.get_by_role("button", name="Apply").click()
    expect(name).to_have_value("Alice")
```

Each `launch` owns a fresh application process and temporary XDG configuration/data directories.
Applications must respect those directories to isolate their settings; this isn't an OS sandbox.
On POSIX, a monitor owns the application's process group and cleans up descendants if the test parent exits.
Windows process ownership hasn't been implemented or validated.
`app.window()` requires one window; `app.window(index=1)` explicitly selects another.
The optional `app_factory` fixture exposes `launch` for tests that prepare files first.

## Locators And Input

Locators resolve their complete ancestor chain on each observation or action.
Names match exactly by default; use `exact=False` for case-insensitive substring matching or a compiled regex.
Actions require exactly one match and report candidate names on ambiguity.
Slint roles use native names, such as `text-input` and `list-item`.

```python
pane = window.get_by_role("region", name="Settings")
name = pane.get_by_role("text-input", name="Name")
row = window.get_by_role("list-item").filter(
    has=window.get_by_accessible_name("Example")
)
first_row = window.get_by_role("list-item").nth(0)
```

`get_by_id` uses an existing Slint element ID, not a separate test-ID facility.
Queries and assertions never focus, scroll, select, or modify controls.
`all()` returns the currently visible matches for direct inspection, while `count()` includes instantiated clipped matches so tests can verify off-screen content before scrolling it into view.
`filter(has=...)` accepts a window-rooted locator and evaluates it relative to each candidate.
`nth(index)` makes an intentional positional choice explicit and rejects negative indexes.

| Operation | Input And Readiness |
| --- | --- |
| `click`, `dblclick` | Fresh unique target, stable geometry, native left-click routing, and automatic scrolling when supported. `click(force=True)` explicitly bypasses hit-target checks. |
| `hover`, `drag` | Basic enabled, geometry, opacity, and stability checks; no native routing guarantee. |
| `scroll_into_view` | Reveal the target center through interactive Flickable ancestors without clicking. |
| `fill`, `clear` | Editable text input plus pointer readiness; click, Control+A, Backspace, then key events. No Enter or blur commit. |
| `press`, `press_sequentially` | Click to focus the target, then dispatch keys. |
| `activate` | Explicit accessibility default action on a fresh unique target. |
| `set_accessible_value` | Explicit accessibility value assignment on a fresh unique target. |
| `window.keyboard` / `window.pointer` | Named low-level input at current focus or logical window coordinates, including down/up, `press_at`, `release_at`, `click_at`, scroll, and pointer exit. These operations make no target or actionability claim. |

The transport reports unspecified accessible-enabled state as false.
Accessibility operations therefore check unique resolution, not enabled state.
They never substitute for pointer or keyboard methods.

New applications negotiate version 1 of the native pointer-target contract.
`click`, `dblclick`, `fill`, `clear`, and locator key methods wait until the transformed target center receives left input.
They wait for overlays and active gestures, check ancestor clipping, and scroll instantiated targets through nested interactive Flickables.
The native click checks again after hover callbacks and sends a press only if the target remains ready at the same position.
The timeline includes scrolling and the observed pointer-target details.

`locator.pointer_target()` returns a read-only status and reason without moving the pointer or invoking input filters.
Statuses distinguish ready, covered, clipped, disabled, busy, no target, and unsupported policies.
This predicts built-in left-click routing, including child popups; it isn't a general hit-test API for every input type.
Unknown custom item policies fail closed. Native top-level popup routing and complex menu chains remain unsupported.
A fixed clipping rectangle can't be scrolled, and virtualized rows that haven't been instantiated can't be found.
Modern queries include instantiated clipped elements, so use scopes to distinguish duplicate labels.

Older binaries retain basic readiness and visible-only discovery.
Their capabilities report no hit testing or scrolling, and action events label the target unverified.
`click(require_hit_target=True)` rejects these binaries before input is dispatched.
Inspect `window.capabilities` to distinguish these modes.
Accessibility methods remain explicit and never substitute for pointer methods.

Public timeouts are milliseconds, defaulting to 5,000.
A locator operation shares its deadline across lookup, readiness, and input; nested operations cannot extend it.
Transport requests are bounded to the remaining budget, with a five-second ceiling per request.
Cancellation is checked between requests, not by interrupting a request already in flight.
A transport timeout or disconnect aborts the operation; dispatched actions are never retried.
After a response timeout, cleanup may take up to five additional seconds to drain that response so failure reporting can safely use the connection.
If draining fails, the connection closes and captures report the disconnect.
Custom `expect.poll` callbacks must themselves be bounded and read-only.

## Assertions And Gestures

```python
expect(name).to_have_value("Alice", timeout=2000)
expect(name).not_to_have_value("Bob")
expect(row).to_have_count(1)
expect(window.get_by_role("button", name="Apply")).to_be_enabled()
expect(row).to_have_geometry(width=200)
expect.poll(read_saved_bytes, message="saved source").to_equal(expected_bytes)
expect.poll(read_saved_bytes).to_remain(original_bytes, for_ms=250)
```

Common matchers cover values, accessible names, enabled/checked/selected state, counts, and geometry.
Geometry accepts `pytest.approx` values and a positional mapping for parameterized property names.
Timeout diagnostics retain the expected value and last observation.
Assertion failures also carry a structured comparison for debugger summaries and saved results.
An unavailable observation is explicit; transport failures are not presented as value mismatches.
Sustained checks fail on the first mismatch; they don't wait for a later good value.

```python
with handle.drag(rotation_degrees=30) as drag:
    drag.move_by(20, 16, space="window")
    window.keyboard.down("Shift")
    expect.poll(read_preview_geometry).to_equal(expected_geometry)
    drag.release()
```

Coordinates are logical window units and aren't clamped at window edges.
`move_by` defaults to the current pointer position; use `origin="start"` for cumulative paths.
A drag releases newly held modifiers on exit.
An unreleased drag sends Escape and releases its pointer, including after an exception.
Explicit `release()` is the commit boundary; it doesn't imply an application-specific source write.
`window.keyboard.press(...)` preserves current focus, unlike `locator.press(...)`.

## Reporting

```python
from slint_test import reporting, step

with reporting(events.append), step("Change name"):
    name.fill("Alice")
    expect(name).to_have_value("Alice")
```

Actions and assertions emit nested start/end events with IDs, parents, source locations, arguments, durations, and outcomes.
Readiness events include target bounds when available and identify unverified hit testing.
Observer exceptions become logged diagnostics and cannot change the test outcome.
Without an observer, tests retain their normal pytest behavior.
Studio installs an observer for the pytest session and persists events in its versioned journal.

## Action Control And Inspection

`slint_test.control.debugging(controller)` installs optional action control independently of the reporting observer.
Controllers receive `before`, `failed`, and `after` callbacks on the executing test thread.
Studio uses this contract for pausing and stepping; normal pytest runs install no controller.
`failed` runs before the action unwinds, preserving the application for inspection.
Operation deadlines use an active clock that excludes time inside `suspended_clock()`.
Transport cleanup deadlines retain wall-clock bounds.

`slint_test.inspection.capture(application)` returns visible element data and a screenshot of the first window.
Inspection reads at most 1,500 elements and stops starting property reads after five seconds.
Individual transport requests remain bounded separately.
Truncated inspections suppress locator suggestions.
Bounds are logical window coordinates; screenshot pixels can use a different scale.
Bounds-based picking cannot establish input targets or effective clipping.

Application adapters can supply source paths without teaching the generic library about their domain:

```python
from slint_test import inspection_sources

with inspection_sources(document_path):
    run_application_actions()
```

The reporting context itself doesn't read those files.
Studio captures up to eight supplied files, capped at 64 KiB each, while paused.
Nested source contexts restore their parent on exit.

## Independent Validation

The fixture application uses Studio's Slint-enabled Python runtime, while its driver uses the editor test environment.
No editor binary or adapter is required:

```sh
SLINT_EMIT_DEBUG_INFO=1 SLINT_ENABLE_EXPERIMENTAL_FEATURES=1 \
  cargo build -p i-slint-backend-testing --features system-testing,renderer-skia \
  --example pointer_fixture
cd tools/slint-test
SLINT_FIXTURE_PYTHON=/path/to/test-studio/.venv/bin/python \
SLINT_POINTER_FIXTURE=../../target/debug/examples/pointer_fixture \
  /path/to/ui-tests/.venv/bin/python -m pytest -q
```

The suite covers input, replaced components, duplicates, relative scopes, disabled/read-only controls, a covered control, clipped list rows, popups, cancellation, deadlines, observer isolation, and orphaned descendant cleanup.
Use the existing `lint:python:test-studio` task for Ruff and ty checks across both runtimes.

The native fixture exercises current routing; the Python fixture also checks compatibility with the older transport.
The wire extension uses a private protobuf descriptor pool and preserves unknown response fields through `slint-testing==0.3`.
It doesn't modify the installed SDK.
After changing `internal/backends/testing/slint_systest.proto`, copy the generated
`OUT_DIR/slint_systest.descriptor` from that backend build to `slint_test/native.descriptor`.
The Rust `python_protocol_matches_native_schema` test rejects a stale descriptor.
Studio includes the descriptor in its preflight cache identity.
