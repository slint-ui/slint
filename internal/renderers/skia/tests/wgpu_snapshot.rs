// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

//! `take_snapshot()` of a window that Skia renders with WGPU.
//!
//! The tests pass without rendering on a machine without a WGPU adapter that Skia can use.

#![cfg(feature = "unstable-wgpu-30")]

use std::rc::Rc;

use i_slint_core::api::PhysicalSize;
use i_slint_core::platform::{Platform, PlatformError};
use i_slint_core::renderer::Renderer;
use i_slint_core::window::WindowAdapter;
use i_slint_renderer_skia::SkiaWGPU30Renderer;
use slint::{ComponentHandle, Image};
use wgpu_30 as wgpu;

slint::slint! {
    export component TextureCase inherits Window {
        width: 40px;
        height: 40px;
        background: white;
        in-out property <image> source;
        Image {
            width: 100%;
            height: 100%;
            source: root.source;
        }
    }

    export component TranslucentCase inherits Window {
        width: 40px;
        height: 40px;
        background: transparent;
        Rectangle {
            background: #ff000080;
        }
    }
}

const SIZE: u32 = 40;

fn poll_once<F: std::future::Future>(future: F) -> Option<F::Output> {
    let mut ctx = std::task::Context::from_waker(std::task::Waker::noop());
    match std::pin::pin!(future).poll(&mut ctx) {
        std::task::Poll::Ready(result) => Some(result),
        std::task::Poll::Pending => None,
    }
}

#[derive(Clone)]
struct Wgpu {
    instance: wgpu::Instance,
    adapter: wgpu::Adapter,
    device: wgpu::Device,
    queue: wgpu::Queue,
}

struct OffscreenPlatform(Wgpu);

impl Platform for OffscreenPlatform {
    fn create_window_adapter(&self) -> Result<Rc<dyn WindowAdapter>, PlatformError> {
        let Wgpu { instance, adapter, device, queue } = self.0.clone();
        let renderer = SkiaWGPU30Renderer::new(instance, adapter, device, queue)?;
        Ok(Rc::new_cyclic(|self_weak| OffscreenWindow {
            window: i_slint_core::api::Window::new(self_weak.clone() as _),
            renderer,
        }))
    }
}

struct OffscreenWindow {
    window: i_slint_core::api::Window,
    renderer: SkiaWGPU30Renderer,
}

impl WindowAdapter for OffscreenWindow {
    fn window(&self) -> &i_slint_core::api::Window {
        &self.window
    }

    fn size(&self) -> PhysicalSize {
        PhysicalSize::new(SIZE, SIZE)
    }

    fn renderer(&self) -> &dyn Renderer {
        &self.renderer
    }
}

/// Sets up the offscreen platform, or returns `None` when no WGPU adapter for Skia is available.
fn init_offscreen_platform() -> Option<Wgpu> {
    let mut descriptor = wgpu::InstanceDescriptor::new_without_display_handle_from_env();
    descriptor.backends = i_slint_core::graphics::wgpu_30::mask_backends(
        descriptor.backends
            & (wgpu::Backends::VULKAN | wgpu::Backends::METAL | wgpu::Backends::DX12),
        i_slint_core::graphics::wgpu_30::default_backends_to_avoid(),
    );
    let instance = wgpu::Instance::new(descriptor);
    let Some(Ok(adapter)) =
        poll_once(instance.request_adapter(&wgpu::RequestAdapterOptions::default()))
    else {
        return None;
    };
    let Some(Ok((device, queue))) =
        poll_once(adapter.request_device(&wgpu::DeviceDescriptor::default()))
    else {
        return None;
    };
    let wgpu = Wgpu { instance, adapter, device, queue };
    i_slint_core::platform::set_platform(Box::new(OffscreenPlatform(wgpu.clone())))
        .expect("platform already initialized");
    Some(wgpu)
}

fn solid_texture(wgpu: &Wgpu, pixel: [u8; 4]) -> wgpu::Texture {
    let size = wgpu::Extent3d { width: 4, height: 4, depth_or_array_layers: 1 };
    let texture = wgpu.device.create_texture(&wgpu::TextureDescriptor {
        label: Some("solid texture"),
        size,
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: wgpu::TextureFormat::Rgba8Unorm,
        usage: wgpu::TextureUsages::TEXTURE_BINDING
            | wgpu::TextureUsages::RENDER_ATTACHMENT
            | wgpu::TextureUsages::COPY_DST,
        view_formats: &[],
    });
    wgpu.queue.write_texture(
        texture.as_image_copy(),
        &pixel.repeat(16),
        wgpu::TexelCopyBufferLayout { offset: 0, bytes_per_row: Some(16), rows_per_image: None },
        size,
    );
    texture
}

fn center_pixel(window: &slint::Window) -> [u8; 4] {
    let snapshot = window.take_snapshot().unwrap();
    let pixel = snapshot.as_slice()[(SIZE / 2 * SIZE + SIZE / 2) as usize];
    [pixel.r, pixel.g, pixel.b, pixel.a]
}

// Both cases share one test, since a process has one platform.
#[test]
fn snapshot() {
    let Some(wgpu) = init_offscreen_platform() else {
        return;
    };

    let texture_case = TextureCase::new().unwrap();
    texture_case.show().unwrap();
    texture_case.set_source(Image::try_from(solid_texture(&wgpu, [0, 0, 255, 255])).unwrap());
    assert_eq!(center_pixel(texture_case.window()), [0, 0, 255, 255]);

    // `take_snapshot()` returns straight alpha, not the premultiplied one Skia renders.
    let translucent_case = TranslucentCase::new().unwrap();
    translucent_case.show().unwrap();
    assert_eq!(center_pixel(translucent_case.window()), [255, 0, 0, 0x80]);
}
