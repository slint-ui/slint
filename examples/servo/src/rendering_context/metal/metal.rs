//! Metal-specific WGPU integration for IOSurface textures.
//!
//! This module provides functionality to create WGPU textures from Metal IOSurfaces,
//! which is essential for efficient GPU memory sharing on macOS. It includes texture
//! flipping operations to handle coordinate system differences between Metal and other APIs.

use objc2::runtime::NSObject;
use objc2::{msg_send, rc::Retained};
use objc2_io_surface::IOSurfaceRef;
use objc2_metal::{MTLPixelFormat, MTLTextureDescriptor, MTLTextureType, MTLTextureUsage};

use foreign_types_shared::ForeignType;
use winit::dpi::PhysicalSize;

use crate::rendering_context::metal::ServoTextureImporter;

/// WGPU texture wrapper for Metal IOSurface textures.
///
/// This struct provides functionality to create WGPU textures from Metal IOSurfaces
/// and perform coordinate system transformations.
pub struct WPGPUTextureFromMetal {
    pub size: PhysicalSize<u32>,
    pub texture_importer: ServoTextureImporter,
}

impl WPGPUTextureFromMetal {
    pub fn new(size: PhysicalSize<u32>, wgpu_device: &wgpu::Device) -> Self {
        Self {
            size,
            texture_importer: ServoTextureImporter::new(wgpu_device),
        }
    }

    pub fn get(
        &self,
        wgpu_device: &wgpu::Device,
        wgpu_queue: &wgpu::Queue,
        surfman_device: &surfman::Device,
        surfman_surface: &surfman::Surface,
    ) -> wgpu::Texture {
        let objc2_metal_texture =
            self.objc2_metal_texture(wgpu_device, surfman_device, surfman_surface);

        let hal_texture = self.wgpu_hal_texture(wgpu_device, objc2_metal_texture);

        self.create_flipped_texture_render(wgpu_device, wgpu_queue, &hal_texture)
    }

    /// Creates a Metal texture from an IOSurface using Objective-C messaging.
    ///
    /// This function uses unsafe Objective-C messaging. The caller must ensure:
    /// - The device pointer is valid and points to a Metal device
    /// - The descriptor contains valid configuration
    /// - The IOSurface is valid and compatible with the descriptor
    fn create_texture_from_iosurface(
        &self,
        device: &objc2::runtime::NSObject,
        descriptor: &MTLTextureDescriptor,
        iosurface: &IOSurfaceRef,
        plane: objc2_foundation::NSUInteger,
    ) -> Option<Retained<NSObject>> {
        unsafe {
            msg_send![device, newTextureWithDescriptor:descriptor, iosurface:iosurface, plane:plane]
        }
    }

    /// Creates a Metal texture object from an IOSurface using the WGPU Metal backend.
    ///
    /// This method extracts the Metal device from the WGPU device and uses it to create
    /// a Metal texture directly from the IOSurface contained in the surfman surface.
    ///
    /// This function contains unsafe code for:
    /// - Extracting the raw Metal device from WGPU
    /// - Converting device pointers for Objective-C messaging
    fn objc2_metal_texture(
        &self,
        wgpu_device: &wgpu::Device,

        surfman_device: &surfman::Device,
        surfman_surface: &surfman::Surface,
    ) -> Retained<NSObject> {
        // SAFETY: We're working with WGPU Metal backend, so the device extraction
        // and pointer manipulations are safe within this controlled context.
        unsafe {
            let metal_device = wgpu_device
                .as_hal::<wgpu::wgc::api::Metal>()
                .expect("WGPU device is not using Metal backend");

            let device_raw = metal_device.raw_device().lock().clone();

            let descriptor = MTLTextureDescriptor::new();
            descriptor.setDepth(1);
            descriptor.setSampleCount(1);
            descriptor.setWidth(self.size.width as usize);
            descriptor.setHeight(self.size.height as usize);
            descriptor.setMipmapLevelCount(1);
            descriptor.setUsage(MTLTextureUsage::ShaderRead);
            descriptor.setPixelFormat(MTLPixelFormat::BGRA8Unorm);
            descriptor.setTextureType(MTLTextureType::Type2D);

            // let texture_descriptor = Self::create_metal_texture_descriptor(self.size);

            let native_surface = surfman_device.native_surface(surfman_surface);
            let io_surface = native_surface.0;

            // SAFETY: The device_raw pointer is valid (obtained from WGPU Metal backend)
            // and we're casting it appropriately for Objective-C messaging.
            let texture = self
                .create_texture_from_iosurface(
                    &*(device_raw.as_ptr() as *mut objc2::runtime::NSObject),
                    &descriptor,
                    &io_surface,
                    0,
                )
                .expect("Failed to create Metal texture from IOSurface");

            texture
        }
    }

