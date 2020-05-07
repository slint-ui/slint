extern crate alloc;
use cgmath::{Matrix4, SquareMatrix};
use kurbo::{BezPath, Rect};

pub struct Color {
    red: u8,
    green: u8,
    blue: u8,
    alpha: u8,
}

impl Color {
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
    fn submit(self);
}

pub trait GraphicsBackend: Sized {
    type RenderingPrimitive;
    type Frame: Frame<RenderingPrimitive = Self::RenderingPrimitive>;
    fn create_path_fill_primitive(
        &mut self,
        path: &BezPath,
        style: FillStyle,
    ) -> Self::RenderingPrimitive;
    fn create_image_primitive(
        &mut self,
        source_rect: impl Into<Rect>,
        dest_rect: impl Into<Rect>,
        image: image::ImageBuffer<image::Rgba<u8>, Vec<u8>>,
    ) -> Self::RenderingPrimitive;
    fn new_frame(&mut self, width: u32, height: u32, clear_color: &Color) -> Self::Frame;
}

struct RenderNodeData<RenderingPrimitive> {
    transform: Option<Matrix4<f32>>,
    content: Option<RenderingPrimitive>,
    children: Vec<usize>,
}

impl<RenderingPrimitive> Default for RenderNodeData<RenderingPrimitive> {
    fn default() -> Self {
        Self { transform: None, content: None, children: vec![] }
    }
}

enum NodeEntry<RenderPrimitive> {
    AllocateEntry(RenderNodeData<RenderPrimitive>),
    FreeEntry(Option<usize>), // contains next free index if exists
}

pub struct RenderNode<'a, RenderingPrimitive> {
    pub idx: usize,
    nodes: &'a Vec<NodeEntry<RenderingPrimitive>>,
}

impl<'a, RenderingPrimitive> RenderNode<'a, RenderingPrimitive> {
    fn data(&self) -> &RenderNodeData<RenderingPrimitive> {
        match &self.nodes[self.idx] {
            NodeEntry::AllocateEntry(data) => return &data,
            _ => unreachable!(),
        }
    }

    pub fn transform(&self) -> Option<Matrix4<f32>> {
        self.data().transform
    }

    pub fn content(&self) -> Option<&RenderingPrimitive> {
        if let Some(ref prim) = self.data().content {
            return Some(&prim);
        }
        return None;
    }

    pub fn children_iter(&'a self) -> std::slice::Iter<'a, usize> {
        self.data().children.iter()
    }
}

pub struct RenderNodeMut<'a, RenderingPrimitive> {
    pub idx: usize,
    nodes: &'a mut Vec<NodeEntry<RenderingPrimitive>>,
}

impl<'a, RenderingPrimitive> RenderNodeMut<'a, RenderingPrimitive> {
    fn data(&self) -> &RenderNodeData<RenderingPrimitive> {
        match &self.nodes[self.idx] {
            NodeEntry::AllocateEntry(data) => return &data,
            _ => unreachable!(),
        }
    }
    fn data_mut(&mut self) -> &mut RenderNodeData<RenderingPrimitive> {
        match &mut self.nodes[self.idx] {
            NodeEntry::AllocateEntry(ref mut data) => return data,
            _ => unreachable!(),
        }
    }

    pub fn set_transform(&mut self, transform: Option<Matrix4<f32>>) {
        self.data_mut().transform = transform;
    }

    pub fn transform(&self) -> Option<Matrix4<f32>> {
        self.data().transform
    }

    pub fn set_content(&mut self, primitive: Option<RenderingPrimitive>) {
        self.data_mut().content = primitive;
    }

    pub fn content(&self) -> Option<&RenderingPrimitive> {
        if let Some(ref prim) = self.data().content {
            return Some(&prim);
        }
        return None;
    }

