//! This module contains the basic datastructures that are exposed to the C API

use core::ptr::NonNull;
use std::cell::Cell;
use vtable::*;

/// 2D Rectangle
pub type Rect = euclid::default::Rect<f32>;
/// 2D Point
pub type Point = euclid::default::Point2D<f32>;
/// 2D Size
pub type Size = euclid::default::Size2D<f32>;

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

/// A Component is representing an unit that is allocated together
#[vtable]
#[repr(C)]
pub struct ComponentVTable {
    /// Allocate an instance of this component
    pub create: extern "C" fn(&ComponentVTable) -> VBox<ComponentVTable>,

    /// Destruct this component.
    pub drop: extern "C" fn(VRefMut<ComponentVTable>),

    /// Returns an array that represent the item tree
    pub item_tree: extern "C" fn(VRef<ComponentVTable>) -> *const ItemTreeNode,
    /*
    /// Returns the layout info for this component
    pub layout_info: extern "C" fn(VRef<ComponentVTable>) -> LayoutInfo,

    /// Will compute the layout of
    pub compute_layout: extern "C" fn(VRef<ComponentVTable>),*/
}

/// This structure must be present in items that are Rendered and contains information.
/// Used by the backend.
#[derive(Default)]
#[repr(C)]
pub struct CachedRenderingData {
    /// Used and modified by the backend, should be initialized to 0 by the user code
    pub(crate) cache_index: Cell<usize>,
    /// Set to false initially and when changes happen that require updating the cache
    pub(crate) cache_ok: Cell<bool>,
}

impl CachedRenderingData {
    pub(crate) fn low_level_rendering_primitive<
        'a,
        GraphicsBackend: crate::graphics::GraphicsBackend,
    >(
        &self,
        cache: &'a crate::graphics::RenderingCache<GraphicsBackend>,
    ) -> Option<&'a GraphicsBackend::LowLevelRenderingPrimitive> {
        if !self.cache_ok.get() {
            return None;
        }
        Some(cache.entry_at(self.cache_index.get()))
    }
}

/// The item tree is an array of ItemTreeNode representing a static tree of items
/// within a component.
#[repr(C)]
pub enum ItemTreeNode {
    /// Static item
    Item {
        /// byte offset where we can find the item (from the *ComponentImpl)
        offset: isize,
        /// virtual table of the item
        vtable: *const ItemVTable,

        /// number of children
        chilren_count: u32,

        /// index of the first children within the item tree
        children_index: u32,
    },
    /// A placeholder for many instance of item in their own component which
    /// are instentiated according to a model.
    DynamicTree {
        /// Component vtable.
        /// This component is going to be instantiated as many time as the model tells
        component_type: *const ComponentVTable,

        /// vtable of the model
        model_type: *const super::model::ModelType,

        /// byte offset of the ModelImpl within the component.
        /// The model is an instance of the model described by model_type and must be
        /// stored within the component
        model_offset: isize,

        /// byte offset of the vector of components within the parent component
        /// (ComponentVec)
        /// a ComponentVec must be stored within the component to represent this tree
        components_holder_offset: isize,
    },
}

/// It is supposed to be in static array
unsafe impl Sync for ItemTreeNode {}

