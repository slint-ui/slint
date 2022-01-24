// Copyright © SixtyFPS GmbH <info@sixtyfps.io>
// SPDX-License-Identifier: (GPL-3.0-only OR LicenseRef-SixtyFPS-commercial)

/*!
# SixtyFPS

This crate is the main entry point for embedding user interfaces designed with
[SixtyFPS UI](https://sixtyfps.io/) in Rust programs.

If you are new to SixtyFPS, start with the [Walk-through tutorial](https://sixtyfps.io/docs/tutorial/rust).
If you are already familiar with SixtyFPS, the following topics provide some

## Foundation fopics

 * [The `.60` language reference](docs::langref)
 * [Builtin Elements](docs::builtin_elements)
 * [Widgets](docs::widgets)
 * [Positioning and Layout of Elements](docs::layouting)
 * [Debugging Techniques](docs::debugging_techniques)

## How to use this crate:

Designs of user interfaces are described in the `.60` design markup language. There are three ways
of including them in Rust:

 - The `.60` code is [inline in a macro](#the-60-code-in-a-macro).
 - The `.60` code in [external files compiled with `build.rs`](#the-60-code-in-external-files-is-compiled-with-buildrs)
 - The `.60` code is loaded dynamically at run-time from the file system, by using the [interpreter API](https://docs.rs/sixtyfps-interpreter/latest/sixtyfps_interpreter/).

With the first two methods, the markup code is translated to Rust code and each component is turned into a Rust
struct with functions. Use these functions to instantiate and show the component, and
to access declared properties. Check out our [sample component](docs::generated_code::SampleComponent) for more
information about the generation functions and how to use them.

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

### The .60 code in external files is compiled with `build.rs`

When your design becomes bigger in terms of markup code, you may want move it to a dedicated
`.60` file. It's also possible to split a `.60` file into multiple files using (modules)[docs::langref#modules].
Use a [build script](https://doc.rust-lang.org/cargo/reference/build-scripts.html) to compile
your main `.60` file:

In your Cargo.toml add a `build` assignment and use the `sixtyfps-build` crate in `build-dependencies`:

```toml
[package]
...
build = "build.rs"
resolver = "2" # avoid dependency conflicts on some platforms
edition = "2018"

[dependencies]
sixtyfps = "0.1.6"
...

[build-dependencies]
sixtyfps-build = "0.1.6"
```

Use the API of the sixtyfps-build crate in the `build.rs` file:

```ignore
fn main() {
    sixtyfps_build::compile("ui/hello.60").unwrap();
}
```

Finally, use the [`include_modules!`] macro in your `main.rs`:

```ignore
sixtyfps::include_modules!();
fn main() {
    HelloWorld::new().run();
}
```

The [cargo-generate](https://github.com/cargo-generate/cargo-generate) tool is a great tool to up and running quickly with a new
Rust project. You can use it in combination with our [Template Repository](https://github.com/sixtyfpsui/sixtyfps-rust-template) to
create a skeleton file hierarchy that uses this method:

```bash
cargo install cargo-generate
cargo generate --git https://github.com/sixtyfpsui/sixtyfps-rust-template
```

## Generated components

Currently, only the last component in a `.60` source file is mapped to a Rust structure that be instantiated. We are tracking the
resolution of this limitation in <https://github.com/sixtyfpsui/sixtyfps/issues/784>.

The component is generated and re-exported to the location of the [`include_modules!`]  or [`sixtyfps!`] macro. It is represented
as a struct with the same name as the component.

For example, if you have

```60
export MyComponent := Window { /*...*/ }
```

in the .60 file, it will create a
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

The types used for properties in `.60` design markup each translate to specific types in Rust.
The follow table summarizes the entire mapping:

| `.60` Type | Rust Type | Note |
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
| structure | `struct` of the same name | |
| array | [`ModelHandle`] |  |

For user defined structures in the .60, an extra struct is generated.
For example, if the `.60` contains
```60,ignore
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

