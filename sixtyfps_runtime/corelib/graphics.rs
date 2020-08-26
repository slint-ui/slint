/* LICENSE BEGIN
    This file is part of the SixtyFPS Project -- https://sixtyfps.io
    Copyright (c) 2020 Olivier Goffart <olivier.goffart@sixtyfps.io>
    Copyright (c) 2020 Simon Hausmann <simon.hausmann@sixtyfps.io>

    SPDX-License-Identifier: GPL-3.0-only
    This file is also available under commercial licensing terms.
    Please contact info@sixtyfps.io for more information.
LICENSE END */
extern crate alloc;
use crate::input::{MouseEvent, MouseEventType};
use crate::items::ItemRef;
use crate::properties::{InterpolatedPropertyValue, Property};
#[cfg(feature = "rtti")]
use crate::rtti::{BuiltinItem, FieldInfo, PropertyInfo, ValueType};
use crate::SharedArray;

use cgmath::Matrix4;
use const_field_offset::FieldOffsets;
use core::pin::Pin;
use sixtyfps_corelib_macros::*;
use std::cell::RefCell;
use std::rc::Rc;

/// 2D Rectangle
pub type Rect = euclid::default::Rect<f32>;
/// 2D Point
pub type Point = euclid::default::Point2D<f32>;
/// 2D Size
pub type Size = euclid::default::Size2D<f32>;

/// RGBA color
#[derive(Copy, Clone, PartialEq, Debug, Default)]
#[repr(C)]
pub struct Color {
    red: u8,
    green: u8,
    blue: u8,
    alpha: u8,
}

impl Color {
    /// Construct a color from an integer encoded as `0xAARRGGBB`
    pub const fn from_argb_encoded(encoded: u32) -> Color {
        Color {
            red: (encoded >> 16) as u8,
            green: (encoded >> 8) as u8,
            blue: encoded as u8,
            alpha: (encoded >> 24) as u8,
        }
    }

    /// Construct a color from its RGBA components as u8
    pub const fn from_rgba(red: u8, green: u8, blue: u8, alpha: u8) -> Color {
        Color { red, green, blue, alpha }
    }
    /// Construct a color from its RGB components as u8
    pub const fn from_rgb(red: u8, green: u8, blue: u8) -> Color {
        Color::from_rgba(red, green, blue, 0xff)
    }

    /// Returns `(red, green, blue, alpha)` encoded as f32
    pub fn as_rgba_f32(&self) -> (f32, f32, f32, f32) {
        (
            (self.red as f32) / 255.0,
            (self.green as f32) / 255.0,
            (self.blue as f32) / 255.0,
            (self.alpha as f32) / 255.0,
        )
    }

    /// Returns `(red, green, blue, alpha)` encoded as u8
    pub fn as_rgba_u8(&self) -> (u8, u8, u8, u8) {
        (self.red, self.green, self.blue, self.alpha)
    }

    /// Returns `(alpha, red, green, blue)` encoded as u32
    pub fn as_argb_encoded(&self) -> u32 {
        ((self.red as u32) << 16)
            | ((self.green as u32) << 8)
            | (self.blue as u32)
            | ((self.alpha as u32) << 24)
    }

    /// A constant for the black color
    pub const BLACK: Color = Color::from_rgb(0, 0, 0);
    /// A constant for the white color
    pub const WHITE: Color = Color::from_rgb(255, 255, 255);
    /// A constant for the transparent color
    pub const TRANSPARENT: Color = Color::from_rgba(0, 0, 0, 0);
}

impl From<u32> for Color {
    fn from(encoded: u32) -> Self {
        Color::from_argb_encoded(encoded)
    }
}

impl InterpolatedPropertyValue for Color {
    fn interpolate(self, target_value: Self, t: f32) -> Self {
        Self {
            red: self.red.interpolate(target_value.red, t),
            green: self.green.interpolate(target_value.green, t),
            blue: self.blue.interpolate(target_value.blue, t),
            alpha: self.alpha.interpolate(target_value.alpha, t),
        }
    }
}

impl std::fmt::Display for Color {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "argb({}, {}, {}, {})", self.alpha, self.red, self.green, self.blue)
    }
}

