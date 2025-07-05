// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

/*!
This module contains types that are public and re-exported in the slint-rs as well as the slint-interpreter crate as public API.
*/

#![warn(missing_docs)]

#[cfg(target_has_atomic = "ptr")]
pub use crate::future::*;
use crate::graphics::{Rgba8Pixel, SharedPixelBuffer};
use crate::input::{KeyEventType, MouseEvent};
use crate::window::{WindowAdapter, WindowInner};
use alloc::boxed::Box;
use alloc::string::String;

/// A position represented in the coordinate space of logical pixels. That is the space before applying
/// a display device specific scale factor.
#[derive(Debug, Default, Copy, Clone, PartialEq)]
#[repr(C)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct LogicalPosition {
    /// The x coordinate.
    pub x: f32,
    /// The y coordinate.
    pub y: f32,
}

impl LogicalPosition {
    /// Construct a new logical position from the given x and y coordinates, that are assumed to be
    /// in the logical coordinate space.
    pub const fn new(x: f32, y: f32) -> Self {
        Self { x, y }
    }

    /// Convert a given physical position to a logical position by dividing the coordinates with the
    /// specified scale factor.
    pub fn from_physical(physical_pos: PhysicalPosition, scale_factor: f32) -> Self {
        Self::new(physical_pos.x as f32 / scale_factor, physical_pos.y as f32 / scale_factor)
    }

    /// Convert this logical position to a physical position by multiplying the coordinates with the
    /// specified scale factor.
    pub fn to_physical(&self, scale_factor: f32) -> PhysicalPosition {
        PhysicalPosition::from_logical(*self, scale_factor)
    }

    pub(crate) fn to_euclid(self) -> crate::lengths::LogicalPoint {
        [self.x as _, self.y as _].into()
    }
    pub(crate) fn from_euclid(p: crate::lengths::LogicalPoint) -> Self {
        Self::new(p.x as _, p.y as _)
    }
}

/// A position represented in the coordinate space of physical device pixels. That is the space after applying
/// a display device specific scale factor to pixels from the logical coordinate space.
#[derive(Debug, Default, Copy, Clone, Eq, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct PhysicalPosition {
    /// The x coordinate.
    pub x: i32,
    /// The y coordinate.
    pub y: i32,
}

impl PhysicalPosition {
    /// Construct a new physical position from the given x and y coordinates, that are assumed to be
    /// in the physical coordinate space.
    pub const fn new(x: i32, y: i32) -> Self {
        Self { x, y }
    }

    /// Convert a given logical position to a physical position by multiplying the coordinates with the
    /// specified scale factor.
    pub fn from_logical(logical_pos: LogicalPosition, scale_factor: f32) -> Self {
        Self::new((logical_pos.x * scale_factor) as i32, (logical_pos.y * scale_factor) as i32)
    }

    /// Convert this physical position to a logical position by dividing the coordinates with the
    /// specified scale factor.
    pub fn to_logical(&self, scale_factor: f32) -> LogicalPosition {
        LogicalPosition::from_physical(*self, scale_factor)
    }

    #[cfg(feature = "ffi")]
    pub(crate) fn to_euclid(&self) -> crate::graphics::euclid::default::Point2D<i32> {
        [self.x, self.y].into()
    }

    #[cfg(feature = "ffi")]
    pub(crate) fn from_euclid(p: crate::graphics::euclid::default::Point2D<i32>) -> Self {
        Self::new(p.x as _, p.y as _)
    }
}

/// The position of the window in either physical or logical pixels. This is used
/// with [`Window::set_position`].
#[derive(Clone, Debug, derive_more::From, PartialEq)]
pub enum WindowPosition {
    /// The position in physical pixels.
    Physical(PhysicalPosition),
    /// The position in logical pixels.
    Logical(LogicalPosition),
}

impl WindowPosition {
    /// Turn the `WindowPosition` into a `PhysicalPosition`.
    pub fn to_physical(&self, scale_factor: f32) -> PhysicalPosition {
        match self {
            WindowPosition::Physical(pos) => *pos,
            WindowPosition::Logical(pos) => pos.to_physical(scale_factor),
        }
    }
}

/// A size represented in the coordinate space of logical pixels. That is the space before applying
/// a display device specific scale factor.
#[repr(C)]
#[derive(Debug, Default, Copy, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct LogicalSize {
    /// The width in logical pixels.
    pub width: f32,
    /// The height in logical.
    pub height: f32,
}

