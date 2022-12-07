// Copyright © SixtyFPS GmbH <info@slint-ui.com>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-commercial

#![doc = include_str!("README.md")]
#![doc(html_logo_url = "https://slint-ui.com/logo/slint-logo-square-light.svg")]

extern crate alloc;

use i_slint_core::platform::EventLoopProxy;
use i_slint_core::window::WindowAdapter;
use renderer::WinitCompatibleRenderer;
use std::rc::Rc;

mod glwindow;
use glwindow::*;
#[cfg(any(feature = "femtovg", skia_backend_opengl))]
mod glcontext;
#[cfg(any(feature = "femtovg", skia_backend_opengl))]
use glcontext::*;
pub(crate) mod event_loop;

mod renderer {
    use std::rc::Weak;

    use i_slint_core::api::PhysicalSize;
    use i_slint_core::lengths::LogicalLength;
    use i_slint_core::window::WindowAdapter;

    #[cfg(any(feature = "renderer-winit-femtovg", enable_skia_renderer))]
    mod boxshadowcache;

    pub(crate) trait WinitCompatibleRenderer: i_slint_core::renderer::Renderer {
        type Canvas: WinitCompatibleCanvas;
        const NAME: &'static str;

        fn new(window_adapter_weak: &Weak<dyn WindowAdapter>) -> Self;

        fn create_canvas(
            &self,
            window: &dyn raw_window_handle::HasRawWindowHandle,
            display: &dyn raw_window_handle::HasRawDisplayHandle,
            size: PhysicalSize,
            #[cfg(target_arch = "wasm32")] canvas_id: &str,
        ) -> Self::Canvas;
        fn release_canvas(&self, canvas: Self::Canvas);

        fn render(&self, canvas: &Self::Canvas, size: PhysicalSize);

        fn default_font_size() -> LogicalLength;
    }

    pub(crate) trait WinitCompatibleCanvas {
        fn component_destroyed(&self, component: i_slint_core::component::ComponentRef);

        fn resize_event(&self, size: PhysicalSize);

        #[cfg(target_arch = "wasm32")]
        fn html_canvas_element(&self) -> std::cell::Ref<web_sys::HtmlCanvasElement>;
    }

    #[cfg(feature = "renderer-winit-femtovg")]
    pub(crate) mod femtovg;
    #[cfg(enable_skia_renderer)]
    pub(crate) mod skia;

    #[cfg(feature = "renderer-winit-software")]
    pub(crate) mod sw;
}

#[cfg(target_arch = "wasm32")]
pub(crate) mod wasm_input_helper;

#[cfg(target_arch = "wasm32")]
pub fn create_gl_window_with_canvas_id(canvas_id: String) -> Rc<dyn WindowAdapter> {
    GLWindow::<crate::renderer::femtovg::FemtoVGRenderer>::new(canvas_id)
}

fn window_factory_fn<R: WinitCompatibleRenderer + 'static>() -> Rc<dyn WindowAdapter> {
    GLWindow::<R>::new(
        #[cfg(target_arch = "wasm32")]
        "canvas".into(),
    )
}

cfg_if::cfg_if! {
    if #[cfg(feature = "renderer-winit-femtovg")] {
        type DefaultRenderer = renderer::femtovg::FemtoVGRenderer;
    } else if #[cfg(enable_skia_renderer)] {
        type DefaultRenderer = renderer::skia::SkiaRenderer;
    } else if #[cfg(feature = "renderer-winit-software")] {
        type DefaultRenderer = renderer::sw::SoftwareRenderer<0>;
    } else {
        compile_error!("Please select a feature to build with the winit backend: `renderer-winit-femtovg`, `renderer-winit-skia`, `renderer-winit-skia-opengl` or `renderer-winit-software`");
    }
}

#[doc(hidden)]
#[cold]
#[cfg(not(target_arch = "wasm32"))]
pub fn use_modules() {}

pub type NativeWidgets = ();
pub type NativeGlobals = ();
pub const HAS_NATIVE_STYLE: bool = false;
pub mod native_widgets {}

pub struct Backend {
    window_factory_fn: fn() -> Rc<dyn WindowAdapter>,
}

