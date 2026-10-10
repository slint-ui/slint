// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

//! [`SlintWindowRenderer`] implementation rendering through [`vello`] on WGPU.
//!
//! The WGPU instance, adapter, device and surface come from
//! [`i_slint_core::graphics::wgpu_30`], the same initialization the FemtoVG
//! and Skia renderers use, so a device requested through
//! `BackendSelector::require_wgpu_30()` is honored here too. Only the scene
//! recording is taken from [`anyrender_vello`]; its window renderer, which
//! sets WGPU up on its own, is not used.
//!
//! Use [`AnyrenderSlintRenderer::new_vello`] to obtain a Slint renderer, and
//! [`resume_window`](AnyrenderSlintRenderer::resume_window) /
//! [`suspend_window`](AnyrenderSlintRenderer::suspend_window) to attach it to
//! a window from the windowing backend's event handling.

// cSpell: ignore blitted blitter msaa readback Texel unpadded unpremultiplied winsys

use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;
use std::sync::Arc;

use anyrender::{WindowHandle, WindowRenderer};
use anyrender_vello::VelloScenePainter;
use i_slint_core::api::PhysicalSize;
use i_slint_core::graphics::wgpu_30::wgpu;
use i_slint_core::graphics::{RequestedGraphicsAPI, Rgba8Pixel, SharedPixelBuffer};
use i_slint_core::platform::PlatformError;
use i_slint_core::renderer::DrawOutcome;

use crate::{AnyrenderSlintRenderer, SlintWindowRenderer, TextureImporter};

/// vello rasterizes with a compute pipeline into a storage texture, so frames
/// are rendered into an intermediate texture of this format and blitted to the
/// surface afterwards. This is what vello recommends: a surface texture rarely
/// permits `STORAGE_BINDING` in the first place, and even where it does, some
/// GPUs optimize for a surface that is never written by a compute pipeline.
const TARGET_TEXTURE_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rgba8Unorm;

/// vello's compute pipelines are compiled for the anti-aliasing methods
/// declared here, so build only the one that is actually used.
const ANTIALIASING_METHOD: vello::AaConfig = vello::AaConfig::Area;

fn antialiasing_support() -> vello::AaSupport {
    vello::AaSupport { area: true, msaa8: false, msaa16: false }
}

fn renderer_options() -> vello::RendererOptions {
    vello::RendererOptions {
        antialiasing_support: antialiasing_support(),
        use_cpu: false,
        num_init_threads: None,
        pipeline_cache: None,
    }
}

/// The intermediate texture vello renders into, together with the view used
/// as its render target.
struct TargetTexture {
    view: wgpu::TextureView,
    width: u32,
    height: u32,
}

impl TargetTexture {
    fn new(device: &wgpu::Device, width: u32, height: u32) -> Self {
        let texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("slint vello target"),
            size: wgpu::Extent3d { width, height, depth_or_array_layers: 1 },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: TARGET_TEXTURE_FORMAT,
            usage: wgpu::TextureUsages::STORAGE_BINDING | wgpu::TextureUsages::TEXTURE_BINDING,
            view_formats: &[],
        });
        Self { view: texture.create_view(&wgpu::TextureViewDescriptor::default()), width, height }
    }
}

/// A WGPU texture that a Slint image wraps, drawn through
/// [`vello::Renderer::override_image`]: vello copies the texture into its image
/// atlas wherever a scene draws [`Self::image`].
struct ImportedTexture {
    /// Stands in for the texture in the scene. Its blob is empty and only
    /// identifies the override.
    image: peniko::ImageData,
    /// vello copies from this texture, which needs `COPY_SRC`. Textures
    /// imported into Slint aren't required to have it, so those are blitted
    /// into a copy that has it, once per frame.
    staging: Option<wgpu::Texture>,
    registered: bool,
    used_in_frame: bool,
}

/// The WGPU textures the most recent frame drew, see [`ImportedTexture`].
#[derive(Default)]
struct TextureImports {
    textures: RefCell<HashMap<wgpu::Texture, ImportedTexture>>,
    blitters: RefCell<HashMap<wgpu::TextureFormat, wgpu::util::TextureBlitter>>,
}

