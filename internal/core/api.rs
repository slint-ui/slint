// Copyright © SixtyFPS GmbH <info@slint-ui.com>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-commercial

/*!
This module contains types that are public and re-exported in the slint-rs as well as the slint-interpreter crate as public API.
*/

#![warn(missing_docs)]

use alloc::boxed::Box;

use crate::component::ComponentVTable;
use crate::window::{WindowAdapter, WindowInner};

pub use crate::lengths::LogicalPx;
pub use crate::lengths::PhysicalPx;

pub use euclid;

/// This enum describes a low-level access to specific graphics APIs used
/// by the renderer.
#[derive(Clone)]
#[non_exhaustive]
pub enum GraphicsAPI<'a> {
    /// The rendering is done using OpenGL.
    NativeOpenGL {
        /// Use this function pointer to obtain access to the OpenGL implementation - similar to `eglGetProcAddress`.
        get_proc_address: &'a dyn Fn(&str) -> *const core::ffi::c_void,
    },
    /// The rendering is done on a HTML Canvas element using WebGL.
    WebGL {
        /// The DOM element id of the HTML Canvas element used for rendering.
        canvas_element_id: &'a str,
        /// The drawing context type used on the HTML Canvas element for rendering. This is the argument to the
        /// `getContext` function on the HTML Canvas element.
        context_type: &'a str,
    },
}

impl<'a> core::fmt::Debug for GraphicsAPI<'a> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            GraphicsAPI::NativeOpenGL { .. } => write!(f, "GraphicsAPI::NativeOpenGL"),
            GraphicsAPI::WebGL { context_type, .. } => {
                write!(f, "GraphicsAPI::WebGL(context_type = {})", context_type)
            }
        }
    }
}

/// This enum describes the different rendering states, that will be provided
/// to the parameter of the callback for `set_rendering_notifier` on the `slint::Window`.
#[derive(Debug, Clone)]
#[repr(C)]
#[non_exhaustive]
pub enum RenderingState {
    /// The window has been created and the graphics adapter/context initialized. When OpenGL
    /// is used for rendering, the context will be current.
    RenderingSetup,
    /// The scene of items is about to be rendered.  When OpenGL
    /// is used for rendering, the context will be current.
    BeforeRendering,
    /// The scene of items was rendered, but the back buffer was not sent for display presentation
    /// yet (for example GL swap buffers). When OpenGL is used for rendering, the context will be current.
    AfterRendering,
    /// The window will be destroyed and/or graphics resources need to be released due to other
    /// constraints.
    RenderingTeardown,
}

/// Internal trait that's used to map rendering state callbacks to either a Rust-API provided
/// impl FnMut or a struct that invokes a C callback and implements Drop to release the closure
/// on the C++ side.
pub trait RenderingNotifier {
    /// Called to notify that rendering has reached a certain state.
    fn notify(&mut self, state: RenderingState, graphics_api: &GraphicsAPI);
}

impl<F: FnMut(RenderingState, &GraphicsAPI)> RenderingNotifier for F {
    fn notify(&mut self, state: RenderingState, graphics_api: &GraphicsAPI) {
        self(state, graphics_api)
    }
}

/// This enum describes the different error scenarios that may occur when the application
/// registers a rendering notifier on a [`crate::Window`](struct.Window.html).
#[derive(Debug, Clone)]
#[repr(C)]
#[non_exhaustive]
pub enum SetRenderingNotifierError {
    /// The rendering backend does not support rendering notifiers.
    Unsupported,
    /// There is already a rendering notifier set, multiple notifiers are not supported.
    AlreadySet,
}

/// This type represents a window towards the windowing system, that's used to render the
/// scene of a component. It provides API to control windowing system specific aspects such
/// as the position on the screen.
#[repr(transparent)]
pub struct Window(WindowInner);

/// This enum describes whether a Window is allowed to be hidden when the user tries to close the window.
/// It is the return type of the callback provided to [Window::on_close_requested].
#[derive(Copy, Clone, Debug, PartialEq)]
#[repr(C)]
pub enum CloseRequestResponse {
    /// The Window will be hidden (default action)
    HideWindow,
    /// The close request is rejected and the window will be kept shown.
    KeepWindowShown,
}

