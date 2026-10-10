// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT

// cSpell: ignore metalness multisampled multisampling texel untextured

//! A small forward renderer for the race track, on Slint's wgpu device.
//!
//! Each frame is one render pass and one queue submission: on small embedded GPUs, such as
//! the i.MX 8M Plus's, the driver makes every submission expensive.

use std::collections::HashMap;
use std::f32::consts::PI;
use std::rc::Rc;

use glam::{Mat4, Quat, Vec2, Vec3, Vec4};
use slint::wgpu_30::wgpu;
use wgpu::util::DeviceExt;

use crate::Quality;

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
    /// A glossy clear coat over the surface, from 0 to 1, as on car paint. Only high
    /// quality draws it.
    pub clear_coat: f32,
    /// How much the distance haze covers the surface, from 0 to 1. Only high quality draws
    /// it.
    pub haze: f32,
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
            clear_coat: 0.0,
            haze: 1.0,
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
    /// Objects with a higher order draw later, whatever their distance, among the opaque
    /// and among the transparent objects. Objects with a negative order are layers under
    /// everything else, and draw first, whatever their blend.
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
    /// The sky's color at the horizon and overhead, which glossy surfaces reflect and the
    /// haze takes, in high quality.
    pub horizon: Vec3,
    pub zenith: Vec3,
    /// How much of the view the haze covers per meter.
    pub haze_density: f32,
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
    /// Medium and high quality's shaders.
    per_pixel: bool,
    bloom: bool,
    blend: Blend,
    double_sided: bool,
    depth_write: bool,
    light_map: bool,
    textured: bool,
    /// Selects one of two identical pipelines. NXP's Vivante driver loses parts of a draw
    /// where the next draw overlaps it with the same pipeline, blended or not, so
    /// consecutive draws alternate between the two.
    copy: bool,
}

/// The floats of `Globals` in `shader.wgsl`.
const GLOBAL_FLOATS: usize = 8 * 4 + 16 + MAX_SPOTS * 12;

/// Half the size of the square around the camera that the sun's shadow map covers, in
/// meters.
const SHADOW_REACH: f32 = 80.0;
const SHADOW_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Depth32Float;

/// High quality draws the scene in this format, so lights can be brighter than white for
/// the bloom, and tone-maps it afterwards.
const HDR_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rgba16Float;
/// The bloom's levels, from half the view's size down.
const BLOOM_LEVELS: usize = 6;

/// The size of the sun's shadow map in texels.
fn shadow_size(quality: Quality) -> u32 {
    if quality == Quality::High { 2048 } else { 1024 }
}

/// The sun's shadow map, and the globals' bind group that has it.
struct Shadow {
    size: u32,
    map: wgpu::TextureView,
    globals_bind_group: wgpu::BindGroup,
}

impl Shadow {
    fn new(
        device: &wgpu::Device,
        layout: &wgpu::BindGroupLayout,
        globals: &wgpu::Buffer,
        sampler: &wgpu::Sampler,
        size: u32,
    ) -> Self {
        let map = device
            .create_texture(&wgpu::TextureDescriptor {
                label: Some("racer shadow map"),
                size: wgpu::Extent3d { width: size, height: size, depth_or_array_layers: 1 },
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format: SHADOW_FORMAT,
                usage: wgpu::TextureUsages::RENDER_ATTACHMENT
                    | wgpu::TextureUsages::TEXTURE_BINDING,
                view_formats: &[],
            })
            .create_view(&Default::default());
        let globals_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: None,
            layout,
            entries: &[
                wgpu::BindGroupEntry { binding: 0, resource: globals.as_entire_binding() },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::TextureView(&map),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: wgpu::BindingResource::Sampler(sampler),
                },
            ],
        });
        Self { size, map, globals_bind_group }
    }
}

