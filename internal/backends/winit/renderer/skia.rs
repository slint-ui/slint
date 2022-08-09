// Copyright © SixtyFPS GmbH <info@slint-ui.com>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-commercial

use std::{
    cell::RefCell,
    rc::{Rc, Weak},
};

use i_slint_core::{
    api::{GraphicsAPI, RenderingNotifier, RenderingState, SetRenderingNotifierError},
    graphics::rendering_metrics_collector::RenderingMetricsCollector,
    item_rendering::ItemCache,
};

use crate::WindowSystemName;

mod itemrenderer;
mod textlayout;

#[cfg(any(not(target_os = "macos"), feature = "renderer-skia-opengl"))]
mod opengl_surface;

#[cfg(target_os = "macos")]
mod metal_surface;

cfg_if::cfg_if! {
    if #[cfg(all(target_os = "macos", not(feature = "renderer-skia-opengl")))] {
        type DefaultSurface = metal_surface::MetalSurface;
    } else {
        type DefaultSurface = opengl_surface::OpenGLSurface;
    }
}

pub struct SkiaRenderer {
    window_weak: Weak<i_slint_core::window::WindowInner>,
    rendering_notifier: RefCell<Option<Box<dyn RenderingNotifier>>>,
}

impl super::WinitCompatibleRenderer for SkiaRenderer {
    type Canvas = SkiaCanvas<DefaultSurface>;

    fn new(window_weak: &std::rc::Weak<i_slint_core::window::WindowInner>) -> Self {
        Self { window_weak: window_weak.clone(), rendering_notifier: Default::default() }
    }

    fn create_canvas(&self, window_builder: winit::window::WindowBuilder) -> Self::Canvas {
        let surface = DefaultSurface::new(window_builder);

        let rendering_metrics_collector = RenderingMetricsCollector::new(
            self.window_weak.clone(),
            &format!(
                "Skia renderer (windowing system: {}; skia backend {})",
                surface.with_window_handle(|winit_window| winit_window.winsys_name()),
                surface.name()
            ),
        );

        let canvas =
            SkiaCanvas { image_cache: Default::default(), surface, rendering_metrics_collector };

        if let Some(callback) = self.rendering_notifier.borrow_mut().as_mut() {
            canvas.with_graphics_api(|api| callback.notify(RenderingState::RenderingSetup, &api))
        }

        canvas
    }

    fn release_canvas(&self, canvas: Self::Canvas) {
        canvas.surface.with_active_surface(|| {
            if let Some(callback) = self.rendering_notifier.borrow_mut().as_mut() {
                canvas.with_graphics_api(|api| {
                    callback.notify(RenderingState::RenderingTeardown, &api)
                })
            }
        });
    }

    fn render(&self, canvas: &Self::Canvas) {
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

                if let Some(callback) = self.rendering_notifier.borrow_mut().as_mut() {
                    // For the BeforeRendering rendering notifier callback it's important that this happens *after* clearing
                    // the back buffer, in order to allow the callback to provide its own rendering of the background.
                    // Skia's clear() will merely schedule a clear call, so flush right away to make it immediate.
                    gr_context.borrow_mut().flush(None);

                    canvas.with_graphics_api(|api| {
                        callback.notify(RenderingState::BeforeRendering, &api)
                    })
                }

                let mut item_renderer =
                    itemrenderer::SkiaRenderer::new(skia_canvas, &window, &canvas.image_cache);

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

            if let Some(callback) = self.rendering_notifier.borrow_mut().as_mut() {
                canvas
                    .with_graphics_api(|api| callback.notify(RenderingState::AfterRendering, &api))
            }
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

    fn set_rendering_notifier(
        &self,
        callback: Box<dyn RenderingNotifier>,
    ) -> std::result::Result<(), SetRenderingNotifierError> {
        if !DefaultSurface::SUPPORTS_GRAPHICS_API {
            return Err(SetRenderingNotifierError::Unsupported);
        }
        let mut notifier = self.rendering_notifier.borrow_mut();
        if notifier.replace(callback).is_some() {
            Err(SetRenderingNotifierError::AlreadySet)
        } else {
            Ok(())
        }
    }
}

pub trait Surface {
    const SUPPORTS_GRAPHICS_API: bool;
    fn new(window_builder: winit::window::WindowBuilder) -> Self;
    fn name(&self) -> &'static str;
    fn with_graphics_api(&self, callback: impl FnOnce(GraphicsAPI<'_>));
    fn with_window_handle<T>(&self, callback: impl FnOnce(&winit::window::Window) -> T) -> T;
    fn with_active_surface<T>(&self, callback: impl FnOnce() -> T) -> T {
        callback()
    }
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
    fn component_destroyed(&self, component: i_slint_core::component::ComponentRef) {
        self.image_cache.component_destroyed(component)
    }

    fn with_window_handle<T>(&self, callback: impl FnOnce(&winit::window::Window) -> T) -> T {
        self.surface.with_window_handle(callback)
    }

    fn resize_event(&self) {
        self.surface.resize_event()
    }
}

impl<SurfaceType: Surface> SkiaCanvas<SurfaceType> {
    fn with_graphics_api(&self, callback: impl FnOnce(GraphicsAPI<'_>)) {
        self.surface.with_graphics_api(callback)
    }
}
