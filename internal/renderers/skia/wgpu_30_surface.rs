// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

#[cfg(feature = "unstable-wgpu-30")]
use i_slint_core::api::GraphicsAPI;
use i_slint_core::api::{PhysicalSize as PhysicalWindowSize, Window};
use i_slint_core::graphics::RequestedGraphicsAPI;
use i_slint_core::partial_renderer::DirtyRegion;
use i_slint_core::platform::PlatformError;
use i_slint_core::renderer::DrawOutcome;

use std::cell::RefCell;
use std::rc::Rc;
use std::sync::Arc;

use wgpu_30 as wgpu;

use crate::SkiaSharedContext;

#[cfg(target_family = "windows")]
mod dx12;
#[cfg(target_vendor = "apple")]
mod metal;
#[cfg(skia_wgpu_vulkan)]
mod vulkan;

/// See [`crate::attachment_color_space`].
pub(crate) fn attachment_color_space(texture: &wgpu::Texture) -> skia_safe::ColorSpace {
    crate::attachment_color_space(crate::TextureEncoding::from_format_is_srgb(
        texture.format().is_srgb(),
    ))
}

/// See [`crate::sampled_texture_color_space`].
#[cfg_attr(not(feature = "unstable-wgpu-30"), allow(dead_code))]
pub(crate) fn sampled_texture_color_space(texture: &wgpu::Texture) -> skia_safe::ColorSpace {
    crate::sampled_texture_color_space(crate::TextureEncoding::from_format_is_srgb(
        texture.format().is_srgb(),
    ))
}

/// Skia rendering surface backed by WGPU. Supports both on-screen rendering (with a
/// window surface) and offscreen rendering into caller-provided textures.
pub struct WGPUSurface {
    pub(crate) gr_context: RefCell<skia_safe::gpu::DirectContext>,
    wgpu: Rc<SharedWgpuState>,
    surface_config: RefCell<Option<wgpu::SurfaceConfiguration>>,
    surface: Option<wgpu::Surface<'static>>,
    textures_to_transition_for_sampling: RefCell<Vec<wgpu::Texture>>,
    pub(crate) backend: Backend,
    alpha_modes: Vec<wgpu::CompositeAlphaMode>,
}

fn backends_to_avoid() -> wgpu::Backends {
    let mut avoid = wgpu::Backends::GL; /* we're not mapping that to skia because we can't save/restore state */
    #[cfg(not(target_vendor = "apple"))]
    avoid.insert(wgpu::Backends::METAL);
    #[cfg(not(target_family = "windows"))]
    avoid.insert(wgpu::Backends::DX12);
    avoid
}

impl WGPUSurface {
    pub fn new_with_surface(
        surface_target: impl Into<i_slint_core::graphics::wgpu_30::SurfaceTarget>,
        size: PhysicalWindowSize,
        requested_graphics_api: Option<RequestedGraphicsAPI>,
    ) -> Result<Self, PlatformError> {
        let (instance, adapter, device, queue, surface) =
            i_slint_core::graphics::wgpu_30::init_instance_adapter_device_queue_surface(
                surface_target,
                requested_graphics_api,
                backends_to_avoid(),
            )?;
        Self::init_with_parts(
            Rc::new(SharedWgpuState { instance, adapter, device, queue }),
            surface,
            size,
        )
    }

    fn init_with_parts(
        wgpu: Rc<SharedWgpuState>,
        surface: wgpu::Surface<'static>,
        size: PhysicalWindowSize,
    ) -> Result<Self, PlatformError> {
        let SharedWgpuState { adapter, device, queue, .. } = &*wgpu;
        #[cfg(target_vendor = "apple")]
        metal::set_layer_contents_gravity(&surface);

        let mut surface_config = surface
            .get_default_config(adapter, size.width, size.height)
            .ok_or_else(|| PlatformError::from("WGPU surface is not compatible with adapter"))?;

        let swapchain_capabilities = surface.get_capabilities(adapter);
        let swapchain_format = swapchain_capabilities
            .formats
            .iter()
            .find(|f| {
                matches!(f, wgpu::TextureFormat::Rgba8Unorm | wgpu::TextureFormat::Bgra8Unorm)
            })
            .copied()
            .unwrap_or_else(|| swapchain_capabilities.formats[0]);
        surface_config.format = swapchain_format;

        // Prefer FIFO modes over the Mailbox that `get_default_config` picks on some backends
        // (it takes the first advertised mode, and DX12 lists Mailbox first), for frame pacing
        // and better energy efficiency. `AutoVsync` falls back to FifoRelaxed and then Fifo, so
        // it is supported everywhere.
        surface_config.present_mode = wgpu::PresentMode::AutoVsync;

        let error_scope = device.push_error_scope(wgpu::ErrorFilter::Validation);
        surface.configure(device, &surface_config);
        if let Some(e) = spin_on::spin_on(error_scope.pop()) {
            return Err(PlatformError::from(format!("Error configuring WGPU surface: {e}")));
        }

        let backend = Backend::new(adapter, device)?;

        let gr_context = backend
            .make_context(adapter, device, queue)
            .ok_or_else(|| PlatformError::from("Failed to create Skia context from WGPU"))?;

        Ok(Self {
            gr_context: gr_context.into(),
            wgpu,
            surface_config: Some(surface_config).into(),
            surface: Some(surface),
            textures_to_transition_for_sampling: RefCell::new(Vec::new()),
            backend,
            alpha_modes: swapchain_capabilities.alpha_modes,
        })
    }

