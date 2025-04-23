// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

#![doc = include_str!("README.md")]
#![doc(html_logo_url = "https://slint.dev/logo/slint-logo-square-light.svg")]
#![warn(missing_docs)]

extern crate alloc;

use event_loop::{CustomEvent, EventLoopState, NotRunningEventLoop};
use i_slint_core::api::EventLoopError;
use i_slint_core::graphics::RequestedGraphicsAPI;
use i_slint_core::platform::{EventLoopProxy, PlatformError};
use i_slint_core::window::WindowAdapter;
use renderer::WinitCompatibleRenderer;
use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;
use std::rc::Weak;

#[cfg(not(target_arch = "wasm32"))]
mod clipboard;
mod drag_resize_window;
mod winitwindowadapter;
use winitwindowadapter::*;
pub(crate) mod event_loop;

/// Re-export of the winit crate.
pub use winit;

/// Internal type used by the winit backend for thread communication and window system updates.
#[non_exhaustive]
#[derive(Debug)]
pub struct SlintUserEvent(CustomEvent);

/// Returned by callbacks passed to [`Window::on_winit_window_event`](WinitWindowAccessor::on_winit_window_event)
/// to determine if winit events should propagate to the Slint event loop.
pub enum WinitWindowEventResult {
    /// The winit event should propagate normally.
    Propagate,
    /// The winit event shouldn't be processed further.
    PreventDefault,
}

mod renderer {
    use std::rc::Rc;

    use i_slint_core::{graphics::RequestedGraphicsAPI, platform::PlatformError};

    pub trait WinitCompatibleRenderer {
        fn render(&self, window: &i_slint_core::api::Window) -> Result<(), PlatformError>;

        fn as_core_renderer(&self) -> &dyn i_slint_core::renderer::Renderer;
        // Got WindowEvent::Occluded
        fn occluded(&self, _: bool) {}

        fn suspend(&self) -> Result<(), PlatformError>;

        // Got winit::Event::Resumed
        fn resume(
            &self,
            event_loop: &dyn crate::event_loop::EventLoopInterface,
            window_attributes: winit::window::WindowAttributes,
            requested_graphics_api: Option<RequestedGraphicsAPI>,
        ) -> Result<Rc<winit::window::Window>, PlatformError>;

        fn is_suspended(&self) -> bool;
    }

    #[cfg(feature = "renderer-femtovg")]
    pub(crate) mod femtovg;
    #[cfg(enable_skia_renderer)]
    pub(crate) mod skia;

    #[cfg(feature = "renderer-software")]
    pub(crate) mod sw;
}

#[cfg(enable_accesskit)]
mod accesskit;
#[cfg(muda)]
mod muda;
#[cfg(not(use_winit_theme))]
mod xdg_color_scheme;

#[cfg(target_arch = "wasm32")]
pub(crate) mod wasm_input_helper;

cfg_if::cfg_if! {
    if #[cfg(feature = "renderer-femtovg")] {
        const DEFAULT_RENDERER_NAME: &str = "FemtoVG";
    } else if #[cfg(enable_skia_renderer)] {
        const DEFAULT_RENDERER_NAME: &'static str = "Skia";
    } else if #[cfg(feature = "renderer-software")] {
        const DEFAULT_RENDERER_NAME: &'static str = "Software";
    } else {
        compile_error!("Please select a feature to build with the winit backend: `renderer-femtovg`, `renderer-skia`, `renderer-skia-opengl`, `renderer-skia-vulkan` or `renderer-software`");
    }
}

fn default_renderer_factory() -> Box<dyn WinitCompatibleRenderer> {
    cfg_if::cfg_if! {
        if #[cfg(enable_skia_renderer)] {
            renderer::skia::WinitSkiaRenderer::new_suspended()
        } else if #[cfg(feature = "renderer-femtovg")] {
            renderer::femtovg::GlutinFemtoVGRenderer::new_suspended()
        } else if #[cfg(feature = "renderer-software")] {
            renderer::sw::WinitSoftwareRenderer::new_suspended()
        } else {
            compile_error!("Please select a feature to build with the winit backend: `renderer-femtovg`, `renderer-skia`, `renderer-skia-opengl`, `renderer-skia-vulkan` or `renderer-software`");
        }
    }
}