/// A resource is a reference to binary data, for example images. They can be accessible on the file
/// system or embedded in the resulting binary. Or they might be URLs to a web server and a downloaded
/// is necessary before they can be used.
#[derive(Clone, PartialEq, Debug)]
#[repr(u8)]
pub enum Resource {
    /// A resource that does not represent any data.
    None,
    /// A resource that points to a file in the file system
    AbsoluteFilePath(crate::SharedString),
    /// A resource that is embedded in the program and accessible via pointer
    /// The format is the same as in a file
    EmbeddedData(super::slice::Slice<'static, u8>),
    /// Raw ARGB
    #[allow(missing_docs)]
    EmbeddedRgbaImage { width: u32, height: u32, data: super::sharedarray::SharedArray<u8> },
}

impl Default for Resource {
    fn default() -> Self {
        Resource::None
    }
}

/// Each item return a RenderingPrimitive to the backend with information about what to draw.
#[derive(PartialEq, Debug)]
#[repr(C)]
#[allow(missing_docs)]
pub enum HighLevelRenderingPrimitive {
    /// There is nothing to draw
    NoContents,
    // Expected rendering variables in order: Color (fill color)
    Rectangle {
        width: f32,
        height: f32,
    },
    // Expected rendering variables in order: Color (fill color), Color (border color)
    BorderRectangle {
        width: f32,
        height: f32,
        border_width: f32,
        border_radius: f32,
    },
    /// Optional rendering variable: ScaledWidth, ScaledHeight
    Image {
        source: crate::Resource,
    },
    // TODO: turn color into a rendering variable. Needs fixing of the wasm canvas code path though.
    Text {
        text: crate::SharedString,
        font_family: crate::SharedString,
        font_size: f32,
        color: Color,
    },
    // Expected rendering variables in order: Color (fill color), Color (stroke color)
    Path {
        width: f32,
        height: f32,
        elements: crate::PathData,
        stroke_width: f32,
    },
}

#[derive(Debug, Clone)]
#[repr(C)]
pub enum RenderingVariable {
    Translate(f32, f32),
    Color(Color),
    ScaledWidth(f32),
    ScaledHeight(f32),
}

impl RenderingVariable {
    pub fn as_color(&self) -> &Color {
        match self {
            RenderingVariable::Color(c) => c,
            _ => panic!("internal error: expected color but found something else"),
        }
    }
    pub fn as_scaled_width(&self) -> f32 {
        match self {
            RenderingVariable::ScaledWidth(w) => *w,
            _ => panic!("internal error: expected scaled width but found something else"),
        }
    }
    pub fn as_scaled_height(&self) -> f32 {
        match self {
            RenderingVariable::ScaledHeight(h) => *h,
            _ => panic!("internal error: expected scaled height but found something else"),
        }
    }
}

pub trait Frame {
    type LowLevelRenderingPrimitive;
    fn render_primitive(
        &mut self,
        primitive: &Self::LowLevelRenderingPrimitive,
        transform: &Matrix4<f32>,
        variables: SharedArray<RenderingVariable>,
    );
}

pub trait RenderingPrimitivesBuilder {
    type LowLevelRenderingPrimitive;

    fn create(
        &mut self,
        primitive: HighLevelRenderingPrimitive,
    ) -> Self::LowLevelRenderingPrimitive;
}

pub trait GraphicsBackend: Sized {
    type LowLevelRenderingPrimitive;
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

struct TrackingRenderingPrimitive<RenderingPrimitive> {
    primitive: RenderingPrimitive,
    dependency_tracker: core::pin::Pin<Box<crate::properties::PropertyTracker>>,
}

impl<RenderingPrimitive> TrackingRenderingPrimitive<RenderingPrimitive> {
    fn new(update_fn: impl FnOnce() -> RenderingPrimitive) -> Self {
        let dependency_tracker = Box::pin(crate::properties::PropertyTracker::default());
        let primitive = dependency_tracker.as_ref().evaluate(update_fn);
        Self { primitive, dependency_tracker }
    }
}

impl<RenderingPrimitive> From<RenderingPrimitive>
    for TrackingRenderingPrimitive<RenderingPrimitive>
{
    fn from(p: RenderingPrimitive) -> Self {
        Self { primitive: p, dependency_tracker: Box::pin(Default::default()) }
    }
}

enum RenderingCacheEntry<RenderingPrimitive> {
    AllocateEntry(TrackingRenderingPrimitive<RenderingPrimitive>),
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
    pub fn ensure_cached(
        &mut self,
        index: Option<usize>,
        update_fn: impl FnOnce() -> Backend::LowLevelRenderingPrimitive,
    ) -> usize {
        if let Some(index) = index {
            match self.nodes[index] {
                RenderingCacheEntry::AllocateEntry(ref mut data) => {
                    if data.dependency_tracker.is_dirty() {
                        data.primitive = data.dependency_tracker.as_ref().evaluate(update_fn)
                    }
                }
                _ => unreachable!(),
            }
            index
        } else {
            self.allocate_entry(update_fn)
        }
    }

