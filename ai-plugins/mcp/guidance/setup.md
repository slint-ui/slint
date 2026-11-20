# Project Setup

Start new projects from the official templates — dependencies and build wiring
set up the way the docs recommend:

- Rust: <https://github.com/slint-ui/slint-rust-template>
- C++ (CMake + `FetchContent`): <https://github.com/slint-ui/slint-cpp-template>
- Node.js: <https://github.com/slint-ui/slint-nodejs-template>
- Python: <https://github.com/slint-ui/slint-python-template>

The moving parts, for orienting in an existing project (details are in each
language's docs section): Rust compiles `.slint` at build time —
`slint-build::compile()` in `build.rs`, `slint::include_modules!()` in main.
C++ uses `slint_target_sources()` and links `Slint::Slint`. Node.js
(`slint-ui` package) and Python (`slint` wheel) load `.slint` files at runtime.

Python note: the `slint` wheel's `requires-python` tracks recent CPython
releases. If `uv add` / `pip install` picks an older Slint than expected, bump
the project's `requires-python` (and `.python-version` for uv) to match the
latest wheel on PyPI before pinning a Slint version.

Preview a `.slint` file without host code: `slint-viewer ui/main.slint`
(hot-reloads on save; install via [tools-install.md](tools-install.md)).

See [interop.md](reference/interop.md) for connecting business logic.
