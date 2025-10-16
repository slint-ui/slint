// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

#![doc = include_str!("README.md")]
#![doc(html_logo_url = "https://slint.dev/logo/slint-logo-square-light.svg")]

use std::cell::{Cell, RefCell};
use std::num::NonZeroU32;
use std::pin::Pin;
use std::rc::{Rc, Weak};
use std::sync::Arc;

use i_slint_common::sharedfontique;
use i_slint_core::Brush;
use i_slint_core::api::PhysicalSize as PhysicalWindowSize;
use i_slint_core::api::{RenderingNotifier, RenderingState, SetRenderingNotifierError};
use i_slint_core::graphics::{BorderRadius, RequestedGraphicsAPI, Rgba8Pixel};
use i_slint_core::graphics::{FontRequest, SharedPixelBuffer};
use i_slint_core::graphics::{euclid, rendering_metrics_collector::RenderingMetricsCollector};
use i_slint_core::item_rendering::ItemRenderer;
use i_slint_core::item_tree::ItemTreeWeak;
use i_slint_core::items::TextWrap;
use i_slint_core::lengths::{
    LogicalLength, LogicalPoint, LogicalRect, LogicalSize, PhysicalPx, ScaleFactor,
};
use i_slint_core::platform::PlatformError;
use i_slint_core::renderer::RendererSealed;
use i_slint_core::textlayout::sharedparley;
use i_slint_core::window::{WindowAdapter, WindowInner};

type PhysicalLength = euclid::Length<f32, PhysicalPx>;
type PhysicalRect = euclid::Rect<f32, PhysicalPx>;
type PhysicalSize = euclid::Size2D<f32, PhysicalPx>;
type PhysicalPoint = euclid::Point2D<f32, PhysicalPx>;
type PhysicalBorderRadius = BorderRadius<f32, PhysicalPx>;

use wgpu_26 as wgpu;

mod itemrenderer;

pub struct VelloRenderer {
    maybe_window_adapter: RefCell<Option<Weak<dyn WindowAdapter>>>,
    rendering_notifier: RefCell<Option<Box<dyn RenderingNotifier>>>,
    rendering_metrics_collector: RefCell<Option<Rc<RenderingMetricsCollector>>>,
    instance: RefCell<Option<wgpu::Instance>>,
    device: RefCell<Option<wgpu::Device>>,
    queue: RefCell<Option<wgpu::Queue>>,
    surface_config: RefCell<Option<wgpu::SurfaceConfiguration>>,
    surface: RefCell<Option<wgpu::Surface<'static>>>,
    blitter: RefCell<Option<wgpu::util::TextureBlitter>>,
    target_texture: RefCell<Option<wgpu::Texture>>,
    renderer: RefCell<Option<vello::Renderer>>,
    scene: RefCell<Option<vello::Scene>>,
}

impl VelloRenderer {
    pub fn new() -> Self {
        Self {
            maybe_window_adapter: Default::default(),
            rendering_notifier: Default::default(),
            rendering_metrics_collector: Default::default(),
            instance: Default::default(),
            device: Default::default(),
            queue: Default::default(),
            surface_config: Default::default(),
            surface: Default::default(),
            blitter: Default::default(),
            target_texture: Default::default(),
            renderer: Default::default(),
            scene: Default::default(),
        }
    }

