# Phase 2: Generic Testing And Visual Editor Helpers

Implementation status and intentional limits are tracked in [Phase 2 implementation](PHASE2-IMPLEMENTATION.md).
The design below includes follow-up API ideas beyond the five migrated pilot families.

This proposal defines both layers and their boundaries.
It supersedes ambiguous API contracts in the [twenty-test comparison](PHASE2-EXAMPLES.md).
The examples remain design sketches, not implemented APIs.
Playwright documentation was reviewed on September 26, 2026.

## Ownership And Dependencies

| Area | Owns | Does Not Own |
| --- | --- | --- |
| Ordinary Python and pytest | Fixtures, parameterization, context managers, typed helpers, plain assertions | A replacement test language or proprietary test-file format |
| Generic Slint Python testing library | Applications, windows, locators, input, action readiness, retrying assertions, geometry, captures, reporting hooks | Editor panes, Slint source serialization, canvas zoom, gradient sessions |
| Visual Editor test adapter | File tree, outline, inspector, canvas elements, handles, source revisions, undo, gradient sessions | A second locator engine, input dispatcher, assertion scheduler, or trace format |
| Test Studio | Discovery, execution, history, and presentation of generic and adapter events | A runtime dependency required to execute a pytest test |

The adapter depends on the generic library.
The generic library must import neither the editor adapter nor Studio.
The pytest reporting plugin connects library events to Studio's run journal.
Tests also run directly through pytest without a Studio process.

Use `app` for an application, `window` for its explicitly selected window, and `editor` for the optional Visual Editor adapter.
The generic API supports any instrumented Slint application; it is not an automation API for arbitrary desktop applications.
Proposed module placement is `slint_testing.ui` for the generic API and `visual_editor_testing` for editor helpers.
Those names are provisional and do not imply published packages.