impl TextureImporter for TextureImports {
    fn import_texture(
        &self,
        image: &i_slint_core::graphics::ImageInner,
    ) -> Option<peniko::ImageData> {
        let i_slint_core::graphics::ImageInner::WGPUTexture(texture) = image else {
            return None;
        };
        let texture = match &**texture {
            i_slint_core::graphics::WGPUTexture::WGPU30Texture(texture) => texture,
            // i-slint-core's `unstable-wgpu-29` feature may be enabled by another crate.
            #[allow(unreachable_patterns)]
            _ => return None,
        };
        let mut textures = self.textures.borrow_mut();
        let imported = textures.entry(texture.clone()).or_insert_with(|| ImportedTexture {
            image: peniko::ImageData {
                data: peniko::Blob::new(Arc::new([])),
                format: peniko::ImageFormat::Rgba8,
                alpha_type: peniko::ImageAlphaType::Alpha,
                width: texture.width(),
                height: texture.height(),
            },
            staging: None,
            registered: false,
            used_in_frame: false,
        });
        imported.used_in_frame = true;
        Some(imported.image.clone())
    }
}

impl TextureImports {
    /// Make the textures imported while recording the frame available to
    /// `renderer`, and release the ones the frame didn't draw.
    fn prepare(&self, renderer: &mut vello::Renderer, device: &wgpu::Device, queue: &wgpu::Queue) {
        let mut textures = self.textures.borrow_mut();
        textures.retain(|_, imported| {
            if !std::mem::take(&mut imported.used_in_frame) {
                renderer.override_image(&imported.image, None);
                return false;
            }
            true
        });
        if textures.is_empty() {
            return;
        }

        let mut blitters = self.blitters.borrow_mut();
        let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("slint vello texture import"),
        });
        for (texture, imported) in textures.iter_mut() {
            if !imported.registered {
                if !texture.usage().contains(wgpu::TextureUsages::COPY_SRC) {
                    imported.staging = Some(device.create_texture(&wgpu::TextureDescriptor {
                        label: Some("slint vello texture import staging"),
                        size: texture.size(),
                        mip_level_count: 1,
                        sample_count: 1,
                        dimension: wgpu::TextureDimension::D2,
                        format: texture.format(),
                        usage: wgpu::TextureUsages::RENDER_ATTACHMENT
                            | wgpu::TextureUsages::COPY_SRC,
                        view_formats: &[],
                    }));
                }
                renderer.override_image(
                    &imported.image,
                    Some(wgpu::TexelCopyTextureInfoBase {
                        texture: imported.staging.clone().unwrap_or_else(|| texture.clone()),
                        mip_level: 0,
                        origin: wgpu::Origin3d::ZERO,
                        aspect: wgpu::TextureAspect::All,
                    }),
                );
                imported.registered = true;
            } else {
                // The application may have rendered into the texture since the last frame.
                renderer.mark_override_image_dirty(&imported.image);
            }

            if let Some(staging) = &imported.staging {
                let blitter = blitters
                    .entry(texture.format())
                    .or_insert_with(|| wgpu::util::TextureBlitter::new(device, texture.format()));
                blitter.copy(
                    device,
                    &mut encoder,
                    &texture.create_view(&wgpu::TextureViewDescriptor::default()),
                    &staging.create_view(&wgpu::TextureViewDescriptor::default()),
                );
            }
        }
        queue.submit(std::iter::once(encoder.finish()));
    }

    /// Forget all textures, for when the vello renderer they were registered with is dropped.
    fn clear(&self) {
        self.textures.borrow_mut().clear();
        self.blitters.borrow_mut().clear();
    }
}

