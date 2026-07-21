// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

//! Slint renderer scaffolding generic over an [`anyrender`] backend.
//!
//! This crate provides:
//! - [`AnyrenderItemRenderer`]: a Slint
//!   [`ItemRenderer`](i_slint_core::item_rendering::ItemRenderer) generic
//!   over any [`anyrender::PaintScene`] sink.
//! - [`SlintWindowRenderer`]: a small extension on top of
//!   [`anyrender::WindowRenderer`] adding the fallible operations Slint
//!   needs (a per-frame render with a base color and a `Result`-returning
//!   draw closure, and a fallible resize).
//! - [`AnyrenderSlintRenderer`]: a Slint
//!   [`Renderer`](i_slint_core::renderer::Renderer) that drives any backend
//!   implementing `SlintWindowRenderer`.
//!
//! Concrete backends (vello over wgpu, vello_cpu over softbuffer, …) live
//! in their own crates and only need to implement `SlintWindowRenderer`.

#![doc(html_logo_url = "https://slint.dev/logo/slint-logo-square-light.svg")]

use std::cell::RefCell;
use std::num::NonZeroU32;
use std::pin::Pin;
use std::rc::{Rc, Weak};

use i_slint_core::Brush;
use i_slint_core::api::SetRenderingNotifierError;
use i_slint_core::graphics::euclid;
use i_slint_core::graphics::{Rgba8Pixel, SharedPixelBuffer};
use i_slint_core::item_tree::ItemTreeWeak;
use i_slint_core::items::TextWrap;
use i_slint_core::lengths::{LogicalLength, LogicalPoint, LogicalRect, LogicalSize, PhysicalPx};
use i_slint_core::platform::PlatformError;
use i_slint_core::renderer::RendererSealed;
use i_slint_core::textlayout::sharedparley;
use i_slint_core::window::{WindowAdapter, WindowInner};

pub(crate) type PhysicalLength = euclid::Length<f32, PhysicalPx>;
pub(crate) type PhysicalRect = euclid::Rect<f32, PhysicalPx>;
pub(crate) type PhysicalSize = euclid::Size2D<f32, PhysicalPx>;

mod itemrenderer;
mod recording;

pub use itemrenderer::AnyrenderItemRenderer;
pub use recording::RecordingWindowRenderer;

