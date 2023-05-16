// Copyright © SixtyFPS GmbH <info@slint-ui.com>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-1.0 OR LicenseRef-Slint-commercial

use std::cell::RefCell;
use std::pin::Pin;
use std::rc::Weak;

use i_slint_core::api::{
    GraphicsAPI, PhysicalSize as PhysicalWindowSize, RenderingNotifier, RenderingState,
    SetRenderingNotifierError,
};
use i_slint_core::lengths::{LogicalLength, LogicalPoint, LogicalRect, LogicalSize, ScaleFactor};
use i_slint_core::platform::PlatformError;
use i_slint_core::renderer::Renderer;
use i_slint_core::window::WindowAdapter;
use i_slint_renderer_femtovg::FemtoVGRenderer;

mod glcontext;

pub struct GlutinFemtoVGRenderer {
    rendering_notifier: RefCell<Option<Box<dyn RenderingNotifier>>>,
    renderer: FemtoVGRenderer,
    // Last field, so that it's dropped last and context exists and is current when destroying the FemtoVG canvas
    opengl_context: glcontext::OpenGLContext,
}

impl GlutinFemtoVGRenderer {
    #[cfg(not(target_arch = "wasm32"))]
    fn with_graphics_api(
        opengl_context: &glcontext::OpenGLContext,
        callback: impl FnOnce(i_slint_core::api::GraphicsAPI<'_>),
    ) -> Result<(), PlatformError> {
        opengl_context.ensure_current()?;
        let api = GraphicsAPI::NativeOpenGL {
            get_proc_address: &|name| opengl_context.get_proc_address(name),
        };
        callback(api);
        Ok(())
    }

    #[cfg(target_arch = "wasm32")]
    fn with_graphics_api(
        opengl_context: &glcontext::OpenGLContext,
        callback: impl FnOnce(i_slint_core::api::GraphicsAPI<'_>),
    ) -> Result<(), PlatformError> {
        let canvas_element_id = opengl_context.html_canvas_element().id();
        let api = GraphicsAPI::WebGL {
            canvas_element_id: canvas_element_id.as_str(),
            context_type: "webgl",
        };
        callback(api);
        Ok(())
    }
}

impl super::WinitCompatibleRenderer for GlutinFemtoVGRenderer {
    fn new(
        window_adapter_weak: &Weak<dyn WindowAdapter>,
        window_builder: winit::window::WindowBuilder,
        #[cfg(target_arch = "wasm32")] canvas_id: &str,
    ) -> Result<(Self, winit::window::Window), PlatformError> {
        let (winit_window, opengl_context) = crate::event_loop::with_window_target(|event_loop| {
            glcontext::OpenGLContext::new_context(
                window_builder,
                event_loop.event_loop_target(),
                #[cfg(target_arch = "wasm32")]
                canvas_id,
            )
        })?;

        let renderer = FemtoVGRenderer::new(
            window_adapter_weak,
            #[cfg(not(target_arch = "wasm32"))]
            |name| opengl_context.get_proc_address(name) as *const _,
            #[cfg(target_arch = "wasm32")]
            &opengl_context.html_canvas_element(),
        )?;

        Ok((
            Self { rendering_notifier: Default::default(), renderer, opengl_context },
            winit_window,
        ))
    }

    fn show(&self) -> Result<(), PlatformError> {
        self.opengl_context.ensure_current()?;
        self.renderer.show();

        if let Some(callback) = self.rendering_notifier.borrow_mut().as_mut() {
            Self::with_graphics_api(&self.opengl_context, |api| {
                callback.notify(RenderingState::RenderingSetup, &api)
            })?;
        }

        Ok(())
    }

    fn hide(&self) -> Result<(), PlatformError> {
        self.opengl_context.ensure_current()?;
        if let Some(callback) = self.rendering_notifier.borrow_mut().as_mut() {
            Self::with_graphics_api(&self.opengl_context, |api| {
                callback.notify(RenderingState::RenderingTeardown, &api)
            })?;
        }
        self.renderer.hide();

        Ok(())
    }

    fn render(&self, size: PhysicalWindowSize) -> Result<(), PlatformError> {
        self.opengl_context.ensure_current()?;

        self.renderer.render(
            size,
            self.rendering_notifier.borrow_mut().as_mut().map(|notifier_fn| {
                || {
                    Self::with_graphics_api(&self.opengl_context, |api| {
                        notifier_fn.notify(RenderingState::BeforeRendering, &api)
                    })
                }
            }),
        )?;

        if let Some(callback) = self.rendering_notifier.borrow_mut().as_mut() {
            Self::with_graphics_api(&self.opengl_context, |api| {
                callback.notify(RenderingState::AfterRendering, &api)
            })?;
        }

        self.opengl_context.swap_buffers()
    }

    fn as_core_renderer(&self) -> &dyn Renderer {
        self
    }

    fn resize_event(&self, size: PhysicalWindowSize) -> Result<(), PlatformError> {
        self.opengl_context.ensure_resized(size)
    }

    #[cfg(target_arch = "wasm32")]
    fn html_canvas_element(&self) -> web_sys::HtmlCanvasElement {
        self.opengl_context.html_canvas_element()
    }
}

impl Renderer for GlutinFemtoVGRenderer {
    fn text_size(
        &self,
        font_request: i_slint_core::graphics::FontRequest,
        text: &str,
        max_width: Option<LogicalLength>,
        scale_factor: ScaleFactor,
    ) -> LogicalSize {
        self.renderer.text_size(font_request, text, max_width, scale_factor)
    }

    fn text_input_byte_offset_for_position(
        &self,
        text_input: Pin<&i_slint_core::items::TextInput>,
        pos: LogicalPoint,
    ) -> usize {
        self.renderer.text_input_byte_offset_for_position(text_input, pos)
    }

    fn text_input_cursor_rect_for_byte_offset(
        &self,
        text_input: Pin<&i_slint_core::items::TextInput>,
        byte_offset: usize,
    ) -> LogicalRect {
        self.renderer.text_input_cursor_rect_for_byte_offset(text_input, byte_offset)
    }

    fn register_font_from_memory(
        &self,
        data: &'static [u8],
    ) -> Result<(), Box<dyn std::error::Error>> {
        self.renderer.register_font_from_memory(data)
    }

    fn register_font_from_path(
        &self,
        path: &std::path::Path,
    ) -> Result<(), Box<dyn std::error::Error>> {
        self.renderer.register_font_from_path(path)
    }

    fn set_rendering_notifier(
        &self,
        callback: Box<dyn RenderingNotifier>,
    ) -> std::result::Result<(), SetRenderingNotifierError> {
        let mut notifier = self.rendering_notifier.borrow_mut();
        if notifier.replace(callback).is_some() {
            Err(SetRenderingNotifierError::AlreadySet)
        } else {
            Ok(())
        }
    }

    fn default_font_size(&self) -> LogicalLength {
        self.renderer.default_font_size()
    }

    fn free_graphics_resources(
        &self,
        component: i_slint_core::component::ComponentRef,
        _items: &mut dyn Iterator<Item = Pin<i_slint_core::items::ItemRef<'_>>>,
    ) -> Result<(), PlatformError> {
        self.opengl_context.ensure_current()?;
        self.renderer.free_graphics_resources(component, _items)
    }
}

impl Drop for GlutinFemtoVGRenderer {
    fn drop(&mut self) {
        // Ensure the context is current before the renderer is destroyed
        self.opengl_context.ensure_current().ok();
    }
}
