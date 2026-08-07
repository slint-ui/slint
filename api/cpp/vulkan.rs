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
