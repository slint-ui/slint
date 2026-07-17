# AGENTS.md

This file provides guidance to AI coding assistants when working with code in this repository.

Slint is a declarative GUI toolkit for embedded, desktop, mobile, and web.
UIs are written in `.slint` markup and connected to Rust, C++, JavaScript, or Python business logic.

## Build Commands

### Rust (Primary)

The repository is split into separate workspaces that share one `target/`
directory (configured in `.cargo/config.toml`): the root workspace holds the
library and tool crates, while `examples/`, `demos/`, `tests/` and
`ui-libraries/material/` are each their own workspace. Keeping examples/demos/
tests out of the root workspace keeps rust-analyzer fast; the shared `target/`
means the common library crates are only built once across all of them. Select
a non-root workspace with `--manifest-path <dir>/Cargo.toml`.

```sh
cargo build                                    # Build the root (library/tool) workspace
cargo build --release                          # Release build (use whenever measuring performance)
cargo test                                     # Run the root workspace tests
cargo build --manifest-path examples/Cargo.toml --workspace \
    --exclude mcu-board-support --exclude mcu-embassy --exclude uefi-demo   # Build the examples
```

### Running Examples
```sh
cargo run --manifest-path examples/Cargo.toml -p gallery   # Run the gallery example
cargo run --bin slint-viewer -- path/to/file.slint         # View a .slint file (root workspace)
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
cd api/node && pnpm install && pnpm build
```

## Testing

Don't run `cargo build` before `cargo test` — `cargo test` compiles what it needs.

### Test Drivers
The integration tests live in the `tests/` workspace, so pass
`--manifest-path tests/Cargo.toml`:
```sh
cargo test --manifest-path tests/Cargo.toml -p test-driver-interpreter   # Fastest: interpreter-based
cargo test --manifest-path tests/Cargo.toml -p test-driver-rust          # Rust API (slow to compile without SLINT_TEST_FILTER)
cargo test --manifest-path tests/Cargo.toml -p test-driver-cpp           # C++ (build slint-cpp first for the dynamic library)
cargo test --manifest-path tests/Cargo.toml -p test-driver-nodejs        # Node.js
cargo test --manifest-path tests/Cargo.toml -p test-driver-python        # Python
cargo test --manifest-path tests/Cargo.toml -p doctests                  # Documentation snippets
```

### Filtering .slint Test Cases

The test drivers compile every `.slint` file under `tests/cases/` into a generated Rust test, which is slow.
Set `SLINT_TEST_FILTER=<substring>` to limit the build to matching case files.

```sh
tests/run_tests.sh rust layout                 # Filter by name via the helper script
```

Only drop the filter for a final full-suite run before committing.

### Writing Slint Test Cases

The `test` property in `tests/cases/*.slint` must be declared `out` or `in-out`
(e.g. `out property<bool> test: ...;`), otherwise the driver passes the test vacuously.

### Syntax Tests (Compiler Errors)
```sh
cargo test -p i-slint-compiler --features display-diagnostics --test syntax_tests
SLINT_SYNTAX_TEST_UPDATE=1 cargo test -p i-slint-compiler --test syntax_tests  # Update expected errors
```

### Screenshot Tests
```sh
cargo test --manifest-path tests/Cargo.toml -p test-driver-screenshots                    # Compare against references
SLINT_CREATE_SCREENSHOTS=1 cargo test --manifest-path tests/Cargo.toml -p test-driver-screenshots  # Generate references
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

`lsp/` (LSP server), `compiler/` (CLI), `viewer/` (hot-reload `.slint` viewer), `slintpad/` (web playground), `figma_import/` (Figma-to-Slint conversion), `figma-inspector/` (Figma plugin: shows Slint markup for a selected design element), `tr-extractor/` (i18n).

### Editor Support (`editors/`)

`vscode/`, `zed/`, `kate/`, `sublime/`, `tree-sitter-slint/`.

### Key Patterns

- Internal crates (`internal/`) are not semver-stable - they use exact version pinning
- FFI modules are gated with `#[cfg(feature = "ffi")]`
- C++ headers generated automatically during the build via `cbindgen` (invoked by `slint-cpp/build.rs`).
- Extensive Cargo features control renderers (`renderer-femtovg`, `renderer-skia`, `renderer-software`) and backends (`backend-winit`, `backend-qt`)

## Language Design Principles

### CSS Alignment

Slint's `.slint` language intentionally stays close to CSS syntax for visual properties. When adding or extending language features, prefer CSS-compatible syntax and naming so that web developers find familiar patterns and the learning curve stays low.

Examples already in place:
- **Color literals** follow CSS syntax (`#rrggbb`, `#rgb`, named colors, `rgb()`, `rgba()`, `hsl()`, `hsla()`).
- **Gradient syntax** mirrors CSS: `@linear-gradient(angle, color stop, ...)`, `@radial-gradient(...)`.
- **FlexboxLayout** implements the CSS flexbox model (via the `taffy` crate); property names map closely to their CSS counterparts.
- **Filter/shadow properties** (`drop-shadow`, `box-shadow`, `blur`) follow CSS conventions.

When this principle applies: any time you design syntax for a new visual or layout property, check how CSS spells it first. Deviate only when Slint's type system or consistency with existing Slint naming requires it, and document the divergence.

## Version Control (Git)

- The default git branch is `master`.
- Prefer linear history — rebase or squash on merge.
- During review, prefer adding small follow-up commits over amending, so the reviewer can
  track how feedback was incorporated; squash them once the review is complete. See
  [`docs/development.md`](docs/development.md#commit-history--code-reviews) for the full
  fixup-then-squash workflow, and for `mise`-based environment setup.

## Code Style

- Rust: `rustfmt` enforced in CI.
- C++: `clang-format` enforced in CI.

### Comments and Docs

- **Always follow the [Writing Style Guide](docs/internal/writing-style-guide.md) for all new comments, doc comments, commit messages, and markdown.** It applies to code comments (internal and public API) as much as to prose. This is a requirement, not a suggestion.
- But don't reformat existing prose just to match the style.

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
- `mcp-server.md` — `internal/backends/testing/mcp_server.rs`, `introspection/`: shared introspection layer, handle/arena, HTTP transport, adding tools.
- `ffi-language-bindings.md` — `api/`, internal FFI: cbindgen, FFI patterns, adding cross-language APIs.
