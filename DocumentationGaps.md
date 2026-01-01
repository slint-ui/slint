# Documentation Gaps Analysis

This document identifies gaps in the Slint project documentation based on an analysis conducted January 2026.

## Missing Documentation

### Platform/Deployment
- No iOS getting-started template (unlike Rust, C++, Node.js, Python)
- No production deployment guide (packaging, signing, distribution)
- No WebAssembly build toolchain documentation

### Tools
- `tools/updater/` - Only 23 lines, no migration examples or rollback procedures
- `tools/figma-inspector/` - Not mentioned in main README
- `tools/docsnapper/` - Purpose unclear, no usage examples

### Examples
- `examples/fancy_demo/` - No README despite being a substantial example
- `examples/cpp/` - No explanation of `platform_native` vs `platform_qt` differences
- ~20 examples not featured in the examples README (wgpu_texture, servo, safe-ui, uefi-demo, MCU examples)

## Incomplete Documentation

### API READMEs
- **Python**: No error handling docs, limited async examples
- **Node.js**: No TypeScript docs, no memory management guidance
- **C++**: Cross-compilation section missing troubleshooting and Windows examples
- **Rust**: Minimal - mostly external links

### Guides
- No threading/concurrency guide across language bindings
- No guide on structuring large apps with modules
- No custom widget library/publishing patterns

## Developer Onboarding Gaps

### Architecture
`docs/development.md` covers repo structure but lacks:
- Data flow diagrams
- Compiler pipeline explanation (lexing → parsing → codegen)
- Property/signal propagation through the system
- Backend selection logic

### Build Configuration
- No decision matrix for renderer selection (skia vs femtovg vs software)
- Backend feature flags (`backend-winit-x11` vs `backend-winit-wayland`) - unclear when to use which
- No verification guide for which backend is actually being used

## Outdated/Inconsistent Information

- Node.js version requirement says "v16" (outdated)
- `docs/embedded-tutorials.md` is a template file with no actual tutorials
- Python marked Beta in its README but featured equally in main README

## Underdocumented Features

- Weak references (Rust API)
- Custom renderer implementation
- Model filtering/sorting
- Animation timing and performance
- Direct OpenGL/graphics integration (examples exist, no systematic guide)
