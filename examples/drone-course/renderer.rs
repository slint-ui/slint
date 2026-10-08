// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT

// cSpell: ignore metalness multisampled multisampling texel

//! A small forward renderer for the racing hall, on Slint's wgpu device.
//!
//! Each frame is one render pass and one queue submission: on small embedded GPUs, such as
//! the i.MX 8M Plus's, the driver makes every submission expensive.

use std::collections::HashMap;
use std::f32::consts::PI;
use std::rc::Rc;

use glam::{Mat4, Quat, Vec2, Vec3, Vec4};
use slint::wgpu_30::wgpu;
use wgpu::util::DeviceExt;

/// The vertices and triangles of a mesh.
#[derive(Clone, Default)]
pub struct Geometry {
    pub positions: Vec<[f32; 3]>,
    pub normals: Vec<[f32; 3]>,
    pub uvs: Vec<[f32; 2]>,
    /// Multiplies the material's color; white where missing.
    pub colors: Vec<[f32; 3]>,
    pub indices: Vec<u32>,
}

/// Floats per vertex: position, normal, uv, and color, the vertex shaders' inputs.
const VERTEX_FLOATS: usize = 11;

impl Geometry {
    fn vertex_data(&self) -> Vec<f32> {
        let mut data = Vec::with_capacity(self.positions.len() * VERTEX_FLOATS);
        for (i, position) in self.positions.iter().enumerate() {
            data.extend(position);
            data.extend(self.normals.get(i).unwrap_or(&[0.0, 1.0, 0.0]));
            data.extend(self.uvs.get(i).unwrap_or(&[0.0, 0.0]));
            data.extend(self.colors.get(i).unwrap_or(&[1.0, 1.0, 1.0]));
        }
        data
    }
}

pub struct Mesh {
    vertices: wgpu::Buffer,
    indices: wgpu::Buffer,
    index_count: u32,
}

/// RGBA8 texels in sRGB, with all mipmaps from the largest down to 1x1.
pub struct TextureData {
    pub width: u32,
    pub height: u32,
    pub levels: Vec<Vec<u8>>,
    pub repeat: bool,
}

pub struct Texture {
    /// Without and with anisotropic filtering, for low and high quality.
    bind_groups: [wgpu::BindGroup; 2],
}

/// A spot of light or shade on the floor, drawn into its light map.
#[derive(Clone, PartialEq)]
pub struct FloorSpot {
    /// On the floor, in x and z.
    pub center: Vec2,
    /// The direction of its length on the floor, in x and z, of length 1.
    pub along: Vec2,
    /// Half its length and width, in meters.
    pub half: Vec2,
    /// The light it adds at full strength.
    pub light: Vec3,
    /// How much of the floor's own light it takes away at full strength, from 0 to 1.
    pub shade: f32,
    pub shape: SpotShape,
}

#[derive(Clone, Copy, PartialEq)]
pub enum SpotShape {
    /// Fading out from the center like `textures::radial`.
    Round,
    /// Full strength inside, fading out over the given share of its half length and width.
    Soft(Vec2),
}

/// The light and shade that spots add to a floor centered on the origin, drawn by
/// `Renderer::draw_light_map`. Its color adds light, and its alpha multiplies the floor's
/// own light. The texture runs along x and against z, like `geometry::plane` laid flat.
pub struct LightMap {
    /// The floor's size in meters.
    size: Vec2,
    target: wgpu::TextureView,
    pub texture: Rc<Texture>,
}

/// The most spots `Renderer::draw_light_map` draws, as in `light_map.wgsl`.
const MAX_SPOTS_IN_LIGHT_MAP: usize = 192;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Blend {
    Opaque,
    Alpha,
    Additive,
}

#[derive(Clone)]
pub struct Material {
    /// Linear RGB, above 1 for surfaces that glow brighter than white.
    pub color: Vec3,
    pub opacity: f32,
    pub texture: Option<Rc<Texture>>,
    pub uv_scale: Vec2,
    pub uv_offset: Vec2,
    /// Roughness and metalness for surfaces the lights shade, `None` for ones that glow
    /// in their own color.
    pub lit: Option<(f32, f32)>,
    pub blend: Blend,
    pub double_sided: bool,
    pub depth_write: bool,
    /// Light added in low quality, across the mesh's texture coordinates before
    /// `uv_scale`.
    pub light_map: Option<Rc<Texture>>,
}

impl Material {
    pub fn unlit(color: Vec3) -> Self {
        Self {
            color,
            opacity: 1.0,
            texture: None,
            uv_scale: Vec2::ONE,
            uv_offset: Vec2::ZERO,
            lit: None,
            blend: Blend::Opaque,
            double_sided: false,
            depth_write: true,
            light_map: None,
        }
    }