impl LogicalSize {
    /// Construct a new logical size from the given width and height values, that are assumed to be
    /// in the logical coordinate space.
    pub const fn new(width: f32, height: f32) -> Self {
        Self { width, height }
    }

    /// Convert a given physical size to a logical size by dividing width and height by the
    /// specified scale factor.
    pub fn from_physical(physical_size: PhysicalSize, scale_factor: f32) -> Self {
        Self::new(
            physical_size.width as f32 / scale_factor,
            physical_size.height as f32 / scale_factor,
        )
    }

    /// Convert this logical size to a physical size by multiplying width and height with the
    /// specified scale factor.
    pub fn to_physical(&self, scale_factor: f32) -> PhysicalSize {
        PhysicalSize::from_logical(*self, scale_factor)
    }

    pub(crate) fn to_euclid(self) -> crate::lengths::LogicalSize {
        [self.width as _, self.height as _].into()
    }

    pub(crate) fn from_euclid(p: crate::lengths::LogicalSize) -> Self {
        Self::new(p.width as _, p.height as _)
    }
}

/// A size represented in the coordinate space of physical device pixels. That is the space after applying
/// a display device specific scale factor to pixels from the logical coordinate space.
#[derive(Debug, Default, Copy, Clone, Eq, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct PhysicalSize {
    /// The width in physical pixels.
    pub width: u32,
    /// The height in physical pixels;
    pub height: u32,
}

impl PhysicalSize {
    /// Construct a new physical size from the width and height values, that are assumed to be
    /// in the physical coordinate space.
    pub const fn new(width: u32, height: u32) -> Self {
        Self { width, height }
    }

    /// Convert a given logical size to a physical size by multiplying width and height with the
    /// specified scale factor.
    pub fn from_logical(logical_size: LogicalSize, scale_factor: f32) -> Self {
        Self::new(
            (logical_size.width * scale_factor) as u32,
            (logical_size.height * scale_factor) as u32,
        )
    }

    /// Convert this physical size to a logical size by dividing width and height by the
    /// specified scale factor.
    pub fn to_logical(&self, scale_factor: f32) -> LogicalSize {
        LogicalSize::from_physical(*self, scale_factor)
    }

    #[cfg(feature = "ffi")]
    pub(crate) fn to_euclid(&self) -> crate::graphics::euclid::default::Size2D<u32> {
        [self.width, self.height].into()
    }
}

/// The size of a window represented in either physical or logical pixels. This is used
/// with [`Window::set_size`].
#[derive(Clone, Debug, derive_more::From, PartialEq)]
pub enum WindowSize {
    /// The size in physical pixels.
    Physical(PhysicalSize),
    /// The size in logical screen pixels.
    Logical(LogicalSize),
}

impl WindowSize {
    /// Turn the `WindowSize` into a `PhysicalSize`.
    pub fn to_physical(&self, scale_factor: f32) -> PhysicalSize {
        match self {
            WindowSize::Physical(size) => *size,
            WindowSize::Logical(size) => size.to_physical(scale_factor),
        }
    }

    /// Turn the `WindowSize` into a `LogicalSize`.
    pub fn to_logical(&self, scale_factor: f32) -> LogicalSize {
        match self {
            WindowSize::Physical(size) => size.to_logical(scale_factor),
            WindowSize::Logical(size) => *size,
        }
    }
}

#[test]
fn logical_physical_pos() {
    use crate::graphics::euclid::approxeq::ApproxEq;

    let phys = PhysicalPosition::new(100, 50);
    let logical = phys.to_logical(2.);
    assert!(logical.x.approx_eq(&50.));
    assert!(logical.y.approx_eq(&25.));

    assert_eq!(logical.to_physical(2.), phys);
}

#[test]
fn logical_physical_size() {
    use crate::graphics::euclid::approxeq::ApproxEq;

    let phys = PhysicalSize::new(100, 50);
    let logical = phys.to_logical(2.);
    assert!(logical.width.approx_eq(&50.));
    assert!(logical.height.approx_eq(&25.));

    assert_eq!(logical.to_physical(2.), phys);
}