    fn allocate_entry(
        &mut self,
        content_fn: impl FnOnce() -> Backend::LowLevelRenderingPrimitive,
    ) -> usize {
        let idx = {
            if let Some(free_idx) = self.next_free {
                let node = &mut self.nodes[free_idx];
                if let RenderingCacheEntry::FreeEntry(next_free) = node {
                    self.next_free = *next_free;
                } else {
                    unreachable!();
                }
                *node =
                    RenderingCacheEntry::AllocateEntry(TrackingRenderingPrimitive::new(content_fn));
                free_idx
            } else {
                self.nodes.push(RenderingCacheEntry::AllocateEntry(
                    TrackingRenderingPrimitive::new(content_fn),
                ));
                self.nodes.len() - 1
            }
        };
        self.len = self.len + 1;
        idx
    }

    pub fn entry_at(&self, idx: usize) -> &Backend::LowLevelRenderingPrimitive {
        match self.nodes[idx] {
            RenderingCacheEntry::AllocateEntry(ref data) => return &data.primitive,
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

type WindowFactoryFn<Backend> =
    dyn Fn(&crate::eventloop::EventLoop, winit::window::WindowBuilder) -> Backend;

struct MappedWindow<Backend: GraphicsBackend + 'static> {
    backend: RefCell<Backend>,
    rendering_cache: RefCell<RenderingCache<Backend>>,
}

enum GraphicsWindowBackendState<Backend: GraphicsBackend + 'static> {
    Unmapped,
    Mapped(MappedWindow<Backend>),
}

impl<Backend: GraphicsBackend + 'static> GraphicsWindowBackendState<Backend> {
    fn as_mapped(&self) -> &MappedWindow<Backend> {
        match self {
            GraphicsWindowBackendState::Unmapped => panic!(
                "internal error: tried to access window functions that require a mapped window"
            ),
            GraphicsWindowBackendState::Mapped(mw) => &mw,
        }
    }
}

#[derive(FieldOffsets)]
#[repr(C)]
#[pin]
struct WindowProperties {
    scale_factor: Property<f32>,
    width: Property<f32>,
    height: Property<f32>,
}

impl Default for WindowProperties {
    fn default() -> Self {
        Self {
            scale_factor: Property::new(1.0),
            width: Property::new(800.),
            height: Property::new(600.),
        }
    }
}

pub struct GraphicsWindow<Backend: GraphicsBackend + 'static> {
    window_factory: Box<WindowFactoryFn<Backend>>,
    map_state: RefCell<GraphicsWindowBackendState<Backend>>,
    properties: Pin<Box<WindowProperties>>,
}

impl<Backend: GraphicsBackend + 'static> GraphicsWindow<Backend> {
    pub fn new(
        graphics_backend_factory: impl Fn(&crate::eventloop::EventLoop, winit::window::WindowBuilder) -> Backend
            + 'static,
    ) -> Rc<Self> {
        Rc::new(Self {
            window_factory: Box::new(graphics_backend_factory),
            map_state: RefCell::new(GraphicsWindowBackendState::Unmapped),
            properties: Box::pin(WindowProperties::default()),
        })
    }

    pub fn id(&self) -> Option<winit::window::WindowId> {
        Some(self.map_state.borrow().as_mapped().backend.borrow().window().id())
    }
}

impl<Backend: GraphicsBackend> Drop for GraphicsWindow<Backend> {
    fn drop(&mut self) {
        match &*self.map_state.borrow() {
            GraphicsWindowBackendState::Unmapped => {}
            GraphicsWindowBackendState::Mapped(mw) => {
                crate::eventloop::unregister_window(mw.backend.borrow().window().id());
            }
        }
    }
}