#![warn(missing_docs)]
#![deny(unsafe_code)]
#![doc(html_logo_url = "https://sixtyfps.io/resources/logo.drawio.svg")]
#![cfg_attr(not(feature = "std"), no_std)]
extern crate alloc;
pub use sixtyfps_macros::sixtyfps;

pub use sixtyfps_corelib::graphics::{
    Brush, Color, Image, LoadImageError, Rgb8Pixel, Rgba8Pixel, RgbaColor, SharedPixelBuffer,
};
pub use sixtyfps_corelib::model::{
    Model, ModelHandle, ModelNotify, ModelPeer, ModelTracker, StandardListViewItem, VecModel,
};
pub use sixtyfps_corelib::sharedvector::SharedVector;
pub use sixtyfps_corelib::string::SharedString;
pub use sixtyfps_corelib::timers::{Timer, TimerMode};

/// This function can be used to register a custom TrueType font with SixtyFPS,
/// for use with the `font-family` property. The provided slice must be a valid TrueType
/// font.
#[doc(hidden)]
#[cfg(feature = "std")]
pub fn register_font_from_memory(data: &'static [u8]) -> Result<(), Box<dyn std::error::Error>> {
    sixtyfps_rendering_backend_default::backend().register_font_from_memory(data)
}

/// This function can be used to register a custom TrueType font with SixtyFPS,
/// for use with the `font-family` property. The provided path must refer to a valid TrueType
/// font.
#[doc(hidden)]
#[cfg(feature = "std")]
pub fn register_font_from_path<P: AsRef<std::path::Path>>(
    path: P,
) -> Result<(), Box<dyn std::error::Error>> {
    sixtyfps_rendering_backend_default::backend().register_font_from_path(path.as_ref())
}

/// internal re_exports used by the macro generated
#[doc(hidden)]
pub mod re_exports {
    pub use alloc::boxed::Box;
    pub use alloc::format;
    pub use alloc::rc::{Rc, Weak};
    pub use alloc::string::String;
    pub use alloc::{vec, vec::Vec};
    pub use const_field_offset::{self, FieldOffsets, PinnedDrop};
    pub use core::iter::FromIterator;
    pub use once_cell::race::OnceBox;
    pub use once_cell::unsync::OnceCell;
    pub use pin_weak::rc::PinWeak;
    pub use sixtyfps_corelib::animations::EasingCurve;
    pub use sixtyfps_corelib::callbacks::Callback;
    pub use sixtyfps_corelib::component::{
        free_component_item_graphics_resources, init_component_items, Component, ComponentRefPin,
        ComponentVTable,
    };
    pub use sixtyfps_corelib::graphics::*;
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
    pub use sixtyfps_corelib::properties::{
        set_state_binding, Property, PropertyTracker, StateInfo,
    };
    pub use sixtyfps_corelib::slice::Slice;
    pub use sixtyfps_corelib::window::{Window, WindowHandleAccess, WindowRc};
    pub use sixtyfps_corelib::Color;
    pub use sixtyfps_corelib::ComponentVTable_static;
    pub use sixtyfps_corelib::SharedString;
    pub use sixtyfps_corelib::SharedVector;
    pub use sixtyfps_rendering_backend_default::native_widgets::*;
    pub use vtable::{self, *};
}

#[doc(hidden)]
pub mod internal {
    use crate::re_exports::*;
    use alloc::rc::Rc;
    use core::pin::Pin;

    // Helper functions called from generated code to reduce code bloat from
    // extra copies of the original functions for each call site due to
    // the impl Fn() they are taking.

    pub trait StrongComponentRef: Sized {
        type Weak: Clone + 'static;
        fn to_weak(&self) -> Self::Weak;
        fn from_weak(weak: &Self::Weak) -> Option<Self>;
    }

