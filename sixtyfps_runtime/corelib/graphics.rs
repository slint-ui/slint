use kurbo::{Affine, BezPath};

pub trait Frame {
    type RenderingPrimitive;
    fn render_primitive(&mut self, primitive: &Self::RenderingPrimitive, transform: &Affine);
    fn submit(self);
}

pub trait GraphicsBackend {
    type RenderingPrimitive;
    type Frame: Frame<RenderingPrimitive = Self::RenderingPrimitive>;
    fn create_path_primitive(&mut self, path: &BezPath) -> Self::RenderingPrimitive;
    fn new_frame(&self) -> Self::Frame;
}
