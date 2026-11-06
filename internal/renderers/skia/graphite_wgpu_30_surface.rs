// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

//! Skia Graphite on wgpu 30, through Skia's Dawn backend that `skia-wgpu` runs on wgpu.
//!
//! All of Skia's GPU work goes through wgpu, so wgpu tracks the usage of every texture,
//! including the swap chain's.

#[cfg(feature = "unstable-wgpu-30")]
use i_slint_core::api::GraphicsAPI;
use i_slint_core::api::{PhysicalSize as PhysicalWindowSize, Window};
use i_slint_core::graphics::RequestedGraphicsAPI;
use i_slint_core::partial_renderer::DirtyRegion;
use i_slint_core::platform::PlatformError;
use i_slint_core::renderer::DrawOutcome;

use std::cell::{Cell, RefCell};
use std::collections::HashMap;
use std::rc::Rc;
use std::sync::Arc;

use skia_safe::gpu::graphite;
use wgpu_30 as wgpu;

use crate::SkiaSharedContext;

/// Number of frames an uploaded image stays cached without being drawn.
const IMAGE_CACHE_FRAMES: u64 = 120;

/// Uploads the raster images that Slint draws, because Graphite drops the draw of an image
/// that doesn't live on the GPU.
#[derive(Default)]
struct ImageUploadCache {
    frame: Cell<u64>,
    textures: RefCell<HashMap<(u32, bool), (skia_safe::Image, u64)>>,
}

impl ImageUploadCache {
    fn end_frame(&self) {
        let frame = self.frame.get();
        self.textures
            .borrow_mut()
            .retain(|_, (_, last_used)| frame - *last_used < IMAGE_CACHE_FRAMES);
        self.frame.set(frame + 1);
    }
}

struct ImageProvider(Rc<ImageUploadCache>);

impl graphite::ImageProvider for ImageProvider {
    fn find_or_create(
        &self,
        recorder: &graphite::BorrowedRecorder,
        image: &skia_safe::Image,
        required: skia_safe::image::RequiredProperties,
    ) -> Option<skia_safe::Image> {
        let key = (image.unique_id(), required.mipmapped);
        let frame = self.0.frame.get();
        if let Some((texture, last_used)) = self.0.textures.borrow_mut().get_mut(&key) {
            *last_used = frame;
            return Some(texture.clone());
        }
        let texture = recorder.texture_from_image(image, required)?;
        self.0.textures.borrow_mut().insert(key, (texture.clone(), frame));
        Some(texture)
    }
}

/// The color type that describes `format` to Skia when it samples a texture.
#[cfg(feature = "unstable-wgpu-30")]
fn sampled_color_type(format: wgpu::TextureFormat) -> Option<skia_safe::ColorType> {
    match format {
        wgpu::TextureFormat::Rgba8UnormSrgb => Some(skia_safe::ColorType::SRGBA8888),
        format => color_type(format),
    }
}

/// The color type that describes `format` to Skia when it renders into a texture.
fn color_type(format: wgpu::TextureFormat) -> Option<skia_safe::ColorType> {
    use wgpu::TextureFormat::*;
    match format {
        Rgba8Unorm | Rgba8UnormSrgb => Some(skia_safe::ColorType::RGBA8888),
        Bgra8Unorm | Bgra8UnormSrgb => Some(skia_safe::ColorType::BGRA8888),
        Rgba16Float => Some(skia_safe::ColorType::RGBAF16),
        Rgb10a2Unorm => Some(skia_safe::ColorType::RGBA1010102),
        _ => None,
    }
}

struct WgpuState {
    instance: wgpu::Instance,
    device: wgpu::Device,
    queue: wgpu::Queue,
}

/// Skia Graphite rendering surface backed by wgpu: either a window's swap chain, or textures
/// that the caller provides.
pub struct GraphiteWGPUSurface {
    // Dropped before the context, and both before the wgpu device.
    recorder: RefCell<graphite::Recorder>,
    context: RefCell<graphite::Context>,
    images: Rc<ImageUploadCache>,
    surface_config: RefCell<Option<wgpu::SurfaceConfiguration>>,
    surface: Option<wgpu::Surface<'static>>,
    wgpu: WgpuState,
}

