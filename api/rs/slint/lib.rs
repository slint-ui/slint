// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-1.2 OR LicenseRef-Slint-commercial

// cSpell: ignore buildrs

/*!
# Slint

This crate is the main entry point for embedding user interfaces designed with
[Slint](https://slint.rs/) in Rust programs.
*/
#![doc = concat!("If you are new to Slint, start with the [Walk-through **tutorial**](https://slint.dev/releases/", env!("CARGO_PKG_VERSION"), "/docs/quickstart/rust)")]
/*! If you are already familiar with Slint, the following topics provide related information.

## Topics

*/
#![doc = concat!("- [The Slint Language Documentation](https://slint.dev/releases/", env!("CARGO_PKG_VERSION"), "/docs/slint)")]
/*! - [Type mappings between .slint and Rust](docs::type_mappings)
 - [Feature flags and backend selection](docs::cargo_features)
 - [Slint on Microcontrollers](docs::mcu)

## How to use this crate:

Designs of user interfaces are described in the `.slint` design markup language. There are three ways
of including them in Rust:

 - The `.slint` code is [inline in a macro](#the-slint-code-in-a-macro).
 - The `.slint` code in [external files compiled with `build.rs`](#the-slint-code-in-external-files-is-compiled-with-buildrs)
*/
#![doc = concat!(" - The `.slint` code is loaded dynamically at run-time from the file system, by using the [interpreter API](https://slint.dev/releases/", env!("CARGO_PKG_VERSION"), "/docs/rust/slint_interpreter/).")]
/*!

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
    HelloWorld::new().unwrap().run().unwrap();
}
```

### The .slint code in external files is compiled with `build.rs`

When your design becomes bigger in terms of markup code, you may want move it to a dedicated*/
#![doc = concat!("`.slint` file. It's also possible to split a `.slint` file into multiple files using [modules](https://slint.dev/releases/", env!("CARGO_PKG_VERSION"), "/docs/slint/src/language/syntax/modules.html).")]
/*!Use a [build script](https://doc.rust-lang.org/cargo/reference/build-scripts.html) to compile
your main `.slint` file:

In your Cargo.toml add a `build` assignment and use the `slint-build` crate in `build-dependencies`:

```toml
[package]
...
build = "build.rs"
edition = "2021"

[dependencies]
slint = "1.5.0"
...

[build-dependencies]
slint-build = "1.5.0"
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
    HelloWorld::new().unwrap().run().unwrap();
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
export component MyComponent inherits Window { /*...*/ }
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

To run an async function or a future, use [`spawn_local()`].

## Exported Global singletons

