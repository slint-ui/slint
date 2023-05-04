// Copyright © SixtyFPS GmbH <info@slint-ui.com>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-commercial

#![doc = include_str!("README.md")]
#![doc(html_logo_url = "https://slint-ui.com/logo/slint-logo-square-light.svg")]

use std::cell::RefCell;
use std::rc::{Rc, Weak};

use i_slint_core::api::{
    GraphicsAPI, PhysicalSize as PhysicalWindowSize, RenderingNotifier, RenderingState,
    SetRenderingNotifierError,
};
use i_slint_core::graphics::euclid::{self, Vector2D};
use i_slint_core::graphics::rendering_metrics_collector::RenderingMetricsCollector;
use i_slint_core::item_rendering::ItemCache;
use i_slint_core::lengths::{
    LogicalLength, LogicalPoint, LogicalRect, LogicalSize, PhysicalPx, ScaleFactor,
};
use i_slint_core::platform::PlatformError;
use i_slint_core::window::{WindowAdapter, WindowInner};
use i_slint_core::Brush;

type PhysicalLength = euclid::Length<f32, PhysicalPx>;
type PhysicalRect = euclid::Rect<f32, PhysicalPx>;
type PhysicalSize = euclid::Size2D<f32, PhysicalPx>;
type PhysicalPoint = euclid::Point2D<f32, PhysicalPx>;

mod cached_image;
mod itemrenderer;
mod textlayout;

#[cfg(target_os = "macos")]
mod metal_surface;

#[cfg(target_family = "windows")]
mod d3d_surface;

cfg_if::cfg_if! {
    if #[cfg(skia_backend_opengl)] {
        mod opengl_surface;
        type DefaultSurface = opengl_surface::OpenGLSurface;
    } else if #[cfg(skia_backend_metal)] {
        type DefaultSurface = metal_surface::MetalSurface;
    } else if #[cfg(skia_backend_d3d)] {
        type DefaultSurface = d3d_surface::D3DSurface;
    }
}

/// Use the SkiaRenderer when implementing a custom Slint platform where you deliver events to
/// Slint and want the scene to be rendered using Skia as underlying graphics library.
pub struct SkiaRenderer<NativeWindowWrapper> {
    window_adapter_weak: Weak<dyn WindowAdapter>,
    rendering_notifier: RefCell<Option<Box<dyn RenderingNotifier>>>,
    canvas: RefCell<Option<SkiaCanvas<DefaultSurface, NativeWindowWrapper>>>,
}

