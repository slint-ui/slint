// Copyright © SixtyFPS GmbH <info@sixtyfps.io>
// SPDX-License-Identifier: (GPL-3.0-only OR LicenseRef-SixtyFPS-commercial)

// Copyright © SixtyFPS GmbH <info@sixtyfps.io>
// SPDX-License-Identifier: (GPL-3.0-only OR LicenseRef-SixtyFPS-commercial)

use std::cell::RefCell;
#[cfg(target_arch = "wasm32")]
use std::rc::Rc;

// glutin::WindowedContext tries to enforce being current or not. Since we need the WindowedContext's window() function
// in the GL renderer regardless whether we're current or not, we wrap the two states back into one type.
enum OpenGLContextState {
    #[cfg(not(target_arch = "wasm32"))]
    NotCurrent(glutin::WindowedContext<glutin::NotCurrent>),
    #[cfg(not(target_arch = "wasm32"))]
    Current(glutin::WindowedContext<glutin::PossiblyCurrent>),
    #[cfg(target_arch = "wasm32")]
    Current(Rc<winit::window::Window>),
}

pub struct OpenGLContext(RefCell<Option<OpenGLContextState>>);

impl OpenGLContext {
    pub fn window(&self) -> std::cell::Ref<winit::window::Window> {
        std::cell::Ref::map(self.0.borrow(), |state| match state.as_ref().unwrap() {
            #[cfg(not(target_arch = "wasm32"))]
            OpenGLContextState::NotCurrent(context) => context.window(),
            #[cfg(not(target_arch = "wasm32"))]
            OpenGLContextState::Current(context) => context.window(),
            #[cfg(target_arch = "wasm32")]
            OpenGLContextState::Current(window) => window.as_ref(),
        })
    }

    pub fn make_current(&self) {
        let mut ctx = self.0.borrow_mut();
        *ctx = Some(match ctx.take().unwrap() {
            #[cfg(not(target_arch = "wasm32"))]
            OpenGLContextState::NotCurrent(not_current_ctx) => {
                let current_ctx = unsafe { not_current_ctx.make_current().unwrap() };
                OpenGLContextState::Current(current_ctx)
            }
            state @ OpenGLContextState::Current(_) => state,
        });
    }

    pub fn make_not_current(&self) {
        #[cfg(not(target_arch = "wasm32"))]
        {
            let mut ctx = self.0.borrow_mut();
            *ctx = Some(match ctx.take().unwrap() {
                state @ OpenGLContextState::NotCurrent(_) => state,
                OpenGLContextState::Current(current_ctx_rc) => {
                    OpenGLContextState::NotCurrent(unsafe {
                        current_ctx_rc.make_not_current().unwrap()
                    })
                }
            });
        }
    }

    pub fn with_current_context<T>(&self, cb: impl FnOnce() -> T) -> T {
        if matches!(self.0.borrow().as_ref().unwrap(), OpenGLContextState::Current(_)) {
            cb()
        } else {
            self.make_current();
            let result = cb();
            self.make_not_current();
            result
        }
    }

    pub fn swap_buffers(&self) {
        #[cfg(not(target_arch = "wasm32"))]
        match &self.0.borrow().as_ref().unwrap() {
            OpenGLContextState::NotCurrent(_) => {}
            OpenGLContextState::Current(current_ctx) => {
                current_ctx.swap_buffers().unwrap();
            }
        }
    }

    pub fn ensure_resized(&self) {
        #[cfg(not(target_arch = "wasm32"))]
        match &self.0.borrow().as_ref().unwrap() {
            OpenGLContextState::NotCurrent(_) => {
                sixtyfps_corelib::debug_log!("internal error: cannot call OpenGLContext::ensure_resized without context being current!")
            }
            OpenGLContextState::Current(_current) => {
                _current.resize(_current.window().inner_size());
            }
        }
    }