impl Default for CloseRequestResponse {
    fn default() -> Self {
        Self::HideWindow
    }
}

impl Window {
    /// Create a new window from a window adapter
    ///
    /// You only need to create the window yourself when you create a
    /// [`WindowAdapter`](crate::platform::WindowAdapter) from
    /// [`Platform::create_window_adapter`](crate::platform::Platform::create_window_adapter)
    ///
    /// Since the window adapter must own the Window, this function is meant to be used with
    /// [`Rc::new_cyclic`](alloc::rc::Rc::new_cyclic)
    ///
    /// # Example
    /// ```rust
    /// use std::rc::Rc;
    /// use slint::platform::WindowAdapter;
    /// use slint::Window;
    /// struct MyWindowAdapter {
    ///     window: Window,
    ///     //...
    /// }
    /// impl WindowAdapter for MyWindowAdapter {
    ///    fn window(&self) -> &Window { &self.window }
    /// # fn renderer(&self) -> &dyn i_slint_core::renderer::Renderer { unimplemented!() }
    /// # fn as_any(&self) -> &(dyn core::any::Any + 'static) { self }
    ///    //...
    /// }
    /// fn create_window_adapter() -> Rc<dyn WindowAdapter> {
    ///    Rc::<MyWindowAdapter>::new_cyclic(|weak| {
    ///        MyWindowAdapter {
    ///           window: Window::new(weak.clone()),
    ///           //...
    ///        }
    ///    })
    /// }
    /// ```
    pub fn new(window_adapter_weak: alloc::rc::Weak<dyn WindowAdapter>) -> Self {
        Self(WindowInner::new(window_adapter_weak))
    }

    /// Registers the window with the windowing system in order to make it visible on the screen.
    pub fn show(&self) {
        self.0.show();
    }

    /// De-registers the window from the windowing system, therefore hiding it.
    pub fn hide(&self) {
        self.0.hide();
    }

    /// This function allows registering a callback that's invoked during the different phases of
    /// rendering. This allows custom rendering on top or below of the scene.
    pub fn set_rendering_notifier(
        &self,
        callback: impl FnMut(RenderingState, &GraphicsAPI) + 'static,
    ) -> Result<(), SetRenderingNotifierError> {
        self.0.window_adapter().renderer().set_rendering_notifier(Box::new(callback))
    }

    /// This function allows registering a callback that's invoked when the user tries to close a window.
    /// The callback has to return a [CloseRequestResponse].
    pub fn on_close_requested(&self, callback: impl FnMut() -> CloseRequestResponse + 'static) {
        self.0.on_close_requested(callback);
    }

    /// This function issues a request to the windowing system to redraw the contents of the window.
    pub fn request_redraw(&self) {
        self.0.window_adapter().request_redraw();

        // When this function is called by the user, we want it to translate to a requestAnimationFrame()
        // on the web. If called through the rendering notifier (so from within the event loop processing),
        // unfortunately winit will only do that if set the control flow to Poll. This hack achieves that.
        // Similarly, the winit win32 event loop doesn't queue the redraw request and needs a Poll nudge.
        #[cfg(any(target_arch = "wasm32", target_os = "windows"))]
        crate::animations::CURRENT_ANIMATION_DRIVER
            .with(|driver| driver.set_has_active_animations());
    }

    /// This function returns an euclid scale that allows conveniently converting between logical and
    /// physical pixels based on the window's scale factor.
    pub fn scale_factor(&self) -> euclid::Scale<f32, LogicalPx, PhysicalPx> {
        self.0.scale()
    }

    /// Returns the position of the window on the screen, in physical screen coordinates and including
    /// a window frame (if present).
    pub fn position(&self) -> euclid::Point2D<i32, PhysicalPx> {
        self.0.window_adapter().position()
    }

    /// Sets the position of the window on the screen, in physical screen coordinates and including
    /// a window frame (if present).
    /// Note that on some windowing systems, such as Wayland, this functionality is not available.
    pub fn set_position(&self, position: euclid::Point2D<i32, PhysicalPx>) {
        self.0.window_adapter().set_position(position)
    }

    /// Returns the size of the window on the screen, in physical screen coordinates and excluding
    /// a window frame (if present).
    pub fn size(&self) -> euclid::Size2D<u32, PhysicalPx> {
        self.0.inner_size.get()
    }