impl<
        NativeWindowWrapper: raw_window_handle::HasRawWindowHandle + raw_window_handle::HasRawDisplayHandle,
    > SkiaRenderer<NativeWindowWrapper>
{
    /// Creates a new renderer is associated with the provided window adapter.
    pub fn new(window_adapter_weak: Weak<dyn WindowAdapter>) -> Self {
        Self {
            window_adapter_weak,
            rendering_notifier: Default::default(),
            canvas: Default::default(),
        }
    }

    /// Use the provided window and display for rendering the Slint scene in future calls to [`Self::render()`].
    /// The size must be identical to the size of the window in physical pixels that is providing the window handle.
    pub fn show(
        &self,
        native_window: NativeWindowWrapper,
        size: PhysicalWindowSize,
    ) -> Result<(), PlatformError> {
        let surface = DefaultSurface::new(&native_window, &native_window, size)?;

        let rendering_metrics_collector = RenderingMetricsCollector::new(
            self.window_adapter_weak.clone(),
            &format!(
                "Skia renderer (skia backend {}; surface: {} bpp)",
                surface.name(),
                surface.bits_per_pixel()?
            ),
        );

        let canvas = SkiaCanvas {
            image_cache: Default::default(),
            path_cache: Default::default(),
            surface,
            rendering_metrics_collector,
            native_window,
        };

        if let Some(callback) = self.rendering_notifier.borrow_mut().as_mut() {
            canvas.with_graphics_api(|api| callback.notify(RenderingState::RenderingSetup, &api))
        }

        *self.canvas.borrow_mut() = Some(canvas);

        Ok(())
    }

    /// Release any graphics resources and disconnect the rendere from a window that it was previously associated when when
    /// calling [`Self::show()]`.
    pub fn hide(&self) -> Result<(), i_slint_core::platform::PlatformError> {
        if let Some(canvas) = self.canvas.borrow_mut().take() {
            canvas.surface.with_active_surface(|| {
                if let Some(callback) = self.rendering_notifier.borrow_mut().as_mut() {
                    canvas.with_graphics_api(|api| {
                        callback.notify(RenderingState::RenderingTeardown, &api)
                    })
                }
            })?;
        }
        Ok(())
    }

    /// Render the scene in the previously associated window. The size parameter must match the size of the window.
    pub fn render(
        &self,
        size: PhysicalWindowSize,
    ) -> Result<(), i_slint_core::platform::PlatformError> {
        let canvas = if self.canvas.borrow().is_some() {
            std::cell::Ref::map(self.canvas.borrow(), |canvas_opt| canvas_opt.as_ref().unwrap())
        } else {
            return Err(format!("Skia renderer: render() called before show()").into());
        };

        let window_adapter = self.window_adapter_weak.upgrade().unwrap();
        let window_inner = WindowInner::from_pub(window_adapter.window());

        canvas.surface.render(size, |skia_canvas, gr_context| {
            window_inner.draw_contents(|components| {
                let window_background_brush =
                    window_inner.window_item().map(|w| w.as_pin_ref().background());

                // Clear with window background if it is a solid color otherwise it will drawn as gradient
                if let Some(Brush::SolidColor(clear_color)) = window_background_brush {
                    skia_canvas.clear(itemrenderer::to_skia_color(&clear_color));
                }

                if let Some(callback) = self.rendering_notifier.borrow_mut().as_mut() {
                    // For the BeforeRendering rendering notifier callback it's important that this happens *after* clearing
                    // the back buffer, in order to allow the callback to provide its own rendering of the background.
                    // Skia's clear() will merely schedule a clear call, so flush right away to make it immediate.
                    gr_context.flush(None);

                    canvas.with_graphics_api(|api| {
                        callback.notify(RenderingState::BeforeRendering, &api)
                    })
                }

                let mut box_shadow_cache = Default::default();

                let window_adapter = self.window_adapter_weak.upgrade().unwrap();

                let mut item_renderer = itemrenderer::SkiaRenderer::new(
                    skia_canvas,
                    window_adapter.window(),
                    &canvas.image_cache,
                    &canvas.path_cache,
                    &mut box_shadow_cache,
                );

                // Draws the window background as gradient
                match window_background_brush {
                    Some(Brush::SolidColor(..)) | None => {}
                    Some(brush @ _) => {
                        item_renderer.draw_rect(
                            i_slint_core::lengths::logical_size_from_api(
                                size.to_logical(window_inner.scale_factor()),
                            ),
                            brush,
                        );
                    }
                }

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
                gr_context.flush(None);
            });

            if let Some(callback) = self.rendering_notifier.borrow_mut().as_mut() {
                canvas
                    .with_graphics_api(|api| callback.notify(RenderingState::AfterRendering, &api))
            }
        })
    }

    /// Call this when you receive a notification from the windowing system that the size of the window has changed.
    pub fn resize_event(
        &self,
        size: PhysicalWindowSize,
    ) -> Result<(), i_slint_core::platform::PlatformError> {
        let canvas = if self.canvas.borrow().is_some() {
            std::cell::Ref::map(self.canvas.borrow(), |canvas_opt| canvas_opt.as_ref().unwrap())
        } else {
            return Ok(());
        };

        canvas.surface.resize_event(size)
    }
}

impl<
        NativeWindowWrapper: raw_window_handle::HasRawWindowHandle + raw_window_handle::HasRawDisplayHandle + Clone,
    > SkiaRenderer<NativeWindowWrapper>
{
    pub fn window(&self) -> Option<NativeWindowWrapper> {
        self.canvas.borrow().as_ref().map(|canvas| canvas.native_window.clone())
    }
}

impl<NativeWindowWrapper> i_slint_core::renderer::Renderer for SkiaRenderer<NativeWindowWrapper> {
    fn text_size(
        &self,
        font_request: i_slint_core::graphics::FontRequest,
        text: &str,
        max_width: Option<LogicalLength>,
        scale_factor: ScaleFactor,
    ) -> LogicalSize {
        let (layout, _) = textlayout::create_layout(
            font_request,
            scale_factor,
            text,
            None,
            max_width.map(|w| w * scale_factor),
            Default::default(),
            Default::default(),
            Default::default(),
            Default::default(),
            None,
        );

        PhysicalSize::new(layout.max_intrinsic_width().ceil(), layout.height().ceil())
            / scale_factor
    }

    fn text_input_byte_offset_for_position(
        &self,
        text_input: std::pin::Pin<&i_slint_core::items::TextInput>,
        pos: LogicalPoint,
    ) -> usize {
        let window_adapter = match self.window_adapter_weak.upgrade() {
            Some(window) => window,
            None => return 0,
        };

        let window = WindowInner::from_pub(window_adapter.window());

        let scale_factor = ScaleFactor::new(window.scale_factor());

        let max_width = text_input.width() * scale_factor;
        let max_height = text_input.height() * scale_factor;
        let pos = pos * scale_factor;

        if max_width.get() <= 0. || max_height.get() <= 0. {
            return 0;
        }

        let visual_representation = text_input.visual_representation(None);

        let font_request = text_input.font_request(&window_adapter);

        let (layout, layout_top_left) = textlayout::create_layout(
            font_request,
            scale_factor,
            &visual_representation.text,
            None,
            Some(max_width),
            max_height,
            text_input.horizontal_alignment(),
            text_input.vertical_alignment(),
            i_slint_core::items::TextOverflow::Clip,
            None,
        );

        let utf16_index =
            layout.get_glyph_position_at_coordinate((pos.x, pos.y - layout_top_left.y)).position;
        let mut utf16_count = 0;
        let byte_offset = visual_representation
            .text
            .char_indices()
            .find(|(_, x)| {
                let r = utf16_count >= utf16_index;
                utf16_count += x.len_utf16() as i32;
                r
            })
            .unwrap_or((visual_representation.text.len(), '\0'))
            .0;

        visual_representation.map_byte_offset_from_byte_offset_in_visual_text(byte_offset)
    }

