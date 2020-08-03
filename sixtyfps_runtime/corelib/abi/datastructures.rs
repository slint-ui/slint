//! This module contains the basic datastructures that are exposed to the C API

use super::slice::Slice;
use core::pin::Pin;
use std::cell::Cell;
use vtable::*;

use crate::graphics::{HighLevelRenderingPrimitive, Point, Rect};
use crate::layout::LayoutInfo;

/// A Component is representing an unit that is allocated together
#[vtable]
#[repr(C)]
pub struct ComponentVTable {
    /// Visit the children of the item at index `index`.
    /// Note that the root item is at index 0, so passing 0 would visit the item under root (the children of root).
    /// If you want to visit the root item, you need to pass -1 as an index
    pub visit_children_item: extern "C" fn(
        core::pin::Pin<VRef<ComponentVTable>>,
        index: isize,
        visitor: VRefMut<ItemVisitorVTable>,
    ),

    /// Returns the layout info for this component
    pub layout_info: extern "C" fn(core::pin::Pin<VRef<ComponentVTable>>) -> LayoutInfo,

    /// Will compute the layout of
    pub compute_layout: extern "C" fn(core::pin::Pin<VRef<ComponentVTable>>),
}

/// Alias for `vtable::VRef<ComponentVTable>` which represent a pointer to a `dyn Component` with
/// the associated vtable
pub type ComponentRef<'a> = vtable::VRef<'a, ComponentVTable>;

/// This structure must be present in items that are Rendered and contains information.
/// Used by the backend.
#[derive(Default, Debug)]
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
#[repr(u8)]
pub enum ItemTreeNode<T> {
    /// Static item
    Item {
        /// byte offset where we can find the item (from the *ComponentImpl)
        item: vtable::VOffset<T, ItemVTable, vtable::PinnedFlag>,

        /// number of children
        chilren_count: u32,

        /// index of the first children within the item tree
        children_index: u32,
    },
    /// A placeholder for many instance of item in their own component which
    /// are instantiated according to a model.
    DynamicTree {
        /// the undex which is passed in the visit_dynamic callback.
        index: usize,
    },
}

/// Items are the nodes in the render tree.
#[vtable]
#[repr(C)]
pub struct ItemVTable {
    /// Returns the geometry of this item (relative to its parent item)
    pub geometry: extern "C" fn(core::pin::Pin<VRef<ItemVTable>>) -> Rect,

    /// offset in bytes fromthe *const ItemImpl.
    /// isize::MAX  means None
    #[allow(non_upper_case_globals)]
    #[field_offset(CachedRenderingData)]
    pub cached_rendering_data_offset: usize,

    /// Return the rendering primitive used to display this item.
    pub rendering_primitive:
        extern "C" fn(core::pin::Pin<VRef<ItemVTable>>) -> HighLevelRenderingPrimitive,

    /// We would need max/min/preferred size, and all layout info
    pub layouting_info: extern "C" fn(core::pin::Pin<VRef<ItemVTable>>) -> LayoutInfo,

    /// input event
    pub input_event: extern "C" fn(core::pin::Pin<VRef<ItemVTable>>, MouseEvent),
}

/// Alias for `vtable::VRef<ItemVTable>` which represent a pointer to a `dyn Item` with
/// the associated vtable
pub type ItemRef<'a> = vtable::VRef<'a, ItemVTable>;

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

#[repr(C)]
#[derive(Default)]
/// WindowProperties is used to pass the references to properties of the instantiated
/// component that the run-time will keep up-to-date.
pub struct WindowProperties<'a> {
    /// A reference to the property that is supposed to be kept up-to-date with the width
    /// of the window.
    pub width: Option<&'a crate::Property<f32>>,
    /// A reference to the property that is supposed to be kept up-to-date with the height
    /// of the window.
    pub height: Option<&'a crate::Property<f32>>,

    /// A reference to the property that is supposed to be kept up-to-date with the current
    /// screen dpi
    pub dpi: Option<&'a crate::Property<f32>>,
}

/// The ComponentWindow is the (rust) facing public type that can render the items
/// of components to the screen.
#[repr(C)]
#[derive(Clone)]
pub struct ComponentWindow(std::rc::Rc<dyn crate::eventloop::GenericWindow>);

impl ComponentWindow {
    /// Creates a new instance of a CompomentWindow based on the given window implementation. Only used
    /// internally.
    pub fn new(window_impl: std::rc::Rc<dyn crate::eventloop::GenericWindow>) -> Self {
        Self(window_impl)
    }
    /// Spins an event loop and renders the items of the provided component in this window.
    pub fn run(&self, component: Pin<VRef<ComponentVTable>>, props: &WindowProperties) {
        let event_loop = crate::eventloop::EventLoop::new();
        self.0.clone().map_window(&event_loop);

        {
            let size = self.0.size();
            if let Some(width_property) = props.width {
                width_property.set(size.width)
            }
            if let Some(height_property) = props.height {
                height_property.set(size.height)
            }
        }

        event_loop.run(component, &props);
    }
}