/// High quality's bloom, see `post.wgsl`: the scene drawn in `HDR_FORMAT`, and the levels
/// that blur its bright parts.
struct Bloom {
    layout: wgpu::BindGroupLayout,
    sampler: wgpu::Sampler,
    bright: wgpu::RenderPipeline,
    down: wgpu::RenderPipeline,
    up: wgpu::RenderPipeline,
    composite: wgpu::RenderPipeline,
    /// The view's size the targets below have.
    size: (u32, u32),
    scene: Option<wgpu::TextureView>,
    levels: Vec<wgpu::TextureView>,
    /// What each pass reads, with the scene: the scene itself, and each level.
    scene_group: Option<wgpu::BindGroup>,
    level_groups: Vec<wgpu::BindGroup>,
}

impl Bloom {
    fn new(device: &wgpu::Device, color_format: wgpu::TextureFormat) -> Self {
        let texture = |binding| wgpu::BindGroupLayoutEntry {
            binding,
            visibility: wgpu::ShaderStages::FRAGMENT,
            ty: wgpu::BindingType::Texture {
                sample_type: wgpu::TextureSampleType::Float { filterable: true },
                view_dimension: wgpu::TextureViewDimension::D2,
                multisampled: false,
            },
            count: None,
        };
        let layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("racer bloom"),
            entries: &[
                texture(0),
                texture(1),
                wgpu::BindGroupLayoutEntry {
                    binding: 2,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                    count: None,
                },
            ],
        });
        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("racer bloom"),
            bind_group_layouts: &[Some(&layout)],
            immediate_size: 0,
        });
        let module = device.create_shader_module(wgpu::include_wgsl!("post.wgsl"));
        let pipeline = |entry: &str, format, blend| {
            device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                label: Some("racer bloom"),
                layout: Some(&pipeline_layout),
                vertex: wgpu::VertexState {
                    module: &module,
                    entry_point: Some("vs_full"),
                    compilation_options: Default::default(),
                    buffers: &[],
                },
                fragment: Some(wgpu::FragmentState {
                    module: &module,
                    entry_point: Some(entry),
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
        let add = wgpu::BlendComponent {
            src_factor: wgpu::BlendFactor::One,
            dst_factor: wgpu::BlendFactor::One,
            operation: wgpu::BlendOperation::Add,
        };
        Self {
            sampler: device.create_sampler(&wgpu::SamplerDescriptor {
                mag_filter: wgpu::FilterMode::Linear,
                min_filter: wgpu::FilterMode::Linear,
                ..Default::default()
            }),
            bright: pipeline("fs_bright", HDR_FORMAT, None),
            down: pipeline("fs_down", HDR_FORMAT, None),
            up: pipeline("fs_up", HDR_FORMAT, Some(wgpu::BlendState { color: add, alpha: add })),
            composite: pipeline("fs_composite", color_format, None),
            layout,
            size: (0, 0),
            scene: None,
            levels: Vec::new(),
            scene_group: None,
            level_groups: Vec::new(),
        }
    }

    /// The scene's target for a view of `size`, made anew when the size changes.
    fn scene(&mut self, device: &wgpu::Device, size: (u32, u32)) -> wgpu::TextureView {
        if self.size != size || self.scene.is_none() {
            self.size = size;
            let target = |width: u32, height: u32| {
                device
                    .create_texture(&wgpu::TextureDescriptor {
                        label: Some("racer bloom"),
                        size: wgpu::Extent3d { width, height, depth_or_array_layers: 1 },
                        mip_level_count: 1,
                        sample_count: 1,
                        dimension: wgpu::TextureDimension::D2,
                        format: HDR_FORMAT,
                        usage: wgpu::TextureUsages::RENDER_ATTACHMENT
                            | wgpu::TextureUsages::TEXTURE_BINDING,
                        view_formats: &[],
                    })
                    .create_view(&Default::default())
            };
            let scene = target(size.0, size.1);
            self.levels = (1..=BLOOM_LEVELS)
                .map(|level| target((size.0 >> level).max(1), (size.1 >> level).max(1)))
                .collect();
            let group = |source: &wgpu::TextureView, scene: &wgpu::TextureView| {
                device.create_bind_group(&wgpu::BindGroupDescriptor {
                    label: Some("racer bloom"),
                    layout: &self.layout,
                    entries: &[
                        wgpu::BindGroupEntry {
                            binding: 0,
                            resource: wgpu::BindingResource::TextureView(source),
                        },
                        wgpu::BindGroupEntry {
                            binding: 1,
                            resource: wgpu::BindingResource::TextureView(scene),
                        },
                        wgpu::BindGroupEntry {
                            binding: 2,
                            resource: wgpu::BindingResource::Sampler(&self.sampler),
                        },
                    ],
                })
            };
            self.scene_group = Some(group(&scene, &scene));
            self.level_groups = self.levels.iter().map(|level| group(level, &scene)).collect();
            self.scene = Some(scene);
        }
        self.scene.clone().expect("the scene's target was just made")
    }

    /// Blurs the scene's bright parts and draws the scene with them into `target`.
    fn draw(&self, encoder: &mut wgpu::CommandEncoder, target: &wgpu::TextureView) {
        let pass = |encoder: &mut wgpu::CommandEncoder,
                    view: &wgpu::TextureView,
                    load: wgpu::LoadOp<wgpu::Color>,
                    pipeline: &wgpu::RenderPipeline,
                    group: &wgpu::BindGroup| {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("racer bloom"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view,
                    depth_slice: None,
                    resolve_target: None,
                    ops: wgpu::Operations { load, store: wgpu::StoreOp::Store },
                })],
                ..Default::default()
            });
            pass.set_pipeline(pipeline);
            pass.set_bind_group(0, group, &[]);
            pass.draw(0..3, 0..1);
        };
        let clear = wgpu::LoadOp::Clear(wgpu::Color::BLACK);
        let Some(scene_group) = &self.scene_group else { return };
        pass(encoder, &self.levels[0], clear, &self.bright, scene_group);
        for level in 1..BLOOM_LEVELS {
            pass(encoder, &self.levels[level], clear, &self.down, &self.level_groups[level - 1]);
        }
        for level in (1..BLOOM_LEVELS).rev() {
            let (view, group) = (&self.levels[level - 1], &self.level_groups[level]);
            pass(encoder, view, wgpu::LoadOp::Load, &self.up, group);
        }
        pass(encoder, target, clear, &self.composite, &self.level_groups[0]);
    }
}
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
    globals_layout: wgpu::BindGroupLayout,
    shadow: Shadow,
    shadow_sampler: wgpu::Sampler,
    /// Only the globals, for drawing the shadow map.
    shadow_globals_bind_group: wgpu::BindGroup,
    shadow_pipeline: wgpu::RenderPipeline,
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
    /// Made when high quality draws its first frame.
    bloom: Option<Bloom>,
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
        let shadow_globals_layout = uniform_layout(false);
        let globals_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: None,
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
                        sample_type: wgpu::TextureSampleType::Depth,
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 2,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Comparison),
                    count: None,
                },
            ],
        });
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
            label: Some("racer globals"),
            size: (GLOBAL_FLOATS * 4) as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let shadow_sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            compare: Some(wgpu::CompareFunction::LessEqual),
            ..Default::default()
        });
        let shadow = Shadow::new(
            device,
            &globals_layout,
            &globals,
            &shadow_sampler,
            shadow_size(Quality::High),
        );
        let shadow_globals_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: None,
            layout: &shadow_globals_layout,
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
        let shadow_pipeline =
            Self::shadow_pipeline(device, &shader, &shadow_globals_layout, &object_layout);
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
            globals_layout,
            shadow,
            shadow_sampler,
            shadow_globals_bind_group,
            shadow_pipeline,
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
            bloom: None,
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
            label: Some("racer light map"),
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
            label: Some("racer light map spots"),
            size: (MAX_SPOTS_IN_LIGHT_MAP * 16 * 4) as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("racer light map"),
            layout: &layout,
            entries: &[wgpu::BindGroupEntry { binding: 0, resource: spots.as_entire_binding() }],
        });
        let shader = device.create_shader_module(wgpu::include_wgsl!("light_map.wgsl"));
        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("racer light map"),
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
            label: Some("racer light map"),
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

    /// The pipeline that draws the sun's shadow map, with only the depth of the shapes.
    fn shadow_pipeline(
        device: &wgpu::Device,
        shader: &wgpu::ShaderModule,
        globals_layout: &wgpu::BindGroupLayout,
        object_layout: &wgpu::BindGroupLayout,
    ) -> wgpu::RenderPipeline {
        let layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("racer shadow"),
            bind_group_layouts: &[Some(globals_layout), Some(object_layout)],
            immediate_size: 0,
        });
        device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("racer shadow"),
            layout: Some(&layout),
            vertex: wgpu::VertexState {
                module: shader,
                entry_point: Some("vs_shadow"),
                compilation_options: Default::default(),
                buffers: &[
                    Some(wgpu::VertexBufferLayout {
                        array_stride: (VERTEX_FLOATS * 4) as u64,
                        step_mode: wgpu::VertexStepMode::Vertex,
                        attributes: &wgpu::vertex_attr_array![0 => Float32x3],
                    }),
                    Some(wgpu::VertexBufferLayout {
                        array_stride: (INSTANCE_FLOATS * 4) as u64,
                        step_mode: wgpu::VertexStepMode::Instance,
                        attributes: &wgpu::vertex_attr_array![
                            4 => Float32x4, 5 => Float32x4, 6 => Float32x4, 7 => Float32x4
                        ],
                    }),
                ],
            },
            fragment: None,
            // Both sides cast shadows, so thin and one-sided surfaces do too.
            primitive: Default::default(),
            depth_stencil: Some(wgpu::DepthStencilState {
                format: SHADOW_FORMAT,
                depth_write_enabled: Some(true),
                depth_compare: Some(wgpu::CompareFunction::LessEqual),
                stencil: Default::default(),
                // The low sun meets most surfaces at a flat angle, where they'd shadow
                // themselves without a bias that grows with the slope.
                bias: wgpu::DepthBiasState { constant: 2, slope_scale: 2.5, clamp: 0.0 },
            }),
            multisample: Default::default(),
            multiview_mask: None,
            cache: None,
        })
    }

    /// The sun's view of the square of `SHADOW_REACH` around a point ahead of `camera`,
    /// snapped to whole texels so the shadows' edges don't crawl as the camera moves.
    fn shadow_matrix(sun: Vec3, camera: &Camera, size: u32) -> Mat4 {
        let forward = (camera.rotation * Vec3::NEG_Z).with_y(0.0).normalize_or_zero();
        let focus = camera.translation + forward * SHADOW_REACH * 0.6;
        let base = Mat4::look_at_rh(sun, Vec3::ZERO, Vec3::Y);
        let p = base.transform_point3(focus);
        let texel = 2.0 * SHADOW_REACH / size as f32;
        let (x, y) = ((p.x / texel).round() * texel, (p.y / texel).round() * texel);
        let view = Mat4::from_translation(Vec3::new(-x, -y, 0.0)) * base;
        let (r, depth) = (SHADOW_REACH, -p.z);
        Mat4::orthographic_rh(-r, r, -r, r, depth - 350.0, depth + 350.0) * view
    }

    fn instance_buffer(device: &wgpu::Device, capacity: usize) -> wgpu::Buffer {
        device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("racer instances"),
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
            label: Some("racer objects"),
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
                "racer vertices",
                bytemuck::cast_slice(&geometry.vertex_data()),
                wgpu::BufferUsages::VERTEX,
            ),
            indices: create(
                "racer indices",
                bytemuck::cast_slice(&geometry.indices),
                wgpu::BufferUsages::INDEX,
            ),
            index_count: geometry.indices.len() as u32,
        })
    }

    pub fn create_texture(&self, data: &TextureData) -> Rc<Texture> {
        upload_texture(&self.device, &self.queue, &self.texture_layout, &self.samplers, data)
    }

    /// A light map for a floor of `size` meters, without spots.
    pub fn create_light_map(&self, size: Vec2, texels_per_meter: f32) -> LightMap {
        let texture = self.device.create_texture(&wgpu::TextureDescriptor {
            label: Some("racer light map"),
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
                label: Some("racer light map"),
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
            let low_fragment = if key.textured { "fs_low" } else { "fs_low_untextured" };
            let (vertex, fragment) = match (key.per_pixel, key.lit, key.light_map) {
                (false, true, true) => ("vs_lit_low_light", "fs_low_light"),
                (false, false, _) => ("vs_unlit_low", low_fragment),
                (false, true, false) => ("vs_lit_low", low_fragment),
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
                label: Some("racer pipeline"),
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
                    compilation_options: wgpu::PipelineCompilationOptions {
                        constants: &[("medium", (key.per_pixel && !key.bloom) as u8 as f64)],
                        ..Default::default()
                    },
                    targets: &[Some(wgpu::ColorTargetState {
                        format: if key.bloom { HDR_FORMAT } else { self.color_format },
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
                    count: if key.per_pixel { SAMPLES } else { 1 },
                    ..Default::default()
                },
                multiview_mask: None,
                cache: None,
            })
        })
    }

    /// Draws `scene` from `camera` into `target`, a view of `size` in the color format.
    /// Medium and high quality light each pixel, add the spot lights and the sun's shadows,
    /// and multisample. High quality's shadows are sharper, and it adds bloom before it
    /// tone-maps.
    pub fn render(
        &mut self,
        scene: &Scene,
        camera: &Camera,
        target: &wgpu::TextureView,
        size: (u32, u32),
        quality: Quality,
    ) {
        let high = quality != Quality::Low;
        let bloom = quality == Quality::High;
        if high && self.shadow.size != shadow_size(quality) {
            self.shadow = Shadow::new(
                &self.device,
                &self.globals_layout,
                &self.globals,
                &self.shadow_sampler,
                shadow_size(quality),
            );
        }
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
            // wgpu can't bind the empty buffers of a mesh without triangles.
            if mesh.index_count == 0 {
                continue;
            }
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
            if object.material.blend == Blend::Opaque || object.render_order < 0 {
                opaque.push(draw);
            } else {
                transparent.push(draw);
            }
        }
        // Front to back, so the GPU skips hidden pixels; transparent ones back to front.
        opaque.sort_by(|a, b| a.0.render_order.cmp(&b.0.render_order).then(a.3.total_cmp(&b.3)));
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
        globals.extend(lights.horizon.extend(lights.haze_density).to_array());
        // The shaders take the sun's shadows from the shadow map while `zenith.w` is 1.
        globals.extend(lights.zenith.extend(if high { 1.0 } else { 0.0 }).to_array());
        let shadow =
            Self::shadow_matrix(lights.sun_direction.normalize(), camera, self.shadow.size);
        globals.extend(shadow.to_cols_array());
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
        // numbers small near it.
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
            // Glows that add to what's behind them fade into the haze rather than taking its
            // color, which the shaders tell from the negative amount.
            let haze =
                if material.blend == Blend::Additive { -material.haze } else { material.haze };
            uniforms[40..44].copy_from_slice(&[roughness, metalness, material.clear_coat, haze]);
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
                per_pixel: high,
                bloom,
                blend: object.material.blend,
                double_sided: object.material.double_sided,
                depth_write: object.material.depth_write,
                light_map: object.material.lit.is_some()
                    && (high || object.material.light_map.is_some()),
                textured: object.material.texture.is_some(),
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

        let samples = if high { SAMPLES } else { 1 };
        let depth = match &self.depth {
            Some(depth)
                if (depth.width(), depth.height()) == size && depth.sample_count() == samples =>
            {
                depth.clone()
            }
            _ => {
                let depth = self.device.create_texture(&wgpu::TextureDescriptor {
                    label: Some("racer depth"),
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
        let scene_format = if bloom { HDR_FORMAT } else { self.color_format };
        let multisampled = high.then(|| match &self.multisampled {
            Some(texture)
                if (texture.width(), texture.height()) == size
                    && texture.format() == scene_format =>
            {
                texture.clone()
            }
            _ => {
                let texture = self.device.create_texture(&wgpu::TextureDescriptor {
                    label: Some("racer multisampled"),
                    size: wgpu::Extent3d {
                        width: size.0,
                        height: size.1,
                        depth_or_array_layers: 1,
                    },
                    mip_level_count: 1,
                    sample_count: SAMPLES,
                    dimension: wgpu::TextureDimension::D2,
                    format: scene_format,
                    usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
                    view_formats: &[],
                });
                self.multisampled = Some(texture.clone());
                texture
            }
        });
        let multisampled_view =
            multisampled.map(|texture| texture.create_view(&Default::default()));
        // High quality draws into the bloom's scene target, and the bloom into `target`.
        let hdr_view = bloom.then(|| {
            let color_format = self.color_format;
            let bloom = self.bloom.get_or_insert_with(|| Bloom::new(&self.device, color_format));
            bloom.scene(&self.device, size)
        });
        let scene_target = hdr_view.as_ref().unwrap_or(target);

        let mut encoder = self.device.create_command_encoder(&Default::default());
        if high {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("racer shadow"),
                color_attachments: &[],
                depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                    view: &self.shadow.map,
                    depth_ops: Some(wgpu::Operations {
                        load: wgpu::LoadOp::Clear(1.0),
                        store: wgpu::StoreOp::Store,
                    }),
                    stencil_ops: None,
                }),
                ..Default::default()
            });
            pass.set_pipeline(&self.shadow_pipeline);
            pass.set_bind_group(0, &self.shadow_globals_bind_group, &[]);
            // The opaque shapes on the ground cast shadows, not the flat layers below them.
            for (i, ((object, mesh, ..), &(first, count))) in draws.iter().zip(&ranges).enumerate()
            {
                if object.material.blend != Blend::Opaque || object.render_order < 0 {
                    continue;
                }
                let offset = (i * self.object_stride) as u32;
                pass.set_bind_group(1, &self.objects_bind_group, &[offset]);
                pass.set_vertex_buffer(0, mesh.vertices.slice(..));
                let first = (first * INSTANCE_FLOATS * 4) as u64;
                pass.set_vertex_buffer(1, self.instances.slice(first..));
                pass.set_index_buffer(mesh.indices.slice(..), wgpu::IndexFormat::Uint32);
                pass.draw_indexed(0..mesh.index_count, 0, 0..count);
            }
        }
        {
            let background = scene.background;
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("racer scene"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: multisampled_view.as_ref().unwrap_or(scene_target),
                    depth_slice: None,
                    resolve_target: multisampled_view.as_ref().map(|_| scene_target),
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color {
                            r: background.x as f64,
                            g: background.y as f64,
                            b: background.z as f64,
                            a: 1.0,
                        }),
                        store: if high { wgpu::StoreOp::Discard } else { wgpu::StoreOp::Store },
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
            pass.set_bind_group(0, &self.shadow.globals_bind_group, &[]);
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
        if let (true, Some(bloom)) = (bloom, &self.bloom) {
            bloom.draw(&mut encoder, target);
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
    let texture = device.create_texture_with_data(
        queue,
        &wgpu::TextureDescriptor {
            label: Some("racer texture"),
            size: wgpu::Extent3d {
                width: data.width,
                height: data.height,
                depth_or_array_layers: 1,
            },
            mip_level_count: data.levels.len() as u32,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8UnormSrgb,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        },
        wgpu::util::TextureDataOrder::LayerMajor,
        &data.levels.concat(),
    );
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
