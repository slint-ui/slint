<!-- Copyright © SixtyFPS GmbH <info@slint.dev> ; SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0 -->

# Counter Example Using slint.codegen

This example mirrors the callback pattern described in the
[Slint Python binding documentation](https://github.com/slint-ui/slint/blob/master/api/python/slint/README.md#readme).
Instead of relying on the runtime auto-loader, it uses the experimental
`slint.codegen` CLI to emit static Python modules (`.py`/`.pyi`) for the
`counter.slint` UI and then subclasses the generated component.

## Steps

1. Generate the bindings:

   ```bash
   uv run python examples/counter/generate.py
   ```

   This produces `examples/counter/generated/counter.py` and
   `examples/counter/generated/counter.pyi` alongside a copy of the
   source `.slint` file, all ready for import.

2. Run the app:

   ```bash
   uv run python -m examples.counter.main
   ```

   Each click anywhere in the window increments the counter via the
   `request_increase` callback implemented in Python.

> **Tip:** The generated `.pyi` file makes the `CounterWindow` API visible to
> type checkers and IDEs, providing a smoother developer experience compared to
> using the dynamic import hook.
