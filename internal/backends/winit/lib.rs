// Copyright © SixtyFPS GmbH <info@slint-ui.com>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-commercial

#![doc = include_str!("README.md")]
#![doc(html_logo_url = "https://slint-ui.com/logo/slint-logo-square-light.svg")]

#[cfg(all(not(feature = "renderer-femtovg"), not(feature = "renderer-skia")))]
compile_error!("Please select a feature to build with the winit event loop: `renderer-femtovg`, `renderer-skia`");

extern crate alloc;

use std::rc::Rc;
use std::sync::Mutex;

use i_slint_core::window::PlatformWindow;

mod glwindow;
use glwindow::*;
mod glcontext;
use glcontext::*;
pub(crate) mod event_loop;
mod renderer {
    use std::rc::Weak;

    use i_slint_core::window::PlatformWindow;

    mod boxshadowcache;

    pub(crate) trait WinitCompatibleRenderer: i_slint_core::renderer::Renderer {
        type Canvas: WinitCompatibleCanvas;

        fn new(
            platform_window_weak: &Weak<dyn PlatformWindow>,
            #[cfg(target_arch = "wasm32")] canvas_id: String,
        ) -> Self;

        fn create_canvas(&self, window_builder: winit::window::WindowBuilder) -> Self::Canvas;
        fn release_canvas(&self, canvas: Self::Canvas);

        fn render(&self, canvas: &Self::Canvas, window: &dyn PlatformWindow);
    }

    pub(crate) trait WinitCompatibleCanvas {
        fn component_destroyed(&self, component: i_slint_core::component::ComponentRef);

        fn with_window_handle<T>(&self, callback: impl FnOnce(&winit::window::Window) -> T) -> T;

        fn resize_event(&self);

        #[cfg(target_arch = "wasm32")]
        fn html_canvas_element(&self) -> std::cell::Ref<web_sys::HtmlCanvasElement>;
    }

    #[cfg(feature = "renderer-femtovg")]
    pub(crate) mod femtovg;
    #[cfg(feature = "renderer-skia")]
    pub(crate) mod skia;

    #[cfg(feature = "renderer-software")]
    pub(crate) mod sw;
}

#[cfg(target_arch = "wasm32")]
pub(crate) mod wasm_input_helper;

mod stylemetrics;

#[cfg(target_arch = "wasm32")]
pub fn create_gl_window_with_canvas_id(canvas_id: String) -> Rc<dyn PlatformWindow> {
    GLWindow::<crate::renderer::femtovg::FemtoVGRenderer>::new(canvas_id)
}

#[doc(hidden)]
#[cold]
#[cfg(not(target_arch = "wasm32"))]
pub fn use_modules() {}

pub type NativeWidgets = ();
pub type NativeGlobals = (stylemetrics::NativeStyleMetrics, ());
pub mod native_widgets {
    pub use super::stylemetrics::NativeStyleMetrics;
}
pub const HAS_NATIVE_STYLE: bool = false;

use i_slint_core::platform::EventLoopProxy;
pub use stylemetrics::native_style_metrics_deinit;
pub use stylemetrics::native_style_metrics_init;

pub struct Backend {
    window_factory_fn: Mutex<Box<dyn Fn() -> Rc<dyn PlatformWindow> + Send>>,
}

impl Backend {
    pub fn new(renderer_name: Option<&str>) -> Self {
        #[cfg(feature = "renderer-femtovg")]
        let (default_renderer, default_renderer_factory) = ("FemtoVG", || {
            GLWindow::<renderer::femtovg::FemtoVGRenderer>::new(
                #[cfg(target_arch = "wasm32")]
                "canvas".into(),
            )
        });
        #[cfg(all(not(feature = "renderer-femtovg"), feature = "renderer-skia"))]
        let (default_renderer, default_renderer_factory) = ("Skia", || {
            GLWindow::<renderer::skia::SkiaRenderer>::new(
                #[cfg(target_arch = "wasm32")]
                "canvas".into(),
            )
        });

        let factory_fn = match renderer_name {
            #[cfg(feature = "renderer-femtovg")]
            Some("gl") | Some("femtovg") => || {
                GLWindow::<renderer::femtovg::FemtoVGRenderer>::new(
                    #[cfg(target_arch = "wasm32")]
                    "canvas".into(),
                )
            },
            #[cfg(feature = "renderer-skia")]
            Some("skia") => || {
                GLWindow::<renderer::skia::SkiaRenderer>::new(
                    #[cfg(target_arch = "wasm32")]
                    "canvas".into(),
                )
            },
            #[cfg(feature = "renderer-software")]
            Some("sw") | Some("software") => || {
                GLWindow::<renderer::sw::SoftwareRenderer>::new(
                    #[cfg(target_arch = "wasm32")]
                    "canvas".into(),
                )
            },
            None => default_renderer_factory,
            Some(renderer_name) => {
                eprintln!(
                    "slint winit: unrecognized renderer {}, falling back to {}",
                    renderer_name, default_renderer
                );
                default_renderer_factory
            }
        };
        Self { window_factory_fn: Mutex::new(Box::new(factory_fn)) }
    }
}

impl i_slint_core::platform::PlatformAbstraction for Backend {
    fn create_window(&self) -> Rc<dyn PlatformWindow> {
        self.window_factory_fn.lock().unwrap()()
    }

    fn run_event_loop(&self, behavior: i_slint_core::platform::EventLoopQuitBehavior) {
        crate::event_loop::run(behavior);
    }

    fn new_event_loop_proxy(&self) -> Option<Box<dyn EventLoopProxy>> {
        struct Proxy;
        impl EventLoopProxy for Proxy {
            fn quit_event_loop(&self) {
                crate::event_loop::with_window_target(|event_loop| {
                    event_loop
                        .event_loop_proxy()
                        .send_event(crate::event_loop::CustomEvent::Exit)
                        .ok();
                })
            }

            fn invoke_from_event_loop(&self, event: Box<dyn FnOnce() + Send>) {
                let e = crate::event_loop::CustomEvent::UserEvent(event);
                #[cfg(not(target_arch = "wasm32"))]
                crate::event_loop::GLOBAL_PROXY
                    .get_or_init(Default::default)
                    .lock()
                    .unwrap()
                    .send_event(e);
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
                        proxy.send_event(crate::event_loop::CustomEvent::WakeEventLoopWorkaround);
                        proxy.send_event(e);
                    });
                }
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

pub(crate) trait WindowSystemName {
    fn winsys_name(&self) -> &'static str;
}

impl WindowSystemName for winit::window::Window {
    fn winsys_name(&self) -> &'static str {
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
                use winit::platform::unix::WindowExtUnix;
                let mut winsys = "unknown";

                #[cfg(feature = "x11")]
                if self.xlib_window().is_some() {
                    winsys = "x11";
                }

                #[cfg(feature = "wayland")]
                if self.wayland_surface().is_some() {
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
}
