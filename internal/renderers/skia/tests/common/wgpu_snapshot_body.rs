// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

// The body of the `wgpu_29_snapshot` and `wgpu_30_snapshot` tests.
// Each includes it after defining `wgpu`, `slint_wgpu`, `SkiaWGPURenderer` and `mapped_bytes`
// for its wgpu version.

use std::io::Write;
use std::rc::Rc;

use i_slint_core::api::PhysicalSize;
use i_slint_core::platform::{Platform, PlatformError};
use i_slint_core::renderer::Renderer;
use i_slint_core::window::{WindowAdapter, WindowAdapterInternal, WindowInner};
use slint::{ComponentHandle, Image};

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

struct OffscreenPlatform(Wgpu);

impl Platform for OffscreenPlatform {
    fn create_window_adapter(&self) -> Result<Rc<dyn WindowAdapter>, PlatformError> {
        let Wgpu { instance, adapter, device, queue } = self.0.clone();
        let renderer = SkiaWGPURenderer::new(instance, adapter, device, queue)?;
        Ok(Rc::new_cyclic(|self_weak| OffscreenWindow {
            window: i_slint_core::api::Window::new(self_weak.clone() as _),
            renderer,
        }))
    }
}

struct OffscreenWindow {
    window: i_slint_core::api::Window,
    renderer: SkiaWGPURenderer,
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

    fn internal(&self, _: i_slint_core::InternalToken) -> Option<&dyn WindowAdapterInternal> {
        Some(self)
    }
}

impl WindowAdapterInternal for OffscreenWindow {}

/// Sets up the offscreen platform, or returns `None` when no WGPU adapter for Skia is available.
fn init_offscreen_platform() -> Option<Wgpu> {
    let mut descriptor = wgpu::InstanceDescriptor::new_without_display_handle_from_env();
    descriptor.backends = slint_wgpu::mask_backends(
        descriptor.backends
            & (wgpu::Backends::VULKAN | wgpu::Backends::METAL | wgpu::Backends::DX12),
        slint_wgpu::default_backends_to_avoid(),
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

/// A texture that WGPU never wrote, for Skia to render into.
fn color_target(wgpu: &Wgpu, mip_level_count: u32) -> wgpu::Texture {
    wgpu.device.create_texture(&wgpu::TextureDescriptor {
        label: Some("color target"),
        size: wgpu::Extent3d { width: SIZE, height: SIZE, depth_or_array_layers: 1 },
        mip_level_count,
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
    let data = mapped_bytes(&buffer);
    [data[0], data[1], data[2], data[3]]
}

fn center_pixel(window: &slint::Window) -> [u8; 4] {
    let snapshot = window.take_snapshot().unwrap();
    let pixel = snapshot.as_slice()[(SIZE / 2 * SIZE + SIZE / 2) as usize];
    [pixel.r, pixel.g, pixel.b, pixel.a]
}

// All cases share one test, since a process has one platform.
#[test]
fn rendering() {
    let Some(wgpu) = init_offscreen_platform() else {
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
    let adapter = WindowInner::from_pub(render_case.window()).window_adapter();
    let internal: &dyn std::any::Any = adapter.internal(i_slint_core::InternalToken).unwrap();
    let renderer = &internal.downcast_ref::<OffscreenWindow>().unwrap().renderer;
    let texture = color_target(&wgpu, 1);
    renderer.render_to_texture(&texture).unwrap();
    assert_eq!(center_texel(&wgpu, &texture), [0, 0, 255, 255]);

    // Skia draws into the first mip level of a mipmapped texture.
    let texture = color_target(&wgpu, 2);
    renderer.render_to_texture(&texture).unwrap();
    assert_eq!(center_texel(&wgpu, &texture), [0, 0, 255, 255]);

    // A texture Skia can't draw into is rejected before anything touches it.
    let texture = wgpu.device.create_texture(&wgpu::TextureDescriptor {
        label: Some("3D texture"),
        size: wgpu::Extent3d { width: SIZE, height: SIZE, depth_or_array_layers: 2 },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D3,
        format: wgpu::TextureFormat::Rgba8Unorm,
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT
            | wgpu::TextureUsages::COPY_SRC
            | wgpu::TextureUsages::COPY_DST,
        view_formats: &[],
    });
    wgpu.queue.write_texture(
        wgpu::TexelCopyTextureInfo {
            texture: &texture,
            mip_level: 0,
            origin: wgpu::Origin3d { x: SIZE / 2, y: SIZE / 2, z: 0 },
            aspect: wgpu::TextureAspect::All,
        },
        &[0, 255, 0, 255],
        wgpu::TexelCopyBufferLayout { offset: 0, bytes_per_row: None, rows_per_image: None },
        wgpu::Extent3d { width: 1, height: 1, depth_or_array_layers: 1 },
    );
    assert!(renderer.render_to_texture(&texture).is_err());
    assert_eq!(center_texel(&wgpu, &texture), [0, 255, 0, 255]);
}