impl<Backend: GraphicsBackend> crate::eventloop::GenericWindow for GraphicsWindow<Backend> {
    fn draw(&self, component: crate::component::ComponentRefPin) {
        {
            let map_state = self.map_state.borrow();
            let window = map_state.as_mapped();
            let mut backend = window.backend.borrow_mut();
            let mut rendering_primitives_builder = backend.new_rendering_primitives_builder();

            // Generate cached rendering data once
            crate::item_tree::visit_items(
                component,
                crate::item_tree::TraversalOrder::BackToFront,
                |_, item, _| {
                    crate::item_rendering::update_item_rendering_data(
                        item,
                        &window.rendering_cache,
                        &mut rendering_primitives_builder,
                    );
                    crate::item_tree::ItemVisitorResult::Continue(())
                },
                (),
            );

            backend.finish_primitives(rendering_primitives_builder);
        }

        let map_state = self.map_state.borrow();
        let window = map_state.as_mapped();
        let mut backend = window.backend.borrow_mut();
        let size = backend.window().inner_size();
        let mut frame = backend.new_frame(size.width, size.height, &Color::WHITE);
        crate::item_rendering::render_component_items(
            component,
            &mut frame,
            &mut window.rendering_cache.borrow_mut(),
        );
        backend.present_frame(frame);
    }
    fn process_mouse_input(
        &self,
        pos: winit::dpi::PhysicalPosition<f64>,
        what: MouseEventType,
        component: crate::component::ComponentRefPin,
    ) {
        component
            .as_ref()
            .input_event(MouseEvent { pos: euclid::point2(pos.x as _, pos.y as _), what });
    }

    fn with_platform_window(&self, callback: &dyn Fn(&winit::window::Window)) {
        let map_state = self.map_state.borrow();
        let window = map_state.as_mapped();
        let backend = window.backend.borrow();
        let handle = backend.window();
        callback(handle);
    }

    fn map_window(
        self: Rc<Self>,
        event_loop: &crate::eventloop::EventLoop,
        root_item: Pin<ItemRef>,
    ) {
        if matches!(&*self.map_state.borrow(), GraphicsWindowBackendState::Mapped(..)) {
            return;
        }

        let id = {
            let window_builder = winit::window::WindowBuilder::new();

            let backend = self.window_factory.as_ref()(&event_loop, window_builder);

            let platform_window = backend.window();
            let window_id = platform_window.id();

            // Ideally we should be passing the initial requested size to the window builder, but those properties
            // may be specified in logical pixels, relative to the scale factory, which we only know *after* mapping
            // the window to the screen. So we first map the window then, propagate the scale factory and *then* the
            // width/height properties should have the correct values calculated via their bindings that multiply with
            // the scale factor.
            // We could pass the logical requested size at window builder time, *if* we knew what the values are.
            {
                self.properties.as_ref().scale_factor.set(platform_window.scale_factor() as _);
                let existing_size = platform_window.inner_size();

                let mut new_size = existing_size;

                if let Some(window_item) = ItemRef::downcast_pin(root_item) {
                    let width =
                        crate::items::Window::FIELD_OFFSETS.width.apply_pin(window_item).get();
                    if width > 0. {
                        new_size.width = width as _;
                    }
                    let height =
                        crate::items::Window::FIELD_OFFSETS.height.apply_pin(window_item).get();
                    if height > 0. {
                        new_size.height = height as _;
                    }

                    {
                        let window = self.clone();
                        window_item.as_ref().width.set_binding(move || {
                            WindowProperties::FIELD_OFFSETS
                                .width
                                .apply_pin(window.properties.as_ref())
                                .get()
                        });
                    }
                    {
                        let window = self.clone();
                        window_item.as_ref().height.set_binding(move || {
                            WindowProperties::FIELD_OFFSETS
                                .height
                                .apply_pin(window.properties.as_ref())
                                .get()
                        });
                    }
                }

                if new_size != existing_size {
                    platform_window.set_inner_size(new_size)
                }

                self.properties.as_ref().width.set(new_size.width as _);
                self.properties.as_ref().height.set(new_size.height as _);
            }

            self.map_state.replace(GraphicsWindowBackendState::Mapped(MappedWindow {
                backend: RefCell::new(backend),
                rendering_cache: Default::default(),
            }));

            window_id
        };

        crate::eventloop::register_window(
            id,
            self.clone() as Rc<dyn crate::eventloop::GenericWindow>,
        );
    }

    fn request_redraw(&self) {
        match &*self.map_state.borrow() {
            GraphicsWindowBackendState::Unmapped => {}
            GraphicsWindowBackendState::Mapped(window) => {
                window.backend.borrow().window().request_redraw()
            }
        }
    }

