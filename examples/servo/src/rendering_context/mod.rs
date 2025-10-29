// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT

mod gpu_rendering_context;

mod servo_rendering_adapter;
mod surfman_context;

pub use servo_rendering_adapter::ServoRenderingAdapter;

#[cfg(not(target_os = "android"))]
pub use gpu_rendering_context::GPURenderingContext;

#[cfg(not(target_os = "android"))]
pub use servo_rendering_adapter::try_create_gpu_context;

#[cfg(target_os = "android")]
pub use servo_rendering_adapter::create_software_context;

#[cfg(target_vendor = "apple")]
mod metal;
