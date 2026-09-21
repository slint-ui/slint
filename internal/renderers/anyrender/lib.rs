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
//! Concrete backends (vello over wgpu, vello_cpu over softbuffer, ...) live
//! in their own crates and only need to implement `SlintWindowRenderer`.

#![doc(html_logo_url = "https://slint.dev/logo/slint-logo-square-light.svg")]
// anyrender doesn't compile on 32-bit targets, so this crate is empty there. The upstream
// fix is https://github.com/DioxusLabs/anyrender/pull/74; drop this once it's released and
// we've updated. See also the target dependency in Cargo.toml.
#![cfg(any(target_pointer_width = "64", target_arch = "wasm32"))]

use std::cell::{Cell, RefCell};
use std::num::NonZeroU32;
use std::pin::Pin;
use std::rc::{Rc, Weak};

use i_slint_core::Brush;
use i_slint_core::api::SetRenderingNotifierError;
use i_slint_core::graphics::euclid;
use i_slint_core::graphics::rendering_metrics_collector::RenderingMetricsCollector;
use i_slint_core::graphics::{Rgba8Pixel, SharedPixelBuffer};
use i_slint_core::item_rendering::ItemCache;
use i_slint_core::item_tree::ItemTreeWeak;
use i_slint_core::lengths::PhysicalPx;
use i_slint_core::platform::PlatformError;
use i_slint_core::renderer::DrawOutcome;
use i_slint_core::renderer::RendererSealed;
use i_slint_core::textlayout::sharedparley;
use i_slint_core::window::{WindowAdapter, WindowInner};

pub(crate) type PhysicalLength = euclid::Length<f32, PhysicalPx>;
pub(crate) type PhysicalPoint = euclid::Point2D<f32, PhysicalPx>;
pub(crate) type PhysicalRect = euclid::Rect<f32, PhysicalPx>;
pub(crate) type PhysicalSize = euclid::Size2D<f32, PhysicalPx>;

// cSpell: ignore imagecache winsys
mod imagecache;
mod itemrenderer;
mod recording;
#[cfg(feature = "vello")]
mod vello;

pub use imagecache::{ImageConversionCache, SharedImageData};
pub use itemrenderer::AnyrenderItemRenderer;
pub use recording::RecordingWindowRenderer;
#[cfg(feature = "vello")]
pub use vello::VelloWindowRenderer;

/// A Slint renderer rendering through vello on WGPU.
#[cfg(feature = "vello")]
pub type VelloRenderer = AnyrenderSlintRenderer<VelloWindowRenderer>;

/// Slint-side extension to [`anyrender::WindowRenderer`].
///
/// Adds the fallible operations Slint needs that do not fit anyrender's
/// own `WindowRenderer` signature: a per-frame render with a
/// caller-supplied base color and a `Result`-returning closure, and a
/// fallible resize.
pub trait SlintWindowRenderer: anyrender::WindowRenderer {
    /// Render one frame of `surface_size` and present it. The surface starts
    /// out filled with `base_color`; `draw` then paints the window's items on
    /// top of it. Errors from `draw` are propagated to the caller.
    ///
    /// Returns what happened to the frame. Anything but
    /// [`DrawOutcome::Success`] means nothing was presented and `draw` may not
    /// have run at all, so the caller has to ask for another frame.
    fn slint_render<F>(
        &mut self,
        surface_size: i_slint_core::api::PhysicalSize,
        base_color: peniko::color::AlphaColor<peniko::color::Srgb>,
        draw: F,
    ) -> Result<DrawOutcome, PlatformError>
    where
        F: FnOnce(&mut Self::ScenePainter<'_>) -> Result<(), PlatformError>;

    /// Resize the surface to `width` x `height` physical pixels. Called when
    /// the window was resized, so unlike
    /// [`anyrender::WindowRenderer::set_size`] this may report a failure to
    /// reconfigure the surface.
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

    /// Describes what actually renders, for the `SLINT_DEBUG_PERFORMANCE`
    /// report. Called once per surface, so it can name the graphics device
    /// the surface ended up on.
    fn winsys_info(&self) -> String {
        "anyrender renderer".into()
    }
}

/// Created on the first frame after a surface becomes available, so that
/// [`SlintWindowRenderer::winsys_info`] can name the graphics device. Stays
/// `None` when the `SLINT_DEBUG_PERFORMANCE` environment variable does not
/// ask for metrics.
type MaybeMetricsCollector = RefCell<Option<Rc<RenderingMetricsCollector>>>;

/// A Slint [`Renderer`](i_slint_core::renderer::Renderer) that drives any
/// [`anyrender`] backend implementing [`SlintWindowRenderer`].
pub struct AnyrenderSlintRenderer<W: SlintWindowRenderer> {
    maybe_window_adapter: RefCell<Option<Weak<dyn WindowAdapter>>>,
    window_renderer: RefCell<W>,
    image_cache: RefCell<ImageConversionCache>,
    item_image_cache: ItemCache<Option<SharedImageData>>,
    text_layout_cache: sharedparley::TextLayoutCache,
    rendering_metrics_collector: MaybeMetricsCollector,
    /// Set when a surface is created, so the collector is rebuilt against the
    /// device that surface ended up on.
    rendering_first_time: Cell<bool>,
}

impl<W: SlintWindowRenderer> AnyrenderSlintRenderer<W> {
    pub fn with_window_renderer(window_renderer: W) -> Self {
        Self {
            maybe_window_adapter: Default::default(),
            window_renderer: RefCell::new(window_renderer),
            image_cache: Default::default(),
            item_image_cache: Default::default(),
            text_layout_cache: Default::default(),
            rendering_metrics_collector: Default::default(),
            rendering_first_time: Cell::new(true),
        }
    }

