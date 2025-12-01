// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT

use std::{cell::Cell, rc::Rc, sync::Arc};

use euclid::default::Size2D;
use image::RgbaImage;
use winit::dpi::PhysicalSize;

use servo::{RenderingContext, webrender_api::units::DeviceIntRect};

use surfman::{
    Connection, Device, Surface, SurfaceTexture, SurfaceType,
    chains::{PreserveBuffer, SwapChain},
};

use wgpu;

#[cfg(any(target_os = "linux", target_os = "android"))]
#[derive(thiserror::Error, Debug)]
pub enum VulkanTextureError {
    #[error("{0:?}")]
    Surfman(surfman::Error),
    #[error("{0}")]
    Vulkan(#[from] ash::vk::Result),
    #[error("No surface returned when the surface was unbound from the context")]
    NoSurface,
    #[error("The surface didn't have a framebuffer object")]
    NoFramebuffer,
    #[error("Wgpu is not using the vulkan backend")]
    WgpuNotVulkan,
    #[error("{0}")]
    OpenGL(String),
}

use super::surfman_context::SurfmanRenderingContext;

pub struct GPURenderingContext {
    pub size: Cell<PhysicalSize<u32>>,
    pub swap_chain: SwapChain<Device>,
    pub surfman_rendering_info: SurfmanRenderingContext,
    pub is_widget: bool,
}

impl Drop for GPURenderingContext {
    fn drop(&mut self) {
        let device = &mut self.surfman_rendering_info.device.borrow_mut();
        let context = &mut self.surfman_rendering_info.context.borrow_mut();
        let _ = self.swap_chain.destroy(device, context);
    }
}

impl GPURenderingContext {
    pub fn new(
        size: PhysicalSize<u32>,
        native_widget: Option<surfman::NativeWidget>,
    ) -> Result<Self, surfman::Error> {
        let connection = Connection::new()?;

        let adapter = connection.create_adapter()?;

        let surfman_rendering_info = SurfmanRenderingContext::new(&connection, &adapter, None)?;

        let surfman_size = Size2D::new(size.width as i32, size.height as i32);

        let is_widget = native_widget.is_some();
        let surface_type = if let Some(native_widget) = native_widget {
            SurfaceType::Widget { native_widget }
        } else {
            SurfaceType::Generic { size: surfman_size }
        };

        let surface = surfman_rendering_info.create_surface(surface_type)?;

        surfman_rendering_info.bind_surface(surface)?;

        surfman_rendering_info.make_current()?;

        let swap_chain = surfman_rendering_info.create_attached_swap_chain()?;

        Ok(Self { swap_chain, size: Cell::new(size), surfman_rendering_info, is_widget })
    }

    /// Imports Metal surface as a WGPU texture for rendering on macOS/iOS.
    /// Unbinds the surface, converts to WGPU texture, then rebinds it.
    #[cfg(target_vendor = "apple")]
    pub fn get_wgpu_texture_from_metal(
        &self,
        wgpu_device: &wgpu::Device,
        wgpu_queue: &wgpu::Queue,
    ) -> Result<wgpu::Texture, surfman::Error> {
        use super::metal::WPGPUTextureFromMetal;

        let device = &self.surfman_rendering_info.device.borrow();
        let mut context = self.surfman_rendering_info.context.borrow_mut();

        let surface = device.unbind_surface_from_context(&mut context)?.unwrap();

        let size = self.size.get();

        let wgpu_texture = WPGPUTextureFromMetal::new(size, wgpu_device).get(
            wgpu_device,
            wgpu_queue,
            device,
            &surface,
        );

        let _ =
            device.bind_surface_to_context(&mut context, surface).map_err(|(err, mut surface)| {
                let _ = device.destroy_surface(&mut context, &mut surface);
                err
            });

        Ok(wgpu_texture)
    }