    impl<C: 'static> StrongComponentRef for VRc<ComponentVTable, C> {
        type Weak = VWeak<ComponentVTable, C>;
        fn to_weak(&self) -> Self::Weak {
            VRc::downgrade(self)
        }
        fn from_weak(weak: &Self::Weak) -> Option<Self> {
            weak.upgrade()
        }
    }

    impl<C: 'static> StrongComponentRef for VRcMapped<ComponentVTable, C> {
        type Weak = VWeakMapped<ComponentVTable, C>;
        fn to_weak(&self) -> Self::Weak {
            VRcMapped::downgrade(self)
        }
        fn from_weak(weak: &Self::Weak) -> Option<Self> {
            weak.upgrade()
        }
    }

    impl<C: 'static> StrongComponentRef for Pin<Rc<C>> {
        type Weak = PinWeak<C>;
        fn to_weak(&self) -> Self::Weak {
            PinWeak::downgrade(self.clone())
        }
        fn from_weak(weak: &Self::Weak) -> Option<Self> {
            weak.upgrade()
        }
    }

    pub fn set_property_binding<T: Clone + 'static, StrongRef: StrongComponentRef + 'static>(
        property: Pin<&Property<T>>,
        component_strong: &StrongRef,
        binding: fn(StrongRef) -> T,
    ) {
        let weak = component_strong.to_weak();
        property.set_binding(move || {
            binding(<StrongRef as StrongComponentRef>::from_weak(&weak).unwrap())
        })
    }

    pub fn set_animated_property_binding<
        T: Clone + sixtyfps_corelib::properties::InterpolatedPropertyValue + 'static,
        StrongRef: StrongComponentRef + 'static,
    >(
        property: Pin<&Property<T>>,
        component_strong: &StrongRef,
        binding: fn(StrongRef) -> T,
        animation_data: PropertyAnimation,
    ) {
        let weak = component_strong.to_weak();
        property.set_animated_binding(
            move || binding(<StrongRef as StrongComponentRef>::from_weak(&weak).unwrap()),
            animation_data,
        )
    }

    pub fn set_animated_property_binding_for_transition<
        T: Clone + sixtyfps_corelib::properties::InterpolatedPropertyValue + 'static,
        StrongRef: StrongComponentRef + 'static,
    >(
        property: Pin<&Property<T>>,
        component_strong: &StrongRef,
        binding: fn(StrongRef) -> T,
        compute_animation_details: fn(
            StrongRef,
        )
            -> (PropertyAnimation, sixtyfps_corelib::animations::Instant),
    ) {
        let weak_1 = component_strong.to_weak();
        let weak_2 = weak_1.clone();
        property.set_animated_binding_for_transition(
            move || binding(<StrongRef as StrongComponentRef>::from_weak(&weak_1).unwrap()),
            move || {
                compute_animation_details(
                    <StrongRef as StrongComponentRef>::from_weak(&weak_2).unwrap(),
                )
            },
        )
    }

    pub fn set_property_state_binding<StrongRef: StrongComponentRef + 'static>(
        property: Pin<&Property<StateInfo>>,
        component_strong: &StrongRef,
        binding: fn(StrongRef) -> i32,
    ) {
        let weak = component_strong.to_weak();
        crate::re_exports::set_state_binding(property, move || {
            binding(<StrongRef as StrongComponentRef>::from_weak(&weak).unwrap())
        })
    }

    pub fn set_callback_handler<
        Arg: ?Sized + 'static,
        Ret: Default + 'static,
        StrongRef: StrongComponentRef + 'static,
    >(
        callback: Pin<&Callback<Arg, Ret>>,
        component_strong: &StrongRef,
        handler: fn(StrongRef, &Arg) -> Ret,
    ) {
        let weak = component_strong.to_weak();
        callback.set_handler(move |arg| {
            handler(<StrongRef as StrongComponentRef>::from_weak(&weak).unwrap(), arg)
        })
    }
}

/// Creates a new window to render components in.
#[doc(hidden)]
pub fn create_window() -> re_exports::WindowRc {
    sixtyfps_rendering_backend_default::backend().create_window()
}

/// Enters the main event loop. This is necessary in order to receive
/// events from the windowing system in order to render to the screen
/// and react to user input.
pub fn run_event_loop() {
    sixtyfps_rendering_backend_default::backend()
        .run_event_loop(sixtyfps_corelib::backend::EventLoopQuitBehavior::QuitOnLastWindowClosed);
}