/// The WGPU objects and the vello renderer of a window that currently has a
/// surface. Dropped when the window is suspended.
struct ActiveState {
    /// Names the WGPU backend and adapter this surface ended up on, for
    /// `SLINT_DEBUG_PERFORMANCE` to report what is doing the rendering.
    winsys_info: String,
    instance: wgpu::Instance,
    device: wgpu::Device,
    queue: wgpu::Queue,
    surface: wgpu::Surface<'static>,
    surface_config: wgpu::SurfaceConfiguration,
    alpha_modes: Vec<wgpu::CompositeAlphaMode>,
    target: TargetTexture,
    blitter: wgpu::util::TextureBlitter,
    renderer: vello::Renderer,
}

/// Renders Slint's scene with vello onto a WGPU surface.
pub struct VelloWindowRenderer {
    state: Option<ActiveState>,
    /// Reused across frames; `vello::Scene` keeps its allocations when reset.
    scene: vello::Scene,
    texture_imports: Rc<TextureImports>,
    requested_graphics_api: Option<RequestedGraphicsAPI>,
    transparent: bool,
    /// [`WindowRenderer::resume`] cannot report failures, so the error is kept
    /// here for [`AnyrenderSlintRenderer::resume_window`] to return.
    resume_error: Option<String>,
    pre_present_callback: Option<Box<dyn Fn()>>,
}

impl VelloWindowRenderer {
    pub fn new() -> Self {
        Self {
            state: None,
            scene: vello::Scene::new(),
            texture_imports: Default::default(),
            requested_graphics_api: None,
            transparent: false,
            resume_error: None,
            pre_present_callback: None,
        }
    }

    /// Set the callback invoked right before a frame is presented, used by
    /// windowing backends that want to notify the window system first.
    pub fn set_pre_present_callback(&mut self, callback: Option<Box<dyn Fn()>>) {
        self.pre_present_callback = callback;
    }

    /// The graphics API the application requested, applied on the next resume.
    pub fn set_requested_graphics_api(&mut self, requested: Option<RequestedGraphicsAPI>) {
        self.requested_graphics_api = requested;
    }

    /// Whether the surface is composited with an alpha channel. Applied to a surface that
    /// already exists, and remembered for the next resume.
    pub fn set_transparent(&mut self, transparent: bool) {
        self.transparent = transparent;

        use wgpu::CompositeAlphaMode::{Opaque, PostMultiplied, PreMultiplied};
        let Some(state) = &mut self.state else { return };
        let wanted: &[wgpu::CompositeAlphaMode] =
            if transparent { &[PreMultiplied, PostMultiplied] } else { &[Opaque] };
        let Some(mode) = wanted.iter().copied().find(|m| state.alpha_modes.contains(m)) else {
            return;
        };
        if state.surface_config.alpha_mode != mode {
            state.surface_config.alpha_mode = mode;
            state.surface.configure(&state.device, &state.surface_config);
        }
    }

    /// Create the surface and everything derived from it, see
    /// [`AnyrenderSlintRenderer::set_surface`].
    pub(crate) fn set_state_from_surface_target(
        &mut self,
        surface_target: impl Into<i_slint_core::graphics::wgpu_30::SurfaceTarget>,
        width: u32,
        height: u32,
    ) -> Result<(), PlatformError> {
        let state = self.create_state(surface_target, width, height)?;
        self.texture_imports.clear();
        self.state = Some(state);
        self.resume_error = None;
        Ok(())
    }