/// Slint-side extension to [`anyrender::WindowRenderer`].
///
/// Adds the fallible operations Slint needs that do not fit anyrender's
/// own `WindowRenderer` signature — namely a per-frame render with a
/// caller-supplied base color and a `Result`-returning closure, and a
/// fallible resize.
pub trait SlintWindowRenderer: anyrender::WindowRenderer {
    fn slint_render<F>(
        &mut self,
        surface_size: i_slint_core::api::PhysicalSize,
        base_color: peniko::color::AlphaColor<peniko::color::Srgb>,
        draw: F,
    ) -> Result<(), PlatformError>
    where
        F: FnOnce(&mut Self::ScenePainter<'_>) -> Result<(), PlatformError>;

    fn slint_set_size(&mut self, width: u32, height: u32) -> Result<(), PlatformError>;

    /// Render `draw` into a CPU-readable RGBA8 buffer instead of presenting
    /// to a surface. Used by [`Window::take_snapshot`](i_slint_core::api::Window::take_snapshot).
    ///
    /// The default impl returns an error; backends override.
    fn slint_take_snapshot<F>(
        &mut self,
        _surface_size: i_slint_core::api::PhysicalSize,
        _base_color: peniko::color::AlphaColor<peniko::color::Srgb>,
        _draw: F,
    ) -> Result<SharedPixelBuffer<Rgba8Pixel>, PlatformError>
    where
        F: FnOnce(&mut Self::ScenePainter<'_>) -> Result<(), PlatformError>,
    {
        Err("take_snapshot is not implemented for this anyrender backend".into())
    }
}

/// A Slint [`Renderer`](i_slint_core::renderer::Renderer) that drives any
/// [`anyrender`] backend implementing [`SlintWindowRenderer`].
pub struct AnyrenderSlintRenderer<W: SlintWindowRenderer> {
    maybe_window_adapter: RefCell<Option<Weak<dyn WindowAdapter>>>,
    window_renderer: RefCell<W>,
    rendering_metrics_collector: std::cell::OnceCell<
        Option<
            std::rc::Rc<
                i_slint_core::graphics::rendering_metrics_collector::RenderingMetricsCollector,
            >,
        >,
    >,
}

impl<W: SlintWindowRenderer> AnyrenderSlintRenderer<W> {
    pub fn with_window_renderer(window_renderer: W) -> Self {
        Self {
            maybe_window_adapter: Default::default(),
            window_renderer: RefCell::new(window_renderer),
            rendering_metrics_collector: Default::default(),
        }
    }

    /// Borrow the underlying [`anyrender::WindowRenderer`] mutably.
    pub fn window_renderer(&self) -> std::cell::RefMut<'_, W> {
        self.window_renderer.borrow_mut()
    }

    pub fn render(&self) -> Result<(), PlatformError> {
        self.render_with_options(0., (0., 0.), None)
    }

    /// Render with optional fixed-screen rotation (used by linuxkms for
    /// portrait/landscape modes) and an optional callback invoked after
    /// item rendering to draw additional content on top — typically the
    /// software mouse cursor in linuxkms.
    pub fn render_with_options(
        &self,
        rotation_angle_degrees: f32,
        translation: (f32, f32),
        post_render_cb: Option<&dyn Fn(&mut dyn i_slint_core::item_rendering::ItemRenderer)>,
    ) -> Result<(), PlatformError> {
        let window_adapter = self.try_window_adapter()?;
        let window = window_adapter.window();
        let surface_size = window.size();

        if surface_size.width == 0 || surface_size.height == 0 {
            return Ok(());
        }

        let window_inner = WindowInner::from_pub(window);

        let collector = self
            .rendering_metrics_collector
            .get_or_init(|| {
                i_slint_core::graphics::rendering_metrics_collector::RenderingMetricsCollector::new(
                    "anyrender renderer",
                )
            })
            .clone();

        let base_color = window_background_color(window_inner);

        let initial_transform = if rotation_angle_degrees != 0. || translation != (0., 0.) {
            kurbo::Affine::translate((translation.0 as f64, translation.1 as f64))
                * kurbo::Affine::rotate((rotation_angle_degrees as f64).to_radians())
        } else {
            kurbo::Affine::IDENTITY
        };

        self.window_renderer.borrow_mut().slint_render(surface_size, base_color, |painter| {
            window_inner
                .draw_contents(|components, post_render| -> Result<(), PlatformError> {
                    let mut item_renderer = AnyrenderItemRenderer::new_with_initial_transform(
                        painter,
                        surface_size.width,
                        surface_size.height,
                        window,
                        initial_transform,
                    );

                    for (component, origin) in components {
                        if let Some(component) = ItemTreeWeak::upgrade(component) {
                            i_slint_core::item_rendering::render_component_items(
                                &component,
                                &mut item_renderer,
                                *origin,
                                &window_adapter,
                            );
                        }
                    }

                    post_render(&mut item_renderer);

                    if let Some(collector) = &collector {
                        collector.measure_frame_rendered(&mut item_renderer, Default::default());
                    }

                    if let Some(cb) = post_render_cb {
                        cb(&mut item_renderer);
                    }

                    Ok(())
                })
                .unwrap_or(Ok(()))
        })
    }

    fn try_window_adapter(&self) -> Result<Rc<dyn WindowAdapter>, PlatformError> {
        self.maybe_window_adapter.borrow().as_ref().and_then(|w| w.upgrade()).ok_or_else(|| {
            "Renderer must be associated with component before use".to_string().into()
        })
    }
}

/// The base color every frame starts out with, before any items are drawn.
///
/// A solid-color window background becomes this base color instead of a
/// regular fill command — see
/// [`AnyrenderItemRenderer::draw_window_background`].
fn window_background_color(window_inner: &WindowInner) -> peniko::Color {
    window_inner
        .window_item()
        .and_then(|w| match w.as_pin_ref().background() {
            Brush::SolidColor(c) => Some(itemrenderer::to_peniko_color(c)),
            _ => None,
        })
        .unwrap_or(peniko::color::palette::css::WHITE)
}

#[doc(hidden)]
impl<W: SlintWindowRenderer> RendererSealed for AnyrenderSlintRenderer<W> {
    fn text_size(
        &self,
        text_item: Pin<&dyn i_slint_core::item_rendering::RenderString>,
        item_rc: &i_slint_core::items::ItemRc,
        max_width: Option<LogicalLength>,
        text_wrap: TextWrap,
    ) -> LogicalSize {
        sharedparley::text_size(self, text_item, item_rc, max_width, text_wrap, None)
            .unwrap_or_default()
    }

    fn char_size(
        &self,
        text_item: Pin<&dyn i_slint_core::item_rendering::HasFont>,
        item_rc: &i_slint_core::item_tree::ItemRc,
        ch: char,
    ) -> LogicalSize {
        self.slint_context()
            .and_then(|ctx| {
                let mut font_ctx = ctx.font_context().borrow_mut();
                sharedparley::char_size(&mut font_ctx, text_item, item_rc, ch)
            })
            .unwrap_or_default()
    }