    pub fn new_context_and_renderer(
        window_builder: winit::window::WindowBuilder,
        #[cfg(target_arch = "wasm32")] canvas_id: &str,
    ) -> (Self, femtovg::renderer::OpenGl) {
        #[cfg(not(target_arch = "wasm32"))]
        {
            use crate::event_loop::EventLoopInterface;
            use glutin::ContextBuilder;
            let windowed_context = crate::event_loop::with_window_target(|event_loop| {
                // Try different strategies for creating an GL context. First request our "favorite", OpenGL ES 2.0,
                // then try GlLatest (but with windows quirk) and finally try glutin's defaults.
                // We might be able to just go back to requesting GlLatest if
                // https://github.com/rust-windowing/glutin/issues/1371 is resolved
                // in favor of falling back to creating a GLES context.
                let context_factory_fns = [
                    |window_builder, event_loop: &dyn EventLoopInterface| {
                        let builder = ContextBuilder::new()
                            .with_vsync(true)
                            .with_gl(glutin::GlRequest::Specific(glutin::Api::OpenGlEs, (2, 0)));
                        #[cfg(target_os = "windows")]
                        let builder = builder.with_srgb(false);
                        builder
                            .build_windowed(window_builder, event_loop.event_loop_target())
                            .map_err(|creation_error| {
                                format!(
                                    "could not create OpenGL ES 2.0 context: {}",
                                    creation_error
                                )
                            })
                    },
                    |window_builder, event_loop: &dyn EventLoopInterface| {
                        let builder = ContextBuilder::new().with_vsync(true);
                        // With latest Windows 10 and VmWare glutin's default for srgb produces surfaces that are always rendered black :(
                        #[cfg(target_os = "windows")]
                        let builder = builder.with_srgb(false);
                        builder
                            .build_windowed(window_builder, event_loop.event_loop_target())
                            .map_err(|creation_error| {
                                format!(
                                    "could not create GlLatest context (with windows quirk): {}",
                                    creation_error
                                )
                            })
                    },
                    |window_builder, event_loop: &dyn EventLoopInterface| {
                        // Try again with glutin defaults
                        ContextBuilder::new()
                            .with_vsync(true)
                            .build_windowed(window_builder, event_loop.event_loop_target())
                            .map_err(|creation_error| {
                                format!("could not create GlLatest context : {}", creation_error)
                            })
                    },
                ];

                let mut last_err = None;
                for factory_fn in context_factory_fns {
                    match factory_fn(window_builder.clone(), event_loop) {
                        Ok(new_context) => {
                            return new_context;
                        }
                        Err(e) => {
                            last_err = Some(e);
                        }
                    }
                }

                panic!("Failed to create OpenGL context: {}", last_err.unwrap())
            });
            let windowed_context = unsafe { windowed_context.make_current().unwrap() };

            let renderer =
                femtovg::renderer::OpenGl::new_from_glutin_context(&windowed_context).unwrap();

            #[cfg(target_os = "macos")]
            {
                use cocoa::appkit::NSView;
                use winit::platform::macos::WindowExtMacOS;
                let ns_view = windowed_context.window().ns_view();
                let view_id: cocoa::base::id = ns_view as *const _ as *mut _;
                unsafe {
                    NSView::setLayerContentsPlacement(view_id, cocoa::appkit::NSViewLayerContentsPlacement::NSViewLayerContentsPlacementTopLeft)
                }
            }

            (Self(RefCell::new(Some(OpenGLContextState::Current(windowed_context)))), renderer)
        }

        #[cfg(target_arch = "wasm32")]
        {
            use wasm_bindgen::JsCast;

            let canvas = web_sys::window()
                .unwrap()
                .document()
                .unwrap()
                .get_element_by_id(canvas_id)
                .unwrap()
                .dyn_into::<web_sys::HtmlCanvasElement>()
                .unwrap();

            use winit::platform::web::WindowBuilderExtWebSys;
            use winit::platform::web::WindowExtWebSys;

            let existing_canvas_size = winit::dpi::LogicalSize::new(
                canvas.client_width() as u32,
                canvas.client_height() as u32,
            );

            let window = Rc::new(crate::event_loop::with_window_target(|event_loop| {
                window_builder
                    .with_canvas(Some(canvas.clone()))
                    .build(&event_loop.event_loop_target())
                    .unwrap()
            }));

            // Try to maintain the existing size of the canvas element. A window created with winit
            // on the web will always have 1024x768 as size otherwise.

            let resize_canvas = {
                let window = window.clone();
                let canvas = canvas.clone();
                move |_: web_sys::Event| {
                    let existing_canvas_size = winit::dpi::LogicalSize::new(
                        canvas.client_width() as u32,
                        canvas.client_height() as u32,
                    );

                    window.set_inner_size(existing_canvas_size);
                    window.request_redraw();
                    crate::event_loop::with_window_target(|event_loop| {
                        event_loop
                            .event_loop_proxy()
                            .send_event(crate::event_loop::CustomEvent::RedrawAllWindows)
                            .ok();
                    })
                }
            };

            let resize_closure =
                wasm_bindgen::closure::Closure::wrap(Box::new(resize_canvas) as Box<dyn FnMut(_)>);
            web_sys::window()
                .unwrap()
                .add_event_listener_with_callback("resize", resize_closure.as_ref().unchecked_ref())
                .unwrap();
            resize_closure.forget();

            {
                let default_size = window.inner_size().to_logical(window.scale_factor());
                let new_size = winit::dpi::LogicalSize::new(
                    if existing_canvas_size.width > 0 {
                        existing_canvas_size.width
                    } else {
                        default_size.width
                    },
                    if existing_canvas_size.height > 0 {
                        existing_canvas_size.height
                    } else {
                        default_size.height
                    },
                );
                if new_size != default_size {
                    window.set_inner_size(new_size);
                }
            }

            let renderer =
                femtovg::renderer::OpenGl::new_from_html_canvas(&window.canvas()).unwrap();
            (Self(RefCell::new(Some(OpenGLContextState::Current(window)))), renderer)
        }
    }
}