    pub fn resume(
        &self,
        window_handle: Box<dyn wgpu::WindowHandle>,
        size: PhysicalWindowSize,
        requested_graphics_api: Option<RequestedGraphicsAPI>,
    ) -> Result<(), PlatformError> {
        let (instance, adapter, device, queue, surface) =
            i_slint_core::graphics::wgpu_26::init_instance_adapter_device_queue_surface(
                window_handle,
                requested_graphics_api,
                /* rendering artifacts :( */
                wgpu::Backends::GL,
            )?;

        let mut surface_config =
            surface.get_default_config(&adapter, size.width, size.height).unwrap();

        let swapchain_capabilities = surface.get_capabilities(&adapter);
        let swapchain_format = swapchain_capabilities
            .formats
            .iter()
            .find(|it| {
                matches!(it, wgpu::TextureFormat::Rgba8Unorm | wgpu::TextureFormat::Bgra8Unorm)
            })
            .copied()
            .unwrap_or_else(|| swapchain_capabilities.formats[0]);
        surface_config.format = swapchain_format;
        surface.configure(&device, &surface_config);

        *self.instance.borrow_mut() = Some(instance.clone());
        *self.device.borrow_mut() = Some(device.clone());
        *self.queue.borrow_mut() = Some(queue.clone());
        *self.surface_config.borrow_mut() = Some(surface_config);
        *self.surface.borrow_mut() = Some(surface);
        *self.blitter.borrow_mut() =
            Some(wgpu::util::TextureBlitter::new(&device, swapchain_format));

        *self.target_texture.borrow_mut() = Some(device.create_texture(&wgpu::TextureDescriptor {
            label: None,
            size: wgpu::Extent3d {
                width: size.width,
                height: size.height,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            usage: wgpu::TextureUsages::STORAGE_BINDING | wgpu::TextureUsages::TEXTURE_BINDING,
            format: wgpu::TextureFormat::Rgba8Unorm,
            view_formats: &[],
        }));

        *self.renderer.borrow_mut() = Some(
            vello::Renderer::new(&device, vello::RendererOptions::default())
                .map_err(|e| format!("Error creating vello renderer: {e}"))?,
        );

        *self.scene.borrow_mut() = Some(vello::Scene::new());

        Ok(())
    }

    pub fn suspend(&self) -> Result<(), PlatformError> {
        todo!()
    }

    pub fn render(&self) -> Result<(), PlatformError> {
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
    ) -> Result<(), PlatformError> {
        /*
        let surface = self.graphics_backend.begin_surface_rendering()?;

        if self.rendering_first_time.take() {
            *self.rendering_metrics_collector.borrow_mut() =
                RenderingMetricsCollector::new("Vello renderer");

            if let Some(callback) = self.rendering_notifier.borrow_mut().as_mut() {
                self.with_graphics_api(|api| {
                    callback.notify(RenderingState::RenderingSetup, &api)
                })?;
            }
        }
        */

        let window_adapter = self.window_adapter()?;
        let window = window_adapter.window();
        let window_size = window.size();

        let Some((width, height)): Option<(NonZeroU32, NonZeroU32)> =
            window_size.width.try_into().ok().zip(window_size.height.try_into().ok())
        else {
            // Nothing to render
            return Ok(());
        };

        let mut scene = self.scene.borrow_mut();
        let Some(mut scene) = scene.as_mut() else { return Ok(()) };
        scene.reset();

        let window_inner = WindowInner::from_pub(window);
        let scale = window_inner.scale_factor().ceil();

        let surface_texture = self
            .surface
            .borrow()
            .as_ref()
            .unwrap()
            .get_current_texture()
            .expect("unable to get next texture from swapchain");

        window_inner
            .draw_contents(|components| -> Result<(), PlatformError> {
                let mut item_renderer = itemrenderer::VelloItemRenderer::new(
                    scene,
                    surface_size.width,
                    surface_size.height,
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

                Ok(())
            })
            .unwrap_or(Ok(()))?;

        let device = self.device.borrow().clone().unwrap();

        let render_params = vello::RenderParams {
            base_color: vello::peniko::color::palette::css::BLUE,
            width: surface_size.width,
            height: surface_size.height,
            antialiasing_method: vello::AaConfig::Area,
        };

        let target_view = self
            .target_texture
            .borrow()
            .as_ref()
            .unwrap()
            .create_view(&wgpu::TextureViewDescriptor::default());

        self.renderer
            .borrow_mut()
            .as_mut()
            .unwrap()
            .render_to_texture(
                &device,
                &*self.queue.borrow().as_ref().unwrap(),
                &scene,
                &target_view,
                &render_params,
            )
            .expect("failed to render to texture");

        let mut encoder = self.device.borrow().as_ref().unwrap().create_command_encoder(
            &wgpu::CommandEncoderDescriptor { label: Some("Surface Blit") },
        );
        self.blitter.borrow().as_ref().unwrap().copy(
            &self.device.borrow().as_ref().unwrap(),
            &mut encoder,
            &target_view,
            &surface_texture.texture.create_view(&wgpu::TextureViewDescriptor::default()),
        );

        self.queue.borrow().as_ref().unwrap().submit([encoder.finish()]);

        if let Some(callback) = self.rendering_notifier.borrow_mut().as_mut() {
            self.with_graphics_api(|api| callback.notify(RenderingState::AfterRendering, &api))?;
        }

        surface_texture.present();

        /*

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
                    if let Some(component) = ItemTreeWeak::upgrade(component) {
                        i_slint_core::item_rendering::render_component_items(
                            &component,
                            &mut item_renderer,
                            *origin,
                            &self.window_adapter()?,
                        );
                    }
                }

                if let Some(cb) = post_render_cb.as_ref() {
                    cb(&mut item_renderer)
                }

                if let Some(collector) = &self.rendering_metrics_collector.borrow().as_ref() {
                    let metrics = item_renderer.metrics();
                    collector.measure_frame_rendered(&mut item_renderer, metrics);
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
        */
        Ok(())
    }

    fn with_graphics_api(
        &self,
        callback: impl FnOnce(i_slint_core::api::GraphicsAPI<'_>),
    ) -> Result<(), PlatformError> {
        unimplemented!()
        //self.graphics_backend.with_graphics_api(|api| callback(api.unwrap()))
    }

    fn window_adapter(&self) -> Result<Rc<dyn WindowAdapter>, PlatformError> {
        self.maybe_window_adapter.borrow().as_ref().and_then(|w| w.upgrade()).ok_or_else(|| {
            "Renderer must be associated with component before use".to_string().into()
        })
    }
}

#[doc(hidden)]
impl RendererSealed for VelloRenderer {
    fn text_size(
        &self,
        text_item: Pin<&dyn i_slint_core::item_rendering::RenderString>,
        item_rc: &i_slint_core::items::ItemRc,
        max_width: Option<LogicalLength>,
        text_wrap: TextWrap,
    ) -> LogicalSize {
        sharedparley::text_size(self, text_item, item_rc, max_width, text_wrap)
    }

    fn char_size(
        &self,
        text_item: Pin<&dyn i_slint_core::item_rendering::HasFont>,
        item_rc: &i_slint_core::item_tree::ItemRc,
        ch: char,
    ) -> LogicalSize {
        sharedparley::char_size(text_item, item_rc, ch).unwrap_or_default()
    }

    fn font_metrics(
        &self,
        font_request: i_slint_core::graphics::FontRequest,
    ) -> i_slint_core::items::FontMetrics {
        sharedparley::font_metrics(font_request)
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
        sharedfontique::get_collection().register_fonts(data.to_vec().into(), None);
        Ok(())
    }

    fn register_font_from_path(
        &self,
        path: &std::path::Path,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let requested_path = path.canonicalize().unwrap_or_else(|_| path.into());
        let contents = std::fs::read(requested_path)?;
        sharedfontique::get_collection().register_fonts(contents.into(), None);
        Ok(())
    }

    fn default_font_size(&self) -> LogicalLength {
        sharedparley::DEFAULT_FONT_SIZE
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
    ) -> Result<(), PlatformError> {
        /*
        if !self.graphics_cache.is_empty() {
            self.graphics_backend.with_graphics_api(|_| {
                self.graphics_cache.component_destroyed(component);
            })?;
        }
        */
        Ok(())
    }

    fn set_window_adapter(&self, window_adapter: &Rc<dyn WindowAdapter>) {
        *self.maybe_window_adapter.borrow_mut() = Some(Rc::downgrade(window_adapter));
        /*
        self.graphics_backend
            .with_graphics_api(|_| {
                self.graphics_cache.clear_all();
                self.texture_cache.borrow_mut().clear();
            })
            .ok();
        */
    }

    fn window_adapter(&self) -> Option<Rc<dyn WindowAdapter>> {
        self.maybe_window_adapter
            .borrow()
            .as_ref()
            .and_then(|window_adapter| window_adapter.upgrade())
    }

    fn resize(&self, size: i_slint_core::api::PhysicalSize) -> Result<(), PlatformError> {
        /*
        if let Some((width, height)) = size.width.try_into().ok().zip(size.height.try_into().ok()) {
            self.graphics_backend.resize(width, height)?;
        };
        */
        Ok(())
    }

    /// Returns an image buffer of what was rendered last by reading the previous front buffer (using glReadPixels).
    fn take_snapshot(&self) -> Result<SharedPixelBuffer<Rgba8Pixel>, PlatformError> {
        todo!()
        /*
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
        */
    }

    fn supports_transformations(&self) -> bool {
        true
    }
}

/*
impl<B: GraphicsBackend> Drop for VelloRenderer<B> {
    fn drop(&mut self) {
        self.clear_graphics_context().ok();
    }
}
    */

/// The purpose of this trait is to add internal API that's accessed from the winit/linuxkms backends, but not
/// public (as the trait isn't re-exported).
#[doc(hidden)]
pub trait VelloRendererExt {
    fn clear_graphics_context(&self) -> Result<(), PlatformError>;
    fn render_transformed_with_post_callback(
        &self,
        rotation_angle_degrees: f32,
        translation: (f32, f32),
        surface_size: i_slint_core::api::PhysicalSize,
        post_render_cb: Option<&dyn Fn(&mut dyn ItemRenderer)>,
    ) -> Result<(), PlatformError>;
}

#[doc(hidden)]
impl VelloRendererExt for VelloRenderer {
    fn clear_graphics_context(&self) -> Result<(), PlatformError> {
        /*
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
        */

        Ok(())
    }

    fn render_transformed_with_post_callback(
        &self,
        rotation_angle_degrees: f32,
        translation: (f32, f32),
        surface_size: i_slint_core::api::PhysicalSize,
        post_render_cb: Option<&dyn Fn(&mut dyn ItemRenderer)>,
    ) -> Result<(), PlatformError> {
        self.internal_render_with_post_callback(
            rotation_angle_degrees,
            translation,
            surface_size,
            post_render_cb,
        )
    }
}