fn try_create_window_with_fallback_renderer(
    shared_backend_data: &Rc<SharedBackendData>,
    attrs: winit::window::WindowAttributes,
    _proxy: &winit::event_loop::EventLoopProxy<SlintUserEvent>,
    #[cfg(all(muda, target_os = "macos"))] muda_enable_default_menu_bar: bool,
) -> Option<Rc<WinitWindowAdapter>> {
    [
        #[cfg(any(
            feature = "renderer-skia",
            feature = "renderer-skia-opengl",
            feature = "renderer-skia-vulkan"
        ))]
        renderer::skia::WinitSkiaRenderer::new_suspended,
        #[cfg(feature = "renderer-femtovg")]
        renderer::femtovg::GlutinFemtoVGRenderer::new_suspended,
        #[cfg(feature = "renderer-software")]
        renderer::sw::WinitSoftwareRenderer::new_suspended,
    ]
    .into_iter()
    .find_map(|renderer_factory| {
        WinitWindowAdapter::new(
            shared_backend_data.clone(),
            renderer_factory(),
            attrs.clone(),
            None,
            #[cfg(any(enable_accesskit, muda))]
            _proxy.clone(),
            #[cfg(all(muda, target_os = "macos"))]
            muda_enable_default_menu_bar,
        )
        .ok()
    })
}

#[doc(hidden)]
pub type NativeWidgets = ();
#[doc(hidden)]
pub type NativeGlobals = ();
#[doc(hidden)]
pub const HAS_NATIVE_STYLE: bool = false;
#[doc(hidden)]
pub mod native_widgets {}

/// Use the BackendBuilder to configure the properties of the Winit Backend before creating it.
/// Create the builder using [`Backend::builder()`], then configure it for example with [`Self::with_renderer_name`],
/// and build the backend using [`Self::build`].
pub struct BackendBuilder {
    allow_fallback: bool,
    requested_graphics_api: Option<RequestedGraphicsAPI>,
    window_attributes_hook:
        Option<Box<dyn Fn(winit::window::WindowAttributes) -> winit::window::WindowAttributes>>,
    renderer_name: Option<String>,
    event_loop_builder: Option<winit::event_loop::EventLoopBuilder<SlintUserEvent>>,
    #[cfg(all(muda, target_os = "macos"))]
    muda_enable_default_menu_bar_bar: bool,
    #[cfg(target_family = "wasm")]
    spawn_event_loop: bool,
}

impl BackendBuilder {
    /// Configures this builder to require a renderer that supports the specified graphics API.
    #[must_use]
    pub fn request_graphics_api(mut self, graphics_api: RequestedGraphicsAPI) -> Self {
        self.requested_graphics_api = Some(graphics_api);
        self
    }

    /// Configures this builder to use the specified renderer name when building the backend later.
    /// Pass `renderer-software` for example to configure the backend to use the Slint software renderer.
    #[must_use]
    pub fn with_renderer_name(mut self, name: impl Into<String>) -> Self {
        self.renderer_name = Some(name.into());
        self
    }

    /// Configures this builder to use the specified hook that will be called before a Window is created.
    ///
    /// It can be used to adjust settings of window that will be created.
    ///
    /// # Example
    ///
    /// ```rust,no_run
    /// let mut backend = i_slint_backend_winit::Backend::builder()
    ///     .with_window_attributes_hook(|attributes| attributes.with_content_protected(true))
    ///     .build()
    ///     .unwrap();
    /// slint::platform::set_platform(Box::new(backend));
    /// ```
    #[must_use]
    pub fn with_window_attributes_hook(
        mut self,
        hook: impl Fn(winit::window::WindowAttributes) -> winit::window::WindowAttributes + 'static,
    ) -> Self {
        self.window_attributes_hook = Some(Box::new(hook));
        self
    }

    /// Configures this builder to use the specified event loop builder when creating the event
    /// loop during a subsequent call to [`Self::build`].
    #[must_use]
    pub fn with_event_loop_builder(
        mut self,
        event_loop_builder: winit::event_loop::EventLoopBuilder<SlintUserEvent>,
    ) -> Self {
        self.event_loop_builder = Some(event_loop_builder);
        self
    }

