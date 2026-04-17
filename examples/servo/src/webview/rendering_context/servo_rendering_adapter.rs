// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT

use std::rc::Rc;

use euclid::Point2D;
use slint::{Image, SharedPixelBuffer};
use winit::dpi::PhysicalSize;

use servo::{DeviceIntRect, RenderingContext, SoftwareRenderingContext};

use {super::GPURenderingContext, slint::wgpu_28::wgpu};

pub fn create_software_context(size: PhysicalSize<u32>) -> Box<dyn ServoRenderingAdapter> {
    let rendering_context = Rc::new(
        SoftwareRenderingContext::new(size).expect("Failed to create software rendering context"),
    );

    Box::new(ServoSoftwareRenderingContext { rendering_context })
}

/// Attempts to create a GPU-accelerated rendering context.
/// Falls back to software rendering if GPU initialization fails or if forced via env var.
pub fn try_create_gpu_context(
    device: wgpu::Device,
    queue: wgpu::Queue,
    size: PhysicalSize<u32>,
) -> Option<Box<dyn ServoRenderingAdapter>> {
    if std::env::var_os("SLINT_SERVO_FORCE_SOFTWARE").is_some() {
        eprintln!("[GPU] Forced software rendering via env var");
        return Some(create_software_context(size));
    }

    match GPURenderingContext::new(size, &device) {
        Ok(gpu_context) => {
            eprintln!("[GPU] GPURenderingContext created successfully — using GPU path");
            let rendering_context = Rc::new(gpu_context);
            Some(Box::new(ServoGPURenderingContext { device, queue, rendering_context }))
        }
        Err(e) => {
            eprintln!("[GPU] GPURenderingContext::new failed: {:?} — falling back to software", e);
            Some(create_software_context(size))
        }
    }
}

pub trait ServoRenderingAdapter {
    fn current_framebuffer_as_image(&self) -> Image;
    fn get_rendering_context(&self) -> Rc<dyn RenderingContext>;
}

struct ServoGPURenderingContext {
    device: wgpu::Device,
    #[allow(dead_code)]
    queue: wgpu::Queue,
    rendering_context: Rc<GPURenderingContext>,
}

impl ServoRenderingAdapter for ServoGPURenderingContext {
    fn current_framebuffer_as_image(&self) -> Image {
        #[cfg(any(target_os = "linux", target_os = "android"))]
        let texture = self.rendering_context.get_wgpu_texture_from_vulkan(&self.device).expect(
            "Failed to get WGPU texture from Vulkan texture - ensure rendering context is valid",
        );

        #[cfg(target_vendor = "apple")]
        let texture =
            self.rendering_context.get_wgpu_texture_from_metal(&self.device, &self.queue).expect(
                "Failed to get WGPU texture from Metal texture - ensure rendering context is valid",
            );

        #[cfg(target_os = "windows")]
        let texture = self
            .rendering_context
            .get_wgpu_texture_from_directx(&self.device)
            .expect("Failed to get WGPU texture from DirectX");

        Image::try_from(texture).expect(
            "Failed to create Slint image from WGPU texture - check texture format compatibility",
        )
    }

    fn get_rendering_context(&self) -> Rc<dyn RenderingContext> {
        self.rendering_context.clone()
    }
}

struct ServoSoftwareRenderingContext {
    rendering_context: Rc<SoftwareRenderingContext>,
}

impl ServoRenderingAdapter for ServoSoftwareRenderingContext {
    fn current_framebuffer_as_image(&self) -> Image {
        let size = self.rendering_context.size2d().to_i32();
        let viewport_rect = DeviceIntRect::from_origin_and_size(Point2D::origin(), size);

        let image_buffer = self.rendering_context.read_to_image(viewport_rect).expect(
            "
        Failed to get image buffer from frame buffer",
        );

        let (width, height) = image_buffer.dimensions();
        let pixel_slice = image_buffer.into_raw();

        let shared_pixel_buffer = SharedPixelBuffer::clone_from_slice(&pixel_slice, width, height);

        Image::from_rgba8(shared_pixel_buffer)
    }

    fn get_rendering_context(&self) -> Rc<dyn RenderingContext> {
        self.rendering_context.clone()
    }
}
