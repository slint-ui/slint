# AGENTS.md

This file provides guidance to AI coding assistants when working with code in this repository.

## Project Overview

Slint is a declarative GUI toolkit for building native user interfaces across embedded systems, desktops, mobile, and web platforms. UIs are written in `.slint` markup files and connected to business logic in Rust, C++, JavaScript, or Python.

## Build Commands

### Rust (Primary)
```sh
cargo build                                    # Build the workspace
cargo build --release                          # Release build
cargo test                                     # Run tests (requires cargo build first!)
cargo build --workspace --exclude uefi-demo --release  # Build all examples
```

### Running Examples
```sh
cargo run --release -p gallery                 # Run the gallery example
cargo run --release --bin slint-viewer -- path/to/file.slint  # View a .slint file
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

**Important**: Run `cargo build` before `cargo test` - the dynamic library must exist first.

### Test Drivers
```sh
cargo test -p test-driver-interpreter         # Fast interpreter tests
cargo test -p test-driver-rust                # Rust API tests
cargo test -p test-driver-cpp                 # C++ tests (build slint-cpp first)
cargo test -p test-driver-nodejs              # Node.js tests
cargo test -p doctests                        # Documentation snippet tests
```

### Filtered Testing
```sh
SLINT_TEST_FILTER=layout cargo test -p test-driver-rust  # Filter by name
```

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
  - `testing/` - Testing backend for automated tests

- **`internal/renderers/`** - Rendering engines:
  - `femtovg/` - OpenGL ES 2.0
  - `skia/` - Skia graphics
  - `software/` - CPU-only fallback

### Language APIs (`api/`)

- `rs/slint/` - Rust public crate
- `rs/macros/` - `slint!` procedural macro
- `rs/build/` - Build script support
- `cpp/` - C++ API with CMake integration
- `node/` - Node.js bindings (Neon)
- `python/` - Python bindings (PyO3)
- `wasm-interpreter/` - WebAssembly bindings for browser use

### Tools

- `tools/lsp/` - Language Server Protocol for editor integration
- `tools/compiler/` - CLI compiler
- `tools/viewer/` - .slint file viewer with hot reload
- `tools/slintpad/` - Web-based Slint editor/playground
- `tools/figma_import/` - Import designs from Figma
- `tools/tr-extractor/` - Translation string extractor for i18n
- `tools/updater/` - Migration tool for Slint version updates

### Editor Support (`editors/`)

- `vscode/` - Visual Studio Code extension
- `zed/` - Zed editor integration
- `kate/` - Kate editor syntax highlighting
- `sublime/` - Sublime Text support
- `tree-sitter-slint/` - Tree-sitter grammar for syntax highlighting

### Key Patterns

- Internal crates (`internal/`) are not semver-stable - they use exact version pinning
- FFI modules are gated with `#[cfg(feature = "ffi")]`
- C++ headers generated via `cargo xtask cbindgen`
- Extensive Cargo features control renderers (`renderer-femtovg`, `renderer-skia`, `renderer-software`) and backends (`backend-winit`, `backend-qt`)

## Code Style

- Rust: `rustfmt` enforced in CI
- C++: `clang-format` enforced in CI
- Linear git history preferred (rebase/squash merges)

## Platform Prerequisites

### Linux
```sh
# Debian/Ubuntu
sudo apt install libxcb-shape0-dev libxcb-xfixes0-dev libxkbcommon-dev \
  libfontconfig-dev libssl-dev clang libavcodec-dev libavformat-dev \
  libavutil-dev libavfilter-dev libavdevice-dev libasound2-dev pkg-config
```

### macOS
```sh
xcode-select --install
brew install pkg-config ffmpeg
```

### Windows
- Enable symlinks: `git clone -c core.symlinks=true https://github.com/slint-ui/slint`
- Install MSVC Build Tools
- FFMPEG via vcpkg or manual installation