    fn text_input_cursor_rect_for_byte_offset(
        &self,
        text_input: std::pin::Pin<&i_slint_core::items::TextInput>,
        byte_offset: usize,
    ) -> LogicalRect {
        let window_adapter = match self.window_adapter_weak.upgrade() {
            Some(window) => window,
            None => return Default::default(),
        };

        let window = WindowInner::from_pub(window_adapter.window());

        let scale_factor = ScaleFactor::new(window.scale_factor());

        let max_width = text_input.width() * scale_factor;
        let max_height = text_input.height() * scale_factor;

        if max_width.get() <= 0. || max_height.get() <= 0. {
            return Default::default();
        }

        let string = text_input.text();
        let string = string.as_str();
        let font_request = text_input.font_request(&window_adapter);

        let (layout, layout_top_left) = textlayout::create_layout(
            font_request,
            scale_factor,
            string,
            None,
            Some(max_width),
            max_height,
            text_input.horizontal_alignment(),
            text_input.vertical_alignment(),
            i_slint_core::items::TextOverflow::Clip,
            None,
        );

        let physical_cursor_rect = textlayout::cursor_rect(
            string,
            byte_offset,
            layout,
            text_input.text_cursor_width() * scale_factor,
        );

        physical_cursor_rect.translate(layout_top_left.to_vector()) / scale_factor
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

    fn default_font_size(&self) -> LogicalLength {
        self::textlayout::DEFAULT_FONT_SIZE
    }

    fn free_graphics_resources(
        &self,
        component: i_slint_core::component::ComponentRef,
        _items: &mut dyn Iterator<Item = std::pin::Pin<i_slint_core::items::ItemRef<'_>>>,
    ) -> Result<(), i_slint_core::platform::PlatformError> {
        let canvas = if self.canvas.borrow().is_some() {
            std::cell::Ref::map(self.canvas.borrow(), |canvas_opt| canvas_opt.as_ref().unwrap())
        } else {
            return Ok(());
        };

        canvas.image_cache.component_destroyed(component);
        canvas.path_cache.component_destroyed(component);
        Ok(())
    }
}

trait Surface {
    const SUPPORTS_GRAPHICS_API: bool;
    fn new(
        window: &dyn raw_window_handle::HasRawWindowHandle,
        display: &dyn raw_window_handle::HasRawDisplayHandle,
        size: PhysicalWindowSize,
    ) -> Result<Self, PlatformError>
    where
        Self: Sized;
    fn name(&self) -> &'static str;
    fn with_graphics_api(&self, callback: impl FnOnce(GraphicsAPI<'_>));
    fn with_active_surface(
        &self,
        callback: impl FnOnce(),
    ) -> Result<(), i_slint_core::platform::PlatformError> {
        callback();
        Ok(())
    }
    fn render(
        &self,
        size: PhysicalWindowSize,
        callback: impl FnOnce(&mut skia_safe::Canvas, &mut skia_safe::gpu::DirectContext),
    ) -> Result<(), i_slint_core::platform::PlatformError>;
    fn resize_event(
        &self,
        size: PhysicalWindowSize,
    ) -> Result<(), i_slint_core::platform::PlatformError>;
    fn bits_per_pixel(&self) -> Result<u8, PlatformError>;
}

struct SkiaCanvas<SurfaceType: Surface, NativeWindowWrapper> {
    image_cache: ItemCache<Option<skia_safe::Image>>,
    path_cache: ItemCache<Option<(Vector2D<f32, PhysicalPx>, skia_safe::Path)>>,
    rendering_metrics_collector: Option<Rc<RenderingMetricsCollector>>,
    surface: SurfaceType,
    // Kept here to make sure that the raw window handles used by the surface are kept alive
    native_window: NativeWindowWrapper,
}

impl<SurfaceType: Surface, NativeWindowWrapper> SkiaCanvas<SurfaceType, NativeWindowWrapper> {
    fn with_graphics_api(&self, callback: impl FnOnce(GraphicsAPI<'_>)) {
        self.surface.with_graphics_api(callback)
    }
}
