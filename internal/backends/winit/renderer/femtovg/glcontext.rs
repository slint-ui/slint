// Copyright © SixtyFPS GmbH <info@slint-ui.com>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-commercial

use std::cell::RefCell;

#[cfg(not(target_arch = "wasm32"))]
use glutin::{
    context::{ContextApi, ContextAttributesBuilder},
    display::GetGlDisplay,
    prelude::*,
    surface::{SurfaceAttributesBuilder, WindowSurface},
};
use i_slint_core::api::PhysicalSize;

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
    pub fn html_canvas_element(&self) -> web_sys::HtmlCanvasElement {
        match self.0.borrow().as_ref().unwrap() {
            OpenGLContextState::Current { canvas, .. } => canvas.clone(),
        }
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

    pub fn ensure_resized(&self, _size: PhysicalSize) {
        #[cfg(not(target_arch = "wasm32"))]
        {
            let mut ctx = self.0.borrow_mut();
            *ctx = Some(match ctx.take().unwrap() {
                #[cfg(not(target_arch = "wasm32"))]
                OpenGLContextState::NotCurrent((not_current_ctx, surface)) => {
                    let current_ctx = not_current_ctx.make_current(&surface).unwrap();
                    surface.resize(
                        &current_ctx,
                        _size.width.try_into().unwrap(),
                        _size.height.try_into().unwrap(),
                    );
                    OpenGLContextState::NotCurrent((
                        current_ctx.make_not_current().unwrap(),
                        surface,
                    ))
                }
                OpenGLContextState::Current((current, surface)) => {
                    surface.resize(
                        &current,
                        _size.width.try_into().unwrap(),
                        _size.height.try_into().unwrap(),
                    );
                    OpenGLContextState::Current((current, surface))
                }
            });
        }
    }

    pub fn new_context(
        _window: &dyn raw_window_handle::HasRawWindowHandle,
        _display: &dyn raw_window_handle::HasRawDisplayHandle,
        _size: PhysicalSize,
        #[cfg(target_arch = "wasm32")] canvas_id: &str,
    ) -> Self {
        #[cfg(not(target_arch = "wasm32"))]
        {
            cfg_if::cfg_if! {
                if #[cfg(target_os = "macos")] {
                    let pref = glutin::display::DisplayApiPreference::Cgl;
                } else if #[cfg(not(target_family = "windows"))] {
                    let pref = glutin::display::DisplayApiPreference::Egl;
                } else {
                    let pref = glutin::display::DisplayApiPreference::EglThenWgl(Some(_window.raw_window_handle()));
                }
            }

            let gl_display = unsafe {
                glutin::display::Display::new(_display.raw_display_handle(), pref).unwrap()
            };

            let config_template = glutin::config::ConfigTemplateBuilder::new()
                .compatible_with_native_window(_window.raw_window_handle())
                .build();

            let config = unsafe {
                gl_display
                    .find_configs(config_template)
                    .unwrap()
                    .reduce(|accum, config| {
                        let transparency_check = config.supports_transparency().unwrap_or(false)
                            & !accum.supports_transparency().unwrap_or(false);

                        if transparency_check || config.num_samples() < accum.num_samples() {
                            config
                        } else {
                            accum
                        }
                    })
                    .unwrap()
            };

            let gles_context_attributes = ContextAttributesBuilder::new()
                .with_context_api(ContextApi::Gles(Some(glutin::context::Version {
                    major: 2,
                    minor: 0,
                })))
                .build(Some(_window.raw_window_handle()));

            let fallback_context_attributes =
                ContextAttributesBuilder::new().build(Some(_window.raw_window_handle()));

            let not_current_gl_context = unsafe {
                gl_display
                    .create_context(&config, &gles_context_attributes)
                    .or_else(|_| gl_display.create_context(&config, &fallback_context_attributes))
                    .expect("failed to create context")
            };

            let attrs = SurfaceAttributesBuilder::<WindowSurface>::new().build(
                _window.raw_window_handle(),
                _size.width.try_into().unwrap(),
                _size.height.try_into().unwrap(),
            );

            let surface =
                unsafe { config.display().create_window_surface(&config, &attrs).unwrap() };

            #[cfg(target_os = "macos")]
            if let raw_window_handle::RawWindowHandle::AppKit(
                raw_window_handle::AppKitWindowHandle { ns_view, .. },
            ) = _window.raw_window_handle()
            {
                use cocoa::appkit::NSView;
                let view_id: cocoa::base::id = ns_view as *const _ as *mut _;
                unsafe {
                    NSView::setLayerContentsPlacement(view_id, cocoa::appkit::NSViewLayerContentsPlacement::NSViewLayerContentsPlacementTopLeft)
                }
            }

            Self(RefCell::new(Some(OpenGLContextState::Current((
                not_current_gl_context.make_current(&surface).unwrap(),
                surface,
            )))))
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

            Self(RefCell::new(Some(OpenGLContextState::Current { canvas })))
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
