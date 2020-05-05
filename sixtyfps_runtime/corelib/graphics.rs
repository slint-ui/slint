extern crate alloc;
use kurbo::{Affine, BezPath};

pub trait Frame {
    type RenderingPrimitive;
    fn render_primitive(&mut self, primitive: &Self::RenderingPrimitive, transform: &Affine);
    fn submit(self);
}

pub trait GraphicsBackend: Sized {
    type RenderingPrimitive;
    type Frame: Frame<RenderingPrimitive = Self::RenderingPrimitive>;
    fn create_path_primitive(&mut self, path: &BezPath) -> Self::RenderingPrimitive;
    fn new_frame(&self) -> Self::Frame;
}

struct RenderNodeData<RenderingPrimitive> {
    transform: Option<Affine>,
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
    nodes: &'a mut Vec<NodeEntry<RenderingPrimitive>>,
}

impl<'a, RenderingPrimitive> RenderNode<'a, RenderingPrimitive> {
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

    pub fn set_transform(&mut self, transform: Option<Affine>) {
        self.data_mut().transform = transform;
    }

    pub fn transform(&self) -> Option<Affine> {
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
}

impl<Backend> Default for RenderTree<Backend>
where
    Backend: GraphicsBackend,
{
    fn default() -> Self {
        Self { nodes: vec![], next_free: None, len: 0 }
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
        transform: Option<Affine>,
    ) -> usize {
        self.allocate_index(RenderNodeData { content, transform, children: vec![] })
    }

    pub fn allocate_node(&mut self) -> RenderNode<Backend::RenderingPrimitive> {
        let idx = self.allocate_index(RenderNodeData::default());
        self.node_at(idx)
    }

    pub fn node_at<'a>(&'a mut self, idx: usize) -> RenderNode<'a, Backend::RenderingPrimitive> {
        RenderNode { idx, nodes: &mut self.nodes }
    }

    pub fn free(&mut self, idx: usize) {
        self.len = self.len - 1;
        self.nodes[idx] = NodeEntry::FreeEntry(self.next_free);
        self.next_free = Some(idx);
    }

    pub fn len(&self) -> usize {
        self.len
    }

    pub fn render(&mut self, renderer: &Backend, root: usize) {
        let mut frame = renderer.new_frame();
        self.render_node(&mut frame, root, &Affine::default());
        frame.submit();
    }

    fn render_node(&mut self, frame: &mut Backend::Frame, idx: usize, parent_transform: &Affine) {
        let node = self.node_at(idx);
        let children: Vec<usize> = node.children_iter().map(|i| *i).collect();

        let transform = *parent_transform
            * match node.transform() {
                Some(transform) => transform,
                None => Affine::default(),
            };

        if let Some(content) = node.content() {
            frame.render_primitive(content, &transform);
        }

        for child_idx in children {
            self.render_node(frame, child_idx, &transform);
        }
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
        fn create_path_primitive(&mut self, _path: &BezPath) -> Self::RenderingPrimitive {
            todo!()
        }
        fn new_frame(&self) -> Self::Frame {
            todo!()
        }
    }

    impl Frame for TestFrame {
        type RenderingPrimitive = TestPrimitive;
        fn render_primitive(&mut self, _primitive: &Self::RenderingPrimitive, _transform: &Affine) {
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