    pub fn children_iter(&'a self) -> std::slice::Iter<'a, usize> {
        self.data().children.iter()
    }

    pub fn append_child(&mut self, child_idx: usize) {
        self.data_mut().children.push(child_idx)
    }
}

pub struct RenderTree<Backend>
where
    Backend: GraphicsBackend,
{
    nodes: Vec<NodeEntry<Backend::RenderingPrimitive>>,
    next_free: Option<usize>,
    len: usize,
    clear_color: Color,
}

impl<Backend> Default for RenderTree<Backend>
where
    Backend: GraphicsBackend,
{
    fn default() -> Self {
        Self { nodes: vec![], next_free: None, len: 0, clear_color: Color::WHITE }
    }
}

impl<Backend> RenderTree<Backend>
where
    Backend: GraphicsBackend,
{
    fn allocate_index(&mut self, content: RenderNodeData<Backend::RenderingPrimitive>) -> usize {
        let idx = {
            if let Some(free_idx) = self.next_free {
                let node = &mut self.nodes[free_idx];
                if let NodeEntry::FreeEntry(next_free) = node {
                    self.next_free = *next_free;
                } else {
                    unreachable!();
                }
                *node = NodeEntry::AllocateEntry(content);
                free_idx
            } else {
                self.nodes.push(NodeEntry::AllocateEntry(content));
                self.nodes.len() - 1
            }
        };
        self.len = self.len + 1;
        idx
    }

    pub fn allocate_index_with_content(
        &mut self,
        content: Option<Backend::RenderingPrimitive>,
        transform: Option<Matrix4<f32>>,
    ) -> usize {
        self.allocate_index(RenderNodeData { content, transform, children: vec![] })
    }

    pub fn allocate_node(&mut self) -> RenderNodeMut<Backend::RenderingPrimitive> {
        let idx = self.allocate_index(RenderNodeData::default());
        self.node_at_mut(idx)
    }

    pub fn node_at_mut<'a>(
        &'a mut self,
        idx: usize,
    ) -> RenderNodeMut<'a, Backend::RenderingPrimitive> {
        RenderNodeMut { idx, nodes: &mut self.nodes }
    }

    pub fn node_at<'a>(&'a self, idx: usize) -> RenderNode<'a, Backend::RenderingPrimitive> {
        RenderNode { idx, nodes: &self.nodes }
    }

    pub fn free(&mut self, idx: usize) {
        self.len = self.len - 1;
        self.nodes[idx] = NodeEntry::FreeEntry(self.next_free);
        self.next_free = Some(idx);
    }

    pub fn len(&self) -> usize {
        self.len
    }

    pub fn render(&self, renderer: &mut Backend, width: u32, height: u32, root: usize) {
        let mut frame = renderer.new_frame(width, height, &self.clear_color);
        self.render_node(&mut frame, root, &Matrix4::identity());
        frame.submit();
    }

    fn render_node(&self, frame: &mut Backend::Frame, idx: usize, parent_transform: &Matrix4<f32>) {
        let node = self.node_at(idx);

        let transform = *parent_transform
            * match node.transform() {
                Some(transform) => transform,
                None => Matrix4::identity(),
            };

        if let Some(content) = node.content() {
            frame.render_primitive(content, &transform);
        }

        for child_idx in node.children_iter() {
            self.render_node(frame, *child_idx, &transform);
        }
    }

    pub fn clear_color(self) -> Color {
        self.clear_color
    }

    pub fn set_clear_color(&mut self, clear_color: Color) {
        self.clear_color = clear_color
    }
}

#[cfg(test)]
mod test {
    use super::*;
    #[derive(Default)]
    struct TestPrimitive {}

    #[derive(Default)]
    struct TestBackend {}

    struct TestFrame {}

    impl GraphicsBackend for TestBackend {
        type RenderingPrimitive = TestPrimitive;
        type Frame = TestFrame;
        fn create_path_fill_primitive(
            &mut self,
            _path: &BezPath,
            _style: FillStyle,
        ) -> Self::RenderingPrimitive {
            todo!()
        }
        fn create_image_primitive(
            &mut self,
            _source_rect: impl Into<Rect>,
            _dest_rect: impl Into<Rect>,
            _image: image::ImageBuffer<image::Rgba<u8>, Vec<u8>>,
        ) -> Self::RenderingPrimitive {
            todo!()
        }
        fn new_frame(&mut self, _width: u32, _height: u32, _clear_color: &Color) -> Self::Frame {
            todo!()
        }
    }

    impl Frame for TestFrame {
        type RenderingPrimitive = TestPrimitive;
        fn render_primitive(
            &mut self,
            _primitive: &Self::RenderingPrimitive,
            _transform: &Matrix4<f32>,
        ) {
            todo!()
        }
        fn submit(self) {
            todo!()
        }
    }

    #[test]
    fn test_empty_tree() {
        let mut tree = RenderTree::<TestBackend>::default();
        assert_eq!(tree.len(), 0);
        let root_idx = {
            let mut root = tree.allocate_node();
            {
                root.set_content(Some(TestPrimitive {}));
            }
            root.idx
        };
        assert_eq!(tree.len(), 1);
        tree.free(root_idx);
        assert_eq!(tree.len(), 0);
    }

    #[test]
    fn test_add_remove() {
        let mut tree = RenderTree::<TestBackend>::default();

        let root_idx = tree.allocate_node().idx;
        let child1_idx = tree.allocate_node().idx;
        let child2_idx = tree.allocate_node().idx;

        assert_eq!(root_idx, 0);
        assert_eq!(child1_idx, 1);
        assert_eq!(child2_idx, 2);

        tree.free(child1_idx);
        tree.free(child2_idx);

        let child3_idx = tree.allocate_node().idx;
        let child4_idx = tree.allocate_node().idx;

        assert_eq!(child3_idx, 2);
        assert_eq!(child4_idx, 1);
    }
}