    pub fn lit(color: Vec3, roughness: f32, metalness: f32) -> Self {
        Self { lit: Some((roughness, metalness)), ..Self::unlit(color) }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Node(usize);

#[derive(Clone)]
pub struct Object {
    pub parent: Option<Node>,
    pub translation: Vec3,
    pub rotation: Quat,
    pub scale: Vec3,
    /// Also hides the children.
    pub visible: bool,
    pub mesh: Option<Rc<Mesh>>,
    pub material: Material,
    /// Transparent objects with a higher order draw later, whatever their distance.
    pub render_order: i32,
    /// Draws the mesh once for each, all in one draw call. `None` draws it once.
    pub instances: Option<Vec<Instance>>,
}

/// One copy of an instanced object's mesh.
#[derive(Clone, Copy)]
pub struct Instance {
    /// Moves, rotates, and scales the copy within the object.
    pub transform: Mat4,
    /// Multiplies the material's color and opacity.
    pub color: Vec4,
}

/// The floats of an instance's vertex inputs in `shader.wgsl`: its transform and color.
const INSTANCE_FLOATS: usize = 16 + 4;

impl Object {
    pub fn new(parent: Option<Node>, mesh: Option<Rc<Mesh>>, material: Material) -> Self {
        Self {
            parent,
            translation: Vec3::ZERO,
            rotation: Quat::IDENTITY,
            scale: Vec3::ONE,
            visible: true,
            mesh,
            material,
            render_order: 0,
            instances: None,
        }
    }

    pub fn place(&mut self, translation: Vec3, rotation: Quat, scale: Vec3) {
        (self.translation, self.rotation, self.scale) = (translation, rotation, scale);
    }
}

pub struct SpotLight {
    pub position: Vec3,
    pub target: Vec3,
    pub color: Vec3,
    pub intensity: f32,
    pub distance: f32,
    /// The half angle of the cone, in radians.
    pub angle: f32,
    /// The part of the cone, from 0 to 1, where the light fades out towards the edge.
    pub penumbra: f32,
}

/// The lights, with intensities as in three.js: a surface lit head-on by a light of
/// intensity pi reflects its own color.
#[derive(Default)]
pub struct Lights {
    pub sky: Vec3,
    pub ground: Vec3,
    pub hemisphere_intensity: f32,
    /// Towards the light.
    pub sun_direction: Vec3,
    pub sun_color: Vec3,
    pub sun_intensity: f32,
    /// Only high quality draws these, see `MAX_SPOTS`.
    pub spots: Vec<SpotLight>,
}

/// The spot lights `shader.wgsl` has room for.
const MAX_SPOTS: usize = 5;

pub struct Camera {
    pub translation: Vec3,
    pub rotation: Quat,
    pub fov_degrees: f32,
    pub aspect: f32,
    pub near: f32,
    pub far: f32,
}

#[derive(Default)]
pub struct Scene {
    objects: Vec<Option<Object>>,
    free: Vec<usize>,
    pub background: Vec3,
    pub lights: Lights,
}

impl Scene {
    pub fn add(&mut self, object: Object) -> Node {
        match self.free.pop() {
            Some(index) => {
                self.objects[index] = Some(object);
                Node(index)
            }
            None => {
                self.objects.push(Some(object));
                Node(self.objects.len() - 1)
            }
        }
    }

    /// Adds `mesh` with `material` under `parent`, moved, rotated, and scaled.
    pub fn add_mesh(
        &mut self,
        parent: Option<Node>,
        mesh: &Rc<Mesh>,
        material: &Material,
        translation: Vec3,
        rotation: Quat,
        scale: Vec3,
    ) -> Node {
        let mut object = Object::new(parent, Some(mesh.clone()), material.clone());
        object.place(translation, rotation, scale);
        self.add(object)
    }

    /// Adds `mesh` with `material`, drawn once for each of its instances, of which it has
    /// none yet.
    pub fn add_instanced(&mut self, mesh: &Rc<Mesh>, material: &Material) -> Node {
        let node = self.add_mesh(None, mesh, material, Vec3::ZERO, Quat::IDENTITY, Vec3::ONE);
        self.object(node).instances = Some(Vec::new());
        node
    }

    /// Adds an object that only groups and moves its children.
    pub fn group(&mut self, parent: Option<Node>) -> Node {
        self.add(Object::new(parent, None, Material::unlit(Vec3::ONE)))
    }

    pub fn object(&mut self, node: Node) -> &mut Object {
        self.objects[node.0].as_mut().expect("the node wasn't removed")
    }

    /// Removes `node`'s children and their descendants.
    pub fn clear(&mut self, node: Node) {
        let children: Vec<usize> = (0..self.objects.len())
            .filter(|&i| self.objects[i].as_ref().is_some_and(|o| o.parent == Some(node)))
            .collect();
        for child in children {
            self.clear(Node(child));
            self.objects[child] = None;
            self.free.push(child);
        }
    }

    /// The object's transform in the world, if it and its ancestors are visible.
    fn world_transform(&self, index: usize, cache: &mut [Option<Option<Mat4>>]) -> Option<Mat4> {
        if let Some(known) = cache[index] {
            return known;
        }
        let object = self.objects[index].as_ref()?;
        let parent = match object.parent {
            Some(parent) => self.world_transform(parent.0, cache),
            None => Some(Mat4::IDENTITY),
        };
        let local = Mat4::from_scale_rotation_translation(
            object.scale,
            object.rotation,
            object.translation,
        );
        let world = parent.filter(|_| object.visible).map(|parent| parent * local);
        cache[index] = Some(world);
        world
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Hash)]
struct PipelineKey {
    lit: bool,
    high: bool,
    blend: Blend,
    double_sided: bool,
    depth_write: bool,
    light_map: bool,
    multisample: bool,
    /// Selects one of two identical pipelines. NXP's Vivante driver loses parts of a draw
    /// where the next draw overlaps it with the same pipeline, blended or not, so
    /// consecutive draws alternate between the two.
    copy: bool,
}

/// The floats of `Globals` in `shader.wgsl`.
const GLOBAL_FLOATS: usize = 6 * 4 + MAX_SPOTS * 12;
/// The floats of `Object` in `shader.wgsl`.
const OBJECT_FLOATS: usize = 2 * 16 + 3 * 4;

pub struct Renderer {
    device: wgpu::Device,
    queue: wgpu::Queue,
    color_format: wgpu::TextureFormat,
    shader: wgpu::ShaderModule,
    /// Without and with a light map.
    pipeline_layouts: [wgpu::PipelineLayout; 2],
    pipelines: HashMap<PipelineKey, wgpu::RenderPipeline>,
    globals: wgpu::Buffer,
    globals_bind_group: wgpu::BindGroup,
    object_layout: wgpu::BindGroupLayout,
    /// Each object's uniforms, `object_stride` bytes apart.
    objects: wgpu::Buffer,
    objects_bind_group: wgpu::BindGroup,
    /// The instances of each frame's draws, after an untransformed white one that draws
    /// objects without instances.
    instances: wgpu::Buffer,
    object_stride: usize,
    texture_layout: wgpu::BindGroupLayout,
    samplers: [[wgpu::Sampler; 2]; 2],
    white: Rc<Texture>,
    /// The light map of high quality's lit surfaces without one: no light added or taken.
    no_light: Rc<Texture>,
    depth_format: wgpu::TextureFormat,
    depth: Option<wgpu::Texture>,
    multisampled: Option<wgpu::Texture>,
    light_map_pipeline: wgpu::RenderPipeline,
    /// The spots for `light_map.wgsl`.
    light_map_spots: wgpu::Buffer,
    light_map_bind_group: wgpu::BindGroup,
}

/// Multisampling for high quality. It costs little on tile-based GPUs, but about 3 ms per
/// frame at 720p on NXP's Vivante GPU.
const SAMPLES: u32 = 4;

/// The most precise depth format `device` can render to. NXP's Vivante driver only renders
/// to 16-bit depth.
#[cfg(not(target_arch = "wasm32"))]
fn depth_format(device: &wgpu::Device) -> wgpu::TextureFormat {
    use wgpu::TextureFormat::*;
    [Depth24Plus, Depth32Float]
        .into_iter()
        .find(|&format| {
            let scope = device.push_error_scope(wgpu::ErrorFilter::Validation);
            let _ = device.create_texture(&wgpu::TextureDescriptor {
                label: None,
                size: wgpu::Extent3d { width: 1, height: 1, depth_or_array_layers: 1 },
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format,
                usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
                view_formats: &[],
            });
            pollster::block_on(scope.pop()).is_none()
        })
        .unwrap_or(Depth16Unorm)
}

/// WebGPU and WebGL 2 both render to 24-bit depth, and a browser can't wait for an error
/// scope synchronously.
#[cfg(target_arch = "wasm32")]
fn depth_format(_device: &wgpu::Device) -> wgpu::TextureFormat {
    wgpu::TextureFormat::Depth24Plus
}

impl Renderer {
    /// A renderer that draws into views of `color_format`.
    pub fn new(
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        color_format: wgpu::TextureFormat,
    ) -> Self {
        let uniform_layout = |dynamic| {
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: None,
                entries: &[wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::VERTEX_FRAGMENT,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: dynamic,
                        min_binding_size: None,
                    },
                    count: None,
                }],
            })
        };
        let globals_layout = uniform_layout(false);
        let object_layout = uniform_layout(true);
        let texture_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: None,
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: true },
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
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
        let pipeline_layouts = [3, 4].map(|groups| {
            let layouts = [
                Some(&globals_layout),
                Some(&object_layout),
                Some(&texture_layout),
                Some(&texture_layout),
            ];
            device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: None,
                bind_group_layouts: &layouts[..groups],
                immediate_size: 0,
            })
        });

        let globals = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("drone course globals"),
            size: (GLOBAL_FLOATS * 4) as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let globals_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: None,
            layout: &globals_layout,
            entries: &[wgpu::BindGroupEntry { binding: 0, resource: globals.as_entire_binding() }],
        });

        let alignment = device.limits().min_uniform_buffer_offset_alignment as usize;
        let object_stride = (OBJECT_FLOATS * 4).div_ceil(alignment) * alignment;
        let (objects, objects_bind_group) =
            Self::object_buffer(device, &object_layout, object_stride, 64);

        let samplers = [false, true].map(|repeat| {
            [1, 4].map(|anisotropy| {
                let address_mode =
                    if repeat { wgpu::AddressMode::Repeat } else { wgpu::AddressMode::ClampToEdge };
                device.create_sampler(&wgpu::SamplerDescriptor {
                    address_mode_u: address_mode,
                    address_mode_v: address_mode,
                    mag_filter: wgpu::FilterMode::Linear,
                    min_filter: wgpu::FilterMode::Linear,
                    // Blending two mip levels costs Vivante GPUs about 4 ms per frame at 1080p,
                    // so low quality picks the nearest one.
                    mipmap_filter: if anisotropy > 1 {
                        wgpu::MipmapFilterMode::Linear
                    } else {
                        wgpu::MipmapFilterMode::Nearest
                    },
                    anisotropy_clamp: anisotropy,
                    ..Default::default()
                })
            })
        });

        let shader = device.create_shader_module(wgpu::include_wgsl!("shader.wgsl"));
        let (light_map_pipeline, light_map_spots, light_map_bind_group) =
            Self::light_map_pipeline(device);
        let white = upload_texture(
            device,
            queue,
            &texture_layout,
            &samplers,
            &TextureData { width: 1, height: 1, levels: vec![vec![255; 4]], repeat: false },
        );
        let no_light = upload_texture(
            device,
            queue,
            &texture_layout,
            &samplers,
            &TextureData { width: 1, height: 1, levels: vec![vec![0, 0, 0, 255]], repeat: false },
        );

        Self {
            device: device.clone(),
            queue: queue.clone(),
            color_format,
            shader,
            pipeline_layouts,
            pipelines: HashMap::new(),
            globals,
            globals_bind_group,
            object_layout,
            objects,
            objects_bind_group,
            object_stride,
            texture_layout,
            samplers,
            white,
            no_light,
            depth_format: depth_format(device),
            depth: None,
            multisampled: None,
            instances: Self::instance_buffer(device, 64),
            light_map_pipeline,
            light_map_spots,
            light_map_bind_group,
        }
    }

    fn light_map_pipeline(
        device: &wgpu::Device,
    ) -> (wgpu::RenderPipeline, wgpu::Buffer, wgpu::BindGroup) {
        let layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("drone course light map"),
            entries: &[wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::VERTEX,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Uniform,
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            }],
        });
        let spots = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("drone course light map spots"),
            size: (MAX_SPOTS_IN_LIGHT_MAP * 16 * 4) as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("drone course light map"),
            layout: &layout,
            entries: &[wgpu::BindGroupEntry { binding: 0, resource: spots.as_entire_binding() }],
        });
        let shader = device.create_shader_module(wgpu::include_wgsl!("light_map.wgsl"));
        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("drone course light map"),
            bind_group_layouts: &[Some(&layout)],
            immediate_size: 0,
        });
        let add = wgpu::BlendComponent {
            src_factor: wgpu::BlendFactor::One,
            dst_factor: wgpu::BlendFactor::One,
            operation: wgpu::BlendOperation::Add,
        };
        let multiply = wgpu::BlendComponent {
            src_factor: wgpu::BlendFactor::Zero,
            dst_factor: wgpu::BlendFactor::SrcAlpha,
            operation: wgpu::BlendOperation::Add,
        };
        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("drone course light map"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vs_spot"),
                compilation_options: Default::default(),
                buffers: &[],
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: Some("fs_spot"),
                compilation_options: Default::default(),
                targets: &[Some(wgpu::ColorTargetState {
                    format: wgpu::TextureFormat::Rgba8UnormSrgb,
                    blend: Some(wgpu::BlendState { color: add, alpha: multiply }),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
            }),
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleStrip,
                ..Default::default()
            },
            depth_stencil: None,
            multisample: Default::default(),
            multiview_mask: None,
            cache: None,
        });
        (pipeline, spots, bind_group)
    }

    fn instance_buffer(device: &wgpu::Device, capacity: usize) -> wgpu::Buffer {
        device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("drone course instances"),
            size: (capacity * INSTANCE_FLOATS * 4) as u64,
            usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        })
    }

    fn object_buffer(
        device: &wgpu::Device,
        layout: &wgpu::BindGroupLayout,
        stride: usize,
        capacity: usize,
    ) -> (wgpu::Buffer, wgpu::BindGroup) {
        let buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("drone course objects"),
            size: (stride * capacity) as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: None,
            layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: wgpu::BindingResource::Buffer(wgpu::BufferBinding {
                    buffer: &buffer,
                    offset: 0,
                    size: wgpu::BufferSize::new((OBJECT_FLOATS * 4) as u64),
                }),
            }],
        });
        (buffer, bind_group)
    }

    pub fn create_mesh(&self, geometry: &Geometry) -> Rc<Mesh> {
        let create = |label, contents: &[u8], usage| {
            self.device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some(label),
                contents,
                usage: usage | wgpu::BufferUsages::COPY_DST,
            })
        };
        Rc::new(Mesh {
            vertices: create(
                "drone course vertices",
                bytemuck::cast_slice(&geometry.vertex_data()),
                wgpu::BufferUsages::VERTEX,
            ),
            indices: create(
                "drone course indices",
                bytemuck::cast_slice(&geometry.indices),
                wgpu::BufferUsages::INDEX,
            ),
            index_count: geometry.indices.len() as u32,
        })
    }

    /// Replaces the vertices of `mesh`, which must have been created with as many.
    pub fn update_mesh(&self, mesh: &Mesh, geometry: &Geometry) {
        self.queue.write_buffer(&mesh.vertices, 0, bytemuck::cast_slice(&geometry.vertex_data()));
    }

    pub fn create_texture(&self, data: &TextureData) -> Rc<Texture> {
        upload_texture(&self.device, &self.queue, &self.texture_layout, &self.samplers, data)
    }

    /// A light map for a floor of `size` meters, without spots.
    pub fn create_light_map(&self, size: Vec2, texels_per_meter: f32) -> LightMap {
        let texture = self.device.create_texture(&wgpu::TextureDescriptor {
            label: Some("drone course light map"),
            size: wgpu::Extent3d {
                width: (size.x * texels_per_meter) as u32,
                height: (size.y * texels_per_meter) as u32,
                depth_or_array_layers: 1,
            },
            // The pools are too soft to need mipmaps.
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8UnormSrgb,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::RENDER_ATTACHMENT,
            view_formats: &[],
        });
        let bind_groups = texture_bind_groups(
            &self.device,
            &self.texture_layout,
            &self.samplers,
            &texture,
            false,
        );
        let light_map = LightMap {
            size,
            target: texture.create_view(&Default::default()),
            texture: Rc::new(Texture { bind_groups }),
        };
        self.draw_light_map(&light_map, &[]);
        light_map
    }

    /// Replaces the spots in `light_map`.
    ///
    /// The GPU draws them, since uploading texels stalls NXP's Vivante driver for about
    /// 100 ms.
    pub fn draw_light_map(&self, light_map: &LightMap, spots: &[FloorSpot]) {
        let spots = &spots[..spots.len().min(MAX_SPOTS_IN_LIGHT_MAP)];
        let mut data = vec![0f32; MAX_SPOTS_IN_LIGHT_MAP * 16];
        let (centers, rest) = data.split_at_mut(MAX_SPOTS_IN_LIGHT_MAP * 4);
        let (axes, rest) = rest.split_at_mut(MAX_SPOTS_IN_LIGHT_MAP * 4);
        let (colors, shapes) = rest.split_at_mut(MAX_SPOTS_IN_LIGHT_MAP * 4);
        let to_clip = 2.0 / light_map.size;
        for (i, spot) in spots.iter().enumerate() {
            let across = spot.along.perp();
            let along = spot.along * spot.half.x * to_clip;
            let across = across * spot.half.y * to_clip;
            let center = spot.center * to_clip;
            centers[i * 4..][..2].copy_from_slice(&center.to_array());
            axes[i * 4..][..4].copy_from_slice(&[along.x, along.y, across.x, across.y]);
            colors[i * 4..][..4].copy_from_slice(&spot.light.extend(spot.shade).to_array());
            // A soft edge of 0 would divide by zero in `smoothstep`.
            let shape = match spot.shape {
                SpotShape::Round => [1.0, 0.01, 0.01, 0.0],
                SpotShape::Soft(edge) => [0.0, edge.x.max(0.01), edge.y.max(0.01), 0.0],
            };
            shapes[i * 4..][..4].copy_from_slice(&shape);
        }
        if !spots.is_empty() {
            self.queue.write_buffer(&self.light_map_spots, 0, bytemuck::cast_slice(&data));
        }
        let mut encoder = self.device.create_command_encoder(&Default::default());
        {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("drone course light map"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &light_map.target,
                    depth_slice: None,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        // No light added, and all of the floor's own.
                        load: wgpu::LoadOp::Clear(wgpu::Color { r: 0.0, g: 0.0, b: 0.0, a: 1.0 }),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                ..Default::default()
            });
            if !spots.is_empty() {
                pass.set_pipeline(&self.light_map_pipeline);
                pass.set_bind_group(0, &self.light_map_bind_group, &[]);
                pass.draw(0..4, 0..spots.len() as u32);
            }
        }
        self.queue.submit([encoder.finish()]);
    }

    fn pipeline(&mut self, key: PipelineKey) -> &wgpu::RenderPipeline {
        self.pipelines.entry(key).or_insert_with(|| {
            let (vertex, fragment) = match (key.high, key.lit, key.light_map) {
                (false, true, true) => ("vs_lit_low_light", "fs_low_light"),
                (false, false, _) => ("vs_unlit_low", "fs_low"),
                (false, true, false) => ("vs_lit_low", "fs_low"),
                (true, false, _) => ("vs_high", "fs_unlit_high"),
                (true, true, _) => ("vs_high", "fs_lit_high"),
            };
            let blend = match key.blend {
                Blend::Opaque => None,
                Blend::Alpha => Some(wgpu::BlendState::ALPHA_BLENDING),
                Blend::Additive => Some(wgpu::BlendState {
                    color: wgpu::BlendComponent {
                        src_factor: wgpu::BlendFactor::SrcAlpha,
                        dst_factor: wgpu::BlendFactor::One,
                        operation: wgpu::BlendOperation::Add,
                    },
                    alpha: wgpu::BlendComponent {
                        src_factor: wgpu::BlendFactor::Zero,
                        dst_factor: wgpu::BlendFactor::One,
                        operation: wgpu::BlendOperation::Add,
                    },
                }),
            };
            self.device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                label: Some("drone course pipeline"),
                layout: Some(&self.pipeline_layouts[key.light_map as usize]),
                vertex: wgpu::VertexState {
                    module: &self.shader,
                    entry_point: Some(vertex),
                    compilation_options: Default::default(),
                    buffers: &[
                        Some(wgpu::VertexBufferLayout {
                            array_stride: (VERTEX_FLOATS * 4) as u64,
                            step_mode: wgpu::VertexStepMode::Vertex,
                            attributes: &wgpu::vertex_attr_array![
                                0 => Float32x3, 1 => Float32x3, 2 => Float32x2, 3 => Float32x3
                            ],
                        }),
                        Some(wgpu::VertexBufferLayout {
                            array_stride: (INSTANCE_FLOATS * 4) as u64,
                            step_mode: wgpu::VertexStepMode::Instance,
                            attributes: &wgpu::vertex_attr_array![
                                4 => Float32x4, 5 => Float32x4, 6 => Float32x4, 7 => Float32x4,
                                8 => Float32x4
                            ],
                        }),
                    ],
                },
                fragment: Some(wgpu::FragmentState {
                    module: &self.shader,
                    entry_point: Some(fragment),
                    compilation_options: Default::default(),
                    targets: &[Some(wgpu::ColorTargetState {
                        format: self.color_format,
                        blend,
                        write_mask: wgpu::ColorWrites::ALL,
                    })],
                }),
                primitive: wgpu::PrimitiveState {
                    cull_mode: (!key.double_sided).then_some(wgpu::Face::Back),
                    ..Default::default()
                },
                depth_stencil: Some(wgpu::DepthStencilState {
                    format: self.depth_format,
                    depth_write_enabled: Some(key.depth_write),
                    depth_compare: Some(wgpu::CompareFunction::LessEqual),
                    stencil: Default::default(),
                    bias: Default::default(),
                }),
                multisample: wgpu::MultisampleState {
                    count: if key.multisample { SAMPLES } else { 1 },
                    ..Default::default()
                },
                multiview_mask: None,
                cache: None,
            })
        })
    }

    /// Draws `scene` from `camera` into `target`, a view of `size` in the color format.
    /// High quality lights each pixel, adds the spot lights, tone-maps, and multisamples.
    pub fn render(
        &mut self,
        scene: &Scene,
        camera: &Camera,
        target: &wgpu::TextureView,
        size: (u32, u32),
        high: bool,
    ) {
        let multisample = high;
        let forward = camera.rotation * Vec3::NEG_Z;
        let view = Mat4::from_rotation_translation(camera.rotation, camera.translation).inverse();
        // As `Mat4::perspective_rh`, which maps depth to 0..1.
        let focal = 1.0 / (camera.fov_degrees.to_radians() / 2.0).tan();
        let depth_scale = camera.far / (camera.near - camera.far);
        let projection = [focal / camera.aspect, focal, depth_scale, depth_scale * camera.near];

        let mut cache = vec![None; scene.objects.len()];
        let mut opaque = Vec::new();
        let mut transparent = Vec::new();
        for index in 0..scene.objects.len() {
            let Some(object) = &scene.objects[index] else { continue };
            let Some(mesh) = &object.mesh else { continue };
            let Some(world) = scene.world_transform(index, &mut cache) else { continue };
            // Instances sort by their mean position.
            let center = match &object.instances {
                None => world.w_axis.truncate(),
                Some(instances) if instances.is_empty() => continue,
                Some(instances) => {
                    let sum: Vec3 = instances.iter().map(|i| i.transform.w_axis.truncate()).sum();
                    world.transform_point3(sum / instances.len() as f32)
                }
            };
            let depth = (center - camera.translation).dot(forward);
            let draw = (object, mesh, world, depth);
            if object.material.blend == Blend::Opaque {
                opaque.push(draw);
            } else {
                transparent.push(draw);
            }
        }
        // Front to back, so the GPU skips hidden pixels; transparent ones back to front.
        opaque.sort_by(|a, b| a.3.total_cmp(&b.3));
        transparent
            .sort_by(|a, b| a.0.render_order.cmp(&b.0.render_order).then(b.3.total_cmp(&a.3)));
        let draws: Vec<_> = opaque.into_iter().chain(transparent).collect();

        let lights = &scene.lights;
        let mut globals = Vec::with_capacity(GLOBAL_FLOATS);
        globals.extend(projection);
        globals.extend(camera.translation.extend(1.0).to_array());
        let hemisphere = lights.hemisphere_intensity / PI;
        globals.extend((lights.sky * hemisphere).extend(0.0).to_array());
        globals.extend((lights.ground * hemisphere).extend(0.0).to_array());
        globals.extend(lights.sun_direction.normalize().extend(0.0).to_array());
        globals.extend((lights.sun_color * lights.sun_intensity / PI).extend(0.0).to_array());
        for i in 0..MAX_SPOTS {
            match lights.spots.get(i) {
                Some(spot) => {
                    let direction = (spot.position - spot.target).normalize();
                    globals.extend(spot.position.extend(spot.distance).to_array());
                    globals.extend(direction.extend(spot.angle.cos()).to_array());
                    let color = spot.color * spot.intensity / PI;
                    let penumbra = (spot.angle * (1.0 - spot.penumbra)).cos();
                    globals.extend(color.extend(penumbra).to_array());
                }
                None => {
                    globals.extend([0.0, 0.0, 0.0, 1.0, 0.0, 1.0, 0.0, 1.0, 0.0, 0.0, 0.0, 1.0])
                }
            }
        }
        self.queue.write_buffer(&self.globals, 0, bytemuck::cast_slice(&globals));

        let stride_floats = self.object_stride / 4;
        let mut objects = vec![0f32; draws.len().max(1) * stride_floats];
        let mut instances = Vec::new();
        instances.extend(Mat4::IDENTITY.to_cols_array());
        instances.extend([1.0; 4]);
        // Each draw's first instance and their count.
        let mut ranges = Vec::with_capacity(draws.len());
        // Instances are placed relative to the camera, which keeps the vertex shader's
        // numbers small near it, see `geometry::boxes`.
        let at_camera = Mat4::from_translation(camera.translation);
        let from_camera = Mat4::from_translation(-camera.translation);
        for (i, (object, _, world, _)) in draws.iter().enumerate() {
            let material = &object.material;
            let (roughness, metalness) = material.lit.unwrap_or((1.0, 0.0));
            let model = match &object.instances {
                Some(list) => {
                    ranges.push((instances.len() / INSTANCE_FLOATS, list.len() as u32));
                    for instance in list {
                        instances
                            .extend((from_camera * *world * instance.transform).to_cols_array());
                        instances.extend(instance.color.to_array());
                    }
                    at_camera
                }
                None => {
                    ranges.push((0, 1));
                    *world
                }
            };
            let uniforms = &mut objects[i * stride_floats..i * stride_floats + OBJECT_FLOATS];
            uniforms[..16].copy_from_slice(&model.to_cols_array());
            uniforms[16..32].copy_from_slice(&(view * model).to_cols_array());
            uniforms[32..36].copy_from_slice(&material.color.extend(material.opacity).to_array());
            uniforms[36..40].copy_from_slice(&[
                material.uv_scale.x,
                material.uv_scale.y,
                material.uv_offset.x,
                material.uv_offset.y,
            ]);
            uniforms[40..44].copy_from_slice(&[roughness, metalness, 0.0, 0.0]);
        }
        let needed = (objects.len() * 4) as u64;
        if needed > self.objects.size() {
            let capacity = draws.len().next_power_of_two();
            (self.objects, self.objects_bind_group) = Self::object_buffer(
                &self.device,
                &self.object_layout,
                self.object_stride,
                capacity,
            );
        }
        self.queue.write_buffer(&self.objects, 0, bytemuck::cast_slice(&objects));
        if (instances.len() * 4) as u64 > self.instances.size() {
            let capacity = (instances.len() / INSTANCE_FLOATS).next_power_of_two();
            self.instances = Self::instance_buffer(&self.device, capacity);
        }
        self.queue.write_buffer(&self.instances, 0, bytemuck::cast_slice(&instances));

        let mut keys: Vec<PipelineKey> = Vec::with_capacity(draws.len());
        for (object, ..) in &draws {
            let mut key = PipelineKey {
                lit: object.material.lit.is_some(),
                high,
                blend: object.material.blend,
                double_sided: object.material.double_sided,
                depth_write: object.material.depth_write,
                light_map: object.material.lit.is_some()
                    && (high || object.material.light_map.is_some()),
                multisample,
                copy: false,
            };
            if let Some(previous) = keys.last() {
                key.copy =
                    PipelineKey { copy: previous.copy, ..key } == *previous && !previous.copy;
            }
            keys.push(key);
        }
        for key in &keys {
            self.pipeline(*key);
        }

        let samples = if multisample { SAMPLES } else { 1 };
        let depth = match &self.depth {
            Some(depth)
                if (depth.width(), depth.height()) == size && depth.sample_count() == samples =>
            {
                depth.clone()
            }
            _ => {
                let depth = self.device.create_texture(&wgpu::TextureDescriptor {
                    label: Some("drone course depth"),
                    size: wgpu::Extent3d {
                        width: size.0,
                        height: size.1,
                        depth_or_array_layers: 1,
                    },
                    mip_level_count: 1,
                    sample_count: samples,
                    dimension: wgpu::TextureDimension::D2,
                    format: self.depth_format,
                    usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
                    view_formats: &[],
                });
                self.depth = Some(depth.clone());
                depth
            }
        };
        let depth_view = depth.create_view(&Default::default());
        let multisampled = multisample.then(|| match &self.multisampled {
            Some(texture) if (texture.width(), texture.height()) == size => texture.clone(),
            _ => {
                let texture = self.device.create_texture(&wgpu::TextureDescriptor {
                    label: Some("drone course multisampled"),
                    size: wgpu::Extent3d {
                        width: size.0,
                        height: size.1,
                        depth_or_array_layers: 1,
                    },
                    mip_level_count: 1,
                    sample_count: SAMPLES,
                    dimension: wgpu::TextureDimension::D2,
                    format: self.color_format,
                    usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
                    view_formats: &[],
                });
                self.multisampled = Some(texture.clone());
                texture
            }
        });
        let multisampled_view =
            multisampled.map(|texture| texture.create_view(&Default::default()));

        let mut encoder = self.device.create_command_encoder(&Default::default());
        {
            let background = scene.background;
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("drone course scene"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: multisampled_view.as_ref().unwrap_or(target),
                    depth_slice: None,
                    resolve_target: multisampled_view.as_ref().map(|_| target),
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color {
                            r: background.x as f64,
                            g: background.y as f64,
                            b: background.z as f64,
                            a: 1.0,
                        }),
                        store: if multisample {
                            wgpu::StoreOp::Discard
                        } else {
                            wgpu::StoreOp::Store
                        },
                    },
                })],
                depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                    view: &depth_view,
                    depth_ops: Some(wgpu::Operations {
                        load: wgpu::LoadOp::Clear(1.0),
                        store: wgpu::StoreOp::Discard,
                    }),
                    stencil_ops: None,
                }),
                ..Default::default()
            });
            pass.set_bind_group(0, &self.globals_bind_group, &[]);
            let mut current_key = None;
            for (i, (((object, mesh, ..), key), &(first, count))) in
                draws.iter().zip(&keys).zip(&ranges).enumerate()
            {
                if current_key != Some(*key) {
                    pass.set_pipeline(&self.pipelines[key]);
                    current_key = Some(*key);
                }
                let offset = (i * self.object_stride) as u32;
                pass.set_bind_group(1, &self.objects_bind_group, &[offset]);
                let texture = object.material.texture.as_ref().unwrap_or(&self.white);
                pass.set_bind_group(2, &texture.bind_groups[high as usize], &[]);
                if key.light_map {
                    let light_map = object.material.light_map.as_ref().unwrap_or(&self.no_light);
                    pass.set_bind_group(3, &light_map.bind_groups[0], &[]);
                }
                pass.set_vertex_buffer(0, mesh.vertices.slice(..));
                let first = (first * INSTANCE_FLOATS * 4) as u64;
                pass.set_vertex_buffer(1, self.instances.slice(first..));
                pass.set_index_buffer(mesh.indices.slice(..), wgpu::IndexFormat::Uint32);
                pass.draw_indexed(0..mesh.index_count, 0, 0..count);
            }
        }
        self.queue.submit([encoder.finish()]);
    }
}

