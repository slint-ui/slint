// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

#![doc = include_str!("README.md")]
#![doc(html_logo_url = "https://slint.dev/logo/slint-logo-square-light.svg")]

#[cfg(any(target_vendor = "apple", skia_backend_vulkan))]
use std::cell::OnceCell;
use std::cell::{Cell, RefCell};
use std::rc::{Rc, Weak};
use std::sync::Arc;

use i_slint_core::api::{
    GraphicsAPI, PhysicalSize as PhysicalWindowSize, RenderingNotifier, RenderingState,
    SetRenderingNotifierError, Window,
};
use i_slint_core::graphics::euclid::{self, Vector2D};
use i_slint_core::graphics::rendering_metrics_collector::RenderingMetricsCollector;
use i_slint_core::graphics::RequestedGraphicsAPI;
use i_slint_core::graphics::{BorderRadius, FontRequest, SharedPixelBuffer};
use i_slint_core::item_rendering::{DirtyRegion, ItemCache, ItemRenderer, PartialRenderingState};
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
type PhysicalBorderRadius = BorderRadius<f32, PhysicalPx>;

mod cached_image;
mod itemrenderer;
mod textlayout;

#[cfg(skia_backend_software)]
pub mod software_surface;

#[cfg(target_vendor = "apple")]
pub mod metal_surface;

#[cfg(target_family = "windows")]
pub mod d3d_surface;

#[cfg(skia_backend_vulkan)]
pub mod vulkan_surface;

#[cfg(any(not(target_vendor = "apple"), target_os = "macos"))]
pub mod opengl_surface;

#[cfg(feature = "unstable-wgpu-26")]
mod wgpu_26_surface;

use i_slint_core::items::TextWrap;
use itemrenderer::to_skia_rect;
pub use skia_safe;

cfg_if::cfg_if! {
    if #[cfg(skia_backend_vulkan)] {
        type DefaultSurface = vulkan_surface::VulkanSurface;
    } else if #[cfg(skia_backend_opengl)] {
        type DefaultSurface = opengl_surface::OpenGLSurface;
    } else if #[cfg(skia_backend_metal)] {
        type DefaultSurface = metal_surface::MetalSurface;
    } else if #[cfg(skia_backend_software)] {
        type DefaultSurface = software_surface::SoftwareSurface;
    }
}

fn create_default_surface(
    context: &SkiaSharedContext,
    window_handle: Arc<dyn raw_window_handle::HasWindowHandle + Sync + Send>,
    display_handle: Arc<dyn raw_window_handle::HasDisplayHandle + Sync + Send>,
    size: PhysicalWindowSize,
    requested_graphics_api: Option<RequestedGraphicsAPI>,
) -> Result<Box<dyn Surface>, PlatformError> {
    match DefaultSurface::new(
        context,
        window_handle.clone(),
        display_handle.clone(),
        size,
        requested_graphics_api,
    ) {
        Ok(gpu_surface) => Ok(Box::new(gpu_surface) as Box<dyn Surface>),
        #[cfg(skia_backend_software)]
        Err(err) => {
            i_slint_core::debug_log!(
                "Failed to initialize Skia GPU renderer: {} . Falling back to software rendering",
                err
            );
            software_surface::SoftwareSurface::new(
                context,
                window_handle,
                display_handle,
                size,
                None,
            )
            .map(|r| Box::new(r) as Box<dyn Surface>)
        }
        #[cfg(not(skia_backend_software))]
        Err(err) => Err(err),
    }
}

enum DirtyRegionDebugMode {
    NoDebug,
    Visualize,
    Log,
}

impl Default for DirtyRegionDebugMode {
    fn default() -> Self {
        match std::env::var("SLINT_SKIA_PARTIAL_RENDERING").as_deref() {
            Ok("visualize") => DirtyRegionDebugMode::Visualize,
            Ok("log") => DirtyRegionDebugMode::Log,
            _ => DirtyRegionDebugMode::NoDebug,
        }
    }
}

fn create_partial_renderer_state(
    maybe_surface: Option<&dyn Surface>,
) -> Option<PartialRenderingState> {
    maybe_surface
        .map_or_else(
            || std::env::var("SLINT_SKIA_PARTIAL_RENDERING").as_deref().is_ok(),
            |surface| surface.use_partial_rendering(),
        )
        .then(|| PartialRenderingState::default())
}