    fn create_state(
        &self,
        surface_target: impl Into<i_slint_core::graphics::wgpu_30::SurfaceTarget>,
        width: u32,
        height: u32,
    ) -> Result<ActiveState, String> {
        let (instance, adapter, device, queue, surface) =
            i_slint_core::graphics::wgpu_30::init_instance_adapter_device_queue_surface(
                surface_target,
                self.requested_graphics_api.clone(),
                i_slint_core::graphics::wgpu_30::default_backends_to_avoid(),
            )
            .map_err(|e| format!("Error initializing WGPU for vello rendering: {e}"))?;

        let mut surface_config = surface
            .get_default_config(&adapter, width, height)
            .ok_or_else(|| "The WGPU surface is not supported by the adapter".to_string())?;

        let capabilities = surface.get_capabilities(&adapter);

        // Prefer FIFO modes over possible Mailbox setting for frame pacing and better energy efficiency.
        surface_config.present_mode = wgpu::PresentMode::AutoVsync;

        // The blit is a plain copy, so prefer a surface format matching the
        // texture vello renders into.
        surface_config.format = capabilities
            .formats
            .iter()
            .find(|format| **format == TARGET_TEXTURE_FORMAT)
            .or_else(|| {
                capabilities
                    .formats
                    .iter()
                    .find(|format| matches!(format, wgpu::TextureFormat::Bgra8Unorm))
            })
            .copied()
            .unwrap_or_else(|| capabilities.formats[0]);

        // The default `Opaque` discards the scene's alpha; pick a translucent
        // mode if offered. Metal (CAMetalLayer) only offers `PostMultiplied`,
        // so it must be a fallback - same choice as the FemtoVG renderer.
        if self.transparent {
            use wgpu::CompositeAlphaMode::{PostMultiplied, PreMultiplied};
            if let Some(mode) = [PreMultiplied, PostMultiplied]
                .into_iter()
                .find(|m| capabilities.alpha_modes.contains(m))
            {
                surface_config.alpha_mode = mode;
            }
        }

        if width > 0 && height > 0 {
            surface.configure(&device, &surface_config);
        }

        let renderer = vello::Renderer::new(&device, renderer_options())
            .map_err(|e| format!("Error creating the vello renderer: {e}"))?;

        let adapter_info = adapter.get_info();

        Ok(ActiveState {
            winsys_info: format!(
                "vello renderer on WGPU ({:?} backend; adapter: {})",
                adapter_info.backend, adapter_info.name
            ),
            target: TargetTexture::new(&device, width.max(1), height.max(1)),
            blitter: wgpu::util::TextureBlitter::new(&device, surface_config.format),
            instance,
            device,
            queue,
            surface,
            surface_config,
            alpha_modes: capabilities.alpha_modes,
            renderer,
        })
    }
}

/// Record the scene through `draw` and rasterize it into `target_view`.
///
/// Takes the pieces of [`ActiveState`] separately so the caller can keep
/// borrowing the rest of it (the surface and the blitter) at the same time.
#[allow(clippy::too_many_arguments)]
fn render_scene_to_target(
    scene: &mut vello::Scene,
    texture_imports: &TextureImports,
    renderer: &mut vello::Renderer,
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    target_view: &wgpu::TextureView,
    width: u32,
    height: u32,
    base_color: peniko::color::AlphaColor<peniko::color::Srgb>,
    draw: impl FnOnce(&mut VelloScenePainter<'static, '_>) -> Result<(), PlatformError>,
) -> Result<(), PlatformError> {
    scene.reset();
    draw(&mut VelloScenePainter::new(scene))?;
    texture_imports.prepare(renderer, device, queue);

    renderer
        .render_to_texture(
            device,
            queue,
            scene,
            target_view,
            &vello::RenderParams {
                base_color,
                width,
                height,
                antialiasing_method: ANTIALIASING_METHOD,
            },
        )
        .map_err(|e| PlatformError::from(format!("Error rendering with vello: {e}")))
}

impl Default for VelloWindowRenderer {
    fn default() -> Self {
        Self::new()
    }
}

impl anyrender::RenderContext for VelloWindowRenderer {}

impl WindowRenderer for VelloWindowRenderer {
    type ScenePainter<'a>
        = VelloScenePainter<'static, 'a>
    where
        Self: 'a;

