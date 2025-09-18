// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

#![doc = include_str!("README.md")]
#![doc(html_logo_url = "https://slint.dev/logo/slint-logo-square-light.svg")]

use std::cell::{Cell, RefCell};
use std::num::NonZeroU32;
use std::pin::Pin;
use std::rc::{Rc, Weak};

use i_slint_common::sharedfontdb;
use i_slint_core::api::{RenderingNotifier, RenderingState, SetRenderingNotifierError};
use i_slint_core::graphics::{euclid, rendering_metrics_collector::RenderingMetricsCollector};
use i_slint_core::graphics::{BorderRadius, Rgba8Pixel};
use i_slint_core::graphics::{FontRequest, SharedPixelBuffer};
use i_slint_core::item_rendering::ItemRenderer;
use i_slint_core::items::TextWrap;
use i_slint_core::lengths::{
    LogicalLength, LogicalPoint, LogicalRect, LogicalSize, PhysicalPx, ScaleFactor,
};
use i_slint_core::platform::PlatformError;
use i_slint_core::renderer::RendererSealed;
use i_slint_core::window::{WindowAdapter, WindowInner};
use i_slint_core::Brush;
use images::TextureImporter;

type PhysicalLength = euclid::Length<f32, PhysicalPx>;
type PhysicalRect = euclid::Rect<f32, PhysicalPx>;
type PhysicalSize = euclid::Size2D<f32, PhysicalPx>;
type PhysicalPoint = euclid::Point2D<f32, PhysicalPx>;
type PhysicalBorderRadius = BorderRadius<f32, PhysicalPx>;

use self::itemrenderer::CanvasRc;

mod fonts;
mod images;
mod itemrenderer;
#[cfg(feature = "opengl")]
pub mod opengl;
#[cfg(feature = "wgpu-26")]
pub mod wgpu;

pub trait WindowSurface<R: femtovg::Renderer> {
    fn render_surface(&self) -> &R::Surface;
}

