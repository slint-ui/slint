extern crate alloc;
use cgmath::Matrix4;
use lyon::path::{math::Rect, Path};

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

pub trait Frame {
    type RenderingPrimitive;
    fn render_primitive(&mut self, primitive: &Self::RenderingPrimitive, transform: &Matrix4<f32>);
}

pub trait GraphicsBackend: Sized {
    type RenderingPrimitive;
    type Frame: Frame<RenderingPrimitive = Self::RenderingPrimitive>;
    fn create_path_fill_primitive(
        &mut self,
        path: &Path,
        style: FillStyle,
    ) -> Self::RenderingPrimitive;
    fn create_image_primitive(
        &mut self,
        source_rect: impl Into<Rect>,
        dest_rect: impl Into<Rect>,
        image: image::ImageBuffer<image::Rgba<u8>, Vec<u8>>,
    ) -> Self::RenderingPrimitive;

    fn create_rect_primitive(
        &mut self,
        x: f32,
        y: f32,
        width: f32,
        height: f32,
        color: Color,
    ) -> Self::RenderingPrimitive {
        use lyon::math::Point;

        let mut rect_path = Path::builder();
        rect_path.move_to(Point::new(x, y));
        rect_path.line_to(Point::new(x + width, y));
        rect_path.line_to(Point::new(x + width, y + height));
        rect_path.line_to(Point::new(x, y + height));
        rect_path.close();
        self.create_path_fill_primitive(&rect_path.build(), FillStyle::SolidColor(color))
    }

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
    nodes: Vec<RenderingCacheEntry<Backend::RenderingPrimitive>>,
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
    pub fn allocate_entry(&mut self, content: Backend::RenderingPrimitive) -> usize {
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

    pub fn entry_at(&self, idx: usize) -> &Backend::RenderingPrimitive {
        match self.nodes[idx] {
            RenderingCacheEntry::AllocateEntry(ref data) => return data,
            _ => unreachable!(),
        }
    }

    pub fn set_entry_at(&mut self, idx: usize, primitive: Backend::RenderingPrimitive) {
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