This matches Playwright's page-object approach: application helpers wrap generic primitives and centralize application knowledge.
Having both layers is deliberate, not a departure from that model.
[Playwright page objects](https://playwright.dev/python/docs/pom).

## General Python Test Improvements

Keep ordinary `def test_...`, pytest fixtures, and `@pytest.mark.parametrize`.
Use fixtures to remove repeated binary/environment arguments, window lookup, and lifecycle cleanup.
Supply both a ready `window` fixture and a launch factory for startup, multiwindow, and custom-environment tests.
Do not eagerly launch an editor when a test needs to prepare source first.

Provide typed `Application`, `Window`, `Locator`, `Drag`, and assertion objects with autocomplete.
Use standard context managers for held input and named steps.
Keep pure calculations and independent expected-source builders as normal Python functions.
Ruff formats and lints these modules; ty checks their types.
These are library and authoring improvements, not changes to Python syntax.

Use a fresh process and isolated fixture directory by default.
Isolate application settings, socket names, and other owned resources where the application supports it.
Tests that intentionally exercise shared OS state must declare and restore it.
Context-manager cleanup and pytest teardown retain Phase 1 process ownership and cancellation guarantees.

## Generic Slint Syntax

The following operations belong to the generic layer:

| API Family | Proposed Surface | Required Meaning |
| --- | --- | --- |
| Window selection | `app.window(...)` | Resolve an explicit window identity or documented selection rule; ambiguous windows fail. |
| Locators | `window.get_by_role(...)`, `get_by_id(...)`, `get_by_accessible_name(...)` | Fresh resolution, scoped descendants, exact or regex names, strict single-target actions. |
| Composition | `locator.get_by_role(...)`, `locator.filter(has=...)` | Re-resolve the entire ancestor chain. Do not retain stale parent handles. |
| User actions | `click`, `dblclick`, `hover`, `fill`, `clear`, `press`, `press_sequentially`, `drag_to` | Consistent names, documented readiness per action, explicit input semantics. |
| Accessibility actions | `activate`, `set_accessible_value` | Explicit accessibility operations; never silently substitute them for a pointer or keyboard action. |
| Raw input | `window.keyboard`, `window.pointer` | Precise event ordering, held modifiers, logical window coordinates, out-of-window movement. |
| Assertions | `expect(locator).to_have_value`, `to_have_count`, `to_be_enabled`, geometry matchers | Fresh read-only observations with bounded retrying and useful expected/actual diagnostics. |
| Custom conditions | `expect.poll(read_only_probe)` | Extension for nonstandard observations, not the normal route for common properties. |
| Reporting | `step(...)`, action/assertion events | Shared trace protocol with source locations and nested steps, independent of Studio. |

Use native Slint role names such as `text-input` and `list-item`.
Document their correspondence to browser roles such as `textbox` and `listitem`; do not suggest they are identical standards.
Use explicit `exact=True` in regression-test examples.
The initial proposal's universal exact matching is still a design choice to settle before implementation.

`get_by_id` uses existing Slint element IDs.
A distinct `get_by_test_id` requires an explicit stable test-ID facility; production element IDs must not be relabeled as that facility.
Rendered text, accessible names, and associated form labels are different concepts.
Avoid a Playwright-shaped `get_by_label` until Slint exposes the corresponding association semantics.
[Playwright locator semantics](https://playwright.dev/python/docs/locators).

## Visual Editor Syntax

| Adapter Family | Proposed Surface | Added Application Knowledge |
| --- | --- | --- |
| Files | `editor.files.row(...)`, `editor.files.rename(...)` | File-tree structure, platform rename shortcut, inline rename completion. |
| Outline | `editor.outline.row(...)`, `drop_target(...)` | Hierarchy, before/onto/after drop positions, expected row state. |
| Inspector | `editor.inspector.field(...)`, `set_geometry(...)` | Pane scoping, field names, editor commit behavior, revision acknowledgment. |
| Canvas | `editor.canvas.element(...)`, `zoom_to(...)`, `center_selection()` | Document identity, preview placement, zoom and selection behavior. |
| Handles | `element.handle(...)`, `move_by(...)`, `resize_by(...)` | Editor handle names and document/local/window coordinate conversions. |
| Source | `SourceSnapshot`, editor source/preview synchronization | Exact project bytes and the revision applied to the preview. |
| Transactions | `editor.undo()`, `redo()`, gradient session helpers | Application shortcuts, commit/cancel boundaries, source revision changes. |

Locator-returning helpers must return the standard generic `Locator` type.
For example, `editor.inspector.field("Width")` scopes and names the field; its `fill`, `press`, and assertions come from the generic layer.
Do not create separate `EditorLocator.click` or `EditorExpect` engines.
Editor-specific assertions may register typed matchers with the generic reporting/deadline machinery.

Queries and assertions must not scroll, focus, select, or modify the application.
The current inspector helper scrolls while searching; migration must split revealing a field from observing it.
Expose `editor.inspector.reveal("Width")` when editor-specific scrolling is necessary.
Generic actions can scroll only under a documented policy backed by native support.

High-level editor actions may wait for a specific committed revision.
Low-level pointer release cannot assume every drag commits source: gradient sessions may still be open.
Keep `fill` and an explicit commit key available for stale-edit and cancellation tests.
Do not hide those boundaries inside every field helper.

## The Same Interaction At Both Levels

These examples preserve the accessibility input path used by existing inspector tests.
They show the same UI-level check; source and preview assertions can be added to either test.

Generic Slint API, with application-specific names supplied as data:

```python
def test_change_width(window):
    window.get_by_role("list-item", name="root-rectangle", exact=True).activate()
    inspector = window.get_by_role(
        "complementary", name="Inspector and outline", exact=True
    )
    width = inspector.get_by_role("text-input", name="Width", exact=True)
    width.set_accessible_value("200")
    expect(width).to_have_value("200")
```

Optional Visual Editor adapter, using the same underlying locator and action:

```python
def test_change_width(editor):
    editor.canvas.element("root-rectangle").select()
    width = editor.inspector.field("Width")
    width.set_accessible_value("200")
    expect(width).to_have_value("200")
```

For a keyboard-edit test, either locator supports the same generic operations:

```python
width.fill("200")
width.press("Enter")
expect(width).to_have_value("200")
```

That keyboard version exercises a different input path; do not substitute it when mechanically migrating an accessibility test.
`select()` must preserve the existing adapter's selection behavior and expose its concrete actions in the trace.

## Corrections To The Earlier Proposal

1. **Define `fill` by a portable control contract.**
   It replaces editable text without pressing Enter or committing through blur.
   Do not define generic fill as the editor's current activate/Delete-per-character recipe.
   A Slint text-control implementation must establish and verify selection, focus, and input behavior.
   Keep the old staging helper until that behavior is proven equivalent for its test.
   Use `press_sequentially` when individual key events are the test.
   Playwright also distinguishes fill from character-by-character input.
   [Playwright input methods](https://playwright.dev/python/docs/api/class-locator#locator-fill).

2. **Make accessibility operations unmistakable.**
   Rename the sketch's ambiguous `set_value` to `set_accessible_value`.
   Keep `activate` distinct from `click`.
   Trace the actual input path so an accessibility shortcut cannot conceal a pointer failure.

3. **Reduce unnecessary naming differences.**
   Prefer `dblclick`, `press`, and `press_sequentially` where their meanings align with Playwright.
   Standardize public action/assertion timeouts as `timeout` in milliseconds.
   Existing source helpers retain seconds during migration; the bridge must convert explicitly.
   Do not copy browser-only methods or browser label semantics by name alone.

4. **Replace common polling lambdas with property assertions.**
   Add generic value, checked state, count, accessible name, bounds, and position matchers.
   Keep `expect.poll` for application-specific observations and independent calculations.
   `expect.poll` here is a proposed extension, not a claim about Playwright Python's documented assertion API.

5. **Treat tracing as part of the API contract.**
   A helper should create a readable parent step while generic calls remain visible as children.
   Retained raw helpers have group-only reporting until their internals are instrumented.
   Mark that limitation in the timeline rather than implying complete capture.

6. **Do not attach screenshots to every input by default.**
   Trace raw events cheaply and preserve their ordering.
   Capture at selected boundaries or on failure so tracing does not insert waits into race tests.

## Playwright Review: Gaps And Opportunities

| Area | Playwright Baseline | Gap In Our Proposal | Required Improvement |
| --- | --- | --- | --- |
| Locators | Fresh resolution, scoping, filtering, strict actions | Mostly single labels and helper aliases | Implement composition, exact/regex matching, ambiguity diagnostics, and ancestor recreation tests. |
| Action readiness | Per-action checks; clicks require visibility, stability, event reception, and enabled state | Existence/enabled checks do not establish clickability | Publish a native readiness matrix and add missing hit-test, clipping, and scrolling support. |
| Input | Distinct fill, sequential typing, keys, pointer operations | Editor staging behavior was embedded in generic fill | Separate input paths and verify focus, selection, and event ordering on independent widgets. |
| Assertions | Retrying assertions, custom messages, expected/actual call logs | Too many handwritten polling functions | Add typed common matchers, useful descriptions, observation history, and image/source attachments. |
| Fixtures | Pytest integration and isolated browser contexts | Examples repeated launch arguments | Typed fixtures and a launch factory; fresh native process and application data by default. |
| Gestures | High-level drag plus low-level mouse/key operations | Held gestures were useful but underspecified | Define explicit release, exception cleanup, coordinate units, modifiers, and dispatch acknowledgment. |
| Trace inspection | Action details, source, logs, screenshots, DOM snapshots | Phase 1 stages/captures are a smaller capability | Add nested actions/assertions, locator candidates, target bounds, wait diagnostics, and capture policy. |
| App helpers | Page objects layer application behavior over primitives | Editor methods looked like core framework methods | Separate imports/types/docs and classify each helper by ownership. |
| Tooling | Python Inspector/code generation/traces; Playwright Test adds UI Mode | API sketches cannot provide recording or debugging alone | Preserve Phase 3 inspector/debugger and Phase 4 recording; Phase 2 supplies structured actions. |

The actionability gap needs native engineering, not just Python wrappers.
The current [testing protocol](../../internal/backends/testing/slint_systest.proto) exposes IDs, roles, labels, values, geometry, enabled/read-only state, and input requests.
It does not establish a complete hit-test, effective clipping, or scroll-into-view readiness contract.
An existing element with nonzero bounds and opacity is not sufficient proof that a pointer click reaches it.
Where native evidence is missing, report an unsupported capability or documented limitation rather than silently invoking accessibility.
[Playwright actionability](https://playwright.dev/python/docs/actionability).

Use one deadline across an operation's lookup, readiness, dispatch, and required acknowledgment.
Assertions get their own explicit deadlines, bounded by cancellation and any enclosing step budget.
Retry safe reads and pre-dispatch resolution; never repeat uncertain dispatched side effects.
Transport loss after dispatch is an uncertain result, not permission to click again.
Unexpected application exit should abort waiting with process evidence.

Playwright Python currently documents soft assertions with sufficiently recent pytest plugins.
We should not present soft assertions as a unique advantage.
For Phase 2, prioritize good hard-assertion diagnostics; soft checks can follow without blocking the pilot.
[Playwright assertions](https://playwright.dev/python/docs/test-assertions).

Potential advantages are native-specific and remain goals to validate:

- A held-drag context can make desktop modifier and cancellation sequences clearer than manually pairing low-level mouse calls.
- Immediate, eventual, and sustained checks can make transient preview and source-write invariants explicit.
- An editor trace can correlate input, source revision, preview acknowledgment, and rendered geometry in one view.
- Typed native geometry can distinguish logical window units, screenshot pixels, document units, and transformed local coordinates.
- Python action and assertion events can be integrated directly into the pytest journal.

Playwright already provides low-level mouse control and rich trace inspection.
Our gesture and revision conveniences should build on those ideas rather than imply that Playwright cannot test such workflows.
[Mouse control](https://playwright.dev/python/docs/api/class-mouse), [Trace Viewer](https://playwright.dev/python/docs/trace-viewer).

Keep the product comparison precise.
Python's pytest integration is the relevant authoring baseline.
Playwright Test's JavaScript/TypeScript UI Mode is the workbench benchmark.
The documented `context.tracing` API does not record test assertions; that limitation is specific to that API.
Our screenshots and element properties are not equivalent to inspectable historical DOM snapshots or live rewind.
[Python pytest integration](https://playwright.dev/python/docs/test-runners), [UI Mode](https://playwright.dev/docs/test-ui-mode), [Tracing API](https://playwright.dev/python/docs/api/class-tracing).

## Classification Of All Twenty Examples

Every row also uses ordinary pytest fixtures and any existing parameterization.
This identifies where the readability gain belongs, even when both layers appear in one function.

| # | Test | Generic Library Improvements | Visual Editor Or Test-Specific Improvements |
| --- | --- | --- | --- |
| 1 | Startup | Role/name locators, count/enabled assertions | Startup labels and launch configuration |
| 2 | Rename | Activation, keyboard input, condition waiting | File rows, rename shortcut, filesystem expectations |
| 3 | Library search | Value assignment, scoped collections, ordered name assertions | Library groups and collapse-state expectations |
| 4 | Delete routing | Real click, keys, presence assertions | Canvas selection and unchanged project source |
| 5 | Inspector geometry | Fresh reads, value/bounds assertions | Inspector scope, preview offset, exact source |
| 6 | Inline text | Double-click, keys, absence assertions | Move handle and editor-specific Escape-commits behavior |
| 7 | Reparent | Drag-to, scoped list observations | Outline targets, hierarchy and source goldens |
| 8 | Cross-pane drag | Held gesture, movement, Escape, release | Palette, canvas and outline previews |
| 9 | Nested rotation | Explicit window deltas and sampled frames | Rotated handle geometry and local-source conversion oracle |
| 10 | Live Shift | Held modifier changes and geometry assertions | Rectangle resize constraints and commit boundary |
| 11 | Outside-window resize | Window size and unclamped pointer movement | Resize handles and resulting source |
| 12 | Zoomed dragging | Pointer deltas, steps and assertions | Zoom, document units, undo/redo source checks |
| 13 | Clipping | Screenshot capture and image-diff support | Protected pane regions and independent pixel oracle |
| 14 | Hover after reload | Pointer control and read-only observations | External source reload and preview hover labels |
| 15 | Stale edit | Keyboard staging, current-focus keys, read-only waits | Source revisions and invalidated inspector edit |
| 16 | Rapid reload | Sustained observations, process/window identity | Unsynchronized source writes and newest-revision oracle |
| 17 | Undo matrix | Fixtures, parameterization, steps and shared assertions | Edit matrix, source byte oracle, oriented/radius geometry |
| 18 | Undo during drag | Held pointer and explicit keyboard shortcut | Editor undo history and release cancellation |
| 19 | Gradient session | Scoped popup locators, input and nested steps | Gradient modes, brush binding, session commit/cancel |
| 20 | Conic seam | Ordered pointer path, transformed geometry reads | Gradient seam winding, 367-degree source, implicit center |

## Delivery And Acceptance

1. **Generic foundation:** typed fixtures, locators, input contracts, common assertions, and capability reporting.
   Validate with a small non-editor Slint application containing text inputs, buttons, lists, scroll areas, and popups.
   Test duplicate names, component replacement, obscured targets, focus, cancellation, and held-input cleanup.
2. **Generic tracing:** nested actions/assertions, durations, diagnostics, optional captures, and direct-pytest output.
   Verify that the same test runs with and without Studio and does not import editor helpers.
3. **Editor adapter:** thin file/outline/inspector/canvas helpers, explicit revision waits, and transaction semantics.
   Pilot examples 2, 5, 10, 15, and 20.
   Compare input sequences and independent assertions against the originals before broad migration.
4. **Trace integration and polish:** show adapter groups with generic child actions, and mark raw-helper gaps.
   Check assertion source lines, capture selection stability, and bounded reporting overhead.

Retain old tests until the new versions establish equivalent coverage.
Inject representative faults to prove they catch premature writes, stale commits, wrong coordinate conversion, and seam wrapping to 7 degrees.
Do not merge generic-library and editor-adapter test coverage into a single editor-only acceptance run.
Recording and interactive debugging remain later milestones.
