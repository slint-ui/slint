// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

#![cfg(feature = "wgpu-30")]

use std::rc::Rc;

use i_slint_core::api::PhysicalSize;
use i_slint_core::platform::{Platform, PlatformError};
use i_slint_core::renderer::Renderer;
use i_slint_core::window::WindowAdapter;
use i_slint_renderer_femtovg::FemtoVGWGPURenderer;
use slint::{ComponentHandle, Image, Rgba8Pixel, SharedPixelBuffer};
use wgpu_30 as wgpu;

slint::slint! {
    #[edition = "1.19"]
    export component TestCase inherits Window {
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
}

fn poll_once<F: std::future::Future>(future: F) -> Option<F::Output> {
    let mut ctx = std::task::Context::from_waker(std::task::Waker::noop());
    match std::pin::pin!(future).poll(&mut ctx) {
        std::task::Poll::Ready(result) => Some(result),
        std::task::Poll::Pending => None,
    }
}

struct OffscreenPlatform {
    instance: wgpu::Instance,
    device: wgpu::Device,
    queue: wgpu::Queue,
}

impl Platform for OffscreenPlatform {
    fn create_window_adapter(&self) -> Result<Rc<dyn WindowAdapter>, PlatformError> {
        let renderer = FemtoVGWGPURenderer::new(
            self.instance.clone(),
            self.device.clone(),
            self.queue.clone(),
        )?;
        Ok(Rc::new_cyclic(|self_weak| OffscreenWindow {
            window: i_slint_core::api::Window::new(self_weak.clone() as _),
            renderer,
        }))
    }
}

struct OffscreenWindow {
    window: i_slint_core::api::Window,
    renderer: FemtoVGWGPURenderer,
}

impl WindowAdapter for OffscreenWindow {
    fn window(&self) -> &i_slint_core::api::Window {
        &self.window
    }

    fn size(&self) -> PhysicalSize {
        PhysicalSize::new(40, 40)
    }

    fn renderer(&self) -> &dyn Renderer {
        &self.renderer
    }
}

/// Sets up the offscreen platform, or returns `false` when no WGPU adapter is available.
fn init_offscreen_platform() -> bool {
    let mut descriptor = wgpu::InstanceDescriptor::new_without_display_handle_from_env();
    descriptor.backends = i_slint_core::graphics::wgpu_30::mask_backends(
        descriptor.backends,
        i_slint_core::graphics::wgpu_30::default_backends_to_avoid(),
    );
    let instance = wgpu::Instance::new(descriptor);
    let Some(Ok(adapter)) =
        poll_once(instance.request_adapter(&wgpu::RequestAdapterOptions::default()))
    else {
        return false;
    };
    let Some(Ok((device, queue))) =
        poll_once(adapter.request_device(&wgpu::DeviceDescriptor::default()))
    else {
        return false;
    };
    i_slint_core::platform::set_platform(Box::new(OffscreenPlatform { instance, device, queue }))
        .expect("platform already initialized");
    true
}

fn solid_image(r: u8, g: u8, b: u8) -> Image {
    let mut buffer = SharedPixelBuffer::<Rgba8Pixel>::new(4, 4);
    buffer.make_mut_slice().fill(Rgba8Pixel { r, g, b, a: 255 });
    Image::from_rgba8(buffer)
}

#[test]
fn image_tracks_source() {
    if !init_offscreen_platform() {
        return;
    }
    let component = TestCase::new().unwrap();
    component.show().unwrap();
    let sample = || {
        let image = component.window().take_snapshot().unwrap();
        let pixel = image.as_slice()[20 * image.width() as usize + 20];
        (pixel.r, pixel.g, pixel.b)
    };
    component.set_source(solid_image(255, 0, 0));
    assert_eq!(sample(), (255, 0, 0));
    component.set_source(solid_image(0, 0, 255));
    assert_eq!(sample(), (0, 0, 255));
}