#[i_slint_core_macros::slint_doc]
/// This enum describes a low-level access to specific graphics APIs used
/// by the renderer.
#[derive(Clone)]
#[non_exhaustive]
pub enum GraphicsAPI<'a> {
    /// The rendering is done using OpenGL.
    NativeOpenGL {
        /// Use this function pointer to obtain access to the OpenGL implementation - similar to `eglGetProcAddress`.
        get_proc_address: &'a dyn Fn(&core::ffi::CStr) -> *const core::ffi::c_void,
    },
    /// The rendering is done on a HTML Canvas element using WebGL.
    WebGL {
        /// The DOM element id of the HTML Canvas element used for rendering.
        canvas_element_id: &'a str,
        /// The drawing context type used on the HTML Canvas element for rendering. This is the argument to the
        /// `getContext` function on the HTML Canvas element.
        context_type: &'a str,
    },
    /// The rendering is based on WGPU 25.x. Use the provided fields to submit commits to the provided
    /// WGPU command queue.
    ///
    /// *Note*: This function is behind the [`unstable-wgpu-25` feature flag](slint:rust:slint/docs/cargo_features/#backends)
    ///         and may be removed or changed in future minor releases, as new major WGPU releases become available.
    ///
    /// See also the [`slint::wgpu_25`](slint:rust:slint/wgpu_25) module.
    #[cfg(feature = "unstable-wgpu-25")]
    #[non_exhaustive]
    WGPU25 {
        /// The WGPU instance used for rendering.
        instance: wgpu_25::Instance,
        /// The WGPU device used for rendering.
        device: wgpu_25::Device,
        /// The WGPU queue for used for command submission.
        queue: wgpu_25::Queue,
    },
}

impl core::fmt::Debug for GraphicsAPI<'_> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            GraphicsAPI::NativeOpenGL { .. } => write!(f, "GraphicsAPI::NativeOpenGL"),
            GraphicsAPI::WebGL { context_type, .. } => {
                write!(f, "GraphicsAPI::WebGL(context_type = {context_type})")
            }
            #[cfg(feature = "unstable-wgpu-25")]
            GraphicsAPI::WGPU25 { .. } => write!(f, "GraphicsAPI::WGPU25"),
        }
    }
}

/// This enum describes the different rendering states, that will be provided
/// to the parameter of the callback for `set_rendering_notifier` on the `slint::Window`.
///
/// When OpenGL is used for rendering, the context will be current.
/// It's safe to call OpenGL functions, but it is crucial that the state of the context is
/// preserved. So make sure to save and restore state such as `TEXTURE_BINDING_2D` or
/// `ARRAY_BUFFER_BINDING` perfectly.
#[derive(Debug, Clone)]
#[repr(u8)]
#[non_exhaustive]
pub enum RenderingState {
    /// The window has been created and the graphics adapter/context initialized.
    RenderingSetup,
    /// The scene of items is about to be rendered.
    BeforeRendering,
    /// The scene of items was rendered, but the back buffer was not sent for display presentation
    /// yet (for example GL swap buffers).
    AfterRendering,
    /// The window will be destroyed and/or graphics resources need to be released due to other
    /// constraints.
    RenderingTeardown,
}

/// Internal trait that's used to map rendering state callbacks to either a Rust-API provided
/// impl FnMut or a struct that invokes a C callback and implements Drop to release the closure
/// on the C++ side.
#[doc(hidden)]
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
/// registers a rendering notifier on a `slint::Window`.
#[derive(Debug, Clone)]
#[repr(u8)]
#[non_exhaustive]
pub enum SetRenderingNotifierError {
    /// The rendering backend does not support rendering notifiers.
    Unsupported,
    /// There is already a rendering notifier set, multiple notifiers are not supported.
    AlreadySet,
}

impl core::fmt::Display for SetRenderingNotifierError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::Unsupported => {
                f.write_str("The rendering backend does not support rendering notifiers.")
            }
            Self::AlreadySet => f.write_str(
                "There is already a rendering notifier set, multiple notifiers are not supported.",
            ),
        }
    }
}

#[cfg(feature = "std")]
impl std::error::Error for SetRenderingNotifierError {}

#[cfg(feature = "raw-window-handle-06")]
#[derive(Clone)]
enum WindowHandleInner {
    HandleByAdapter(alloc::rc::Rc<dyn WindowAdapter>),
    #[cfg(feature = "std")]
    HandleByRcRWH {
        window_handle_provider: std::sync::Arc<dyn raw_window_handle_06::HasWindowHandle>,
        display_handle_provider: std::sync::Arc<dyn raw_window_handle_06::HasDisplayHandle>,
    },
}

/// This struct represents a persistent handle to a window and implements the
/// [`raw_window_handle_06::HasWindowHandle`] and [`raw_window_handle_06::HasDisplayHandle`]
/// traits for accessing exposing raw window and display handles.
/// Obtain an instance of this by calling [`Window::window_handle()`].
#[cfg(feature = "raw-window-handle-06")]
#[derive(Clone)]
pub struct WindowHandle {
    inner: WindowHandleInner,
}

