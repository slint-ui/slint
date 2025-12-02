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

use crate::webview::rendering_context::surfman_context::SurfmanRenderingContext;
use winit::dpi::PhysicalSize;

pub struct WPGPUTextureFromVulkan<'a> {
    wgpu_device: &'a wgpu::Device,
    wgpu_queue: &'a wgpu::Queue,
    surfman_rendering_info: &'a SurfmanRenderingContext,
    size: PhysicalSize<u32>,
}

impl<'a> WPGPUTextureFromVulkan<'a> {
    pub fn new(
        wgpu_device: &'a wgpu::Device,
        wgpu_queue: &'a wgpu::Queue,
        surfman_rendering_info: &'a SurfmanRenderingContext,
        size: PhysicalSize<u32>,
    ) -> Self {
        Self { wgpu_device, wgpu_queue, surfman_rendering_info, size }
    }

    /// Imports Vulkan surface as a WGPU texture for rendering on Linux and Android.
    /// Creates a Vulkan image with external memory, imports to OpenGL, blits content, then wraps as WGPU texture.
    pub fn get(&self) -> Result<wgpu::Texture, VulkanTextureError> {
        // Check if we are running on an emulator.
        // The optimized path is known to be unstable on the Android Emulator.
        let is_emulator = {
            use glow::HasContext;
            let gl = self.surfman_rendering_info.glow_gl.clone();
            unsafe {
                let renderer = gl.get_parameter_string(glow::RENDERER);
                renderer.contains("Android Emulator")
                    || renderer.contains("Goldfish")
                    || renderer.contains("SwiftShader")
            }
        };

        if is_emulator {
            log::warn!(
                "Detected Android Emulator. Skipping optimized Vulkan texture sharing and using CPU fallback."
            );
            return self.get_wgpu_texture_from_vulkan_cpu_fallback();
        }

        // Try optimized path first
        match self.get_wgpu_texture_from_vulkan_optimized() {
            Ok(texture) => Ok(texture),
            Err(err) => {
                log::warn!(
                    "Optimized Vulkan texture sharing failed: {:?}. Falling back to CPU copy.",
                    err
                );
                self.get_wgpu_texture_from_vulkan_cpu_fallback()
            }
        }
    }