/// Schedules the main event loop for termination. This function is meant
/// to be called from callbacks triggered by the UI. After calling the function,
/// it will return immediately and once control is passed back to the event loop,
/// the initial call to [`run_event_loop()`] will return.
pub fn quit_event_loop() {
    sixtyfps_rendering_backend_default::backend().quit_event_loop();
}

/// Adds the specified function to an internal queue, notifies the event loop to wake up.
/// Once woken up, any queued up functors will be invoked.
///
/// This function is thread-safe and can be called from any thread, including the one
/// running the event loop. The provided functors will only be invoked from the thread
/// that started the event loop.
///
/// You can use this to set properties or use any other SixtyFPS APIs from other threads,
/// by collecting the code in a functor and queuing it up for invocation within the event loop.
///
/// See also [`Weak::upgrade_in_event_loop`]
///
/// # Example
/// ```rust
/// sixtyfps::sixtyfps! { MyApp := Window { property <int> foo; /* ... */ } }
/// let handle = MyApp::new();
/// let handle_weak = handle.as_weak();
/// let thread = std::thread::spawn(move || {
///     // ... Do some computation in the thread
///     let foo = 42;
///      // now forward the data to the main thread using invoke_from_event_loop
///     let handle_copy = handle_weak.clone();
///     sixtyfps::invoke_from_event_loop(move || handle_copy.unwrap().set_foo(foo));
/// });
/// # thread.join().unwrap(); return; // don't run the event loop in examples
/// handle.run();
/// ```
pub fn invoke_from_event_loop(func: impl FnOnce() + Send + 'static) {
    sixtyfps_rendering_backend_default::backend().post_event(alloc::boxed::Box::new(func))
}

/// This trait is used to obtain references to global singletons exported in `.60`
/// markup. Alternatively, you can use [`ComponentHandle::global`] to obtain access.
///
/// This trait is implemented by the compiler for each global singleton that's exported.
///
/// # Example
/// The following example of `.60` markup defines a global singleton called `Palette`, exports
/// it and modifies it from Rust code:
/// ```rust
/// sixtyfps::sixtyfps!{
/// export global Palette := {
///     property<color> foreground-color;
///     property<color> background-color;
/// }
///
/// export App := Window {
///    background: Palette.background-color;
///    Text {
///       text: "Hello";
///       color: Palette.foreground-color;
///    }
///    // ...
/// }
/// }
/// let app = App::new();
/// app.global::<Palette>().set_background_color(sixtyfps::Color::from_rgb_u8(0, 0, 0));
///
/// // alternate way to access the global singleton:
/// Palette::get(&app).set_foreground_color(sixtyfps::Color::from_rgb_u8(255, 255, 255));
/// ```
///
/// See also the [language reference for global singletons](docs/langref/index.html#global-singletons) for more information.
pub trait Global<'a, Component> {
    /// Returns a reference that's tied to the life time of the provided component.
    fn get(component: &'a Component) -> Self;
}

/// This trait describes the common public API of a strongly referenced SixtyFPS component.
/// It allows creating strongly-referenced clones, a conversion into/ a weak pointer as well
/// as other convenience functions.
///
/// This trait is implemented by the [generated component](mod@crate#generated-components)
pub trait ComponentHandle {
    /// The type of the generated component.
    #[doc(hidden)]
    type Inner;
    /// Returns a new weak pointer.
    fn as_weak(&self) -> Weak<Self>
    where
        Self: Sized;

    /// Returns a clone of this handle that's a strong reference.
    #[must_use]
    fn clone_strong(&self) -> Self;

    /// Internal function used when upgrading a weak reference to a strong one.
    #[doc(hidden)]
    fn from_inner(_: vtable::VRc<re_exports::ComponentVTable, Self::Inner>) -> Self;

    /// Marks the window of this component to be shown on the screen. This registers
    /// the window with the windowing system. In order to react to events from the windowing system,
    /// such as draw requests or mouse/touch input, it is still necessary to spin the event loop,
    /// using [`crate::run_event_loop`].
    fn show(&self);

