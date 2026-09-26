# Slint Test Studio

A native desktop workbench for Visual Editor pytest tests.
Studio discovers the complete suite, validates an existing editor build, and preserves inspectable run results.
Tests remain ordinary Python files that run independently through pytest.

See the [roadmap](ROADMAP.md) for later authoring, debugging, and recording milestones.

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
Discovery paths are relative to `tools/editor/ui-tests`, one per line.
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

Run details contain named stages and captured screenshots.
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
Recording, action debugging, parallel workers, source editing, and portable CI bundles remain future milestones.

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