    pub(crate) fn new_offscreen(
        instance: wgpu::Instance,
        adapter: wgpu::Adapter,
        device: wgpu::Device,
        queue: wgpu::Queue,
        backend: Backend,
        gr_context: skia_safe::gpu::DirectContext,
    ) -> Self {
        Self {
            gr_context: RefCell::new(gr_context),
            wgpu: Rc::new(SharedWgpuState { instance, adapter, device, queue }),
            surface_config: None.into(),
            surface: None,
            textures_to_transition_for_sampling: RefCell::new(Vec::new()),
            backend,
            alpha_modes: vec![],
        }
    }

    /// Transitions any imported wgpu textures to sampling state and flushes
    /// the Skia graphics context. Must be called after rendering to ensure
    /// Skia's GPU work is submitted.
    pub(crate) fn flush_and_submit(&self, gr_context: &mut skia_safe::gpu::DirectContext) {
        let textures_to_transition = self.textures_to_transition_for_sampling.take();
        if !textures_to_transition.is_empty() {
            let mut encoder =
                self.wgpu.device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
                    label: Some("Skia texture transition encoder"),
                });
            encoder.transition_resources(
                std::iter::empty(),
                textures_to_transition.iter().map(|texture| wgpu::TextureTransition {
                    texture,
                    selector: None,
                    state: wgpu::TextureUses::RESOURCE,
                }),
            );
            self.wgpu.queue.submit(Some(encoder.finish()));
        }

        gr_context.submit(None);
    }
}

/// The wgpu stack a surface renders with.
///
/// [`SkiaSharedContext`] caches the one the first window created, so that later windows it can
/// serve reuse it instead of creating their own.
/// That cache holds a weak reference and every surface a strong one, so the resources go away
/// with the last surface using them.
/// Were they to outlive the surfaces, they'd be destroyed whenever the last renderer happens to
/// be dropped, and tearing a wgpu device down from a thread-local destructor aborts the process
/// on macOS, where the Metal backend needs an autorelease pool that is gone by then.
/// A window that is still shown owns its surface, so that case remains.
pub(crate) struct SharedWgpuState {
    instance: wgpu::Instance,
    adapter: wgpu::Adapter,
    device: wgpu::Device,
    queue: wgpu::Queue,
}

/// Whether `device` offers everything `settings` asks of it.
///
/// The instance and adapter parts of the settings are covered by
/// [`adapter_matches_graphics_api_request`].
#[cfg(feature = "unstable-wgpu-30")]
fn device_satisfies_settings(
    device: &wgpu::Device,
    settings: &i_slint_core::graphics::wgpu_30::api::WGPUSettings,
) -> bool {
    device.features().contains(settings.device_required_features)
        && settings.device_required_limits.check_limits(&device.limits())
}