fn upload_texture(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    layout: &wgpu::BindGroupLayout,
    samplers: &[[wgpu::Sampler; 2]; 2],
    data: &TextureData,
) -> Rc<Texture> {
    let size = wgpu::Extent3d { width: data.width, height: data.height, depth_or_array_layers: 1 };
    let texture = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("drone course texture"),
        size,
        mip_level_count: data.levels.len() as u32,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: wgpu::TextureFormat::Rgba8UnormSrgb,
        usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
        view_formats: &[],
    });
    for (level, texels) in data.levels.iter().enumerate() {
        let size = size.mip_level_size(level as u32, wgpu::TextureDimension::D2);
        queue.write_texture(
            wgpu::TexelCopyTextureInfo {
                texture: &texture,
                mip_level: level as u32,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            texels,
            wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(size.width * 4),
                rows_per_image: None,
            },
            size,
        );
    }
    Rc::new(Texture {
        bind_groups: texture_bind_groups(device, layout, samplers, &texture, data.repeat),
    })
}

fn texture_bind_groups(
    device: &wgpu::Device,
    layout: &wgpu::BindGroupLayout,
    samplers: &[[wgpu::Sampler; 2]; 2],
    texture: &wgpu::Texture,
    repeat: bool,
) -> [wgpu::BindGroup; 2] {
    let view = texture.create_view(&Default::default());
    [0, 1].map(|anisotropic| {
        device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: None,
            layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(&view),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Sampler(
                        &samplers[repeat as usize][anisotropic],
                    ),
                },
            ],
        })
    })
}
