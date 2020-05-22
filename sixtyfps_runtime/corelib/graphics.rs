extern crate alloc;
use cgmath::Matrix4;

#[derive(Copy, Clone)]
pub struct Color {
    red: u8,
    green: u8,
    blue: u8,
    alpha: u8,
}

impl Color {
    pub const fn from_argb_encoded(encoded: u32) -> Color {
        Color {
            red: (encoded >> 16) as u8,
            green: (encoded >> 8) as u8,
            blue: encoded as u8,
            alpha: (encoded >> 24) as u8,
        }
    }

    pub const fn from_rgba(red: u8, green: u8, blue: u8, alpha: u8) -> Color {
        Color { red, green, blue, alpha }
    }
    pub const fn from_rgb(red: u8, green: u8, blue: u8) -> Color {
        Color::from_rgba(red, green, blue, 0xff)
    }

    pub fn as_rgba_f32(&self) -> (f32, f32, f32, f32) {
        (
            (self.red as f32) / 255.0,
            (self.green as f32) / 255.0,
            (self.blue as f32) / 255.0,
            (self.alpha as f32) / 255.0,
        )
    }

    pub const BLACK: Color = Color::from_rgb(0, 0, 0);
    pub const RED: Color = Color::from_rgb(255, 0, 0);
    pub const GREEN: Color = Color::from_rgb(0, 255, 0);
    pub const BLUE: Color = Color::from_rgb(0, 0, 255);
    pub const WHITE: Color = Color::from_rgb(255, 255, 255);
}

pub enum FillStyle {
    SolidColor(Color),
}

pub enum RenderingPrimitive {
    NoContents,
    Rectangle {
        x: f32,
        y: f32,
        width: f32,
        height: f32,
        color: Color,
    },
    Image {
        x: f32,
        y: f32,
        source: crate::SharedString,
    },
    Text {
        x: f32,
        y: f32,
        text: crate::SharedString,
        font_family: crate::SharedString,
        font_pixel_size: f32,
        color: Color,
    },
}

pub trait HasRenderingPrimitive {
    fn primitive(&self) -> Option<&RenderingPrimitive>;
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

pub struct RenderingCache<Backend>
where
    Backend: GraphicsBackend,
{
    nodes: Vec<RenderingCacheEntry<Backend::LowLevelRenderingPrimitive>>,
    next_free: Option<usize>,
    len: usize,
}

impl<Backend> Default for RenderingCache<Backend>
where
    Backend: GraphicsBackend,
{
    fn default() -> Self {
        Self { nodes: vec![], next_free: None, len: 0 }
    }
}

impl<Backend> RenderingCache<Backend>
where
    Backend: GraphicsBackend,
{
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
