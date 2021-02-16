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
[SixtyFPS UI](https://sixtyfps.io/) in Rust programs.

Included in this documentation is also the [language reference](docs::langref),
documentation of [builtin elements](docs::builtin_elements), [widgets](docs::widgets) and [layouting](docs::layouting).

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
    HelloWorld::new().run();
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
sixtyfps = "0.0.5"
...

[build-dependencies]
sixtyfps-build = "0.0.5"
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
    HelloWorld::new().run();
}
```

### Generated components

As of now, only the last component of a .60 source is generated. It is planned to generate all exported components.

The component is generated and re-exported at the location of the [`include_modules!`]  or [`sixtyfps!`] macro.
it consist of a struct of the same name of the component.
For example, if you have `export MyComponent := Window { /*...*/ }` in the .60 file, it will create a `struct MyComponent{ /*...*/ }`.
This documentation contains a documented generated component: [`docs::generated_code::SampleComponent`].

The following associated function are added to the component:

  - [`fn new() -> Self`](docs::generated_code::SampleComponent::new): to instantiate the component.
  - [`fn show(&self)`]()docs::generated_code::SampleComponent::show): to show the window of the component.
  - [`fn hide(&self)`]()docs::generated_code::SampleComponent::hide): to hide the window of the component.
  - [`fn run(&self)`]()docs::generated_code::SampleComponent::run): a convenience function that first calls `show()`,
    followed by spinning the event loop, and `hide()` when returning from the event loop.

For each top-level property
  - A setter [`fn set_<property_name>(&self, value: <PropertyType>)`](docs::generated_code::SampleComponent::set_counter)
  - A getter [`fn get_<property_name>(&self) -> <PropertyType>`](docs::generated_code::SampleComponent::get_counter)

For each top-level callback
  - [`fn call_<callback_name>(&self)`](docs::generated_code::SampleComponent::call_hello): to emit the callback
  - [`fn on_<callback_name>(&self, callback: impl Fn(<CallbackArgs>) + 'static)`](docs::generated_code::SampleComponent::on_hello): to set the callback handler.

After instantiating the component you can call just [`fn run(&self)`] on it, in order to show it and spin the event loop to
render and react to input events. If you want to show multiple components simultaneously, then you can also call just
`show()` first. When you're ready to enter the event loop, just call [`run_event_loop()`].

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
| array | [`ModelHandle`] |  |

For user defined structures in the .60, an extra struct is generated.
For example, if the `.60` contains
```60
export struct MyStruct := {
    foo: int,
    bar: string,
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
#![doc(html_logo_url = "https://sixtyfps.io/resources/logo.drawio.svg")]

pub use sixtyfps_macros::sixtyfps;

pub use sixtyfps_corelib::model::{
    Model, ModelHandle, ModelNotify, ModelPeer, StandardListViewItem, VecModel,
};
pub use sixtyfps_corelib::sharedvector::SharedVector;
pub use sixtyfps_corelib::string::SharedString;
pub use sixtyfps_corelib::timers::{Timer, TimerMode};
pub use sixtyfps_corelib::{Brush, Color, RgbaColor};

/// This function can be used to register a custom TrueType font with SixtyFPS,
/// for use with the `font-family` property. The provided slice must be a valid TrueType
/// font.
pub fn register_application_font_from_memory(
    data: &'static [u8],
) -> Result<(), Box<dyn std::error::Error>> {
    sixtyfps_rendering_backend_default::backend().register_application_font_from_memory(data)
}

// FIXME: this should not be in this namespace
// but the name is `sixtyfps::StateInfo` in builtin.60
#[doc(hidden)]
pub use sixtyfps_corelib::properties::StateInfo;

/// internal re_exports used by the macro generated
#[doc(hidden)]
pub mod re_exports {
    pub use const_field_offset::{self, FieldOffsets, PinnedDrop};
    pub use core::iter::FromIterator;
    pub use once_cell::sync::Lazy;
    pub use once_cell::unsync::OnceCell;
    pub use pin_weak::rc::PinWeak;
    pub use sixtyfps_corelib::animations::EasingCurve;
    pub use sixtyfps_corelib::callbacks::Callback;
    pub use sixtyfps_corelib::component::{
        init_component_items, Component, ComponentRefPin, ComponentVTable,
    };
    pub use sixtyfps_corelib::graphics::{
        Brush, GradientStop, LinearGradientBrush, PathArcTo, PathCubicTo, PathData, PathElement,
        PathEvent, PathLineTo, PathMoveTo, PathQuadraticTo, Point, Rect, Size,
    };
    pub use sixtyfps_corelib::input::{
        FocusEvent, InputEventResult, KeyEvent, KeyEventResult, KeyboardModifiers, MouseEvent,
    };
    pub use sixtyfps_corelib::item_tree::{
        visit_item_tree, ItemTreeNode, ItemVisitorRefMut, ItemVisitorVTable, TraversalOrder,
        VisitChildrenResult,
    };
    pub use sixtyfps_corelib::items::*;
    pub use sixtyfps_corelib::layout::*;
    pub use sixtyfps_corelib::model::*;
    pub use sixtyfps_corelib::properties::{set_state_binding, Property, PropertyTracker};
    pub use sixtyfps_corelib::slice::Slice;
    pub use sixtyfps_corelib::window::ComponentWindow;
    pub use sixtyfps_corelib::Color;
    pub use sixtyfps_corelib::ComponentVTable_static;
    pub use sixtyfps_corelib::Resource;
    pub use sixtyfps_corelib::SharedString;
    pub use sixtyfps_corelib::SharedVector;
    pub use sixtyfps_rendering_backend_default::native_widgets::*;
    pub use vtable::{self, *};
}

/// Creates a new window to render components in.
#[doc(hidden)]
pub fn create_window() -> re_exports::ComponentWindow {
    sixtyfps_rendering_backend_default::backend().create_window()
}

/// Enters the main event loop. This is necessary in order to receive
/// events from the windowing system in order to render to the screen
/// and react to user input.
pub fn run_event_loop() {
    sixtyfps_rendering_backend_default::backend().run_event_loop();
}

/// This trait describes the conversion of a strongly referenced SixtyFPS component,
/// held by a [vtable::VRc] into a weak reference.
pub trait IntoWeak {
    /// The type of the generated component.
    #[doc(hidden)]
    type Inner;
    /// Returns a new weak pointer.
    fn as_weak(&self) -> Weak<Self>
    where
        Self: Sized;

    /// Internal function used when upgrading a weak reference to a strong one.
    #[doc(hidden)]
    fn from_inner(_: vtable::VRc<re_exports::ComponentVTable, Self::Inner>) -> Self;
}

/// Struct that's used to hold weak references for SixtyFPS components.
pub struct Weak<T: IntoWeak> {
    inner: vtable::VWeak<re_exports::ComponentVTable, T::Inner>,
}

impl<T: IntoWeak> Clone for Weak<T> {
    fn clone(&self) -> Self {
        Self { inner: self.inner.clone() }
    }
}

impl<T: IntoWeak> Weak<T> {
    #[doc(hidden)]
    pub fn new(rc: &vtable::VRc<re_exports::ComponentVTable, T::Inner>) -> Self {
        Self { inner: vtable::VRc::downgrade(&rc) }
    }

    /// Returns a new strongly referenced component if some other instance still
    /// holds a strong reference. Otherwise, returns None.
    pub fn upgrade(&self) -> Option<T>
    where
        T: IntoWeak,
    {
        self.inner.upgrade().map(|inner| T::from_inner(inner))
    }

    /// Convenience function that returns a new stronlyg referenced component if
    /// some other instance still holds a strong reference. Otherwise, this function
    /// panics.
    pub fn unwrap(&self) -> T {
        self.upgrade().unwrap()
    }
}

/// This module contains functions useful for unit tests
pub mod testing {
    use core::cell::Cell;
    thread_local!(static KEYBOARD_MODIFIERS : Cell<crate::re_exports::KeyboardModifiers> = Default::default());

    /// This trait gives access to the underyling Window of a component for the
    /// purposes of testing.
    pub trait HasWindow {
        /// Returns a reference to the component's window.
        fn component_window(&self) -> &super::re_exports::ComponentWindow;
    }

    pub use sixtyfps_corelib::tests::sixtyfps_mock_elapsed_time as mock_elapsed_time;
    /// Simulate a mouse click
    pub fn send_mouse_click<
        X: vtable::HasStaticVTable<sixtyfps_corelib::component::ComponentVTable> + HasWindow + 'static,
        Component: Into<vtable::VRc<sixtyfps_corelib::component::ComponentVTable, X>> + Clone,
    >(
        component: &Component,
        x: f32,
        y: f32,
    ) {
        let rc = component.clone().into();
        let dyn_rc = vtable::VRc::into_dyn(rc.clone());
        sixtyfps_corelib::tests::sixtyfps_send_mouse_click(&dyn_rc, x, y, rc.component_window());
    }

    /// Simulate a change in keyboard modifiers being pressed
    pub fn set_current_keyboard_modifiers<
        X: vtable::HasStaticVTable<sixtyfps_corelib::component::ComponentVTable> + HasWindow,
        Component: Into<vtable::VRc<sixtyfps_corelib::component::ComponentVTable, X>> + Clone,
    >(
        _component: &Component,
        modifiers: crate::re_exports::KeyboardModifiers,
    ) {
        KEYBOARD_MODIFIERS.with(|x| x.set(modifiers))
    }

    /// Simulate entering a sequence of ascii characters key by key.
    pub fn send_keyboard_string_sequence<
        X: vtable::HasStaticVTable<sixtyfps_corelib::component::ComponentVTable> + HasWindow,
        Component: Into<vtable::VRc<sixtyfps_corelib::component::ComponentVTable, X>> + Clone,
    >(
        component: &Component,
        sequence: &str,
    ) {
        let component = component.clone().into();
        sixtyfps_corelib::tests::send_keyboard_string_sequence(
            &super::SharedString::from(sequence),
            KEYBOARD_MODIFIERS.with(|x| x.get()),
            component.component_window(),
        )
    }

    /// Applies the specified rectangular constraints to the component's layout.
    pub fn apply_layout<
        X: vtable::HasStaticVTable<sixtyfps_corelib::component::ComponentVTable>,
        Component: Into<vtable::VRc<sixtyfps_corelib::component::ComponentVTable, X>> + Clone,
    >(
        component: &Component,
        rect: sixtyfps_corelib::graphics::Rect,
    ) {
        let rc = component.clone().into();
        vtable::VRc::borrow_pin(&rc).as_ref().apply_layout(rect);
    }

    /// Applies the specified scale factor to the window that's associated with the given component.
    /// This overrides the value provided by the windowing system.
    pub fn set_window_scale_factor<
        X: vtable::HasStaticVTable<sixtyfps_corelib::component::ComponentVTable> + HasWindow,
        Component: Into<vtable::VRc<sixtyfps_corelib::component::ComponentVTable, X>> + Clone,
    >(
        component: &Component,
        factor: f32,
    ) {
        let component = component.clone().into();
        component.component_window().set_scale_factor(factor)
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
pub struct VersionCheck_0_0_5;

#[cfg(doctest)]
mod compile_fail_tests;

#[cfg(all(doc, nightly))]
pub mod docs;
