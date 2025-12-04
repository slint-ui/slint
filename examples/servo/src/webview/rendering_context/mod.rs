// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT

//! This module handles the rendering context for the Servo WebView.
//!
//! It provides abstractions for GPU and software rendering, integrating with
//! `surfman` for cross-platform surface management and `wgpu` for modern graphics API support.
//!
//! Key components:
//! - `GPURenderingContext`: Manages GPU resources and swap chains.
//! - `SurfmanRenderingContext`: Low-level integration with `surfman` and OpenGL.
//! - `ServoRenderingAdapter`: Adapter trait for integrating Servo's rendering with Slint.
mod gpu_rendering_context;

mod servo_rendering_adapter;
mod surfman_context;
mod utils;

pub use gpu_rendering_context::GPURenderingContext;
pub use servo_rendering_adapter::ServoRenderingAdapter;
pub use servo_rendering_adapter::try_create_gpu_context;

#[cfg(target_vendor = "apple")]
mod metal;

#[cfg(any(target_os = "linux", target_os = "android"))]
mod vulkan;
