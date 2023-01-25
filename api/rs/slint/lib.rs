// Copyright © SixtyFPS GmbH <info@slint-ui.com>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-commercial

// cSpell: ignore buildrs

/*!
# Slint

This crate is the main entry point for embedding user interfaces designed with
[Slint UI](https://slint-ui.com/) in Rust programs.

If you are new to Slint, start with the [Walk-through tutorial](https://slint-ui.com/docs/tutorial/rust).
If you are already familiar with Slint, the following topics provide related information.

## Related topics

 * [Examples and Recipes](docs::recipes)
 * [The `.slint` language reference](docs::langref)
 * [Builtin Elements](docs::builtin_elements)
 * [Builtin Enums](docs::builtin_enums)
 * [Widgets](docs::widgets)
 * [Positioning and Layout of Elements](docs::layouting)
 * [Debugging Techniques](docs::debugging_techniques)
 * [Slint on Microcontrollers](docs::mcu)

## How to use this crate:

Designs of user interfaces are described in the `.slint` design markup language. There are three ways
of including them in Rust:

 - The `.slint` code is [inline in a macro](#the-slint-code-in-a-macro).
 - The `.slint` code in [external files compiled with `build.rs`](#the-slint-code-in-external-files-is-compiled-with-buildrs)
 - The `.slint` code is loaded dynamically at run-time from the file system, by using the [interpreter API](https://docs.rs/slint-interpreter).

With the first two methods, the markup code is translated to Rust code and each component is turned into a Rust
struct with functions. Use these functions to instantiate and show the component, and
to access declared properties. Check out our [sample component](docs::generated_code::SampleComponent) for more
information about the generation functions and how to use them.

### The .slint code in a macro

This method combines your Rust code with the `.slint` design markup in one file, using a macro:

```rust
slint::slint!{
    export component HelloWorld {
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

### The .slint code in external files is compiled with `build.rs`

When your design becomes bigger in terms of markup code, you may want move it to a dedicated
`.slint` file. It's also possible to split a `.slint` file into multiple files using [modules](docs::langref#modules).
Use a [build script](https://doc.rust-lang.org/cargo/reference/build-scripts.html) to compile
your main `.slint` file:

In your Cargo.toml add a `build` assignment and use the `slint-build` crate in `build-dependencies`:

```toml
[package]
...
build = "build.rs"
edition = "2021"

[dependencies]
slint = "0.3.4"
...

[build-dependencies]
slint-build = "0.3.4"
```

Use the API of the slint-build crate in the `build.rs` file:

```rust,no_run
fn main() {
    slint_build::compile("ui/hello.slint").unwrap();
}
```

Finally, use the [`include_modules!`] macro in your `main.rs`:

```ignore
slint::include_modules!();
fn main() {
    HelloWorld::new().run();
}
```

The [cargo-generate](https://github.com/cargo-generate/cargo-generate) tool is a great tool to up and running quickly with a new
Rust project. You can use it in combination with our [Template Repository](https://github.com/slint-ui/slint-rust-template) to
create a skeleton file hierarchy that uses this method:

```bash
cargo install cargo-generate
cargo generate --git https://github.com/slint-ui/slint-rust-template
```

## Generated components

Currently, only the last component in a `.slint` source file is mapped to a Rust structure that be instantiated. We are tracking the
resolution of this limitation in <https://github.com/slint-ui/slint/issues/784>.

The component is generated and re-exported to the location of the [`include_modules!`]  or [`slint!`] macro. It is represented
as a struct with the same name as the component.

For example, if you have

```slint,no-preview
export MyComponent := Window { /*...*/ }
```

in the .slint file, it will create a
```rust
struct MyComponent{ /*...*/ }
```

See also our [sample component](docs::generated_code::SampleComponent) for more information about the API of the generated struct.

A component is instantiated using the [`fn new() -> Self`](docs::generated_code::SampleComponent::new) function. The following
convenience functions are available through the [`ComponentHandle`] implementation:

  - [`fn clone_strong(&self) -> Self`](docs::generated_code::SampleComponent::clone_strong): creates a strongly referenced clone of the component instance.
  - [`fn as_weak(&self) -> Weak`](docs::generated_code::SampleComponent::as_weak): to create a [weak](Weak) reference to the component instance.
  - [`fn show(&self)`](docs::generated_code::SampleComponent::show): to show the window of the component.
  - [`fn hide(&self)`](docs::generated_code::SampleComponent::hide): to hide the window of the component.
  - [`fn run(&self)`](docs::generated_code::SampleComponent::run): a convenience function that first calls `show()`,
    followed by spinning the event loop, and `hide()` when returning from the event loop.
  - [`fn global<T: Global<Self>>(&self) -> T`](docs::generated_code::SampleComponent::global): an accessor to the global singletons,

For each top-level property
  - A setter [`fn set_<property_name>(&self, value: <PropertyType>)`](docs::generated_code::SampleComponent::set_counter)
  - A getter [`fn get_<property_name>(&self) -> <PropertyType>`](docs::generated_code::SampleComponent::get_counter)

For each top-level callback
  - [`fn invoke_<callback_name>(&self)`](docs::generated_code::SampleComponent::invoke_hello): to invoke the callback
  - [`fn on_<callback_name>(&self, callback: impl Fn(<CallbackArgs>) + 'static)`](docs::generated_code::SampleComponent::on_hello): to set the callback handler.

Note: All dashes (`-`) are replaced by underscores (`_`) in names of types or functions.

After instantiating the component, call [`ComponentHandle::run()`] on show it on the screen and spin the event loop to
react to input events. To show multiple components simultaneously, call [`ComponentHandle::show()`] on each instance.
Call [`run_event_loop()`] when you're ready to enter the event loop.

The generated component struct acts as a handle holding a strong reference (similar to an `Rc`). The `Clone` trait is
not implemented. Instead you need to make explicit [`ComponentHandle::clone_strong`] and [`ComponentHandle::as_weak`]
calls. A strong reference should not be captured by the closures given to a callback, as this would produce a reference
loop and leak the component. Instead, the callback function should capture a weak component.

## Threading and Event-loop

For platform-specific reasons, the event loop must run in the main thread, in most backends, and all the components
must be created in the same thread as the thread the event loop is running or is going to run.

You should perform the minimum amount of work in the main thread and delegate the actual logic to another
thread to avoid blocking animations. Use the [`invoke_from_event_loop`] function to communicate from your worker thread to the UI thread.

To run a function with a delay or with an interval use a [`Timer`].

## Type Mappings

The types used for properties in `.slint` design markup each translate to specific types in Rust.
The follow table summarizes the entire mapping:

| `.slint` Type | Rust Type | Note |
| --- | --- | --- |
| `int` | `i32` | |
| `float` | `f32` | |
| `bool` | `bool` | |
| `string` | [`SharedString`] | A reference-counted string type that can be easily converted to a str reference. |
| `color` | [`Color`] | |
| `brush` | [`Brush`] | |
| `image` | [`Image`] | |
| `physical_length` | `f32` | The unit are physical pixels. |
| `length` | `f32` | At run-time, logical lengths are automatically translated to physical pixels using the device pixel ratio. |
| `duration` | `i64` | At run-time, durations are always represented as signed 64-bit integers with millisecond precision. |
| `angle` | `f32` | The value in degrees |
| `relative-font-size` | `f32` | Relative font size factor that is multiplied with the `Window.default-font-size` and can be converted to a `length`. |
| structure | `struct` of the same name | |
| array | [`ModelRc`] |  |

For user defined structures in the .slint, an extra struct is generated.
For example, if the `.slint` contains
```slint,ignore
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
    bar: slint::SharedString,
}
```

## Exported Global singletons

When you export a [global singleton](docs::langref#global-singletons) from the main file,
it is also generated with the exported name. Like the main component, the generated struct have
inherent method to access the properties and callback:

For each property
  - A setter: `fn set_<property_name>(&self, value: <PropertyType>)`
  - A getter: `fn get_<property_name>(&self) -> <PropertyType>`

For each callback
  - `fn invoke_<callback_name>(&self, <CallbackArgs>) -> <ReturnValue>` to invoke the callback
  - `fn on_<callback_name>(&self, callback: impl Fn(<CallbackArgs>) + 'static)` to set the callback handler.

The global can be accessed with the [`ComponentHandle::global()`] function, or with [`Global::get()`]

See the [documentation of the `Global` trait](Global) for an example.
*/
//! ## Feature flags
#![cfg_attr(feature = "document-features", doc = document_features::document_features!())]
#![warn(missing_docs)]
#![deny(unsafe_code)]
#![doc(html_logo_url = "https://slint-ui.com/logo/slint-logo-square-light.svg")]
#![cfg_attr(not(feature = "std"), no_std)]

extern crate alloc;

#[cfg(not(feature = "compat-0-3-0"))]
compile_error!(
    "The feature `compat-0-3-0` must be enabled to ensure \
    forward compatibility with future version of this crate"
);

pub use slint_macros::slint;

pub use i_slint_core::api::*;
pub use i_slint_core::graphics::{
    Brush, Color, Image, LoadImageError, Rgb8Pixel, Rgba8Pixel, RgbaColor, SharedPixelBuffer,
};
pub use i_slint_core::model::{
    FilterModel, MapModel, Model, ModelExt, ModelNotify, ModelPeer, ModelRc, ModelTracker,
    SortModel, StandardListViewItem, TableColumn, VecModel,
};
pub use i_slint_core::sharedvector::SharedVector;
pub use i_slint_core::timers::{Timer, TimerMode};
pub use i_slint_core::{format, string::SharedString};

#[deprecated(note = "Use platform::PointerEventButton instead")]
/// Deprecated type alias for [`slint::platform::PointerEventButton`](`crate::platform::PointerEventButton`).
pub type PointerEventButton = crate::platform::PointerEventButton;

#[deprecated(note = "Use platform::WindowEvent instead")]
/// Deprecated type alias for [`slint::platform::WindowEvent`](`crate::platform::WindowEvent`).
pub type WindowEvent = crate::platform::WindowEvent;

pub mod private_unstable_api;

/// Enters the main event loop. This is necessary in order to receive
/// events from the windowing system in order to render to the screen
/// and react to user input.
pub fn run_event_loop() {
    i_slint_backend_selector::with_platform(|b| b.run_event_loop())
}

/// Include the code generated with the slint-build crate from the build script. After calling `slint_build::compile`
/// in your `build.rs` build script, the use of this macro includes the generated Rust code and makes the exported types
/// available for you to instantiate.
///
/// Check the documentation of the `slint-build` crate for more information.
#[macro_export]
macro_rules! include_modules {
    () => {
        include!(env!("SLINT_INCLUDE_GENERATED"));
    };
}

/// This module contains items that you need to use or implement if you want use Slint in an environment without
/// one of the supplied platform backends such as qt or winit.
///
/// The primary interface is the [`platform::Platform`] trait. Pass your implementation of it to Slint by calling
/// [`platform::set_platform()`] early on in your application, before creating any Slint components.
///
/// The [Slint on Microcontrollers](crate::docs::mcu) documentation has additional examples.
pub mod platform {
    pub use i_slint_core::platform::*;
}

/// Helper type that helps checking that the generated code is generated for the right version
#[doc(hidden)]
#[allow(non_camel_case_types)]
pub struct VersionCheck_0_3_4;

#[cfg(doctest)]
mod compile_fail_tests;

#[cfg(doc)]
pub mod docs;
