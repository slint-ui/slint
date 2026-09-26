# Slint Test Studio

A native desktop app for browsing and running Visual Editor pytest tests.
The UI uses the Primer Slint components from `github-app`.
The app runs separately from the editor and doesn't modify its tests.

See the [roadmap](ROADMAP.md) for planned milestones and completion criteria.

## Run

Install the app environment with `uv sync --locked` in this directory.
Then run:

```sh
./run.command --repo /path/to/gb-slint \
  --components /path/to/github-app/packages/primer-slint \
  --editor-binary /path/to/test-enabled/slint-editor
```

`--repo` selects the checkout whose tests run.
The default test interpreter is that checkout's `tools/editor/ui-tests/.venv/bin/python`.
Use `--test-python` to select another prepared environment.
On Windows, launch `uv run app.py` and use the equivalent interpreter path.

Build the editor with system testing, headless support, and debug information:

```sh
SLINT_EMIT_DEBUG_INFO=1 SLINT_ENABLE_EXPERIMENTAL_FEATURES=1 \
  cargo build --locked -p slint-editor --all-features --features slint/mcp
```

The app defaults to headless editor tests.
Use **Editor: headless** to switch to visible editor windows for manual observation.
The app itself is always a desktop window unless a headless backend is explicitly selected.

Optional `local.json` settings preserve local paths:

```json
{
  "repo": "/path/to/gb-slint",
  "test_python": "/path/to/ui-tests/.venv/bin/python",
  "editor_binary": "/path/to/test-enabled/slint-editor"
}
```

Command-line options override these settings.
Set `SLINT_PRIMER_DIR` or pass `--components` for the component library.
The default component path is `~/slint/github-app/packages/primer-slint`.
The app imports that library directly; it doesn't copy or modify the components.

For a double-clickable macOS launcher, run `uv run make_launcher.py`.
Open `.local/Slint Test Studio.app`.
This is a local development launcher that depends on this checkout, its Python environment, and the component library.
It isn't a distributable application bundle.

## Use

- Browse 5 suites: undo/redo, startup, selection, navigation, and canvas zoom.
  Discovery uses pytest, including parameterized cases.
- Search by test name, suite, or parameter, and filter failed tests.
- Run one test or all tests currently shown.
- Stop a run, including editor subprocesses.
- Select execution steps to inspect their screenshots.
- Read the actual Python source and pytest failure output.
- Switch between light and dark themes.

The bridge captures editor launch, existing `replay_stage` checkpoints, and the final state before the editor closes.
Tests without named checkpoints show launch and final captures.
The preview displays captured states, not live video.
Screenshot capture adds execution time, so these runs aren't performance benchmarks.

Each run keeps screenshots, structured events, pytest output, and temporary fixtures in a `slint-test-studio-*` system temporary directory.
Use **Artifacts** to open a run's directory.
These artifacts remain after closing the app; remove old run directories when no longer needed.

Recording new tests, breakpoints, live stepping, and editing test source aren't implemented in this first version.
The existing Python tests remain the source of truth.

## Validate

```sh
uv run pytest
```

The runner tests cover parameterized discovery, reporting, setup failures, skipped tests, partial event writes, and cancellation of child processes.

To inspect the app through Slint's MCP interface:

```sh
SLINT_EMIT_DEBUG_INFO=1 SLINT_BACKEND=headless-skia SLINT_MCP_PORT=9419 \
  uv run app.py --repo /path/to/gb-slint --editor-binary /path/to/slint-editor
```