*/
#![doc = concat!("When you export a [global singleton](https://slint.dev/releases/", env!("CARGO_PKG_VERSION"), "/docs/slint/src/language/syntax/globals.html) from the main file,")]
/*! it is also generated with the exported name. Like the main component, the generated struct have
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

#![warn(missing_docs)]
#![deny(unsafe_code)]
#![doc(html_logo_url = "https://slint.dev/logo/slint-logo-square-light.svg")]
#![cfg_attr(not(feature = "std"), no_std)]
#![allow(clippy::needless_doctest_main)] // We document how to write a main function

extern crate alloc;

#[cfg(not(feature = "compat-1-2"))]
compile_error!(
    "The feature `compat-1-2` must be enabled to ensure \
    forward compatibility with future version of this crate"
);

pub use slint_macros::slint;

pub use i_slint_core::api::*;
#[doc(hidden)]
#[deprecated(note = "Experimental type was made public by mistake")]
pub use i_slint_core::component_factory::ComponentFactory;
#[cfg(not(target_arch = "wasm32"))]
pub use i_slint_core::graphics::{BorrowedOpenGLTextureBuilder, BorrowedOpenGLTextureOrigin};
// keep in sync with internal/interpreter/api.rs
pub use i_slint_core::graphics::{
    Brush, Color, Image, LoadImageError, Rgb8Pixel, Rgba8Pixel, RgbaColor, SharedPixelBuffer,
};
pub use i_slint_core::model::{
    FilterModel, MapModel, Model, ModelExt, ModelNotify, ModelPeer, ModelRc, ModelTracker,
    ReverseModel, SortModel, StandardListViewItem, TableColumn, VecModel,
};
pub use i_slint_core::sharedvector::SharedVector;
pub use i_slint_core::timers::{Timer, TimerMode};
pub use i_slint_core::{format, string::SharedString};

pub mod private_unstable_api;

/// Enters the main event loop. This is necessary in order to receive
/// events from the windowing system for rendering to the screen
/// and reacting to user input.
/// This function will run until the last window is closed or until
/// [`quit_event_loop()`] is called.
///
/// See also [`run_event_loop_until_quit()`] to keep the event loop running until
/// [`quit_event_loop()`] is called, even if all windows are closed.
pub fn run_event_loop() -> Result<(), PlatformError> {
    i_slint_backend_selector::with_platform(|b| b.run_event_loop())
}

/// Similar to [`run_event_loop()`], but this function enters the main event loop
/// and continues to run even when the last window is closed, until
/// [`quit_event_loop()`] is called.
///
/// This is useful for system tray applications where the application needs to stay alive
/// even if no windows are visible.
pub fn run_event_loop_until_quit() -> Result<(), PlatformError> {
    i_slint_backend_selector::with_platform(|b| {
        #[allow(deprecated)]
        b.set_event_loop_quit_on_last_window_closed(false);
        b.run_event_loop()
    })
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

/// Initialize translations when using the `gettext` feature.
///
/// Call this in your main function with the path where translations are located.
/// This macro internally calls the [`bindtextdomain`](https://man7.org/linux/man-pages/man3/bindtextdomain.3.html) function from gettext.
///
/// The first argument of the macro must be an expression that implements `Into<std::path::PathBuf>`.
/// It specifies the directory in which gettext should search for translations.
///
/// Translations are expected to be found at `<dirname>/<locale>/LC_MESSAGES/<crate>.mo`,
/// where `dirname` is the directory passed as an argument to this macro,
/// `locale` is a locale name (e.g., `en`, `en_GB`, `fr`), and
/// `crate` is the package name obtained from the `CARGO_PKG_NAME` environment variable.
///
/// ### Example
/// ```rust
/// fn main() {
///    slint::init_translations!(concat!(env!("CARGO_MANIFEST_DIR"), "/translations/"));
///    // ...
/// }
/// ```
///
/// For example, assuming this is in a crate called `example` and the default locale
/// is configured to be French, it will load translations at runtime from
/// `/path/to/example/translations/fr/LC_MESSAGES/example.mo`.
///
/// Another example of loading translations relative to the executable:
/// ```rust
/// slint::init_translations!(std::env::current_exe().unwrap().parent().unwrap().join("translations"));
/// ```
#[cfg(feature = "gettext")]
#[macro_export]
macro_rules! init_translations {
    ($dirname:expr) => {
        $crate::private_unstable_api::init_translations(env!("CARGO_PKG_NAME"), $dirname);
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

    /// This module contains the [`femtovg_renderer::FemtoVGRenderer`] and related types.
    ///
    /// It is only enabled when the `renderer-femtovg` Slint feature is enabled.
    #[cfg(all(feature = "renderer-femtovg", not(target_os = "android")))]
    pub mod femtovg_renderer {
        pub use i_slint_renderer_femtovg::FemtoVGRenderer;
        pub use i_slint_renderer_femtovg::OpenGLInterface;
    }
}

#[cfg(any(doc, all(target_os = "android", feature = "backend-android-activity-05")))]
pub mod android;

/// Helper type that helps checking that the generated code is generated for the right version
#[doc(hidden)]
#[allow(non_camel_case_types)]
pub struct VersionCheck_1_6_0;

#[cfg(doctest)]
mod compile_fail_tests;

#[cfg(doc)]
pub mod docs;
