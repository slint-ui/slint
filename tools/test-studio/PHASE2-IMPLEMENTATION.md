# Phase 2 Implementation

Phase 2 now has a generic Python API, an optional Visual Editor adapter, and a nested action timeline in Studio.
Five readable pilot families remain alongside their original tests.
Three additional gesture families now use the readable API in place.
This is an experimental authoring API. Action debugging has since been added; recording remains a later milestone.

## Ownership

| Layer | Location | Implemented Responsibilities |
| --- | --- | --- |
| Generic Python API | `tools/slint-test/slint_test` | Application fixtures, fresh scoped locators, strict matching, input readiness, pointer/keyboard/accessibility actions, retrying and sustained assertions, held gestures, optional reporting. |
| Visual Editor adapter | `tools/editor/ui-tests/tests/visual_editor_testing.py` | File rename, outline rows, inspector fields and explicit reveal, canvas selection and handles, zoom/centering, source helpers, undo/redo shortcuts. |
| Studio | `tools/test-studio` | Session observer, nested action journal/reducer, group expansion, readable action details, failure captures, history replay, and capture selection stability. |

Adapter methods return the generic `Locator`; they don't create an independent action or assertion engine.
The generic library imports neither editor helpers nor Studio.
The separate native fixture exercises text controls, buttons, replaced components, duplicate names, clipped lists, covered targets, and popup scopes.
See [the generic API guide](../slint-test/README.md) for runnable examples and exact semantics.

## Pilot Coverage

| Family | Cases | Preserved Invariant |
| --- | --- | --- |
| File rename | 1 | New filename, old filename absent, unchanged file content. |
| Inspector geometry | 4 | Exact source bytes and the corresponding rendered geometry. |
| Shift changes during resize | 2 | Modifier changes affect preview without extra pointer motion; source remains unchanged until release. |
| Stale revision rejection | 4 | External revision wins and the invalidated edit cannot overwrite it. |
| Conic seam crossing | 2 | Ordered path crosses 360 degrees, saves 367 rather than 7, retains implicit center, and survives undo/redo and reopening. |

These are additions, not wholesale migrations of the suite.
The twenty before/after examples remain design sketches beyond these five implemented families.
Some retained editor helpers provide a named group without internal action reporting; Studio labels that limitation.
SourceSnapshot remains the existing independent byte/preview oracle, with its original seconds-based timeouts.
Generic API timeouts use milliseconds.

## Trace Behavior

Action events retain IDs, parent IDs, source calls, arguments, duration, status, and diagnostics in the versioned JSON Lines journal.
A shared reducer handles live events and restored runs.
Completing an action updates its existing row, preserving selected indices and captures.
Groups support pointer, keyboard, and accessibility expansion.

Action details appear before test output, so a large log cannot hide the selected operation.
Long titles stay within their rows.
Actions without a capture show the preceding screenshot with an explicit “last capture” caption.
Images remain visible while another capture loads.

The default capture policy takes stage-boundary and failure captures, not screenshots for every raw input.
Set `SLINT_STUDIO_CAPTURES` to `boundaries`, `failures`, or `none` before launching Studio.
Observers are scoped to pytest, and observer errors become diagnostics without altering test outcomes.
Preflight identity and cache invalidation include the generic library's contents.

A timed-out native response is drained before failure capture uses the same connection.
This cleanup has a separate five-second ceiling and never resends the action.
If cleanup cannot recover framing, the connection closes and reporting explains the unavailable capture.

## Initial Phase 2 Validation

Validated on macOS with the existing feature-enabled editor binary and `headless-skia`:

- Complete editor suite: **655 passed**, including 642 original cases and 13 readable cases.
- Generic library: **23 passed**, including an independent native application, held-input cleanup, parent death, stubborn descendants, and partial-response timeout recovery.
- Studio: **53 passed**, including action replay, capture policies, selection stability, and library cache invalidation.
- Ruff formatting/lint and ty checks use each project's declared Python version and the existing editor/Studio runtimes.
- Native Studio discovered the complete suite and ran all **13 readable cases successfully**, storing **226 actions and 39 captures**.
  Its exact IDs and passed outcomes match direct pytest.
  One local sample took 14.64 seconds of test time directly and 28.43 seconds with Studio boundary captures; this comparison does not isolate tracing overhead.
- Native UI inspection covered group expansion, action selection, details, capture display, and restart with the same project, test, selected action, and saved capture.
- Temporary acceptance tests verified a failed assertion retains its screenshot, rerun-failed selects that failure, and Stop cancels a gesture while its pointer and Shift are held.
  The saved failure was reopened through Recent Runs.
  Process inspection confirmed no owned editor or launch-helper processes remained.