    /// Configures this builder to enable or disable the default menu bar.
    /// By default, the menu bar is provided by Slint. Set this to false
    /// if you're providing your own menu bar.
    /// Note that an application provided menu bar will be overriden by a `MenuBar`
    /// declared in Slint code.
    #[must_use]
    #[cfg(all(muda, target_os = "macos"))]
    pub fn with_default_menu_bar(mut self, enable: bool) -> Self {
        self.muda_enable_default_menu_bar_bar = enable;
        self
    }

    #[cfg(target_family = "wasm")]
    /// Configures this builder to spawn the event loop using [`winit::platform::web::EventLoopExtWebSys::spawn()`]
    /// run `run_event_loop()` is called.
    pub fn with_spawn_event_loop(mut self, enable: bool) -> Self {
        self.spawn_event_loop = enable;
        self
    }

    /// Builds the backend with the parameters configured previously. Set the resulting backend
    /// with `slint::platform::set_platform()`:
    ///
    /// # Example
    ///
    /// ```rust,no_run
    /// let mut backend = i_slint_backend_winit::Backend::builder()
    ///     .with_renderer_name("renderer-software")
    ///     .build()
    ///     .unwrap();
    /// slint::platform::set_platform(Box::new(backend));
    /// ```
    pub fn build(self) -> Result<Backend, PlatformError> {
        #[allow(unused_mut)]
        let mut event_loop_builder =
            self.event_loop_builder.unwrap_or_else(winit::event_loop::EventLoop::with_user_event);

        // Never use winit's menu bar. Either we provide one ourselves with muda, or
        // the user provides one.
        #[cfg(all(feature = "muda", target_os = "macos"))]
        winit::platform::macos::EventLoopBuilderExtMacOS::with_default_menu(
            &mut event_loop_builder,
            false,
        );

        // Initialize the winit event loop and propagate errors if for example `DISPLAY` or `WAYLAND_DISPLAY` isn't set.

        let shared_data = Rc::new(SharedBackendData::new(event_loop_builder)?);

        let renderer_factory_fn = match (
            self.renderer_name.as_deref(),
            self.requested_graphics_api.as_ref(),
        ) {
            #[cfg(feature = "renderer-femtovg")]
            (Some("gl"), maybe_graphics_api) | (Some("femtovg"), maybe_graphics_api) => {
                // If a graphics API was requested, double check that it's GL. FemtoVG doesn't support Metal, etc.
                if let Some(api) = maybe_graphics_api {
                    i_slint_core::graphics::RequestedOpenGLVersion::try_from(api.clone())?;
                }
                renderer::femtovg::GlutinFemtoVGRenderer::new_suspended
            }
            #[cfg(enable_skia_renderer)]
            (Some("skia"), maybe_graphics_api) => {
                renderer::skia::WinitSkiaRenderer::factory_for_graphics_api(maybe_graphics_api)?
            }
            #[cfg(all(enable_skia_renderer, supports_opengl))]
            (Some("skia-opengl"), maybe_graphics_api @ _) => {
                // If a graphics API was requested, double check that it's GL. FemtoVG doesn't support Metal, etc.
                if let Some(api) = maybe_graphics_api {
                    i_slint_core::graphics::RequestedOpenGLVersion::try_from(api.clone())?;
                }
                renderer::skia::WinitSkiaRenderer::new_opengl_suspended
            }
            #[cfg(all(enable_skia_renderer, not(target_os = "android")))]
            (Some("skia-software"), None) => {
                renderer::skia::WinitSkiaRenderer::new_software_suspended
            }
            #[cfg(feature = "renderer-software")]
            (Some("sw"), None) | (Some("software"), None) => {
                renderer::sw::WinitSoftwareRenderer::new_suspended
            }
            (None, None) => default_renderer_factory,
            (Some(renderer_name), _) => {
                if self.allow_fallback {
                    eprintln!(
                        "slint winit: unrecognized renderer {renderer_name}, falling back to {DEFAULT_RENDERER_NAME}"
                    );
                    default_renderer_factory
                } else {
                    return Err(PlatformError::NoPlatform);
                }
            }
            (None, Some(_requested_graphics_api)) => {
                cfg_if::cfg_if! {
                    if #[cfg(enable_skia_renderer)] {
                        renderer::skia::WinitSkiaRenderer::factory_for_graphics_api(Some(_requested_graphics_api))?
                    } else if #[cfg(feature = "renderer-femtovg")] {
                        // If a graphics API was requested, double check that it's GL. FemtoVG doesn't support Metal, etc.
                        i_slint_core::graphics::RequestedOpenGLVersion::try_from(_requested_graphics_api.clone())?;
                        renderer::femtovg::GlutinFemtoVGRenderer::new_suspended
                    } else {
                        return Err(format!("Graphics API use requested by the compile-time enabled renderers don't support that").into())
                    }
                }
            }
        };

        Ok(Backend {
            requested_graphics_api: self.requested_graphics_api,
            renderer_factory_fn,
            event_loop_state: Default::default(),
            window_attributes_hook: self.window_attributes_hook,
            shared_data,
            #[cfg(all(muda, target_os = "macos"))]
            muda_enable_default_menu_bar_bar: self.muda_enable_default_menu_bar_bar,
            #[cfg(target_family = "wasm")]
            spawn_event_loop: self.spawn_event_loop,
        })
    }
}

