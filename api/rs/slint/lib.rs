// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

// cSpell: ignore buildrs

#![cfg_attr(docsrs, feature(doc_cfg, doc_auto_cfg))]

/*!
# Slint

This crate is the main entry point for embedding user interfaces designed with
[Slint](https://slint.rs/) in Rust programs.
*/
#![doc = i_slint_core_macros::slint_doc_str!("If you are new to Slint, start with the [Walk-through **tutorial**](slint:quickstart).")]
/*! If you are already familiar with Slint, the following topics provide related information.

## Topics

*/
#![doc = i_slint_core_macros::slint_doc_str!("- [The Slint Language Documentation](slint:index)")]
/*! - [Type mappings between .slint and Rust](docs::type_mappings)
 - [Feature flags and backend selection](docs::cargo_features)
 - [Slint on Microcontrollers](docs::mcu)

## How to use this crate:

Designs of user interfaces are described in the `.slint` design markup language. There are three ways
of including them in Rust:

 - The `.slint` code is [inline in a macro](#the-slint-code-in-a-macro).
 - The `.slint` code in [external files compiled with `build.rs`](#the-slint-code-in-external-files-is-compiled-with-buildrs)
*/
#![doc = i_slint_core_macros::slint_doc_str!(" - The `.slint` code is loaded dynamically at run-time from the file system, by using the [interpreter API](slint:rust:slint_interpreter/).")]
/*!

With the first two methods, the markup code is translated to Rust code and each component is turned into a Rust
struct with functions. Use these functions to instantiate and show the component, and
to access declared properties. Check out our [sample component](docs::generated_code::SampleComponent) for more
information about the generation functions and how to use them.

### The .slint code in a macro

This method combines your Rust code with the `.slint` design markup in one file, using a macro:

```rust,no_run
slint::slint!{
    export component HelloWorld inherits Window {
        Text {
            text: "hello world";
            color: green;
        }
    }
}
fn main() {
    HelloWorld::new().unwrap().run().unwrap();
}
```

### The .slint code in external files is compiled with `build.rs`

When your design becomes bigger in terms of markup code, you may want move it to a dedicated
`.slint` file. */
#![doc = i_slint_core_macros::slint_doc_str!("It's also possible to split a `.slint` file into multiple files using [modules](slint:modules).")]
/*!Use a [build script](https://doc.rust-lang.org/cargo/reference/build-scripts.html) to compile
your main `.slint` file:

*/
#![doc = i_slint_core_macros::slint_doc_str!("In your Cargo.toml add a `build` assignment and use the [`slint-build`](slint:rust:slint_build/) crate in `build-dependencies`:")]
/*!

```toml
[package]
...
build = "build.rs"
edition = "2021"

[dependencies]
slint = "1.13"
...

[build-dependencies]
slint-build = "1.13"
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

Use our [Template Repository](https://github.com/slint-ui/slint-rust-template) to create a skeleton file
hierarchy that uses this method:

1. Download and extract the [ZIP archive of the Rust Template](https://github.com/slint-ui/slint-rust-template/archive/refs/heads/main.zip).
2. Rename the extracted directory and change into it:

```bash
mv slint-rust-template-main my-project
cd my-project
```

## Generated components

Exported component from the macro or the main file that inherit `Window` or `Dialog` is mapped to a Rust structure.

The components are generated and re-exported to the location of the [`include_modules!`] or [`slint!`] macro.
It is represented as a struct with the same name as the component.

For example, if you have

```slint,no-preview
export component MyComponent inherits Window { /*...*/ }
```

in the .slint file, it will create a
```rust
struct MyComponent { /*...*/ }
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
#![doc = i_slint_core_macros::slint_doc_str!("When you export a [global singleton](slint:globals) from the main file,")]
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

**Note**: Global singletons are instantiated once per component. When declaring multiple components for `export` to Rust,
each instance will have their own instance of associated globals singletons.
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
pub use i_slint_core::translations::{select_bundled_translation, SelectBundledTranslationError};
pub use i_slint_core::{
    format,
    string::{SharedString, ToSharedString},
};

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

