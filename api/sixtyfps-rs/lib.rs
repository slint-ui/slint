/* LICENSE BEGIN
    This file is part of the SixtyFPS Project -- https://sixtyfps.io
    Copyright (c) 2020 Olivier Goffart <olivier.goffart@sixtyfps.io>
    Copyright (c) 2020 Simon Hausmann <simon.hausmann@sixtyfps.io>

    SPDX-License-Identifier: GPL-3.0-only
    This file is also available under commercial licensing terms.
    Please contact info@sixtyfps.io for more information.
LICENSE END */
/*!
# SixtyFPS

This crate is the main entry point for embedding user interfaces designed with
[SixtyFPS UI](https://www.sixtyfps.io/) in Rust programs.

Included in this documentation is also the [language reference](docs::langref).

## How to use:

The user interfaces are described in the `.60` design markup language. There are two ways
of including the design in Rust:

 - The `.60` code is inline in a macro.
 - The `.60` code in external files compiled with `build.rs`

 This markup code is translated to Rust code and each component is turned into a Rust
 struct with functions to instantiated, show or access properties. This documentation
 includes an [example](docs::generated_code::SampleComponent) of how the API looks
 like.

### The .60 code in a macro

This method combines your Rust code with the `.60` design markup in one file, using a macro:

```rust
sixtyfps::sixtyfps!{
    HelloWorld := Window {
        Text {
            text: "hello world";
            color: green;
        }
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

```toml
[package]
...
build = "build.rs"

[dependencies]
sixtyfps = "0.0.2"
...

[build-dependencies]
sixtyfps-build = "0.0.2"
```

In the `build.rs` file:

```ignore
fn main() {
    sixtyfps_build::compile("ui/hello.60").unwrap();
}
```

Then in your main file

```ignore
sixtyfps::include_modules!();
fn main() {
    HelloWorld::new().run()
}
```

### Generated components

As of now, only the last component of a .60 source is generated. It is planed to generate all exported components.

The component is generated and re-exported at the location of the [`include_modules!`]  or [`sixtyfps!`] macro.
it consist of a struct of the same name of the component.
For example, if you have `export MyComponent := Window { /*...*/ }` in the .60 file, it will create a `struct MyComponent{ /*...*/ }`.
This documentation contains a documented generated component: [`docs::generated_code::SampleComponent`].

The following associated function are added to the component:

  - [`fn new() -> Pin<Rc<Self>>`](docs::generated_code::SampleComponent::new): to instantiate the component.
  - [`fn run(self: Pin<Rc<Self>>)`](docs::generated_code::SampleComponent::run): to show and start the event loop.
  - [`fn as_weak(self: Pin<Rc<Self>>)`](docs::generated_code::SampleComponent::as_weak): Convenience to create a weak reference pointer.

For each top-level property
  - A setter [`fn set_<property_name>(&self, value: <PropertyType>)`](docs::generated_code::SampleComponent::set_counter)
  - A getter [`fn get_<property_name>(self: Pin<&Self>) -> <PropertyType>`](docs::generated_code::SampleComponent::get_counter)

For each top-level signal
  - [`fn emit_<signal_name>(self: Pin<&Self>)`](docs::generated_code::SampleComponent::emit_hello): to emit the signal
  - [`fn on_<signal_name>(self: Pin<&Self>, callback: impl Fn(<SignalArgs>) + 'static)`](docs::generated_code::SampleComponent::on_hello): to set the signal handler.

### Type Mappings

The types used for properties in `.60` design markup each translate to specific types in Rust.
The follow table summarizes the entire mapping:

| `.60` Type | Rust Type | Note |
| --- | --- | --- |
| `int` | `i32` | |
| `float` | `f32` | |
| `string` | [`SharedString`] | A reference-counted string type that can be easily converted to a str reference. |
| `color` | [`Color`] | |
| `length` | `f32` | The unit are physical pixels. |
| `logical_length` | `f32` | At run-time, logical lengths are automatically translated to physical pixels using the device pixel ratio. |
| `duration` | `i64` | At run-time, durations are always represented as signed 64-bit integers with milisecond precision. |
| structure | `struct` of the same name | |

For user defined structures in the .60, an extra struct is generated.
For example, if the `.60` contains
```60
export MyStruct := {
    property <int> foo;
    property <string> bar;
}
```

The following struct would be generated:

```rust
#[derive(Default, Clone, Debug, PartialEq)]
struct MyStruct {
    foo : i32,
    bar: sixtyfps::SharedString,
}
```

*/

#![cfg_attr(nightly, feature(doc_cfg, external_doc))]
#![warn(missing_docs)]
#![deny(unsafe_code)]

pub use sixtyfps_macros::sixtyfps;

pub use sixtyfps_corelib::model::{
    Model, ModelHandle, ModelNotify, ModelPeer, StandardListViewItem, VecModel,
};
pub use sixtyfps_corelib::sharedarray::SharedArray;
pub use sixtyfps_corelib::string::SharedString;
pub use sixtyfps_corelib::{ARGBColor, Color};

/// internal re_exports used by the macro generated
#[doc(hidden)]
pub mod re_exports {
    pub use const_field_offset::{self, FieldOffsets, PinnedDrop};
    pub use core::iter::FromIterator;
    pub use once_cell::sync::Lazy;
    pub use once_cell::unsync::OnceCell;
    pub use pin_weak::rc::*;
    pub use sixtyfps_corelib::animations::EasingCurve;
    pub use sixtyfps_corelib::component::{
        init_component_items, Component, ComponentRefPin, ComponentVTable,
    };
    pub use sixtyfps_corelib::eventloop::ComponentWindow;
    pub use sixtyfps_corelib::graphics::{
        PathArcTo, PathData, PathElement, PathEvent, PathLineTo, Point, Rect, Size,
    };
    pub use sixtyfps_corelib::input::{
        locate_and_activate_focus_item, process_ungrabbed_mouse_event, FocusEvent,
        FocusEventResult, InputEventResult, KeyCode, KeyEvent, KeyEventResult, KeyboardModifiers,
        MouseEvent, ALT_MODIFIER, CONTROL_MODIFIER, COPY_PASTE_MODIFIER, LOGO_MODIFIER,
        NO_MODIFIER, SHIFT_MODIFIER,
    };
    pub use sixtyfps_corelib::item_tree::{
        item_offset, visit_item_tree, ItemTreeNode, ItemVisitorRefMut, ItemVisitorVTable,
        TraversalOrder, VisitChildrenResult,
    };
    pub use sixtyfps_corelib::items::*;
    pub use sixtyfps_corelib::layout::LayoutInfo;
    pub use sixtyfps_corelib::layout::{
        grid_layout_info, solve_grid_layout, solve_path_layout, GridLayoutCellData, GridLayoutData,
        Padding, PathLayoutData, PathLayoutItemData,
    };
    pub use sixtyfps_corelib::model::*;
    pub use sixtyfps_corelib::properties::{Property, PropertyTracker};
    pub use sixtyfps_corelib::signals::Signal;
    pub use sixtyfps_corelib::slice::Slice;
    pub use sixtyfps_corelib::Color;
    pub use sixtyfps_corelib::ComponentVTable_static;
    pub use sixtyfps_corelib::Resource;
    pub use sixtyfps_corelib::SharedArray;
    pub use sixtyfps_corelib::SharedString;
    pub use sixtyfps_rendering_backend_default::native_widgets::*;
    pub use vtable::{self, *};
}

/// Creates a new window to render components in.
#[doc(hidden)]
pub fn create_window() -> re_exports::ComponentWindow {
    sixtyfps_rendering_backend_default::create_window()
}

/// This module contains functions useful for unit tests
pub mod testing {
    /// This trait gives access to the underyling Window of a component for the
    /// purposes of testing.
    pub trait HasWindow {
        /// Returns a reference to the component's window.
        fn component_window(&self) -> &super::re_exports::ComponentWindow;
    }

    pub use sixtyfps_corelib::tests::sixtyfps_mock_elapsed_time as mock_elapsed_time;
    /// Simulate a mouse click
    pub fn send_mouse_click<
        X: vtable::HasStaticVTable<sixtyfps_corelib::component::ComponentVTable> + HasWindow,
    >(
        component: core::pin::Pin<&X>,
        x: f32,
        y: f32,
    ) {
        sixtyfps_corelib::tests::sixtyfps_send_mouse_click(
            vtable::VRef::new_pin(component),
            x,
            y,
            component.component_window(),
        );
    }

    /// Simulate a change in keyboard modifiers being pressed
    pub fn set_current_keyboard_modifiers<X: HasWindow>(
        component: core::pin::Pin<&X>,
        modifiers: crate::re_exports::KeyboardModifiers,
    ) {
        sixtyfps_corelib::tests::sixtyfps_set_keyboard_modifiers(
            component.component_window(),
            modifiers,
        )
    }

    /// Simulate a series of key press and release event
    pub fn send_key_clicks<
        X: vtable::HasStaticVTable<sixtyfps_corelib::component::ComponentVTable> + HasWindow,
    >(
        component: core::pin::Pin<&X>,
        key_codes: &[crate::re_exports::KeyCode],
    ) {
        sixtyfps_corelib::tests::sixtyfps_send_key_clicks(
            vtable::VRef::new_pin(component),
            &crate::re_exports::Slice::from_slice(key_codes),
            component.component_window(),
        )
    }

    /// Simulate entering a sequence of ascii characters key by key.
    pub fn send_keyboard_string_sequence<
        X: vtable::HasStaticVTable<sixtyfps_corelib::component::ComponentVTable> + HasWindow,
    >(
        component: core::pin::Pin<&X>,
        sequence: &str,
    ) {
        sixtyfps_corelib::tests::send_keyboard_string_sequence(
            vtable::VRef::new_pin(component),
            &super::SharedString::from(sequence),
            component.component_window(),
        )
    }
}

/// Include the code generated with the sixtyfps-build crate from the build script. After calling `sixtyfps_build::compile`
/// in your `build.rs` build script, the use of this macro includes the generated Rust code and makes the exported types
/// available for you to instantiate.
///
/// Check the documentation of the `sixtyfps-build` crate for more information.
#[macro_export]
macro_rules! include_modules {
    () => {
        include!(env!("SIXTYFPS_INCLUDE_GENERATED"));
    };
}

/// Helper type that helps checking that the generated code is generated for the right version
#[doc(hidden)]
#[allow(non_camel_case_types)]
pub struct VersionCheck_0_0_2;

#[cfg(doctest)]
mod compile_fail_tests;

#[cfg(all(doc, nightly))]
pub mod docs;
