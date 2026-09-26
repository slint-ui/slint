# Inspector And Action Debugger: First Increment

## Delivered

Studio adds Debug selected, Pause, Continue, Step over, Step into, and action-title breakpoints.
Failure pauses occur before action unwinding and application teardown.
Pauses exclude time from generic operation deadlines and action durations.
They don't freeze application timers or arbitrary Python code.

The inspector searches visible elements, selects bounds in a capture, shows properties, and suggests unique locators.
Repeated clicks cycle overlapping bounds.
Captures and element data swap together after background image loading.
A selected unique locator is retained when a refreshed snapshot still contains it.
The optional editor adapter supplies the active source file; generic inspection contains no editor concepts.

Commands use a versioned, sequenced journal scoped to one run.
Resume commands also identify the pause, preventing delayed clicks from resuming a later pause.
The pytest thread handles inspection and input transport sequentially.
The Studio service writes commands off the UI thread.
Stop and parent-liveness supervision continue to own process cleanup.

Inspection captures, source snapshots, and pause events survive restart.
Recovered interrupted runs cannot become live debugger sessions.
Missing captures remain diagnostic errors rather than blocking other history.

## Validation

Automated coverage includes nested stepping, stale commands, partial command writes, early command-consumer exit, failure preservation, and suspended deadlines.
Native generic tests cover locator uniqueness, duplicate names, bounded inspection, source-context restoration, and scale conversion.
View tests cover overlapping picking, logical coordinate mapping, stale image results, and historical control isolation.

The macOS headless native UI acceptance workflow:

1. Discover the configured suite and debug the readable resize test.
2. Set a Drag breakpoint, inspect the resize handle, and read its unique locator.
3. Step over the gesture, verify the source changed to 200 px, and continue to a passing result.
4. Pause on a deliberate assertion failure before teardown, inspect it, and continue to pytest exit 1.
5. Stop while a drag holds pointer and Shift; verify no owned application process remains.
6. Kill Studio during another held drag; verify the supervisor cleans up its process group.
7. Restart Studio, recover Interrupted history, and reopen the saved inspection with live controls disabled.

Temporary failure and cancellation cases were removed after acceptance.
The original editor suite remains unchanged apart from the existing Phase 2 pilots and optional adapter reporting.
No Rust backend implementation or build changed in this increment.

### Capture Cost

A sequential macOS headless sample ran all 13 readable editor cases through three paths; all passed with the same selected IDs.
Direct pytest took 14.5 seconds.
Studio tracing with captures disabled took 14.0 seconds and emitted 226 actions.
Studio boundary capture took 28.3 seconds, including 14.9 seconds in 39 capture calls.
This single sample identifies capture cost; it doesn't establish a statistically meaningful tracing speed difference.
Debugger inspection captures are separate from ordinary boundary capture policy and record their own duration.

## Remaining Scope

This is the first inspector/debugger increment, not all of milestone 3.
Continuous live refresh, multi-window selection, and opening an external editor at a source line remain follow-ups.
Action-title breakpoints aren't arbitrary Python breakpoints.
Legacy helpers only pause at their instrumented boundaries.

Native input currently has no read-only target query.
Calling input filters as a probe would mutate application state.
Hit-target verification, effective clipping, transformed highlights, and automatic scrolling therefore remain unavailable.
The inspector explicitly labels bounds-based picking and doesn't claim these stronger contracts.
Establish the native contract before enabling those capabilities in the Python API.
