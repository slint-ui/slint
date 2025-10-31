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

#[cfg(not(target_os = "android"))]
use slint::wgpu_27::wgpu;

#[cfg(target_os = "linux")]
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

use crate::rendering_context::surfman_context::SurfmanRenderingContext;

pub struct GPURenderingContext {
    pub size: Cell<PhysicalSize<u32>>,
    pub swap_chain: SwapChain<Device>,
    pub surfman_rendering_info: SurfmanRenderingContext,
}

impl Drop for GPURenderingContext {
    fn drop(&mut self) {
        let device = &mut self.surfman_rendering_info.device.borrow_mut();
        let context = &mut self.surfman_rendering_info.context.borrow_mut();
        let _ = self.swap_chain.destroy(device, context);
    }
}

impl GPURenderingContext {
    pub fn new(size: PhysicalSize<u32>) -> Result<Self, surfman::Error> {
        let connection = Connection::new()?;

        let adapter = connection.create_adapter()?;

        let surfman_rendering_info = SurfmanRenderingContext::new(&connection, &adapter)?;

        let surfman_size = Size2D::new(size.width as i32, size.height as i32);

        let surface =
            surfman_rendering_info.create_surface(SurfaceType::Generic { size: surfman_size })?;

        surfman_rendering_info.bind_surface(surface)?;

        surfman_rendering_info.make_current()?;

        let swap_chain = surfman_rendering_info.create_attached_swap_chain()?;

        Ok(Self {
            swap_chain,
            size: Cell::new(size),
            surfman_rendering_info,
        })
    }

    #[cfg(target_vendor = "apple")]
    pub fn get_wgpu_texture_from_metal(
        &self,
        wgpu_device: &wgpu::Device,
        wgpu_queue: &wgpu::Queue,
    ) -> Result<wgpu::Texture, surfman::Error> {
        let device = &self.surfman_rendering_info.device.borrow();
        let mut context = self.surfman_rendering_info.context.borrow_mut();

        let surface = device.unbind_surface_from_context(&mut context)?.unwrap();

        let size = self.size.get();

        let wgpu_texture = crate::rendering_context::metal::WPGPUTextureFromMetal::new(
            size,
            wgpu_device,
        )
        .get(wgpu_device, wgpu_queue, device, &surface);

        let _ = device
            .bind_surface_to_context(&mut context, surface)
            .map_err(|(err, mut surface)| {
                let _ = device.destroy_surface(&mut context, &mut surface);
                err
            });

        Ok(wgpu_texture)
    }