/// Spawns a [`Future`](core::future::Future) to execute in the Slint event loop.
///
/// This function is intended to be invoked only from the main Slint thread that runs the event loop.
///
/// For spawning a `Send` future from a different thread, this function should be called from a closure
/// passed to [`invoke_from_event_loop()`].
///
/// This function is typically called from a UI callback.
///
/// # Example
///
/// ```rust,no_run
/// slint::spawn_local(async move {
///     // your async code goes here
/// }).unwrap();
/// ```
///
/// # Compatibility with Tokio and other runtimes
///
/// The runtime used to execute the future on the main thread is platform-dependent,
/// for instance, it could be the winit event loop. Therefore, futures that assume a specific runtime
/// may not work. This may be an issue if you call `.await` on a future created by another
/// runtime, or pass the future directly to `spawn_local`.
///
/// Futures from the [smol](https://docs.rs/smol/latest/smol/) runtime always hand off their work to
/// separate I/O threads that run in parallel to the Slint event loop.
///
/// The [Tokio](https://docs.rs/tokio/latest/tokio/index.html) runtime is subject to the following constraints:
///
/// * Tokio futures require entering the context of a global Tokio runtime.
/// * Tokio futures aren't guaranteed to hand off their work to separate threads and may therefore not complete, because
/// the Slint runtime can't drive the Tokio runtime.
/// * Tokio futures require regular yielding to the Tokio runtime for fairness, a constraint that also can't be met by Slint.
/// * Tokio's [current-thread schedule](https://docs.rs/tokio/latest/tokio/runtime/index.html#current-thread-scheduler)
/// cannot be used in Slint main thread, because Slint cannot yield to it.
///
/// To address these constraints, use [async_compat](https://docs.rs/async-compat/latest/async_compat/index.html)'s [Compat::new()](https://docs.rs/async-compat/latest/async_compat/struct.Compat.html#method.new)
/// to implicitly allocate a shared, multi-threaded Tokio runtime that will be used for Tokio futures.
///
/// The following little example demonstrates the use of Tokio's [`TcpStream`](https://docs.rs/tokio/latest/tokio/net/struct.TcpStream.html) to
/// read from a network socket. The entire future passed to `spawn_local()` is wrapped in `Compat::new()` to make it run:
///
/// ```rust,no_run
/// // A dummy TCP server that once reports "Hello World"
/// # i_slint_backend_testing::init_integration_test_with_mock_time();
/// use std::io::Write;
///
/// let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
/// let local_addr = listener.local_addr().unwrap();
/// let server = std::thread::spawn(move || {
///     let mut stream = listener.incoming().next().unwrap().unwrap();
///     stream.write("Hello World".as_bytes()).unwrap();
/// });
///
/// let slint_future = async move {
///     use tokio::io::AsyncReadExt;
///     let mut stream = tokio::net::TcpStream::connect(local_addr).await.unwrap();
///     let mut data = Vec::new();
///     stream.read_to_end(&mut data).await.unwrap();
///     assert_eq!(data, "Hello World".as_bytes());
///     slint::quit_event_loop().unwrap();
/// };
///
/// // Wrap the future that includes Tokio futures in async_compat's `Compat` to ensure
/// // presence of a Tokio run-time.
/// slint::spawn_local(async_compat::Compat::new(slint_future)).unwrap();
///
/// slint::run_event_loop_until_quit().unwrap();
///
/// server.join().unwrap();
/// ```
///
/// The use of `#[tokio::main]` is **not recommended**. If it's necessary to use though, wrap the call to enter the Slint
/// event loop  in a call to [`tokio::task::block_in_place`](https://docs.rs/tokio/latest/tokio/task/fn.block_in_place.html):
///
/// ```rust, no_run
/// // Wrap the call to run_event_loop to ensure presence of a Tokio run-time.
/// tokio::task::block_in_place(slint::run_event_loop).unwrap();
/// ```
#[cfg(target_has_atomic = "ptr")]
pub fn spawn_local<F: core::future::Future + 'static>(
    fut: F,
) -> Result<JoinHandle<F::Output>, EventLoopError> {
    i_slint_backend_selector::with_global_context(|ctx| ctx.spawn_local(fut))
        .map_err(|_| EventLoopError::NoEventLoopProvider)?
}

