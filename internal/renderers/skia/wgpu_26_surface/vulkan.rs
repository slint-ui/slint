// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

use i_slint_core::api::PhysicalSize as PhysicalWindowSize;

use ash::vk::Handle;
use skia_safe::gpu::vk;

use wgpu_26 as wgpu;

pub unsafe fn make_vulkan_surface(
    size: PhysicalWindowSize,
    gr_context: &mut skia_safe::gpu::DirectContext,
    frame: &wgpu::SurfaceTexture,
) -> Option<skia_safe::Surface> {
    let vulkan_texture = frame.texture.as_hal::<wgpu::wgc::api::Vulkan>();

    let alloc = skia_safe::gpu::vk::Alloc::default();

    let (vk_format, color_type) = match frame.texture.format() {
        wgpu::TextureFormat::Rgba8Unorm => {
            (skia_safe::gpu::vk::Format::R8G8B8A8_UNORM, skia_safe::ColorType::RGBA8888)
        }
        wgpu::TextureFormat::Rgba8UnormSrgb => {
            (skia_safe::gpu::vk::Format::R8G8B8A8_SRGB, skia_safe::ColorType::SRGBA8888)
        }
        wgpu::TextureFormat::Bgra8Unorm => {
            (skia_safe::gpu::vk::Format::B8G8R8A8_UNORM, skia_safe::ColorType::BGRA8888)
        }
        _ => return None,
    };

    let texture_info = &unsafe {
        skia_safe::gpu::vk::ImageInfo::new(
            vulkan_texture.unwrap().raw_handle().as_raw() as _,
            alloc,
            skia_safe::gpu::vk::ImageTiling::OPTIMAL,
            skia_safe::gpu::vk::ImageLayout::COLOR_ATTACHMENT_OPTIMAL,
            vk_format,
            1,
            None,
            None,
            None,
            None,
        )
    };

    let backend_render_target = skia_safe::gpu::backend_render_targets::make_vk(
        (size.width as i32, size.height as i32),
        &texture_info,
    );

    skia_safe::gpu::surfaces::wrap_backend_render_target(
        gr_context,
        &backend_render_target,
        skia_safe::gpu::SurfaceOrigin::TopLeft,
        color_type,
        None,
        None,
    )
}

pub unsafe fn import_vulkan_texture(
    canvas: &skia_safe::Canvas,
    texture: wgpu::Texture,
) -> Option<skia_safe::Image> {
    let vulkan_texture = texture.as_hal::<wgpu::wgc::api::Vulkan>();

    let alloc = skia_safe::gpu::vk::Alloc::default();

    let (vk_format, color_type) = match texture.format() {
        wgpu::TextureFormat::Rgba8Unorm => {
            (skia_safe::gpu::vk::Format::R8G8B8A8_UNORM, skia_safe::ColorType::RGBA8888)
        }
        wgpu::TextureFormat::Rgba8UnormSrgb => {
            (skia_safe::gpu::vk::Format::R8G8B8A8_SRGB, skia_safe::ColorType::SRGBA8888)
        }
        wgpu::TextureFormat::Bgra8Unorm => {
            (skia_safe::gpu::vk::Format::B8G8R8A8_UNORM, skia_safe::ColorType::BGRA8888)
        }
        _ => return None,
    };

    let texture_info = &unsafe {
        skia_safe::gpu::vk::ImageInfo::new(
            vulkan_texture.unwrap().raw_handle().as_raw() as _,
            alloc,
            skia_safe::gpu::vk::ImageTiling::OPTIMAL,
            skia_safe::gpu::vk::ImageLayout::COLOR_ATTACHMENT_OPTIMAL,
            vk_format,
            1,
            None,
            None,
            None,
            None,
        )
    };

    let size = texture.size();

    let backend_texture = skia_safe::gpu::backend_textures::make_vk(
        (size.width as _, size.height as _),
        &texture_info,
        "Borrowed Vulkan texture",
    );
    Some(
        skia_safe::image::Image::from_texture(
            canvas.recording_context().as_mut().unwrap(),
            &backend_texture,
            skia_safe::gpu::SurfaceOrigin::TopLeft,
            color_type,
            skia_safe::AlphaType::Unpremul,
            None,
        )
        .unwrap(),
    )
}

pub unsafe fn make_vulkan_context(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
) -> Option<skia_safe::gpu::DirectContext> {
    let maybe_vulkan_device = device.as_hal::<wgpu::wgc::api::Vulkan>();
    let maybe_vulkan_queue = queue.as_hal::<wgpu::wgc::api::Vulkan>();

    maybe_vulkan_device.and_then(|vulkan_device| {
        maybe_vulkan_queue.map(|vulkan_queue| {
            let vulkan_queue_raw = vulkan_queue.as_raw();

            let get_proc = |of| {
                let result = match of {
                    skia_safe::gpu::vk::GetProcOf::Instance(instance, name) => vulkan_device
                        .shared_instance()
                        .entry()
                        .get_instance_proc_addr(ash::vk::Instance::from_raw(instance as _), name),
                    skia_safe::gpu::vk::GetProcOf::Device(device, name) => vulkan_device
                        .shared_instance()
                        .raw_instance()
                        .get_device_proc_addr(ash::vk::Device::from_raw(device as _), name),
                };

                match result {
                    Some(f) => f as _,
                    None => {
                        //println!("resolve of {} failed", of.name().to_str().unwrap());
                        core::ptr::null()
                    }
                }
            };

            let backend = vk::BackendContext::new(
                vulkan_device.shared_instance().raw_instance().handle().as_raw() as _,
                vulkan_device.raw_physical_device().as_raw() as _,
                vulkan_device.raw_device().handle().as_raw() as _,
                (vulkan_queue_raw.as_raw() as _, vulkan_device.queue_family_index() as _),
                &get_proc,
            );
            skia_safe::gpu::direct_contexts::make_vulkan(&backend, None)
        })
    })?
}