#[cfg(feature = "raw-window-handle-06")]
impl raw_window_handle_06::HasWindowHandle for WindowHandle {
    fn window_handle(
        &self,
    ) -> Result<raw_window_handle_06::WindowHandle<'_>, raw_window_handle_06::HandleError> {
        match &self.inner {
            WindowHandleInner::HandleByAdapter(adapter) => adapter.window_handle_06(),
            #[cfg(feature = "std")]
            WindowHandleInner::HandleByRcRWH { window_handle_provider, .. } => {
                window_handle_provider.window_handle()
            }
        }
    }
}

#[cfg(feature = "raw-window-handle-06")]
impl raw_window_handle_06::HasDisplayHandle for WindowHandle {
    fn display_handle(
        &self,
    ) -> Result<raw_window_handle_06::DisplayHandle<'_>, raw_window_handle_06::HandleError> {
        match &self.inner {
            WindowHandleInner::HandleByAdapter(adapter) => adapter.display_handle_06(),
            #[cfg(feature = "std")]
            WindowHandleInner::HandleByRcRWH { display_handle_provider, .. } => {
                display_handle_provider.display_handle()
            }
        }
    }
}

/// This type represents a window towards the windowing system, that's used to render the
/// scene of a component. It provides API to control windowing system specific aspects such
/// as the position on the screen.
#[repr(transparent)]
pub struct Window(pub(crate) WindowInner);

/// This enum describes whether a Window is allowed to be hidden when the user tries to close the window.
/// It is the return type of the callback provided to [Window::on_close_requested].
#[derive(Copy, Clone, Debug, PartialEq, Default)]
#[repr(u8)]
pub enum CloseRequestResponse {
    /// The Window will be hidden (default action)
    #[default]
    HideWindow = 0,
    /// The close request is rejected and the window will be kept shown.
    KeepWindowShown = 1,
}

impl Window {
    /// Create a new window from a window adapter
    ///
    /// You only need to create the window yourself when you create a [`WindowAdapter`] from
    /// [`Platform::create_window_adapter`](crate::platform::Platform::create_window_adapter)
    ///
    /// Since the window adapter must own the Window, this function is meant to be used with
    /// [`Rc::new_cyclic`](alloc::rc::Rc::new_cyclic)
    ///
    /// # Example
    /// ```rust
    /// use std::rc::Rc;
    /// use slint::platform::{WindowAdapter, Renderer};
    /// use slint::{Window, PhysicalSize};
    /// struct MyWindowAdapter {
    ///     window: Window,
    ///     //...
    /// }
    /// impl WindowAdapter for MyWindowAdapter {
    ///    fn window(&self) -> &Window { &self.window }
    ///    fn size(&self) -> PhysicalSize { unimplemented!() }
    ///    fn renderer(&self) -> &dyn Renderer { unimplemented!() }
    /// }
    ///
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

    /// Shows the window on the screen. An additional strong reference on the
    /// associated component is maintained while the window is visible.
    ///
    /// Call [`Self::hide()`] to make the window invisible again, and drop the additional
    /// strong reference.
    pub fn show(&self) -> Result<(), PlatformError> {
        self.0.show()
    }

    /// Hides the window, so that it is not visible anymore. The additional strong
    /// reference on the associated component, that was created when [`Self::show()`] was called, is
    /// dropped.
    pub fn hide(&self) -> Result<(), PlatformError> {
        self.0.hide()
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
        self.0.window_adapter().request_redraw()
    }

    /// This function returns the scale factor that allows converting between logical and
    /// physical pixels.
    pub fn scale_factor(&self) -> f32 {
        self.0.scale_factor()
    }

    /// Returns the position of the window on the screen, in physical screen coordinates and including
    /// a window frame (if present).
    pub fn position(&self) -> PhysicalPosition {
        self.0.window_adapter().position().unwrap_or_default()
    }

    /// Sets the position of the window on the screen, in physical screen coordinates and including
    /// a window frame (if present).
    /// Note that on some windowing systems, such as Wayland, this functionality is not available.
    pub fn set_position(&self, position: impl Into<WindowPosition>) {
        let position = position.into();
        self.0.window_adapter().set_position(position)
    }

    /// Returns the size of the window on the screen, in physical screen coordinates and excluding
    /// a window frame (if present).
    pub fn size(&self) -> PhysicalSize {
        self.0.window_adapter().size()
    }

