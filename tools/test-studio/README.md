# Slint Test Studio

A native desktop workbench for Visual Editor pytest tests.
Studio discovers the complete suite, validates an existing editor build, and preserves inspectable run results.
Tests remain ordinary Python files that run independently through pytest.

Studio includes readable Python actions, an optional Visual Editor adapter, and nested action reporting.
It also includes a snapshot inspector and action debugger.
See [the generic API](../slint-test/README.md) and [the roadmap](ROADMAP.md).

## Set Up

Install Studio's environment explicitly:

```sh
cd tools/test-studio
uv sync --locked
```

Studio uses Primer Slint components from `github-app`, pinned to revision `17ef19d5582858f1c7a91dd677dbf537941e2b1a`.
Prepare a separate checkout at that revision:

```sh
git -C /path/to/github-app worktree add --detach /path/to/studio-primer 17ef19d5582858f1c7a91dd677dbf537941e2b1a
```

Run Studio with an existing test environment and editor binary:

```sh
./run.command --repo /path/to/gb-slint \
  --test-python /path/to/ui-tests/.venv/bin/python \
  --editor-binary /path/to/slint-editor \
  --components /path/to/studio-primer/packages/primer-slint
```

The component path defaults to `~/slint/github-app/packages/primer-slint`.
`SLINT_PRIMER_DIR` also selects it.
Studio reports changed revisions and local component edits as overrides.
It doesn't modify the component checkout.

Prepare the editor's test environment separately with `uv sync --locked` in `tools/editor/ui-tests`.
Build the editor from a checkout containing the capability command:

```sh
SLINT_EMIT_DEBUG_INFO=1 SLINT_ENABLE_EXPERIMENTAL_FEATURES=1 \
  cargo build --locked -p slint-editor --all-features --features slint/mcp
```

Studio validates paths, imports, compiled capabilities, connection, window discovery, element inspection, and screenshot capture before running tests.
Unsupported binaries produce setup instructions without opening an editor window.
Successful probes are cached until the interpreter, binary, backend, packages, or harness changes.
Discovery works without a supported editor binary.
Studio never builds the editor or installs environments automatically.

For a local macOS launcher, run `uv run make_launcher.py` and open `.local/Slint Test Studio.app`.
The launcher depends on this checkout, its Python environment, and the component library.
It isn't a distributable application bundle.

## Use

**Project settings** configures the checkout, interpreter, binary, discovery paths, backend, and history retention.
Discovery paths accept absolute paths or paths relative to `tools/editor/ui-tests`, one per line.
External tests keep absolute node IDs so collection, execution, and reruns resolve the same files.
The default `tests` path discovers the complete suite.
Use the recent-project selector to switch projects while idle.

The tree groups files, classes, tests, and parameter cases.
Search by name or node ID, filter by marker name, or choose an outcome.
Arrow keys navigate the tree and expand or collapse groups.

- **Run selected** runs the selected test or matching descendants of a selected group.
- **Run shown** runs every matching test, including collapsed descendants.
- **Rerun failed** uses the selected run's failures and errors, independent of search filters.
  Tests removed since that run are listed explicitly.
- **Stop** cancels discovery, preflight, or execution and cleans up application subprocesses.

One project and one sequential operation are active at a time.
The default editor backend is headless Skia.
Visible editor windows require selecting the visible backend in settings.

Run details contain named stages, nested actions and assertions, and captured screenshots.
Expand a group using its arrow; select an action and open Output for its locator, input arguments, source location, wait diagnostics, and outcome.
Helpers retained from the old harness are labeled when their internal actions are not traced.
Capture-free actions show the preceding screenshot with a “last capture” label.
Set `SLINT_STUDIO_CAPTURES=boundaries` (default), `failures`, or `none` before launching Studio to control capture overhead.
Raw pointer/key events never capture automatically.
The preview shows historical captures, not live video.
Selecting a capture holds that selection while more results arrive.
The Source tab colors Python syntax for light and dark themes while preserving indentation and native text selection.
Use **Copy source** to copy the original Python, including whitespace.
Historical-source labels stay outside the copied code.
The Output tab also supports selecting and copying text.
Environment records the checkout revision, dirty state, binary hash, Python/package versions, backend, and capabilities.
The binary's source revision is not inferred from the checkout revision.

Setup and teardown errors are distinct from assertion failures.
Expected failures, unexpected passes, skips, cancellations, and confirmed application crashes have separate outcomes.
A cancelled batch retains completed results and leaves unstarted tests marked **Not run**.
Failed discovery preserves the previous tree with a stale indicator.

## Inspect And Debug

Select an instrumented test and choose **Debug selected**.
Studio pauses before its first action after application launch.
The **Inspect & debug** tab shows the source line, a capture, searchable elements, properties, and locator suggestions.
Suggestions are unique within the captured visible element set; actions still resolve their locators strictly when executed.
Click the capture to pick by bounds, or click again to cycle overlapping candidates.
Picking and highlights don't establish clipping, rotation, or which element receives pointer input.

