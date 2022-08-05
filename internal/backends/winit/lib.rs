// Copyright © SixtyFPS GmbH <info@slint-ui.com>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-commercial

#![doc = include_str!("README.md")]
#![doc(html_logo_url = "https://slint-ui.com/logo/slint-logo-square-light.svg")]

#[cfg(all(not(feature = "renderer-femtovg"), not(feature = "renderer-skia")))]
compile_error!("Please select a feature to build with the winit event loop: `renderer-femtovg`, `renderer-skia`");

extern crate alloc;

use std::rc::Rc;
use std::sync::Mutex;

mod glwindow;
use glwindow::*;
mod glcontext;
use glcontext::*;
pub(crate) mod event_loop;
mod renderer {
    use std::rc::Weak;

    pub(crate) trait WinitCompatibleRenderer: i_slint_core::renderer::Renderer {
        type Canvas: WinitCompatibleCanvas;

        fn new(window_weak: &Weak<i_slint_core::window::WindowInner>) -> Self;

        #[cfg(not(target_arch = "wasm32"))]
        fn create_canvas_from_glutin_context(
            &self,
            gl_context: &glutin::WindowedContext<glutin::PossiblyCurrent>,
            winsys_name: Option<&str>,
        ) -> Self::Canvas;

        #[cfg(target_arch = "wasm32")]
        fn create_canvas_from_html_canvas(
            &self,
            canvas_element: &web_sys::HtmlCanvasElement,
        ) -> Self::Canvas;

        fn render(
            &self,
            canvas: &Self::Canvas,
            width: u32,
            height: u32,
            #[cfg(not(target_arch = "wasm32"))] gl_context: &glutin::WindowedContext<
                glutin::PossiblyCurrent,
            >,
            before_rendering_callback: impl FnOnce(),
        );
    }

    pub(crate) trait WinitCompatibleCanvas {
        fn release_graphics_resources(&self);

        fn component_destroyed(&self, component: i_slint_core::component::ComponentRef);
    }

    #[cfg(feature = "renderer-femtovg")]
    pub(crate) mod femtovg;
    #[cfg(feature = "renderer-skia")]
    pub(crate) mod skia;
}

#[cfg(target_arch = "wasm32")]
pub(crate) mod wasm_input_helper;

mod stylemetrics;

#[cfg(target_arch = "wasm32")]
pub fn create_gl_window_with_canvas_id(canvas_id: String) -> i_slint_core::api::Window {
    i_slint_core::window::WindowInner::new(|window| {
        GLWindow::<crate::renderer::femtovg::FemtoVGRenderer>::new(window, canvas_id)
    })
    .into()
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

pub use stylemetrics::native_style_metrics_deinit;
pub use stylemetrics::native_style_metrics_init;

pub struct Backend {
    window_factory_fn: Mutex<Box<dyn Fn() -> i_slint_core::api::Window + Send>>,
}

impl Backend {
    pub fn new(renderer_name: Option<&str>) -> Self {
        #[cfg(feature = "renderer-femtovg")]
        let (default_renderer, default_renderer_factory) = ("FemtoVG", || {
            i_slint_core::window::WindowInner::new(|window| {
                GLWindow::<renderer::femtovg::FemtoVGRenderer>::new(
                    window,
                    #[cfg(target_arch = "wasm32")]
                    "canvas".into(),
                )
            })
            .into()
        });
        #[cfg(all(not(feature = "renderer-femtovg"), feature = "renderer-skia"))]
        let (default_renderer, default_renderer_factory) = ("Skia", || {
            i_slint_core::window::WindowInner::new(|window| {
                GLWindow::<renderer::skia::SkiaRenderer>::new(
                    window,
                    #[cfg(target_arch = "wasm32")]
                    "canvas".into(),
                )
            })
            .into()
        });

        let factory_fn = match renderer_name {
            #[cfg(feature = "renderer-femtovg")]
            Some("gl") | Some("femtovg") => || {
                i_slint_core::window::WindowInner::new(|window| {
                    GLWindow::<renderer::femtovg::FemtoVGRenderer>::new(
                        window,
                        #[cfg(target_arch = "wasm32")]
                        "canvas".into(),
                    )
                })
                .into()
            },
            #[cfg(feature = "renderer-skia")]
            Some("skia") => || {
                i_slint_core::window::WindowInner::new(|window| {
                    GLWindow::<renderer::skia::SkiaRenderer>::new(
                        window,
                        #[cfg(target_arch = "wasm32")]
                        "canvas".into(),
                    )
                })
                .into()
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

impl i_slint_core::backend::Backend for Backend {
    fn create_window(&self) -> i_slint_core::api::Window {
        self.window_factory_fn.lock().unwrap()()
    }

    fn run_event_loop(&self, behavior: i_slint_core::backend::EventLoopQuitBehavior) {
        crate::event_loop::run(behavior);
    }

    fn quit_event_loop(&self) {
        crate::event_loop::with_window_target(|event_loop| {
            event_loop.event_loop_proxy().send_event(crate::event_loop::CustomEvent::Exit).ok();
        })
    }

    fn post_event(&self, event: Box<dyn FnOnce() + Send>) {
        let e = crate::event_loop::CustomEvent::UserEvent(event);
        #[cfg(not(target_arch = "wasm32"))]
        crate::event_loop::GLOBAL_PROXY.get_or_init(Default::default).lock().unwrap().send_event(e);
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