    /// Resizes the window to the specified size on the screen, in physical pixels and excluding
    /// a window frame (if present).
    pub fn set_size(&self, size: euclid::Size2D<u32, PhysicalPx>) {
        if self.0.inner_size.replace(size) == size {
            return;
        }

        let l = size.cast() / self.scale_factor();
        self.0.set_window_item_geometry(l.width as _, l.height as _);
        self.0.window_adapter().set_inner_size(size)
    }

    /// Dispatch a window event to the window
    ///
    /// Any position in the event should be in logical pixel relative to the window coordinate
    ///
    /// Note: This function is usually called by the Slint backend. You should only call this function
    /// if implementing your own backend or for testing purposes.
    pub fn dispatch_event(&self, event: WindowEvent) {
        self.0.process_mouse_input(event.into())
    }

    /// Returns true if there is an animation currently running
    pub fn has_active_animations(&self) -> bool {
        // TODO make it really per window.
        crate::animations::CURRENT_ANIMATION_DRIVER.with(|driver| driver.has_active_animations())
    }
}

impl crate::window::WindowHandleAccess for Window {
    fn window_handle(&self) -> &crate::window::WindowInner {
        &self.0
    }
}

pub use crate::input::PointerEventButton;

/// An event sent to a window
#[allow(missing_docs)]
#[derive(Debug, Clone, Copy, PartialEq)]
#[non_exhaustive]
pub enum WindowEvent {
    /// The mouse or finger was pressed
    PointerPressed { position: euclid::Point2D<f32, LogicalPx>, button: PointerEventButton },
    /// The mouse or finger was released
    PointerReleased { position: euclid::Point2D<f32, LogicalPx>, button: PointerEventButton },
    /// The position of the pointer has changed
    PointerMoved { position: euclid::Point2D<f32, LogicalPx> },
    /// Wheel was rotated.
    /// `pos` is the position of the mouse when the event happens.
    /// `delta` is the amount of pixel to scroll.
    PointerScrolled {
        position: euclid::Point2D<f32, LogicalPx>,
        delta: euclid::Vector2D<f32, LogicalPx>,
    },
    /// The mouse exited the item or component
    PointerExited,
}

impl WindowEvent {
    /// The position of the cursor for this event, if any
    pub fn position(&self) -> Option<euclid::Point2D<f32, LogicalPx>> {
        match self {
            WindowEvent::PointerPressed { position, .. } => Some(*position),
            WindowEvent::PointerReleased { position, .. } => Some(*position),
            WindowEvent::PointerMoved { position } => Some(*position),
            WindowEvent::PointerScrolled { position, .. } => Some(*position),
            WindowEvent::PointerExited => None,
        }
    }
}

/// This trait is used to obtain references to global singletons exported in `.slint`
/// markup. Alternatively, you can use [`ComponentHandle::global`] to obtain access.
///
/// This trait is implemented by the compiler for each global singleton that's exported.
///
/// # Example
/// The following example of `.slint` markup defines a global singleton called `Palette`, exports
/// it and modifies it from Rust code:
/// ```rust
/// # i_slint_backend_testing::init();
/// slint::slint!{
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
/// app.global::<Palette>().set_background_color(slint::Color::from_rgb_u8(0, 0, 0));
///
/// // alternate way to access the global singleton:
/// Palette::get(&app).set_foreground_color(slint::Color::from_rgb_u8(255, 255, 255));
/// ```
///
/// See also the [language reference for global singletons](docs/langref/index.html#global-singletons) for more information.
///
/// **Note:** Only globals that are exported or re-exported from the main .slint file will
/// be exposed in the API
pub trait Global<'a, Component> {
    /// Returns a reference that's tied to the life time of the provided component.
    fn get(component: &'a Component) -> Self;
}