    /// Marks the window of this component to be hidden on the screen. This de-registers
    /// the window from the windowing system and it will not receive any further events.
    fn hide(&self);

    /// Returns the Window associated with this component. The window API can be used
    /// to control different aspects of the integration into the windowing system,
    /// such as the position on the screen.
    fn window(&self) -> &Window;

    /// This is a convenience function that first calls [`Self::show`], followed by [`crate::run_event_loop()`]
    /// and [`Self::hide`].
    fn run(&self);

    /// This function provides access to instances of global singletons exported in `.60`.
    /// See [`Global`] for an example how to export and access globals from `.60` markup.
    fn global<'a, T: Global<'a, Self>>(&'a self) -> T
    where
        Self: Sized;
}

mod weak_handle {

    use super::*;

    /// Struct that's used to hold weak references of [SixtyFPS component](mod@crate#generated-components)
    ///
    /// In order to create a Weak, you should use [`ComponentHandle::as_weak`].
    ///
    /// Strong references should not be captured by the functions given to a lambda,
    /// as this would produce a reference loop and leak the component.
    /// Instead, the callback function should capture a weak component.
    ///
    /// The Weak component also implement `Send` and can be send to another thread.
    /// but the upgrade function will only return a valid component from the same thread
    /// as the one it has been created from.
    /// This is useful to use with [`invoke_from_event_loop()`] or [`Self::upgrade_in_event_loop()`].
    pub struct Weak<T: ComponentHandle> {
        inner: vtable::VWeak<re_exports::ComponentVTable, T::Inner>,
        #[cfg(feature = "std")]
        thread: std::thread::ThreadId,
    }

    impl<T: ComponentHandle> Clone for Weak<T> {
        fn clone(&self) -> Self {
            Self {
                inner: self.inner.clone(),
                #[cfg(feature = "std")]
                thread: self.thread,
            }
        }
    }

    impl<T: ComponentHandle> Weak<T> {
        #[doc(hidden)]
        pub fn new(rc: &vtable::VRc<re_exports::ComponentVTable, T::Inner>) -> Self {
            Self {
                inner: vtable::VRc::downgrade(rc),
                #[cfg(feature = "std")]
                thread: std::thread::current().id(),
            }
        }

        /// Returns a new strongly referenced component if some other instance still
        /// holds a strong reference. Otherwise, returns None.
        ///
        /// This also returns None if the current thread is not the thread that created
        /// the component
        pub fn upgrade(&self) -> Option<T>
        where
            T: ComponentHandle,
        {
            #[cfg(feature = "std")]
            if std::thread::current().id() != self.thread {
                return None;
            }
            self.inner.upgrade().map(T::from_inner)
        }

        /// Convenience function that returns a new strongly referenced component if
        /// some other instance still holds a strong reference and the current thread
        /// is the thread that created this component.
        /// Otherwise, this function panics.
        pub fn unwrap(&self) -> T {
            self.upgrade().unwrap()
        }

        /// Convenience function that combines [`invoke_from_event_loop()`] with [`Self::upgrade()`]
        ///
        /// The given functor will be added to an internal queue and will wake the event loop.
        /// On the next iteration of the event loop, the functor will be executed with a `T` as an argument.
        ///
        /// If the component was dropped because there are no more strong reference to the component,
        /// the functor will not be called.
        ///
        /// # Example
        /// ```rust
        /// sixtyfps::sixtyfps! { MyApp := Window { property <int> foo; /* ... */ } }
        /// let handle = MyApp::new();
        /// let handle_weak = handle.as_weak();
        /// let thread = std::thread::spawn(move || {
        ///     // ... Do some computation in the thread
        ///     let foo = 42;
        ///     # assert!(handle_weak.upgrade().is_none()); // note that upgrade fails in a thread
        ///     // now forward the data to the main thread using upgrade_in_event_loop
        ///     handle_weak.upgrade_in_event_loop(move |handle| handle.set_foo(foo));
        /// });
        /// # thread.join().unwrap(); return; // don't run the event loop in examples
        /// handle.run();
        /// ```
        #[cfg(feature = "std")]
        pub fn upgrade_in_event_loop(self, func: impl FnOnce(T) + Send + 'static)
        where
            T: 'static,
        {
            crate::invoke_from_event_loop(move || {
                if let Some(h) = self.upgrade() {
                    func(h);
                }
            })
        }
    }