impl GraphiteWGPUSurface {
    fn new_with_parts(
        wgpu: WgpuState,
        surface: Option<(wgpu::Surface<'static>, wgpu::SurfaceConfiguration)>,
    ) -> Result<Self, PlatformError> {
        let mut context =
            graphite::wgpu::context_factory::make_wgpu(&wgpu.device, &wgpu.queue, None)
                .ok_or_else(|| PlatformError::from("Failed to create a Skia Graphite context"))?;
        let images = Rc::new(ImageUploadCache::default());
        let mut options = graphite::RecorderOptions::new();
        options.set_image_provider(ImageProvider(images.clone()));
        let recorder = context
            .make_recorder(Some(&options))
            .ok_or_else(|| PlatformError::from("Failed to create a Skia Graphite recorder"))?;
        let (surface, surface_config) = surface.unzip();
        Ok(Self {
            recorder: recorder.into(),
            context: context.into(),
            images,
            surface_config: surface_config.into(),
            surface,
            wgpu,
        })
    }

    pub(crate) fn new_offscreen(
        instance: wgpu::Instance,
        device: wgpu::Device,
        queue: wgpu::Queue,
    ) -> Result<Self, PlatformError> {
        Self::new_with_parts(WgpuState { instance, device, queue }, None)
    }

    /// Draws with `callback` into `texture`, and submits the work to the queue.
    pub(crate) fn render_to_texture(
        &self,
        texture: &wgpu::Texture,
        callback: &mut dyn FnMut(&skia_safe::Canvas) -> Result<(), PlatformError>,
    ) -> Result<(), PlatformError> {
        let color_type = color_type(texture.format()).ok_or_else(|| {
            PlatformError::from(format!("Unsupported texture format {:?}", texture.format()))
        })?;
        let color_space = crate::attachment_color_space(
            crate::TextureEncoding::from_format_is_srgb(texture.format().is_srgb()),
        );
        let mut skia_surface = graphite::wgpu::surfaces::wrap_texture(
            &mut self.recorder.borrow_mut(),
            texture,
            color_type,
            color_space,
            None,
        )
        .ok_or_else(|| PlatformError::from("Failed to wrap the wgpu texture for Skia"))?;
        // `import_wgpu_texture` borrows the recorder while the callback draws.
        callback(skia_surface.canvas())?;
        drop(skia_surface);
        let mut recording = self
            .recorder
            .borrow_mut()
            .snap()
            .ok_or_else(|| PlatformError::from("Skia recorded nothing"))?;
        let mut context = self.context.borrow_mut();
        context.insert_recording(&graphite::InsertRecordingInfo::new(&mut recording));
        if !context.submit(None) {
            return Err("Skia renderer: submitting the rendering to the GPU failed".into());
        }
        self.images.end_frame();
        Ok(())
    }
}

impl crate::Surface for GraphiteWGPUSurface {
    fn new(
        _shared_context: &SkiaSharedContext,
        window_handle: Arc<dyn raw_window_handle::HasWindowHandle + Send + Sync>,
        display_handle: Arc<dyn raw_window_handle::HasDisplayHandle + Send + Sync>,
        size: PhysicalWindowSize,
        requested_graphics_api: Option<RequestedGraphicsAPI>,
        _transparent: bool,
    ) -> Result<Self, PlatformError> {
        let (instance, adapter, device, queue, surface) =
            i_slint_core::graphics::wgpu_30::init_instance_adapter_device_queue_surface(
                Box::new(crate::wgpu_30_surface::WindowAndDisplayHandle(
                    window_handle,
                    display_handle,
                )) as Box<dyn wgpu::DisplayAndWindowHandle>,
                requested_graphics_api,
                i_slint_core::graphics::wgpu_30::default_backends_to_avoid(),
            )?;

        let mut surface_config = surface
            .get_default_config(&adapter, size.width.max(1), size.height.max(1))
            .ok_or_else(|| PlatformError::from("WGPU surface is not compatible with adapter"))?;
        let capabilities = surface.get_capabilities(&adapter);
        surface_config.format = capabilities
            .formats
            .iter()
            .copied()
            .find(|f| {
                matches!(f, wgpu::TextureFormat::Rgba8Unorm | wgpu::TextureFormat::Bgra8Unorm)
            })
            .unwrap_or(capabilities.formats[0]);
        // Skia copies or samples the backdrop of a backdrop blur, and without either usage
        // draws the blur over nothing.
        surface_config.usage |= capabilities.usages
            & (wgpu::TextureUsages::COPY_SRC | wgpu::TextureUsages::TEXTURE_BINDING);
        surface_config.present_mode = wgpu::PresentMode::AutoVsync;
        surface.configure(&device, &surface_config);

        Self::new_with_parts(WgpuState { instance, device, queue }, Some((surface, surface_config)))
    }

