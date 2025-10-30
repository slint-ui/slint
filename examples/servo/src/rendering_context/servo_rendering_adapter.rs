// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT

use std::rc::Rc;

use euclid::Point2D;
use winit::dpi::PhysicalSize;

use slint::{Image, SharedPixelBuffer};

use servo::{RenderingContext, SoftwareRenderingContext, webrender_api::units::DeviceIntRect};

#[cfg(not(target_os = "android"))]
use crate::rendering_context::GPURenderingContext;

pub fn create_software_context(size: PhysicalSize<u32>) -> Box<dyn ServoRenderingAdapter> {
    let rendering_context = Rc::new(
        SoftwareRenderingContext::new(size).expect("Failed to create software rendering context"),
    );

    Box::new(ServoSoftwareRenderingContext { rendering_context })
}

#[cfg(not(target_os = "android"))]
pub fn try_create_gpu_context(
    device: wgpu::Device,
    queue: wgpu::Queue,
    size: PhysicalSize<u32>,
) -> Option<Box<dyn ServoRenderingAdapter>> {
    if std::env::var_os("SLINT_SERVO_FORCE_SOFTWARE").is_some() {
        return Some(create_software_context(size));
    }

    // Try to create GPU rendering context, fall back to software if it fails
    match GPURenderingContext::new(size) {
        Ok(gpu_context) => {
            let rendering_context = Rc::new(gpu_context);
            Some(Box::new(ServoGPURenderingContext {
                device: device.clone(),
                queue: queue.clone(),
                rendering_context,
            }))
        }
        Err(_) => {
            // GPU rendering context creation failed, fall back to software rendering
            Some(create_software_context(size))
        }
    }
}

pub trait ServoRenderingAdapter {
    fn current_framebuffer_as_image(&self) -> Image;
    fn get_rendering_context(&self) -> Rc<dyn RenderingContext>;
}

#[cfg(not(target_os = "android"))]
struct ServoGPURenderingContext {
    device: wgpu::Device,
    queue: wgpu::Queue,
    rendering_context: Rc<GPURenderingContext>,
}

#[cfg(not(target_os = "android"))]
impl ServoRenderingAdapter for ServoGPURenderingContext {
    fn current_framebuffer_as_image(&self) -> Image {
        #[cfg(target_os = "linux")]
        let texture = self.rendering_context
            .get_wgpu_texture_from_vulkan(&self.device, &self.queue)
            .expect(
                "Failed to get WGPU texture from Vulkan texture - ensure rendering context is valid",
            );

        #[cfg(target_vendor = "apple")]
        let texture = self
            .rendering_context
            .get_wgpu_texture_from_metal(&self.device, &self.queue)
            .expect(
                "Failed to get WGPU texture from Metal texture - ensure rendering context is valid",
            );

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