pub(crate) struct SharedBackendData {
    active_windows: RefCell<HashMap<winit::window::WindowId, Weak<WinitWindowAdapter>>>,
    #[cfg(not(target_arch = "wasm32"))]
    clipboard: std::cell::RefCell<clipboard::ClipboardPair>,
    not_running_event_loop: RefCell<Option<crate::event_loop::NotRunningEventLoop>>,
    event_loop_proxy: winit::event_loop::EventLoopProxy<SlintUserEvent>,
}

impl SharedBackendData {
    fn new(
        builder: winit::event_loop::EventLoopBuilder<SlintUserEvent>,
    ) -> Result<Self, PlatformError> {
        #[cfg(not(target_arch = "wasm32"))]
        use raw_window_handle::HasDisplayHandle;

        let nre = NotRunningEventLoop::new(builder)?;
        let event_loop_proxy = nre.instance.create_proxy();
        #[cfg(not(target_arch = "wasm32"))]
        let clipboard = crate::clipboard::create_clipboard(
            &nre.instance
                .display_handle()
                .map_err(|display_err| PlatformError::OtherError(display_err.into()))?,
        );
        Ok(Self {
            active_windows: Default::default(),
            #[cfg(not(target_arch = "wasm32"))]
            clipboard: RefCell::new(clipboard),
            not_running_event_loop: RefCell::new(Some(nre)),
            event_loop_proxy,
        })
    }

    pub(crate) fn with_event_loop<T>(
        &self,
        callback: impl FnOnce(
            &dyn crate::event_loop::EventLoopInterface,
        ) -> Result<T, Box<dyn std::error::Error + Send + Sync>>,
    ) -> Result<T, Box<dyn std::error::Error + Send + Sync>> {
        if crate::event_loop::CURRENT_WINDOW_TARGET.is_set() {
            crate::event_loop::CURRENT_WINDOW_TARGET.with(|current_target| callback(current_target))
        } else {
            match self.not_running_event_loop.borrow().as_ref() {
                Some(event_loop) => callback(event_loop),
                None => {
                    Err(PlatformError::from("Event loop functions called without event loop")
                        .into())
                }
            }
        }
    }

    pub fn register_window(&self, id: winit::window::WindowId, window: Rc<WinitWindowAdapter>) {
        self.active_windows.borrow_mut().insert(id, Rc::downgrade(&window));
    }

    pub fn unregister_window(&self, id: winit::window::WindowId) {
        self.active_windows.borrow_mut().remove(&id);
    }

    pub fn window_by_id(&self, id: winit::window::WindowId) -> Option<Rc<WinitWindowAdapter>> {
        self.active_windows.borrow().get(&id).and_then(|weakref| weakref.upgrade())
    }
}

#[i_slint_core_macros::slint_doc]
/// This struct implements the Slint Platform trait.
/// Use this in conjunction with [`slint::platform::set_platform`](slint:rust:slint/platform/fn.set_platform.html) to initialize.
/// Slint to use winit for all windowing system interaction.
///
/// ```rust,no_run
/// use i_slint_backend_winit::Backend;
/// slint::platform::set_platform(Box::new(Backend::new().unwrap()));
/// ```
pub struct Backend {
    requested_graphics_api: Option<RequestedGraphicsAPI>,
    renderer_factory_fn: fn() -> Box<dyn WinitCompatibleRenderer>,
    event_loop_state: std::cell::RefCell<Option<crate::event_loop::EventLoopState>>,
    shared_data: Rc<SharedBackendData>,