fn adapter_matches_graphics_api_request(
    adapter: &wgpu::Adapter,
    requested_graphics_api: Option<&RequestedGraphicsAPI>,
) -> bool {
    let backend = adapter.get_info().backend;

    #[cfg_attr(slint_nightly_test, allow(non_exhaustive_omitted_patterns))]
    match requested_graphics_api {
        None => true,
        Some(RequestedGraphicsAPI::Metal) => backend == wgpu::Backend::Metal,
        Some(RequestedGraphicsAPI::Vulkan) => backend == wgpu::Backend::Vulkan,
        Some(RequestedGraphicsAPI::Direct3D) => backend == wgpu::Backend::Dx12,
        #[cfg(feature = "unstable-wgpu-30")]
        Some(RequestedGraphicsAPI::WGPU30(
            i_slint_core::graphics::wgpu_30::api::WGPUConfiguration::Automatic(settings),
        )) => {
            let backend_bit = match backend {
                wgpu::Backend::Vulkan => wgpu::Backends::VULKAN,
                wgpu::Backend::Metal => wgpu::Backends::METAL,
                wgpu::Backend::Dx12 => wgpu::Backends::DX12,
                wgpu::Backend::Gl => wgpu::Backends::GL,
                wgpu::Backend::BrowserWebGpu => wgpu::Backends::BROWSER_WEBGPU,
                _ => return false,
            };
            settings.backends.contains(backend_bit)
        }
        Some(_) => false,
    }
}

impl crate::Surface for WGPUSurface {
    fn new(
        shared_context: &SkiaSharedContext,
        window_handle: Arc<dyn raw_window_handle::HasWindowHandle + Send + Sync>,
        display_handle: Arc<dyn raw_window_handle::HasDisplayHandle + Send + Sync>,
        size: PhysicalWindowSize,
        requested_graphics_api: Option<RequestedGraphicsAPI>,
    ) -> Result<Self, PlatformError> {
        let make_target = || -> Box<dyn wgpu::DisplayAndWindowHandle + 'static> {
            Box::new(WindowAndDisplayHandle(window_handle.clone(), display_handle.clone()))
        };

        #[cfg(feature = "unstable-wgpu-30")]
        let manual_configuration = matches!(
            &requested_graphics_api,
            Some(RequestedGraphicsAPI::WGPU30(
                i_slint_core::graphics::wgpu_30::api::WGPUConfiguration::Manual { .. }
            ))
        );
        #[cfg(not(feature = "unstable-wgpu-30"))]
        let manual_configuration = false;

        #[cfg(feature = "unstable-wgpu-30")]
        #[cfg_attr(slint_nightly_test, allow(non_exhaustive_omitted_patterns))]
        let requested_settings = match &requested_graphics_api {
            Some(RequestedGraphicsAPI::WGPU30(
                i_slint_core::graphics::wgpu_30::api::WGPUConfiguration::Automatic(settings),
            )) => Some(settings.clone()),
            _ => None,
        };

        // try to reuse old / shared graphics primitives from previous windows with matching
        // settings
        if !manual_configuration {
            let shared_state = shared_context.0.wgpu_30_state.borrow().upgrade();
            if let Some(shared) = shared_state {
                #[cfg(feature = "unstable-wgpu-30")]
                let settings_compatible = requested_settings
                    .as_ref()
                    .is_none_or(|settings| device_satisfies_settings(&shared.device, settings));
                #[cfg(not(feature = "unstable-wgpu-30"))]
                let settings_compatible = true;
                if settings_compatible
                    && adapter_matches_graphics_api_request(
                        &shared.adapter,
                        requested_graphics_api.as_ref(),
                    )
                    && let Ok(surface) = shared.instance.create_surface(make_target())
                    && shared.adapter.is_surface_supported(&surface)
                {
                    match Self::init_with_parts(shared, surface, size) {
                        Ok(surface) => return Ok(surface),
                        Err(err) => {
                            // The shared device may be lost. Drop it and start over.
                            i_slint_core::debug_log!(
                                "Failed to reuse the shared WGPU device: {err} . Re-initializing"
                            );
                            shared_context.0.wgpu_30_state.take();
                        }
                    }
                }
            }
        }

