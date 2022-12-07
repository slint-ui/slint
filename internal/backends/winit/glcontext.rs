// Copyright © SixtyFPS GmbH <info@slint-ui.com>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-commercial

use std::cell::RefCell;
use std::rc::Rc;

#[cfg(not(target_arch = "wasm32"))]
use glutin::{
    context::{ContextApi, ContextAttributesBuilder},
    display::GetGlDisplay,
    prelude::*,
    surface::{SurfaceAttributesBuilder, WindowSurface},
};

// glutin::WindowedContext tries to enforce being current or not. Since we need the WindowedContext's window() function
// in the GL renderer regardless whether we're current or not, we wrap the two states back into one type.
enum OpenGLContextState {
    #[cfg(not(target_arch = "wasm32"))]
    NotCurrent(
        (
            glutin::context::NotCurrentContext,
            glutin::surface::Surface<glutin::surface::WindowSurface>,
        ),
    ),
    #[cfg(not(target_arch = "wasm32"))]
    Current(
        (
            glutin::context::PossiblyCurrentContext,
            glutin::surface::Surface<glutin::surface::WindowSurface>,
        ),
    ),
    #[cfg(target_arch = "wasm32")]
    Current { canvas: web_sys::HtmlCanvasElement },
}

pub struct OpenGLContext(RefCell<Option<OpenGLContextState>>);

impl OpenGLContext {
    #[cfg(target_arch = "wasm32")]
    pub fn html_canvas_element(&self) -> std::cell::Ref<web_sys::HtmlCanvasElement> {
        std::cell::Ref::map(self.0.borrow(), |state| match state.as_ref().unwrap() {
            OpenGLContextState::Current { canvas, .. } => canvas,
        })
    }

    #[cfg(skia_backend_opengl)]
    pub fn glutin_context(&self) -> std::cell::Ref<glutin::context::PossiblyCurrentContext> {
        std::cell::Ref::map(self.0.borrow(), |state| match state.as_ref().unwrap() {
            OpenGLContextState::Current((gl_context, ..)) => gl_context,
            OpenGLContextState::NotCurrent(..) => {
                panic!("internal error: glutin_context() called without current context")
            }
        })
    }

    pub fn make_current(&self) {
        let mut ctx = self.0.borrow_mut();
        *ctx = Some(match ctx.take().unwrap() {
            #[cfg(not(target_arch = "wasm32"))]
            OpenGLContextState::NotCurrent((not_current_ctx, surface)) => {
                let current_ctx = not_current_ctx.make_current(&surface).unwrap();
                OpenGLContextState::Current((current_ctx, surface))
            }
            state @ OpenGLContextState::Current { .. } => state,
        });
    }

    pub fn make_not_current(&self) {
        #[cfg(not(target_arch = "wasm32"))]
        {
            let mut ctx = self.0.borrow_mut();
            *ctx = Some(match ctx.take().unwrap() {
                state @ OpenGLContextState::NotCurrent(_) => state,
                OpenGLContextState::Current((current_ctx_rc, surface)) => {
                    OpenGLContextState::NotCurrent({
                        (current_ctx_rc.make_not_current().unwrap(), surface)
                    })
                }
            });
        }
    }

    #[cfg(any(feature = "renderer-winit-femtovg", enable_skia_renderer))]
    pub fn with_current_context<T>(&self, cb: impl FnOnce(&Self) -> T) -> T {
        if matches!(self.0.borrow().as_ref().unwrap(), OpenGLContextState::Current { .. }) {
            cb(self)
        } else {
            self.make_current();
            let result = cb(self);
            self.make_not_current();
            result
        }
    }

    pub fn swap_buffers(&self) {
        #[cfg(not(target_arch = "wasm32"))]
        match &self.0.borrow().as_ref().unwrap() {
            OpenGLContextState::NotCurrent(_) => {}
            OpenGLContextState::Current((current_ctx, surface)) => {
                surface.swap_buffers(current_ctx).unwrap();
            }
        }
    }

