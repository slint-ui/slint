// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

//! `take_snapshot()` of a window that Skia renders with wgpu 30,
//! and `render_to_texture()` into a wgpu 30 texture.
//!
//! The tests pass without rendering on a machine without a WGPU adapter that Skia can use.

#![cfg(feature = "unstable-wgpu-30")]

use i_slint_core::graphics::wgpu_30 as slint_wgpu;
use i_slint_renderer_skia::SkiaWGPU30Renderer as SkiaWGPURenderer;
use wgpu_30 as wgpu;

fn mapped_bytes(buffer: &wgpu::Buffer) -> Vec<u8> {
    buffer.slice(..).get_mapped_range().unwrap().to_vec()
}

include!("common/wgpu_snapshot_body.rs");