#[i_slint_core_macros::slint_doc]
/// Include the code generated with the slint-build crate from the build script. After calling `slint_build::compile`
/// in your `build.rs` build script, the use of this macro includes the generated Rust code and makes the exported types
/// available for you to instantiate.
///
/// Check the documentation of the [`slint-build`](slint:rust:slint_build) crate for more information.
#[macro_export]
macro_rules! include_modules {
    () => {
        include!(env!("SLINT_INCLUDE_GENERATED"));
    };
}

#[i_slint_core_macros::slint_doc]
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
/// See also the [Translation documentation](slint:translations).
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
        pub use i_slint_renderer_femtovg::opengl::OpenGLInterface;
        pub use i_slint_renderer_femtovg::FemtoVGOpenGLRenderer as FemtoVGRenderer;
    }
}

#[cfg(any(
    doc,
    all(
        target_os = "android",
        any(feature = "backend-android-activity-05", feature = "backend-android-activity-06")
    )
))]
pub mod android;

pub use i_slint_backend_selector::api::*;

/// Helper type that helps checking that the generated code is generated for the right version
#[doc(hidden)]
#[allow(non_camel_case_types)]
pub struct VersionCheck_1_14_0;

#[cfg(doctest)]
mod compile_fail_tests;

#[cfg(doc)]
pub mod docs;

#[cfg(feature = "unstable-wgpu-26")]
pub mod wgpu_26 {
    //! WGPU 26.x specific types and re-exports.
    //!
    //! *Note*: This module is behind a feature flag and may be removed or changed in future minor releases,
    //!         as new major WGPU releases become available.
    //!
    //! Use the types in this module in combination with other APIs to integrate external, WGPU-based rendering engines
    //! into a UI with Slint.
    //!
    //! First, ensure that WGPU is used for rendering with Slint by using [`slint::BackendSelector::require_wgpu_26()`](i_slint_backend_selector::api::BackendSelector::require_wgpu_26()).
    //! This function accepts a pre-configured WGPU setup or configuration hints such as required features or memory limits.
    //!
    //! For rendering, it's crucial that you're using the same [`wgpu::Device`] and [`wgpu::Queue`] for allocating textures or submitting commands as Slint. Obtain the same queue
    //! by either using [`WGPUConfiguration::Manual`] to make Slint use an existing WGPU configuration, or use [`slint::Window::set_rendering_notifier()`](i_slint_core::api::Window::set_rendering_notifier())
    //! to let Slint invoke a callback that provides access device, queue, etc. in [`slint::GraphicsAPI::WGPU26`](i_slint_core::api::GraphicsAPI::WGPU26).
    //!
    //! To integrate rendering content into a scene shared with a Slint UI, use either [`slint::Window::set_rendering_notifier()`](i_slint_core::api::Window::set_rendering_notifier()) to render an underlay
    //! or overlay, or integrate externally produced [`wgpu::Texture`]s using [`slint::Image::try_from<wgpu::Texture>()`](i_slint_core::graphics::Image::try_from).
    //!
    //! The following example allocates a [`wgpu::Texture`] and, for the sake of simplicity in this documentation, fills with green as color, and then proceeds to set it as a `slint::Image` in the scene.
    //!
    //! `Cargo.toml`:
    //! ```toml
    //! slint = { version = "~1.13", features = ["unstable-wgpu-26"] }
    //! ```
    //!
    //! `main.rs`:
    //!```rust,no_run
    //!
    //! use slint::wgpu_26::wgpu;
    //! use wgpu::util::DeviceExt;
    //!
    //!slint::slint!{
    //!    export component HelloWorld inherits Window {
    //!        preferred-width: 320px;
    //!        preferred-height: 300px;
    //!        in-out property <image> app-texture;
    //!        VerticalLayout {
    //!            Text {
    //!                text: "hello world";
    //!                color: green;
    //!            }
    //!            Image { source: root.app-texture; }
    //!        }
    //!    }
    //!}
    //!fn main() -> Result<(), Box<dyn std::error::Error>> {
    //!    slint::BackendSelector::new()
    //!        .require_wgpu_26(slint::wgpu_26::WGPUConfiguration::default())
    //!        .select()?;
    //!    let app = HelloWorld::new()?;
    //!
    //!    let app_weak = app.as_weak();
    //!
    //!    app.window().set_rendering_notifier(move |state, graphics_api| {
    //!        let (Some(app), slint::RenderingState::RenderingSetup, slint::GraphicsAPI::WGPU26{ device, queue, ..}) = (app_weak.upgrade(), state, graphics_api) else {
    //!            return;
    //!        };
    //!
    //!        let mut pixels = slint::SharedPixelBuffer::<slint::Rgba8Pixel>::new(320, 200);
    //!        pixels.make_mut_slice().fill(slint::Rgba8Pixel {
    //!            r: 0,
    //!            g: 255,
    //!            b :0,
    //!            a: 255,
    //!        });
    //!
    //!        let texture = device.create_texture_with_data(queue,
    //!            &wgpu::TextureDescriptor {
    //!                label: None,
    //!                size: wgpu::Extent3d { width: 320, height: 200, depth_or_array_layers: 1 },
    //!                mip_level_count: 1,
    //!                sample_count: 1,
    //!                dimension: wgpu::TextureDimension::D2,
    //!                format: wgpu::TextureFormat::Rgba8Unorm,
    //!                usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
    //!                view_formats: &[],
    //!            },
    //!            wgpu::util::TextureDataOrder::default(),
    //!            pixels.as_bytes(),
    //!        );
    //!
    //!        let imported_image = slint::Image::try_from(texture).unwrap();
    //!
    //!        app.set_app_texture(imported_image);
    //!    })?;
    //!
    //!    app.run()?;
    //!
    //!    Ok(())
    //!}
    //!```
    //!
    pub use i_slint_core::graphics::wgpu_26::api::*;
}

