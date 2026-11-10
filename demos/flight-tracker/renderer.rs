// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT

// cSpell: ignore equirectangular multisampled

use glam::{Mat4, Vec2, Vec3, Vec4, Vec4Swizzles};
use slint::wgpu_30::wgpu;
use wgpu::util::DeviceExt;

use crate::compress;
use crate::map;
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

    /// RGBA pixels, whose width and height are multiples of 4, in the format that
    /// `compressed_format()` gives for `features`.
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

/// The pixels of the frame that the globe and the glow of its atmosphere reach, as their left,
/// top, width, and height. `None` where the camera is inside the glow.
fn globe_rectangle(frame: &Frame) -> Option<[u32; 4]> {
    // The glow that `globe.wgsl` draws around the globe is below what 8 bits hold from here on.
    const REACH: f32 = 1.16;
    let distance = frame.eye.length();
    if distance <= REACH {
        return None;
    }
    // The rays that pass the center at `REACH` form a cone around the direction to it.
    let axis = -frame.eye / distance;
    let (side, up) = axis.any_orthonormal_pair();
    let sine = REACH / distance;
    let cosine = (1.0 - sine * sine).sqrt();
    let (mut low, mut high) = (Vec2::MAX, Vec2::MIN);
    const RAYS: usize = 32;
    for ray in 0..RAYS {
        let (s, c) = (ray as f32 / RAYS as f32 * std::f32::consts::TAU).sin_cos();
        let direction = axis * cosine + (side * c + up * s) * sine;
        let point = frame.view_projection * (frame.eye + direction).extend(1.0);
        if point.w <= 0.0 {
            return None;
        }
        let pixel = (point.xy() / point.w * Vec2::new(0.5, -0.5) + 0.5) * frame.viewport;
        (low, high) = (low.min(pixel), high.max(pixel));
    }
    // The cone's outline bulges out between the rays, by less than a hundredth of its size.
    let margin = (high - low) * 0.01 + 2.0;
    let low = (low - margin).floor().clamp(Vec2::ZERO, frame.viewport);
    let high = (high + margin).ceil().clamp(Vec2::ZERO, frame.viewport);
    let size = (high - low).max(Vec2::ONE);
    Some([low.x as u32, low.y as u32, size.x as u32, size.y as u32])
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
    layout: wgpu::BindGroupLayout,
    sampler: wgpu::Sampler,
    /// The map, the imagery of the day and of the night side, and the clouds.
    views: [wgpu::TextureView; 4],
    /// The detail windows of `tiles.rs`, one per layer.
    tile_texture: wgpu::Texture,
    tile_view: wgpu::TextureView,
    tile_sampler: wgpu::Sampler,
    tile_format: wgpu::TextureFormat,
}