    /// Creates a WGPU texture descriptor with standard settings for this use case.
    fn create_wgpu_texture_descriptor(
        size: PhysicalSize<u32>,
        label: &str,
        usage: wgpu::TextureUsages,
        format: wgpu::TextureFormat,
    ) -> wgpu::TextureDescriptor<'_> {
        wgpu::TextureDescriptor {
            label: Some(label),
            size: wgpu::Extent3d {
                width: size.width,
                height: size.height,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format,
            usage,
            view_formats: &[],
        }
    }

    /// Converts a Metal texture object into a WGPU texture.
    ///
    /// This method takes a Metal texture (as an NSObject) and wraps it in WGPU's
    /// texture abstraction, allowing it to be used with WGPU rendering operations.
    ///
    /// This function contains unsafe code for:
    /// - Converting Objective-C objects to Metal API objects
    /// - Creating HAL textures from raw Metal textures
    /// - Managing memory ownership transfer between different APIs
    fn wgpu_hal_texture(
        &self,
        wgpu_device: &wgpu::Device,
        metal_texture: Retained<NSObject>,
    ) -> wgpu::Texture {
        // SAFETY: We're converting between compatible object types within the same
        // Metal/WGPU ecosystem. The ownership transfer is handled correctly.
        unsafe {
            let ptr: *mut objc2_foundation::NSObject = Retained::into_raw(metal_texture);

            // SAFETY: The ptr comes from a valid Metal texture object
            let metal_texture = metal::Texture::from_ptr(ptr as *mut _);

            let hal_texture = wgpu::hal::metal::Device::texture_from_raw(
                metal_texture,
                wgpu::TextureFormat::Bgra8Unorm,
                metal::MTLTextureType::D2,
                0,
                0,
                wgpu::hal::CopyExtent {
                    width: self.size.width,
                    height: self.size.height,
                    depth: 0,
                },
            );

            let wgpu_descriptor = Self::create_wgpu_texture_descriptor(
                self.size,
                "Metal IOSurface Texture",
                wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::RENDER_ATTACHMENT,
                wgpu::TextureFormat::Bgra8Unorm,
            );

            wgpu_device
                .create_texture_from_hal::<wgpu::wgc::api::Metal>(hal_texture, &wgpu_descriptor)
        }
    }

    /// Creates and applies a texture flipping render operation using pre-initialized resources.
    fn create_flipped_texture_render(
        &self,
        wgpu_device: &wgpu::Device,
        wgpu_queue: &wgpu::Queue,
        source_texture: &wgpu::Texture,
    ) -> wgpu::Texture {
        // Create the output texture
        let descriptor = WPGPUTextureFromMetal::create_wgpu_texture_descriptor(
            self.size,
            "Flipped Metal IOSurface Texture",
            wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::RENDER_ATTACHMENT,
            wgpu::TextureFormat::Rgba8Unorm,
        );

        let flipped_texture = wgpu_device.create_texture(&descriptor);

        let source_view = source_texture.create_view(&wgpu::TextureViewDescriptor::default());

        let bind_group = wgpu_device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Metal Texture Flip Bind Group"),
            layout: &self.texture_importer.bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(&source_view),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Sampler(&self.texture_importer.sampler),
                },
            ],
        });

        // Execute the render pass
        let target_view = &flipped_texture.create_view(&wgpu::TextureViewDescriptor::default());

        let mut encoder = wgpu_device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("Metal Texture Flip Command Encoder"),
        });

        {
            let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("Metal Texture Flip Render Pass"),
                timestamp_writes: None,
                occlusion_query_set: None,
                depth_stencil_attachment: None,
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: target_view,
                    depth_slice: None,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        store: wgpu::StoreOp::Store,
                        load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                    },
                })],
            });

            render_pass.set_pipeline(&self.texture_importer.render_pipeline);
            render_pass.set_bind_group(0, &bind_group, &[]);
            render_pass.draw(0..3, 0..1); // Draw a fullscreen triangle
        }

        wgpu_queue.submit(std::iter::once(encoder.finish()));

        flipped_texture
    }
}
