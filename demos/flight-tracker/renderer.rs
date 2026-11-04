// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT

// cSpell: ignore equirectangular multisampled

use glam::{Mat4, Vec2, Vec3, Vec4};
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
    /// How wide the borders are in pixels.
    pub border_width: f32,
    /// The size of the frame in pixels.
    pub viewport: Vec2,
    pub time: f32,
    /// 1 shows the stars and the black of space around the globe, 0 makes it transparent.
    pub space: f32,
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
    border_pipeline: wgpu::RenderPipeline,
    border_segments: wgpu::Buffer,
    border_segment_count: u32,
}

const GLOBAL_FLOATS: usize = 48;

impl Renderer {
    pub fn new(
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        map: &Map,
        imagery: &Imagery,
        night: &Imagery,
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
                format: wgpu::TextureFormat::R8Unorm,
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
        let night_texture = device.create_texture_with_data(
            queue,
            &wgpu::TextureDescriptor {
                label: Some("globe night imagery"),
                size: wgpu::Extent3d {
                    width: night.width,
                    height: night.height,
                    depth_or_array_layers: 1,
                },
                mip_level_count: night.levels.len() as u32,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format: night.format,
                usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
                view_formats: &[],
            },
            wgpu::util::TextureDataOrder::LayerMajor,
            &night.levels.concat(),
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
                    visibility: wgpu::ShaderStages::VERTEX_FRAGMENT,
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
                wgpu::BindGroupLayoutEntry {
                    binding: 4,
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
        let night_view = night_texture.create_view(&Default::default());
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
                wgpu::BindGroupEntry {
                    binding: 4,
                    resource: wgpu::BindingResource::TextureView(&night_view),
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
        let segments = map::border_segments();
        let border_segments = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("globe borders"),
            contents: bytemuck::cast_slice(&segments),
            usage: wgpu::BufferUsages::VERTEX,
        });
        let border_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("globe borders"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &module,
                entry_point: Some("vs_border"),
                compilation_options: Default::default(),
                buffers: &[Some(wgpu::VertexBufferLayout {
                    array_stride: std::mem::size_of::<[[f32; 3]; 2]>() as u64,
                    step_mode: wgpu::VertexStepMode::Instance,
                    attributes: &wgpu::vertex_attr_array![0 => Float32x3, 1 => Float32x3],
                })],
            },
            fragment: Some(wgpu::FragmentState {
                module: &module,
                entry_point: Some("fs_border"),
                compilation_options: Default::default(),
                targets: &[Some(wgpu::ColorTargetState {
                    format,
                    // The frame has straight alpha, which the borders leave as it is.
                    blend: Some(wgpu::BlendState {
                        color: wgpu::BlendComponent {
                            src_factor: wgpu::BlendFactor::SrcAlpha,
                            dst_factor: wgpu::BlendFactor::OneMinusSrcAlpha,
                            operation: wgpu::BlendOperation::Add,
                        },
                        alpha: wgpu::BlendComponent {
                            src_factor: wgpu::BlendFactor::Zero,
                            dst_factor: wgpu::BlendFactor::One,
                            operation: wgpu::BlendOperation::Add,
                        },
                    }),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
            }),
            primitive: Default::default(),
            depth_stencil: None,
            multisample: Default::default(),
            multiview_mask: None,
            cache: None,
        });
        Self {
            device: device.clone(),
            queue: queue.clone(),
            globals,
            bind_group,
            pipeline,
            border_pipeline,
            border_segments,
            border_segment_count: segments.len() as u32,
        }
    }

    /// Renders `frame` into the top left of `target`, as large as `frame.viewport`.
    pub fn render(&self, frame: &Frame, target: &wgpu::TextureView) {
        let options = Vec4::new(frame.space, frame.border_width, frame.time, 0.0);
        let mut globals = [0.0f32; GLOBAL_FLOATS];
        globals[..16].copy_from_slice(&frame.view_projection.to_cols_array());
        globals[16..32].copy_from_slice(&frame.view_projection.inverse().to_cols_array());
        globals[32..36].copy_from_slice(&frame.eye.extend(1.0).to_array());
        globals[36..40].copy_from_slice(&frame.sun.extend(frame.night).to_array());
        globals[40..44].copy_from_slice(&options.to_array());
        globals[44..46].copy_from_slice(&frame.viewport.to_array());
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
                        load: wgpu::LoadOp::Clear(wgpu::Color::TRANSPARENT),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                ..Default::default()
            });
            pass.set_viewport(0.0, 0.0, frame.viewport.x, frame.viewport.y, 0.0, 1.0);
            pass.set_pipeline(&self.pipeline);
            pass.set_bind_group(0, &self.bind_group, &[]);
            pass.draw(0..3, 0..1);
            if frame.borders {
                pass.set_pipeline(&self.border_pipeline);
                pass.set_vertex_buffer(0, self.border_segments.slice(..));
                pass.draw(0..6, 0..self.border_segment_count);
            }
        }
        self.queue.submit([encoder.finish()]);
    }
}