#[cfg(feature = "unstable-winit-030")]
pub mod winit_030 {
    //! Winit 0.30.x specific types and re-exports.
    //!
    //! *Note*: This module is behind a feature flag and may be removed or changed in future minor releases,
    //!         as new major Winit releases become available.
    //!
    //! Use the types and traits in this module in combination with other APIs to access additional window properties,
    //! create custom windows, or hook into the winit event loop.
    //!
    //! For example, use the [`WinitWindowAccessor`] to obtain access to the underling [`winit::window::Window`]:
    //!
    //! `Cargo.toml`:
    //! ```toml
    //! slint = { version = "~1.12", features = ["unstable-winit-030"] }
    //! ```
    //!
    //! `main.rs`:
    //! ```rust,no_run
    //! // Bring winit and accessor traits into scope.
    //! use slint::winit_030::{WinitWindowAccessor, winit};
    //!
    //! slint::slint!{
    //!     import { VerticalBox, Button } from "std-widgets.slint";
    //!     export component HelloWorld inherits Window {
    //!         callback clicked;
    //!         VerticalBox {
    //!             Text {
    //!                 text: "hello world";
    //!                 color: green;
    //!             }
    //!             Button {
    //!                 text: "Click me";
    //!                 clicked => { root.clicked(); }
    //!             }
    //!         }
    //!     }
    //! }
    //! fn main() -> Result<(), Box<dyn std::error::Error>> {
    //!     // Make sure the winit backed is selected:
    //!    slint::BackendSelector::new()
    //!        .backend_name("winit".into())
    //!        .select()?;
    //!
    //!     let app = HelloWorld::new()?;
    //!     let app_weak = app.as_weak();
    //!     app.on_clicked(move || {
    //!         // access the winit window
    //!         let app = app_weak.unwrap();
    //!         app.window().with_winit_window(|winit_window: &winit::window::Window| {
    //!             eprintln!("window id = {:#?}", winit_window.id());
    //!         });
    //!     });
    //!     app.run()?;
    //!     Ok(())
    //! }
    //! ```
    //! See also [`BackendSelector::with_winit_event_loop_builder()`](crate::BackendSelector::with_winit_event_loop_builder())
    //! and [`BackendSelector::with_winit_window_attributes_hook()`](crate::BackendSelector::with_winit_window_attributes_hook()).

    pub use i_slint_backend_winit::{
        winit, CustomApplicationHandler, EventLoopBuilder, EventResult, SlintEvent,
        WinitWindowAccessor,
    };

    #[deprecated(note = "Renamed to `EventResult`")]
    /// Deprecated alias to [`EventResult`]
    pub type WinitWindowEventResult = EventResult;
}