    /// Imports Vulkan surface as a WGPU texture for rendering on Linux.
    /// Creates a Vulkan image with external memory, imports to OpenGL, blits content, then wraps as WGPU texture.
    #[cfg(any(target_os = "linux", target_os = "android"))]
    pub fn get_wgpu_texture_from_vulkan(
        &self,
        wgpu_device: &wgpu::Device,
        wgpu_queue: &wgpu::Queue,
    ) -> Result<wgpu::Texture, VulkanTextureError> {
        if self.is_widget {
            // Return dummy texture
            let texture_desc = wgpu::TextureDescriptor {
                label: Some("Servo Dummy Texture"),
                size: wgpu::Extent3d { width: 1, height: 1, depth_or_array_layers: 1 },
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format: wgpu::TextureFormat::Rgba8Unorm,
                usage: wgpu::TextureUsages::TEXTURE_BINDING,
                view_formats: &[],
            };
            return Ok(wgpu_device.create_texture(&texture_desc));
        }

        use crate::gl_bindings as gl;
        use glow::HasContext;

        let device = &self.surfman_rendering_info.device.borrow();
        let mut context = self.surfman_rendering_info.context.borrow_mut();

        let surface = device
            .unbind_surface_from_context(&mut context)
            .map_err(VulkanTextureError::Surfman)?
            .ok_or(VulkanTextureError::NoSurface)?;

        device.make_context_current(&mut context).map_err(VulkanTextureError::Surfman)?;

        let surface_info = device.surface_info(&surface);
        let size = self.size.get();

        // Fallback to CPU copy for Android/Emulator where extensions might be missing
        let gl = &self.surfman_rendering_info.glow_gl;

        let read_framebuffer =
            surface_info.framebuffer_object.ok_or(VulkanTextureError::NoFramebuffer)?;

        let mut pixels = vec![0u8; (size.width * size.height * 4) as usize];

        unsafe {
            gl.bind_framebuffer(gl::READ_FRAMEBUFFER, Some(read_framebuffer));
            gl.read_pixels(
                0,
                0,
                size.width as i32,
                size.height as i32,
                gl::RGBA,
                gl::UNSIGNED_BYTE,
                glow::PixelPackData::Slice(Some(&mut pixels)),
            );
        }

        // Flip image vertically (OpenGL textures are upside down)
        let stride = (size.width * 4) as usize;
        let height = size.height as usize;
        let mut row_buffer = vec![0u8; stride];
        for y in 0..height / 2 {
            let top_row_start = y * stride;
            let bottom_row_start = (height - y - 1) * stride;

            // Swap rows
            row_buffer.copy_from_slice(&pixels[top_row_start..top_row_start + stride]);
            pixels.copy_within(bottom_row_start..bottom_row_start + stride, top_row_start);
            pixels[bottom_row_start..bottom_row_start + stride].copy_from_slice(&row_buffer);
        }

        // Create wgpu texture
        let texture_desc = wgpu::TextureDescriptor {
            label: Some("Servo Texture"),
            size: wgpu::Extent3d {
                width: size.width,
                height: size.height,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8Unorm,
            usage: wgpu::TextureUsages::TEXTURE_BINDING
                | wgpu::TextureUsages::COPY_DST
                | wgpu::TextureUsages::RENDER_ATTACHMENT
                | wgpu::TextureUsages::COPY_SRC,
            view_formats: &[],
        };

        let texture = wgpu_device.create_texture(&texture_desc);

        // Upload pixels
        wgpu_queue.write_texture(
            wgpu::TexelCopyTextureInfo {
                texture: &texture,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            &pixels,
            wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(size.width * 4),
                rows_per_image: Some(size.height),
            },
            texture_desc.size,
        );

        // Rebind surface to context (surfman requirement usually)
        let _ = device.bind_surface_to_context(&mut context, surface).map_err(
            |(err, mut surface)| {
                let _ = device.destroy_surface(&mut context, &mut surface);
                VulkanTextureError::Surfman(err)
            },
        )?;

        Ok(texture)
    }
}

impl RenderingContext for GPURenderingContext {
    fn prepare_for_rendering(&self) {
        self.surfman_rendering_info.prepare_for_rendering();
    }

    fn read_to_image(&self, source_rectangle: DeviceIntRect) -> Option<RgbaImage> {
        self.surfman_rendering_info.read_to_image(source_rectangle)
    }

    fn size(&self) -> PhysicalSize<u32> {
        self.size.get()
    }

    fn resize(&self, size: PhysicalSize<u32>) {
        if self.size.get() == size {
            return;
        }

        self.size.set(size);

        let mut device = self.surfman_rendering_info.device.borrow_mut();
        let mut context = self.surfman_rendering_info.context.borrow_mut();
        let size = Size2D::new(size.width as i32, size.height as i32);
        let _ = self.swap_chain.resize(&mut *device, &mut *context, size);
    }

    fn present(&self) {
        let mut device = self.surfman_rendering_info.device.borrow_mut();
        let mut context = self.surfman_rendering_info.context.borrow_mut();
        let _ = self.swap_chain.swap_buffers(&mut *device, &mut *context, PreserveBuffer::No);
    }

    fn make_current(&self) -> std::result::Result<(), surfman::Error> {
        self.surfman_rendering_info.make_current()
    }

    fn gleam_gl_api(&self) -> Rc<dyn gleam::gl::Gl> {
        self.surfman_rendering_info.gleam_gl.clone()
    }

    fn glow_gl_api(&self) -> Arc<glow::Context> {
        self.surfman_rendering_info.glow_gl.clone()
    }

    fn create_texture(&self, surface: Surface) -> Option<(SurfaceTexture, u32, Size2D<i32>)> {
        self.surfman_rendering_info.create_texture(surface)
    }

    fn destroy_texture(&self, surface_texture: SurfaceTexture) -> Option<Surface> {
        self.surfman_rendering_info.destroy_texture(surface_texture)
    }

    fn connection(&self) -> Option<Connection> {
        self.surfman_rendering_info.connection()
    }
}
