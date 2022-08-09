// Copyright © SixtyFPS GmbH <info@slint-ui.com>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-commercial

use std::{
    cell::RefCell,
    rc::{Rc, Weak},
};

use i_slint_core::{
    api::GraphicsAPI, graphics::rendering_metrics_collector::RenderingMetricsCollector,
    item_rendering::ItemCache,
};

use crate::WindowSystemName;

mod itemrenderer;
mod textlayout;

mod opengl_surface;

pub struct SkiaRenderer {
    window_weak: Weak<i_slint_core::window::WindowInner>,
}

impl super::WinitCompatibleRenderer for SkiaRenderer {
    type Canvas = SkiaCanvas<opengl_surface::OpenGLSurface>;

    fn new(window_weak: &std::rc::Weak<i_slint_core::window::WindowInner>) -> Self {
        Self { window_weak: window_weak.clone() }
    }

    fn create_canvas(&self, window_builder: winit::window::WindowBuilder) -> Self::Canvas {
        let surface = opengl_surface::OpenGLSurface::new(window_builder);

        let rendering_metrics_collector = RenderingMetricsCollector::new(
            self.window_weak.clone(),
            &format!(
                "Skia renderer (windowing system: {})",
                surface.with_window_handle(|winit_window| winit_window.winsys_name())
            ),
        );

        SkiaCanvas { image_cache: Default::default(), surface, rendering_metrics_collector }
    }

    fn render(
        &self,
        canvas: &Self::Canvas,
        before_rendering_callback: impl FnOnce(),
        after_rendering_callback: impl FnOnce(),
    ) {
        let window = match self.window_weak.upgrade() {
            Some(window) => window,
            None => return,
        };

        canvas.surface.render(|skia_canvas, gr_context| {
            window.clone().draw_contents(|components| {
                if let Some(window_item) = window.window_item() {
                    skia_canvas
                        .clear(itemrenderer::to_skia_color(&window_item.as_pin_ref().background()));
                }

                gr_context.borrow_mut().flush(None);

                let mut item_renderer =
                    itemrenderer::SkiaRenderer::new(skia_canvas, &window, &canvas.image_cache);

                before_rendering_callback();

                for (component, origin) in components {
                    i_slint_core::item_rendering::render_component_items(
                        component,
                        &mut item_renderer,
                        *origin,
                    );
                }

                if let Some(collector) = &canvas.rendering_metrics_collector {
                    collector.measure_frame_rendered(&mut item_renderer);
                }

                drop(item_renderer);
                gr_context.borrow_mut().flush(None);
            });

            after_rendering_callback();
        });
    }
}

impl i_slint_core::renderer::Renderer for SkiaRenderer {
    fn text_size(
        &self,
        font_request: i_slint_core::graphics::FontRequest,
        text: &str,
        max_width: Option<i_slint_core::Coord>,
        scale_factor: f32,
    ) -> i_slint_core::graphics::Size {
        let layout = textlayout::create_layout(
            font_request,
            scale_factor,
            text,
            None,
            max_width.map(|w| w * scale_factor),
            Default::default(),
            Default::default(),
        );

        [layout.max_intrinsic_width().ceil() / scale_factor, layout.height().ceil() / scale_factor]
            .into()
    }

    fn text_input_byte_offset_for_position(
        &self,
        _text_input: std::pin::Pin<&i_slint_core::items::TextInput>,
        _pos: i_slint_core::graphics::Point,
    ) -> usize {
        todo!()
    }

    fn text_input_cursor_rect_for_byte_offset(
        &self,
        _text_input: std::pin::Pin<&i_slint_core::items::TextInput>,
        _byte_offset: usize,
    ) -> i_slint_core::graphics::Rect {
        todo!()
    }

    fn register_font_from_memory(
        &self,
        data: &'static [u8],
    ) -> Result<(), Box<dyn std::error::Error>> {
        textlayout::register_font_from_memory(data)
    }

    fn register_font_from_path(
        &self,
        path: &std::path::Path,
    ) -> Result<(), Box<dyn std::error::Error>> {
        textlayout::register_font_from_path(path)
    }
}

pub trait Surface {
    fn new(window_builder: winit::window::WindowBuilder) -> Self;
    fn with_graphics_api(&self, callback: impl FnOnce(GraphicsAPI<'_>));
    fn with_window_handle<T>(&self, callback: impl FnOnce(&winit::window::Window) -> T) -> T;
    fn render(
        &self,
        callback: impl FnOnce(&mut skia_safe::Canvas, &RefCell<skia_safe::gpu::DirectContext>),
    );
    fn resize_event(&self);
}

pub struct SkiaCanvas<SurfaceType: Surface> {
    image_cache: ItemCache<Option<skia_safe::Image>>,
    rendering_metrics_collector: Option<Rc<RenderingMetricsCollector>>,
    surface: SurfaceType,
}

impl<SurfaceType: Surface> super::WinitCompatibleCanvas for SkiaCanvas<SurfaceType> {
    fn release_graphics_resources(&self) {
        self.image_cache.clear_all();
    }

    fn component_destroyed(&self, component: i_slint_core::component::ComponentRef) {
        self.image_cache.component_destroyed(component)
    }

    fn with_graphics_api(&self, callback: impl FnOnce(GraphicsAPI<'_>)) {
        self.surface.with_graphics_api(callback)
    }

    fn with_window_handle<T>(&self, callback: impl FnOnce(&winit::window::Window) -> T) -> T {
        self.surface.with_window_handle(callback)
    }

    fn resize_event(&self) {
        self.surface.resize_event()
    }
}
