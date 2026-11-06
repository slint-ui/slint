// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

//! Renders one scene with Skia Graphite on wgpu, and compares it with the snapshot that
//! Skia renders on the CPU.
//!
//! The test passes without rendering on a machine without a WGPU adapter that Skia can use.
//! Set `SLINT_GRAPHITE_TEST_OUTPUT` to a directory to write both images there as PNG.

#![cfg(all(feature = "graphite-wgpu-30", feature = "unstable-wgpu-30"))]

use std::io::Write;
use std::rc::Rc;

use i_slint_core::api::PhysicalSize;
use i_slint_core::platform::{Platform, PlatformError};
use i_slint_core::renderer::Renderer;
use i_slint_core::window::WindowAdapter;
use i_slint_renderer_skia::{SkiaGraphiteWGPU30Renderer, skia_safe};
use slint::ComponentHandle;
use wgpu_30 as wgpu;

slint::slint! {
    #[edition = "1.19"]
    export component Scene inherits Window {
        width: 256px;
        height: 256px;
        background: @linear-gradient(90deg, #eef 0%, #fee 100%);
        Rectangle {
            x: 10px; y: 10px; width: 80px; height: 50px;
            background: #3a7;
            border-radius: 12px;
            border-width: 3px;
            border-color: #135;
            drop-shadow-blur: 6px;
            drop-shadow-color: #0008;
            drop-shadow-offset-x: 3px;
            drop-shadow-offset-y: 3px;
        }
        Rectangle {
            x: 110px; y: 10px; width: 80px; height: 80px;
            background: @radial-gradient(circle, #f80 0%, #08f 100%);
            border-radius: 40px;
        }
        Text {
            x: 10px; y: 70px;
            text: "Graphite";
            font-size: 22px;
            color: #222;
        }
        Image {
            x: 10px; y: 105px; width: 90px; height: 45px;
            source: @image-url("../../../../logo/MadeWithSlint-logo-light.png");
        }
        Rectangle {
            x: 110px; y: 105px; width: 80px; height: 80px;
            clip: true;
            border-radius: 20px;
            opacity: 0.6;
            Rectangle {
                width: 120px; height: 20px; x: -10px; y: 30px;
                background: #c22;
                transform-rotation: 30deg;
            }
        }
        Path {
            x: 10px; y: 155px; width: 90px; height: 40px;
            commands: "M 0 40 C 30 0 60 0 90 40";
            stroke: #a0a;
            stroke-width: 4px;
        }
    }

    export component TextureCase inherits Window {
        width: 256px;
        height: 256px;
        background: white;
        in property <image> source;
        Image {
            width: 50%;
            height: 50%;
            source: root.source;
        }
    }
}

const SIZE: u32 = 256;

thread_local! {
    static LAST_WINDOW: std::cell::RefCell<Option<Rc<OffscreenWindow>>> = const { std::cell::RefCell::new(None) };
}

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
    device: wgpu::Device,
    queue: wgpu::Queue,
}

struct OffscreenPlatform(Wgpu);

impl Platform for OffscreenPlatform {
    fn create_window_adapter(&self) -> Result<Rc<dyn WindowAdapter>, PlatformError> {
        let Wgpu { instance, device, queue } = self.0.clone();
        let renderer = SkiaGraphiteWGPU30Renderer::new(instance, device, queue)?;
        let window = Rc::new_cyclic(|self_weak: &std::rc::Weak<OffscreenWindow>| OffscreenWindow {
            window: i_slint_core::api::Window::new(self_weak.clone() as _),
            renderer,
        });
        LAST_WINDOW.set(Some(window.clone()));
        Ok(window)
    }
}

struct OffscreenWindow {
    window: i_slint_core::api::Window,
    renderer: SkiaGraphiteWGPU30Renderer,
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
    writeln!(std::io::stderr(), "adapter: {:?}", adapter.get_info()).unwrap();
    let Some(Ok((device, queue))) =
        poll_once(adapter.request_device(&wgpu::DeviceDescriptor::default()))
    else {
        return None;
    };
    let wgpu = Wgpu { instance, device, queue };
    i_slint_core::platform::set_platform(Box::new(OffscreenPlatform(wgpu.clone())))
        .expect("platform already initialized");
    Some(wgpu)
}

/// Returns the pixels rendered on the GPU, and those of the CPU snapshot, both premultiplied.
fn render(wgpu: &Wgpu) -> (Vec<u8>, Vec<u8>) {
    let scene = Scene::new().unwrap();
    scene.show().unwrap();
    let gpu = render_last_window(wgpu);
    let snapshot = scene.window().take_snapshot().unwrap();
    let cpu = snapshot
        .as_slice()
        .iter()
        .flat_map(|p| {
            let premultiply = |c: u8| (c as u32 * p.a as u32 / 255) as u8;
            [premultiply(p.r), premultiply(p.g), premultiply(p.b), p.a]
        })
        .collect();
    (gpu, cpu)
}

/// Renders the window that was created last into a new texture, and returns its pixels.
fn render_last_window(wgpu: &Wgpu) -> Vec<u8> {
    let texture = wgpu.device.create_texture(&wgpu::TextureDescriptor {
        label: Some("scene"),
        size: wgpu::Extent3d { width: SIZE, height: SIZE, depth_or_array_layers: 1 },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: wgpu::TextureFormat::Rgba8Unorm,
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT
            | wgpu::TextureUsages::TEXTURE_BINDING
            | wgpu::TextureUsages::COPY_SRC
            | wgpu::TextureUsages::COPY_DST,
        view_formats: &[],
    });
    let window = LAST_WINDOW.take().unwrap();
    window.renderer.render_to_texture(&texture).unwrap();
    read_texture(wgpu, &texture)
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

fn pixel(pixels: &[u8], x: u32, y: u32) -> [u8; 4] {
    let i = ((y * SIZE + x) * 4) as usize;
    [pixels[i], pixels[i + 1], pixels[i + 2], pixels[i + 3]]
}

fn read_texture(wgpu: &Wgpu, texture: &wgpu::Texture) -> Vec<u8> {
    let bytes_per_row = SIZE * 4;
    let buffer = wgpu.device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("readback"),
        size: u64::from(bytes_per_row * SIZE),
        usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
        mapped_at_creation: false,
    });
    let mut encoder = wgpu.device.create_command_encoder(&Default::default());
    encoder.copy_texture_to_buffer(
        texture.as_image_copy(),
        wgpu::TexelCopyBufferInfo {
            buffer: &buffer,
            layout: wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(bytes_per_row),
                rows_per_image: None,
            },
        },
        texture.size(),
    );
    wgpu.queue.submit([encoder.finish()]);
    buffer.map_async(wgpu::MapMode::Read, .., |result| result.unwrap());
    wgpu.device.poll(wgpu::PollType::Wait { submission_index: None, timeout: None }).unwrap();
    buffer.get_mapped_range(..).unwrap().to_vec()
}