    /// Resizes the window to the specified size on the screen, in physical pixels and excluding
    /// a window frame (if present).
    pub fn set_size(&self, size: impl Into<WindowSize>) {
        let size = size.into();
        crate::window::WindowAdapter::set_size(&*self.0.window_adapter(), size);
    }

    /// Returns if the window is currently fullscreen
    pub fn is_fullscreen(&self) -> bool {
        self.0.is_fullscreen()
    }

    /// Set or unset the window to display fullscreen.
    pub fn set_fullscreen(&self, fullscreen: bool) {
        self.0.set_fullscreen(fullscreen);
    }

    /// Returns if the window is currently maximized
    pub fn is_maximized(&self) -> bool {
        self.0.is_maximized()
    }

    /// Maximize or unmaximize the window.
    pub fn set_maximized(&self, maximized: bool) {
        self.0.set_maximized(maximized);
    }

    /// Returns if the window is currently minimized
    pub fn is_minimized(&self) -> bool {
        self.0.is_minimized()
    }

    /// Minimize or unminimze the window.
    pub fn set_minimized(&self, minimized: bool) {
        self.0.set_minimized(minimized);
    }

    /// Dispatch a window event to the scene.
    ///
    /// Use this when you're implementing your own backend and want to forward user input events.
    ///
    /// Any position fields in the event must be in the logical pixel coordinate system relative to
    /// the top left corner of the window.
    ///
    /// This function panics if there is an error processing the event.
    /// Use [`Self::try_dispatch_event()`] to handle the error.
    #[track_caller]
    pub fn dispatch_event(&self, event: crate::platform::WindowEvent) {
        self.try_dispatch_event(event).unwrap()
    }

    /// Dispatch a window event to the scene.
    ///
    /// Use this when you're implementing your own backend and want to forward user input events.
    ///
    /// Any position fields in the event must be in the logical pixel coordinate system relative to
    /// the top left corner of the window.
    pub fn try_dispatch_event(
        &self,
        event: crate::platform::WindowEvent,
    ) -> Result<(), PlatformError> {
        match event {
            crate::platform::WindowEvent::PointerPressed { position, button } => {
                self.0.process_mouse_input(MouseEvent::Pressed {
                    position: position.to_euclid().cast(),
                    button,
                    click_count: 0,
                });
            }
            crate::platform::WindowEvent::PointerReleased { position, button } => {
                self.0.process_mouse_input(MouseEvent::Released {
                    position: position.to_euclid().cast(),
                    button,
                    click_count: 0,
                });
            }
            crate::platform::WindowEvent::PointerMoved { position } => {
                self.0.process_mouse_input(MouseEvent::Moved {
                    position: position.to_euclid().cast(),
                });
            }
            crate::platform::WindowEvent::PointerScrolled { position, delta_x, delta_y } => {
                self.0.process_mouse_input(MouseEvent::Wheel {
                    position: position.to_euclid().cast(),
                    delta_x: delta_x as _,
                    delta_y: delta_y as _,
                });
            }
            crate::platform::WindowEvent::PointerExited => {
                self.0.process_mouse_input(MouseEvent::Exit)
            }

            crate::platform::WindowEvent::KeyPressed { text } => {
                self.0.process_key_input(crate::input::KeyEvent {
                    text,
                    repeat: false,
                    event_type: KeyEventType::KeyPressed,
                    ..Default::default()
                })
            }
            crate::platform::WindowEvent::KeyPressRepeated { text } => {
                self.0.process_key_input(crate::input::KeyEvent {
                    text,
                    repeat: true,
                    event_type: KeyEventType::KeyPressed,
                    ..Default::default()
                })
            }
            crate::platform::WindowEvent::KeyReleased { text } => {
                self.0.process_key_input(crate::input::KeyEvent {
                    text,
                    event_type: KeyEventType::KeyReleased,
                    ..Default::default()
                })
            }
            crate::platform::WindowEvent::ScaleFactorChanged { scale_factor } => {
                self.0.set_scale_factor(scale_factor);
            }
            crate::platform::WindowEvent::Resized { size } => {
                self.0.set_window_item_geometry(size.to_euclid());
                self.0.window_adapter().renderer().resize(size.to_physical(self.scale_factor()))?;
            }
            crate::platform::WindowEvent::CloseRequested => {
                if self.0.request_close() {
                    self.hide()?;
                }
            }
            crate::platform::WindowEvent::WindowActiveChanged(bool) => self.0.set_active(bool),
        };
        Ok(())
    }

