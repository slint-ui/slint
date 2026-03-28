# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project Overview

Slint is a declarative GUI toolkit for Rust, C++, JavaScript, and Python. UIs are defined in `.slint` markup files and compiled to native code or interpreted at runtime. The project targets embedded, desktop, mobile, and web platforms.

## Build Commands

**Important**: Always run `cargo build` before `cargo test` — the dynamic library must exist first.

```sh
cargo build                                    # Build workspace
cargo build --release                          # Release build (required for performance testing)
cargo run -p gallery                           # Run the gallery example
cargo run --bin slint-viewer -- path/to.slint  # View a .slint file with hot reload
cargo xtask cbindgen                           # Regenerate C++ headers from Rust FFI
```

### C++ (CMake)
```sh
cargo build --lib -p slint-cpp                 # Build C++ library
mkdir cppbuild && cd cppbuild && cmake -GNinja .. && cmake --build .
```

### Node.js
```sh
cd api/node && pnpm install
```

## Testing

### Test Drivers (fastest to slowest)
```sh
cargo test -p test-driver-interpreter          # Interpreter — fastest, no compilation
cargo test -p test-driver-rust                 # Rust API via slint! macro
cargo test -p test-driver-cpp                  # C++ (build slint-cpp first!)
cargo test -p test-driver-nodejs               # Node.js
cargo test -p doctests                         # Doc snippet validation
```

### Filtered Testing
```sh
SLINT_TEST_FILTER=layout cargo test -p test-driver-rust
tests/run_tests.sh rust layout
```

### Syntax Tests (compiler error messages)
```sh
cargo test -p i-slint-compiler --features display-diagnostics --test syntax_tests
SLINT_SYNTAX_TEST_UPDATE=1 cargo test -p i-slint-compiler --test syntax_tests  # update expected
```

Test files in `internal/compiler/tests/syntax/` use `> <error{message}` comments to mark expected errors.

### Screenshot Tests (visual regression)
```sh
cargo test -p test-driver-screenshots                               # compare
SLINT_CREATE_SCREENSHOTS=1 cargo test -p test-driver-screenshots    # generate references
```

## Formatting and Linting

```sh
cargo fmt --all                    # Rust formatting
cargo clippy --all-targets         # Rust linting
mise run ci:autofix:fix            # All languages: Rust, C++, Python, JS/TS, TOML, license headers
mise run ci:autofix:lint           # REUSE compliance + TypeScript type checking
cargo xtask check_license_headers --fix-it  # Fix license headers only
```

Environment setup with mise: `mise trust -a && mise install`

Pre-commit hook: `mise generate git-pre-commit --write --task=ci:autofix:fix`

## Architecture

### Compiler Pipeline (`internal/compiler/`)

```
.slint source → Lexer → Parser (Rowan CST) → Object Tree → ~50 Passes → LLR → Code Generator
```

The compiler transforms `.slint` files through a high-level IR (object tree), runs optimization/transformation passes, lowers to LLR (Low-Level Representation), then generates target code (Rust, C++, Python, JS). Nothing in the compiler depends on runtime crates.

### Runtime (`internal/core/`)

Reactive property system with automatic dependency tracking. `Property<T>` values lazily recompute when dependencies change. The runtime manages layout solving, animations, text layout, input events, and accessibility.

### Two Execution Modes

- **Compiled**: `slint!` macro or `slint-build` in build.rs generates static Rust code. C++ uses generated .h files. Maximum performance.
- **Interpreted**: `internal/interpreter/` loads .slint at runtime via dynamic dispatch. Used by the LSP, viewer, and scripting language bindings.

### Backends (`internal/backends/`) — Platform integration
- `winit/` — primary cross-platform backend
- `qt/` — Qt integration (used when `qmake` is in PATH)
- `android-activity/` — Android
- `linuxkms/` — direct Linux framebuffer rendering
- `testing/` — headless backend for automated tests
- `selector/` — runtime backend selection

### Renderers (`internal/renderers/`) — Drawing engines
- `femtovg/` — OpenGL ES 2.0
- `skia/` — Skia graphics library
- `software/` — CPU-only, no GPU required

### Language APIs (`api/`)
- `rs/slint/` — public Rust crate; `rs/macros/` — `slint!` proc macro; `rs/build/` — build script
- `cpp/` — C++ with CMake; FFI via `#[repr(C)]` + cbindgen; gated with `#[cfg(feature = "ffi")]`
- `node/` — Node.js (Neon/N-API)
- `python/` — Python (PyO3)
- `wasm-interpreter/` — WebAssembly for browser

### Tools (`tools/`)
- `lsp/` — Language Server Protocol implementation
- `viewer/` — .slint viewer with hot reload
- `compiler/` — CLI compiler for ahead-of-time compilation
- `figma_import/` — Figma design import
- `updater/` — version migration tool

## Key Patterns

- Internal crates use exact version pinning (e.g., `"=1.16.0"`), not semver ranges
- Cargo features control renderers (`renderer-femtovg`, `renderer-skia`, `renderer-software`) and backends (`backend-winit`, `backend-qt`)
- FFI modules live in `ffi` submodules gated with `#[cfg(feature = "ffi")]`
- Test `.slint` files embed test code in comments: ` ```rust ` blocks for Rust, ` ```cpp ` for C++, ` ```js ` for Node.js
- A test component with `in-out property <bool> test: true;` is auto-verified by the interpreter driver
- Workspace resolver is "3"; minimum Rust version is 1.88; edition 2024
- Linear git history preferred (rebase/squash merges); default branch is `master`

## Deep Dive Documentation

For detailed architecture docs, see `docs/development/`:
- `compiler-internals.md` — compiler pipeline, passes, LLR, code generation
- `type-system.md` — type definitions, conversions, name resolution
- `property-binding-deep-dive.md` — reactive property system internals
- `custom-renderer.md` — renderer traits and drawing API
- `layout-system.md` — layout solving and constraints
- `item-tree.md` — component instantiation and traversal
- `input-event-system.md` — mouse/touch/keyboard event routing
- `ffi-language-bindings.md` — C++/Node.js/Python binding patterns
- `lsp-architecture.md` — LSP server and IDE integration