/// This trait describes the common public API of a strongly referenced Slint component.
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
    fn from_inner(_: vtable::VRc<ComponentVTable, Self::Inner>) -> Self;

    /// Marks the window of this component to be shown on the screen. This registers
    /// the window with the windowing system. In order to react to events from the windowing system,
    /// such as draw requests or mouse/touch input, it is still necessary to spin the event loop,
    /// using [`crate::run_event_loop`](fn.run_event_loop.html).
    fn show(&self);

    /// Marks the window of this component to be hidden on the screen. This de-registers
    /// the window from the windowing system and it will not receive any further events.
    fn hide(&self);

    /// Returns the Window associated with this component. The window API can be used
    /// to control different aspects of the integration into the windowing system,
    /// such as the position on the screen.
    fn window(&self) -> &Window;

    /// This is a convenience function that first calls [`Self::show`], followed by [`crate::run_event_loop()`](fn.run_event_loop.html)
    /// and [`Self::hide`].
    fn run(&self);

    /// This function provides access to instances of global singletons exported in `.slint`.
    /// See [`Global`] for an example how to export and access globals from `.slint` markup.
    fn global<'a, T: Global<'a, Self>>(&'a self) -> T
    where
        Self: Sized;
}

mod weak_handle {

    use super::*;

    /// Struct that's used to hold weak references of a [Slint component](mod@crate#generated-components)
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
        inner: vtable::VWeak<ComponentVTable, T::Inner>,
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
        pub fn new(rc: &vtable::VRc<ComponentVTable, T::Inner>) -> Self {
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
        /// # i_slint_backend_testing::init();
        /// slint::slint! { MyApp := Window { property <int> foo; /* ... */ } }
        /// let handle = MyApp::new();
        /// let handle_weak = handle.as_weak();
        /// let thread = std::thread::spawn(move || {
        ///     // ... Do some computation in the thread
        ///     let foo = 42;
        ///     # assert!(handle_weak.upgrade().is_none()); // note that upgrade fails in a thread
        ///     # return; // don't upgrade_in_event_loop in our examples
        ///     // now forward the data to the main thread using upgrade_in_event_loop
        ///     handle_weak.upgrade_in_event_loop(move |handle| handle.set_foo(foo));
        /// });
        /// # thread.join().unwrap(); return; // don't run the event loop in examples
        /// handle.run();
        /// ```
        #[cfg(feature = "std")]
        pub fn upgrade_in_event_loop(&self, func: impl FnOnce(T) + Send + 'static)
        where
            T: 'static,
        {
            let weak_handle = self.clone();
            super::invoke_from_event_loop(move || {
                if let Some(h) = weak_handle.upgrade() {
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

/// Adds the specified function to an internal queue, notifies the event loop to wake up.
/// Once woken up, any queued up functors will be invoked.
///
/// This function is thread-safe and can be called from any thread, including the one
/// running the event loop. The provided functors will only be invoked from the thread
/// that started the event loop.
///
/// You can use this to set properties or use any other Slint APIs from other threads,
/// by collecting the code in a functor and queuing it up for invocation within the event loop.
///
/// See also [`Weak::upgrade_in_event_loop`]
///
/// # Example
/// ```rust
/// slint::slint! { MyApp := Window { property <int> foo; /* ... */ } }
/// # i_slint_backend_testing::init();
/// let handle = MyApp::new();
/// let handle_weak = handle.as_weak();
/// # return; // don't run the event loop in examples
/// let thread = std::thread::spawn(move || {
///     // ... Do some computation in the thread
///     let foo = 42;
///      // now forward the data to the main thread using invoke_from_event_loop
///     let handle_copy = handle_weak.clone();
///     slint::invoke_from_event_loop(move || handle_copy.unwrap().set_foo(foo));
/// });
/// handle.run();
/// ```
pub fn invoke_from_event_loop(func: impl FnOnce() + Send + 'static) {
    crate::platform::event_loop_proxy()
        .expect("quit_event_loop() called before the slint platform abstraction was initialized, or the platform does not support event loop")
        .invoke_from_event_loop(alloc::boxed::Box::new(func))
}

/// Schedules the main event loop for termination. This function is meant
/// to be called from callbacks triggered by the UI. After calling the function,
/// it will return immediately and once control is passed back to the event loop,
/// the initial call to `slint::run_event_loop()` will return.
pub fn quit_event_loop() {
    crate::platform::event_loop_proxy()
        .expect("quit_event_loop() called before the slint platform abstraction was initialized, or the platform does not support event loop")
        .quit_event_loop()
}