    // Safety: we make sure in upgrade that the thread is the proper one,
    // and the VWeak only use atomic pointer so it is safe to clone and drop in another thread
    #[allow(unsafe_code)]
    #[cfg(feature = "std")]
    unsafe impl<T: ComponentHandle> Send for Weak<T> {}
}

pub use weak_handle::*;

pub use sixtyfps_corelib::window::api::Window;

/// This module contains functions useful for unit tests
#[cfg(feature = "std")]
pub mod testing {
    use core::cell::Cell;
    thread_local!(static KEYBOARD_MODIFIERS : Cell<crate::re_exports::KeyboardModifiers> = Default::default());

    use super::ComponentHandle;

    pub use sixtyfps_corelib::tests::sixtyfps_mock_elapsed_time as mock_elapsed_time;

    /// Simulate a mouse click
    pub fn send_mouse_click<
        X: vtable::HasStaticVTable<sixtyfps_corelib::component::ComponentVTable>
            + crate::re_exports::WindowHandleAccess
            + 'static,
        Component: Into<vtable::VRc<sixtyfps_corelib::component::ComponentVTable, X>> + ComponentHandle,
    >(
        component: &Component,
        x: f32,
        y: f32,
    ) {
        let rc = component.clone_strong().into();
        let dyn_rc = vtable::VRc::into_dyn(rc.clone());
        sixtyfps_corelib::tests::sixtyfps_send_mouse_click(
            &dyn_rc,
            x,
            y,
            &rc.window_handle().clone(),
        );
    }

    /// Simulate a change in keyboard modifiers being pressed
    pub fn set_current_keyboard_modifiers<
        X: vtable::HasStaticVTable<sixtyfps_corelib::component::ComponentVTable>
            + crate::re_exports::WindowHandleAccess,
        Component: Into<vtable::VRc<sixtyfps_corelib::component::ComponentVTable, X>> + ComponentHandle,
    >(
        _component: &Component,
        modifiers: crate::re_exports::KeyboardModifiers,
    ) {
        KEYBOARD_MODIFIERS.with(|x| x.set(modifiers))
    }

    /// Simulate entering a sequence of ascii characters key by key.
    pub fn send_keyboard_string_sequence<
        X: vtable::HasStaticVTable<sixtyfps_corelib::component::ComponentVTable>
            + crate::re_exports::WindowHandleAccess,
        Component: Into<vtable::VRc<sixtyfps_corelib::component::ComponentVTable, X>> + ComponentHandle,
    >(
        component: &Component,
        sequence: &str,
    ) {
        let component = component.clone_strong().into();
        sixtyfps_corelib::tests::send_keyboard_string_sequence(
            &super::SharedString::from(sequence),
            KEYBOARD_MODIFIERS.with(|x| x.get()),
            &component.window_handle().clone(),
        )
    }

    /// Applies the specified scale factor to the window that's associated with the given component.
    /// This overrides the value provided by the windowing system.
    pub fn set_window_scale_factor<
        X: vtable::HasStaticVTable<sixtyfps_corelib::component::ComponentVTable>
            + crate::re_exports::WindowHandleAccess,
        Component: Into<vtable::VRc<sixtyfps_corelib::component::ComponentVTable, X>> + ComponentHandle,
    >(
        component: &Component,
        factor: f32,
    ) {
        let component = component.clone_strong().into();
        component.window_handle().set_scale_factor(factor)
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
pub struct VersionCheck_0_2_0;

#[cfg(doctest)]
mod compile_fail_tests;

#[cfg(doc)]
pub mod docs;
