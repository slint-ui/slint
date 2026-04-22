// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT

//! Metal-specific WGPU integration for IOSurface textures.
//!
//! This module provides functionality to create WGPU textures from Metal IOSurfaces,
//! which is essential for efficient GPU memory sharing on macOS. It includes texture
//! flipping operations to handle coordinate system differences between Metal and other APIs.

use objc2::runtime::NSObject;
use objc2::{msg_send, rc::Retained};
use objc2_metal::{MTLPixelFormat, MTLTextureDescriptor, MTLTextureType, MTLTextureUsage};

use foreign_types_shared::ForeignType;
use winit::dpi::PhysicalSize;

use slint::wgpu_28::wgpu;

impl super::super::GPURenderingContext {
    /// Imports Metal surface as a WGPU texture for rendering on macOS/iOS.
    /// Unbinds the surface, converts to WGPU texture, then rebinds it.
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

        let objc2_metal_texture =
            objc2_metal_texture_from_iosurface(size, wgpu_device, device, &surface);

        let hal_texture = wgpu_hal_texture(size, wgpu_device, objc2_metal_texture);

        let wgpu_texture =
            create_flipped_texture_render(size, wgpu_device, wgpu_queue, &hal_texture);

        let _ =
            device.bind_surface_to_context(&mut context, surface).map_err(|(err, mut surface)| {
                let _ = device.destroy_surface(&mut context, &mut surface);
                err
            });

        Ok(wgpu_texture)
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
fn objc2_metal_texture_from_iosurface(
    size: PhysicalSize<u32>,
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

        let device_raw = metal_device.raw_device().clone();

        let descriptor = MTLTextureDescriptor::new();
        descriptor.setDepth(1);
        descriptor.setSampleCount(1);
        descriptor.setWidth(size.width as usize);
        descriptor.setHeight(size.height as usize);
        descriptor.setMipmapLevelCount(1);
        descriptor.setUsage(MTLTextureUsage::ShaderRead);
        descriptor.setPixelFormat(MTLPixelFormat::BGRA8Unorm);
        descriptor.setTextureType(MTLTextureType::Type2D);

        let native_surface = surfman_device.native_surface(surfman_surface);
        let io_surface = native_surface.0;

        let device_ns = &*(device_raw.as_ptr() as *mut NSObject);

        let texture: Option<Retained<NSObject>> = msg_send![
            device_ns,
            newTextureWithDescriptor: &*descriptor,
            iosurface: &*io_surface,
            plane: 0usize
        ];

        texture.expect("Failed to create Metal texture from IOSurface")
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
    size: PhysicalSize<u32>,
    wgpu_device: &wgpu::Device,
    metal_texture: Retained<NSObject>,
) -> wgpu::Texture {
    unsafe {
        let ptr: *mut objc2_foundation::NSObject = Retained::into_raw(metal_texture);

        let metal_texture = metal::Texture::from_ptr(ptr as *mut _);

        let hal_texture = wgpu::hal::metal::Device::texture_from_raw(
            metal_texture,
            wgpu::TextureFormat::Bgra8Unorm,
            metal::MTLTextureType::D2,
            0,
            0,
            wgpu::hal::CopyExtent { width: size.width, height: size.height, depth: 0 },
        );

        let descriptor = create_wgpu_texture_descriptor(
            size,
            "Metal IOSurface Texture",
            wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::RENDER_ATTACHMENT,
            wgpu::TextureFormat::Bgra8Unorm,
        );

        wgpu_device.create_texture_from_hal::<wgpu::wgc::api::Metal>(hal_texture, &descriptor)
    }
}

/// Creates and applies a texture flipping render operation using pre-initialized resources.
fn create_flipped_texture_render(
    size: PhysicalSize<u32>,
    wgpu_device: &wgpu::Device,
    wgpu_queue: &wgpu::Queue,
    source_texture: &wgpu::Texture,
) -> wgpu::Texture {
    

    let (bind_group_layout, pipeline, sampler) = create_flip_pipeline_resources(wgpu_device);

    // Create the output texture
    let descriptor = create_wgpu_texture_descriptor(
        size,
        "Flipped Metal IOSurface Texture",
        wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::RENDER_ATTACHMENT,
        wgpu::TextureFormat::Rgba8Unorm,
    );

    let flipped_texture = wgpu_device.create_texture(&descriptor);

    let source_view = source_texture.create_view(&wgpu::TextureViewDescriptor::default());

    let bind_group = wgpu_device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: Some("Metal Texture Flip Bind Group"),
        layout: &bind_group_layout,
        entries: &[
            wgpu::BindGroupEntry {
                binding: 0,
                resource: wgpu::BindingResource::TextureView(&source_view),
            },
            wgpu::BindGroupEntry { binding: 1, resource: wgpu::BindingResource::Sampler(&sampler) },
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
            multiview_mask: None,
        });