fn write_png(dir: &std::path::Path, name: &str, pixels: &[u8]) {
    let info = skia_safe::ImageInfo::new(
        (SIZE as i32, SIZE as i32),
        skia_safe::ColorType::RGBA8888,
        skia_safe::AlphaType::Premul,
        None,
    );
    let image = skia_safe::images::raster_from_data(
        &info,
        skia_safe::Data::new_copy(pixels),
        SIZE as usize * 4,
    )
    .unwrap();
    let data = image.encode(None, skia_safe::EncodedImageFormat::PNG, None).unwrap();
    std::fs::write(dir.join(format!("{name}.png")), data.as_bytes()).unwrap();
}

#[test]
fn graphite_matches_cpu() {
    let Some(wgpu) = init_offscreen_platform() else {
        writeln!(std::io::stderr(), "skipping: no WGPU adapter that Skia can use").unwrap();
        return;
    };

    let (graphite, cpu) = render(&wgpu);

    if let Some(dir) = std::env::var_os("SLINT_GRAPHITE_TEST_OUTPUT") {
        let dir = std::path::Path::new(&dir);
        write_png(dir, "cpu", &cpu);
        write_png(dir, "graphite", &graphite);
    }

    let differing = cpu
        .chunks(4)
        .zip(graphite.chunks(4))
        .filter(|(a, b)| a.iter().zip(b.iter()).any(|(a, b)| a.abs_diff(*b) > 24))
        .count();
    let percent = differing as f64 * 100. / (SIZE * SIZE) as f64;
    writeln!(std::io::stderr(), "{percent:.2}% of the pixels differ").unwrap();
    assert!(percent < 2., "{percent:.2}% of the pixels differ");

    // The image covers the top left quarter of the window.
    let texture_case = TextureCase::new().unwrap();
    texture_case
        .set_source(slint::Image::try_from(solid_texture(&wgpu, [0, 0, 255, 255])).unwrap());
    texture_case.show().unwrap();
    let pixels = render_last_window(&wgpu);
    assert_eq!(pixel(&pixels, SIZE / 4, SIZE / 4), [0, 0, 255, 255], "the texture is not drawn");
    assert_eq!(pixel(&pixels, SIZE * 3 / 4, SIZE * 3 / 4), [255, 255, 255, 255]);
}