pub trait GraphicsBackend {
    type Renderer: femtovg::Renderer + TextureImporter;
    type WindowSurface: WindowSurface<Self::Renderer>;
    const NAME: &'static str;
    fn new_suspended() -> Self;
    fn clear_graphics_context(&self);
    fn begin_surface_rendering(
        &self,
    ) -> Result<Self::WindowSurface, Box<dyn std::error::Error + Send + Sync>>;
    fn submit_commands(&self, commands: <Self::Renderer as femtovg::Renderer>::CommandBuffer);
    fn present_surface(
        &self,
        surface: Self::WindowSurface,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>>;
    fn with_graphics_api<R>(
        &self,
        callback: impl FnOnce(Option<i_slint_core::api::GraphicsAPI<'_>>) -> R,
    ) -> Result<R, i_slint_core::platform::PlatformError>;
    /// This function is called by the renderers when the surface needs to be resized, typically
    /// in response to the windowing system notifying of a change in the window system.
    /// For most implementations this is a no-op, with the exception for wayland for example.
    fn resize(
        &self,
        width: NonZeroU32,
        height: NonZeroU32,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>>;
}

/// Use the FemtoVG renderer when implementing a custom Slint platform where you deliver events to
/// Slint and want the scene to be rendered using OpenGL. The rendering is done using the [FemtoVG](https://github.com/femtovg/femtovg)
/// library.
pub struct FemtoVGRenderer<B: GraphicsBackend> {
    maybe_window_adapter: RefCell<Option<Weak<dyn WindowAdapter>>>,
    rendering_notifier: RefCell<Option<Box<dyn RenderingNotifier>>>,
    canvas: RefCell<Option<CanvasRc<B::Renderer>>>,
    graphics_cache: itemrenderer::ItemGraphicsCache<B::Renderer>,
    texture_cache: RefCell<images::TextureCache<B::Renderer>>,
    rendering_metrics_collector: RefCell<Option<Rc<RenderingMetricsCollector>>>,
    rendering_first_time: Cell<bool>,
    // Last field, so that it's dropped last and for example the OpenGL context exists and is current when destroying the FemtoVG canvas
    graphics_backend: B,
}

impl<B: GraphicsBackend> FemtoVGRenderer<B> {
    /// Render the scene using OpenGL.
    pub fn render(&self) -> Result<(), i_slint_core::platform::PlatformError> {
        self.internal_render_with_post_callback(
            0.,
            (0., 0.),
            self.window_adapter()?.window().size(),
            None,
        )
    }

    fn internal_render_with_post_callback(
        &self,
        rotation_angle_degrees: f32,
        translation: (f32, f32),
        surface_size: i_slint_core::api::PhysicalSize,
        post_render_cb: Option<&dyn Fn(&mut dyn ItemRenderer)>,
    ) -> Result<(), i_slint_core::platform::PlatformError> {
        let surface = self.graphics_backend.begin_surface_rendering()?;

        if self.rendering_first_time.take() {
            *self.rendering_metrics_collector.borrow_mut() = RenderingMetricsCollector::new(
                &format!("FemtoVG renderer with {} backend", B::NAME),
            );

            if let Some(callback) = self.rendering_notifier.borrow_mut().as_mut() {
                self.with_graphics_api(|api| {
                    callback.notify(RenderingState::RenderingSetup, &api)
                })?;
            }
        }

        let window_adapter = self.window_adapter()?;
        let window = window_adapter.window();
        let window_size = window.size();

        let Some((width, height)): Option<(NonZeroU32, NonZeroU32)> =
            window_size.width.try_into().ok().zip(window_size.height.try_into().ok())
        else {
            // Nothing to render
            return Ok(());
        };

        if self.canvas.borrow().is_none() {
            // Nothing to render
            return Ok(());
        }

        let window_inner = WindowInner::from_pub(window);
        let scale = window_inner.scale_factor().ceil();

        window_inner
            .draw_contents(|components| -> Result<(), PlatformError> {
                // self.canvas is checked for being Some(...) at the beginning of this function
                let canvas = self.canvas.borrow().as_ref().unwrap().clone();

                let window_background_brush =
                    window_inner.window_item().map(|w| w.as_pin_ref().background());

                {
                    let mut femtovg_canvas = canvas.borrow_mut();
                    // We pass an integer that is greater than or equal to the scale factor as
                    // dpi / device pixel ratio as the anti-alias of femtovg needs that to draw text clearly.
                    // We need to care about that `ceil()` when calculating metrics.
                    femtovg_canvas.set_size(surface_size.width, surface_size.height, scale);

                    // Clear with window background if it is a solid color otherwise it will drawn as gradient
                    if let Some(Brush::SolidColor(clear_color)) = window_background_brush {
                        femtovg_canvas.clear_rect(
                            0,
                            0,
                            surface_size.width,
                            surface_size.height,
                            self::itemrenderer::to_femtovg_color(&clear_color),
                        );
                    }
                }

                {
                    let mut femtovg_canvas = canvas.borrow_mut();
                    femtovg_canvas.reset();
                    femtovg_canvas.rotate(rotation_angle_degrees.to_radians());
                    femtovg_canvas.translate(translation.0, translation.1);
                }

                if let Some(notifier_fn) = self.rendering_notifier.borrow_mut().as_mut() {
                    let mut femtovg_canvas = canvas.borrow_mut();
                    // For the BeforeRendering rendering notifier callback it's important that this happens *after* clearing
                    // the back buffer, in order to allow the callback to provide its own rendering of the background.
                    // femtovg's clear_rect() will merely schedule a clear call, so flush right away to make it immediate.

                    let commands = femtovg_canvas.flush_to_surface(surface.render_surface());
                    self.graphics_backend.submit_commands(commands);

                    femtovg_canvas.set_size(width.get(), height.get(), scale);
                    drop(femtovg_canvas);

                    self.with_graphics_api(|api| {
                        notifier_fn.notify(RenderingState::BeforeRendering, &api)
                    })?;
                }

                self.graphics_cache.clear_cache_if_scale_factor_changed(window);

                let mut item_renderer = self::itemrenderer::GLItemRenderer::new(
                    &canvas,
                    &self.graphics_cache,
                    &self.texture_cache,
                    window,
                    width.get(),
                    height.get(),
                );

                if let Some(window_item_rc) = window_inner.window_item_rc() {
                    let window_item =
                        window_item_rc.downcast::<i_slint_core::items::WindowItem>().unwrap();
                    match window_item.as_pin_ref().background() {
                        Brush::SolidColor(..) => {
                            // clear_rect is called earlier
                        }
                        _ => {
                            // Draws the window background as gradient
                            item_renderer.draw_rectangle(
                                window_item.as_pin_ref(),
                                &window_item_rc,
                                i_slint_core::lengths::logical_size_from_api(
                                    window.size().to_logical(window_inner.scale_factor()),
                                ),
                                &window_item.as_pin_ref().cached_rendering_data,
                            );
                        }
                    }
                }

                for (component, origin) in components {
                    i_slint_core::item_rendering::render_component_items(
                        component,
                        &mut item_renderer,
                        *origin,
                        &self.window_adapter()?,
                    );
                }

                if let Some(cb) = post_render_cb.as_ref() {
                    cb(&mut item_renderer)
                }

                if let Some(collector) = &self.rendering_metrics_collector.borrow().as_ref() {
                    collector.measure_frame_rendered(&mut item_renderer);
                }

                let commands = canvas.borrow_mut().flush_to_surface(surface.render_surface());
                self.graphics_backend.submit_commands(commands);

                // Delete any images and layer images (and their FBOs) before making the context not current anymore, to
                // avoid GPU memory leaks.
                self.texture_cache.borrow_mut().drain();
                drop(item_renderer);
                Ok(())
            })
            .unwrap_or(Ok(()))?;

        if let Some(callback) = self.rendering_notifier.borrow_mut().as_mut() {
            self.with_graphics_api(|api| callback.notify(RenderingState::AfterRendering, &api))?;
        }

        self.graphics_backend.present_surface(surface)?;
        Ok(())
    }

    fn with_graphics_api(
        &self,
        callback: impl FnOnce(i_slint_core::api::GraphicsAPI<'_>),
    ) -> Result<(), PlatformError> {
        self.graphics_backend.with_graphics_api(|api| callback(api.unwrap()))
    }

    fn window_adapter(&self) -> Result<Rc<dyn WindowAdapter>, PlatformError> {
        self.maybe_window_adapter.borrow().as_ref().and_then(|w| w.upgrade()).ok_or_else(|| {
            "Renderer must be associated with component before use".to_string().into()
        })
    }

    fn reset_canvas(&self, canvas: CanvasRc<B::Renderer>) {
        *self.canvas.borrow_mut() = canvas.into();
        self.rendering_first_time.set(true);
    }
}

#[doc(hidden)]
impl<B: GraphicsBackend> RendererSealed for FemtoVGRenderer<B> {
    fn text_size(
        &self,
        font_request: i_slint_core::graphics::FontRequest,
        text: &str,
        max_width: Option<LogicalLength>,
        scale_factor: ScaleFactor,
        _text_wrap: TextWrap, //TODO: Add support for char-wrap
    ) -> LogicalSize {
        crate::fonts::text_size(&font_request, scale_factor, text, max_width)
    }

    fn font_metrics(
        &self,
        font_request: i_slint_core::graphics::FontRequest,
        _scale_factor: ScaleFactor,
    ) -> i_slint_core::items::FontMetrics {
        crate::fonts::font_metrics(font_request)
    }

    fn text_input_byte_offset_for_position(
        &self,
        text_input: Pin<&i_slint_core::items::TextInput>,
        pos: LogicalPoint,
        font_request: FontRequest,
        scale_factor: ScaleFactor,
    ) -> usize {
        let pos = pos * scale_factor;
        let text = text_input.text();

        let mut result = text.len();

        let width = text_input.width() * scale_factor;
        let height = text_input.height() * scale_factor;
        if width.get() <= 0. || height.get() <= 0. || pos.y < 0. {
            return 0;
        }

        let font = crate::fonts::FONT_CACHE
            .with(|cache| cache.borrow_mut().font(font_request, scale_factor, &text_input.text()));

        let visual_representation = text_input.visual_representation(None);

        let paint = font.init_paint(text_input.letter_spacing() * scale_factor, Default::default());
        let text_context =
            crate::fonts::FONT_CACHE.with(|cache| cache.borrow().text_context.clone());
        let font_height = text_context.measure_font(&paint).unwrap().height();
        crate::fonts::layout_text_lines(
            &visual_representation.text,
            &font,
            PhysicalSize::from_lengths(width, height),
            (text_input.horizontal_alignment(), text_input.vertical_alignment()),
            text_input.wrap(),
            i_slint_core::items::TextOverflow::Clip,
            text_input.single_line(),
            None,
            &paint,
            |line_text, line_pos, start, metrics| {
                if (line_pos.y..(line_pos.y + font_height)).contains(&pos.y) {
                    let mut current_x = 0.;
                    for glyph in &metrics.glyphs {
                        if line_pos.x + current_x + glyph.advance_x / 2. >= pos.x {
                            result = start + glyph.byte_index;
                            return;
                        }
                        current_x += glyph.advance_x;
                    }
                    result = start + line_text.trim_end().len();
                }
            },
        );

        visual_representation.map_byte_offset_from_byte_offset_in_visual_text(result)
    }

    fn text_input_cursor_rect_for_byte_offset(
        &self,
        text_input: Pin<&i_slint_core::items::TextInput>,
        byte_offset: usize,
        font_request: FontRequest,
        scale_factor: ScaleFactor,
    ) -> LogicalRect {
        let text = text_input.text();

        let font_size = font_request.pixel_size.unwrap_or(fonts::DEFAULT_FONT_SIZE);

        let width = text_input.width() * scale_factor;
        let height = text_input.height() * scale_factor;
        if width.get() <= 0. || height.get() <= 0. {
            return LogicalRect::new(
                LogicalPoint::default(),
                LogicalSize::from_lengths(LogicalLength::new(1.0), font_size),
            );
        }

        let font = crate::fonts::FONT_CACHE
            .with(|cache| cache.borrow_mut().font(font_request, scale_factor, &text_input.text()));

        let paint = font.init_paint(text_input.letter_spacing() * scale_factor, Default::default());
        let cursor_point = fonts::layout_text_lines(
            text.as_str(),
            &font,
            PhysicalSize::from_lengths(width, height),
            (text_input.horizontal_alignment(), text_input.vertical_alignment()),
            text_input.wrap(),
            i_slint_core::items::TextOverflow::Clip,
            text_input.single_line(),
            Some(byte_offset),
            &paint,
            |_, _, _, _| {},
        );

        LogicalRect::new(
            cursor_point.unwrap_or_default() / scale_factor,
            LogicalSize::from_lengths(LogicalLength::new(1.0), font_size),
        )
    }

    fn register_font_from_memory(
        &self,
        data: &'static [u8],
    ) -> Result<(), Box<dyn std::error::Error>> {
        sharedfontdb::register_font_from_memory(data)
    }

    fn register_font_from_path(
        &self,
        path: &std::path::Path,
    ) -> Result<(), Box<dyn std::error::Error>> {
        sharedfontdb::register_font_from_path(path)
    }

    fn default_font_size(&self) -> LogicalLength {
        self::fonts::DEFAULT_FONT_SIZE
    }

    fn set_rendering_notifier(
        &self,
        callback: Box<dyn i_slint_core::api::RenderingNotifier>,
    ) -> Result<(), i_slint_core::api::SetRenderingNotifierError> {
        let mut notifier = self.rendering_notifier.borrow_mut();
        if notifier.replace(callback).is_some() {
            Err(SetRenderingNotifierError::AlreadySet)
        } else {
            Ok(())
        }
    }

    fn free_graphics_resources(
        &self,
        component: i_slint_core::item_tree::ItemTreeRef,
        _items: &mut dyn Iterator<Item = Pin<i_slint_core::items::ItemRef<'_>>>,
    ) -> Result<(), i_slint_core::platform::PlatformError> {
        if !self.graphics_cache.is_empty() {
            self.graphics_backend.with_graphics_api(|_| {
                self.graphics_cache.component_destroyed(component);
            })?;
        }
        Ok(())
    }

    fn set_window_adapter(&self, window_adapter: &Rc<dyn WindowAdapter>) {
        *self.maybe_window_adapter.borrow_mut() = Some(Rc::downgrade(window_adapter));
        self.graphics_backend
            .with_graphics_api(|_| {
                self.graphics_cache.clear_all();
                self.texture_cache.borrow_mut().clear();
            })
            .ok();
    }

    fn resize(&self, size: i_slint_core::api::PhysicalSize) -> Result<(), PlatformError> {
        if let Some((width, height)) = size.width.try_into().ok().zip(size.height.try_into().ok()) {
            self.graphics_backend.resize(width, height)?;
        };
        Ok(())
    }

    /// Returns an image buffer of what was rendered last by reading the previous front buffer (using glReadPixels).
    fn take_snapshot(&self) -> Result<SharedPixelBuffer<Rgba8Pixel>, PlatformError> {
        self.graphics_backend.with_graphics_api(|_| {
            let Some(canvas) = self.canvas.borrow().as_ref().cloned() else {
                return Err("FemtoVG renderer cannot take screenshot without a window".into());
            };
            let screenshot = canvas
                .borrow_mut()
                .screenshot()
                .map_err(|e| format!("FemtoVG error reading current back buffer: {e}"))?;

            use rgb::ComponentBytes;
            Ok(SharedPixelBuffer::clone_from_slice(
                screenshot.buf().as_bytes(),
                screenshot.width() as u32,
                screenshot.height() as u32,
            ))
        })?
    }

    fn supports_transformations(&self) -> bool {
        true
    }
}

impl<B: GraphicsBackend> Drop for FemtoVGRenderer<B> {
    fn drop(&mut self) {
        self.clear_graphics_context().ok();
    }
}

/// The purpose of this trait is to add internal API that's accessed from the winit/linuxkms backends, but not
/// public (as the trait isn't re-exported).
#[doc(hidden)]
pub trait FemtoVGRendererExt {
    fn new_suspended() -> Self;
    fn clear_graphics_context(&self) -> Result<(), i_slint_core::platform::PlatformError>;
    fn render_transformed_with_post_callback(
        &self,
        rotation_angle_degrees: f32,
        translation: (f32, f32),
        surface_size: i_slint_core::api::PhysicalSize,
        post_render_cb: Option<&dyn Fn(&mut dyn ItemRenderer)>,
    ) -> Result<(), i_slint_core::platform::PlatformError>;
}

/// The purpose of this trait is to add internal API specific to the OpenGL renderer that's accessed from the winit
/// backend. In this case, the ability to resume a suspended OpenGL renderer by providing a new context.
#[doc(hidden)]
#[cfg(feature = "opengl")]
pub trait FemtoVGOpenGLRendererExt {
    fn set_opengl_context(
        &self,
        #[cfg(not(target_arch = "wasm32"))] opengl_context: impl opengl::OpenGLInterface + 'static,
        #[cfg(target_arch = "wasm32")] html_canvas: web_sys::HtmlCanvasElement,
    ) -> Result<(), i_slint_core::platform::PlatformError>;
}

#[doc(hidden)]
impl<B: GraphicsBackend> FemtoVGRendererExt for FemtoVGRenderer<B> {
    /// Creates a new renderer in suspended state without OpenGL. Any attempts at rendering, etc. will produce an error,
    /// until [`Self::set_opengl_context()`] was called successfully.
    fn new_suspended() -> Self {
        Self {
            maybe_window_adapter: Default::default(),
            rendering_notifier: Default::default(),
            canvas: RefCell::new(None),
            graphics_cache: Default::default(),
            texture_cache: Default::default(),
            rendering_metrics_collector: Default::default(),
            rendering_first_time: Cell::new(true),
            graphics_backend: B::new_suspended(),
        }
    }

    fn clear_graphics_context(&self) -> Result<(), i_slint_core::platform::PlatformError> {
        // Ensure the context is current before the renderer is destroyed
        self.graphics_backend.with_graphics_api(|api| {
            // If we've rendered a frame before, then we need to invoke the RenderingTearDown notifier.
            if !self.rendering_first_time.get() && api.is_some() {
                if let Some(callback) = self.rendering_notifier.borrow_mut().as_mut() {
                    self.with_graphics_api(|api| {
                        callback.notify(RenderingState::RenderingTeardown, &api)
                    })
                    .ok();
                }
            }

            self.graphics_cache.clear_all();
            self.texture_cache.borrow_mut().clear();
        })?;

        if let Some(canvas) = self.canvas.borrow_mut().take() {
            if Rc::strong_count(&canvas) != 1 {
                i_slint_core::debug_log!("internal warning: there are canvas references left when destroying the window. OpenGL resources will be leaked.")
            }
        }

        self.graphics_backend.clear_graphics_context();

        Ok(())
    }

    fn render_transformed_with_post_callback(
        &self,
        rotation_angle_degrees: f32,
        translation: (f32, f32),
        surface_size: i_slint_core::api::PhysicalSize,
        post_render_cb: Option<&dyn Fn(&mut dyn ItemRenderer)>,
    ) -> Result<(), i_slint_core::platform::PlatformError> {
        self.internal_render_with_post_callback(
            rotation_angle_degrees,
            translation,
            surface_size,
            post_render_cb,
        )
    }
}

#[cfg(feature = "opengl")]
impl FemtoVGOpenGLRendererExt for FemtoVGRenderer<opengl::OpenGLBackend> {
    fn set_opengl_context(
        &self,
        #[cfg(not(target_arch = "wasm32"))] opengl_context: impl opengl::OpenGLInterface + 'static,
        #[cfg(target_arch = "wasm32")] html_canvas: web_sys::HtmlCanvasElement,
    ) -> Result<(), i_slint_core::platform::PlatformError> {
        self.graphics_backend.set_opengl_context(
            self,
            #[cfg(not(target_arch = "wasm32"))]
            opengl_context,
            #[cfg(target_arch = "wasm32")]
            html_canvas,
        )
    }
}

#[cfg(feature = "opengl")]
pub type FemtoVGOpenGLRenderer = FemtoVGRenderer<opengl::OpenGLBackend>;