    fn resume<F: FnOnce() + 'static>(
        &mut self,
        window: Arc<dyn WindowHandle>,
        width: u32,
        height: u32,
        on_ready: F,
    ) {
        let surface_target: i_slint_core::graphics::wgpu_30::SurfaceTarget =
            (Box::new(window) as Box<dyn wgpu::DisplayAndWindowHandle>).into();

        self.texture_imports.clear();
        match self.create_state(surface_target, width, height) {
            Ok(state) => {
                self.state = Some(state);
                self.resume_error = None;
            }
            Err(error) => {
                self.state = None;
                self.resume_error = Some(error);
            }
        }
        on_ready();
    }

    fn complete_resume(&mut self) -> bool {
        self.state.is_some()
    }

    fn suspend(&mut self) {
        self.texture_imports.clear();
        self.state = None;
    }

    fn is_active(&self) -> bool {
        self.state.is_some()
    }

    fn set_size(&mut self, width: u32, height: u32) {
        let Some(state) = &mut self.state else { return };
        if width == 0 || height == 0 {
            return;
        }
        if state.surface_config.width == width && state.surface_config.height == height {
            return;
        }
        state.surface_config.width = width;
        state.surface_config.height = height;
        state.surface.configure(&state.device, &state.surface_config);
        state.target = TargetTexture::new(&state.device, width, height);
    }

    fn render<F: FnOnce(&mut Self::ScenePainter<'_>)>(&mut self, draw_fn: F) {
        let _ = self.slint_render(
            PhysicalSize::default(),
            peniko::color::palette::css::WHITE,
            |scene| {
                draw_fn(scene);
                Ok(())
            },
        );
    }
}

impl SlintWindowRenderer for VelloWindowRenderer {
    fn winsys_info(&self) -> String {
        self.state
            .as_ref()
            .map(|state| state.winsys_info.clone())
            .unwrap_or_else(|| "vello renderer on WGPU (no surface)".into())
    }

    const PROVIDES_GRAPHICS_API: bool = true;

    fn texture_importer(&self) -> Option<Rc<dyn TextureImporter>> {
        Some(self.texture_imports.clone())
    }