        let (instance, adapter, device, queue, surface) =
            i_slint_core::graphics::wgpu_30::init_instance_adapter_device_queue_surface(
                make_target(),
                requested_graphics_api,
                backends_to_avoid(),
            )?;
        let wgpu = Rc::new(SharedWgpuState { instance, adapter, device, queue });
        let new_surface = Self::init_with_parts(wgpu.clone(), surface, size)?;
        if !manual_configuration {
            *shared_context.0.wgpu_30_state.borrow_mut() = Rc::downgrade(&wgpu);
        }
        Ok(new_surface)
    }

    fn name(&self) -> &'static str {
        if self.surface.is_some() { "wgpu" } else { "wgpu-texture" }
    }

    fn resize_event(&self, size: PhysicalWindowSize) -> Result<(), PlatformError> {
        let mut surface_config_opt = self.surface_config.borrow_mut();
        let (Some(surface_config), Some(surface)) = (surface_config_opt.as_mut(), &self.surface)
        else {
            return Ok(());
        };

        // Skip reconfigure if size hasn't changed — DRM/KMS surfaces don't
        // support being reconfigured.
        if surface_config.width == size.width && surface_config.height == size.height {
            return Ok(());
        }

        {
            let gr_context = &mut self.gr_context.borrow_mut();
            // This is brute force, but for the lack of access to the fences this seems to work: Avoid any pending work so that
            // IDXGISwapChain::ResizeBuffers doesn't complain that the surface is still in use.
            gr_context.flush_submit_and_sync_cpu();
        }

        surface_config.width = size.width;
        surface_config.height = size.height;

        surface.configure(&self.wgpu.device, surface_config);
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
        ) -> Option<DirtyRegion>,
        pre_present_callback: &RefCell<Option<Box<dyn FnMut()>>>,
    ) -> Result<DrawOutcome, PlatformError> {
        let (Some(surface), Some(surface_config)) = (&self.surface, &*self.surface_config.borrow())
        else {
            return Err("WGPUSurface::render() called on offscreen surface".into());
        };

        let gr_context = &mut self.gr_context.borrow_mut();

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
                // `Suboptimal` carries a live `SurfaceTexture`; matched with `_` it is not bound,
                // so the value returned by `get_current_texture()` keeps it alive across the
                // `surface.configure()` below — which wgpu forbids ("`SurfaceOutput` must be
                // dropped before a new `Surface` is made"), panicking on the first frame on
                // Wayland. Drop it first. (`Outdated`/`Lost` carry nothing → no-op.)
                drop(stale);
                surface.configure(&self.wgpu.device, surface_config);
                match surface.get_current_texture() {
                    wgpu::CurrentSurfaceTexture::Success(t) => t,
                    _ => return Ok(DrawOutcome::Occluded),
                }
            }
        };

        // Skia renders through the raw backend queue, invisible to wgpu's usage
        // tracking, so `Queue::present` would treat the frame as never written
        // and clear it. Clear it through wgpu instead before Skia draws; the
        // in-order queue keeps Skia's later-committed work on top.
        let view = frame.texture.create_view(&wgpu::TextureViewDescriptor::default());
        let mut encoder =
            self.wgpu.device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("Slint frame init"),
            });
        encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("Slint frame init"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: &view,
                depth_slice: None,
                resolve_target: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Clear(wgpu::Color::TRANSPARENT),
                    store: wgpu::StoreOp::Store,
                },
            })],
            depth_stencil_attachment: None,
            timestamp_writes: None,
            occlusion_query_set: None,
            multiview_mask: None,
        });
        self.wgpu.queue.submit(Some(encoder.finish()));

        let skia_surface = self.backend.make_swapchain_surface(gr_context, &frame.texture);

        let mut skia_surface = skia_surface
            .ok_or_else(|| PlatformError::from("Failed to create Skia surface from WGPU"))?;

        callback(skia_surface.canvas(), Some(gr_context), 0);

        self.backend.release_swapchain_surface(gr_context, &mut skia_surface);

        self.flush_and_submit(gr_context);

        // Skia drew via the raw queue behind wgpu's back, and `Queue::present` skips its own
        // transition when the tracker already says PRESENT, so nothing would order the
        // presentation after Skia's work: the clear submission above signals its semaphore
        // before Skia even starts. Referencing the frame texture in a submission of our own
        // makes present wait for it.
        let mut encoder =
            self.wgpu.device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("Skia present ordering encoder"),
            });
        encoder.transition_resources(
            std::iter::empty(),
            std::iter::once(wgpu::TextureTransition {
                texture: &frame.texture,
                selector: None,
                state: wgpu::TextureUses::PRESENT,
            }),
        );
        self.wgpu.queue.submit(Some(encoder.finish()));

        if let Some(pre_present_callback) = pre_present_callback.borrow_mut().as_mut() {
            pre_present_callback();
        }

        self.wgpu.queue.present(frame);

        Ok(DrawOutcome::Success)
    }

    fn bits_per_pixel(&self) -> Result<u8, PlatformError> {
        if let Some(surface_config) = &*self.surface_config.borrow() {
            Ok(match surface_config.format {
                wgpu_30::TextureFormat::Rgba8Unorm
                | wgpu_30::TextureFormat::Rgba8UnormSrgb
                | wgpu_30::TextureFormat::Bgra8Unorm
                | wgpu_30::TextureFormat::Bgra8UnormSrgb => 32,
                fmt => return Err(format!("Unsupported surface format {:#?}", fmt).into()),
            })
        } else {
            // All supported render-target formats (Rgba8Unorm, Bgra8Unorm, and sRGB variants) are 32bpp.
            Ok(32)
        }
    }

    #[cfg(feature = "unstable-wgpu-30")]
    fn with_graphics_api(&self, callback: &mut dyn FnMut(GraphicsAPI<'_>)) {
        let api = i_slint_core::graphics::create_graphics_api_wgpu_30(
            self.wgpu.instance.clone(),
            self.wgpu.device.clone(),
            self.wgpu.queue.clone(),
        );
        callback(api)
    }

    #[cfg(any(feature = "unstable-wgpu-29", feature = "unstable-wgpu-30"))]
    fn import_wgpu_texture(
        &self,
        canvas: &skia_safe::Canvas,
        any_wgpu_texture: &i_slint_core::graphics::WGPUTexture,
    ) -> Option<skia_safe::Image> {
        let texture: wgpu_30::Texture = match any_wgpu_texture {
            #[cfg(feature = "unstable-wgpu-29")]
            i_slint_core::graphics::WGPUTexture::WGPU29Texture(..) => return None,
            #[cfg(feature = "unstable-wgpu-30")]
            i_slint_core::graphics::WGPUTexture::WGPU30Texture(texture) => texture.clone(),
        };

        // Skia won't submit commands right away, so remember the texture and transition before
        // submitting.
        self.textures_to_transition_for_sampling.borrow_mut().push(texture.clone());

        self.backend.import_texture(canvas, texture)
    }

    fn set_transparent(&self, transparent: bool) -> Result<(), PlatformError> {
        if transparent {
            // The default `Opaque` discards the scene's alpha; pick a translucent mode if offered.
            // Metal (CAMetalLayer) only offers `PostMultiplied`, so it must be a fallback.
            use wgpu::CompositeAlphaMode::{PostMultiplied, PreMultiplied};
            if let Some(mode) =
                [PreMultiplied, PostMultiplied].into_iter().find(|m| self.alpha_modes.contains(m))
            {
                let mut surface_config_opt = self.surface_config.borrow_mut();
                let (Some(surface_config), Some(surface)) =
                    (surface_config_opt.as_mut(), &self.surface)
                else {
                    return Ok(());
                };
                surface_config.alpha_mode = mode;
                surface.configure(&self.wgpu.device, surface_config);
            }
        }
        Ok(())
    }
}

