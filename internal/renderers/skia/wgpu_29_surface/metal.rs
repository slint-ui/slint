// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

use objc2::rc::Retained;
use objc2::runtime::ProtocolObject;
use objc2_metal::{MTLCommandQueue, MTLDevice, MTLTexture};
use skia_safe::gpu::mtl;

use wgpu_29 as wgpu;

/// Matches the limit wgpu's own Metal backend uses when creating its command queue
/// (`wgpu_hal::metal::adapter::MAX_COMMAND_BUFFERS`).
const MAX_COMMAND_BUFFERS: usize = 4096;

/// Creates an `MTLCommandQueue` on `device`, sized like wgpu's own queue so the two agree on the
/// in-flight command-buffer limit.
fn make_command_queue(
    device: &ProtocolObject<dyn MTLDevice>,
) -> Option<Retained<ProtocolObject<dyn MTLCommandQueue>>> {
    device.newCommandQueueWithMaxCommandBufferCount(MAX_COMMAND_BUFFERS)
}

/// # Safety
/// `metal_handle` must be a valid Metal texture handle for the lifetime of the returned Surface.
unsafe fn wrap_metal_texture(
    width: i32,
    height: i32,
    gr_context: &mut skia_safe::gpu::DirectContext,
    metal_handle: mtl::Handle,
    color_type: skia_safe::ColorType,
) -> Option<skia_safe::Surface> {
    unsafe {
        let texture_info = mtl::TextureInfo::new(metal_handle);
        let backend_render_target =
            skia_safe::gpu::backend_render_targets::make_mtl((width, height), &texture_info);
        skia_safe::gpu::surfaces::wrap_backend_render_target(
            gr_context,
            &backend_render_target,
            skia_safe::gpu::SurfaceOrigin::TopLeft,
            color_type,
            None,
            None,
        )
    }
}

/// # Safety
/// The caller must ensure `texture` was created by a Metal-backed wgpu device and remains
/// valid for the lifetime of the returned `skia_safe::Surface`.
pub unsafe fn make_metal_surface(
    gr_context: &mut skia_safe::gpu::DirectContext,
    texture: &wgpu::Texture,
) -> Option<skia_safe::Surface> {
    // SAFETY: texture is borrowed for the duration of this call; the Metal handle is copied
    // into Skia's internal BackendRenderTarget via wrap_metal_texture.
    unsafe {
        let metal_texture = texture.as_hal::<wgpu::wgc::api::Metal>()?;
        let handle =
            metal_texture.raw_handle() as *const ProtocolObject<dyn MTLTexture> as mtl::Handle;
        let size = texture.size();
        let color_type = match texture.format() {
            wgpu::TextureFormat::Bgra8Unorm => skia_safe::ColorType::BGRA8888,
            wgpu::TextureFormat::Rgba8Unorm => skia_safe::ColorType::RGBA8888,
            wgpu::TextureFormat::Rgba8UnormSrgb => skia_safe::ColorType::SRGBA8888,
            _ => return None,
        };
        wrap_metal_texture(size.width as i32, size.height as i32, gr_context, handle, color_type)
    }
}

pub unsafe fn import_metal_texture(
    canvas: &skia_safe::Canvas,
    texture: wgpu::Texture,
) -> Option<skia_safe::Image> {
    unsafe {
        let metal_texture = texture.as_hal::<wgpu::wgc::api::Metal>();

        let texture_info = mtl::TextureInfo::new(metal_texture.unwrap().raw_handle()
            as *const ProtocolObject<dyn MTLTexture>
            as mtl::Handle);
        let size = texture.size();

        let backend_texture = skia_safe::gpu::backend_textures::make_mtl(
            (size.width as _, size.height as _),
            skia_safe::gpu::Mipmapped::No,
            &texture_info,
            "Borrowed Metal texture",
        );
        Some(
            skia_safe::image::Image::from_texture(
                canvas.recording_context().as_mut().unwrap(),
                &backend_texture,
                skia_safe::gpu::SurfaceOrigin::TopLeft,
                match texture.format() {
                    wgpu::TextureFormat::Rgba8Unorm => skia_safe::ColorType::RGBA8888,
                    wgpu::TextureFormat::Rgba8UnormSrgb => skia_safe::ColorType::SRGBA8888,
                    _ => return None,
                },
                skia_safe::AlphaType::Unpremul,
                None,
            )
            .unwrap(),
        )
    }
}

