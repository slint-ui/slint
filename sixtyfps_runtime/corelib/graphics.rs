use kurbo::{Affine, BezPath};

pub trait Frame {
    type RenderingPrimitive;
    fn render_primitive(&mut self, primitive: &Self::RenderingPrimitive, transform: &Affine);
    fn submit(self: Box<Self>);
}

pub trait GraphicsBackend {
    type RenderingPrimitive;
    fn create_path_primitive(&mut self, path: &BezPath) -> Self::RenderingPrimitive;
    fn new_frame(&self) -> Box<dyn Frame<RenderingPrimitive = Self::RenderingPrimitive>>;
}