    #[cfg(target_os = "linux")]
    pub fn get_wgpu_texture_from_vulkan(
        &self,
        wgpu_device: &wgpu::Device,
        _wgpu_queue: &wgpu::Queue,
    ) -> Result<wgpu::Texture, VulkanTextureError> {
        use crate::gl_bindings as gl;
        use ash::vk;
        use glow::HasContext;

        let device = &self.surfman_rendering_info.device.borrow();
        let mut context = self.surfman_rendering_info.context.borrow_mut();

        let surface = device
            .unbind_surface_from_context(&mut context)
            .map_err(VulkanTextureError::Surfman)?
            .ok_or(VulkanTextureError::NoSurface)?;

        device
            .make_context_current(&mut context)
            .map_err(VulkanTextureError::Surfman)?;

        let surface_info = device.surface_info(&surface);

        let size = self.size.get();

        let texture = unsafe {
            let hal_device = wgpu_device
                .as_hal::<wgpu::wgc::api::Vulkan>()
                .ok_or(VulkanTextureError::WgpuNotVulkan)?;
            let vulkan_device = hal_device.raw_device().clone();
            let vulkan_instance = hal_device.shared_instance().raw_instance();

            // Create image

            let mut external_memory_image_info = vk::ExternalMemoryImageCreateInfo::default()
                .handle_types(vk::ExternalMemoryHandleTypeFlags::OPAQUE_FD);

            let vulkan_image = vulkan_device.create_image(
                &vk::ImageCreateInfo::default()
                    .image_type(vk::ImageType::TYPE_2D)
                    .format(vk::Format::R8G8B8A8_UNORM)
                    .extent(vk::Extent3D {
                        width: size.width,
                        height: size.height,
                        depth: 1,
                    })
                    .mip_levels(1)
                    .array_layers(1)
                    .samples(vk::SampleCountFlags::TYPE_1)
                    .tiling(vk::ImageTiling::OPTIMAL)
                    .usage(vk::ImageUsageFlags::SAMPLED | vk::ImageUsageFlags::COLOR_ATTACHMENT)
                    .sharing_mode(vk::SharingMode::EXCLUSIVE)
                    .initial_layout(vk::ImageLayout::UNDEFINED)
                    .push_next(&mut external_memory_image_info),
                None,
            )?;

            // Allocate memory and bind to image

            let memory_requirements = vulkan_device.get_image_memory_requirements(vulkan_image);

            let mut dedicated_allocate_info =
                vk::MemoryDedicatedAllocateInfo::default().image(vulkan_image);

            let mut export_info = vk::ExportMemoryAllocateInfo::default()
                .handle_types(vk::ExternalMemoryHandleTypeFlags::OPAQUE_FD);

            let memory = vulkan_device.allocate_memory(
                &vk::MemoryAllocateInfo::default()
                    .allocation_size(memory_requirements.size)
                    // todo: required?
                    //.memory_type_index(mem_type_index as _)
                    .push_next(&mut dedicated_allocate_info)
                    .push_next(&mut export_info),
                None,
            )?;

            vulkan_device.bind_image_memory(vulkan_image, memory, 0)?;

            // Get memory handle

            let external_memory_fd_api =
                ash::khr::external_memory_fd::Device::new(&vulkan_instance, &vulkan_device);

            let memory_handle = external_memory_fd_api.get_memory_fd(
                &vk::MemoryGetFdInfoKHR::default()
                    .memory(memory)
                    .handle_type(vk::ExternalMemoryHandleTypeFlags::OPAQUE_FD),
            )?;

            // import into gl

            let gl = &self.surfman_rendering_info.glow_gl;

            let gl_with_extensions =
                gl::Gl::load_with(|function_name| device.get_proc_address(&context, function_name));

            let mut memory_object = 0;
            gl_with_extensions.CreateMemoryObjectsEXT(1, &mut memory_object);
            // We're using a dedicated allocation.
            // todo: taken from https://bxt.rs/blog/fast-half-life-video-recording-with-vulkan/, not sure if required.
            gl_with_extensions.MemoryObjectParameterivEXT(
                memory_object,
                gl::DEDICATED_MEMORY_OBJECT_EXT,
                &1,
            );
            gl_with_extensions.ImportMemoryFdEXT(
                memory_object,
                memory_requirements.size,
                gl::HANDLE_TYPE_OPAQUE_FD_EXT,
                memory_handle,
            );
            // Create a texture and bind it to the imported memory.
            let texture = gl.create_texture().map_err(VulkanTextureError::OpenGL)?;
            gl.bind_texture(gl::TEXTURE_2D, Some(texture));
            gl_with_extensions.TexStorageMem2DEXT(
                gl::TEXTURE_2D,
                1,
                gl::RGBA8,
                size.width as i32,
                size.height as i32,
                memory_object,
                0,
            );

            // Blit to it

            let draw_framebuffer = gl
                .create_framebuffer()
                .map_err(VulkanTextureError::OpenGL)?;
            let read_framebuffer = surface_info
                .framebuffer_object
                .ok_or(VulkanTextureError::NoFramebuffer)?;
            // todo: tried using gl.named_framebuffer_texture instead but it errored.
            gl.bind_framebuffer(gl::DRAW_FRAMEBUFFER, Some(draw_framebuffer));
            gl.framebuffer_texture_2d(
                gl::DRAW_FRAMEBUFFER,
                gl::COLOR_ATTACHMENT0,
                gl::TEXTURE_2D,
                Some(texture),
                0,
            );

            gl.blit_named_framebuffer(
                Some(read_framebuffer),
                Some(draw_framebuffer),
                0,
                0,
                size.width as i32,
                size.height as i32,
                // flipped upside down
                0,
                size.height as i32,
                size.width as i32,
                0,
                gl::COLOR_BUFFER_BIT,
                gl::NEAREST,
            );
            gl.flush();
            // Delete all the opengl objects. Seems to be required to prevent memory leaks
            // according to `amdgpu_top`.
            gl.delete_framebuffer(draw_framebuffer);
            gl.delete_texture(texture);
            gl_with_extensions.DeleteMemoryObjectsEXT(1, &memory_object);

            wgpu_device.create_texture_from_hal::<wgpu::wgc::api::Vulkan>(
                hal_device.texture_from_raw(
                    vulkan_image,
                    &wgpu_hal::TextureDescriptor {
                        label: None,
                        size: wgpu::Extent3d {
                            width: size.width,
                            height: size.height,
                            depth_or_array_layers: 1,
                        },
                        format: wgpu::TextureFormat::Rgba8Unorm,
                        dimension: wgpu::TextureDimension::D2,
                        mip_level_count: 1,
                        sample_count: 1,
                        usage: wgpu::TextureUses::RESOURCE | wgpu::TextureUses::COLOR_TARGET,
                        view_formats: vec![],
                        memory_flags: wgpu_hal::MemoryFlags::empty(),
                    },
                    Some(Box::new(move || {
                        // Images aren't cleaned up by wgpu-hal if theres a drop callback set so do it manually
                        vulkan_device.destroy_image(vulkan_image, None);
                        // Free the memory
                        vulkan_device.free_memory(memory, None);
                    })),
                ),
                &wgpu::TextureDescriptor {
                    label: None,
                    size: wgpu::Extent3d {
                        width: size.width,
                        height: size.height,
                        depth_or_array_layers: 1,
                    },
                    format: wgpu::TextureFormat::Rgba8Unorm,
                    dimension: wgpu::TextureDimension::D2,
                    mip_level_count: 1,
                    sample_count: 1,
                    usage: wgpu::TextureUsages::TEXTURE_BINDING
                        | wgpu::TextureUsages::RENDER_ATTACHMENT,
                    view_formats: &[],
                },
            )
        };

        let _ = device
            .bind_surface_to_context(&mut context, surface)
            .map_err(|(err, mut surface)| {
                let _ = device.destroy_surface(&mut context, &mut surface);
                err
            });

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
        let _ = self
            .swap_chain
            .swap_buffers(&mut *device, &mut *context, PreserveBuffer::No);
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