    /// This hook is called before a Window is created.
    ///
    /// It can be used to adjust settings of window that will be created
    ///
    /// See also [`BackendBuilder::with_window_attributes_hook`].
    ///
    /// # Example
    ///
    /// ```rust,no_run
    /// let mut backend = i_slint_backend_winit::Backend::new().unwrap();
    /// backend.window_attributes_hook = Some(Box::new(|attributes| attributes.with_content_protected(true)));
    /// slint::platform::set_platform(Box::new(backend));
    /// ```
    pub window_attributes_hook:
        Option<Box<dyn Fn(winit::window::WindowAttributes) -> winit::window::WindowAttributes>>,

    #[cfg(all(muda, target_os = "macos"))]
    muda_enable_default_menu_bar_bar: bool,

    #[cfg(target_family = "wasm")]
    spawn_event_loop: bool,
}

impl Backend {
    #[i_slint_core_macros::slint_doc]
    /// Creates a new winit backend with the default renderer that's compiled in.
    ///
    /// See the [backend documentation](slint:backends_and_renderers) for details on how to select the default renderer.
    pub fn new() -> Result<Self, PlatformError> {
        Self::builder().build()
    }

    #[i_slint_core_macros::slint_doc]
    /// Creates a new winit backend with the renderer specified by name.
    ///
    /// See the [backend documentation](slint:backends_and_renderers) for details on how to select the default renderer.
    ///
    /// If the renderer name is `None` or the name is not recognized, the default renderer is selected.
    pub fn new_with_renderer_by_name(renderer_name: Option<&str>) -> Result<Self, PlatformError> {
        let mut builder = Self::builder();
        if let Some(name) = renderer_name {
            builder = builder.with_renderer_name(name.to_string());
        }
        builder.build()
    }

    /// Creates a new BackendBuilder for configuring aspects of the Winit backend before
    /// setting it as the platform backend.
    pub fn builder() -> BackendBuilder {
        BackendBuilder {
            allow_fallback: true,
            requested_graphics_api: None,
            window_attributes_hook: None,
            renderer_name: None,
            event_loop_builder: None,
            #[cfg(all(muda, target_os = "macos"))]
            muda_enable_default_menu_bar_bar: true,
            #[cfg(target_family = "wasm")]
            spawn_event_loop: false,
        }
    }
}

impl i_slint_core::platform::Platform for Backend {
    fn create_window_adapter(&self) -> Result<Rc<dyn WindowAdapter>, PlatformError> {
        let mut attrs = WinitWindowAdapter::window_attributes()?;

        if let Some(hook) = &self.window_attributes_hook {
            attrs = hook(attrs);
        }

        let adapter = WinitWindowAdapter::new(
            self.shared_data.clone(),
            (self.renderer_factory_fn)(),
            attrs.clone(),
            self.requested_graphics_api.clone(),
            #[cfg(any(enable_accesskit, muda))]
            self.shared_data.event_loop_proxy.clone(),
            #[cfg(all(muda, target_os = "macos"))]
            self.muda_enable_default_menu_bar_bar,
        )
        .or_else(|e| {
            try_create_window_with_fallback_renderer(
                &self.shared_data,
                attrs,
                &self.shared_data.event_loop_proxy.clone(),
                #[cfg(all(muda, target_os = "macos"))]
                self.muda_enable_default_menu_bar_bar,
            )
            .ok_or_else(|| format!("Winit backend failed to find a suitable renderer: {e}"))
        })?;
        Ok(adapter)
    }

    fn run_event_loop(&self) -> Result<(), PlatformError> {
        let loop_state = self
            .event_loop_state
            .borrow_mut()
            .take()
            .unwrap_or_else(|| EventLoopState::new(self.shared_data.clone()));
        #[cfg(target_family = "wasm")]
        {
            if self.spawn_event_loop {
                return loop_state.spawn();
            }
        }
        let new_state = loop_state.run()?;
        *self.event_loop_state.borrow_mut() = Some(new_state);
        Ok(())
    }