    pub fn ensure_resized(&self, _window: &winit::window::Window) {
        #[cfg(not(target_arch = "wasm32"))]
        {
            let mut ctx = self.0.borrow_mut();
            *ctx = Some(match ctx.take().unwrap() {
                #[cfg(not(target_arch = "wasm32"))]
                OpenGLContextState::NotCurrent((not_current_ctx, surface)) => {
                    let current_ctx = not_current_ctx.make_current(&surface).unwrap();
                    let size = _window.inner_size();
                    surface.resize(
                        &current_ctx,
                        size.width.try_into().unwrap(),
                        size.height.try_into().unwrap(),
                    );
                    OpenGLContextState::NotCurrent((
                        current_ctx.make_not_current().unwrap(),
                        surface,
                    ))
                }
                OpenGLContextState::Current((current, surface)) => {
                    let size = _window.inner_size();
                    surface.resize(
                        &current,
                        size.width.try_into().unwrap(),
                        size.height.try_into().unwrap(),
                    );
                    OpenGLContextState::Current((current, surface))
                }
            });
        }
    }

    pub fn new_context(
        window_builder: winit::window::WindowBuilder,
        #[cfg(target_arch = "wasm32")] canvas_id: &str,
    ) -> (Self, Rc<winit::window::Window>) {
        #[cfg(not(target_arch = "wasm32"))]
        {
            let (maybe_window, config) = crate::event_loop::with_window_target(|event_loop| {
                glutin_winit::DisplayBuilder::new()
                    .with_preference(glutin_winit::ApiPrefence::PreferEgl)
                    .with_window_builder(Some(window_builder))
                    .build(
                        event_loop.event_loop_target(),
                        glutin::config::ConfigTemplateBuilder::new(),
                        |configs| {
                            configs
                                .reduce(|accum, config| {
                                    let transparency_check =
                                        config.supports_transparency().unwrap_or(false)
                                            & !accum.supports_transparency().unwrap_or(false);

                                    if transparency_check
                                        || config.num_samples() < accum.num_samples()
                                    {
                                        config
                                    } else {
                                        accum
                                    }
                                })
                                .unwrap()
                        },
                    )
                    .unwrap()
            });

            let window = maybe_window.unwrap();

            let gl_display = config.display();

            use raw_window_handle::HasRawWindowHandle;
            let gles_context_attributes = ContextAttributesBuilder::new()
                .with_context_api(ContextApi::Gles(Some(glutin::context::Version {
                    major: 2,
                    minor: 0,
                })))
                .build(Some(window.raw_window_handle()));

            let fallback_context_attributes =
                ContextAttributesBuilder::new().build(Some(window.raw_window_handle()));

            let not_current_gl_context = unsafe {
                gl_display
                    .create_context(&config, &gles_context_attributes)
                    .or_else(|_| gl_display.create_context(&config, &fallback_context_attributes))
                    .expect("failed to create context")
            };

            #[cfg(target_os = "macos")]
            {
                use cocoa::appkit::NSView;
                use winit::platform::macos::WindowExtMacOS;
                let ns_view = window.ns_view();
                let view_id: cocoa::base::id = ns_view as *const _ as *mut _;
                unsafe {
                    NSView::setLayerContentsPlacement(view_id, cocoa::appkit::NSViewLayerContentsPlacement::NSViewLayerContentsPlacementTopLeft)
                }
            }

            let (width, height): (u32, u32) = window.inner_size().into();
            let attrs = SurfaceAttributesBuilder::<WindowSurface>::new().build(
                window.raw_window_handle(),
                width.try_into().unwrap(),
                height.try_into().unwrap(),
            );

            let surface =
                unsafe { config.display().create_window_surface(&config, &attrs).unwrap() };

            (
                Self(RefCell::new(Some(OpenGLContextState::Current((
                    not_current_gl_context.make_current(&surface).unwrap(),
                    surface,
                ))))),
                Rc::new(window),
            )
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
                    let winit_window_weak = send_wrapper::SendWrapper::new(Rc::downgrade(&window));
                    i_slint_core::api::invoke_from_event_loop(move || {
                        if let Some(winit_window) = winit_window_weak.take().upgrade() {
                            winit_window.request_redraw();
                        }
                    })
                    .unwrap();
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

            (Self(RefCell::new(Some(OpenGLContextState::Current { canvas }))), window)
        }
    }

    // TODO: fix this interface to also take a ffi::CStr so that we can avoid the allocation. Problem: It's in our public api.
    #[cfg(not(target_arch = "wasm32"))]
    pub fn get_proc_address(&self, name: &str) -> *const std::ffi::c_void {
        match &self.0.borrow().as_ref().unwrap() {
            OpenGLContextState::NotCurrent(_) => std::ptr::null(),
            OpenGLContextState::Current((current_ctx, _)) => {
                current_ctx.display().get_proc_address(&std::ffi::CString::new(name).unwrap())
            }
        }
    }
}
