// Copyright © SixtyFPS GmbH <info@slint-ui.com>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-commercial

#[cfg(not(target_arch = "wasm32"))]
use glutin::{
    context::{ContextApi, ContextAttributesBuilder},
    display::GetGlDisplay,
    prelude::*,
    surface::{SurfaceAttributesBuilder, WindowSurface},
};
use i_slint_core::api::PhysicalSize;

pub struct OpenGLContext {
    #[cfg(not(target_arch = "wasm32"))]
    context: glutin::context::PossiblyCurrentContext,
    #[cfg(not(target_arch = "wasm32"))]
    surface: glutin::surface::Surface<glutin::surface::WindowSurface>,
    #[cfg(target_arch = "wasm32")]
    canvas: web_sys::HtmlCanvasElement,
}

impl OpenGLContext {
    #[cfg(target_arch = "wasm32")]
    pub fn html_canvas_element(&self) -> web_sys::HtmlCanvasElement {
        self.canvas.clone()
    }

    pub fn ensure_current(&self) {
        #[cfg(not(target_arch = "wasm32"))]
        if !self.context.is_current() {
            self.context.make_current(&self.surface).unwrap();
        }
    }

    pub fn swap_buffers(&self) {
        #[cfg(not(target_arch = "wasm32"))]
        self.surface.swap_buffers(&self.context).unwrap();
    }

    pub fn ensure_resized(&self, _size: PhysicalSize) {
        #[cfg(not(target_arch = "wasm32"))]
        {
            self.ensure_current();
            self.surface.resize(
                &self.context,
                _size.width.try_into().unwrap(),
                _size.height.try_into().unwrap(),
            );
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
                    let prefs = [glutin::display::DisplayApiPreference::Cgl];
                } else if #[cfg(all(feature = "x11", not(target_family = "windows")))] {
                    let prefs = [glutin::display::DisplayApiPreference::Egl, glutin::display::DisplayApiPreference::Glx(Box::new(winit::platform::x11::register_xlib_error_hook))];
                } else if #[cfg(not(target_family = "windows"))] {
                    let prefs = [glutin::display::DisplayApiPreference::Egl];
                } else {
                    let prefs = [glutin::display::DisplayApiPreference::EglThenWgl(Some(_window.raw_window_handle()))];
                }
            }

            let try_create_surface = |display_api_preference| -> glutin::error::Result<(_, _)> {
                let gl_display = unsafe {
                    glutin::display::Display::new(
                        _display.raw_display_handle(),
                        display_api_preference,
                    )?
                };

                let config_template = glutin::config::ConfigTemplateBuilder::new()
                    .compatible_with_native_window(_window.raw_window_handle())
                    .build();

                let config = unsafe {
                    gl_display
                        .find_configs(config_template)
                        .unwrap()
                        .reduce(|accum, config| {
                            let transparency_check =
                                config.supports_transparency().unwrap_or(false)
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
                    gl_display.create_context(&config, &gles_context_attributes).or_else(|_| {
                        gl_display.create_context(&config, &fallback_context_attributes)
                    })?
                };

                let attrs = SurfaceAttributesBuilder::<WindowSurface>::new().build(
                    _window.raw_window_handle(),
                    _size.width.try_into().unwrap(),
                    _size.height.try_into().unwrap(),
                );

                let surface = unsafe { config.display().create_window_surface(&config, &attrs)? };

                Ok((surface, not_current_gl_context))
            };

            let num_prefs = prefs.len();
            let (surface, not_current_gl_context) = prefs
                .into_iter()
                .enumerate()
                .find_map(|(i, pref)| {
                    let is_last = i == num_prefs - 1;

                    match try_create_surface(pref) {
                        Ok(result) => Some(result),
                        Err(glutin_error) => {
                            if is_last {
                                panic!("Glutin error creating GL surface: {}", glutin_error);
                            }
                            None
                        }
                    }
                })
                .unwrap();

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

            Self { context: not_current_gl_context.make_current(&surface).unwrap(), surface }
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

            Self { canvas }
        }
    }

    // TODO: fix this interface to also take a ffi::CStr so that we can avoid the allocation. Problem: It's in our public api.
    #[cfg(not(target_arch = "wasm32"))]
    pub fn get_proc_address(&self, name: &str) -> *const std::ffi::c_void {
        self.context.display().get_proc_address(&std::ffi::CString::new(name).unwrap())
    }
}