    fn unmap_window(self: Rc<Self>) {
        self.map_state.replace(GraphicsWindowBackendState::Unmapped);
    }

    fn scale_factor(&self) -> f32 {
        WindowProperties::FIELD_OFFSETS.scale_factor.apply_pin(self.properties.as_ref()).get()
    }

    fn set_scale_factor(&self, factor: f32) {
        self.properties.as_ref().scale_factor.set(factor);
    }

    fn set_width(&self, width: f32) {
        self.properties.as_ref().width.set(width);
    }

    fn set_height(&self, height: f32) {
        self.properties.as_ref().height.set(height);
    }

    fn free_graphics_resources(
        self: Rc<Self>,
        component: core::pin::Pin<crate::component::ComponentRef>,
    ) {
        match &*self.map_state.borrow() {
            GraphicsWindowBackendState::Unmapped => {}
            GraphicsWindowBackendState::Mapped(window) => {
                crate::item_rendering::free_item_rendering_data(component, &window.rendering_cache)
            }
        }
    }
}

#[repr(C)]
#[derive(FieldOffsets, Default, BuiltinItem, Clone, Debug, PartialEq)]
#[pin]
/// PathLineTo describes the event of moving the cursor on the path to the specified location
/// along a straight line.
pub struct PathLineTo {
    #[rtti_field]
    /// The x coordinate where the line should go to.
    pub x: f32,
    #[rtti_field]
    /// The y coordinate where the line should go to.
    pub y: f32,
}

#[repr(C)]
#[derive(FieldOffsets, Default, BuiltinItem, Clone, Debug, PartialEq)]
#[pin]
/// PathArcTo describes the event of moving the cursor on the path across an arc to the specified
/// x/y coordinates, with the specified x/y radius and additional properties.
pub struct PathArcTo {
    #[rtti_field]
    /// The x coordinate where the arc should end up.
    pub x: f32,
    #[rtti_field]
    /// The y coordinate where the arc should end up.
    pub y: f32,
    #[rtti_field]
    /// The radius on the x-axis of the arc.
    pub radius_x: f32,
    #[rtti_field]
    /// The radius on the y-axis of the arc.
    pub radius_y: f32,
    #[rtti_field]
    /// The rotation along the x-axis of the arc in degress.
    pub x_rotation: f32,
    #[rtti_field]
    /// large_arc indicates whether to take the long or the shorter path to complete the arc.
    pub large_arc: bool,
    #[rtti_field]
    /// sweep indicates the direction of the arc. If true, a clockwise direction is chosen,
    /// otherwise counter-clockwise.
    pub sweep: bool,
}

#[repr(C)]
#[derive(Clone, Debug, PartialEq)]
/// PathElement describes a single element on a path, such as move-to, line-to, etc.
pub enum PathElement {
    /// The LineTo variant describes a line.
    LineTo(PathLineTo),
    /// The PathArcTo variant describes an arc.
    ArcTo(PathArcTo),
    /// Indicates that the path should be closed now by connecting to the starting point.
    Close,
}

#[repr(C)]
#[derive(Clone, Debug, PartialEq)]
/// PathEvent is a low-level data structure describing the composition of a path. Typically it is
/// generated at compile time from a higher-level description, such as SVG commands.
pub enum PathEvent {
    /// The beginning of the path.
    Begin,
    /// A straight line on the path.
    Line,
    /// A quadratic bezier curve on the path.
    Quadratic,
    /// A cubic bezier curve on the path.
    Cubic,
    /// The end of the path that remains open.
    EndOpen,
    /// The end of a path that is closed.
    EndClosed,
}

struct ToLyonPathEventIterator<'a> {
    events_it: std::slice::Iter<'a, PathEvent>,
    coordinates_it: std::slice::Iter<'a, Point>,
    first: Option<&'a Point>,
    last: Option<&'a Point>,
}

