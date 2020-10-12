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

Included in this documentation is also the [language reference](langref/index.html).

## How to use:

The user interfaces are described in the `.60` design markup language. There are two ways
of including the design in Rust:

 - The `.60` code is inline in a macro.
 - The `.60` code in external files compiled with `build.rs`

 This markup code is translated to Rust code and each component is turned into a Rust
 struct with functions to instantiated, show or access properties. This documentation
 includes an [example][`generated_code::SampleComponent`] of how the API looks
 like.

### The .60 code in a macro

This method combines your Rust code with the `.60` design markup in one file, using a macro:

```rust
sixtyfps::sixtyfps!{
    HelloWorld := Text {
        text: "hello world";
        color: green;
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

### Types

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

*/

#![cfg_attr(nightly, feature(doc_cfg, external_doc))]
#![warn(missing_docs)]
#![deny(unsafe_code)]

pub use sixtyfps_rs_macro::sixtyfps;

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
#[macro_export]
macro_rules! include_modules {
    () => {
        include!(env!("SIXTYFPS_INCLUDE_GENERATED"));
    };
}

/// Helper type that helps checking that the generated code is generated for the right version
#[doc(hidden)]
#[allow(non_camel_case_types)]
pub struct VersionCheck_0_0_1;

#[cfg(doctest)]
mod compile_fail_tests;

#[cfg(all(doc, nightly))]
pub mod langref {
    #![doc(include = "docs/langref.md")]
    #![doc = ""]
}

#[cfg(all(doc, nightly))]
pub mod builtin_elements {
    #![doc(include = "docs/builtin_elements.md")]
    #![doc = ""]
}

#[cfg(all(doc, nightly))]
pub mod widgets {
    #![doc(include = "docs/widgets.md")]
    #![doc = ""]
}

/// This module exists only to explain the API of the code generated from `.60` design markup. Its described structure
/// is not really contained in the compiled crate.
#[cfg(doc)]
pub mod generated_code {
    /// This an example of the API that is generated for a component in `.60` design markup. This may help you understand
    /// what functions you can call and how you can pass data in and out.
    /// This is the source code:
    /// ```60
    /// SampleComponent := Window {
    ///     property<int> counter;
    ///     property<string> user_name;
    ///     signal hello;
    ///     /// ... maybe more elements here
    /// }
    /// ```
    pub struct SampleComponent {}
    impl SampleComponent {
        /// Creates a new instance that is reference counted and pinned in memory.
        pub fn new() -> core::pin::Pin<std::rc::Rc<Self>> {
            unimplemented!()
        }
        /// Creates a window on the screen, renders this component in it and spins an event loop to react
        /// to user input. A typical sequence of creating an instance and showing it may look like this:
        /// ```ignore
        /// fn main() {
        ///     let sample = SampleComponent::new();
        ///     /// other setup code here, connect to signal handlers, set property values
        ///     sample.run();
        /// }
        /// ```
        pub fn run(self: core::pin::Pin<std::rc::Rc<Self>>) {}
        /// Returns a weak pointer for an instance of this component. You can use this to in captures of
        /// closures, for example signal handlers, to access the component later.
        pub fn as_weak(
            self: core::pin::Pin<std::rc::Rc<Self>>,
        ) -> super::re_exports::PinWeak<Self> {
            unimplemented!()
        }
        /// A getter is generated for each property declared at the root of the component.
        /// In this case, this is the getter that returns the value of the `counter`
        /// property declared in the `.60` design markup.
        pub fn get_counter(self: ::core::pin::Pin<&Self>) -> i32 {
            unimplemented!()
        }
        /// A setter is generated for each property declared at the root of the component,
        /// In this case, this is the setter that sets the value of the `counter` property
        /// declared in the `.60` design markup.
        pub fn set_counter(&self, value: i32) {}
        /// Returns the value of the `user_name` property declared in the `.60` design markup.
        pub fn get_user_name(self: ::core::pin::Pin<&Self>) -> super::re_exports::SharedString {
            unimplemented!()
        }
        /// Assigns a new value to the `user_name` property.
        pub fn set_user_name(&self, value: super::re_exports::SharedString) {}
        /// For each signal declared at the root of the component, a function to emit that
        /// signal is generated. This is the function that emits the `hello` signal declared
        /// in the `.60` design markup.
        pub fn emit_hello(self: ::core::pin::Pin<&Self>) {}
        /// For each signal declared at the root of the component, a function connect to that signal
        /// is generated. This is the function that registers the function f as callback when the
        /// signal `hello` is emitted. In order to access
        /// the component in the callback, you'd typically capture a weak reference obtained using
        /// [`SampleComponent::as_weak`]
        /// and then upgrade it to a strong reference when the callback is run:
        /// ```ignore
        ///     let sample = SampleComponent::new();
        ///     let sample_weak = sample.clone().as_weak();
        ///     sample.as_ref().on_hello(move || {
        ///         let sample = sample_weak.upgrade().unwrap();
        ///         sample.as_ref().set_counter(42);
        ///     });
        /// ```
        pub fn on_hello(self: ::core::pin::Pin<&Self>, f: impl Fn() + 'static) {}
    }
}