    /// Returns true if there is an animation currently active on any property in the Window; false otherwise.
    pub fn has_active_animations(&self) -> bool {
        // TODO make it really per window.
        crate::animations::CURRENT_ANIMATION_DRIVER.with(|driver| driver.has_active_animations())
    }

    /// Returns the visibility state of the window. This function can return false even if you previously called show()
    /// on it, for example if the user minimized the window.
    pub fn is_visible(&self) -> bool {
        self.0.is_visible()
    }

    /// Returns a struct that implements the raw window handle traits to access the windowing system specific window
    /// and display handles. This function is only accessible if you enable the `raw-window-handle-06` crate feature.
    #[cfg(feature = "raw-window-handle-06")]
    pub fn window_handle(&self) -> WindowHandle {
        let adapter = self.0.window_adapter();
        #[cfg(feature = "std")]
        if let Some((window_handle_provider, display_handle_provider)) =
            adapter.internal(crate::InternalToken).and_then(|internal| {
                internal.window_handle_06_rc().ok().zip(internal.display_handle_06_rc().ok())
            })
        {
            return WindowHandle {
                inner: WindowHandleInner::HandleByRcRWH {
                    window_handle_provider,
                    display_handle_provider,
                },
            };
        }

        WindowHandle { inner: WindowHandleInner::HandleByAdapter(adapter) }
    }

    /// Takes a snapshot of the window contents and returns it as RGBA8 encoded pixel buffer.
    ///
    /// Note that this function may be slow to call as it may need to re-render the scene.
    pub fn take_snapshot(&self) -> Result<SharedPixelBuffer<Rgba8Pixel>, PlatformError> {
        self.0.window_adapter().renderer().take_snapshot()
    }
}

pub use crate::SharedString;

#[i_slint_core_macros::slint_doc]
/// This trait is used to obtain references to global singletons exported in `.slint`
/// markup. Alternatively, you can use [`ComponentHandle::global`] to obtain access.
///
/// This trait is implemented by the compiler for each global singleton that's exported.
///
/// # Example
/// The following example of `.slint` markup defines a global singleton called `Palette`, exports
/// it and modifies it from Rust code:
/// ```rust
/// # i_slint_backend_testing::init_no_event_loop();
/// slint::slint!{
/// export global Palette {
///     in property<color> foreground-color;
///     in property<color> background-color;
/// }
///
/// export component App inherits Window {
///    background: Palette.background-color;
///    Text {
///       text: "Hello";
///       color: Palette.foreground-color;
///    }
///    // ...
/// }
/// }
/// let app = App::new().unwrap();
/// app.global::<Palette>().set_background_color(slint::Color::from_rgb_u8(0, 0, 0));
///
/// // alternate way to access the global singleton:
/// Palette::get(&app).set_foreground_color(slint::Color::from_rgb_u8(255, 255, 255));
/// ```
///
/// See also the [language documentation for global singletons](slint:globals) for more information.
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
/// This trait is implemented by the [generated component](index.html#generated-components)
pub trait ComponentHandle {
    /// The internal Inner type for `Weak<Self>::inner`.
    #[doc(hidden)]
    type WeakInner: Clone + Default;
    /// Returns a new weak pointer.
    fn as_weak(&self) -> Weak<Self>
    where
        Self: Sized;

    /// Returns a clone of this handle that's a strong reference.
    #[must_use]
    fn clone_strong(&self) -> Self;

    /// Internal function used when upgrading a weak reference to a strong one.
    #[doc(hidden)]
    fn upgrade_from_weak_inner(_: &Self::WeakInner) -> Option<Self>
    where
        Self: Sized;

    /// Convenience function for [`crate::Window::show()`](struct.Window.html#method.show).
    /// This shows the window on the screen and maintains an extra strong reference while
    /// the window is visible. To react to events from the windowing system, such as draw
    /// requests or mouse/touch input, it is still necessary to spin the event loop,
    /// using [`crate::run_event_loop`](fn.run_event_loop.html).
    fn show(&self) -> Result<(), PlatformError>;

    /// Convenience function for [`crate::Window::hide()`](struct.Window.html#method.hide).
    /// Hides the window, so that it is not visible anymore. The additional strong reference
    /// on the associated component, that was created when show() was called, is dropped.
    fn hide(&self) -> Result<(), PlatformError>;

