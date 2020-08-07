/*!
# SixtyFPS

This create is the main entry point for project using SixtyFPS UI in rust.

## How to use:

There are two ways to use this crate.

 - The `.60` code inline in a macro.
 - The `.60` code in external files compiled with `build.rs`

### The .60 code in a macro

This is the simpler way, just put the

```rust
sixtyfps::sixtyfps!{
    HelloWorld := Text { text: "hello world"; }
}
fn main() {
#   return; // Don't run a window in an example
    HelloWorld::new().run()
}
```

### The .60 file in external files compiled with `build.rs`

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

#![warn(missing_docs)]
#![deny(unsafe_code)]

pub use sixtyfps_rs_macro::sixtyfps;

pub(crate) mod repeater;

/// internal re_exports used by the macro generated
#[doc(hidden)]
pub mod re_exports {
    pub use crate::repeater::*;
    pub use const_field_offset::{self, FieldOffsets};
    pub use once_cell::sync::Lazy;
    pub use once_cell::unsync::OnceCell;
    pub use pin_weak::rc::*;
    pub use sixtyfps_corelib::abi::datastructures::*;
    pub use sixtyfps_corelib::animations::EasingCurve;
    pub use sixtyfps_corelib::eventloop::ComponentWindow;
    pub use sixtyfps_corelib::graphics::{
        PathArcTo, PathData, PathElement, PathEvent, PathLineTo, Point, Rect, Size,
    };
    pub use sixtyfps_corelib::input::{
        process_ungrabbed_mouse_event, InputEventResult, MouseEvent,
    };
    pub use sixtyfps_corelib::item_tree::{
        item_offset, visit_item_tree, ItemTreeNode, ItemVisitorRefMut, ItemVisitorVTable,
        VisitChildrenResult,
    };
    pub use sixtyfps_corelib::items::*;
    pub use sixtyfps_corelib::layout::LayoutInfo;
    pub use sixtyfps_corelib::layout::{
        grid_layout_info, solve_grid_layout, solve_path_layout, GridLayoutCellData, GridLayoutData,
        PathLayoutData, PathLayoutItemData,
    };
    pub use sixtyfps_corelib::properties::{Property, PropertyListenerScope};
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
pub fn create_window() -> re_exports::ComponentWindow {
    sixtyfps_rendering_backend_gl::create_gl_window()
}

/// This module contains functions usefull for unit tests
pub mod testing {
    pub use sixtyfps_corelib::tests::sixtyfps_mock_elapsed_time as mock_elapsed_time;
}

/// Include the code generated with the sixtyfps-build crate from the build script
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