    #[cfg(all(not(target_arch = "wasm32"), not(target_os = "ios")))]
    fn process_events(
        &self,
        timeout: core::time::Duration,
        _: i_slint_core::InternalToken,
    ) -> Result<core::ops::ControlFlow<()>, PlatformError> {
        let loop_state = self
            .event_loop_state
            .borrow_mut()
            .take()
            .unwrap_or_else(|| EventLoopState::new(self.shared_data.clone()));
        let (new_state, status) = loop_state.pump_events(Some(timeout))?;
        *self.event_loop_state.borrow_mut() = Some(new_state);
        match status {
            winit::platform::pump_events::PumpStatus::Continue => {
                Ok(core::ops::ControlFlow::Continue(()))
            }
            winit::platform::pump_events::PumpStatus::Exit(code) => {
                if code == 0 {
                    Ok(core::ops::ControlFlow::Break(()))
                } else {
                    Err(format!("Event loop exited with non-zero code {code}").into())
                }
            }
        }
    }

    fn new_event_loop_proxy(&self) -> Option<Box<dyn EventLoopProxy>> {
        struct Proxy(winit::event_loop::EventLoopProxy<SlintUserEvent>);
        impl EventLoopProxy for Proxy {
            fn quit_event_loop(&self) -> Result<(), EventLoopError> {
                self.0
                    .send_event(SlintUserEvent(CustomEvent::Exit))
                    .map_err(|_| EventLoopError::EventLoopTerminated)
            }

            fn invoke_from_event_loop(
                &self,
                event: Box<dyn FnOnce() + Send>,
            ) -> Result<(), EventLoopError> {
                // Calling send_event is usually done by winit at the bottom of the stack,
                // in event handlers, and thus winit might decide to process the event
                // immediately within that stack.
                // To prevent re-entrancy issues that might happen by getting the application
                // event processed on top of the current stack, set winit in Poll mode so that
                // events are queued and process on top of a clean stack during a requested animation
                // frame a few moments later.
                // This also allows batching multiple post_event calls and redraw their state changes
                // all at once.
                #[cfg(target_arch = "wasm32")]
                self.0
                    .send_event(SlintUserEvent(CustomEvent::WakeEventLoopWorkaround))
                    .map_err(|_| EventLoopError::EventLoopTerminated)?;

                self.0
                    .send_event(SlintUserEvent(CustomEvent::UserEvent(event)))
                    .map_err(|_| EventLoopError::EventLoopTerminated)
            }
        }
        Some(Box::new(Proxy(self.shared_data.event_loop_proxy.clone())))
    }

    #[cfg(target_arch = "wasm32")]
    fn set_clipboard_text(&self, text: &str, clipboard: i_slint_core::platform::Clipboard) {
        crate::wasm_input_helper::set_clipboard_text(text.into(), clipboard);
    }

    #[cfg(not(target_arch = "wasm32"))]
    fn set_clipboard_text(&self, text: &str, clipboard: i_slint_core::platform::Clipboard) {
        let mut pair = self.shared_data.clipboard.borrow_mut();
        if let Some(clipboard) = clipboard::select_clipboard(&mut pair, clipboard) {
            clipboard.set_contents(text.into()).ok();
        }
    }

    #[cfg(target_arch = "wasm32")]
    fn clipboard_text(&self, clipboard: i_slint_core::platform::Clipboard) -> Option<String> {
        crate::wasm_input_helper::get_clipboard_text(clipboard)
    }

    #[cfg(not(target_arch = "wasm32"))]
    fn clipboard_text(&self, clipboard: i_slint_core::platform::Clipboard) -> Option<String> {
        let mut pair = self.shared_data.clipboard.borrow_mut();
        clipboard::select_clipboard(&mut pair, clipboard).and_then(|c| c.get_contents().ok())
    }
}

mod private {
    pub trait WinitWindowAccessorSealed {}
}

