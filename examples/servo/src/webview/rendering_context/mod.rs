// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT

mod gpu_rendering_context;

mod servo_rendering_adapter;
mod surfman_context;

pub use gpu_rendering_context::GPURenderingContext;
pub use servo_rendering_adapter::ServoRenderingAdapter;
pub use servo_rendering_adapter::try_create_gpu_context;

#[cfg(target_vendor = "apple")]
mod metal;

#[cfg(any(target_os = "linux", target_os = "android"))]
mod vulkan;
