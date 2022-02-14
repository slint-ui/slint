// Copyright © SixtyFPS GmbH <info@slint-ui.com>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-commercial

/*!
This module contains types that are public and re-exported in the slint-rs as well as the slint-interpreter crate as public API.
*/

use alloc::boxed::Box;
use alloc::rc::Rc;

use crate::component::ComponentVTable;
use crate::window::WindowRc;

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

/// This enum describes the different error scenarios that may occur when the applicaton
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
pub struct Window(WindowRc);

#[doc(hidden)]
impl From<WindowRc> for Window {
    fn from(window: WindowRc) -> Self {
        Self(window)
    }
}

impl Window {
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
        self.0.set_rendering_notifier(Box::new(callback))
    }

    /// This function issues a request to the windowing system to redraw the contents of the window.
    pub fn request_redraw(&self) {
        self.0.request_redraw();

        // When this function is called by the user, we want it to translate to a requestAnimationFrame()
        // on the web. If called through the rendering notifier (so from within the event loop processing),
        // unfortunately winit will only do that if set the control flow to Poll. This hack achieves that.
        // Similarly, the winit win32 event loop doesn't queue the redraw request and needs a Poll nudge.
        #[cfg(any(target_arch = "wasm32", target_os = "windows"))]
        crate::animations::CURRENT_ANIMATION_DRIVER
            .with(|driver| driver.set_has_active_animations());
    }
}

impl crate::window::WindowHandleAccess for Window {
    fn window_handle(&self) -> &Rc<crate::window::Window> {
        &self.0
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
/// let thread = std::thread::spawn(move || {
///     // ... Do some computation in the thread
///     let foo = 42;
///      // now forward the data to the main thread using invoke_from_event_loop
///     let handle_copy = handle_weak.clone();
///     slint::invoke_from_event_loop(move || handle_copy.unwrap().set_foo(foo));
/// });
/// # thread.join().unwrap(); return; // don't run the event loop in examples
/// handle.run();
/// ```
pub fn invoke_from_event_loop(func: impl FnOnce() + Send + 'static) {
    if let Some(backend) = crate::backend::instance() {
        backend.post_event(alloc::boxed::Box::new(func))
    } else {
        panic!("slint::invoke_from_event_loop() must be called after the Slint backend is initialized.")
    }
}
