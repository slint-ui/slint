// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

//! `take_snapshot()` of a window that Skia renders with WGPU,
//! and `render_to_texture()` into a WGPU texture.
//!
//! The tests pass without rendering on a machine without a WGPU adapter that Skia can use.

#![cfg(feature = "unstable-wgpu-30")]

use std::cell::RefCell;
use std::io::Write;
use std::rc::Rc;

use i_slint_core::api::PhysicalSize;
use i_slint_core::platform::{Platform, PlatformError};
use i_slint_core::renderer::Renderer;
use i_slint_core::window::WindowAdapter;
use i_slint_renderer_skia::SkiaWGPU30Renderer;
use slint::{ComponentHandle, Image};
use wgpu_30 as wgpu;

slint::slint! {
    #[edition = "1.20"]
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

    export component RenderToTextureCase inherits Window {
        width: 40px;
        height: 40px;
        background: blue;
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

struct OffscreenPlatform {
    wgpu: Wgpu,
    last_window: Rc<RefCell<Option<Rc<OffscreenWindow>>>>,
}

impl Platform for OffscreenPlatform {
    fn create_window_adapter(&self) -> Result<Rc<dyn WindowAdapter>, PlatformError> {
        let Wgpu { instance, adapter, device, queue } = self.wgpu.clone();
        let renderer = SkiaWGPU30Renderer::new(instance, adapter, device, queue)?;
        let window = Rc::new_cyclic(|self_weak| OffscreenWindow {
            window: i_slint_core::api::Window::new(self_weak.clone() as _),
            renderer,
        });
        *self.last_window.borrow_mut() = Some(window.clone());
        Ok(window)
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
/// Also returns where the platform keeps the window it created last.
fn init_offscreen_platform() -> Option<(Wgpu, Rc<RefCell<Option<Rc<OffscreenWindow>>>>)> {
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
    let last_window = Rc::new(RefCell::new(None));
    i_slint_core::platform::set_platform(Box::new(OffscreenPlatform {
        wgpu: wgpu.clone(),
        last_window: last_window.clone(),
    }))
    .expect("platform already initialized");
    Some((wgpu, last_window))
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

/// A texture that WGPU never wrote, for Skia to render into.
fn color_target(wgpu: &Wgpu) -> wgpu::Texture {
    wgpu.device.create_texture(&wgpu::TextureDescriptor {
        label: Some("color target"),
        size: wgpu::Extent3d { width: SIZE, height: SIZE, depth_or_array_layers: 1 },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: wgpu::TextureFormat::Rgba8Unorm,
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT
            | wgpu::TextureUsages::COPY_SRC
            | wgpu::TextureUsages::COPY_DST,
        view_formats: &[],
    })
}

/// Reads the center texel back through WGPU, as an application using the texture would.
fn center_texel(wgpu: &Wgpu, texture: &wgpu::Texture) -> [u8; 4] {
    let buffer = wgpu.device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("center texel"),
        size: 4,
        usage: wgpu::BufferUsages::MAP_READ | wgpu::BufferUsages::COPY_DST,
        mapped_at_creation: false,
    });
    let mut encoder =
        wgpu.device.create_command_encoder(&wgpu::CommandEncoderDescriptor::default());
    encoder.copy_texture_to_buffer(
        wgpu::TexelCopyTextureInfo {
            texture,
            mip_level: 0,
            origin: wgpu::Origin3d { x: SIZE / 2, y: SIZE / 2, z: 0 },
            aspect: wgpu::TextureAspect::All,
        },
        wgpu::TexelCopyBufferInfo {
            buffer: &buffer,
            layout: wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: None,
                rows_per_image: None,
            },
        },
        wgpu::Extent3d { width: 1, height: 1, depth_or_array_layers: 1 },
    );
    wgpu.queue.submit(Some(encoder.finish()));
    buffer.slice(..).map_async(wgpu::MapMode::Read, |_| {});
    wgpu.device.poll(wgpu::PollType::wait_indefinitely()).unwrap();
    let data = buffer.slice(..).get_mapped_range().unwrap();
    [data[0], data[1], data[2], data[3]]
}

fn center_pixel(window: &slint::Window) -> [u8; 4] {
    let snapshot = window.take_snapshot().unwrap();
    let pixel = snapshot.as_slice()[(SIZE / 2 * SIZE + SIZE / 2) as usize];
    [pixel.r, pixel.g, pixel.b, pixel.a]
}

// Both cases share one test, since a process has one platform.
#[test]
fn snapshot() {
    let Some((wgpu, last_window)) = init_offscreen_platform() else {
        // libtest captures `eprintln!` of a passing test, but not writes to the stream itself.
        writeln!(std::io::stderr(), "skipping: no WGPU adapter that Skia can use").unwrap();
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

    // The texture's first use must already show what Skia rendered.
    let render_case = RenderToTextureCase::new().unwrap();
    render_case.show().unwrap();
    let window = last_window.borrow().clone().unwrap();
    let texture = color_target(&wgpu);
    window.renderer.render_to_texture(&texture).unwrap();
    assert_eq!(center_texel(&wgpu, &texture), [0, 0, 255, 255]);
}
