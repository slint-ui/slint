// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT

// cSpell: ignore equirectangular multisampled

use glam::{Mat4, Vec2, Vec3, Vec4};
use slint::wgpu_30::wgpu;
use wgpu::util::DeviceExt;

use crate::compress;
use crate::map::{self, Map};
use crate::tiles;

/// What one frame of the globe shows, see `Globals` in `globe.wgsl`.
pub struct Frame {
    pub view_projection: Mat4,
    pub eye: Vec3,
    pub sun: Vec3,
    /// 0 lights the whole globe, 1 darkens the side away from the sun.
    pub night: f32,
    /// How strongly the borders show, from 0 to 1.
    pub borders: f32,
    /// How wide the borders are in pixels.
    pub border_width: f32,
    /// The size of the frame in pixels.
    pub viewport: Vec2,
    pub time: f32,
    /// 1 shows the stars and the black of space around the globe, 0 makes it transparent.
    pub space: f32,
    /// How strongly the clouds show, from 0 to 1.
    pub clouds: f32,
    /// The detail windows, see `tiles::Detail::update()`.
    pub windows: [[f32; tiles::WINDOW_FLOATS]; tiles::LEVELS],
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
    /// A single pixel of `rgba`, in sRGB.
    pub fn solid(rgba: [u8; 4]) -> Self {
        Self {
            width: 1,
            height: 1,
            format: wgpu::TextureFormat::Rgba8UnormSrgb,
            levels: vec![rgba.to_vec()],
        }
    }

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

    /// The format that `compress()` gives for `features`.
    pub fn compressed_format(features: wgpu::Features) -> wgpu::TextureFormat {
        Self::encoder(features).map_or(wgpu::TextureFormat::Rgba8UnormSrgb, |(_, format)| format)
    }

    /// BC1 on most desktop GPUs, ETC2 on most mobile and embedded ones.
    fn encoder(
        features: wgpu::Features,
    ) -> Option<(fn(&[u8], usize, usize) -> Vec<u8>, wgpu::TextureFormat)> {
        if features.contains(wgpu::Features::TEXTURE_COMPRESSION_BC) {
            Some((compress::bc1, wgpu::TextureFormat::Bc1RgbaUnormSrgb))
        } else if features.contains(wgpu::Features::TEXTURE_COMPRESSION_ETC2) {
            Some((compress::etc2, wgpu::TextureFormat::Etc2Rgb8UnormSrgb))
        } else {
            None
        }
    }

    /// One level of RGBA pixels in the format that `compressed_format()` gives for `features`.
    pub fn compress_level(features: wgpu::Features, rgba: Vec<u8>, size: usize) -> Vec<u8> {
        match Self::encoder(features) {
            Some((encode, _)) => encode(&rgba, size, size),
            None => rgba,
        }
    }

    /// Compresses the imagery to a format that `features` support, see `compressed_format()`,
    /// and otherwise leaves it as it is.
    pub fn compress(self, features: wgpu::Features) -> Self {
        let Some((encode, format)) = Self::encoder(features) else { return self };
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

fn imagery_view(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    imagery: &Imagery,
) -> wgpu::TextureView {
    let texture = device.create_texture_with_data(
        queue,
        &wgpu::TextureDescriptor {
            label: Some("globe imagery"),
            size: wgpu::Extent3d {
                width: imagery.width,
                height: imagery.height,
                depth_or_array_layers: 1,
            },
            mip_level_count: imagery.levels.len() as u32,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: imagery.format,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        },
        wgpu::util::TextureDataOrder::LayerMajor,
        &imagery.levels.concat(),
    );
    texture.create_view(&Default::default())
}

/// The bindings of `globe.wgsl`, with the map, the day and night imagery, and the clouds.
fn bind_group(
    device: &wgpu::Device,
    layout: &wgpu::BindGroupLayout,
    globals: &wgpu::Buffer,
    sampler: &wgpu::Sampler,
    [map, day, night, clouds]: [&wgpu::TextureView; 4],
    (tiles, tile_sampler): (&wgpu::TextureView, &wgpu::Sampler),
) -> wgpu::BindGroup {
    let view = |binding, view| wgpu::BindGroupEntry {
        binding,
        resource: wgpu::BindingResource::TextureView(view),
    };
    device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: Some("globe"),
        layout,
        entries: &[
            wgpu::BindGroupEntry { binding: 0, resource: globals.as_entire_binding() },
            view(1, map),
            wgpu::BindGroupEntry { binding: 2, resource: wgpu::BindingResource::Sampler(sampler) },
            view(3, day),
            view(4, night),
            view(5, clouds),
            view(6, tiles),
            wgpu::BindGroupEntry {
                binding: 7,
                resource: wgpu::BindingResource::Sampler(tile_sampler),
            },
        ],
    })
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
    /// Whether the shader encodes sRGB, because the target's format doesn't.
    encode_srgb: bool,
    cloud_texture: wgpu::Texture,
    layout: wgpu::BindGroupLayout,
    sampler: wgpu::Sampler,
    map_view: wgpu::TextureView,
    day_view: wgpu::TextureView,
    night_view: wgpu::TextureView,
    cloud_view: wgpu::TextureView,
    /// The detail windows of `tiles.rs`, one per layer.
    tile_texture: wgpu::Texture,
    tile_view: wgpu::TextureView,
    tile_sampler: wgpu::Sampler,
    tile_format: wgpu::TextureFormat,
}