    fn name(&self) -> &'static str {
        if self.surface.is_some() { "graphite-wgpu" } else { "graphite-wgpu-texture" }
    }

    fn resize_event(&self, size: PhysicalWindowSize) -> Result<(), PlatformError> {
        let mut config = self.surface_config.borrow_mut();
        let (Some(config), Some(surface)) = (config.as_mut(), &self.surface) else {
            return Ok(());
        };
        if config.width != size.width || config.height != size.height {
            config.width = size.width;
            config.height = size.height;
            surface.configure(&self.wgpu.device, config);
        }
        Ok(())
    }

    fn render(
        &self,
        _window: &Window,
        _size: PhysicalWindowSize,
        callback: &dyn Fn(
            &skia_safe::Canvas,
            Option<&mut skia_safe::gpu::DirectContext>,
            u8,
        ) -> Result<Option<DirtyRegion>, PlatformError>,
        pre_present_callback: &RefCell<Option<Box<dyn FnMut()>>>,
    ) -> Result<DrawOutcome, PlatformError> {
        let (Some(surface), Some(surface_config)) = (&self.surface, &*self.surface_config.borrow())
        else {
            return Err("GraphiteWGPUSurface::render() called on offscreen surface".into());
        };

        let frame = match surface.get_current_texture() {
            wgpu::CurrentSurfaceTexture::Success(t) => t,
            wgpu::CurrentSurfaceTexture::Occluded => return Ok(DrawOutcome::Occluded),
            wgpu::CurrentSurfaceTexture::Timeout => return Ok(DrawOutcome::Timeout),
            wgpu::CurrentSurfaceTexture::Validation => {
                return Err("WGPU surface validation error in get_current_texture".into());
            }
            stale @ (wgpu::CurrentSurfaceTexture::Outdated
            | wgpu::CurrentSurfaceTexture::Suboptimal(_)
            | wgpu::CurrentSurfaceTexture::Lost) => {
                drop(stale);
                surface.configure(&self.wgpu.device, surface_config);
                match surface.get_current_texture() {
                    wgpu::CurrentSurfaceTexture::Success(t) => t,
                    _ => return Ok(DrawOutcome::Occluded),
                }
            }
        };

        self.render_to_texture(&frame.texture, &mut |canvas| {
            callback(canvas, None, 0).map(|_| ())
        })?;

        if let Some(pre_present_callback) = pre_present_callback.borrow_mut().as_mut() {
            pre_present_callback();
        }
        self.wgpu.queue.present(frame);
        Ok(DrawOutcome::Success)
    }

    fn bits_per_pixel(&self) -> Result<u8, PlatformError> {
        Ok(32)
    }

    #[cfg(feature = "unstable-wgpu-30")]
    fn with_graphics_api(&self, callback: &mut dyn FnMut(GraphicsAPI<'_>)) {
        callback(i_slint_core::graphics::create_graphics_api_wgpu_30(
            self.wgpu.instance.clone(),
            self.wgpu.device.clone(),
            self.wgpu.queue.clone(),
        ))
    }

    #[cfg(feature = "unstable-wgpu-30")]
    fn import_wgpu_texture(
        &self,
        _canvas: &skia_safe::Canvas,
        texture: &i_slint_core::graphics::WGPUTexture,
    ) -> Option<skia_safe::Image> {
        let texture = match texture {
            i_slint_core::graphics::WGPUTexture::WGPU30Texture(texture) => texture,
            #[allow(unreachable_patterns)]
            _ => return None,
        };
        let mut recorder = self.recorder.try_borrow_mut().ok()?;
        graphite::wgpu::images::wrap_texture(
            &mut recorder,
            texture,
            sampled_color_type(texture.format())?,
            skia_safe::AlphaType::Unpremul,
            crate::sampled_texture_color_space(crate::TextureEncoding::from_format_is_srgb(
                texture.format().is_srgb(),
            )),
        )
    }

    fn as_any(&self) -> &dyn core::any::Any {
        self
    }

    fn free_gpu_resources(&self) {
        self.images.textures.borrow_mut().clear();
    }
}