#[derive(Default)]
struct SkiaSharedContextInner {
    #[cfg(target_vendor = "apple")]
    metal_context: OnceCell<metal_surface::SharedMetalContext>,
    #[cfg(skia_backend_vulkan)]
    vulkan_context: OnceCell<vulkan_surface::SharedVulkanContext>,
}

/// This data structure contains data that's intended to be shared across several instances of SkiaRenderer.
/// For example, for Vulkan rendering, this shares the Vulkan instance.
///
/// Create an instance once and pass clones of it to the difference constructor functions, to ensure most
/// efficient resource usage.
#[derive(Clone, Default)]
pub struct SkiaSharedContext(#[allow(dead_code)] Rc<SkiaSharedContextInner>);

/// Use the SkiaRenderer when implementing a custom Slint platform where you deliver events to
/// Slint and want the scene to be rendered using Skia as underlying graphics library.
pub struct SkiaRenderer {
    maybe_window_adapter: RefCell<Option<Weak<dyn WindowAdapter>>>,
    rendering_notifier: RefCell<Option<Box<dyn RenderingNotifier>>>,
    image_cache: ItemCache<Option<skia_safe::Image>>,
    path_cache: ItemCache<Option<(Vector2D<f32, PhysicalPx>, skia_safe::Path)>>,
    rendering_metrics_collector: RefCell<Option<Rc<RenderingMetricsCollector>>>,
    rendering_first_time: Cell<bool>,
    surface: RefCell<Option<Box<dyn Surface>>>,
    surface_factory: fn(
        &SkiaSharedContext,
        window_handle: Arc<dyn raw_window_handle::HasWindowHandle + Send + Sync>,
        display_handle: Arc<dyn raw_window_handle::HasDisplayHandle + Send + Sync>,
        size: PhysicalWindowSize,
        requested_graphics_api: Option<RequestedGraphicsAPI>,
    ) -> Result<Box<dyn Surface>, PlatformError>,
    pre_present_callback: RefCell<Option<Box<dyn FnMut()>>>,
    partial_rendering_state: Option<PartialRenderingState>,
    dirty_region_debug_mode: DirtyRegionDebugMode,
    /// Tracking dirty regions indexed by buffer age - 1. More than 3 back buffers aren't supported, but also unlikely to happen.
    dirty_region_history: RefCell<[DirtyRegion; 3]>,
    shared_context: SkiaSharedContext,
}

impl SkiaRenderer {
    pub fn default(context: &SkiaSharedContext) -> Self {
        Self {
            maybe_window_adapter: Default::default(),
            rendering_notifier: Default::default(),
            image_cache: Default::default(),
            path_cache: Default::default(),
            rendering_metrics_collector: Default::default(),
            rendering_first_time: Default::default(),
            surface: Default::default(),
            surface_factory: create_default_surface,
            pre_present_callback: Default::default(),
            partial_rendering_state: create_partial_renderer_state(None),
            dirty_region_debug_mode: Default::default(),
            dirty_region_history: Default::default(),
            shared_context: context.clone(),
        }
    }

    #[cfg(skia_backend_software)]
    /// Creates a new SkiaRenderer that will always use Skia's software renderer.
    pub fn default_software(context: &SkiaSharedContext) -> Self {
        Self {
            maybe_window_adapter: Default::default(),
            rendering_notifier: Default::default(),
            image_cache: Default::default(),
            path_cache: Default::default(),
            rendering_metrics_collector: Default::default(),
            rendering_first_time: Default::default(),
            surface: Default::default(),
            surface_factory: |context,
                              window_handle,
                              display_handle,
                              size,
                              requested_graphics_api| {
                software_surface::SoftwareSurface::new(
                    context,
                    window_handle,
                    display_handle,
                    size,
                    requested_graphics_api,
                )
                .map(|r| Box::new(r) as Box<dyn Surface>)
            },
            pre_present_callback: Default::default(),
            partial_rendering_state: PartialRenderingState::default().into(),
            dirty_region_debug_mode: Default::default(),
            dirty_region_history: Default::default(),
            shared_context: context.clone(),
        }
    }

    #[cfg(any(not(target_vendor = "apple"), target_os = "macos"))]
    /// Creates a new SkiaRenderer that will always use Skia's OpenGL renderer.
    pub fn default_opengl(context: &SkiaSharedContext) -> Self {
        Self {
            maybe_window_adapter: Default::default(),
            rendering_notifier: Default::default(),
            image_cache: Default::default(),
            path_cache: Default::default(),
            rendering_metrics_collector: Default::default(),
            rendering_first_time: Default::default(),
            surface: Default::default(),
            surface_factory: |context,
                              window_handle,
                              display_handle,
                              size,
                              requested_graphics_api| {
                opengl_surface::OpenGLSurface::new(
                    context,
                    window_handle,
                    display_handle,
                    size,
                    requested_graphics_api,
                )
                .map(|r| Box::new(r) as Box<dyn Surface>)
            },
            pre_present_callback: Default::default(),
            partial_rendering_state: create_partial_renderer_state(None),
            dirty_region_debug_mode: Default::default(),
            dirty_region_history: Default::default(),
            shared_context: context.clone(),
        }
    }

    #[cfg(target_vendor = "apple")]
    /// Creates a new SkiaRenderer that will always use Skia's Metal renderer.
    pub fn default_metal(context: &SkiaSharedContext) -> Self {
        Self {
            maybe_window_adapter: Default::default(),
            rendering_notifier: Default::default(),
            image_cache: Default::default(),
            path_cache: Default::default(),
            rendering_metrics_collector: Default::default(),
            rendering_first_time: Default::default(),
            surface: Default::default(),
            surface_factory: |context,
                              window_handle,
                              display_handle,
                              size,
                              requested_graphics_api| {
                metal_surface::MetalSurface::new(
                    context,
                    window_handle,
                    display_handle,
                    size,
                    requested_graphics_api,
                )
                .map(|r| Box::new(r) as Box<dyn Surface>)
            },
            pre_present_callback: Default::default(),
            partial_rendering_state: create_partial_renderer_state(None),
            dirty_region_debug_mode: Default::default(),
            dirty_region_history: Default::default(),
            shared_context: context.clone(),
        }
    }

    #[cfg(skia_backend_vulkan)]
    /// Creates a new SkiaRenderer that will always use Skia's Vulkan renderer.
    pub fn default_vulkan(context: &SkiaSharedContext) -> Self {
        Self {
            maybe_window_adapter: Default::default(),
            rendering_notifier: Default::default(),
            image_cache: Default::default(),
            path_cache: Default::default(),
            rendering_metrics_collector: Default::default(),
            rendering_first_time: Default::default(),
            surface: Default::default(),
            surface_factory: |context,
                              window_handle,
                              display_handle,
                              size,
                              requested_graphics_api| {
                vulkan_surface::VulkanSurface::new(
                    context,
                    window_handle,
                    display_handle,
                    size,
                    requested_graphics_api,
                )
                .map(|r| Box::new(r) as Box<dyn Surface>)
            },
            pre_present_callback: Default::default(),
            partial_rendering_state: create_partial_renderer_state(None),
            dirty_region_debug_mode: Default::default(),
            dirty_region_history: Default::default(),
            shared_context: context.clone(),
        }
    }

    #[cfg(target_family = "windows")]
    /// Creates a new SkiaRenderer that will always use Skia's Direct3D renderer.
    pub fn default_direct3d(context: &SkiaSharedContext) -> Self {
        Self {
            maybe_window_adapter: Default::default(),
            rendering_notifier: Default::default(),
            image_cache: Default::default(),
            path_cache: Default::default(),
            rendering_metrics_collector: Default::default(),
            rendering_first_time: Default::default(),
            surface: Default::default(),
            surface_factory: |context,
                              window_handle,
                              display_handle,
                              size,
                              requested_graphics_api| {
                d3d_surface::D3DSurface::new(
                    context,
                    window_handle,
                    display_handle,
                    size,
                    requested_graphics_api,
                )
                .map(|r| Box::new(r) as Box<dyn Surface>)
            },
            pre_present_callback: Default::default(),
            partial_rendering_state: create_partial_renderer_state(None),
            dirty_region_debug_mode: Default::default(),
            dirty_region_history: Default::default(),
            shared_context: context.clone(),
        }
    }

    #[cfg(feature = "unstable-wgpu-26")]
    /// Creates a new SkiaRenderer that will always use Skia's Vulkan renderer.
    pub fn default_wgpu_26(context: &SkiaSharedContext) -> Self {
        Self {
            maybe_window_adapter: Default::default(),
            rendering_notifier: Default::default(),
            image_cache: Default::default(),
            path_cache: Default::default(),
            rendering_metrics_collector: Default::default(),
            rendering_first_time: Default::default(),
            surface: Default::default(),
            surface_factory: |context,
                              window_handle,
                              display_handle,
                              size,
                              requested_graphics_api| {
                wgpu_26_surface::WGPUSurface::new(
                    context,
                    window_handle,
                    display_handle,
                    size,
                    requested_graphics_api,
                )
                .map(|r| Box::new(r) as Box<dyn Surface>)
            },
            pre_present_callback: Default::default(),
            partial_rendering_state: create_partial_renderer_state(None),
            dirty_region_debug_mode: Default::default(),
            dirty_region_history: Default::default(),
            shared_context: context.clone(),
        }
    }

    /// Creates a new renderer is associated with the provided window adapter.
    pub fn new(
        context: &SkiaSharedContext,
        window_handle: Arc<dyn raw_window_handle::HasWindowHandle + Send + Sync>,
        display_handle: Arc<dyn raw_window_handle::HasDisplayHandle + Send + Sync>,
        size: PhysicalWindowSize,
    ) -> Result<Self, PlatformError> {
        Ok(Self::new_with_surface(
            context,
            create_default_surface(context, window_handle, display_handle, size, None)?,
        ))
    }

    /// Creates a new renderer with the given surface trait implementation.
    pub fn new_with_surface(
        context: &SkiaSharedContext,
        surface: Box<dyn Surface + 'static>,
    ) -> Self {
        let partial_rendering_state = create_partial_renderer_state(Some(surface.as_ref())).into();
        Self {
            maybe_window_adapter: Default::default(),
            rendering_notifier: Default::default(),
            image_cache: Default::default(),
            path_cache: Default::default(),
            rendering_metrics_collector: Default::default(),
            rendering_first_time: Cell::new(true),
            surface: RefCell::new(Some(surface)),
            surface_factory: |_, _, _, _, _| {
                Err("Skia renderer constructed with surface does not support dynamic surface re-creation".into())
            },
            pre_present_callback: Default::default(),
            partial_rendering_state,
            dirty_region_debug_mode: Default::default(),
            dirty_region_history: Default::default(),
            shared_context: context.clone(),
        }
    }

    /// Reset the surface to a new surface. (destroy the previously set surface if any)
    pub fn set_surface(&self, surface: Box<dyn Surface + 'static>) {
        self.image_cache.clear_all();
        self.path_cache.clear_all();
        self.rendering_first_time.set(true);
        *self.surface.borrow_mut() = Some(surface);
    }

    fn clear_surface(&self) {
        let Some(surface) = self.surface.borrow_mut().take() else {
            return;
        };

        // If we've rendered a frame before, then we need to invoke the RenderingTearDown notifier.
        if !self.rendering_first_time.get() {
            if let Some(callback) = self.rendering_notifier.borrow_mut().as_mut() {
                surface
                    .with_active_surface(&mut || {
                        surface.with_graphics_api(&mut |api| {
                            callback.notify(RenderingState::RenderingTeardown, &api)
                        })
                    })
                    .ok();
            }
        }

        drop(surface);
    }

    /// Suspends the renderer by freeing all graphics related resources as well as the underlying
    /// rendering surface. Call [`Self::set_window_handle()`] to re-associate the renderer with a new
    /// window surface for subsequent rendering.
    pub fn suspend(&self) -> Result<(), PlatformError> {
        self.image_cache.clear_all();
        self.path_cache.clear_all();
        // Destroy the old surface before allocating the new one, to work around
        // the vivante drivers using zwp_linux_explicit_synchronization_v1 and
        // trying to create a second synchronization object and that's not allowed.
        self.clear_surface();
        Ok(())
    }

    /// Reset the surface to the window given the window handle
    pub fn set_window_handle(
        &self,
        window_handle: Arc<dyn raw_window_handle::HasWindowHandle + Send + Sync>,
        display_handle: Arc<dyn raw_window_handle::HasDisplayHandle + Send + Sync>,
        size: PhysicalWindowSize,
        requested_graphics_api: Option<RequestedGraphicsAPI>,
    ) -> Result<(), PlatformError> {
        // just in case
        self.suspend()?;
        let surface = (self.surface_factory)(
            &self.shared_context,
            window_handle,
            display_handle,
            size,
            requested_graphics_api,
        )?;
        self.set_surface(surface);
        Ok(())
    }

    /// Render the scene in the previously associated window.
    pub fn render(&self) -> Result<(), i_slint_core::platform::PlatformError> {
        let window_adapter = self.window_adapter()?;
        let size = window_adapter.window().size();
        self.internal_render_with_post_callback(0., (0., 0.), size, None)
    }

    fn internal_render_with_post_callback(
        &self,
        rotation_angle_degrees: f32,
        translation: (f32, f32),
        surface_size: PhysicalWindowSize,
        post_render_cb: Option<&dyn Fn(&mut dyn ItemRenderer)>,
    ) -> Result<(), i_slint_core::platform::PlatformError> {
        let surface = self.surface.borrow();
        let Some(surface) = surface.as_ref() else { return Ok(()) };
        if self.rendering_first_time.take() {
            *self.rendering_metrics_collector.borrow_mut() =
                RenderingMetricsCollector::new(&format!(
                    "Skia renderer (skia backend {}; surface: {} bpp)",
                    surface.name(),
                    surface.bits_per_pixel()?
                ));

            if let Some(callback) = self.rendering_notifier.borrow_mut().as_mut() {
                surface.with_graphics_api(&mut |api| {
                    callback.notify(RenderingState::RenderingSetup, &api)
                })
            }
        }

        let window_adapter = self.window_adapter()?;
        let window = window_adapter.window();

        surface.render(
            window,
            surface_size,
            &|skia_canvas, gr_context, back_buffer_age| {
                self.render_to_canvas(
                    skia_canvas,
                    rotation_angle_degrees,
                    translation,
                    gr_context,
                    back_buffer_age,
                    Some(surface.as_ref()),
                    window,
                    post_render_cb,
                )
            },
            &self.pre_present_callback,
        )
    }

    fn render_to_canvas(
        &self,
        skia_canvas: &skia_safe::Canvas,
        rotation_angle_degrees: f32,
        translation: (f32, f32),
        gr_context: Option<&mut skia_safe::gpu::DirectContext>,
        back_buffer_age: u8,
        surface: Option<&dyn Surface>,
        window: &i_slint_core::api::Window,
        post_render_cb: Option<&dyn Fn(&mut dyn ItemRenderer)>,
    ) -> Option<DirtyRegion> {
        skia_canvas.rotate(rotation_angle_degrees, None);
        skia_canvas.translate(translation);

        let window_inner = WindowInner::from_pub(window);

        let dirty_region = window_inner
            .draw_contents(|components| {
                self.render_components_to_canvas(
                    skia_canvas,
                    gr_context,
                    back_buffer_age,
                    surface,
                    window,
                    post_render_cb,
                    components,
                )
            })
            .unwrap_or_default();

        if let Some(callback) = self.rendering_notifier.borrow_mut().as_mut() {
            if let Some(surface) = surface {
                surface.with_graphics_api(&mut |api| {
                    callback.notify(RenderingState::AfterRendering, &api)
                })
            }
        }

        dirty_region
    }

    fn render_components_to_canvas(
        &self,
        skia_canvas: &skia_safe::Canvas,
        mut gr_context: Option<&mut skia_safe::gpu::DirectContext>,
        back_buffer_age: u8,
        surface: Option<&dyn Surface>,
        window: &i_slint_core::api::Window,
        post_render_cb: Option<&dyn Fn(&mut dyn ItemRenderer)>,
        components: &[(&i_slint_core::item_tree::ItemTreeRc, LogicalPoint)],
    ) -> Option<DirtyRegion> {
        let window_inner = WindowInner::from_pub(window);
        let window_adapter = window_inner.window_adapter();

        let mut box_shadow_cache = Default::default();

        self.image_cache.clear_cache_if_scale_factor_changed(window);
        self.path_cache.clear_cache_if_scale_factor_changed(window);

        let mut skia_item_renderer = itemrenderer::SkiaItemRenderer::new(
            skia_canvas,
            window,
            surface,
            &self.image_cache,
            &self.path_cache,
            &mut box_shadow_cache,
        );

        let scale_factor = ScaleFactor::new(window_inner.scale_factor());
        let logical_window_size = i_slint_core::lengths::logical_size_from_api(
            window.size().to_logical(window_inner.scale_factor()),
        );

        let mut dirty_region = None;

        {
            let mut item_renderer: &mut dyn ItemRenderer = &mut skia_item_renderer;
            let mut partial_renderer;
            let mut dirty_region_to_visualize = None;

            if let Some(partial_rendering_state) = self.partial_rendering_state() {
                partial_renderer =
                    partial_rendering_state.create_partial_renderer(skia_item_renderer);

                let mut dirty_region_history = self.dirty_region_history.borrow_mut();

                let buffer_dirty_region = if back_buffer_age > 0
                    && back_buffer_age as usize - 1 < dirty_region_history.len()
                {
                    // The dirty region is the union of all the previous dirty regions
                    Some(
                        dirty_region_history[0..back_buffer_age as usize - 1]
                            .iter()
                            .fold(DirtyRegion::default(), |acc, region| acc.union(region)),
                    )
                } else {
                    Some(LogicalRect::from_size(logical_window_size).into())
                };

                let dirty_region_for_this_frame = partial_rendering_state.apply_dirty_region(
                    &mut partial_renderer,
                    components,
                    logical_window_size,
                    buffer_dirty_region,
                );

                let mut clip_path = skia_safe::Path::new();

                for dirty_rect in partial_renderer.dirty_region.iter() {
                    let physical_rect = (dirty_rect * scale_factor).to_rect().round_out();
                    clip_path.add_rect(&to_skia_rect(&physical_rect), None);
                }

                if matches!(self.dirty_region_debug_mode, DirtyRegionDebugMode::Log) {
                    let area_to_repaint: f32 =
                        partial_renderer.dirty_region.iter().map(|b| b.area()).sum();
                    i_slint_core::debug_log!(
                        "repainting {:.2}%",
                        area_to_repaint * 100. / logical_window_size.area()
                    );
                }

                dirty_region = partial_renderer.dirty_region.clone().into();

                dirty_region_history.rotate_right(1);
                dirty_region_history[0] = dirty_region_for_this_frame;

                skia_canvas.clip_path(&clip_path, None, false);

                if matches!(self.dirty_region_debug_mode, DirtyRegionDebugMode::Visualize) {
                    dirty_region_to_visualize = Some(clip_path);
                }

                item_renderer = &mut partial_renderer;
            }

            if let Some(window_item_rc) = window_inner.window_item_rc() {
                let window_item =
                    window_item_rc.downcast::<i_slint_core::items::WindowItem>().unwrap();
                match window_item.as_pin_ref().background() {
                    Brush::SolidColor(clear_color) => {
                        skia_canvas.clear(itemrenderer::to_skia_color(&clear_color));
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

            if let Some(callback) = self.rendering_notifier.borrow_mut().as_mut() {
                // For the BeforeRendering rendering notifier callback it's important that this happens *after* clearing
                // the back buffer, in order to allow the callback to provide its own rendering of the background.
                // Skia's clear() will merely schedule a clear call, so flush right away to make it immediate.
                if let Some(ctx) = gr_context.as_mut() {
                    ctx.flush(None);
                }

                if let Some(surface) = surface {
                    surface.with_graphics_api(&mut |api| {
                        callback.notify(RenderingState::BeforeRendering, &api)
                    })
                }
            }

            for (component, origin) in components {
                i_slint_core::item_rendering::render_component_items(
                    component,
                    item_renderer,
                    *origin,
                    &window_adapter,
                );
            }

            if let Some(path) = dirty_region_to_visualize {
                let mut paint = skia_safe::Paint::new(
                    &skia_safe::Color4f { a: 0.5, r: 1.0, g: 0., b: 0. },
                    None,
                );
                paint.set_style(skia_safe::PaintStyle::Stroke);
                skia_canvas.draw_path(&path, &paint);
            }

            if let Some(collector) = &self.rendering_metrics_collector.borrow_mut().as_ref() {
                collector.measure_frame_rendered(item_renderer);
                if collector.refresh_mode()
                    == i_slint_core::graphics::rendering_metrics_collector::RefreshMode::FullSpeed
                {
                    if let Some(partial_rendering_state) = self.partial_rendering_state() {
                        partial_rendering_state.force_screen_refresh();
                    }
                }
            }

            if let Some(cb) = post_render_cb.as_ref() {
                cb(item_renderer)
            }
        }

        if let Some(ctx) = gr_context.as_mut() {
            ctx.flush(None);
        }

        dirty_region
    }

    fn window_adapter(&self) -> Result<Rc<dyn WindowAdapter>, PlatformError> {
        self.maybe_window_adapter.borrow().as_ref().and_then(|w| w.upgrade()).ok_or_else(|| {
            "Renderer must be associated with component before use".to_string().into()
        })
    }

    /// Sets the specified callback, that's invoked before presenting the rendered buffer to the windowing system.
    /// This can be useful to implement frame throttling, i.e. for requesting a frame callback from the wayland compositor.
    pub fn set_pre_present_callback(&self, callback: Option<Box<dyn FnMut()>>) {
        *self.pre_present_callback.borrow_mut() = callback;
    }

    fn partial_rendering_state(&self) -> Option<&PartialRenderingState> {
        // We don't know where the application might render to, so disable partial rendering.
        if self.rendering_notifier.borrow().is_some() {
            None
        } else {
            self.partial_rendering_state.as_ref()
        }
    }
}

impl i_slint_core::renderer::RendererSealed for SkiaRenderer {
    fn text_size(
        &self,
        font_request: i_slint_core::graphics::FontRequest,
        text: &str,
        max_width: Option<LogicalLength>,
        scale_factor: ScaleFactor,
        _text_wrap: TextWrap, //TODO: Add support for char-wrap
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
            TextWrap::WordWrap,
            Default::default(),
            None,
        );

        PhysicalSize::new(layout.max_intrinsic_width().ceil(), layout.height().ceil())
            / scale_factor
    }

    fn font_metrics(
        &self,
        font_request: i_slint_core::graphics::FontRequest,
        scale_factor: ScaleFactor,
    ) -> i_slint_core::items::FontMetrics {
        textlayout::font_metrics(font_request, scale_factor)
    }

    fn text_input_byte_offset_for_position(
        &self,
        text_input: std::pin::Pin<&i_slint_core::items::TextInput>,
        pos: LogicalPoint,
        font_request: FontRequest,
        scale_factor: ScaleFactor,
    ) -> usize {
        let max_width = text_input.width() * scale_factor;
        let max_height = text_input.height() * scale_factor;
        let pos = pos * scale_factor;

        if max_width.get() <= 0. || max_height.get() <= 0. {
            return 0;
        }

        let visual_representation = text_input.visual_representation(None);

        let (layout, layout_top_left) = textlayout::create_layout(
            font_request,
            scale_factor,
            &visual_representation.text,
            None,
            Some(max_width),
            max_height,
            text_input.horizontal_alignment(),
            text_input.vertical_alignment(),
            text_input.wrap(),
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
        font_request: FontRequest,
        scale_factor: ScaleFactor,
    ) -> LogicalRect {
        let max_width = text_input.width() * scale_factor;
        let max_height = text_input.height() * scale_factor;

        if max_width.get() <= 0. || max_height.get() <= 0. {
            return Default::default();
        }

        let string = text_input.text();
        let string = string.as_str();

        let (layout, layout_top_left) = textlayout::create_layout(
            font_request,
            scale_factor,
            string,
            None,
            Some(max_width),
            max_height,
            text_input.horizontal_alignment(),
            text_input.vertical_alignment(),
            text_input.wrap(),
            i_slint_core::items::TextOverflow::Clip,
            None,
        );

        let physical_cursor_rect = textlayout::cursor_rect(
            string,
            byte_offset,
            layout,
            text_input.text_cursor_width() * scale_factor,
            text_input.horizontal_alignment(),
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
        component: i_slint_core::item_tree::ItemTreeRef,
        items: &mut dyn Iterator<Item = std::pin::Pin<i_slint_core::items::ItemRef<'_>>>,
    ) -> Result<(), i_slint_core::platform::PlatformError> {
        self.image_cache.component_destroyed(component);
        self.path_cache.component_destroyed(component);

        if let Some(partial_rendering_state) = self.partial_rendering_state() {
            partial_rendering_state.free_graphics_resources(items);
        }

        Ok(())
    }

    fn set_window_adapter(&self, window_adapter: &Rc<dyn WindowAdapter>) {
        *self.maybe_window_adapter.borrow_mut() = Some(Rc::downgrade(window_adapter));
        self.image_cache.clear_all();
        self.path_cache.clear_all();

        if let Some(partial_rendering_state) = self.partial_rendering_state() {
            partial_rendering_state.clear_cache();
        }
    }

    fn resize(&self, size: i_slint_core::api::PhysicalSize) -> Result<(), PlatformError> {
        if let Some(surface) = self.surface.borrow().as_ref() {
            surface.resize_event(size)
        } else {
            Ok(())
        }
    }

    /// Returns an image buffer of what was rendered last by reading the previous front buffer (using glReadPixels).
    fn take_snapshot(
        &self,
    ) -> Result<SharedPixelBuffer<i_slint_core::graphics::Rgba8Pixel>, PlatformError> {
        let window_adapter = self.window_adapter()?;
        let window = window_adapter.window();
        let size = window_adapter.window().size();
        let (width, height) = (size.width, size.height);
        let mut target_buffer =
            SharedPixelBuffer::<i_slint_core::graphics::Rgba8Pixel>::new(width, height);

        let mut surface_borrow = skia_safe::surfaces::wrap_pixels(
            &skia_safe::ImageInfo::new(
                (width as i32, height as i32),
                skia_safe::ColorType::RGBA8888,
                skia_safe::AlphaType::Opaque,
                None,
            ),
            target_buffer.make_mut_bytes(),
            None,
            None,
        )
        .ok_or_else(|| "Error wrapping target buffer for rendering into with Skia".to_string())?;

        self.render_to_canvas(surface_borrow.canvas(), 0., (0.0, 0.0), None, 0, None, window, None);

        Ok(target_buffer)
    }

    fn mark_dirty_region(&self, region: i_slint_core::item_rendering::DirtyRegion) {
        if let Some(partial_rendering_state) = self.partial_rendering_state() {
            partial_rendering_state.mark_dirty_region(region);
        }
    }

    fn supports_transformations(&self) -> bool {
        true
    }
}

impl Drop for SkiaRenderer {
    fn drop(&mut self) {
        self.clear_surface()
    }
}

/// This trait represents the interface between the Skia renderer and the underlying rendering surface, such as a window
/// with a metal layer, a wayland window with an OpenGL context, etc.
pub trait Surface {
    /// Creates a new surface with the given window, display, and size.
    fn new(
        shared_context: &SkiaSharedContext,
        window_handle: Arc<dyn raw_window_handle::HasWindowHandle + Sync + Send>,
        display_handle: Arc<dyn raw_window_handle::HasDisplayHandle + Sync + Send>,
        size: PhysicalWindowSize,
        requested_graphics_api: Option<RequestedGraphicsAPI>,
    ) -> Result<Self, PlatformError>
    where
        Self: Sized;
    /// Returns the name of the surface, for diagnostic purposes.
    fn name(&self) -> &'static str;

    /// If supported, this invokes the specified callback with access to the platform graphics API.
    fn with_graphics_api(&self, _callback: &mut dyn FnMut(GraphicsAPI<'_>)) {}
    /// Invokes the callback with the surface active. This has only a meaning for OpenGL rendering, where
    /// the implementation must make the GL context current.
    fn with_active_surface(
        &self,
        callback: &mut dyn FnMut(),
    ) -> Result<(), i_slint_core::platform::PlatformError> {
        callback();
        Ok(())
    }
    /// Prepares the surface for rendering and invokes the provided callback with access to a Skia canvas and
    /// rendering context.
    fn render(
        &self,
        window: &Window,
        size: PhysicalWindowSize,
        render_callback: &dyn Fn(
            &skia_safe::Canvas,
            Option<&mut skia_safe::gpu::DirectContext>,
            u8,
        ) -> Option<DirtyRegion>,
        pre_present_callback: &RefCell<Option<Box<dyn FnMut()>>>,
    ) -> Result<(), i_slint_core::platform::PlatformError>;
    /// Called when the surface should be resized.
    fn resize_event(
        &self,
        size: PhysicalWindowSize,
    ) -> Result<(), i_slint_core::platform::PlatformError>;
    fn bits_per_pixel(&self) -> Result<u8, PlatformError>;

    fn use_partial_rendering(&self) -> bool {
        false
    }

    fn import_opengl_texture(
        &self,
        _canvas: &skia_safe::Canvas,
        _texture: &i_slint_core::graphics::BorrowedOpenGLTexture,
    ) -> Option<skia_safe::Image> {
        None
    }

    #[cfg(feature = "unstable-wgpu-26")]
    fn import_wgpu_texture(
        &self,
        _canvas: &skia_safe::Canvas,
        _texture: &i_slint_core::graphics::WGPUTexture,
    ) -> Option<skia_safe::Image> {
        None
    }

    /// Implementations should return self to allow upcasting.
    fn as_any(&self) -> &dyn core::any::Any {
        &()
    }
}

pub trait SkiaRendererExt {
    fn render_transformed_with_post_callback(
        &self,
        rotation_angle_degrees: f32,
        translation: (f32, f32),
        surface_size: PhysicalWindowSize,
        post_render_cb: Option<&dyn Fn(&mut dyn ItemRenderer)>,
    ) -> Result<(), i_slint_core::platform::PlatformError>;
}

impl SkiaRendererExt for SkiaRenderer {
    fn render_transformed_with_post_callback(
        &self,
        rotation_angle_degrees: f32,
        translation: (f32, f32),
        surface_size: PhysicalWindowSize,
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
