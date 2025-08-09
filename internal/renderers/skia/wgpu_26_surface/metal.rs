// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

use foreign_types::ForeignType;
use i_slint_core::api::PhysicalSize as PhysicalWindowSize;

use skia_safe::gpu::mtl;

use wgpu_26 as wgpu;

pub unsafe fn make_metal_surface(
    size: PhysicalWindowSize,
    gr_context: &mut skia_safe::gpu::DirectContext,
    frame: &wgpu::SurfaceTexture,
) -> Option<skia_safe::Surface> {
    let metal_texture = frame.texture.as_hal::<wgpu::wgc::api::Metal>();

    let texture_info =
        mtl::TextureInfo::new(metal_texture.unwrap().raw_handle().as_ptr() as mtl::Handle);

    let backend_render_target = skia_safe::gpu::backend_render_targets::make_mtl(
        (size.width as i32, size.height as i32),
        &texture_info,
    );

    skia_safe::gpu::surfaces::wrap_backend_render_target(
        gr_context,
        &backend_render_target,
        skia_safe::gpu::SurfaceOrigin::TopLeft,
        skia_safe::ColorType::BGRA8888,
        None,
        None,
    )
}

pub unsafe fn import_metal_texture(
    canvas: &skia_safe::Canvas,
    texture: wgpu::Texture,
) -> Option<skia_safe::Image> {
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

pub fn make_metal_context(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
) -> Option<skia_safe::gpu::DirectContext> {
    let backend = unsafe {
        let maybe_metal_device = device.as_hal::<wgpu::wgc::api::Metal>();
        let maybe_metal_queue = queue.as_hal::<wgpu::wgc::api::Metal>();

        maybe_metal_device.and_then(|metal_device| {
            let metal_device_raw = &*metal_device.raw_device().lock();

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