    /// Returns the Window associated with this component. The window API can be used
    /// to control different aspects of the integration into the windowing system,
    /// such as the position on the screen.
    fn window(&self) -> &Window;

    /// This is a convenience function that first calls [`Self::show`], followed by [`crate::run_event_loop()`](fn.run_event_loop.html)
    /// and [`Self::hide`].
    fn run(&self) -> Result<(), PlatformError>;

    /// This function provides access to instances of global singletons exported in `.slint`.
    /// See [`Global`] for an example how to export and access globals from `.slint` markup.
    fn global<'a, T: Global<'a, Self>>(&'a self) -> T
    where
        Self: Sized;
}

mod weak_handle {

    use super::*;

    /// Struct that's used to hold weak references of a [Slint component](index.html#generated-components)
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
        inner: T::WeakInner,
        #[cfg(feature = "std")]
        thread: std::thread::ThreadId,
    }

    impl<T: ComponentHandle> Default for Weak<T> {
        fn default() -> Self {
            Self {
                inner: T::WeakInner::default(),
                #[cfg(feature = "std")]
                thread: std::thread::current().id(),
            }
        }
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
        pub fn new(inner: T::WeakInner) -> Self {
            Self {
                inner,
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
            T::upgrade_from_weak_inner(&self.inner)
        }

        /// Convenience function that returns a new strongly referenced component if
        /// some other instance still holds a strong reference and the current thread
        /// is the thread that created this component.
        /// Otherwise, this function panics.
        #[track_caller]
        pub fn unwrap(&self) -> T {
            #[cfg(feature = "std")]
            if std::thread::current().id() != self.thread {
                panic!(
                    "Trying to upgrade a Weak from a different thread than the one it belongs to"
                );
            }
            T::upgrade_from_weak_inner(&self.inner)
                .expect("The Weak doesn't hold a valid component")
        }

        /// A helper function to allow creation on `component_factory::Component` from
        /// a `ComponentHandle`
        pub(crate) fn inner(&self) -> T::WeakInner {
            self.inner.clone()
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
        /// # i_slint_backend_testing::init_no_event_loop();
        /// slint::slint! { export component MyApp inherits Window { in property <int> foo; /* ... */ } }
        /// let handle = MyApp::new().unwrap();
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
        /// handle.run().unwrap();
        /// ```
        #[cfg(any(feature = "std", feature = "unsafe-single-threaded"))]
        pub fn upgrade_in_event_loop(
            &self,
            func: impl FnOnce(T) + Send + 'static,
        ) -> Result<(), EventLoopError>
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
    #[cfg(any(feature = "std", feature = "unsafe-single-threaded"))]
    unsafe impl<T: ComponentHandle> Send for Weak<T> {}
    #[allow(unsafe_code)]
    #[cfg(any(feature = "std", feature = "unsafe-single-threaded"))]
    unsafe impl<T: ComponentHandle> Sync for Weak<T> {}
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
/// If you want to capture non-Send types to run in the next event loop iteration,
/// you can use the `slint::spawn_local` function instead.
///
/// See also [`Weak::upgrade_in_event_loop`].
///
/// # Example
/// ```rust
/// slint::slint! { export component MyApp inherits Window { in property <int> foo; /* ... */ } }
/// # i_slint_backend_testing::init_no_event_loop();
/// let handle = MyApp::new().unwrap();
/// let handle_weak = handle.as_weak();
/// # return; // don't run the event loop in examples
/// let thread = std::thread::spawn(move || {
///     // ... Do some computation in the thread
///     let foo = 42;
///      // now forward the data to the main thread using invoke_from_event_loop
///     let handle_copy = handle_weak.clone();
///     slint::invoke_from_event_loop(move || handle_copy.unwrap().set_foo(foo));
/// });
/// handle.run().unwrap();
/// ```
pub fn invoke_from_event_loop(func: impl FnOnce() + Send + 'static) -> Result<(), EventLoopError> {
    crate::platform::with_event_loop_proxy(|proxy| {
        proxy
            .ok_or(EventLoopError::NoEventLoopProvider)?
            .invoke_from_event_loop(alloc::boxed::Box::new(func))
    })
}

/// Schedules the main event loop for termination. This function is meant
/// to be called from callbacks triggered by the UI. After calling the function,
/// it will return immediately and once control is passed back to the event loop,
/// the initial call to `slint::run_event_loop()` will return.
///
/// This function can be called from any thread
///
/// Any previously queued events may or may not be processed before the loop terminates.
/// This is platform dependent behaviour.
pub fn quit_event_loop() -> Result<(), EventLoopError> {
    crate::platform::with_event_loop_proxy(|proxy| {
        proxy.ok_or(EventLoopError::NoEventLoopProvider)?.quit_event_loop()
    })
}

#[derive(Debug, Clone, Eq, PartialEq)]
#[non_exhaustive]
/// Error returned from the [`invoke_from_event_loop()`] and [`quit_event_loop()`] function
pub enum EventLoopError {
    /// The event could not be sent because the event loop was terminated already
    EventLoopTerminated,
    /// The event could not be sent because the Slint platform abstraction was not yet initialized,
    /// or the platform does not support event loop.
    NoEventLoopProvider,
}

impl core::fmt::Display for EventLoopError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            EventLoopError::EventLoopTerminated => {
                f.write_str("The event loop was already terminated")
            }
            EventLoopError::NoEventLoopProvider => {
                f.write_str("The Slint platform does not provide an event loop")
            }
        }
    }
}

#[cfg(feature = "std")]
impl std::error::Error for EventLoopError {}

/// The platform encountered a fatal error.
///
/// This error typically indicates an issue with initialization or connecting to the windowing system.
///
/// This can be constructed from a `String`:
/// ```rust
/// use slint::platform::PlatformError;
/// PlatformError::from(format!("Could not load resource {}", 1234));
/// ```
#[non_exhaustive]
pub enum PlatformError {
    /// No default platform was selected, or no platform could be initialized.
    ///
    /// If you encounter this error, make sure to either selected trough the `backend-*` cargo features flags,
    /// or call [`platform::set_platform()`](crate::platform::set_platform)
    /// before running the event loop
    NoPlatform,
    /// The Slint Platform does not provide an event loop.
    ///
    /// The [`Platform::run_event_loop`](crate::platform::Platform::run_event_loop)
    /// is not implemented for the current platform.
    NoEventLoopProvider,

