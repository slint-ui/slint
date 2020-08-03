extern crate alloc;
use crate::abi::datastructures::{Color, Size};
use cgmath::Matrix4;
use std::cell::RefCell;
use std::rc::Rc;

pub enum FillStyle {
    SolidColor(Color),
}

/// Each item return a RenderingPrimitive to the backend with information about what to draw.
#[derive(PartialEq, Debug)]
#[repr(C)]
#[allow(missing_docs)]
pub enum RenderingPrimitive {
    /// There is nothing to draw
    NoContents,
    Rectangle {
        x: f32,
        y: f32,
        width: f32,
        height: f32,
        color: Color,
    },
    BorderRectangle {
        x: f32,
        y: f32,
        width: f32,
        height: f32,
        color: Color,
        border_width: f32,
        border_radius: f32,
        border_color: Color,
    },
    Image {
        x: f32,
        y: f32,
        source: crate::Resource,
    },
    Text {
        x: f32,
        y: f32,
        text: crate::SharedString,
        font_family: crate::SharedString,
        font_pixel_size: f32,
        color: Color,
    },
    Path {
        x: f32,
        y: f32,
        width: f32,
        height: f32,
        elements: crate::PathData,
        fill_color: Color,
        stroke_color: Color,
        stroke_width: f32,
    },
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

pub struct GraphicsWindow<Backend: GraphicsBackend + 'static> {
    graphics_backend_factory:
        Box<dyn Fn(&crate::eventloop::EventLoop, winit::window::WindowBuilder) -> Backend>,
    graphics_backend: Option<Backend>,
    rendering_cache: RenderingCache<Backend>,
}

impl<Backend: GraphicsBackend + 'static> GraphicsWindow<Backend> {
    pub fn new(
        graphics_backend_factory: impl Fn(&crate::eventloop::EventLoop, winit::window::WindowBuilder) -> Backend
            + 'static,
    ) -> Rc<RefCell<Self>> {
        let this = Rc::new(RefCell::new(Self {
            graphics_backend_factory: Box::new(graphics_backend_factory),
            graphics_backend: None,
            rendering_cache: RenderingCache::default(),
        }));

        this
    }

    pub fn id(&self) -> Option<winit::window::WindowId> {
        self.graphics_backend.as_ref().map(|backend| backend.window().id())
    }
}

impl<Backend: GraphicsBackend> Drop for GraphicsWindow<Backend> {
    fn drop(&mut self) {
        if let Some(backend) = self.graphics_backend.as_ref() {
            crate::eventloop::unregister_window(backend.window().id());
        }
    }
}

impl<Backend: GraphicsBackend> crate::eventloop::GenericWindow
    for RefCell<GraphicsWindow<Backend>>
{
    fn draw(&self, component: crate::ComponentRefPin) {
        let mut this = self.borrow_mut();

        {
            let mut rendering_primitives_builder =
                this.graphics_backend.as_mut().unwrap().new_rendering_primitives_builder();

            // Generate cached rendering data once
            crate::item_tree::visit_items(
                component,
                |_, item, _| {
                    crate::item_rendering::update_item_rendering_data(
                        item,
                        &mut this.rendering_cache,
                        &mut rendering_primitives_builder,
                    );
                },
                (),
            );

            this.graphics_backend.as_mut().unwrap().finish_primitives(rendering_primitives_builder);
        }

        let window = this.graphics_backend.as_ref().unwrap().window();

        let size = window.inner_size();
        let mut frame = this.graphics_backend.as_mut().unwrap().new_frame(
            size.width,
            size.height,
            &Color::WHITE,
        );
        crate::item_rendering::render_component_items(
            component,
            &mut frame,
            &mut this.rendering_cache,
        );
        this.graphics_backend.as_mut().unwrap().present_frame(frame);
    }
    fn process_mouse_input(
        &self,
        pos: winit::dpi::PhysicalPosition<f64>,
        what: crate::abi::datastructures::MouseEventType,
        component: crate::ComponentRefPin,
    ) {
        crate::input::process_mouse_event(
            component,
            crate::abi::datastructures::MouseEvent {
                pos: euclid::point2(pos.x as _, pos.y as _),
                what,
            },
        );
    }
    fn window_handle(&self) -> std::cell::Ref<winit::window::Window> {
        std::cell::Ref::map(self.borrow(), |mw| mw.graphics_backend.as_ref().unwrap().window())
    }
    fn map_window(self: Rc<Self>, event_loop: &crate::eventloop::EventLoop) {
        if self.borrow().graphics_backend.is_some() {
            return;
        }

        let id = {
            let window_builder = winit::window::WindowBuilder::new();

            let mut this = self.borrow_mut();
            let factory = this.graphics_backend_factory.as_mut();
            let backend = factory(&event_loop, window_builder);

            let window_id = backend.window().id();

            this.graphics_backend = Some(backend);

            window_id
        };

        crate::eventloop::register_window(
            id,
            self.clone() as Rc<dyn crate::eventloop::GenericWindow>,
        );
    }

    fn request_redraw(&self) {
        if let Some(backend) = self.borrow().graphics_backend.as_ref() {
            backend.window().request_redraw();
        }
    }

    fn size(&self) -> Size {
        if let Some(backend) = self.borrow().graphics_backend.as_ref() {
            let size = backend.window().inner_size();
            Size::new(size.width as _, size.height as _)
        } else {
            Size::default()
        }
    }
}