#[i_slint_core_macros::slint_doc]
/// This helper trait can be used to obtain access to the [`winit::window::Window`] for a given
/// [`slint::Window`](slint:rust:slint/struct.window).")]
pub trait WinitWindowAccessor: private::WinitWindowAccessorSealed {
    /// Returns true if a [`winit::window::Window`] exists for this window. This is the case if the window is
    /// backed by this winit backend.
    fn has_winit_window(&self) -> bool;
    /// Invokes the specified callback with a reference to the [`winit::window::Window`] that exists for this Slint window
    /// and returns `Some(T)`; otherwise `None`.
    fn with_winit_window<T>(&self, callback: impl FnOnce(&winit::window::Window) -> T)
        -> Option<T>;
    /// Registers a window event filter callback for this Slint window.
    ///
    /// The callback is invoked in the winit event loop whenever a window event is received with a reference to the
    /// [`slint::Window`](i_slint_core::api::Window) and the [`winit::event::WindowEvent`]. The return value of the
    /// callback specifies whether Slint should handle this event.
    ///
    /// If this window [is not backed by winit](WinitWindowAccessor::has_winit_window), this function is a no-op.
    fn on_winit_window_event(
        &self,
        callback: impl FnMut(&i_slint_core::api::Window, &winit::event::WindowEvent) -> WinitWindowEventResult
            + 'static,
    );

    /// Creates a non Slint aware window with winit
    fn create_winit_window(
        &self,
        window_attributes: winit::window::WindowAttributes,
    ) -> Result<winit::window::Window, winit::error::OsError>;
}

impl WinitWindowAccessor for i_slint_core::api::Window {
    fn has_winit_window(&self) -> bool {
        i_slint_core::window::WindowInner::from_pub(self)
            .window_adapter()
            .internal(i_slint_core::InternalToken)
            .and_then(|wa| wa.as_any().downcast_ref::<WinitWindowAdapter>())
            .is_some_and(|adapter| adapter.winit_window().is_some())
    }

    fn with_winit_window<T>(
        &self,
        callback: impl FnOnce(&winit::window::Window) -> T,
    ) -> Option<T> {
        i_slint_core::window::WindowInner::from_pub(self)
            .window_adapter()
            .internal(i_slint_core::InternalToken)
            .and_then(|wa| wa.as_any().downcast_ref::<WinitWindowAdapter>())
            .and_then(|adapter| adapter.winit_window().map(|w| callback(&w)))
    }

    fn on_winit_window_event(
        &self,
        mut callback: impl FnMut(&i_slint_core::api::Window, &winit::event::WindowEvent) -> WinitWindowEventResult
            + 'static,
    ) {
        if let Some(adapter) = i_slint_core::window::WindowInner::from_pub(self)
            .window_adapter()
            .internal(i_slint_core::InternalToken)
            .and_then(|wa| wa.as_any().downcast_ref::<WinitWindowAdapter>())
        {
            adapter
                .window_event_filter
                .set(Some(Box::new(move |window, event| callback(window, event))));
        }
    }

    /// Creates a non Slint aware window with winit
    fn create_winit_window(
        &self,
        window_attributes: winit::window::WindowAttributes,
    ) -> Result<winit::window::Window, winit::error::OsError> {
        i_slint_core::window::WindowInner::from_pub(self)
            .window_adapter()
            .internal(i_slint_core::InternalToken)
            .unwrap()
            .as_any()
            .downcast_ref::<WinitWindowAdapter>()
            .unwrap()
            .shared_backend_data
            .with_event_loop(|eli| Ok(eli.create_window(window_attributes)))
            .unwrap()
    }
}

impl private::WinitWindowAccessorSealed for i_slint_core::api::Window {}

#[cfg(test)]
mod testui {
    slint::slint! {
        export component App inherits Window {
            Text { text: "Ok"; }
        }
    }
}

// Sorry, can't test with rust test harness and multiple threads.
#[cfg(not(any(target_arch = "wasm32", target_os = "macos", target_os = "ios")))]
#[test]
fn test_window_accessor_and_rwh() {
    slint::platform::set_platform(Box::new(crate::Backend::new().unwrap())).unwrap();

    use testui::*;
    let app = App::new().unwrap();
    let slint_window = app.window();
    assert!(slint_window.has_winit_window());
    let handle = slint_window.window_handle();
    use raw_window_handle::{HasDisplayHandle, HasWindowHandle};
    assert!(handle.window_handle().is_ok());
    assert!(handle.display_handle().is_ok());
}
