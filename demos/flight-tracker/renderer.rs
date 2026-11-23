// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT

// cSpell: ignore equirectangular multisampled

use std::cell::Cell;

use glam::{Mat4, Vec2, Vec3, Vec4};
use slint::wgpu_30::wgpu;
use wgpu::util::DeviceExt;

use crate::compress;
use crate::map;
use crate::patches;
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
    pub windows: [[f32; tiles::WINDOW_FLOATS]; tiles::WINDOWS],
    /// Filters the imagery anisotropically, which keeps it sharp towards the horizon,
    /// and takes some GPUs a few milliseconds of a frame.
    pub anisotropic: bool,
    /// The flight's route, drawn over the globe in this order.
    pub route: Vec<RouteSegment>,
}

/// A segment of the route, as `vs_route` in `globe.wgsl` takes it.
#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
pub struct RouteSegment {
    pub start: [f32; 3],
    /// In pixels of the frame.
    pub width: f32,
    pub end: [f32; 3],
    pub unused: f32,
    /// Linear, with straight alpha.
    pub color: [f32; 4],
}

/// How many segments of the route fit into the buffer for them.
const MAX_ROUTE_SEGMENTS: usize = 1536;

/// How many patches fit into the buffer for them.
const MAX_PATCHES: usize = 2048;
/// How many stars there are around the globe.
const STARS: u32 = 9000;
/// The vertices of `vs_glow`, `vs_glare`, `vs_limb`, and `vs_glint` in `globe.wgsl`: six for each
/// of their segments, by their rings.
const GLOW_VERTICES: u32 = 6 * 128 * 7;
const GLARE_VERTICES: u32 = 6 * 32 * 32;
const LIMB_VERTICES: u32 = 6 * 128 * 2;
const GLINT_VERTICES: u32 = 6 * 24 * 8;

/// The grids of `vs_patch` in `globe.wgsl`, one for each of `patches::SEGMENTS`, as their
/// vertices and the indices of their triangles, and where each grid's are.
#[allow(clippy::type_complexity)]
fn patch_meshes()
-> (Vec<[f32; 2]>, Vec<u16>, [(i32, std::ops::Range<u32>); patches::SEGMENTS.len()]) {
    let (mut vertices, mut indices) = (Vec::new(), Vec::new());
    let ranges = patches::SEGMENTS.map(|segments| {
        let (base, first) = (vertices.len() as i32, indices.len() as u32);
        vertices.extend((0..=segments).flat_map(|row| {
            (0..=segments)
                .map(move |column| [column as f32 / segments as f32, row as f32 / segments as f32])
        }));
        let across = segments + 1;
        indices.extend((0..segments).flat_map(|row| {
            (0..segments).flat_map(move |column| {
                let (top, bottom) = (row * across + column, (row + 1) * across + column);
                [top, top + 1, bottom, top + 1, bottom + 1, bottom]
            })
        }));
        (base, first..indices.len() as u32)
    });
    (vertices, indices, ranges)
}

/// An image of the earth, equirectangular like the map, with its mip levels.
pub struct Imagery {
    pub width: u32,
    pub height: u32,
    pub format: wgpu::TextureFormat,
    pub levels: Vec<Vec<u8>>,
}

