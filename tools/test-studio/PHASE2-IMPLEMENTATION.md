# Phase 2 Implementation

Phase 2 now has a generic Python API, an optional Visual Editor adapter, and a nested action timeline in Studio.
The original editor tests remain intact alongside five readable pilot families.
This is an experimental authoring API; recording and interactive debugging remain later milestones.

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

## Validation

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

## Remaining Native Work

Slint's current testing transport doesn't expose complete hit testing, effective clipping, or scrolling into view.
Basic click readiness checks unique resolution, enabled state, opacity, nonzero size, and stable bounds.
It cannot prove that a covered target receives input.
`click(require_hit_target=True)` rejects unsupported strict hit testing before dispatch.
No pointer method silently invokes an accessibility action.

Accessibility operations deliberately check resolution only because the transport represents unspecified accessible-enabled state as false.
Assertions remain read-only; editor field reveal is explicit.
Process ownership is implemented for POSIX and validated on macOS, not Windows.
Screenshots and action properties don't provide historical DOM-style inspection or live rewind.
Full native actionability, broader adapter migration, recording, and interactive debugging remain follow-up work.
