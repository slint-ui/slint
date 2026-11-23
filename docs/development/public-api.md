# Public API

Public API is what Slint users can rely on; everything else is internal and can change at any time.

## Rust

The public API is what the published crates `slint`, `slint-interpreter`, `slint-build`, and `slint-sc` export.
The `i-slint-*` crates are internal.
Their items only become public API when one of the public crates re-exports them.

The `slint` crate re-exports its modules from these crates:

- Top level: `i_slint_core::api`, `i_slint_backend_selector::api`,
  and selected `i_slint_core` items, such as models and timers.
- `slint!`: `slint-macros`.
- `platform`: `i_slint_core::platform`.
- `platform::software_renderer`: `i_slint_renderer_software::api`.
- `platform::femtovg_renderer` and `platform::skia_renderer`: selected items from `i_slint_renderer_femtovg` and `i_slint_renderer_skia`.
- `language`: the built-in structs and enums from `i_slint_core::items`.
- `wgpu_29` and `wgpu_30`: `i_slint_core::graphics::wgpu_*::api`.
- `winit_030`: selected items from `i_slint_backend_winit`.
- `fontique_012`: `fontique`, through `i_slint_common::sharedfontique`.
- `android`: `i_slint_backend_android_activity`.

`slint-interpreter` exports its `api` module at the top level.

These aren't public API, even when reachable:

- Modules and items marked `#[doc(hidden)]`.
- The `private_unstable_api` module, which only generated code uses.
- Items behind `internal*` features.

Items behind `unstable-*` features are public, but exempt from the compatibility promise.

## C++

The public API is what the headers in `api/cpp/include` declare.
Anything in a `private_api` or `cbindgen_private` namespace is internal, as are the headers in `api/cpp/include/private`.

## Node.js

The public API is what `api/node/typescript/index.ts` exports.

## Python

The public API is what the `slint` package exports in `__all__`.
Names that start with `_`, such as the `_native` module, are internal.

## Slint Language

The public API is the syntax, the built-in elements, structs, enums, and functions, and the `std-widgets.slint` widgets,
with their properties, callbacks, and functions.
Elements marked `@is_internal` in `internal/compiler/builtin_elements.rs` are internal.
Experimental features are public, but exempt from the compatibility promise.

## Tools

The command-line options of the tools, such as `slint-compiler`, `slint-viewer`, and `slint-lsp`, are public API too.