/// The mip levels of an image with `channels` bytes per pixel, from `base` down to one pixel,
/// each the average of the 2 by 2 pixels of the one before.
fn mip_levels(base: Vec<u8>, width: u32, height: u32, channels: usize) -> Vec<Vec<u8>> {
    let (mut width, mut height) = (width as usize, height as usize);
    let mut levels = vec![base];
    while width > 1 || height > 1 {
        let last = levels.last().unwrap();
        let (half_width, half_height) = ((width / 2).max(1), (height / 2).max(1));
        let mut level = Vec::with_capacity(half_width * half_height * channels);
        for y in 0..half_height {
            let row = |y: usize| &last[y.min(height - 1) * width * channels..][..width * channels];
            let (top, bottom) = (row(2 * y), row(2 * y + 1));
            for x in 0..half_width {
                let (left, right) = (2 * x * channels, (2 * x + 1).min(width - 1) * channels);
                level.extend((0..channels).map(|c| {
                    let sum = top[left + c] as u32
                        + top[right + c] as u32
                        + bottom[left + c] as u32
                        + bottom[right + c] as u32;
                    ((sum + 2) / 4) as u8
                }));
            }
        }
        levels.push(level);
        (width, height) = (half_width, half_height);
    }
    levels
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

    /// An image of one byte per pixel, with its mip levels.
    pub fn gray(width: u32, height: u32, pixels: Vec<u8>) -> Self {
        let levels = mip_levels(pixels, width, height, 1);
        Self { width, height, format: wgpu::TextureFormat::R8Unorm, levels }
    }

    /// Decodes a JPEG or PNG image to sRGB RGBA, and computes its mip levels.
    pub fn decode(bytes: &[u8]) -> Result<Self, image::ImageError> {
        let base = image::load_from_memory(bytes)?.to_rgba8();
        let (width, height) = base.dimensions();
        let levels = mip_levels(base.into_raw(), width, height, 4);
        Ok(Self { width, height, format: wgpu::TextureFormat::Rgba8UnormSrgb, levels })
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

    /// RGBA pixels, whose width and height are multiples of 4,
    /// in the format that `compressed_format()` gives for `features`.
    pub fn compress_level(
        features: wgpu::Features,
        rgba: &[u8],
        width: usize,
        height: usize,
    ) -> Vec<u8> {
        match Self::encoder(features) {
            Some((encode, _)) => encode(rgba, width, height),
            None => rgba.to_vec(),
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
    [map, day, night, clouds]: &[wgpu::TextureView; 4],
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
    /// Without and with anisotropic filtering.
    bind_groups: [wgpu::BindGroup; 2],
    border_pipeline: wgpu::RenderPipeline,
    border_segments: wgpu::Buffer,
    border_segment_count: u32,
    /// For each `patches::Kind`, under a clear sky and under clouds.
    patch_pipelines: [[wgpu::RenderPipeline; 5]; 2],
    route_pipeline: wgpu::RenderPipeline,
    route_segments: wgpu::Buffer,
    star_pipeline: wgpu::RenderPipeline,
    stars: wgpu::Buffer,
    glare_pipeline: wgpu::RenderPipeline,
    glint_pipeline: wgpu::RenderPipeline,
    /// The color and the alpha of the globe's smooth outline.
    limb_pipelines: [wgpu::RenderPipeline; 2],
    patch_vertices: wgpu::Buffer,
    patch_indices: wgpu::Buffer,
    patch_ranges: [(i32, std::ops::Range<u32>); patches::SEGMENTS.len()],
    patch_instances: wgpu::Buffer,
    glow_pipeline: wgpu::RenderPipeline,
    sun_pipeline: wgpu::RenderPipeline,
    /// Whether the shader encodes sRGB, because the target's format doesn't.
    encode_srgb: bool,
    layout: wgpu::BindGroupLayout,
    /// For the textures of the globe and for the tiles, as in `bind_groups`.
    samplers: [(wgpu::Sampler, wgpu::Sampler); 2],
    /// The map, the imagery of the day and of the night side, and the clouds.
    views: [wgpu::TextureView; 4],
    /// The detail windows of `tiles.rs`, one per layer.
    tile_texture: wgpu::Texture,
    tile_view: wgpu::TextureView,
    tile_format: wgpu::TextureFormat,
    /// Draws the tiles into `tile_texture`, where `draws_tiles()`.
    tile_drawing: Option<TileDrawing>,
    /// The detail window and the slot of the tile in `TileDrawing::levels`, to draw with the next
    /// frame.
    tile_draw: Cell<Option<(u32, (u32, u32))>>,
}

/// Draws a tile into its slot, see `Renderer::set_tile()`.
struct TileDrawing {
    pipeline: wgpu::RenderPipeline,
    /// A buffer for each of a tile's mip levels, as `TILE_DRAWING` reads it, and its bind group.
    /// They're made once: NXP's Vivante Vulkan driver reports its descriptor pools as full early,
    /// which wgpu doesn't expect.
    levels: Vec<(wgpu::Buffer, wgpu::BindGroup)>,
}

/// Draws a tile's mip level from a storage buffer into the viewport,
/// which covers the tile's slot in its detail window.
const TILE_DRAWING: &str = r#"
struct Level {
    // xy: the slot's top left in pixels, z: the level's width.
    header: vec4<u32>,
    // RGBA, a byte each.
    pixels: array<u32>,
}

@group(0) @binding(0) var<storage, read> level: Level;

@vertex
fn vs_main(@builtin(vertex_index) index: u32) -> @builtin(position) vec4<f32> {
    let corner = vec2<f32>(f32((index << 1u) & 2u), f32(index & 2u));
    return vec4<f32>(corner * 2.0 - 1.0, 0.0, 1.0);
}

@fragment
fn fs_main(@builtin(position) position: vec4<f32>) -> @location(0) vec4<f32> {
    let pixel = vec2<u32>(position.xy) - level.header.xy;
    return unpack4x8unorm(level.pixels[pixel.y * level.header.z + pixel.x]);
}
"#;

/// Whether the tiles go onto the GPU by drawing them from a buffer, uncompressed, rather than by
/// copying them.
/// NXP's Vivante Vulkan driver compiles a shader for many copies into a texture, which takes
/// about 100 ms each time.
/// WebGL can't read a buffer in a fragment shader.
pub fn draws_tiles(device: &wgpu::Device) -> bool {
    device.limits().max_storage_buffers_per_shader_stage > 0
}

const GLOBAL_FLOATS: usize = 32;

impl Renderer {
    pub fn new(
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        map: &Imagery,
        format: wgpu::TextureFormat,
        with_tiles: bool,
    ) -> Self {
        // A dark ocean, no city lights, and a clear sky, until `set_day()`, `set_night()`, and
        // `set_clouds()` bring them.
        let placeholder = |rgba| imagery_view(device, queue, &Imagery::solid(rgba));
        let views = [
            imagery_view(device, queue, map),
            placeholder([6, 14, 32, 255]),
            placeholder([0, 0, 0, 255]),
            placeholder([0, 0, 0, 255]),
        ];
        let smooth = wgpu::SamplerDescriptor {
            address_mode_u: wgpu::AddressMode::Repeat,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            mipmap_filter: wgpu::MipmapFilterMode::Linear,
            anisotropy_clamp: 8,
            ..Default::default()
        };
        // For the textures of the globe and for the tiles, without and with anisotropic
        // filtering.
        let samplers = [1, smooth.anisotropy_clamp].map(|anisotropy_clamp| {
            let smooth = wgpu::SamplerDescriptor { anisotropy_clamp, ..smooth.clone() };
            let tiles = wgpu::SamplerDescriptor {
                address_mode_v: wgpu::AddressMode::Repeat,
                ..smooth.clone()
            };
            (device.create_sampler(&smooth), device.create_sampler(&tiles))
        });
        let globals = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("globe globals"),
            size: std::mem::size_of::<[f32; GLOBAL_FLOATS]>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let texture = |binding, view_dimension| wgpu::BindGroupLayoutEntry {
            binding,
            visibility: wgpu::ShaderStages::FRAGMENT,
            ty: wgpu::BindingType::Texture {
                sample_type: wgpu::TextureSampleType::Float { filterable: true },
                view_dimension,
                multisampled: false,
            },
            count: None,
        };
        let filtering = |binding| wgpu::BindGroupLayoutEntry {
            binding,
            visibility: wgpu::ShaderStages::FRAGMENT,
            ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
            count: None,
        };
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
                texture(1, wgpu::TextureViewDimension::D2),
                filtering(2),
                texture(3, wgpu::TextureViewDimension::D2),
                texture(4, wgpu::TextureViewDimension::D2),
                texture(5, wgpu::TextureViewDimension::D2),
                texture(6, wgpu::TextureViewDimension::D2Array),
                filtering(7),
            ],
        });
        let drawn = with_tiles && draws_tiles(device);
        let tile_format = if drawn {
            wgpu::TextureFormat::Rgba8UnormSrgb
        } else {
            Imagery::compressed_format(device.features())
        };
        // Without tiles, the shader never samples it, so one block per layer will do.
        let (window, mips) =
            if with_tiles { (tiles::WINDOW_TILES * tiles::TILE, tiles::MIPS) } else { (4, 1) };
        let tile_texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("globe detail"),
            size: wgpu::Extent3d {
                width: window,
                height: window,
                depth_or_array_layers: tiles::WINDOWS as u32,
            },
            mip_level_count: mips,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: tile_format,
            usage: if drawn {
                wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::RENDER_ATTACHMENT
            } else {
                wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST
            },
            // The tiles' bytes are already sRGB, and go in as they are.
            view_formats: if drawn { &[wgpu::TextureFormat::Rgba8Unorm] } else { &[] },
        });
        let tile_drawing = drawn.then(|| {
            let module = device.create_shader_module(wgpu::ShaderModuleDescriptor {
                label: Some("tile drawing"),
                source: wgpu::ShaderSource::Wgsl(TILE_DRAWING.into()),
            });
            let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                label: Some("tile drawing"),
                layout: None,
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
                    targets: &[Some(wgpu::TextureFormat::Rgba8Unorm.into())],
                }),
                primitive: Default::default(),
                depth_stencil: None,
                multisample: Default::default(),
                multiview_mask: None,
                cache: None,
            });
            let levels = (0..tiles::MIPS)
                .map(|level| {
                    let size = (tiles::TILE >> level) as u64;
                    let buffer = device.create_buffer(&wgpu::BufferDescriptor {
                        label: Some("tile level"),
                        size: 16 + size * size * 4,
                        usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
                        mapped_at_creation: false,
                    });
                    let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
                        label: Some("tile level"),
                        layout: &pipeline.get_bind_group_layout(0),
                        entries: &[wgpu::BindGroupEntry {
                            binding: 0,
                            resource: buffer.as_entire_binding(),
                        }],
                    });
                    (buffer, bind_group)
                })
                .collect();
            TileDrawing { pipeline, levels }
        });
        let tile_view = tile_texture.create_view(&wgpu::TextureViewDescriptor {
            dimension: Some(wgpu::TextureViewDimension::D2Array),
            ..Default::default()
        });
        let bind_groups = samplers.each_ref().map(|(sampler, tile_sampler)| {
            bind_group(device, &layout, &globals, sampler, &views, (&tile_view, tile_sampler))
        });
        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("globe"),
            bind_group_layouts: &[Some(&layout)],
            immediate_size: 0,
        });
        let module = device.create_shader_module(wgpu::include_wgsl!("globe.wgsl"));
        let pipeline_with = |label,
                             [vertex, fragment]: [&str; 2],
                             buffers: &[_],
                             blend,
                             primitive: wgpu::PrimitiveState,
                             write_mask| {
            device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                label: Some(label),
                layout: Some(&pipeline_layout),
                vertex: wgpu::VertexState {
                    module: &module,
                    entry_point: Some(vertex),
                    compilation_options: Default::default(),
                    buffers,
                },
                fragment: Some(wgpu::FragmentState {
                    module: &module,
                    entry_point: Some(fragment),
                    compilation_options: Default::default(),
                    targets: &[Some(wgpu::ColorTargetState { format, blend, write_mask })],
                }),
                primitive,
                depth_stencil: None,
                multisample: Default::default(),
                multiview_mask: None,
                cache: None,
            })
        };
        let pipeline = |label, entries, buffers: &[_], blend| {
            pipeline_with(
                label,
                entries,
                buffers,
                blend,
                Default::default(),
                wgpu::ColorWrites::ALL,
            )
        };
        let segments = map::border_segments();
        let border_segments = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("globe borders"),
            contents: bytemuck::cast_slice(&segments),
            usage: wgpu::BufferUsages::VERTEX,
        });
        let segment = wgpu::VertexBufferLayout {
            array_stride: std::mem::size_of::<[[f32; 3]; 2]>() as u64,
            step_mode: wgpu::VertexStepMode::Instance,
            attributes: &wgpu::vertex_attr_array![0 => Float32x3, 1 => Float32x3],
        };
        let grid = wgpu::VertexBufferLayout {
            array_stride: std::mem::size_of::<[f32; 2]>() as u64,
            step_mode: wgpu::VertexStepMode::Vertex,
            attributes: &wgpu::vertex_attr_array![0 => Float32x2],
        };
        // The glow and the sun add their light, and the frame is opaque where either is.
        let add = wgpu::BlendState {
            color: wgpu::BlendComponent {
                src_factor: wgpu::BlendFactor::One,
                dst_factor: wgpu::BlendFactor::One,
                operation: wgpu::BlendOperation::Add,
            },
            alpha: wgpu::BlendComponent {
                src_factor: wgpu::BlendFactor::One,
                dst_factor: wgpu::BlendFactor::One,
                operation: wgpu::BlendOperation::Max,
            },
        };
        let over = wgpu::BlendState {
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
        };
        let (patch_vertices, patch_indices, patch_ranges) = patch_meshes();
        let patch_vertices = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("globe patch"),
            contents: bytemuck::cast_slice(&patch_vertices),
            usage: wgpu::BufferUsages::VERTEX,
        });
        let patch_indices = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("globe patch"),
            contents: bytemuck::cast_slice(&patch_indices),
            usage: wgpu::BufferUsages::INDEX,
        });
        let patch_instances = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("globe patches"),
            size: (MAX_PATCHES * std::mem::size_of::<patches::Patch>()) as u64,
            usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let patch_buffers = [
            Some(grid.clone()),
            Some(wgpu::VertexBufferLayout {
                array_stride: std::mem::size_of::<patches::Patch>() as u64,
                step_mode: wgpu::VertexStepMode::Instance,
                attributes: &wgpu::vertex_attr_array![1 => Float32x4, 2 => Float32x4],
            }),
        ];
        // The far side of the globe is culled: seen from outside, the triangles go clockwise.
        let outside = wgpu::PrimitiveState {
            front_face: wgpu::FrontFace::Cw,
            cull_mode: Some(wgpu::Face::Back),
            ..Default::default()
        };
        let patch_pipelines = [
            ["fs_patch_map", "fs_patch_tile", "fs_patch_night"],
            ["fs_patch_map_clouds", "fs_patch_tile_clouds", "fs_patch_night_clouds"],
        ]
        .map(|[map, tile, night]| {
            // In the order of `patches::Kind`.
            [(map, None), (tile, None), (night, None), (tile, Some(over)), (night, Some(over))].map(
                |(fragment, blend)| {
                    let entries = ["vs_patch", fragment];
                    let all = wgpu::ColorWrites::ALL;
                    pipeline_with("globe patch", entries, &patch_buffers, blend, outside, all)
                },
            )
        });
        let route_segments = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("globe route"),
            size: (MAX_ROUTE_SEGMENTS * std::mem::size_of::<RouteSegment>()) as u64,
            usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let route = wgpu::VertexBufferLayout {
            array_stride: std::mem::size_of::<RouteSegment>() as u64,
            step_mode: wgpu::VertexStepMode::Instance,
            attributes: &wgpu::vertex_attr_array![0 => Float32x4, 1 => Float32x4, 2 => Float32x4],
        };
        let stars = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("globe stars"),
            contents: bytemuck::cast_slice(&patches::stars(STARS as usize)),
            usage: wgpu::BufferUsages::VERTEX,
        });
        let star = wgpu::VertexBufferLayout {
            array_stride: std::mem::size_of::<[f32; 6]>() as u64,
            step_mode: wgpu::VertexStepMode::Instance,
            attributes: &wgpu::vertex_attr_array![0 => Float32x3, 1 => Float32x3],
        };
        // The ground fades into the glow's color by the alpha that the color pipeline writes,
        // and the frame's alpha fades by the one that the alpha pipeline writes.
        let fade = |color, alpha| wgpu::BlendState {
            color: wgpu::BlendComponent {
                src_factor: color,
                dst_factor: wgpu::BlendFactor::OneMinusSrcAlpha,
                operation: wgpu::BlendOperation::Add,
            },
            alpha: wgpu::BlendComponent {
                src_factor: alpha,
                dst_factor: wgpu::BlendFactor::OneMinusSrcAlpha,
                operation: wgpu::BlendOperation::Add,
            },
        };
        let limb_pipelines = [
            pipeline_with(
                "globe limb",
                ["vs_limb", "fs_limb_color"],
                &[],
                Some(fade(wgpu::BlendFactor::SrcAlpha, wgpu::BlendFactor::Zero)),
                Default::default(),
                wgpu::ColorWrites::COLOR,
            ),
            pipeline_with(
                "globe limb",
                ["vs_limb", "fs_limb_alpha"],
                &[],
                Some(fade(wgpu::BlendFactor::Zero, wgpu::BlendFactor::Zero)),
                Default::default(),
                wgpu::ColorWrites::ALPHA,
            ),
        ];
        Self {
            device: device.clone(),
            queue: queue.clone(),
            globals,
            bind_groups,
            patch_pipelines,
            route_pipeline: pipeline(
                "globe route",
                ["vs_route", "fs_route"],
                &[Some(route)],
                Some(over),
            ),
            route_segments,
            star_pipeline: pipeline(
                "globe stars",
                ["vs_star", "fs_star"],
                &[Some(star)],
                Some(add),
            ),
            stars,
            glare_pipeline: pipeline("globe glare", ["vs_glare", "fs_glare"], &[], Some(add)),
            glint_pipeline: pipeline("globe glint", ["vs_glint", "fs_glint"], &[], Some(add)),
            limb_pipelines,
            patch_vertices,
            patch_indices,
            patch_ranges,
            patch_instances,
            border_pipeline: pipeline(
                "globe borders",
                ["vs_border", "fs_border"],
                &[Some(segment)],
                Some(over),
            ),
            border_segments,
            border_segment_count: segments.len() as u32,
            glow_pipeline: pipeline("globe glow", ["vs_glow", "fs_glow"], &[], Some(add)),
            sun_pipeline: pipeline("globe sun", ["vs_sun", "fs_sun"], &[], Some(add)),
            encode_srgb: !format.is_srgb(),
            layout,
            samplers,
            views,
            tile_texture,
            tile_view,
            tile_format,
            tile_drawing,
            tile_draw: Default::default(),
        }
    }

    /// Replaces the placeholder for the imagery of the day side.
    pub fn set_day(&mut self, imagery: &Imagery) {
        self.set_view(1, imagery);
    }

    /// Replaces the placeholder for the lights of the night side.
    pub fn set_night(&mut self, imagery: &Imagery) {
        self.set_view(2, imagery);
    }

    /// Replaces the clear sky with the cloud cover from `clouds::generate()`.
    pub fn set_clouds(&mut self, clouds: &Imagery) {
        self.set_view(3, clouds);
    }

    fn set_view(&mut self, index: usize, imagery: &Imagery) {
        self.views[index] = imagery_view(&self.device, &self.queue, imagery);
        self.bind_groups = self.samplers.each_ref().map(|(sampler, tile_sampler)| {
            bind_group(
                &self.device,
                &self.layout,
                &self.globals,
                sampler,
                &self.views,
                (&self.tile_view, tile_sampler),
            )
        });
    }

    /// Puts a tile into the detail window `layer`, at the slot `slot` of its columns and rows.
    /// `mips` are the tile's `tiles::MIPS` levels, as `Imagery::compress_level()` gives them,
    /// or as RGBA pixels where `draws_tiles()`.
    /// A drawn tile goes onto the GPU with the next frame.
    /// On NXP's i.MX 8M Plus, that adds about 12 ms to the frame.
    pub fn set_tile(&self, layer: u32, slot: (u32, u32), mips: &[Vec<u8>]) {
        if let Some(drawing) = &self.tile_drawing {
            // The buffers hold one tile at a time.
            if self.tile_draw.get().is_some() {
                let mut encoder = self.device.create_command_encoder(&Default::default());
                self.draw_tile(&mut encoder);
                self.queue.submit([encoder.finish()]);
            }
            for (level, (pixels, (buffer, _))) in mips.iter().zip(&drawing.levels).enumerate() {
                let size = tiles::TILE >> level;
                let header: [u32; 4] = [slot.0 * size, slot.1 * size, size, 0];
                self.queue.write_buffer(buffer, 0, bytemuck::cast_slice(&header));
                self.queue.write_buffer(buffer, 16, pixels);
            }
            self.tile_draw.set(Some((layer, slot)));
            return;
        }
        let block = self.tile_format.block_dimensions().0;
        let block_bytes = self.tile_format.block_copy_size(None).unwrap_or_default();
        for (level, data) in mips.iter().enumerate() {
            let size = tiles::TILE >> level;
            let (bytes_per_row, rows) = (size / block * block_bytes, size / block);
            // NXP's Vivante Vulkan driver ignores the row length of a copy into part of a
            // compressed texture. wgpu pads the rows to `COPY_BYTES_PER_ROW_ALIGNMENT`, so
            // narrower levels go one row of blocks at a time.
            let rows_at_once =
                if bytes_per_row % wgpu::COPY_BYTES_PER_ROW_ALIGNMENT == 0 { rows } else { 1 };
            for first in (0..rows).step_by(rows_at_once as usize) {
                self.queue.write_texture(
                    wgpu::TexelCopyTextureInfo {
                        texture: &self.tile_texture,
                        mip_level: level as u32,
                        origin: wgpu::Origin3d {
                            x: slot.0 * size,
                            y: slot.1 * size + first * block,
                            z: layer,
                        },
                        aspect: wgpu::TextureAspect::All,
                    },
                    &data[(first * bytes_per_row) as usize..]
                        [..(rows_at_once * bytes_per_row) as usize],
                    wgpu::TexelCopyBufferLayout {
                        offset: 0,
                        bytes_per_row: Some(bytes_per_row),
                        rows_per_image: Some(rows_at_once),
                    },
                    wgpu::Extent3d {
                        width: size,
                        height: rows_at_once * block,
                        depth_or_array_layers: 1,
                    },
                );
            }
        }
    }

    /// Draws the tile that `set_tile()` left in the buffers into its slot.
    fn draw_tile(&self, encoder: &mut wgpu::CommandEncoder) {
        let (Some(drawing), Some((layer, slot))) = (&self.tile_drawing, self.tile_draw.take())
        else {
            return;
        };
        for (level, (_, bind_group)) in drawing.levels.iter().enumerate() {
            let size = tiles::TILE >> level;
            let view = self.tile_texture.create_view(&wgpu::TextureViewDescriptor {
                format: Some(wgpu::TextureFormat::Rgba8Unorm),
                dimension: Some(wgpu::TextureViewDimension::D2),
                base_mip_level: level as u32,
                mip_level_count: Some(1),
                base_array_layer: layer,
                array_layer_count: Some(1),
                ..Default::default()
            });
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("tile level"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &view,
                    depth_slice: None,
                    resolve_target: None,
                    ops: wgpu::Operations { load: wgpu::LoadOp::Load, store: wgpu::StoreOp::Store },
                })],
                ..Default::default()
            });
            let (x, y) = ((slot.0 * size) as f32, (slot.1 * size) as f32);
            pass.set_viewport(x, y, size as f32, size as f32, 0.0, 1.0);
            pass.set_pipeline(&drawing.pipeline);
            pass.set_bind_group(0, bind_group, &[]);
            pass.draw(0..3, 0..1);
        }
    }

    /// Draws the globe of `frame` as the patches of `patches.rs`, with its smooth outline and
    /// the sun's glint on the water.
    fn draw_patches(&self, pass: &mut wgpu::RenderPass, frame: &Frame) {
        let sun = (frame.night > 0.0).then_some(frame.sun);
        let patches = patches::patches(frame.eye, sun, &frame.windows);
        let mut instances: Vec<patches::Patch> =
            patches.0.iter().flat_map(|group| group.iter().flatten()).copied().collect();
        instances.truncate(MAX_PATCHES);
        self.queue.write_buffer(&self.patch_instances, 0, bytemuck::cast_slice(&instances));
        pass.set_vertex_buffer(0, self.patch_vertices.slice(..));
        pass.set_vertex_buffer(1, self.patch_instances.slice(..));
        pass.set_index_buffer(self.patch_indices.slice(..), wgpu::IndexFormat::Uint16);
        let mut first = 0;
        let pipelines = &self.patch_pipelines[(frame.clouds > 0.0) as usize];
        for (pipeline, group) in pipelines.iter().zip(&patches.0) {
            if group.iter().all(Vec::is_empty) {
                continue;
            }
            pass.set_pipeline(pipeline);
            for (patches, (base, indices)) in group.iter().zip(&self.patch_ranges) {
                let end = (first + patches.len()).min(instances.len());
                if end > first {
                    pass.draw_indexed(indices.clone(), *base, first as u32..end as u32);
                }
                first = end;
            }
        }
        let route = &frame.route[..frame.route.len().min(MAX_ROUTE_SEGMENTS)];
        if !route.is_empty() {
            self.queue.write_buffer(&self.route_segments, 0, bytemuck::cast_slice(route));
            pass.set_pipeline(&self.route_pipeline);
            pass.set_vertex_buffer(0, self.route_segments.slice(..));
            pass.draw(0..6, 0..route.len() as u32);
        }
        for pipeline in &self.limb_pipelines {
            pass.set_pipeline(pipeline);
            pass.draw(0..LIMB_VERTICES, 0..1);
        }
        pass.set_pipeline(&self.glint_pipeline);
        pass.draw(0..GLINT_VERTICES, 0..1);
    }

    /// Renders `frame` into the top left of `target`, as large as `frame.viewport`.
    pub fn render(&self, frame: &Frame, target: &wgpu::TextureView) {
        let options = Vec4::new(frame.space, frame.border_width, frame.time, frame.borders);
        let mut globals = [0.0f32; GLOBAL_FLOATS];
        globals[..16].copy_from_slice(&frame.view_projection.to_cols_array());
        globals[16..20].copy_from_slice(&frame.eye.extend(1.0).to_array());
        globals[20..24].copy_from_slice(&frame.sun.extend(frame.night).to_array());
        globals[24..28].copy_from_slice(&options.to_array());
        globals[28..30].copy_from_slice(&frame.viewport.to_array());
        globals[30] = self.encode_srgb as u8 as f32;
        globals[31] = frame.clouds;
        self.queue.write_buffer(&self.globals, 0, bytemuck::cast_slice(&globals));

        let mut encoder = self.device.create_command_encoder(&Default::default());
        // In the frame's commands, since a submission of their own takes long on some GPUs.
        self.draw_tile(&mut encoder);
        {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("globe"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: target,
                    depth_slice: None,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        // Space, or nothing where the globe has none around it.
                        load: wgpu::LoadOp::Clear(wgpu::Color {
                            a: frame.space as f64,
                            ..wgpu::Color::BLACK
                        }),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                ..Default::default()
            });
            pass.set_viewport(0.0, 0.0, frame.viewport.x, frame.viewport.y, 0.0, 1.0);
            pass.set_bind_group(0, &self.bind_groups[frame.anisotropic as usize], &[]);
            if frame.space > 0.0 {
                pass.set_pipeline(&self.star_pipeline);
                pass.set_vertex_buffer(0, self.stars.slice(..));
                pass.draw(0..3, 0..STARS);
                // The globe covers the sun where it's in front of it.
                pass.set_pipeline(&self.sun_pipeline);
                pass.draw(0..6, 0..1);
            }
            // The patches cover the glow where it reaches under the globe's outline.
            pass.set_pipeline(&self.glow_pipeline);
            pass.draw(0..GLOW_VERTICES, 0..1);
            self.draw_patches(&mut pass, frame);
            // The glare spills over everything in front of the sun.
            if frame.space > 0.0 {
                pass.set_pipeline(&self.glare_pipeline);
                pass.draw(0..GLARE_VERTICES, 0..1);
            }
            if frame.borders > 0.0 {
                pass.set_pipeline(&self.border_pipeline);
                pass.set_vertex_buffer(0, self.border_segments.slice(..));
                pass.draw(0..6, 0..self.border_segment_count);
            }
        }
        self.queue.submit([encoder.finish()]);
    }
}