const GLOBAL_FLOATS: usize = 48 + tiles::WINDOWS * tiles::WINDOW_FLOATS;

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
        let sampler = device.create_sampler(&smooth);
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
        let tile_format = Imagery::compressed_format(device.features());
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
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });
        let tile_view = tile_texture.create_view(&wgpu::TextureViewDescriptor {
            dimension: Some(wgpu::TextureViewDimension::D2Array),
            ..Default::default()
        });
        let tile_sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            address_mode_v: wgpu::AddressMode::Repeat,
            ..smooth
        });
        let bind_group =
            bind_group(device, &layout, &globals, &sampler, &views, (&tile_view, &tile_sampler));
        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("globe"),
            bind_group_layouts: &[Some(&layout)],
            immediate_size: 0,
        });
        let module = device.create_shader_module(wgpu::include_wgsl!("globe.wgsl"));
        let pipeline = |label, [vertex, fragment]: [&str; 2], buffers: &[_], blend| {
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
                    targets: &[Some(wgpu::ColorTargetState {
                        format,
                        blend,
                        write_mask: wgpu::ColorWrites::ALL,
                    })],
                }),
                primitive: Default::default(),
                depth_stencil: None,
                multisample: Default::default(),
                multiview_mask: None,
                cache: None,
            })
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
        // The frame has straight alpha, which the borders leave as it is.
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
        Self {
            device: device.clone(),
            queue: queue.clone(),
            globals,
            bind_group,
            pipeline: pipeline("globe", ["vs_main", "fs_main"], &[], None),
            border_pipeline: pipeline(
                "globe borders",
                ["vs_border", "fs_border"],
                &[Some(segment)],
                Some(over),
            ),
            border_segments,
            border_segment_count: segments.len() as u32,
            encode_srgb: !format.is_srgb(),
            layout,
            sampler,
            views,
            tile_texture,
            tile_view,
            tile_sampler,
            tile_format,
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
        self.bind_group = bind_group(
            &self.device,
            &self.layout,
            &self.globals,
            &self.sampler,
            &self.views,
            (&self.tile_view, &self.tile_sampler),
        );
    }

    /// Puts a tile into the detail window `layer`, at the slot `slot` of its columns and rows.
    /// `mips` are the tile's `tiles::MIPS` levels, as `Imagery::compress_level()` gives them.
    pub fn set_tile(&self, layer: u32, slot: (u32, u32), mips: &[Vec<u8>]) {
        let block = self.tile_format.block_dimensions().0;
        let block_bytes = self.tile_format.block_copy_size(None).unwrap_or_default();
        for (level, data) in mips.iter().enumerate() {
            let size = tiles::TILE >> level;
            let (bytes_per_row, rows) = (size / block * block_bytes, size / block);
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
            // Without space around it, the globe leaves the rest of the frame transparent, as
            // the pass cleared it.
            if frame.space == 0.0
                && let Some([x, y, width, height]) = globe_rectangle(frame)
            {
                pass.set_scissor_rect(x, y, width, height);
            }
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::camera::{Pose, surface};

    /// The rays through the pixels around the globe's rectangle pass the globe beyond the reach
    /// of its glow, with the globe anywhere in the frame, near or far, and tilted.
    #[test]
    fn globe_rectangle_holds_the_glow() {
        let viewport = Vec2::new(640.0, 400.0);
        let aspect = viewport.x / viewport.y;
        for (range, tilt, shift) in [
            (2.6, 0.0, Vec2::ZERO),
            (4.0, 0.0, Vec2::new(0.5, -0.2)),
            (1.5, 0.0, Vec2::new(-0.6, 0.3)),
            (0.6, 50.0, Vec2::new(0.2, 0.4)),
            (6.0, 20.0, Vec2::new(-0.3, -0.5)),
        ] {
            let pose = Pose {
                target: surface(0.4, 0.3),
                heading: 0.7,
                tilt: f32::to_radians(tilt),
                range,
            };
            let view_projection =
                Mat4::from_translation(shift.extend(0.0)) * pose.view_projection(aspect);
            let frame = Frame {
                view_projection,
                eye: pose.eye(),
                sun: Vec3::X,
                night: 1.0,
                borders: 0.0,
                border_width: 1.0,
                viewport,
                time: 0.0,
                space: 0.0,
                clouds: 0.0,
                windows: Default::default(),
            };
            let [left, top, width, height] = globe_rectangle(&frame).unwrap();
            let (right, bottom) = (left + width, top + height);
            let inverse = view_projection.inverse();
            for y in 0..viewport.y as u32 {
                for x in 0..viewport.x as u32 {
                    if (left..right).contains(&x) && (top..bottom).contains(&y) {
                        continue;
                    }
                    let ndc = (Vec2::new(x as f32, y as f32) + 0.5) / viewport
                        * Vec2::new(2.0, -2.0)
                        + Vec2::new(-1.0, 1.0);
                    let near = inverse.project_point3(ndc.extend(0.0));
                    let far = inverse.project_point3(ndc.extend(1.0));
                    let direction = (far - near).normalize();
                    let along = -frame.eye.dot(direction);
                    let miss = (frame.eye + direction * along).length();
                    assert!(along <= 0.0 || miss > 1.14, "{miss} at {x}, {y} for range {range}");
                }
            }
        }
    }
}
