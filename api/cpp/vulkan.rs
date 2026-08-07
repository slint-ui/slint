// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

//! FFI for reaching the Vulkan objects the renderer is using, from C++.
//!
//! Slint renders through wgpu here and asks it for its Vulkan backend. Rather than exposing wgpu
//! to C++, which has no C API that reaches down to the native handles, this hands over the
//! handles wgpu created. The C++ side wraps them in `slint::vulkan` (see `slint-vulkan.h`).

use alloc::boxed::Box;
use ash::vk::Handle;
use core::ffi::{c_char, c_void};

use i_slint_core::api::{RenderingState, SetRenderingNotifierError};
use i_slint_core::graphics::wgpu_30::wgpu;
use i_slint_core::window::{WindowAdapter, ffi::WindowAdapterRcOpaque};

/// A `PFN_vkVoidFunction`, the type `vkGetInstanceProcAddr` resolves to.
pub type VoidFunction = Option<unsafe extern "C" fn()>;

/// The Vulkan objects the renderer is rendering with.
///
/// All of these are owned by the renderer and stay valid between
/// [`RenderingState::RenderingSetup`] and [`RenderingState::RenderingTeardown`]. The handles are
/// passed as pointers because cbindgen can't name Vulkan's types; the C++ side casts them back.
#[repr(C)]
pub struct VulkanApi {
    /// The `VkInstance` the renderer created.
    pub instance: *mut c_void,
    /// The `VkPhysicalDevice` the renderer picked.
    pub physical_device: *mut c_void,
    /// The `VkDevice` the renderer created. Allocate from this one, or Slint can't use what you
    /// give it.
    pub device: *mut c_void,
    /// The `VkQueue` the renderer submits on. Submitting on this same queue is what orders your
    /// commands against Slint's, without any semaphore of your own.
    pub queue: *mut c_void,
    /// The family `queue` belongs to.
    pub queue_family_index: u32,
    /// `vkGetInstanceProcAddr` from the loader the renderer is using. Resolve entry points
    /// through this to be sure of talking to the same driver.
    pub get_instance_proc_addr:
        Option<unsafe extern "C" fn(instance: *mut c_void, name: *const c_char) -> VoidFunction>,
    /// The `wgpu::Device` behind all of the above, for [`slint_vulkan_texture_import`]. Borrowed
    /// for the duration of the notifier callback only.
    pub renderer_device: *const c_void,
    /// The `wgpu::Queue` behind [`Self::queue`], likewise borrowed for the callback only.
    pub renderer_queue: *const c_void,
}

/// The pixel formats an imported texture can have.
///
/// Narrower than what Vulkan allows: these are the ones the renderer can sample from.
#[repr(u8)]
#[derive(Clone, Copy)]
pub enum VulkanTextureFormat {
    /// `VK_FORMAT_R8G8B8A8_UNORM`
    Rgba8Unorm,
    /// `VK_FORMAT_R8G8B8A8_SRGB`
    Rgba8UnormSrgb,
}

impl VulkanTextureFormat {
    fn to_wgpu(self) -> wgpu::TextureFormat {
        match self {
            Self::Rgba8Unorm => wgpu::TextureFormat::Rgba8Unorm,
            Self::Rgba8UnormSrgb => wgpu::TextureFormat::Rgba8UnormSrgb,
        }
    }
}

/// Describes the `VkImage` an application is handing to the renderer.
#[repr(C)]
pub struct VulkanTextureImportInfo {
    /// The `VkImage`, allocated from [`VulkanApi::device`] and still owned by the application.
    ///
    /// It must have been created with `VK_IMAGE_USAGE_COLOR_ATTACHMENT_BIT` and
    /// `VK_IMAGE_USAGE_SAMPLED_BIT`, one mip level, one sample and one array layer.
    pub image: u64,
    /// Width in pixels, as passed to `vkCreateImage`.
    pub width: u32,
    /// Height in pixels, as passed to `vkCreateImage`.
    pub height: u32,
    /// Format, as passed to `vkCreateImage`.
    pub format: VulkanTextureFormat,
    /// Invoked once the renderer is done with the image, which is when it becomes safe to
    /// destroy it. Not before: an image handed to the scene outlives the frame.
    pub on_released: Option<extern "C" fn(user_data: *mut c_void)>,
    /// Passed back to `on_released` untouched.
    pub user_data: *mut c_void,
}

/// The renderer's side of an imported `VkImage`, see [`slint_vulkan_texture_import`].
pub struct VulkanTexture {
    texture: wgpu::Texture,
    device: wgpu::Device,
    queue: wgpu::Queue,
}