    /// There is already a platform set from another thread.
    SetPlatformError(crate::platform::SetPlatformError),

    /// Another platform-specific error occurred
    Other(String),
    /// Another platform-specific error occurred.
    #[cfg(feature = "std")]
    OtherError(Box<dyn std::error::Error + Send + Sync>),
}

#[cfg(target_arch = "wasm32")]
impl From<PlatformError> for wasm_bindgen::JsValue {
    fn from(err: PlatformError) -> wasm_bindgen::JsValue {
        wasm_bindgen::JsError::from(err).into()
    }
}

impl core::fmt::Debug for PlatformError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        core::fmt::Display::fmt(self, f)
    }
}

impl core::fmt::Display for PlatformError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            PlatformError::NoPlatform => f.write_str(
                "No default Slint platform was selected, and no Slint platform was initialized",
            ),
            PlatformError::NoEventLoopProvider => {
                f.write_str("The Slint platform does not provide an event loop")
            }
            PlatformError::SetPlatformError(_) => {
                f.write_str("The Slint platform was initialized in another thread")
            }
            PlatformError::Other(str) => f.write_str(str),
            #[cfg(feature = "std")]
            PlatformError::OtherError(error) => error.fmt(f),
        }
    }
}

impl From<String> for PlatformError {
    fn from(value: String) -> Self {
        Self::Other(value)
    }
}
impl From<&str> for PlatformError {
    fn from(value: &str) -> Self {
        Self::Other(value.into())
    }
}

#[cfg(feature = "std")]
impl From<Box<dyn std::error::Error + Send + Sync>> for PlatformError {
    fn from(error: Box<dyn std::error::Error + Send + Sync>) -> Self {
        Self::OtherError(error)
    }
}

#[cfg(feature = "std")]
impl std::error::Error for PlatformError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            PlatformError::OtherError(err) => Some(err.as_ref()),
            _ => None,
        }
    }
}

#[test]
#[cfg(feature = "std")]
fn error_is_send() {
    let _: Box<dyn std::error::Error + Send + Sync + 'static> = PlatformError::NoPlatform.into();
}

/// Sets the application id for use on Wayland or X11 with [xdg](https://specifications.freedesktop.org/desktop-entry-spec/latest/)
/// compliant window managers. This must be set before the window is shown, and has only an effect on Wayland or X11.
pub fn set_xdg_app_id(app_id: impl Into<SharedString>) -> Result<(), PlatformError> {
    crate::context::with_global_context(
        || Err(crate::platform::PlatformError::NoPlatform),
        |ctx| ctx.set_xdg_app_id(app_id.into()),
    )
}
