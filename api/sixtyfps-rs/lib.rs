/* LICENSE BEGIN

    This file is part of the Sixty FPS Project

    Copyright (c) 2020 Olivier Goffart <olivier.goffart@sixtyfps.io>
    Copyright (c) 2020 Simon Hausmann <simon.hausmann@sixtyfps.io>

    SPDX-License-Identifier: GPL-3.0-only

LICENSE END */
/*!
# SixtyFPS

This crate is the main entry point for embedding user interfaces designed with
SixtyFPS UI in Rust programs.

Included in this documentation is also the [language reference](langref/index.html).

## How to use:

The user interfaces are described in the `.60` markup language. There are two ways
of including the design in Rust:

 - The `.60` code is inline in a macro.
 - The `.60` code in external files compiled with `build.rs`

### The .60 code in a macro

This method combines your Rust code with the `.60` markup in one file, using a macro:

```rust
sixtyfps::sixtyfps!{
    HelloWorld := Text {
        text: "hello world";
        color: black;
    }
}
fn main() {
#   return; // Don't run a window in an example
    HelloWorld::new().run()
}
```

### The .60 file in external files compiled with `build.rs`

This method allows you to a separate `.60` file on the file system, which works well if
your design becomes bigger and you split it up across multiple files. You need to use a
so-called [build script](https://doc.rust-lang.org/cargo/reference/build-scripts.html)
to trigger the compilation of the `.60` file.

In your Cargo.toml:

FIXME! set the version

```toml
[package]
...
build = "build.rs"

[dependencies]
sixtyfps = "*"
...

[build-dependencies]
sixtyfps-build = "*"
```

In the `build.rs` file:

```ignore
fn main() {
    sixtyfps_build::compile("ui/hello.60");
}
```

Then in your main file

```ignore
sixtyfps::include_modules!();
fn main() {
    HelloWorld::new().run()
}
```
*/

#![cfg_attr(nightly, feature(doc_cfg, external_doc))]
#![warn(missing_docs)]
#![deny(unsafe_code)]

pub use sixtyfps_rs_macro::sixtyfps;

pub(crate) mod repeater;

/// internal re_exports used by the macro generated
#[doc(hidden)]
pub mod re_exports {
    pub use crate::repeater::*;
    pub use const_field_offset::{self, FieldOffsets, PinnedDrop};
    pub use once_cell::sync::Lazy;
    pub use once_cell::unsync::OnceCell;
    pub use pin_weak::rc::*;
    pub use sixtyfps_corelib::animations::EasingCurve;
    pub use sixtyfps_corelib::component::{Component, ComponentVTable};
    pub use sixtyfps_corelib::eventloop::ComponentWindow;
    pub use sixtyfps_corelib::graphics::{
        PathArcTo, PathData, PathElement, PathEvent, PathLineTo, Point, Rect, Size,
    };
    pub use sixtyfps_corelib::input::{
        process_ungrabbed_mouse_event, InputEventResult, MouseEvent,
    };
    pub use sixtyfps_corelib::item_tree::{
        item_offset, visit_item_tree, ItemTreeNode, ItemVisitorRefMut, ItemVisitorVTable,
        TraversalOrder, VisitChildrenResult,
    };
    pub use sixtyfps_corelib::items::*;
    pub use sixtyfps_corelib::layout::LayoutInfo;
    pub use sixtyfps_corelib::layout::{
        grid_layout_info, solve_grid_layout, solve_path_layout, GridLayoutCellData, GridLayoutData,
        PathLayoutData, PathLayoutItemData,
    };
    pub use sixtyfps_corelib::properties::{Property, PropertyTracker};
    pub use sixtyfps_corelib::signals::Signal;
    pub use sixtyfps_corelib::slice::Slice;
    pub use sixtyfps_corelib::Color;
    pub use sixtyfps_corelib::ComponentVTable_static;
    pub use sixtyfps_corelib::Resource;
    pub use sixtyfps_corelib::SharedArray;
    pub use sixtyfps_corelib::SharedString;
    pub use vtable::{self, *};
}

/// Creates a new window to render components in.
#[doc(hidden)]
pub fn create_window() -> re_exports::ComponentWindow {
    sixtyfps_rendering_backend_gl::create_gl_window()
}

/// This module contains functions useful for unit tests
pub mod testing {
    pub use sixtyfps_corelib::tests::sixtyfps_mock_elapsed_time as mock_elapsed_time;
    /// Simulate a mouse click
    pub fn send_mouse_click<
        X: vtable::HasStaticVTable<sixtyfps_corelib::component::ComponentVTable>,
    >(
        component: core::pin::Pin<&X>,
        x: f32,
        y: f32,
    ) {
        sixtyfps_corelib::tests::sixtyfps_send_mouse_click(vtable::VRef::new_pin(component), x, y);
    }
}

/// Include the code generated with the sixtyfps-build crate from the build script. After calling `sixtyfps_build::compile`
/// in your `build.rs` build script, the use of this macro includes the generated Rust code and makes the exported types
/// available for you to instantiate.
#[macro_export]
macro_rules! include_modules {
    () => {
        include!(env!("SIXTYFPS_INCLUDE_GENERATED"));
    };
}

/// Helper type that helps checking that the generated code is generated for the right version
#[doc(hidden)]
#[allow(non_camel_case_types)]
pub struct VersionCheck_0_1_0;

#[cfg(doctest)]
mod compile_fail_tests;

pub mod langref {
    #![cfg_attr(nightly, doc(include = "../../docs/langref.md"))]
    #![doc = ""]
}