/// Registers a rendering notifier that reports the renderer's Vulkan objects.
///
/// The callback receives a null `api` whenever the renderer isn't running on Vulkan, so that a
/// C++ application still sees the state changes and can report the mismatch itself.
///
/// # Safety
/// `handle` must be a valid window adapter, and `user_data` is passed back untouched.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn slint_windowrc_set_vulkan_rendering_notifier(
    handle: *const WindowAdapterRcOpaque,
    callback: extern "C" fn(
        rendering_state: RenderingState,
        api: *const VulkanApi,
        user_data: *mut c_void,
    ),
    drop_user_data: extern "C" fn(*mut c_void),
    user_data: *mut c_void,
    error: *mut SetRenderingNotifierError,
) -> bool {
    struct VulkanNotifier {
        callback: extern "C" fn(RenderingState, *const VulkanApi, *mut c_void),
        drop_user_data: extern "C" fn(*mut c_void),
        user_data: *mut c_void,
    }

    impl Drop for VulkanNotifier {
        fn drop(&mut self) {
            (self.drop_user_data)(self.user_data)
        }
    }

    impl i_slint_core::api::RenderingNotifier for VulkanNotifier {
        fn notify(&mut self, state: RenderingState, graphics_api: &i_slint_core::api::GraphicsAPI) {
            let i_slint_core::api::GraphicsAPI::WGPU30 { device, queue, .. } = graphics_api else {
                (self.callback)(state, core::ptr::null(), self.user_data);
                return;
            };

            // SAFETY: the hal guards don't outlive this block, and the handles read out of them
            // stay valid as long as the wgpu device and queue do, which outlives the callback.
            let api = unsafe {
                let (Some(hal_device), Some(hal_queue)) = (
                    device.as_hal::<wgpu::wgc::api::Vulkan>(),
                    queue.as_hal::<wgpu::wgc::api::Vulkan>(),
                ) else {
                    // wgpu is running on some other backend.
                    (self.callback)(state, core::ptr::null(), self.user_data);
                    return;
                };

                let instance = hal_device.shared_instance();
                VulkanApi {
                    instance: instance.raw_instance().handle().as_raw() as _,
                    physical_device: hal_device.raw_physical_device().as_raw() as _,
                    device: hal_device.raw_device().handle().as_raw() as _,
                    queue: hal_queue.as_raw().as_raw() as _,
                    queue_family_index: hal_device.queue_family_index(),
                    renderer_device: core::ptr::from_ref(device).cast(),
                    renderer_queue: core::ptr::from_ref(queue).cast(),
                    get_instance_proc_addr: core::mem::transmute::<
                        ash::vk::PFN_vkGetInstanceProcAddr,
                        Option<unsafe extern "C" fn(*mut c_void, *const c_char) -> VoidFunction>,
                    >(
                        instance.entry().static_fn().get_instance_proc_addr
                    ),
                }
            };

            (self.callback)(state, &api, self.user_data);
        }
    }

    // SAFETY: the caller guarantees `handle` refers to a live window adapter.
    let window = unsafe { &*(handle as *const alloc::rc::Rc<dyn WindowAdapter>) };
    match window.renderer().set_rendering_notifier(Box::new(VulkanNotifier {
        callback,
        drop_user_data,
        user_data,
    })) {
        Ok(()) => true,
        Err(err) => {
            // SAFETY: the caller provides a valid out pointer.
            unsafe { *error = err };
            false
        }
    }
}

/// A pointer the application gave us and that we only ever hand straight back.
#[derive(Clone, Copy)]
struct SendPointer(*mut c_void);

// SAFETY: this is opaque to us. The application owns whatever it refers to, and gets it back
// unchanged; we never dereference it. wgpu requires its drop callbacks to be both, and the
// pointer only travels from the thread that imported the texture back to the main one.
unsafe impl Send for SendPointer {}
unsafe impl Sync for SendPointer {}

/// wgpu retires resources on whichever thread last let go of them, but an application will want
/// to call `vkDestroyImage` where it does the rest of its Vulkan work.
fn release_on_main_thread(on_released: extern "C" fn(*mut c_void), user_data: SendPointer) {
    let posted = i_slint_core::api::invoke_from_event_loop(move || {
        let user_data = user_data;
        on_released(user_data.0)
    });
    if posted.is_err() {
        // No event loop left to post to, so nothing can be mid-frame either.
        on_released(user_data.0);
    }
}