const GLOBAL_FLOATS: usize = 48 + tiles::LEVELS * tiles::WINDOW_FLOATS;

impl Renderer {
    pub fn new(
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        map: &Map,
        format: wgpu::TextureFormat,
        with_tiles: bool,
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
        // A dark ocean and no city lights, until `set_day()` and `set_night()` bring the imagery.
        let day_view = imagery_view(device, queue, &Imagery::solid([6, 14, 32, 255]));
        let night_view = imagery_view(device, queue, &Imagery::solid([0, 0, 0, 255]));
        // Clear until `set_clouds()` fills it.
        let cloud_texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("globe clouds"),
            size: wgpu::Extent3d {
                width: crate::clouds::WIDTH,
                height: crate::clouds::HEIGHT,
                depth_or_array_layers: 1,
            },
            mip_level_count: crate::clouds::LEVELS,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::R8Unorm,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });
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
                wgpu::BindGroupLayoutEntry {
                    binding: 5,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: true },
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 6,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: true },
                        view_dimension: wgpu::TextureViewDimension::D2Array,
                        multisampled: false,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 7,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                    count: None,
                },
            ],
        });
        let map_view = texture.create_view(&Default::default());
        let cloud_view = cloud_texture.create_view(&Default::default());
        let tile_format = Imagery::compressed_format(device.features());
        // Without tiles, the shader never samples it, so one block per layer will do.
        let (window, mips) =
            if with_tiles { (tiles::WINDOW_TILES * tiles::TILE, tiles::MIPS) } else { (4, 1) };
        let tile_texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("globe detail"),
            size: wgpu::Extent3d {
                width: window,
                height: window,
                depth_or_array_layers: tiles::LEVELS as u32,
            },
            mip_level_count: mips,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: tile_format,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });
        let tile_view = tile_texture.create_view(&wgpu::TextureViewDescriptor {
            dimension: Some(wgpu::TextureViewDimension::D2Array),
            ..Default::default()
        });
        // A tile's place in its window comes from its column and row modulo the window's size,
        // so the texture repeats in both directions.
        let tile_sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            address_mode_u: wgpu::AddressMode::Repeat,
            address_mode_v: wgpu::AddressMode::Repeat,
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            mipmap_filter: wgpu::MipmapFilterMode::Linear,
            anisotropy_clamp: 8,
            ..Default::default()
        });
        let views = [&map_view, &day_view, &night_view, &cloud_view];
        let bind_group =
            bind_group(device, &layout, &globals, &sampler, views, (&tile_view, &tile_sampler));
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
            encode_srgb: !format.is_srgb(),
            cloud_texture,
            layout,
            sampler,
            map_view,
            day_view,
            night_view,
            cloud_view,
            tile_texture,
            tile_view,
            tile_sampler,
            tile_format,
        }
    }

    /// Replaces the placeholder for the imagery of the day side.
    pub fn set_day(&mut self, imagery: &Imagery) {
        self.day_view = imagery_view(&self.device, &self.queue, imagery);
        self.rebind();
    }

    /// Replaces the placeholder for the lights of the night side.
    pub fn set_night(&mut self, imagery: &Imagery) {
        self.night_view = imagery_view(&self.device, &self.queue, imagery);
        self.rebind();
    }

    fn rebind(&mut self) {
        let views = [&self.map_view, &self.day_view, &self.night_view, &self.cloud_view];
        self.bind_group = bind_group(
            &self.device,
            &self.layout,
            &self.globals,
            &self.sampler,
            views,
            (&self.tile_view, &self.tile_sampler),
        );
    }

    /// Puts a tile into the detail window `layer`, at the slot `slot` of its columns and rows.
    /// `mips` are the tile's `tiles::MIPS` levels, as `Imagery::compress_level()` gives them.
    pub fn set_tile(&self, layer: u32, slot: (u32, u32), mips: &[Vec<u8>]) {
        let compressed = self.tile_format != wgpu::TextureFormat::Rgba8UnormSrgb;
        for (level, data) in mips.iter().enumerate() {
            let size = tiles::TILE >> level;
            // BC1 and ETC2 store blocks of 4 by 4 pixels in 8 bytes.
            let (bytes_per_row, rows) =
                if compressed { (size / 4 * 8, size / 4) } else { (size * 4, size) };
            self.queue.write_texture(
                wgpu::TexelCopyTextureInfo {
                    texture: &self.tile_texture,
                    mip_level: level as u32,
                    origin: wgpu::Origin3d { x: slot.0 * size, y: slot.1 * size, z: layer },
                    aspect: wgpu::TextureAspect::All,
                },
                data,
                wgpu::TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some(bytes_per_row),
                    rows_per_image: Some(rows),
                },
                wgpu::Extent3d { width: size, height: size, depth_or_array_layers: 1 },
            );
        }
    }

    /// Fills the cloud texture with the mip levels from `clouds::generate()`.
    pub fn set_clouds(&self, levels: &[Vec<u8>]) {
        for (level, data) in levels.iter().enumerate() {
            let width = (crate::clouds::WIDTH >> level).max(1);
            let height = (crate::clouds::HEIGHT >> level).max(1);
            self.queue.write_texture(
                wgpu::TexelCopyTextureInfo {
                    texture: &self.cloud_texture,
                    mip_level: level as u32,
                    origin: wgpu::Origin3d::ZERO,
                    aspect: wgpu::TextureAspect::All,
                },
                data,
                wgpu::TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some(width),
                    rows_per_image: Some(height),
                },
                wgpu::Extent3d { width, height, depth_or_array_layers: 1 },
            );
        }
    }

    /// Renders `frame` into the top left of `target`, as large as `frame.viewport`.
    pub fn render(&self, frame: &Frame, target: &wgpu::TextureView) {
        let options = Vec4::new(frame.space, frame.border_width, frame.time, frame.borders);
        let mut globals = [0.0f32; GLOBAL_FLOATS];
        globals[..16].copy_from_slice(&frame.view_projection.to_cols_array());
        globals[16..32].copy_from_slice(&frame.view_projection.inverse().to_cols_array());
        globals[32..36].copy_from_slice(&frame.eye.extend(1.0).to_array());
        globals[36..40].copy_from_slice(&frame.sun.extend(frame.night).to_array());
        globals[40..44].copy_from_slice(&options.to_array());
        globals[44..46].copy_from_slice(&frame.viewport.to_array());
        globals[46] = self.encode_srgb as u8 as f32;
        globals[47] = frame.clouds;
        for (window, floats) in frame.windows.iter().enumerate() {
            let start = 48 + window * tiles::WINDOW_FLOATS;
            globals[start..start + tiles::WINDOW_FLOATS].copy_from_slice(floats);
        }
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
            if frame.borders > 0.0 {
                pass.set_pipeline(&self.border_pipeline);
                pass.set_vertex_buffer(0, self.border_segments.slice(..));
                pass.draw(0..6, 0..self.border_segment_count);
            }
        }
        self.queue.submit([encoder.finish()]);
    }
}
