# AGENTS.md

This file provides guidance to AI coding assistants when working with code in this repository.

Slint is a declarative GUI toolkit for embedded, desktop, mobile, and web.
UIs are written in `.slint` markup and connected to Rust, C++, JavaScript, or Python business logic.

## Build Commands

### Rust (Primary)
```sh
cargo build                                    # Build the workspace
cargo build --release                          # Release build (use whenever measuring performance)
cargo test                                     # Run tests
cargo build --workspace --exclude uefi-demo    # Build all examples
```

### Running Examples
```sh
cargo run -p gallery                 # Run the gallery example
cargo run --bin slint-viewer -- path/to/file.slint  # View a .slint file
```

### C++ Build
```sh
cargo build --lib -p slint-cpp                 # Build C++ library
mkdir cppbuild && cd cppbuild
cmake -GNinja ..
cmake --build .
```

### Node.js Build
```sh
cd api/node && pnpm install
```

## Testing

Don't run `cargo build` before `cargo test` — `cargo test` compiles what it needs.

### Test Drivers
```sh
cargo test -p test-driver-interpreter         # Fastest: interpreter-based
cargo test -p test-driver-rust                # Rust API (slow to compile without SLINT_TEST_FILTER)
cargo test -p test-driver-cpp                 # C++ (build slint-cpp first for the dynamic library)
cargo test -p test-driver-nodejs              # Node.js
cargo test -p doctests                        # Documentation snippets
```

### Filtering .slint Test Cases

The test drivers compile every `.slint` file under `tests/cases/` into a generated Rust test, which is slow.
Set `SLINT_TEST_FILTER=<substring>` to limit the build to matching case files.

```sh
tests/run_tests.sh rust layout                 # Filter by name via the helper script
```

Only drop the filter for a final full-suite run before committing.

### Writing Slint Test Cases

The `test` property in `tests/cases/*.slint` must be declared `out property<bool> test: ...;`.
Without `out`, the compiler treats it as private and the driver passes the test vacuously.

### Syntax Tests (Compiler Errors)
```sh
cargo test -p i-slint-compiler --features display-diagnostics --test syntax_tests
SLINT_SYNTAX_TEST_UPDATE=1 cargo test -p i-slint-compiler --test syntax_tests  # Update expected errors
```

### Screenshot Tests
```sh
cargo test -p test-driver-screenshots                    # Compare against references
SLINT_CREATE_SCREENSHOTS=1 cargo test -p test-driver-screenshots  # Generate references
```

## Architecture

### Core Components

- **`internal/compiler/`** - Slint language compiler (lexer, parser, code generators)
  - `parser/` - .slint syntax parsing using Rowan
  - `passes/` - Optimization passes
  - `generator/` - Code generators for C++, Rust, Python, JS
  - `tests/syntax/` - Syntax error test cases

- **`internal/core/`** - Runtime library (properties, layout, animations, accessibility)

- **`internal/core-macros/`** - Procedural macros for i-slint-core

- **`internal/common/`** - Shared code and data structures between compiler and runtime

- **`internal/interpreter/`** - Dynamic compilation for scripting languages

- **`internal/backends/`** - Platform windowing/input:
  - `winit/` - Cross-platform (primary)
  - `qt/` - Qt integration
  - `android-activity/` - Android platform support
  - `linuxkms/` - Linux KMS/DRM direct rendering
  - `selector/` - Runtime backend selection
  - `testing/` - Testing backend for automated tests, system testing (protobuf/TCP), and embedded MCP server for AI-assisted UI introspection

- **`internal/renderers/`** - Rendering engines:
  - `femtovg/` - OpenGL ES 2.0
  - `skia/` - Skia graphics
  - `software/` - CPU-only fallback

### Language APIs (`api/`)

Rust (`rs/slint/`, `rs/macros/` for `slint!`, `rs/build/`), C++ (`cpp/`, CMake), Node.js (`node/`, Neon), Python (`python/`, PyO3), WebAssembly (`wasm-interpreter/`).

### Tools (`tools/`)

`lsp/` (LSP server), `compiler/` (CLI), `viewer/` (hot-reload `.slint` viewer), `slintpad/` (web playground), `figma_import/`, `tr-extractor/` (i18n), `updater/` (version migration).

### Editor Support (`editors/`)

`vscode/`, `zed/`, `kate/`, `sublime/`, `tree-sitter-slint/`.

### Key Patterns

- Internal crates (`internal/`) are not semver-stable - they use exact version pinning
- FFI modules are gated with `#[cfg(feature = "ffi")]`
- C++ headers generated via `cargo xtask cbindgen`
- Extensive Cargo features control renderers (`renderer-femtovg`, `renderer-skia`, `renderer-software`) and backends (`backend-winit`, `backend-qt`)

## Version Control (Git)

- The default git branch is `master`.
- Prefer linear history — rebase or squash on merge.

## Code Style

- Rust: `rustfmt` enforced in CI.
- C++: `clang-format` enforced in CI.

### Comments and Docs

- Follow `docs/astro/writing-style-guide.md` when writing *new* comments, doc comments, commit messages, or markdown.
- But don't reformat existing prose to match the style.

## Platform Prerequisites

See [`docs/building.md`](docs/building.md) for Linux/macOS/Windows system packages, FFMPEG setup, and the Windows symlink `git clone` flag.

## Deep Dive Documentation

Load the relevant file under `docs/development/` when working in the listed area:

- `compiler-internals.md` — `internal/compiler/`: pipeline, passes, LLR, codegen.
- `type-system.md` — `langtype.rs`, `lookup.rs`, type checking: unit types, conversions, name resolution, type register.
- `property-binding-deep-dive.md` — `internal/core/properties.rs`, binding bugs: reactivity, dependency tracking, two-way bindings, PropertyTracker, ChangeTracker.
- `custom-renderer.md` — `internal/renderers/`, drawing bugs: renderer traits, drawing API, backend integration.
- `animation-internals.md` — `internal/core/animations.rs`: timing, easing curves, debugging.
- `layout-system.md` — `internal/core/layout.rs`, sizing bugs: constraints, GridLayout/BoxLayout, compile-time lowering.
- `python-tests.md` — Python tests, `test-driver-python`: pytest setup, rebuilding slint-python, compile vs runtime issues.
- `item-tree.md` — `internal/core/item_tree.rs`, event handling, component model: item tree, instantiation, traversal, focus.
- `model-repeater-system.md` — `internal/core/model.rs`, `for` loops: Model trait, VecModel, adapters, Repeater, ListView virtualization.
- `input-event-system.md` — `internal/core/input.rs`, event handling: routing, focus, drag-drop, shortcuts.
- `text-layout.md` — `internal/core/textlayout/`, text rendering: shaping, line breaking, styled text.
- `window-backend-integration.md` — `internal/core/window.rs`, `internal/backends/`: WindowAdapter, Platform, WindowEvent, popups.
- `lsp-architecture.md` — `tools/lsp/`, IDE tooling: completion, hover, semantic tokens, live preview.
- `mcp-server.md` — `internal/backends/testing/mcp_server.rs`, `introspection.rs`: shared introspection layer, handle/arena, HTTP transport, adding tools.
- `ffi-language-bindings.md` — `api/`, internal FFI: cbindgen, FFI patterns, adding cross-language APIs.
