// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

#![doc = include_str!("README.md")]
#![doc(html_logo_url = "https://slint.dev/logo/slint-logo-square-light.svg")]

extern crate alloc;

use event_loop::CustomEvent;
use i_slint_core::platform::EventLoopProxy;
use i_slint_core::window::WindowAdapter;
use renderer::WinitCompatibleRenderer;
use std::rc::Rc;
#[cfg(not(target_arch = "wasm32"))]
use std::rc::Weak;

#[cfg(not(target_arch = "wasm32"))]
mod clipboard;
mod drag_resize_window;
mod winitwindowadapter;

use i_slint_core::platform::PlatformError;
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

    use i_slint_core::platform::PlatformError;

    pub trait WinitCompatibleRenderer {
        fn render(&self, window: &i_slint_core::api::Window) -> Result<(), PlatformError>;

        fn as_core_renderer(&self) -> &dyn i_slint_core::renderer::Renderer;
        // Got WindowEvent::Occluded
        fn occluded(&self, _: bool) {}

        fn suspend(&self) -> Result<(), PlatformError>;

        // Got winit::Event::Resumed
        fn resume(
            &self,
            window_attributes: winit::window::WindowAttributes,
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

#[cfg(target_arch = "wasm32")]
pub(crate) mod wasm_input_helper;

#[cfg(all(target_arch = "wasm32", feature = "renderer-femtovg"))]
pub fn create_gl_window_with_canvas_id(
    canvas_id: &str,
) -> Result<Rc<dyn WindowAdapter>, PlatformError> {
    let attrs = WinitWindowAdapter::window_attributes(canvas_id)?;
    let adapter =
        WinitWindowAdapter::new(renderer::femtovg::GlutinFemtoVGRenderer::new_suspended(), attrs)?;
    Ok(adapter)
}

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
    attrs: winit::window::WindowAttributes,
    _proxy: &winit::event_loop::EventLoopProxy<SlintUserEvent>,
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
            renderer_factory(),
            attrs.clone(),
            #[cfg(enable_accesskit)]
            _proxy.clone(),
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

#[doc = concat!("This struct implements the Slint Platform trait. Use this in conjunction with [`slint::platform::set_platform`](https://slint.dev/releases/", env!("CARGO_PKG_VERSION"), "/docs/rust/slint/platform/fn.set_platform.html) to initialize.")]
/// Slint to use winit for all windowing system interaction.
///
/// ```rust,no_run
/// use i_slint_backend_winit::Backend;
/// slint::platform::set_platform(Box::new(Backend::new().unwrap()));
/// ```
pub struct Backend {
    renderer_factory_fn: fn() -> Box<dyn WinitCompatibleRenderer>,
    event_loop_state: std::cell::RefCell<Option<crate::event_loop::EventLoopState>>,
    proxy: winit::event_loop::EventLoopProxy<SlintUserEvent>,

    /// This hook is called before a Window is created.
    ///
    /// It can be used to adjust settings of window that will be created
    ///
    /// # Example
    ///
    /// ```rust,no_run
    /// let mut backend = i_slint_backend_winit::Backend::new().unwrap();
    /// backend.window_builder_hook = Some(Box::new(|builder| builder.with_content_protected(true)));
    /// slint::platform::set_platform(Box::new(backend));
    /// ```
    pub window_builder_hook:
        Option<Box<dyn Fn(winit::window::WindowAttributes) -> winit::window::WindowAttributes>>,

    #[cfg(not(target_arch = "wasm32"))]
    clipboard: Weak<std::cell::RefCell<clipboard::ClipboardPair>>,
}

impl Backend {
    #[doc = concat!("Creates a new winit backend with the default renderer that's compiled in. See the [backend documentation](https://slint.dev/releases/", env!("CARGO_PKG_VERSION"), "/docs/rust/slint/index.html#backends) for")]
    /// details on how to select the default renderer.
    pub fn new() -> Result<Self, PlatformError> {
        Self::new_with_renderer_by_name(None)
    }

    #[doc = concat!("Creates a new winit backend with the renderer specified by name. See the [backend documentation](https://slint.dev/releases/", env!("CARGO_PKG_VERSION"), "/docs/rust/slint/index.html#backends) for")]
    /// details on how to select the default renderer.
    /// If the renderer name is `None` or the name is not recognized, the default renderer is selected.
    pub fn new_with_renderer_by_name(renderer_name: Option<&str>) -> Result<Self, PlatformError> {
        // Initialize the winit event loop and propagate errors if for example `DISPLAY` or `WAYLAND_DISPLAY` isn't set.

        let proxy =
            crate::event_loop::with_not_running_event_loop(|nre| Ok(nre.instance.create_proxy()))?;

        #[cfg(not(target_arch = "wasm32"))]
        let clipboard = crate::event_loop::with_not_running_event_loop(|nre| {
            Ok(Rc::downgrade(&nre.clipboard))
        })?;

        let renderer_factory_fn = match renderer_name {
            #[cfg(feature = "renderer-femtovg")]
            Some("gl") | Some("femtovg") => renderer::femtovg::GlutinFemtoVGRenderer::new_suspended,
            #[cfg(enable_skia_renderer)]
            Some("skia") => renderer::skia::WinitSkiaRenderer::new_suspended,
            #[cfg(enable_skia_renderer)]
            Some("skia-opengl") => renderer::skia::WinitSkiaRenderer::new_opengl_suspended,
            #[cfg(all(enable_skia_renderer, not(target_os = "android")))]
            Some("skia-software") => renderer::skia::WinitSkiaRenderer::new_software_suspended,
            #[cfg(feature = "renderer-software")]
            Some("sw") | Some("software") => renderer::sw::WinitSoftwareRenderer::new_suspended,
            None => default_renderer_factory,
            Some(renderer_name) => {
                eprintln!(
                    "slint winit: unrecognized renderer {}, falling back to {}",
                    renderer_name, DEFAULT_RENDERER_NAME
                );
                default_renderer_factory
            }
        };
        Ok(Self {
            renderer_factory_fn,
            event_loop_state: Default::default(),
            window_builder_hook: None,
            #[cfg(not(target_arch = "wasm32"))]
            clipboard: clipboard.into(),
            proxy,
        })
    }
}

fn send_event_via_global_event_loop_proxy(
    event: SlintUserEvent,
) -> Result<(), i_slint_core::api::EventLoopError> {
    #[cfg(not(target_arch = "wasm32"))]
    crate::event_loop::GLOBAL_PROXY
        .get_or_init(Default::default)
        .lock()
        .unwrap()
        .send_event(event)?;
    #[cfg(target_arch = "wasm32")]
    {
        crate::event_loop::GLOBAL_PROXY.with(|global_proxy| {
            let mut maybe_proxy = global_proxy.borrow_mut();
            let proxy = maybe_proxy.get_or_insert_with(Default::default);
            // Calling send_event is usually done by winit at the bottom of the stack,
            // in event handlers, and thus winit might decide to process the event
            // immediately within that stack.
            // To prevent re-entrancy issues that might happen by getting the application
            // event processed on top of the current stack, set winit in Poll mode so that
            // events are queued and process on top of a clean stack during a requested animation
            // frame a few moments later.
            // This also allows batching multiple post_event calls and redraw their state changes
            // all at once.
            proxy.send_event(SlintUserEvent(CustomEvent::WakeEventLoopWorkaround))?;
            proxy.send_event(event)?;
            Ok(())
        })?
    }
    Ok(())
}

impl i_slint_core::platform::Platform for Backend {
    fn create_window_adapter(&self) -> Result<Rc<dyn WindowAdapter>, PlatformError> {
        let mut builder = WinitWindowAdapter::window_attributes(
            #[cfg(target_arch = "wasm32")]
            "canvas".into(),
        )?;

        if let Some(hook) = &self.window_builder_hook {
            builder = hook(builder);
        }

        let adapter = WinitWindowAdapter::new(
            (self.renderer_factory_fn)(),
            builder.clone(),
            #[cfg(enable_accesskit)]
            self.proxy.clone(),
        )
        .or_else(|e| {
            try_create_window_with_fallback_renderer(builder, &self.proxy)
                .ok_or_else(|| format!("Winit backend failed to find a suitable renderer: {e}"))
        })?;
        Ok(adapter)
    }

    fn run_event_loop(&self) -> Result<(), PlatformError> {
        let loop_state = self.event_loop_state.borrow_mut().take().unwrap_or_default();
        let new_state = loop_state.run()?;
        *self.event_loop_state.borrow_mut() = Some(new_state);
        Ok(())
    }

    #[cfg(not(target_arch = "wasm32"))]
    fn process_events(
        &self,
        timeout: core::time::Duration,
        _: i_slint_core::InternalToken,
    ) -> Result<core::ops::ControlFlow<()>, PlatformError> {
        let loop_state = self.event_loop_state.borrow_mut().take().unwrap_or_default();
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
        struct Proxy;
        impl EventLoopProxy for Proxy {
            fn quit_event_loop(&self) -> Result<(), i_slint_core::api::EventLoopError> {
                send_event_via_global_event_loop_proxy(SlintUserEvent(CustomEvent::Exit))
            }

            fn invoke_from_event_loop(
                &self,
                event: Box<dyn FnOnce() + Send>,
            ) -> Result<(), i_slint_core::api::EventLoopError> {
                let e = SlintUserEvent(CustomEvent::UserEvent(event));
                send_event_via_global_event_loop_proxy(e)
            }
        }
        Some(Box::new(Proxy))
    }

    #[cfg(target_arch = "wasm32")]
    fn set_clipboard_text(&self, text: &str, clipboard: i_slint_core::platform::Clipboard) {
        crate::wasm_input_helper::set_clipboard_text(text.into(), clipboard);
    }

    #[cfg(not(target_arch = "wasm32"))]
    fn set_clipboard_text(&self, text: &str, clipboard: i_slint_core::platform::Clipboard) {
        let Some(clipboard_pair) = self.clipboard.upgrade() else { return };
        let mut pair = clipboard_pair.borrow_mut();
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
        let clipboard_pair = self.clipboard.upgrade()?;
        let mut pair = clipboard_pair.borrow_mut();
        clipboard::select_clipboard(&mut pair, clipboard).and_then(|c| c.get_contents().ok())
    }
}

/// Spawn the event loop, using [`winit::platform::web::EventLoopExtWebSys::spawn()`]
#[cfg(target_arch = "wasm32")]
pub fn spawn_event_loop() -> Result<(), PlatformError> {
    crate::event_loop::spawn()
}

mod private {
    pub trait WinitWindowAccessorSealed {}
}

#[doc = concat!("This helper trait can be used to obtain access to the [`winit::window::Window`] for a given [`slint::Window`](https://slint.dev/releases/", env!("CARGO_PKG_VERSION"), "/docs/rust/slint/struct.window).")]
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
}

impl WinitWindowAccessor for i_slint_core::api::Window {
    fn has_winit_window(&self) -> bool {
        i_slint_core::window::WindowInner::from_pub(self)
            .window_adapter()
            .internal(i_slint_core::InternalToken)
            .and_then(|wa| wa.as_any().downcast_ref::<WinitWindowAdapter>())
            .map_or(false, |adapter| adapter.winit_window().is_some())
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
        i_slint_core::window::WindowInner::from_pub(&self)
            .window_adapter()
            .internal(i_slint_core::InternalToken)
            .and_then(|wa| wa.as_any().downcast_ref::<WinitWindowAdapter>())
            .map(|adapter| {
                adapter
                    .window_event_filter
                    .set(Some(Box::new(move |window, event| callback(window, event))));
            });
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