impl Backend {
    pub fn new(renderer_name: Option<&str>) -> Self {
        let window_factory_fn = match renderer_name {
            #[cfg(feature = "renderer-winit-femtovg")]
            Some("gl") | Some("femtovg") => window_factory_fn::<renderer::femtovg::FemtoVGRenderer>,
            #[cfg(enable_skia_renderer)]
            Some("skia") => window_factory_fn::<renderer::skia::SkiaRenderer>,
            #[cfg(feature = "renderer-winit-software")]
            Some("sw") | Some("software") => window_factory_fn::<renderer::sw::SoftwareRenderer<0>>,
            None => window_factory_fn::<DefaultRenderer>,
            Some(renderer_name) => {
                eprintln!(
                    "slint winit: unrecognized renderer {}, falling back to {}",
                    renderer_name,
                    DefaultRenderer::NAME
                );
                window_factory_fn::<DefaultRenderer>
            }
        };
        Self { window_factory_fn }
    }
}

impl i_slint_core::platform::Platform for Backend {
    fn create_window_adapter(&self) -> Rc<dyn WindowAdapter> {
        (self.window_factory_fn)()
    }

    #[doc(hidden)]
    fn set_event_loop_quit_on_last_window_closed(&self, quit_on_last_window_closed: bool) {
        event_loop::QUIT_ON_LAST_WINDOW_CLOSED
            .store(quit_on_last_window_closed, std::sync::atomic::Ordering::Relaxed);
    }

    fn run_event_loop(&self) {
        crate::event_loop::run();
    }

    fn new_event_loop_proxy(&self) -> Option<Box<dyn EventLoopProxy>> {
        struct Proxy;
        impl EventLoopProxy for Proxy {
            fn quit_event_loop(&self) -> Result<(), i_slint_core::api::EventLoopError> {
                crate::event_loop::with_window_target(|event_loop| {
                    event_loop
                        .event_loop_proxy()
                        .send_event(crate::event_loop::CustomEvent::Exit)
                        .map_err(|_| i_slint_core::api::EventLoopError::EventLoopTerminated)
                })
            }

            fn invoke_from_event_loop(
                &self,
                event: Box<dyn FnOnce() + Send>,
            ) -> Result<(), i_slint_core::api::EventLoopError> {
                let e = crate::event_loop::CustomEvent::UserEvent(event);
                #[cfg(not(target_arch = "wasm32"))]
                crate::event_loop::GLOBAL_PROXY
                    .get_or_init(Default::default)
                    .lock()
                    .unwrap()
                    .send_event(e)?;
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
                        proxy
                            .send_event(crate::event_loop::CustomEvent::WakeEventLoopWorkaround)?;
                        proxy.send_event(e)?;
                        Ok(())
                    })?
                }
                Ok(())
            }
        }
        Some(Box::new(Proxy))
    }

    fn set_clipboard_text(&self, text: &str) {
        crate::event_loop::with_window_target(|event_loop_target| {
            event_loop_target.clipboard().set_contents(text.into()).ok()
        });
    }

    fn clipboard_text(&self) -> Option<String> {
        crate::event_loop::with_window_target(|event_loop_target| {
            event_loop_target.clipboard().get_contents().ok()
        })
    }
}

fn winsys_name(_window: &dyn raw_window_handle::HasRawWindowHandle) -> &'static str {
    cfg_if::cfg_if! {
        if #[cfg(target_arch = "wasm32")] {
            let winsys = "HTML Canvas";
        } else if #[cfg(any(
            target_os = "linux",
            target_os = "dragonfly",
            target_os = "freebsd",
            target_os = "netbsd",
            target_os = "openbsd"
        ))] {
            let mut winsys = "unknown";

            #[cfg(feature = "x11")]
            if matches!(_window.raw_window_handle(), raw_window_handle::RawWindowHandle::Xcb(..)) {
                winsys = "x11";
            }

            #[cfg(feature = "wayland")]
            if matches!(_window.raw_window_handle(), raw_window_handle::RawWindowHandle::Wayland(..)) {
                winsys = "wayland"
            }
        } else if #[cfg(target_os = "windows")] {
            let winsys = "windows";
        } else if #[cfg(target_os = "macos")] {
            let winsys = "macos";
        } else {
            let winsys = "unknown";
        }
    }
    winsys
}