/// Wraps a `VkImage` the application allocated, so that the renderer can show it.
///
/// The image stays owned by the application; the renderer borrows it, and reports through
/// `info.on_released` when that borrow ends. Import once per image and keep the result: importing
/// per frame would restart the renderer's tracking of the image each time.
///
/// Returns null if the image can't be wrapped, which is the case when `api` didn't come from a
/// Vulkan-backed renderer.
///
/// # Safety
/// `info.image` must be a live `VkImage` allocated from [`VulkanApi::device`] that matches
/// `info`'s size and format, and `api` must be one the notifier handed over.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn slint_vulkan_texture_import(
    api: &VulkanApi,
    info: &VulkanTextureImportInfo,
) -> *mut VulkanTexture {
    // SAFETY: the notifier sets these to the device and queue borrowed from the graphics API,
    // and the caller is inside that callback.
    let (Some(device), Some(queue)) =
        (unsafe { (api.renderer_device as *const wgpu::Device).as_ref() }, unsafe {
            (api.renderer_queue as *const wgpu::Queue).as_ref()
        })
    else {
        return core::ptr::null_mut();
    };

    // SAFETY: `api` says the renderer is on Vulkan, so the device has a Vulkan hal.
    let Some(hal_device) = (unsafe { device.as_hal::<wgpu::wgc::api::Vulkan>() }) else {
        return core::ptr::null_mut();
    };

    let size = wgpu::Extent3d { width: info.width, height: info.height, depth_or_array_layers: 1 };
    let format = info.format.to_wgpu();

    let on_released = info.on_released;
    let user_data = SendPointer(info.user_data);
    let drop_callback: wgpu_hal_30::DropCallback = alloc::boxed::Box::new(move || {
        if let Some(on_released) = on_released {
            release_on_main_thread(on_released, user_data);
        }
    });

    // SAFETY: the caller guarantees the image matches `info` and stays alive until
    // `on_released` says the renderer is done with it.
    let hal_texture = unsafe {
        hal_device.texture_from_raw(
            ash::vk::Image::from_raw(info.image),
            &wgpu_hal_30::TextureDescriptor {
                label: Some("slint::vulkan::Texture"),
                size,
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format,
                usage: wgpu::TextureUses::COLOR_TARGET | wgpu::TextureUses::RESOURCE,
                memory_flags: wgpu_hal_30::MemoryFlags::empty(),
                view_formats: alloc::vec::Vec::new(),
            },
            Some(drop_callback),
            // The application allocated the memory and frees it when `on_released` fires.
            wgpu_hal_30::vulkan::TextureMemory::External,
        )
    };
    drop(hal_device);

    // SAFETY: `hal_texture` was just made by this very device.
    let texture = unsafe {
        device.create_texture_from_hal::<wgpu::wgc::api::Vulkan>(
            hal_texture,
            &wgpu::TextureDescriptor {
                label: Some("slint::vulkan::Texture"),
                size,
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format,
                usage: wgpu::TextureUsages::RENDER_ATTACHMENT
                    | wgpu::TextureUsages::TEXTURE_BINDING,
                view_formats: &[],
            },
            // A freshly created VkImage is in `VK_IMAGE_LAYOUT_UNDEFINED`.
            wgpu::TextureUses::UNINITIALIZED,
        )
    };

    alloc::boxed::Box::into_raw(alloc::boxed::Box::new(VulkanTexture {
        texture,
        device: device.clone(),
        queue: queue.clone(),
    }))
}

/// Tells the renderer that the application is about to render into the texture.
///
/// The renderer can't see the application's command buffers, so without this it still believes
/// the image holds whatever it last saw, and the barrier it later emits to sample the image names
/// a source scope that doesn't cover the application's writes.
///
/// Call this before submitting, once per frame that renders into the texture.
#[unsafe(no_mangle)]
pub extern "C" fn slint_vulkan_texture_begin_render(texture: &VulkanTexture) {
    let mut encoder = texture.device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
        label: Some("slint::vulkan::Texture::begin_render"),
    });
    encoder.transition_resources(
        core::iter::empty(),
        core::iter::once(wgpu::TextureTransition {
            texture: &texture.texture,
            selector: None,
            state: wgpu::TextureUses::COLOR_TARGET,
        }),
    );
    texture.queue.submit(Some(encoder.finish()));
}

/// Produces an `Image` referring to the texture, for use as the source of an `Image` element.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn slint_vulkan_texture_to_image(
    texture: &VulkanTexture,
    out: *mut i_slint_core::graphics::Image,
) {
    let image = i_slint_core::graphics::Image::try_from(texture.texture.clone())
        .expect("internal error: the imported texture's format and usage were checked on import");
    // SAFETY: the caller provides an uninitialized but valid out pointer.
    unsafe { core::ptr::write(out, image) };
}

/// Releases the renderer's handle on the texture.
///
/// The `VkImage` may still be in use by frames that haven't finished; `on_released` reports when
/// destroying it is safe.
///
/// # Safety
/// `texture` must come from [`slint_vulkan_texture_import`] and not have been dropped yet.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn slint_vulkan_texture_drop(texture: *mut VulkanTexture) {
    // SAFETY: the caller guarantees ownership.
    drop(unsafe { alloc::boxed::Box::from_raw(texture) });
}
