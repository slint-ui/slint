# Python Test Infrastructure

There are two separate test systems for Python:

## 1. Python-native tests (pytest)

Located in `api/python/slint/tests/`. These test the `slint` Python API directly.

```sh
cd api/python/slint && uv run pytest -s -v
```

This automatically builds the `slint-python` shared library via maturin if needed.

## 2. Rust test driver (`test-driver-python`)

The Rust test driver (`tests/driver/python/python.rs`) processes `.slint` test cases from `tests/cases/`:

1. **Compiles the `.slint` file** using `OutputFormat::Python` (via `compile_syntax_node` + `generator::generate`). This goes through LLR lowering and generates a `.py` file.

2. **Runs the generated `.py` file** as a subprocess using `uv run`. The subprocess loads the `slint` Python module, which re-compiles the `.slint` source using `slint-interpreter` (with full inlining enabled).

Run with:
```sh
cargo test -p test-driver-python
# or with a filter:
cargo test -p test-driver-python -- test_name
```

## Rebuilding slint-python

The `slint-python` shared library used by the Python subprocess is built by `uv sync` in `api/python/slint/`, not by `cargo build`. It's installed into a Python venv managed by `uv`.

To force a rebuild after code changes:

```sh
cd api/python/slint && uv sync --reinstall-package slint && cd -
```

`cargo build -p slint-python` builds a separate artifact that is NOT used by the Python tests. The test driver's `LazyLock<PYTHON_PATH>` calls `uv sync` once per test run, but this may not detect source changes.

## Debugging

Issues can occur in two places:

- **Test driver process** (compilation): the test driver compiles the `.slint` source with `OutputFormat::Python` and generates the `.py` file. To debug, modify compiler code and rebuild with `cargo build -p test-driver-python`.

- **Python subprocess** (runtime): the `slint` Python module re-compiles the `.slint` source via `slint-interpreter` and executes it. To debug, modify compiler/runtime code AND rebuild slint-python (`cd api/python/slint && uv sync --reinstall-package slint`).

The subprocess's STDERR/STDOUT is captured and printed in the test output under `STDERR:` / `STDOUT:` headers.
