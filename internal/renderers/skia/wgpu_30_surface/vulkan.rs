// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

use crate::vulkan_handle;
use skia_safe::gpu::vk;

use wgpu_30 as wgpu;

// Raw values of the Vulkan handles behind wgpu-hal's objects, for Skia's `vk` API. The
// parameter types pin which `ash::vk` handle each accessor returns; see `vulkan_handle`.
fn vk_instance(device: &wgpu::hal::vulkan::Device) -> u64 {
    unsafe { vulkan_handle::dispatchable(device.shared_instance().raw_instance().handle()) }
}
fn vk_physical_device(device: &wgpu::hal::vulkan::Device) -> u64 {
    unsafe { vulkan_handle::dispatchable(device.raw_physical_device()) }
}
fn vk_device(device: &wgpu::hal::vulkan::Device) -> u64 {
    unsafe { vulkan_handle::dispatchable(device.raw_device().handle()) }
}
fn vk_queue(queue: &wgpu::hal::vulkan::Queue) -> u64 {
    unsafe { vulkan_handle::dispatchable(queue.as_raw()) }
}
fn vk_image(texture: &wgpu::hal::vulkan::Texture) -> u64 {
    unsafe { vulkan_handle::non_dispatchable(texture.raw_handle()) }
}

fn vk_format_and_color_type(
    format: wgpu::TextureFormat,
) -> Option<(skia_safe::gpu::vk::Format, skia_safe::ColorType)> {
    match format {
        wgpu::TextureFormat::Rgba8Unorm => {
            Some((skia_safe::gpu::vk::Format::R8G8B8A8_UNORM, skia_safe::ColorType::RGBA8888))
        }
        wgpu::TextureFormat::Rgba8UnormSrgb => {
            Some((skia_safe::gpu::vk::Format::R8G8B8A8_SRGB, skia_safe::ColorType::SRGBA8888))
        }
        wgpu::TextureFormat::Bgra8Unorm => {
            Some((skia_safe::gpu::vk::Format::B8G8R8A8_UNORM, skia_safe::ColorType::BGRA8888))
        }
        _ => None,
    }
}

/// # Safety
/// `vk_image_raw` must be a valid Vulkan image handle for the lifetime of the returned Surface.
unsafe fn wrap_vulkan_texture(
    width: i32,
    height: i32,
    gr_context: &mut skia_safe::gpu::DirectContext,
    vk_image_raw: u64,
    vk_format: skia_safe::gpu::vk::Format,
    color_type: skia_safe::ColorType,
    layout: skia_safe::gpu::vk::ImageLayout,
    color_space: skia_safe::ColorSpace,
) -> Option<skia_safe::Surface> {
    unsafe {
        let texture_info = &skia_safe::gpu::vk::ImageInfo::new(
            vk_image_raw as _,
            skia_safe::gpu::vk::Alloc::default(),
            skia_safe::gpu::vk::ImageTiling::OPTIMAL,
            layout,
            vk_format,
            1,
            None,
            None,
            None,
            None,
        );
        let backend_render_target =
            skia_safe::gpu::backend_render_targets::make_vk((width, height), texture_info);
        skia_safe::gpu::surfaces::wrap_backend_render_target(
            gr_context,
            &backend_render_target,
            skia_safe::gpu::SurfaceOrigin::TopLeft,
            color_type,
            color_space,
            None,
        )
    }
}

/// # Safety
/// The caller must ensure `texture` was created by a Vulkan-backed wgpu device and remains
/// valid for the lifetime of the returned `skia_safe::Surface`.
///
/// `layout` is the layout wgpu leaves the image in, which Skia transitions away from before
/// its first draw. Getting it wrong doesn't just trip the validation layers, it makes the
/// barrier Skia emits name the wrong source layout.
pub unsafe fn make_vulkan_surface(
    gr_context: &mut skia_safe::gpu::DirectContext,
    texture: &wgpu::Texture,
    layout: skia_safe::gpu::vk::ImageLayout,
) -> Option<skia_safe::Surface> {
    // SAFETY: texture is borrowed for the duration of this call; the Vulkan handle is copied
    // into Skia's internal BackendRenderTarget via wrap_vulkan_texture.
    unsafe {
        let vulkan_texture = texture.as_hal::<wgpu::wgc::api::Vulkan>()?;
        let vk_image_raw = vk_image(&vulkan_texture);
        let size = texture.size();
        let (vk_format, color_type) = vk_format_and_color_type(texture.format())?;
        wrap_vulkan_texture(
            size.width as i32,
            size.height as i32,
            gr_context,
            vk_image_raw,
            vk_format,
            color_type,
            layout,
            super::attachment_color_space(texture),
        )
    }
}

/// Records the transition back to `PRESENT_SRC_KHR` and flushes it.
///
/// wgpu's tracker still believes the swapchain image is in its `PRESENT` state, because that is
/// where wgpu itself left it before Skia took over. Leaving it in Skia's color-attachment layout
/// would make the barrier `wgpu::Queue::present` emits name a source layout the image isn't in.
pub fn release_vulkan_swapchain_surface(
    gr_context: &mut skia_safe::gpu::DirectContext,
    skia_surface: &mut skia_safe::Surface,
    queue_family_index: u32,
) {
    let present_state = skia_safe::gpu::vk::mutable_texture_states::new_vulkan(
        skia_safe::gpu::vk::ImageLayout::PRESENT_SRC_KHR,
        queue_family_index,
    );
    gr_context.flush_surface_with_texture_state(
        skia_surface,
        &skia_safe::gpu::FlushInfo::default(),
        Some(&present_state),
    );
}