    fn graphics_api(&self) -> Option<i_slint_core::api::GraphicsAPI<'static>> {
        let state = self.state.as_ref()?;
        Some(i_slint_core::graphics::create_graphics_api_wgpu_30(
            state.instance.clone(),
            state.device.clone(),
            state.queue.clone(),
        ))
    }

    fn slint_render<F>(
        &mut self,
        _surface_size: PhysicalSize,
        base_color: peniko::color::AlphaColor<peniko::color::Srgb>,
        draw: F,
    ) -> Result<DrawOutcome, PlatformError>
    where
        F: FnOnce(&mut Self::ScenePainter<'_>) -> Result<(), PlatformError>,
    {
        let Self { state, scene, texture_imports, pre_present_callback, .. } = self;

        // Before the surface is created (or while suspended) there is nothing
        // to present to.
        let Some(ActiveState {
            device,
            queue,
            surface,
            surface_config,
            target,
            blitter,
            renderer,
            ..
        }) = state
        else {
            return Ok(DrawOutcome::Skipped);
        };

        // The size of the configured surface wins over the window size the
        // caller passed: a resize that has not reached the surface yet would
        // make the blit fail.
        let (width, height) = (surface_config.width, surface_config.height);
        if width == 0 || height == 0 {
            return Ok(DrawOutcome::Skipped);
        }

        if target.width != width || target.height != height {
            *target = TargetTexture::new(device, width, height);
        }

        let surface_texture = match surface.get_current_texture() {
            wgpu::CurrentSurfaceTexture::Success(surface_texture) => surface_texture,
            wgpu::CurrentSurfaceTexture::Occluded => return Ok(DrawOutcome::Occluded),
            wgpu::CurrentSurfaceTexture::Timeout => return Ok(DrawOutcome::Timeout),
            wgpu::CurrentSurfaceTexture::Validation => {
                return Err("WGPU surface validation error in get_current_texture".into());
            }
            stale @ (wgpu::CurrentSurfaceTexture::Outdated
            | wgpu::CurrentSurfaceTexture::Suboptimal(_)
            | wgpu::CurrentSurfaceTexture::Lost) => {
                // `Suboptimal` carries a live `SurfaceTexture` that wgpu forbids
                // keeping across `configure()`, so drop it before reconfiguring.
                drop(stale);
                surface.configure(device, surface_config);
                match surface.get_current_texture() {
                    wgpu::CurrentSurfaceTexture::Success(surface_texture) => surface_texture,
                    _ => return Ok(DrawOutcome::Skipped),
                }
            }
        };

        render_scene_to_target(
            scene,
            texture_imports,
            renderer,
            device,
            queue,
            &target.view,
            width,
            height,
            base_color,
            draw,
        )?;

        let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("slint vello surface blit"),
        });
        blitter.copy(
            device,
            &mut encoder,
            &target.view,
            &surface_texture.texture.create_view(&wgpu::TextureViewDescriptor::default()),
        );
        queue.submit(std::iter::once(encoder.finish()));

        if let Some(callback) = pre_present_callback {
            callback();
        }
        queue.present(surface_texture);

        Ok(DrawOutcome::Success)
    }

    fn slint_set_size(&mut self, width: u32, height: u32) -> Result<(), PlatformError> {
        WindowRenderer::set_size(self, width, height);
        Ok(())
    }

    fn slint_take_snapshot<F>(
        &mut self,
        surface_size: PhysicalSize,
        base_color: peniko::color::AlphaColor<peniko::color::Srgb>,
        draw: F,
    ) -> Result<SharedPixelBuffer<Rgba8Pixel>, PlatformError>
    where
        F: FnOnce(&mut Self::ScenePainter<'_>) -> Result<(), PlatformError>,
    {
        let Self { state, scene, texture_imports, .. } = self;
        let Some(ActiveState { device, queue, renderer, .. }) = state else {
            return Err("take_snapshot requires the vello renderer to have a window".into());
        };
        let (width, height) = (surface_size.width.max(1), surface_size.height.max(1));

        let texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("slint vello snapshot"),
            size: wgpu::Extent3d { width, height, depth_or_array_layers: 1 },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: TARGET_TEXTURE_FORMAT,
            usage: wgpu::TextureUsages::STORAGE_BINDING | wgpu::TextureUsages::COPY_SRC,
            view_formats: &[],
        });
        let view = texture.create_view(&wgpu::TextureViewDescriptor::default());

        render_scene_to_target(
            scene,
            texture_imports,
            renderer,
            device,
            queue,
            &view,
            width,
            height,
            base_color,
            draw,
        )?;

        let unpadded_bytes_per_row = width * 4;
        let bytes_per_row = (unpadded_bytes_per_row + wgpu::COPY_BYTES_PER_ROW_ALIGNMENT - 1)
            & !(wgpu::COPY_BYTES_PER_ROW_ALIGNMENT - 1);
        let readback_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("slint vello snapshot readback"),
            size: (bytes_per_row * height) as u64,
            usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
            mapped_at_creation: false,
        });
        let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor::default());
        encoder.copy_texture_to_buffer(
            wgpu::TexelCopyTextureInfo {
                texture: &texture,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            wgpu::TexelCopyBufferInfo {
                buffer: &readback_buffer,
                layout: wgpu::TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some(bytes_per_row),
                    rows_per_image: None,
                },
            },
            wgpu::Extent3d { width, height, depth_or_array_layers: 1 },
        );
        queue.submit(std::iter::once(encoder.finish()));

        let slice = readback_buffer.slice(..);
        let (sender, receiver) = std::sync::mpsc::channel();
        slice.map_async(wgpu::MapMode::Read, move |result| {
            let _ = sender.send(result);
        });
        device
            .poll(wgpu::PollType::wait_indefinitely())
            .map_err(|e| format!("take_snapshot: device poll failed: {e}"))?;
        receiver
            .recv()
            .map_err(|e| format!("take_snapshot: map_async callback was not delivered: {e}"))?
            .map_err(|e| format!("take_snapshot: map_async failed: {e}"))?;

        let mapped = slice
            .get_mapped_range()
            .map_err(|e| format!("take_snapshot: mapping the readback buffer failed: {e}"))?;
        let mut pixels = SharedPixelBuffer::<Rgba8Pixel>::new(width, height);
        let destination = pixels.make_mut_bytes();
        for (row_index, source_row) in mapped.chunks(bytes_per_row as usize).enumerate() {
            let row_bytes = unpadded_bytes_per_row as usize;
            let start = row_index * row_bytes;
            destination[start..start + row_bytes].copy_from_slice(&source_row[..row_bytes]);
        }
        drop(mapped);
        readback_buffer.unmap();

        unpremultiply_rgba(pixels.make_mut_bytes());
        Ok(pixels)
    }
}

