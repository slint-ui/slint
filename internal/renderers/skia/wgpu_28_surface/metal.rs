// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

use foreign_types::ForeignType;

use skia_safe::gpu::mtl;

use wgpu_28 as wgpu;

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
        let handle = metal_texture.raw_handle().as_ptr() as mtl::Handle;
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

        let texture_info =
            mtl::TextureInfo::new(metal_texture.unwrap().raw_handle().as_ptr() as mtl::Handle);
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

pub fn make_metal_context(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
) -> Option<skia_safe::gpu::DirectContext> {
    let backend = unsafe {
        let maybe_metal_device = device.as_hal::<wgpu::wgc::api::Metal>();
        let maybe_metal_queue = queue.as_hal::<wgpu::wgc::api::Metal>();

        maybe_metal_device.and_then(|metal_device| {
            let metal_device_raw = metal_device.raw_device();

            maybe_metal_queue.map(|metal_queue| {
                let metal_queue_raw = &*metal_queue.as_raw().lock();
                mtl::BackendContext::new(
                    metal_device_raw.as_ptr() as mtl::Handle,
                    metal_queue_raw.as_ptr() as mtl::Handle,
                )
            })
        })?
    };

    skia_safe::gpu::direct_contexts::make_metal(&backend, None)
}