    /// Call after a surface was created, so that the next frame reports the
    /// metrics against the device it ended up on.
    pub fn reset_metrics_collector(&self) {
        self.rendering_first_time.set(true);
    }

    /// Borrow the underlying [`anyrender::WindowRenderer`] mutably.
    pub fn window_renderer(&self) -> std::cell::RefMut<'_, W> {
        self.window_renderer.borrow_mut()
    }

    pub fn render(&self) -> Result<DrawOutcome, PlatformError> {
        self.render_with_options(0., (0., 0.), None)
    }

    /// Render with optional fixed-screen rotation (used by linuxkms for
    /// portrait/landscape modes) and an optional callback invoked after
    /// item rendering to draw additional content on top, typically the
    /// software mouse cursor in linuxkms.
    pub fn render_with_options(
        &self,
        rotation_angle_degrees: f32,
        translation: (f32, f32),
        post_render_cb: Option<&dyn Fn(&mut dyn i_slint_core::item_rendering::ItemRenderer)>,
    ) -> Result<DrawOutcome, PlatformError> {
        let window_adapter = self.try_window_adapter()?;
        let window = window_adapter.window();
        let surface_size = window.size();

        if surface_size.width == 0 || surface_size.height == 0 {
            return Ok(DrawOutcome::Skipped);
        }

        let window_inner = WindowInner::from_pub(window);

        if self.rendering_first_time.take() {
            *self.rendering_metrics_collector.borrow_mut() =
                RenderingMetricsCollector::new(&self.window_renderer.borrow().winsys_info());
        }
        let collector = self.rendering_metrics_collector.borrow().clone();

        self.item_image_cache.clear_cache_if_scale_factor_changed(window);

        let base_color = window_background_color(window_inner);

        let initial_transform = if rotation_angle_degrees != 0. || translation != (0., 0.) {
            kurbo::Affine::translate((translation.0 as f64, translation.1 as f64))
                * kurbo::Affine::rotate((rotation_angle_degrees as f64).to_radians())
        } else {
            kurbo::Affine::IDENTITY
        };

        let result =
            self.window_renderer.borrow_mut().slint_render(surface_size, base_color, |painter| {
                window_inner
                    .draw_contents(|components, post_render| -> Result<(), PlatformError> {
                        let mut item_renderer = AnyrenderItemRenderer::new_with_initial_transform(
                            painter,
                            surface_size.width,
                            surface_size.height,
                            window,
                            &self.image_cache,
                            &self.item_image_cache,
                            &self.text_layout_cache,
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
                            collector
                                .measure_frame_rendered(&mut item_renderer, Default::default());
                        }

                        if let Some(cb) = post_render_cb {
                            cb(&mut item_renderer);
                        }

                        Ok(())
                    })
                    .unwrap_or(Ok(()))
            });

        self.image_cache.borrow_mut().drain();

        result
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
/// regular fill command, see
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
    fn text_layout_cache(&self) -> Option<&sharedparley::TextLayoutCache> {
        Some(&self.text_layout_cache)
    }

    fn set_rendering_notifier(
        &self,
        _callback: Box<dyn i_slint_core::api::RenderingNotifier>,
    ) -> Result<(), i_slint_core::api::SetRenderingNotifierError> {
        Err(SetRenderingNotifierError::Unsupported)
    }

    fn free_graphics_resources(
        &self,
        component: i_slint_core::item_tree::ItemTreeRef,
        _items: &mut dyn Iterator<Item = Pin<i_slint_core::items::ItemRef<'_>>>,
    ) -> Result<(), PlatformError> {
        self.item_image_cache.component_destroyed(component);
        self.text_layout_cache.component_destroyed(component);
        Ok(())
    }

    fn set_window_adapter(&self, window_adapter: &Rc<dyn WindowAdapter>) {
        *self.maybe_window_adapter.borrow_mut() = Some(Rc::downgrade(window_adapter));
        self.item_image_cache.clear_all();
        self.text_layout_cache.clear_all();
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

        let result = self.window_renderer.borrow_mut().slint_take_snapshot(
            window_size,
            base_color,
            |painter| {
                window_inner
                    .draw_contents(|components, post_render| -> Result<(), PlatformError> {
                        let mut item_renderer = AnyrenderItemRenderer::new(
                            painter,
                            window_size.width,
                            window_size.height,
                            window,
                            &self.image_cache,
                            &self.item_image_cache,
                            &self.text_layout_cache,
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
            },
        );

        self.image_cache.borrow_mut().drain();

        result
    }

    fn supports_transformations(&self) -> bool {
        true
    }
}