/// Builds the Skia Metal context.
///
/// When `shared_command_queue` is `Some`, it is the `MTLCommandQueue` that wgpu's device was also
/// built from (see [`create_shared_metal_device_queue`]). Sharing one queue lets Metal's automatic
/// per-queue hazard tracking order wgpu's work (e.g. rendering into a texture) before Skia samples
/// it, avoiding reads of not-yet-produced (magenta under Metal validation) content.
///
/// When it is `None` (a developer-provided `Manual` device/queue, whose `MTLCommandQueue` wgpu 29
/// doesn't expose), Skia falls back to its own queue. Cross-queue accesses are then not
/// synchronized; see the note in `WGPUSurface::import_wgpu_texture`.
pub fn make_metal_context(
    device: &wgpu::Device,
    _queue: &wgpu::Queue,
    shared_command_queue: Option<*const core::ffi::c_void>,
) -> Option<skia_safe::gpu::DirectContext> {
    let metal_device = unsafe { device.as_hal::<wgpu::wgc::api::Metal>() }?;
    let metal_device_raw: &Retained<ProtocolObject<dyn MTLDevice>> = metal_device.raw_device();

    // When wgpu's queue isn't shared with us, create our own; keep it owned so it outlives the
    // `BackendContext` creation below, which retains it.
    let owned_command_queue = match shared_command_queue {
        Some(_) => None,
        None => Some(make_command_queue(metal_device_raw)?),
    };
    let command_queue_handle: mtl::Handle = match &owned_command_queue {
        Some(queue) => Retained::as_ptr(queue) as mtl::Handle,
        None => shared_command_queue.expect("present when we didn't create our own queue") as _,
    };

    let backend = unsafe {
        mtl::BackendContext::new(
            Retained::as_ptr(metal_device_raw) as mtl::Handle,
            command_queue_handle,
        )
    };

    skia_safe::gpu::direct_contexts::make_metal(&backend, None)
}

/// Creates the wgpu device and queue from an `MTLCommandQueue` we allocate ourselves, and returns
/// that queue through `shared_command_queue_out` so the Skia context can submit on it too.
///
/// This is the device/queue factory passed to `init_instance_adapter_device_queue_surface` for the
/// Metal backend when Slint owns device creation. wgpu 29 doesn't let us read back the queue it would
/// create itself, so we substitute our own before building the device.
///
/// TODO(wgpu>29): once a wgpu release restores `Queue::as_raw` (gfx-rs/wgpu#9560), drop this whole
/// hal-interop dance and just read the queue back from the device wgpu creates normally.
pub fn create_shared_metal_device_queue(
    adapter: &wgpu::Adapter,
    descriptor: &wgpu::DeviceDescriptor<'_>,
    shared_command_queue_out: &mut Option<Retained<ProtocolObject<dyn MTLCommandQueue>>>,
) -> Result<(wgpu::Device, wgpu::Queue), Box<dyn std::error::Error + Send + Sync + 'static>> {
    // The hal `Adapter::open` trait method; aliased so it doesn't clash with `wgpu::Adapter`.
    use wgpu::hal::Adapter as _;

    unsafe {
        let hal_adapter = adapter
            .as_hal::<wgpu::wgc::api::Metal>()
            .ok_or("Skia: expected a Metal hal adapter to share the command queue")?;

        // `open` also builds a queue we can't read back, so we replace it with our own below.
        let mut open_device = hal_adapter
            .open(
                descriptor.required_features,
                &descriptor.required_limits,
                &descriptor.memory_hints,
            )
            .map_err(|e| format!("Skia: failed to open Metal device for queue sharing: {e}"))?;

        let metal_device: Retained<ProtocolObject<dyn MTLDevice>> =
            open_device.device.raw_device().clone();

        let shared_command_queue = make_command_queue(&metal_device)
            .ok_or("Skia: failed to create a shared Metal command queue")?;

        // Mirror wgpu's own timestamp-period heuristic (wgpu_hal::metal::adapter::open). Slint never
        // reads GPU timestamps, so the exact value only matters for API parity.
        let timestamp_period =
            if metal_device.name().to_string().starts_with("Intel") { 83.333 } else { 1.0 };

        open_device.queue =
            wgpu::hal::metal::Queue::queue_from_raw(shared_command_queue.clone(), timestamp_period);

        let (device, queue) = adapter
            .create_device_from_hal::<wgpu::wgc::api::Metal>(open_device, descriptor)
            .map_err(|e| format!("Skia: failed to create wgpu device from shared queue: {e}"))?;

        *shared_command_queue_out = Some(shared_command_queue);

        Ok((device, queue))
    }
}
