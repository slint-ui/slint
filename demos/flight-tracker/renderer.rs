// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT

// cSpell: ignore equirectangular multisampled

use glam::{Mat4, Vec3, Vec4};
use slint::wgpu_30::wgpu;
use wgpu::util::DeviceExt;

use crate::compress;
use crate::map::{self, Map};

/// What one frame of the globe shows, see `Globals` in `globe.wgsl`.
pub struct Frame {
    pub view_projection: Mat4,
    pub eye: Vec3,
    pub sun: Vec3,
    /// 0 lights the whole globe, 1 darkens the side away from the sun.
    pub night: f32,
    pub borders: bool,
    pub time: f32,
}

/// Satellite imagery of the earth, equirectangular like the map, with its mip levels in sRGB
/// RGBA.
pub struct Imagery {
    pub width: u32,
    pub height: u32,
    pub format: wgpu::TextureFormat,
    pub levels: Vec<Vec<u8>>,
}

impl Imagery {
    /// Decodes a JPEG or PNG image, and computes its mip levels.
    pub fn decode(bytes: &[u8]) -> Result<Self, image::ImageError> {
        let base = image::load_from_memory(bytes)?.to_rgba8();
        let (width, height) = base.dimensions();
        let mut levels = vec![base];
        while levels.last().is_some_and(|level| level.width() > 1 || level.height() > 1) {
            let last = levels.last().unwrap();
            let size = ((last.width() / 2).max(1), (last.height() / 2).max(1));
            levels.push(image::imageops::resize(
                last,
                size.0,
                size.1,
                image::imageops::FilterType::Triangle,
            ));
        }
        Ok(Self {
            width,
            height,
            format: wgpu::TextureFormat::Rgba8UnormSrgb,
            levels: levels.into_iter().map(|level| level.into_raw()).collect(),
        })
    }

    /// Compresses the imagery to a format that `features` support: BC1 on most desktop GPUs,
    /// ETC2 on most mobile and embedded ones, and otherwise leaves it as it is.
    pub fn compress(self, features: wgpu::Features) -> Self {
        let encode: fn(&[u8], usize, usize) -> Vec<u8>;
        let format;
        if features.contains(wgpu::Features::TEXTURE_COMPRESSION_BC) {
            (encode, format) = (compress::bc1, wgpu::TextureFormat::Bc1RgbaUnormSrgb);
        } else if features.contains(wgpu::Features::TEXTURE_COMPRESSION_ETC2) {
            (encode, format) = (compress::etc2, wgpu::TextureFormat::Etc2Rgb8UnormSrgb);
        } else {
            return self;
        }
        let mut size = (self.width as usize, self.height as usize);
        let levels = self
            .levels
            .iter()
            .map(|level| {
                let compressed = encode(level, size.0, size.1);
                size = ((size.0 / 2).max(1), (size.1 / 2).max(1));
                compressed
            })
            .collect();
        Self { levels, format, ..self }
    }
}

pub struct Renderer {
    device: wgpu::Device,
    queue: wgpu::Queue,
    globals: wgpu::Buffer,
    bind_group: wgpu::BindGroup,
    pipeline: wgpu::RenderPipeline,
}

const GLOBAL_FLOATS: usize = 32;

impl Renderer {
    pub fn new(
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        map: &Map,
        imagery: &Imagery,
        format: wgpu::TextureFormat,
    ) -> Self {
        let texture = device.create_texture_with_data(
            queue,
            &wgpu::TextureDescriptor {
                label: Some("globe map"),
                size: wgpu::Extent3d {
                    width: map::WIDTH,
                    height: map::HEIGHT,
                    depth_or_array_layers: 1,
                },
                mip_level_count: map.levels.len() as u32,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format: wgpu::TextureFormat::Rgba8Unorm,
                usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
                view_formats: &[],
            },
            wgpu::util::TextureDataOrder::LayerMajor,
            &map.levels.concat(),
        );
        let day = imagery;
        let day_texture = device.create_texture_with_data(
            queue,
            &wgpu::TextureDescriptor {
                label: Some("globe imagery"),
                size: wgpu::Extent3d {
                    width: day.width,
                    height: day.height,
                    depth_or_array_layers: 1,
                },
                mip_level_count: day.levels.len() as u32,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format: day.format,
                usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
                view_formats: &[],
            },
            wgpu::util::TextureDataOrder::LayerMajor,
            &day.levels.concat(),
        );
        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            address_mode_u: wgpu::AddressMode::Repeat,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            mipmap_filter: wgpu::MipmapFilterMode::Linear,
            anisotropy_clamp: 8,
            ..Default::default()
        });
        let globals = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("globe globals"),
            size: std::mem::size_of::<[f32; GLOBAL_FLOATS]>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("globe"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: true },
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 2,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 3,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: true },
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
            ],
        });
        let view = texture.create_view(&Default::default());
        let day_view = day_texture.create_view(&Default::default());
        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("globe"),
            layout: &layout,
            entries: &[
                wgpu::BindGroupEntry { binding: 0, resource: globals.as_entire_binding() },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::TextureView(&view),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: wgpu::BindingResource::Sampler(&sampler),
                },
                wgpu::BindGroupEntry {
                    binding: 3,
                    resource: wgpu::BindingResource::TextureView(&day_view),
                },
            ],
        });
        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("globe"),
            bind_group_layouts: &[Some(&layout)],
            immediate_size: 0,
        });
        let module = device.create_shader_module(wgpu::include_wgsl!("globe.wgsl"));
        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("globe"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &module,
                entry_point: Some("vs_main"),
                compilation_options: Default::default(),
                buffers: &[],
            },
            fragment: Some(wgpu::FragmentState {
                module: &module,
                entry_point: Some("fs_main"),
                compilation_options: Default::default(),
                targets: &[Some(wgpu::ColorTargetState {
                    format,
                    blend: None,
                    write_mask: wgpu::ColorWrites::ALL,
                })],
            }),
            primitive: Default::default(),
            depth_stencil: None,
            multisample: Default::default(),
            multiview_mask: None,
            cache: None,
        });
        Self { device: device.clone(), queue: queue.clone(), globals, bind_group, pipeline }
    }

    pub fn render(&self, frame: &Frame, target: &wgpu::TextureView) {
        let options = Vec4::new(0.0, frame.borders as u8 as f32, frame.time, 0.0);
        let spreads = Vec4::new(map::SPREAD, map::SIDE_SPREAD, 0.0, 0.0);
        let mut globals = [0.0f32; GLOBAL_FLOATS];
        globals[..16].copy_from_slice(&frame.view_projection.inverse().to_cols_array());
        globals[16..20].copy_from_slice(&frame.eye.extend(1.0).to_array());
        globals[20..24].copy_from_slice(&frame.sun.extend(frame.night).to_array());
        globals[24..28].copy_from_slice(&options.to_array());
        globals[28..32].copy_from_slice(&spreads.to_array());
        self.queue.write_buffer(&self.globals, 0, bytemuck::cast_slice(&globals));

        let mut encoder = self.device.create_command_encoder(&Default::default());
        {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("globe"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: target,
                    depth_slice: None,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                ..Default::default()
            });
            pass.set_pipeline(&self.pipeline);
            pass.set_bind_group(0, &self.bind_group, &[]);
            pass.draw(0..3, 0..1);
        }
        self.queue.submit([encoder.finish()]);
    }
}