    fn get_wgpu_texture_from_vulkan_optimized(&self) -> Result<wgpu::Texture, VulkanTextureError> {
        use crate::gl_bindings as gl;
        use ash::vk;
        use glow::HasContext;

        let surfman_device = &self.surfman_rendering_info.device.borrow();

        let mut context = self.surfman_rendering_info.context.borrow_mut();

        let surface = surfman_device
            .unbind_surface_from_context(&mut context)
            .map_err(VulkanTextureError::Surfman)?
            .ok_or(VulkanTextureError::NoSurface)?;

        surfman_device.make_context_current(&mut context).map_err(VulkanTextureError::Surfman)?;

        let surface_info = surfman_device.surface_info(&surface);

        let size = self.size;

        let texture = unsafe {
            let hal_device = self
                .wgpu_device
                .as_hal::<wgpu::wgc::api::Vulkan>()
                .ok_or(VulkanTextureError::WgpuNotVulkan)?;
            let vulkan_device = hal_device.raw_device().clone();
            let vulkan_instance = hal_device.shared_instance().raw_instance();

            // Check if the required extension is supported to avoid panics in ash.
            // Ash's `Device::new` (or extension loaders) might panic if functions are missing.
            // We verify `vkGetMemoryFdKHR` availability dynamically using `get_device_proc_addr`.
            let get_memory_fd_khr_name = std::ffi::CString::new("vkGetMemoryFdKHR").unwrap();
            let get_memory_fd_khr = unsafe {
                vulkan_instance
                    .get_device_proc_addr(vulkan_device.handle(), get_memory_fd_khr_name.as_ptr())
            };

            if get_memory_fd_khr.is_none() {
                return Err(VulkanTextureError::Vulkan(vk::Result::ERROR_EXTENSION_NOT_PRESENT));
            }

            // Create Vulkan image with external memory for sharing with OpenGL

            let mut external_memory_image_info = vk::ExternalMemoryImageCreateInfo::default()
                .handle_types(vk::ExternalMemoryHandleTypeFlags::OPAQUE_FD);

            let vulkan_image = vulkan_device.create_image(
                &vk::ImageCreateInfo::default()
                    .image_type(vk::ImageType::TYPE_2D)
                    .format(vk::Format::R8G8B8A8_UNORM)
                    .extent(vk::Extent3D { width: size.width, height: size.height, depth: 1 })
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

            // Allocate dedicated Vulkan memory and bind to the created image

            let memory_requirements = vulkan_device.get_image_memory_requirements(vulkan_image);

            let mut dedicated_allocate_info =
                vk::MemoryDedicatedAllocateInfo::default().image(vulkan_image);

            let mut export_info = vk::ExportMemoryAllocateInfo::default()
                .handle_types(vk::ExternalMemoryHandleTypeFlags::OPAQUE_FD);

            let memory = vulkan_device.allocate_memory(
                &vk::MemoryAllocateInfo::default()
                    .allocation_size(memory_requirements.size)
                    .push_next(&mut dedicated_allocate_info)
                    .push_next(&mut export_info),
                None,
            )?;

            vulkan_device.bind_image_memory(vulkan_image, memory, 0)?;

            // Export Vulkan memory as a file descriptor for OpenGL import

            let external_memory_fd_api =
                ash::khr::external_memory_fd::Device::new(&vulkan_instance, &vulkan_device);

            let memory_handle = external_memory_fd_api.get_memory_fd(
                &vk::MemoryGetFdInfoKHR::default()
                    .memory(memory)
                    .handle_type(vk::ExternalMemoryHandleTypeFlags::OPAQUE_FD),
            )?;

            // Import Vulkan memory into OpenGL using EXT_external_objects

            let gl = &self.surfman_rendering_info.glow_gl;

            #[cfg(not(target_os = "android"))]
            use gl::Gl;
            #[cfg(target_os = "android")]
            use gl::Gles2 as Gl;

            let gl_with_extensions = Gl::load_with(|function_name| {
                surfman_device.get_proc_address(&context, function_name)
            });

            let mut memory_object = 0;
            gl_with_extensions.CreateMemoryObjectsEXT(1, &mut memory_object);
            // We're using a dedicated allocation.
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

            // Blit Servo's framebuffer to the imported texture

            let draw_framebuffer = gl.create_framebuffer().map_err(VulkanTextureError::OpenGL)?;
            let read_framebuffer =
                surface_info.framebuffer_object.ok_or(VulkanTextureError::NoFramebuffer)?;

            gl.bind_framebuffer(gl::DRAW_FRAMEBUFFER, Some(draw_framebuffer));
            gl.framebuffer_texture_2d(
                gl::DRAW_FRAMEBUFFER,
                gl::COLOR_ATTACHMENT0,
                gl::TEXTURE_2D,
                Some(texture),
                0,
            );

            gl.bind_framebuffer(gl::READ_FRAMEBUFFER, Some(read_framebuffer));
            gl.bind_framebuffer(gl::DRAW_FRAMEBUFFER, Some(draw_framebuffer));

            gl.blit_framebuffer(
                0,
                0,
                size.width as i32,
                size.height as i32,
                // Flip vertically - OpenGL origin is bottom-left, texture origin is top-left
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

            self.wgpu_device.create_texture_from_hal::<wgpu::wgc::api::Vulkan>(
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

        let _ = surfman_device.bind_surface_to_context(&mut context, surface).map_err(
            |(err, mut surface)| {
                let _ = surfman_device.destroy_surface(&mut context, &mut surface);
                err
            },
        );

        Ok(texture)
    }

    fn get_wgpu_texture_from_vulkan_cpu_fallback(
        &self,
    ) -> Result<wgpu::Texture, VulkanTextureError> {
        use crate::gl_bindings as gl;
        use glow::HasContext;

        let device = &self.surfman_rendering_info.device.borrow();
        let mut context = self.surfman_rendering_info.context.borrow_mut();

        let surface = device
            .unbind_surface_from_context(&mut context)
            .map_err(VulkanTextureError::Surfman)?
            .ok_or(VulkanTextureError::NoSurface)?;

        // Wrap in ManuallyDrop to prevent panic during unwinding if a panic occurs
        let mut surface = std::mem::ManuallyDrop::new(surface);

        // Use a closure to capture the result of operations that might fail,
        // so we can ensure the surface is always rebound or destroyed.
        let result = (|| -> Result<wgpu::Texture, VulkanTextureError> {
            device.make_context_current(&mut context).map_err(VulkanTextureError::Surfman)?;

            let surface_info = device.surface_info(&surface);
            let size = self.size;

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
                label: Some("Servo Texture Fallback"),
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

            let texture = self.wgpu_device.create_texture(&texture_desc);

            // Upload pixels
            self.wgpu_queue.write_texture(
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

            Ok(texture)
        })();

        // Take the surface out of ManuallyDrop to rebind or destroy it
        let mut surface = unsafe { std::mem::ManuallyDrop::take(&mut surface) };

        if result.is_err() {
            // If the operation failed, the context state might be bad.
            // Don't try to rebind. Just destroy and forget to avoid panic.
            let _ = device.destroy_surface(&mut context, &mut surface);
            std::mem::forget(surface);
            return result;
        }

        // Rebind surface to context (surfman requirement usually)
        match device.bind_surface_to_context(&mut context, surface) {
            Ok(_) => result,
            Err((err, mut surface)) => {
                // If rebind fails, destroy and forget.
                let _ = device.destroy_surface(&mut context, &mut surface);
                std::mem::forget(surface);
                // If the main operation failed, return that error.
                // If it succeeded but rebind failed, return the rebind error.
                result.and(Err(VulkanTextureError::Surfman(err)))
            }
        }
    }
}
