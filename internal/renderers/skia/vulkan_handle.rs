// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

// cSpell: ignore dispatchable
//! Raw values of the Vulkan handles wgpu-hal hands out.
//!
//! wgpu-hal exposes its Vulkan objects as `ash::vk` handle types without re-exporting `ash`,
//! and Skia's `vk` API takes the raw values.
//! Every `ash::vk` handle is a `#[repr(transparent)]` wrapper mirroring the C type:
//! a pointer for dispatchable handles (`VkInstance`, `VkQueue`, ...)
//! and a `u64` for non-dispatchable ones (`VkImage`, ...).
//! Reading the wrapper is what `ash::vk::Handle::as_raw` does, minus the dependency.

/// Returns the raw value of a dispatchable handle.
///
/// # Safety
/// `handle` must be one of `ash::vk`'s dispatchable handle types.
pub unsafe fn dispatchable<H: Copy>(handle: H) -> u64 {
    const { assert!(size_of::<H>() == size_of::<*mut u8>()) };
    unsafe { core::mem::transmute_copy::<H, *mut u8>(&handle) as u64 }
}

/// Returns the raw value of a non-dispatchable handle.
///
/// # Safety
/// `handle` must be one of `ash::vk`'s non-dispatchable handle types.
pub unsafe fn non_dispatchable<H: Copy>(handle: H) -> u64 {
    const { assert!(size_of::<H>() == size_of::<u64>()) };
    unsafe { core::mem::transmute_copy::<H, u64>(&handle) }
}