- A temporary fault campaign produced **12 expected failures**: premature source writes, stale source restoration, incorrect coordinate expectations, and a seam value changed to 7 degrees. The temporary copies were removed.

The full suite ran before the final transport framing fix; the affected 13 readable cases were rerun against the final library.
The 642 original cases don't use that transport adapter.
No Rust or editor-shell production code changed in Phase 2; Rust builds, Clippy, and the monorepo-wide autofix task were not run for these Python/Studio changes.

## Native Pointer Follow-Up

The generic library now negotiates version 1 of a native left-click targeting contract.
Click-derived actions check transformed centers, ancestor clipping, foreground input items,
child popups, and active pointer or scroll capture before input.
They reveal instantiated targets through nested interactive Flickables.
The native click checks again after hover callbacks; a redirected or replaced target receives no press.
A confirmed replacement before input can be retried within the original deadline.

Read-only queries never invoke input filters or move the pointer.
Unknown item policies fail closed, and native top-level popup routing remains unsupported.
Virtualized rows that aren't instantiated remain unavailable.
Hover and held drags retain basic readiness; this contract is specifically for left clicks.
Older binaries keep basic readiness and report unverified targeting.
No pointer method silently invokes an accessibility action.

The generic fixture covers overlays, hover-triggered obstructions, fixed clips, nested scrolling,
transformed targets, popup routing, text input inside drop regions, cancellation, and unsupported policies.
A second fixture verifies compatibility with the older installed Python runtime.
Wire-schema parity and Studio preflight invalidation have automated coverage.
See [the generic API guide](../slint-test/README.md) for commands and limitations.

Process ownership is implemented for POSIX and validated on macOS.
Broader adapter migration, multi-window inspection, live refresh, and recording remain follow-up work.

### Follow-Up Validation

Validated on macOS with headless Skia and the rebuilt editor:

- Editor UI: 655 passed; editor Rust: 213 passed.
- Generic library: 59 passed; Studio: 68 passed.
- Native testing backend: 77 passed, including doctests and schema parity.
- Native Studio ran and reran four filtered inspector cases, recorded verified target details,
  and displayed their saved captures. The centered-scroll fixture was inspected as a render.
- Ruff, ty, repository autofixes, and strict backend and root-workspace Clippy passed.
- The full repository Clippy invocation encountered an unrelated FFmpeg/header incompatibility
  in the examples workspace. Windows and Linux runtime behavior wasn't validated.

## Complex Gesture Migration

Three existing test families now use the readable API in place, retaining all 16 node IDs and parameter cases.
The five earlier pilot families remain alongside their original tests.

- Rotated resizing checks multiple transient preview states, pointer tracking, a fixed opposite corner, unchanged source before release, exact saved bytes, and applied preview.
- Nested rotated movement checks transient preview states and the exact local coordinates produced by window-space input.
- Conic cancellation checks picker controls, restored handle coordinates, and sustained unchanged source after closing the picker.

`CanvasElement.drag()` and `handle_center()` apply the existing selection-frame rotation calculation to generic locators and gestures.
They add no assertions or source mutation and return generic `Drag` and `Point` values.
Conic gestures retain an explicit window-space pointer start, as in the original tests and existing readable conic pilot.
Locator-based drag readiness timed out on these handles; this migration does not claim stronger drag actionability.
Picker setup retains the existing shared helper and is labeled as a legacy group without internal tracing.
Named assertion groups expose the preserved invariants in Studio's timeline.

Validation on macOS with headless Skia:

- All 16 original cases passed before migration; all 16 migrated cases passed through Studio.
- The complete editor UI suite passed: 655 tests.
  All 22 conic cases passed again after the final shared-helper cleanup.
- The editor Rust suite passed: 213 tests.
- Incorrect local coordinates, resize dimensions, and restored handle positions each failed in both implementations: six expected failures.
- Ruff formatting/lint and ty passed for the changed Python files.
- Strict Clippy passed for the root, demos, and tests workspaces.
  The examples workspace failed on the existing FFmpeg/header incompatibility.
- Repository autofixes passed after a separate foundation commit corrected spelling/type configuration and recognition of the licensed binary descriptor.
  The complete Clippy script still fails on the examples dependency noted above.
- Native computer control verified the passing batch and an intentional failure paused before teardown, with actual and expected coordinates visible.
- A breakpoint stopped the resize after pointer movement but before release.
  Studio Stop marked it cancelled and left no owned editor or test processes running.