struct WindowAndDisplayHandle(
    Arc<dyn raw_window_handle::HasWindowHandle + Send + Sync>,
    Arc<dyn raw_window_handle::HasDisplayHandle + Send + Sync>,
);

impl raw_window_handle::HasWindowHandle for WindowAndDisplayHandle {
    fn window_handle(
        &self,
    ) -> Result<raw_window_handle::WindowHandle<'_>, raw_window_handle::HandleError> {
        self.0.window_handle()
    }
}

impl raw_window_handle::HasDisplayHandle for WindowAndDisplayHandle {
    fn display_handle(
        &self,
    ) -> Result<raw_window_handle::DisplayHandle<'_>, raw_window_handle::HandleError> {
        self.1.display_handle()
    }
}

pub(crate) enum Backend {
    #[cfg(target_vendor = "apple")]
    Metal,
    #[cfg(target_family = "windows")]
    Dx12,
    #[cfg(skia_wgpu_vulkan)]
    Vulkan {
        /// The family Skia has to hand the swapchain image back to, see
        /// [`Backend::release_swapchain_surface`].
        queue_family_index: u32,
    },
}

impl Backend {
    pub(crate) fn new(
        adapter: &wgpu::Adapter,
        _device: &wgpu::Device,
    ) -> Result<Self, PlatformError> {
        match adapter.get_info().backend {
            wgpu_30::Backend::Noop => {
                Err(PlatformError::from("Cannot use WGPU Noop backend with Skia"))
            }
            #[cfg(skia_wgpu_vulkan)]
            wgpu_30::Backend::Vulkan => Ok(Self::Vulkan {
                // SAFETY: `_device` is a Vulkan device, as the adapter's backend just said.
                queue_family_index: unsafe { vulkan::queue_family_index(_device) }.ok_or_else(
                    || {
                        PlatformError::from(
                            "Cannot query the queue family of the WGPU Vulkan device",
                        )
                    },
                )?,
            }),
            #[cfg(target_vendor = "apple")]
            wgpu_30::Backend::Metal => Ok(Self::Metal),
            #[cfg(target_family = "windows")]
            wgpu_30::Backend::Dx12 => Ok(Self::Dx12),
            other => Err(PlatformError::from(format!(
                "Unsupported WGPU backend for use with Skia: {}",
                other
            ))),
        }
    }

