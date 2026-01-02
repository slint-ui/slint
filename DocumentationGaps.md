# Documentation Gaps Analysis

This document identifies gaps in the Slint project documentation based on an analysis conducted January 2026.

## Fixed

### Tools (Fixed)
- ~~`tools/updater/` - Only 23 lines, no migration examples or rollback procedures~~ → README expanded with CLI options, transformation examples, and supported file types
- ~~`tools/figma-inspector/` - Not mentioned in main README~~ → README rewritten with features, supported elements, converted properties, and Figma variables documentation
- ~~`tools/docsnapper/` - Purpose unclear, no usage examples~~ → README rewritten with workflow explanation, tag format, attributes, and CLI options

### Examples (Fixed)
- ~~`examples/fancy_demo/` - No README despite being a substantial example~~ → README added documenting custom widget implementations
- ~~`examples/cpp/` - No explanation of `platform_native` vs `platform_qt` differences~~ → README added with use cases and comparison table
- ~~~20 examples not featured in the examples README~~ → Added "Additional Examples", "Embedded/MCU Examples", and "Platform Integration Examples" sections

## Missing Documentation

### Platform/Deployment
- No iOS getting-started template (unlike Rust, C++, Node.js, Python)
- No production deployment guide (packaging, signing, distribution)
- No WebAssembly build toolchain documentation

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

### Documentation Issues (Fixed)

- ~~Node.js version requirement says "v16" (outdated)~~ → Updated to v20 or newer across all documentation files

## Outdated/Inconsistent Information

- `docs/embedded-tutorials.md` is a template file with no actual tutorials
- Python marked Beta in its README but featured equally in main README

## Underdocumented Features

- Weak references (Rust API)
- Custom renderer implementation
- Model filtering/sorting
- Animation timing and performance
- Direct OpenGL/graphics integration (examples exist, no systematic guide)