/// Items are the nodes in the render tree.
#[vtable]
#[repr(C)]
pub struct ItemVTable {
    /// Returns the geometry of this item (relative to its parent item)
    pub geometry: extern "C" fn(VRef<'_, ItemVTable>, context: &crate::EvaluationContext) -> Rect,

    /// offset in bytes fromthe *const ItemImpl.
    /// isize::MAX  means None
    #[allow(non_upper_case_globals)]
    #[offset(CachedRenderingData)]
    pub cached_rendering_data_offset: usize,

    /// Return the rendering primitive used to display this item.
    pub rendering_primitive: extern "C" fn(
        VRef<'_, ItemVTable>,
        context: &crate::EvaluationContext,
    ) -> RenderingPrimitive,

    /// We would need max/min/preferred size, and all layout info
    pub layouting_info: extern "C" fn(VRef<'_, ItemVTable>) -> LayoutInfo,

    /// input event
    pub input_event: extern "C" fn(VRef<'_, ItemVTable>, MouseEvent, &crate::EvaluationContext),
}

/// The constraint that applies to an item
#[repr(C)]
#[derive(Clone)]
pub struct LayoutInfo {
    min_width: f32,
    max_width: f32,
    min_height: f32,
    max_height: f32,
}

impl Default for LayoutInfo {
    fn default() -> Self {
        LayoutInfo { min_width: 0., max_width: f32::MAX, min_height: 0., max_height: f32::MAX }
    }
}

/// RGBA color
#[derive(Copy, Clone, PartialEq, Debug)]
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

    /// A constant for the black color
    pub const BLACK: Color = Color::from_rgb(0, 0, 0);
    /// A constant for the white color
    pub const WHITE: Color = Color::from_rgb(255, 255, 255);
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
    EmbeddedData(super::slice::Slice<'static, u8>),
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
}

/// The type of a MouseEvent
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub enum MouseEventType {
    /// The mouse was pressed
    MousePressed,
    /// The mouse was relased
    MouseReleased,
    /// The mouse position has changed
    MouseMoved,
}

/// Structur representing a mouse event
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct MouseEvent {
    /// The position of the cursor
    pub pos: Point,
    /// The action performed (pressed/released/moced)
    pub what: MouseEventType,
}

/// Visit each items recursively
///
/// The state parametter returned by the visitor is passed to each children.
pub fn visit_items<State>(
    component: VRef<'_, ComponentVTable>,
    mut visitor: impl FnMut(ItemRef<'_>, &State) -> State,
    state: State,
) {
    visit_internal(component, &mut visitor, 0, &state)
}

fn visit_internal<State>(
    component: VRef<'_, ComponentVTable>,
    visitor: &mut impl FnMut(ItemRef<'_>, &State) -> State,
    index: isize,
    state: &State,
) {
    let item_tree = component.item_tree();
    match unsafe { &*item_tree.offset(index) } {
        ItemTreeNode::Item { vtable, offset, children_index, chilren_count } => {
            let item = unsafe {
                ItemRef::from_raw(
                    NonNull::new_unchecked(*vtable as *mut _),
                    NonNull::new_unchecked(component.as_ptr().offset(*offset) as *mut _),
                )
            };
            let state = visitor(item, state);
            for c in *children_index..(*children_index + *chilren_count) {
                visit_internal(component, visitor, c as isize, &state)
            }
        }
        ItemTreeNode::DynamicTree { .. } => todo!(),
    }
}

/// Same as `visit_items`, but over mutable items.
///
/// The visitor also accept a re-borrow of the component given in imput
pub fn visit_items_mut<State>(
    component: VRefMut<'_, ComponentVTable>,
    mut visitor: impl FnMut(VRefMut<'_, ComponentVTable>, ItemRefMut<'_>, &State) -> State,
    state: State,
) {
    visit_internal_mut(component, &mut visitor, 0, &state)
}

fn visit_internal_mut<State>(
    mut component: VRefMut<'_, ComponentVTable>,
    visitor: &mut impl FnMut(VRefMut<'_, ComponentVTable>, ItemRefMut<'_>, &State) -> State,
    index: isize,
    state: &State,
) {
    let item_tree = component.item_tree();
    match unsafe { &*item_tree.offset(index) } {
        ItemTreeNode::Item { vtable, offset, children_index, chilren_count } => {
            let mut item = unsafe {
                ItemRefMut::from_raw(
                    NonNull::new_unchecked(*vtable as *mut _),
                    NonNull::new_unchecked(
                        (component.as_ptr() as *mut u8).offset(*offset) as *mut _
                    ),
                )
            };
            let state = visitor(component.borrow_mut(), item.borrow_mut(), state);
            for c in *children_index..(*children_index + *chilren_count) {
                visit_internal_mut(component.borrow_mut(), visitor, c as isize, &state)
            }
        }
        ItemTreeNode::DynamicTree { .. } => todo!(),
    }
}

// This is here because for some reason (rust bug?) the ItemVTable_static is not accessible in the other modules

ItemVTable_static! {
    /// The VTable for `Image`
    #[no_mangle]
    pub static ImageVTable for crate::abi::primitives::Image
}
ItemVTable_static! {
    /// The VTable for `Rectangle`
    #[no_mangle]
    pub static RectangleVTable for crate::abi::primitives::Rectangle
}
ItemVTable_static! {
    /// The VTable for `Text`
    #[no_mangle]
    pub static TextVTable for crate::abi::primitives::Text
}
ItemVTable_static! {
    /// The VTable for `TouchArea`
    #[no_mangle]
    pub static TouchAreaVTable for crate::abi::primitives::TouchArea
}