    fn font_metrics(
        &self,
        font_request: i_slint_core::graphics::FontRequest,
    ) -> i_slint_core::items::FontMetrics {
        self.slint_context()
            .map(|ctx| {
                let mut font_ctx = ctx.font_context().borrow_mut();
                sharedparley::font_metrics(&mut font_ctx, font_request)
            })
            .unwrap_or_default()
    }

    fn text_input_byte_offset_for_position(
        &self,
        text_input: Pin<&i_slint_core::items::TextInput>,
        item_rc: &i_slint_core::item_tree::ItemRc,
        pos: LogicalPoint,
    ) -> usize {
        sharedparley::text_input_byte_offset_for_position(self, text_input, item_rc, pos)
    }

    fn text_input_cursor_rect_for_byte_offset(
        &self,
        text_input: std::pin::Pin<&i_slint_core::items::TextInput>,
        item_rc: &i_slint_core::item_tree::ItemRc,
        byte_offset: usize,
    ) -> LogicalRect {
        sharedparley::text_input_cursor_rect_for_byte_offset(self, text_input, item_rc, byte_offset)
    }

    fn register_font_from_memory(
        &self,
        data: &'static [u8],
    ) -> Result<(), Box<dyn std::error::Error>> {
        let ctx = self.slint_context().ok_or("slint platform not initialized")?;
        ctx.font_context().borrow_mut().collection.register_fonts(data.to_vec().into(), None);
        Ok(())
    }

    fn register_font_from_path(
        &self,
        path: &std::path::Path,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let requested_path = path.canonicalize().unwrap_or_else(|_| path.into());
        let contents = std::fs::read(requested_path)?;
        let ctx = self.slint_context().ok_or("slint platform not initialized")?;
        ctx.font_context().borrow_mut().collection.register_fonts(contents.into(), None);
        Ok(())
    }

    fn set_rendering_notifier(
        &self,
        _callback: Box<dyn i_slint_core::api::RenderingNotifier>,
    ) -> Result<(), i_slint_core::api::SetRenderingNotifierError> {
        Err(SetRenderingNotifierError::Unsupported)
    }

    fn free_graphics_resources(
        &self,
        _component: i_slint_core::item_tree::ItemTreeRef,
        _items: &mut dyn Iterator<Item = Pin<i_slint_core::items::ItemRef<'_>>>,
    ) -> Result<(), PlatformError> {
        Ok(())
    }

    fn set_window_adapter(&self, window_adapter: &Rc<dyn WindowAdapter>) {
        *self.maybe_window_adapter.borrow_mut() = Some(Rc::downgrade(window_adapter));
    }

    fn window_adapter(&self) -> Option<Rc<dyn WindowAdapter>> {
        self.maybe_window_adapter
            .borrow()
            .as_ref()
            .and_then(|window_adapter| window_adapter.upgrade())
    }

    fn resize(&self, size: i_slint_core::api::PhysicalSize) -> Result<(), PlatformError> {
        let Some((width, height)): Option<(NonZeroU32, NonZeroU32)> =
            size.width.try_into().ok().zip(size.height.try_into().ok())
        else {
            return Ok(());
        };
        self.window_renderer.borrow_mut().slint_set_size(width.get(), height.get())
    }

    fn take_snapshot(&self) -> Result<SharedPixelBuffer<Rgba8Pixel>, PlatformError> {
        let window_adapter = self.try_window_adapter()?;
        let window = window_adapter.window();
        let window_size = window.size();
        if window_size.width == 0 || window_size.height == 0 {
            return Err("window has zero size".to_string().into());
        }
        let window_inner = WindowInner::from_pub(window);
        let base_color = window_background_color(window_inner);

        self.window_renderer.borrow_mut().slint_take_snapshot(window_size, base_color, |painter| {
            window_inner
                .draw_contents(|components, post_render| -> Result<(), PlatformError> {
                    let mut item_renderer = AnyrenderItemRenderer::new(
                        painter,
                        window_size.width,
                        window_size.height,
                        window,
                    );
                    for (component, origin) in components {
                        if let Some(component) = ItemTreeWeak::upgrade(component) {
                            i_slint_core::item_rendering::render_component_items(
                                &component,
                                &mut item_renderer,
                                *origin,
                                &window_adapter,
                            );
                        }
                    }
                    post_render(&mut item_renderer);
                    Ok(())
                })
                .unwrap_or(Ok(()))
        })
    }

    fn supports_transformations(&self) -> bool {
        true
    }
}