impl<'a> Iterator for ToLyonPathEventIterator<'a> {
    type Item = lyon::path::Event<lyon::math::Point, lyon::math::Point>;
    fn next(&mut self) -> Option<Self::Item> {
        use lyon::path::Event;

        self.events_it.next().map(|event| match event {
            PathEvent::Begin => Event::Begin { at: self.coordinates_it.next().unwrap().clone() },
            PathEvent::Line => Event::Line {
                from: self.coordinates_it.next().unwrap().clone(),
                to: self.coordinates_it.next().unwrap().clone(),
            },
            PathEvent::Quadratic => Event::Quadratic {
                from: self.coordinates_it.next().unwrap().clone(),
                ctrl: self.coordinates_it.next().unwrap().clone(),
                to: self.coordinates_it.next().unwrap().clone(),
            },
            PathEvent::Cubic => Event::Cubic {
                from: self.coordinates_it.next().unwrap().clone(),
                ctrl1: self.coordinates_it.next().unwrap().clone(),
                ctrl2: self.coordinates_it.next().unwrap().clone(),
                to: self.coordinates_it.next().unwrap().clone(),
            },
            PathEvent::EndOpen => Event::End {
                first: self.first.unwrap().clone(),
                last: self.last.unwrap().clone(),
                close: false,
            },
            PathEvent::EndClosed => Event::End {
                first: self.first.unwrap().clone(),
                last: self.last.unwrap().clone(),
                close: true,
            },
        })
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        self.events_it.size_hint()
    }
}

impl<'a> ExactSizeIterator for ToLyonPathEventIterator<'a> {}

struct TransformedLyonPathIterator<EventIt> {
    it: EventIt,
    transform: lyon::math::Transform,
}

impl<EventIt: Iterator<Item = lyon::path::Event<lyon::math::Point, lyon::math::Point>>> Iterator
    for TransformedLyonPathIterator<EventIt>
{
    type Item = lyon::path::Event<lyon::math::Point, lyon::math::Point>;
    fn next(&mut self) -> Option<Self::Item> {
        self.it.next().map(|ev| ev.transformed(&self.transform))
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        self.it.size_hint()
    }
}

impl<EventIt: Iterator<Item = lyon::path::Event<lyon::math::Point, lyon::math::Point>>>
    ExactSizeIterator for TransformedLyonPathIterator<EventIt>
{
}

/// PathDataIterator is a data structure that acts as starting point for iterating
/// through the low-level events of a path. If the path was constructed from said
/// events, then it is a very thin abstraction. If the path was created from higher-level
/// elements, then an intermediate lyon path is required/built.
pub struct PathDataIterator<'a> {
    it: LyonPathIteratorVariant<'a>,
    transform: Option<lyon::math::Transform>,
}

enum LyonPathIteratorVariant<'a> {
    FromPath(lyon::path::Path),
    FromEvents(&'a crate::SharedArray<PathEvent>, &'a crate::SharedArray<Point>),
}

impl<'a> PathDataIterator<'a> {
    /// Create a new iterator for path traversal.
    pub fn iter(
        &'a self,
    ) -> Box<dyn Iterator<Item = lyon::path::Event<lyon::math::Point, lyon::math::Point>> + 'a>
    {
        match &self.it {
            LyonPathIteratorVariant::FromPath(path) => self.apply_transform(path.iter()),
            LyonPathIteratorVariant::FromEvents(events, coordinates) => {
                Box::new(self.apply_transform(ToLyonPathEventIterator {
                    events_it: events.iter(),
                    coordinates_it: coordinates.iter(),
                    first: coordinates.first(),
                    last: coordinates.last(),
                }))
            }
        }
    }

    fn fit(&mut self, width: f32, height: f32) {
        if width > 0. || height > 0. {
            let br = lyon::algorithms::aabb::bounding_rect(self.iter());
            self.transform = Some(lyon::algorithms::fit::fit_rectangle(
                &br,
                &Rect::from_size(Size::new(width, height)),
                lyon::algorithms::fit::FitStyle::Min,
            ));
        }
    }

    fn apply_transform(
        &'a self,
        event_it: impl Iterator<Item = lyon::path::Event<lyon::math::Point, lyon::math::Point>> + 'a,
    ) -> Box<dyn Iterator<Item = lyon::path::Event<lyon::math::Point, lyon::math::Point>> + 'a>
    {
        match self.transform {
            Some(transform) => Box::new(TransformedLyonPathIterator { it: event_it, transform }),
            None => Box::new(event_it),
        }
    }
}

#[repr(C)]
#[derive(Clone, Debug, PartialEq)]
/// PathData represents a path described by either high-level elements or low-level
/// events and coordinates.
pub enum PathData {
    /// None is the variant when the path is empty.
    None,
    /// The Elements variant is used to make a Path from shared arrays of elements.
    Elements(crate::SharedArray<PathElement>),
    /// The Events variant describes the path as a series of low-level events and
    /// associated coordinates.
    Events(crate::SharedArray<PathEvent>, crate::SharedArray<Point>),
}

