// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

//! `take_snapshot()` of a window that Skia renders with wgpu 29,
//! and `render_to_texture()` into a wgpu 29 texture.
//!
//! The tests pass without rendering on a machine without a WGPU adapter that Skia can use.

#![cfg(feature = "unstable-wgpu-29")]

use i_slint_core::graphics::wgpu_29 as slint_wgpu;
use i_slint_renderer_skia::SkiaWGPU29Renderer as SkiaWGPURenderer;
use wgpu_29 as wgpu;

fn mapped_bytes(buffer: &wgpu::Buffer) -> Vec<u8> {
    buffer.slice(..).get_mapped_range().to_vec()
}

include!("common/wgpu_snapshot_body.rs");
