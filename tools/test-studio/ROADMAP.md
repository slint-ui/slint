# Test Studio Roadmap

## Product Goal

Build a native Slint workbench for writing, running, recording, and debugging UI tests.
Start with the Visual Editor, then support other Slint applications through adapters.
Keep pytest and ordinary Python files as the source of truth.
Every test created in Studio must also run from the command line and in CI.

This is a proposed sequence, not a delivery schedule.
Complete each milestone's acceptance workflow before expanding its scope.

## Current Baseline

Phase 1 adds complete pytest discovery, project settings, capability checks, sequential execution, and persistent run history.
The Visual Editor checkout used for validation contains 642 cases across 30 test files.
Studio supports filtering, group execution, rerun-failed, cancellation, source inspection, and captured screenshots.
The native UI uses pinned Primer Slint components from `github-app`.

The preview shows historical captures.
Recording, interactive debugging, source editing, and parallel execution remain future milestones.
The macOS launcher still depends on local checkouts and Python environments.
See [the README](README.md) for setup, storage policies, and validation commands.

## Milestones

| Milestone | User Outcome | Dependencies |
| --- | --- | --- |
| 1. Reliable Runner | Use Studio for everyday Visual Editor test runs | Existing prototype |
| 2. Readable Actions And Traces | Write concise tests and understand each action | Runner event contract |
| 3. Inspect And Debug | Find elements, pause tests, and inspect failures | Structured actions and locators |
| 4. Record And Edit | Turn an interaction into a maintainable pytest test | Locators, action model, inspector |
| 5. CI And Run Comparison | Diagnose shared failures and compare runs | Portable trace format |
| 6. Distributable Slint Tool | Install Studio and connect another Slint application | Stable adapter contract |

### 1. Reliable Runner

Implemented in version 0.2; the criteria below define its behavior.

Make the existing app dependable before adding new authoring modes.

- Discover the full configured pytest suite, with a file/test/parameter tree, markers, search, and rerun-failed.
- Provide project settings for the checkout, test interpreter, application binary, backend, and test selection.
- Check testing capabilities and backend availability before execution; explain incompatible binaries without waiting for test timeouts.
- Record binary identity, checkout revision, dirty state, runtime versions, and backend with every run.
- Distinguish test failures, fixture errors, collection errors, application crashes, and user cancellation.
- Preserve selection and results across refreshes; keep execution and collection off the UI thread.
- Save recent runs with configurable retention and a disk budget; preserve explicitly pinned runs.
- Pin the component-library dependency and document reproducible local setup.

Replace temporary bridge monkey patches with explicit integration hooks or a documented adapter where upstream hooks are unavailable.
Define versioned events with stable run, test, action, and attachment identifiers.
Keep the event contract independent of its transport.

**Completion test:** Discover all configured editor tests, run a filtered batch, cancel it, rerun failures, and reopen its results after restarting Studio.
An unsupported binary must produce an actionable setup error before any test starts.
Cancellation must leave no application subprocesses running.

### 2. Readable Actions And Traces

Create the common foundation for handwritten tests, debugging, and recording.

- Add locators that resolve at action time, using stable IDs and supported accessibility roles or names.
- Define strict matching rules and explain ambiguous or missing targets with visible candidates.
- Add actions such as click, fill, key press, drag, and scroll, with explicit timeout and readiness behavior.
- Add retrying assertions for text, values, visibility, and geometry.
- Express gestures relative to their target and coordinate space, preserving explicit coordinates where the test requires them.
- Emit action start/end, source location, target, timing, outcome, and diagnostic attachments from the same execution layer.
- Keep Visual Editor concepts in its adapter: canvas selection, handles, source changes, and preview synchronization.
- Make screenshot policies configurable so normal runs avoid unnecessary capture overhead.

Pilot the API on a few existing tests before committing to its public shape.
Choose move/resize, undo/redo, Escape cancellation, an inspector edit, and navigation.
Preserve low-level input access for tests that need exact event sequences.

Illustrative API direction, subject to the pilot:

```python
def test_resize_can_be_undone(editor):
    rectangle = editor.canvas.element("rectangle")
    before = rectangle.geometry()

    rectangle.resize_by(width=40, height=20)
    expect(rectangle).to_have_size(before.width + 40, before.height + 20)

    editor.undo()
    expect(rectangle).to_have_geometry(before)
```

The adapter must wait for the relevant source and preview acknowledgments.
Keep committed edits and transient preview/cancel behavior as separate contracts.
Shared helpers should reduce duplication without implying automatic coverage of every editor feature.