**Step over** completes the selected action and its children, then pauses before the next action.
**Step into** pauses before the next child or subsequent action.
**Continue** runs until a breakpoint, failure, or requested pause.
**Pause** takes effect at the next instrumented action.
Enter an action-title substring and choose **Set breakpoint**; an empty value clears it.
A failed action pauses before teardown so the application remains inspectable.
Assertion failures lead with a short comparison, such as “Expected 250, observed 200,” followed by the field and timeout.
Choose **Show details** for the complete locator, error, and source path.
**Stop** cleans up the test process even while paused with input held.

Generic operation deadlines and action durations exclude debugger pauses.
Application timers, animations, arbitrary Python, and legacy helper deadlines keep running.
Tests without action instrumentation run normally and report that debugging was unavailable.

**Refresh view** captures the first application window again while paused.
The view is a snapshot, not a continuously refreshed stream.
The optional Visual Editor adapter supplies **Application source** snapshots of its active file.
Application source uses Slint token coloring, line numbers, and a rounded red outline on a resolved failing binding.
For geometry assertions, the adapter supplies the selected outline element and property; Studio highlights only a unique direct binding in that captured file.
Unresolved, ambiguous, and truncated source snapshots remain readable without a guessed highlight.
**Properties & locator** selects the failing control and outlines the observed property using its transport handle.
Both views scroll to the highlighted line and offer **Copy** for the complete original text.
Other applications can supply sources through `inspection_sources(...)` without depending on Studio or the editor.
Captures and source text remain available after restarting; historical inspections have no live controls.

## Results And Storage

Studio keeps settings and managed run directories in the user's application-data directory:

- macOS: `~/Library/Application Support/Slint Test Studio`
- Linux: `$XDG_DATA_HOME/slint-test-studio`, defaulting to `~/.local/share/slint-test-studio`
- Windows: `%LOCALAPPDATA%/Slint Test Studio`

Use `--data-dir` for an isolated Studio instance or test campaign.
A lock prevents two instances from modifying the same history.
macOS is the validated development platform; Windows and Linux retain portable interfaces without a platform-validation claim.

Each run includes metadata, a versioned event journal, source snapshots, logs, captures, and temporary test fixtures.
Recent Runs opens historical results, pins important runs, and deletes unpinned completed runs.
Historical source is labeled and kept separate from current discovery.
The Artifacts button works even when a run has no screenshots.
Collection uses transient managed storage.
Studio discards completed collection artifacts after updating the current test tree, so discovery doesn't consume run retention.

Default retention is 20 completed runs, 30 days, and a 5 GiB storage target.
Studio prunes oldest unpinned completed runs after operations and settings changes.
Active and pinned runs are protected; they may exceed the storage target, producing a warning.
Interrupted runs are recovered on restart.
Malformed history and missing captures produce diagnostics without preventing other runs from opening.

Existing `local.json` settings are imported once.
Command-line project overrides apply only to the current session.
Saving Project Settings explicitly makes those settings persistent.

## Reporting Integration

The editor harness exposes optional reporting observers through `ui_reporting`.
Observers receive application ready/closing/exit and stage start/end events.
The bridge installs one observer for its pytest session and resets it afterward.
Legacy harness stages are translated into the same action timeline as readable tests.
Observer failures cannot change test outcomes.
Older harnesses run with a visible warning and no detailed captures.

The editor's `--test-capabilities` command prints schema version 1 before initializing any backend.
It reports the editor version, system-testing transport, and compiled backend/renderer combinations.
The subsequent runtime probe verifies initialization and inspection on the current machine.

Studio events use schema version 1 with run ID, sequence, timestamp, kind, and optional pytest node ID.
Screenshot paths are relative to their run directory.
The same state reducer drives live results and restored history.

Screenshot capture adds execution time.
Use ordinary pytest for performance measurements and record capture overhead separately.
Recording, parallel workers, source editing, and portable CI bundles remain future milestones.

## Validate

```sh
uv run pytest
uv run ruff check .
uv run ruff format --check .
uv run ty check --exclude probe.py
uv run ty check probe.py --python ../editor/ui-tests/.venv
```

Run all three code checks from the repository root with `mise run lint:python:test-studio`.
Ruff and ty match the versions pinned by the editor UI tests.
The subprocess probe is checked separately against the editor test environment and its `slint-testing` API.
Set `SLINT_STUDIO_TEST_PYTHON=/path/to/test/python` when that environment lives outside this checkout.
The mise task uses that interpreter for the probe check.
The optional reporting observer is imported from the selected project at runtime.

The tests cover outcomes, discovery, process supervision, cancellation, preflight, observer isolation, history, retention, and UI state.
Run editor Rust tests and the complete headless editor UI suite after changing its capability or reporting hooks.

For native UI inspection through MCP:

```sh
SLINT_EMIT_DEBUG_INFO=1 SLINT_BACKEND=headless-skia SLINT_MCP_PORT=9431 \
  uv run app.py --data-dir /tmp/studio-validation \
  --repo /path/to/gb-slint --editor-binary /path/to/slint-editor
```