        render_pass.set_pipeline(&pipeline);
        render_pass.set_bind_group(0, &bind_group, &[]);
        render_pass.draw(0..3, 0..1); // Draw a fullscreen triangle
    }

    wgpu_queue.submit(std::iter::once(encoder.finish()));

    flipped_texture
}

fn create_flip_pipeline_resources(
    wgpu_device: &wgpu::Device,
) -> (wgpu::BindGroupLayout, wgpu::RenderPipeline, wgpu::Sampler) {
    // Vertex and fragment in one module. The vertex shader emits a fullscreen triangle;
    // the fragment shader samples the source texture with a vertical flip.
    const FLIP_SHADER: &str = r#"
        @vertex
        fn vs_main(@builtin(vertex_index) i: u32) -> @builtin(position) vec4<f32> {
            let uv = vec2<f32>(f32(i >> 1u), f32(i & 1u)) * 2.0;
            return vec4<f32>(uv * vec2<f32>(2.0, -2.0) + vec2<f32>(-1.0, 1.0), 0.0, 1.0);
        }

        @group(0) @binding(0) var src: texture_2d<f32>;
        @group(0) @binding(1) var smp: sampler;

        @fragment
        fn fs_main(@builtin(position) pos: vec4<f32>) -> @location(0) vec4<f32> {
            let sz = textureDimensions(src);
            let uv = vec2<f32>(pos.x / f32(sz.x), 1.0 - pos.y / f32(sz.y));
            return textureSample(src, smp, uv);
        }
    "#;

    let shader = wgpu_device.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: Some("Metal Flip Shader"),
        source: wgpu::ShaderSource::Wgsl(FLIP_SHADER.into()),
    });

    let bind_group_layout =
        wgpu_device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("Metal Texture Flip Bind Group Layout"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        multisampled: false,
                        view_dimension: wgpu::TextureViewDimension::D2,
                        sample_type: wgpu::TextureSampleType::Float { filterable: true },
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                    count: None,
                },
            ],
        });

    let layout = wgpu_device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
        label: Some("Metal Texture Flip Pipeline Layout"),
        bind_group_layouts: &[&bind_group_layout],
        immediate_size: 0,
    });

    let pipeline = wgpu_device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
        label: Some("Metal Texture Flip Render Pipeline"),
        layout: Some(&layout),
        vertex: wgpu::VertexState {
            module: &shader,
            entry_point: Some("vs_main"),
            buffers: &[],
            compilation_options: Default::default(),
        },
        fragment: Some(wgpu::FragmentState {
            module: &shader,
            entry_point: Some("fs_main"),
            targets: &[Some(wgpu::ColorTargetState {
                format: wgpu::TextureFormat::Rgba8Unorm,
                blend: None,
                write_mask: wgpu::ColorWrites::ALL,
            })],
            compilation_options: wgpu::PipelineCompilationOptions::default(),
        }),
        primitive: wgpu::PrimitiveState {
            topology: wgpu::PrimitiveTopology::TriangleList,
            strip_index_format: None,
            front_face: wgpu::FrontFace::Ccw,
            cull_mode: None,
            unclipped_depth: false,
            polygon_mode: wgpu::PolygonMode::Fill,
            conservative: false,
        },
        depth_stencil: None,
        multisample: wgpu::MultisampleState {
            count: 1,
            mask: !0,
            alpha_to_coverage_enabled: false,
        },
        multiview_mask: None,
        cache: None,
    });

    let sampler = wgpu_device.create_sampler(&wgpu::SamplerDescriptor {
        label: Some("Metal Texture Sampler"),
        compare: None,
        border_color: None,
        lod_min_clamp: 0.0,
        lod_max_clamp: 0.0,
        anisotropy_clamp: 1,
        mag_filter: wgpu::FilterMode::Linear,
        min_filter: wgpu::FilterMode::Linear,
        mipmap_filter: wgpu::MipmapFilterMode::Nearest,
        address_mode_u: wgpu::AddressMode::ClampToEdge,
        address_mode_v: wgpu::AddressMode::ClampToEdge,
        address_mode_w: wgpu::AddressMode::ClampToEdge,
    });

    (bind_group_layout, pipeline, sampler)
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
        size: wgpu::Extent3d { width: size.width, height: size.height, depth_or_array_layers: 1 },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format,
        usage,
        view_formats: &[],
    }
}