    pub(crate) fn make_context(
        &self,
        _adapter: &wgpu::Adapter,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
    ) -> Option<skia_safe::gpu::DirectContext> {
        match self {
            #[cfg(target_vendor = "apple")]
            Self::Metal => metal::make_metal_context(device, queue),
            #[cfg(target_family = "windows")]
            Self::Dx12 => unsafe { dx12::make_dx12_context(&_adapter, &device, &queue) },
            #[cfg(skia_wgpu_vulkan)]
            Self::Vulkan { .. } => unsafe { vulkan::make_vulkan_context(device, queue) },
        }
    }

    pub(crate) fn make_surface(
        &self,
        gr_context: &mut skia_safe::gpu::DirectContext,
        texture: &wgpu::Texture,
    ) -> Option<skia_safe::Surface> {
        match self {
            #[cfg(target_vendor = "apple")]
            Self::Metal => unsafe { metal::make_metal_surface(gr_context, texture) },
            #[cfg(target_family = "windows")]
            Self::Dx12 => unsafe { dx12::make_dx12_surface(gr_context, texture) },
            #[cfg(skia_wgpu_vulkan)]
            Self::Vulkan { .. } => unsafe {
                vulkan::make_vulkan_surface(
                    gr_context,
                    texture,
                    skia_safe::gpu::vk::ImageLayout::COLOR_ATTACHMENT_OPTIMAL,
                )
            },
        }
    }

    /// Like [`Self::make_surface`], but for the swapchain image handed out by
    /// [`wgpu::Surface::get_current_texture`].
    ///
    /// wgpu returns surface textures to their `PRESENT` state at the end of every submission,
    /// so that is the layout Skia finds the image in, not the color-attachment one a plain
    /// render target is left in. Pair every call with [`Self::release_swapchain_surface`].
    pub(crate) fn make_swapchain_surface(
        &self,
        gr_context: &mut skia_safe::gpu::DirectContext,
        texture: &wgpu::Texture,
    ) -> Option<skia_safe::Surface> {
        match self {
            #[cfg(target_vendor = "apple")]
            Self::Metal => unsafe { metal::make_metal_surface(gr_context, texture) },
            #[cfg(target_family = "windows")]
            Self::Dx12 => unsafe { dx12::make_dx12_surface(gr_context, texture) },
            #[cfg(skia_wgpu_vulkan)]
            Self::Vulkan { .. } => unsafe {
                vulkan::make_vulkan_surface(
                    gr_context,
                    texture,
                    skia_safe::gpu::vk::ImageLayout::PRESENT_SRC_KHR,
                )
            },
        }
    }

    /// Hands a surface made by [`Self::make_swapchain_surface`] back in the state
    /// [`wgpu::Queue::present`] expects to find it in.
    pub(crate) fn release_swapchain_surface(
        &self,
        _gr_context: &mut skia_safe::gpu::DirectContext,
        _skia_surface: &mut skia_safe::Surface,
    ) {
        match self {
            // Metal and D3D12 have no image layout for Skia to hand back.
            #[cfg(target_vendor = "apple")]
            Self::Metal => {}
            #[cfg(target_family = "windows")]
            Self::Dx12 => {}
            #[cfg(skia_wgpu_vulkan)]
            Self::Vulkan { queue_family_index } => vulkan::release_vulkan_swapchain_surface(
                _gr_context,
                _skia_surface,
                *queue_family_index,
            ),
        }
    }

    #[cfg_attr(not(feature = "unstable-wgpu-30"), allow(dead_code))]
    pub(crate) fn import_texture(
        &self,
        canvas: &skia_safe::Canvas,
        texture: wgpu::Texture,
    ) -> Option<skia_safe::Image> {
        match self {
            #[cfg(target_vendor = "apple")]
            Self::Metal => unsafe { metal::import_metal_texture(canvas, texture) },
            #[cfg(target_family = "windows")]
            Self::Dx12 => unsafe { dx12::import_dx12_texture(canvas, texture) },
            #[cfg(skia_wgpu_vulkan)]
            Self::Vulkan { .. } => unsafe { vulkan::import_vulkan_texture(canvas, texture) },
        }
    }
}