/// Extension names as `String`, for [`vk::BackendContextBuilder::with_extensions`]. Vulkan
/// extension names are ASCII, so the lossy conversion can't lose anything.
fn cstr_names(names: &[&'static std::ffi::CStr]) -> Vec<String> {
    names.iter().map(|name| name.to_string_lossy().into_owned()).collect()
}

/// The queue family the wgpu device submits on, which is also the one Skia has to hand the
/// swapchain image back to.
///
/// # Safety
/// `device` must be backed by the Vulkan wgpu backend.
pub unsafe fn queue_family_index(device: &wgpu::Device) -> Option<u32> {
    unsafe { Some(device.as_hal::<wgpu::wgc::api::Vulkan>()?.queue_family_index()) }
}

#[cfg_attr(not(feature = "unstable-wgpu-30"), allow(dead_code))]
pub unsafe fn import_vulkan_texture(
    canvas: &skia_safe::Canvas,
    texture: wgpu::Texture,
) -> Option<skia_safe::Image> {
    unsafe {
        let color_space = super::sampled_texture_color_space(&texture);
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

        let texture_info = &skia_safe::gpu::vk::ImageInfo::new(
            vk_image(&vulkan_texture.unwrap()) as _,
            alloc,
            skia_safe::gpu::vk::ImageTiling::OPTIMAL,
            skia_safe::gpu::vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL,
            vk_format,
            1,
            None,
            None,
            None,
            None,
        );

        let size = texture.size();

        let backend_texture = skia_safe::gpu::backend_textures::make_vk(
            (size.width as _, size.height as _),
            texture_info,
            "Borrowed Vulkan texture",
        );
        Some(
            skia_safe::image::Image::from_texture(
                canvas.recording_context().as_mut().unwrap(),
                &backend_texture,
                skia_safe::gpu::SurfaceOrigin::TopLeft,
                color_type,
                skia_safe::AlphaType::Unpremul,
                color_space,
            )
            .unwrap(),
        )
    }
}

pub unsafe fn make_vulkan_context(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
) -> Option<skia_safe::gpu::DirectContext> {
    unsafe {
        let vulkan_device = device.as_hal::<wgpu::wgc::api::Vulkan>()?;
        let vulkan_queue = queue.as_hal::<wgpu::wgc::api::Vulkan>()?;

        let shared_instance = vulkan_device.shared_instance();

        // Skia only asks about the instance and device handed to it below, or about the loader
        // itself with a null instance. wgpu-hal has those handles in typed form, so use them
        // rather than rebuilding them from the raw values Skia passes back.
        let get_proc = |of| {
            let result = match of {
                skia_safe::gpu::vk::GetProcOf::Instance(instance, name) => {
                    let instance_handle = if instance.is_null() {
                        Default::default() // VK_NULL_HANDLE
                    } else {
                        let handle = shared_instance.raw_instance().handle();
                        debug_assert_eq!(instance as u64, vk_instance(&vulkan_device));
                        handle
                    };
                    shared_instance.entry().get_instance_proc_addr(instance_handle, name)
                }
                skia_safe::gpu::vk::GetProcOf::Device(device, name) => {
                    let device_handle = vulkan_device.raw_device().handle();
                    debug_assert_eq!(device as u64, vk_device(&vulkan_device));
                    shared_instance.raw_instance().get_device_proc_addr(device_handle, name)
                }
            };

            match result {
                Some(f) => f as _,
                None => {
                    //println!("resolve of {} failed", of.name().to_str().unwrap());
                    core::ptr::null()
                }
            }
        };

        // Skia gates features on the extensions it's told about, and rejects anything it
        // considers unsupported: without `VK_KHR_swapchain` in this list it refuses to wrap an
        // image that's in `PRESENT_SRC_KHR` layout, which is how wgpu hands out swapchain
        // images. Hand it what wgpu actually enabled, no more.
        let instance_extensions = cstr_names(shared_instance.extensions());
        let device_extensions = cstr_names(vulkan_device.enabled_device_extensions());

        // WGPU 29 is locked to vulkan 1.3 and skia assumes the highest vulkan API version of the
        // physical device is chosen, causing it to ask for unsupported features/functions.
        let backend = vk::BackendContext::new_builder(
            vk_instance(&vulkan_device) as _,
            vk_physical_device(&vulkan_device) as _,
            vk_device(&vulkan_device) as _,
            (vk_queue(&vulkan_queue) as _, vulkan_device.queue_family_index() as _),
            &get_proc,
            Some(vk::Version::new(1, 3, 0)),
        )
        .with_extensions(
            &instance_extensions.iter().map(String::as_str).collect::<Vec<_>>(),
            &device_extensions.iter().map(String::as_str).collect::<Vec<_>>(),
        )
        .build();

        skia_safe::gpu::direct_contexts::make_vulkan(&backend, None)
    }
}
