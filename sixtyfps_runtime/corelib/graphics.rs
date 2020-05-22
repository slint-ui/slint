extern crate alloc;
use crate::abi::datastructures::{Color, RenderingPrimitive};
use cgmath::Matrix4;

pub enum FillStyle {
    SolidColor(Color),
}

pub trait HasRenderingPrimitive {
    fn primitive(&self) -> &RenderingPrimitive;
}

pub trait Frame {
    type LowLevelRenderingPrimitive: HasRenderingPrimitive;
    fn render_primitive(
        &mut self,
        primitive: &Self::LowLevelRenderingPrimitive,
        transform: &Matrix4<f32>,
    );
}

pub trait RenderingPrimitivesBuilder {
    type LowLevelRenderingPrimitive: HasRenderingPrimitive;

    fn create(&mut self, primitive: RenderingPrimitive) -> Self::LowLevelRenderingPrimitive;
}

pub trait GraphicsBackend: Sized {
    type LowLevelRenderingPrimitive: HasRenderingPrimitive;
    type Frame: Frame<LowLevelRenderingPrimitive = Self::LowLevelRenderingPrimitive>;
    type RenderingPrimitivesBuilder: RenderingPrimitivesBuilder<
        LowLevelRenderingPrimitive = Self::LowLevelRenderingPrimitive,
    >;

    fn new_rendering_primitives_builder(&mut self) -> Self::RenderingPrimitivesBuilder;
    fn finish_primitives(&mut self, builder: Self::RenderingPrimitivesBuilder);

    fn new_frame(&mut self, width: u32, height: u32, clear_color: &Color) -> Self::Frame;
    fn present_frame(&mut self, frame: Self::Frame);

    fn window(&self) -> &winit::window::Window;
}

enum RenderingCacheEntry<RenderingPrimitive> {
    AllocateEntry(RenderingPrimitive),
    FreeEntry(Option<usize>), // contains next free index if exists
}

pub struct RenderingCache<Backend: GraphicsBackend> {
    nodes: Vec<RenderingCacheEntry<Backend::LowLevelRenderingPrimitive>>,
    next_free: Option<usize>,
    len: usize,
}

impl<Backend: GraphicsBackend> Default for RenderingCache<Backend> {
    fn default() -> Self {
        Self { nodes: vec![], next_free: None, len: 0 }
    }
}

impl<Backend: GraphicsBackend> RenderingCache<Backend> {
    pub fn allocate_entry(&mut self, content: Backend::LowLevelRenderingPrimitive) -> usize {
        let idx = {
            if let Some(free_idx) = self.next_free {
                let node = &mut self.nodes[free_idx];
                if let RenderingCacheEntry::FreeEntry(next_free) = node {
                    self.next_free = *next_free;
                } else {
                    unreachable!();
                }
                *node = RenderingCacheEntry::AllocateEntry(content);
                free_idx
            } else {
                self.nodes.push(RenderingCacheEntry::AllocateEntry(content));
                self.nodes.len() - 1
            }
        };
        self.len = self.len + 1;
        idx
    }

    pub fn entry_at(&self, idx: usize) -> &Backend::LowLevelRenderingPrimitive {
        match self.nodes[idx] {
            RenderingCacheEntry::AllocateEntry(ref data) => return data,
            _ => unreachable!(),
        }
    }

    pub fn set_entry_at(&mut self, idx: usize, primitive: Backend::LowLevelRenderingPrimitive) {
        match self.nodes[idx] {
            RenderingCacheEntry::AllocateEntry(ref mut data) => *data = primitive,
            _ => unreachable!(),
        }
    }

    pub fn free_entry(&mut self, idx: usize) {
        self.len = self.len - 1;
        self.nodes[idx] = RenderingCacheEntry::FreeEntry(self.next_free);
        self.next_free = Some(idx);
    }

    pub fn len(&self) -> usize {
        self.len
    }
}