#[allow(non_camel_case_types)]
type c_void = ();

/// Same layout as ComponentWindow (fat pointer)
#[repr(C)]
pub struct ComponentWindowOpaque(*const c_void, *const c_void);

/// Releases the reference to the component window held by handle.
#[no_mangle]
pub unsafe extern "C" fn sixtyfps_component_window_drop(handle: *mut ComponentWindowOpaque) {
    assert_eq!(
        core::mem::size_of::<ComponentWindow>(),
        core::mem::size_of::<ComponentWindowOpaque>()
    );
    core::ptr::read(handle as *mut ComponentWindow);
}

/// Spins an event loop and renders the items of the provided component in this window.
#[no_mangle]
pub unsafe extern "C" fn sixtyfps_component_window_run(
    handle: *mut ComponentWindowOpaque,
    component: Pin<VRef<ComponentVTable>>,
    window_props: *mut WindowProperties,
) {
    let window = &*(handle as *const ComponentWindow);
    let window_props = &*(window_props as *const WindowProperties);
    window.run(component, &window_props);
}

#[repr(C)]
#[vtable]
/// Object to be passed in visit_item_children method of the Component.
pub struct ItemVisitorVTable {
    /// Called for each children of the visited item
    ///
    /// The `component` parameter is the component in which the item live which might not be the same
    /// as the parent's component.
    /// `index` is to be used again in the visit_item_children function of the Component (the one passed as parameter)
    /// and `item` is a reference to the item itself
    visit_item: fn(
        VRefMut<ItemVisitorVTable>,
        component: Pin<VRef<ComponentVTable>>,
        index: isize,
        item: Pin<VRef<ItemVTable>>,
    ),
    /// Destructor
    drop: fn(VRefMut<ItemVisitorVTable>),
}

/// Type alias to `vtable::VRefMut<ItemVisitorVTable>`
pub type ItemVisitorRefMut<'a> = vtable::VRefMut<'a, ItemVisitorVTable>;

impl<T: FnMut(crate::ComponentRefPin, isize, Pin<ItemRef>)> ItemVisitor for T {
    fn visit_item(&mut self, component: crate::ComponentRefPin, index: isize, item: Pin<ItemRef>) {
        self(component, index, item)
    }
}

/// Expose `crate::item_tree::visit_item_tree` to C++
///
/// Safety: Assume a correct implementation of the item_tree array
#[no_mangle]
pub unsafe extern "C" fn sixtyfps_visit_item_tree(
    component: Pin<VRef<ComponentVTable>>,
    item_tree: Slice<ItemTreeNode<u8>>,
    index: isize,
    visitor: VRefMut<ItemVisitorVTable>,
    visit_dynamic: extern "C" fn(
        base: &u8,
        visitor: vtable::VRefMut<ItemVisitorVTable>,
        dyn_index: usize,
    ),
) {
    crate::item_tree::visit_item_tree(
        Pin::new_unchecked(&*(component.as_ptr() as *const u8)),
        component,
        item_tree.as_slice(),
        index,
        visitor,
        |a, b, c| visit_dynamic(a.get_ref(), b, c),
    )
}

// This is here because for some reason (rust bug?) the ItemVTable_static is not accessible in the other modules

ItemVTable_static! {
    /// The VTable for `Image`
    #[no_mangle]
    pub static ImageVTable for crate::items::Image
}
ItemVTable_static! {
    /// The VTable for `Rectangle`
    #[no_mangle]
    pub static RectangleVTable for crate::items::Rectangle
}
ItemVTable_static! {
    /// The VTable for `BorderRectangle`
    #[no_mangle]
    pub static BorderRectangleVTable for crate::items::BorderRectangle
}
ItemVTable_static! {
    /// The VTable for `Text`
    #[no_mangle]
    pub static TextVTable for crate::items::Text
}
ItemVTable_static! {
    /// The VTable for `TouchArea`
    #[no_mangle]
    pub static TouchAreaVTable for crate::items::TouchArea
}
ItemVTable_static! {
    /// The VTable for `Path`
    #[no_mangle]
    pub static PathVTable for crate::items::Path
}

ItemVTable_static! {
    /// The VTable for `Flickable`
    #[no_mangle]
    pub static FlickableVTable for crate::items::Flickable
}