impl Default for PathData {
    fn default() -> Self {
        Self::None
    }
}

impl PathData {
    /// This function returns an iterator that allows traversing the path by means of lyon events.
    pub fn iter(&self) -> PathDataIterator {
        PathDataIterator {
            it: match self {
                PathData::None => LyonPathIteratorVariant::FromPath(lyon::path::Path::new()),
                PathData::Elements(elements) => LyonPathIteratorVariant::FromPath(
                    PathData::build_path(elements.as_slice().iter()),
                ),
                PathData::Events(events, coordinates) => {
                    LyonPathIteratorVariant::FromEvents(events, coordinates)
                }
            },
            transform: None,
        }
    }

    /// This function returns an iterator that allows traversing the path by means of lyon events.
    pub fn iter_fitted(&self, width: f32, height: f32) -> PathDataIterator {
        let mut it = self.iter();
        it.fit(width, height);
        it
    }

    fn build_path(element_it: std::slice::Iter<PathElement>) -> lyon::path::Path {
        use lyon::geom::SvgArc;
        use lyon::math::{Angle, Point, Vector};
        use lyon::path::{
            builder::{Build, FlatPathBuilder, SvgBuilder},
            ArcFlags,
        };

        let mut path_builder = lyon::path::Path::builder().with_svg();
        for element in element_it {
            match element {
                PathElement::LineTo(PathLineTo { x, y }) => {
                    path_builder.line_to(Point::new(*x, *y))
                }
                PathElement::ArcTo(PathArcTo {
                    x,
                    y,
                    radius_x,
                    radius_y,
                    x_rotation,
                    large_arc,
                    sweep,
                }) => {
                    let radii = Vector::new(*radius_x, *radius_y);
                    let x_rotation = Angle::degrees(*x_rotation);
                    let flags = ArcFlags { large_arc: *large_arc, sweep: *sweep };
                    let to = Point::new(*x, *y);

                    let svg_arc = SvgArc {
                        from: path_builder.current_position(),
                        radii,
                        x_rotation,
                        flags,
                        to,
                    };

                    if svg_arc.is_straight_line() {
                        path_builder.line_to(to);
                    } else {
                        path_builder.arc_to(radii, x_rotation, flags, to)
                    }
                }
                PathElement::Close => path_builder.close(),
            }
        }

        path_builder.build()
    }
}

pub(crate) mod ffi {
    #![allow(unsafe_code)]

    use super::*;

    #[allow(non_camel_case_types)]
    type c_void = ();

    /// Expand Rect so that cbindgen can see it. ( is in fact euclid::default::Rect<f32>)
    #[cfg(cbindgen)]
    #[repr(C)]
    struct Rect {
        x: f32,
        y: f32,
        width: f32,
        height: f32,
    }

    /// Expand Point so that cbindgen can see it. ( is in fact euclid::default::PointD2<f32>)
    #[cfg(cbindgen)]
    #[repr(C)]
    struct Point {
        x: f32,
        y: f32,
    }

    #[no_mangle]
    /// This function is used for the low-level C++ interface to allocate the backing vector for a shared path element array.
    pub unsafe extern "C" fn sixtyfps_new_path_elements(
        out: *mut c_void,
        first_element: *const PathElement,
        count: usize,
    ) {
        let arr = crate::SharedArray::from(std::slice::from_raw_parts(first_element, count));
        core::ptr::write(out as *mut crate::SharedArray<PathElement>, arr.clone());
    }

    #[no_mangle]
    /// This function is used for the low-level C++ interface to allocate the backing vector for a shared path event array.
    pub unsafe extern "C" fn sixtyfps_new_path_events(
        out_events: *mut c_void,
        out_coordinates: *mut c_void,
        first_event: *const PathEvent,
        event_count: usize,
        first_coordinate: *const Point,
        coordinate_count: usize,
    ) {
        let events = crate::SharedArray::from(std::slice::from_raw_parts(first_event, event_count));
        core::ptr::write(out_events as *mut crate::SharedArray<PathEvent>, events.clone());
        let coordinates = crate::SharedArray::from(std::slice::from_raw_parts(
            first_coordinate,
            coordinate_count,
        ));
        core::ptr::write(out_coordinates as *mut crate::SharedArray<Point>, coordinates.clone());
    }
}