/// vello renders with premultiplied alpha, while [`SharedPixelBuffer`] of
/// [`Rgba8Pixel`] is unpremultiplied.
fn unpremultiply_rgba(buffer: &mut [u8]) {
    for pixel in buffer.as_chunks_mut::<4>().0 {
        let alpha = pixel[3];
        if alpha == 0 || alpha == u8::MAX {
            continue;
        }
        for channel in &mut pixel[..3] {
            *channel = ((*channel as u32 * 255 + alpha as u32 / 2) / alpha as u32).min(255) as u8;
        }
    }
}

impl AnyrenderSlintRenderer<VelloWindowRenderer> {
    /// Create a Slint renderer that renders through vello on WGPU. It starts
    /// out suspended; call [`Self::resume_window`] to attach it to a window.
    pub fn new_vello() -> Self {
        Self::with_window_renderer(VelloWindowRenderer::new())
    }

    /// Create the WGPU surface for `window` and make the renderer ready to
    /// present frames of `width` x `height` physical pixels. `transparent`
    /// selects whether the surface is composited with an alpha channel, and
    /// `requested_graphics_api` carries an application supplied WGPU
    /// configuration, if any.
    /// Adjusts the surface for a window that became transparent or opaque after it was
    /// created, so that the scene's alpha is kept or discarded to match.
    pub fn set_transparent(&self, transparent: bool) {
        self.window_renderer().set_transparent(transparent);
    }

    pub fn resume_window<W: WindowHandle + 'static>(
        &self,
        window: Arc<W>,
        width: u32,
        height: u32,
        transparent: bool,
        requested_graphics_api: Option<RequestedGraphicsAPI>,
    ) -> Result<(), PlatformError> {
        self.notify_rendering_teardown();
        let mut window_renderer = self.window_renderer();
        window_renderer.set_transparent(transparent);
        window_renderer.set_requested_graphics_api(requested_graphics_api);
        window_renderer.resume(window as Arc<dyn WindowHandle>, width, height, || {});
        if window_renderer.complete_resume() {
            drop(window_renderer);
            self.reset_metrics_collector();
            Ok(())
        } else {
            Err(window_renderer
                .resume_error
                .take()
                .unwrap_or_else(|| "the vello renderer did not become active".to_string())
                .into())
        }
    }

    /// Render to `surface_target` directly, for windowing systems that hand
    /// out a WGPU surface target rather than a window handle - linuxkms, which
    /// renders to a DRM plane.
    pub fn set_surface(
        &self,
        surface_target: impl Into<i_slint_core::graphics::wgpu_30::SurfaceTarget>,
        width: u32,
        height: u32,
        requested_graphics_api: Option<RequestedGraphicsAPI>,
    ) -> Result<(), PlatformError> {
        self.notify_rendering_teardown();
        let mut window_renderer = self.window_renderer();
        window_renderer.set_requested_graphics_api(requested_graphics_api);
        window_renderer.set_state_from_surface_target(surface_target, width, height)?;
        drop(window_renderer);
        self.reset_metrics_collector();
        Ok(())
    }

    /// Release the WGPU surface, for example when the windowing system takes
    /// the window away. Rendering is skipped until the next
    /// [`Self::resume_window`].
    pub fn suspend_window(&self) {
        self.notify_rendering_teardown();
        self.window_renderer().suspend();
    }

    /// Set the callback invoked right before a frame is presented.
    pub fn set_pre_present_callback(&self, callback: Option<Box<dyn Fn()>>) {
        self.window_renderer().set_pre_present_callback(callback);
    }
}