**Completion test:** Pilot tests run unchanged through pytest CLI and Studio, expose named actions, and identify the failing action and source line.
Locators survive component recreation; ambiguous targets fail clearly.
Repeated headless runs establish reliability before wider migration.

### 3. Inspect And Debug

Make a failure understandable without adding temporary prints or screenshots.

- Add an element picker with highlighting, properties, and a suggested locator that can be checked for uniqueness.
- Offer a live application view, clearly distinguished from captured history.
- Connect the action timeline to source lines, screenshots, target bounds, and assertion details.
- Show source and preview changes where the Visual Editor adapter can provide them.
- Pause before an action, step one action, continue, stop, and pause on failure before teardown.
- Support action breakpoints first; integrate a Python debugger later for arbitrary code and fixture debugging.
- Allow opening the test at its source line in the user's editor.

Introduce a command channel and an explicit running/paused/stopping state machine.
Specify how pause affects action timeouts and teardown.
Pausing test execution does not freeze application timers or animations.
Existing tests without action instrumentation retain ordinary run and report support.

**Completion test:** Pause before a resize, inspect the selected element, step the gesture, inspect the resulting source and preview, then continue.
A failing assertion remains inspectable until the user resumes or stops.
Stopping from any paused state must clean up the application.

### 4. Record And Edit

Record maintainable tests using the same actions that handwritten tests use.

- Capture native input and resolve semantic targets while the interaction happens.
- Group typing, clicks, drags, scrolls, and modifiers into meaningful actions.
- Prefer stable locators; identify coordinate fallbacks and ambiguous targets in the recording review.
- Add assertions through the inspector: choose a target, property, and expected value.
- Capture or require explicit startup fixtures and project state.
- Review, rename, delete, and reorder recorded steps before saving.
- Generate editable Python and open a diff before modifying an existing test file.
- Run the generated test immediately in a fresh application process.

First validate native recording exposure in the Python client, event ordering, target resolution, and buffer overflow behavior.
Raw input capture alone cannot produce dependable locators or infer the intended assertions.
Detect dropped events and mark the recording incomplete rather than silently exporting it.

**Completion test:** Record opening a fixture, selecting a rectangle, resizing it, asserting its size, and undoing the change.
Save the test and run it through both Studio and pytest from a fresh launch.
Repeat at another window size to verify locator and gesture portability.

### 5. CI And Run Comparison

Make a CI failure as inspectable as a local failure.

- Export and import self-contained, versioned run bundles containing traces, captures, logs, source excerpts, and environment identity.
- Support offline timeline playback with clear historical-state labeling.
- Compare assertions, screenshots, source changes, and environments between runs.
- Collect parallel worker events without mixing application identities or attachments.
- Add run history, failure grouping, and measured flaky-test history.
- Provide configurable capture and retention policies, including failure-focused CI artifacts.
- Define handling for logs, screenshots, and fixture files before sharing bundles.

Keep historical playback separate from live execution.
It does not rewind a running application or restore arbitrary application state.

**Completion test:** Open a failure bundle from another machine without the original checkout or a running editor.
Identify the failed action, its target, expected/actual values, source context, and relevant environment differences.

### 6. Distributable Slint Tool

Make installation and application integration independent of this development machine.

- Package the Slint UI, Python host, and pinned UI components into a reproducible macOS application.
- Define the supported Slint runtime, test client, and protocol versions.
- Keep test-project environments separate from Studio's own environment.
- Extract the application adapter contract and prove it with a second, small Slint application.
- Validate packaging and native interaction on Linux and Windows before declaring support.
- Document adapter setup, authoring, recording limitations, and CI integration.

The generic layer owns projects, pytest execution, locators, actions, traces, and debugging.
The Visual Editor adapter owns document fixtures, canvas semantics, source synchronization, and editor-specific assertions.

**Completion test:** Install Studio on a clean supported machine and configure both the Visual Editor and the second sample application.
Run and inspect tests without modifying Studio's source or relying on a developer's absolute paths.

## Next Implementation Slice

Prototype milestone 2's action API on one move/undo test.
Keep the current CLI and Studio execution paths working while introducing locators, retrying assertions, and structured actions.
Validate that API on a small set of real editor interactions before migrating more tests.

## Quality Gates

Every milestone needs an automated contract test and a real native UI workflow demonstrating its promised behavior.
Use headless Skia for routine Visual Editor validation; test visible interaction deliberately for picker and recorder features.
Cover application crashes, disconnects, stale targets, cancellation, and diagnostic capture failures at the shared boundaries.
Measure capture overhead separately from test performance.

Defer a full code editor, arbitrary execution rewind, AI-generated assertions, and wholesale conversion of the existing suite.
Revisit scheduling and estimates after the locator and native-recording feasibility work establishes their constraints.
