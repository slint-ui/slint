//! This module contains the basic datastructures that are exposed to the C API

use vtable::*;

use crate::graphics::{HighLevelRenderingPrimitive, Rect, RenderingVariable};
use crate::input::{InputEventResult, MouseEvent};
use crate::item_rendering::CachedRenderingData;
use crate::item_tree::ItemVisitorVTable;
use crate::{layout::LayoutInfo, SharedArray};

/// A Component is representing an unit that is allocated together
#[vtable]
#[repr(C)]
pub struct ComponentVTable {
    /// Visit the children of the item at index `index`.
    /// Note that the root item is at index 0, so passing 0 would visit the item under root (the children of root).
    /// If you want to visit the root item, you need to pass -1 as an index.
    /// Returns the index of the item that stopped, or -1 if no visitors cancelled
    pub visit_children_item: extern "C" fn(
        core::pin::Pin<VRef<ComponentVTable>>,
        index: isize,
        visitor: VRefMut<ItemVisitorVTable>,
    ) -> isize,

    /// Returns the layout info for this component
    pub layout_info: extern "C" fn(core::pin::Pin<VRef<ComponentVTable>>) -> LayoutInfo,

    /// Will compute the layout of
    pub compute_layout: extern "C" fn(core::pin::Pin<VRef<ComponentVTable>>),

    /// input event
    pub input_event:
        extern "C" fn(core::pin::Pin<VRef<ComponentVTable>>, MouseEvent) -> InputEventResult,
}

/// Alias for `vtable::VRef<ComponentVTable>` which represent a pointer to a `dyn Component` with
/// the associated vtable
pub type ComponentRef<'a> = vtable::VRef<'a, ComponentVTable>;

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

    /// Return the rendering primitive used to display this item. This should depend on only
    /// rarely changed properties as it typically contains data uploaded to the GPU.
    pub rendering_primitive:
        extern "C" fn(core::pin::Pin<VRef<ItemVTable>>) -> HighLevelRenderingPrimitive,

    /// Return the variables needed to render the graphical primitives of this item. These
    /// are typically variables that do not require uploading any data sets to the GPU and
    /// can instead be represented using uniforms.
    pub rendering_variables:
        extern "C" fn(core::pin::Pin<VRef<ItemVTable>>) -> SharedArray<RenderingVariable>,

    /// We would need max/min/preferred size, and all layout info
    pub layouting_info: extern "C" fn(core::pin::Pin<VRef<ItemVTable>>) -> LayoutInfo,

    /// input event
    pub input_event:
        extern "C" fn(core::pin::Pin<VRef<ItemVTable>>, MouseEvent) -> InputEventResult,
}

/// Alias for `vtable::VRef<ItemVTable>` which represent a pointer to a `dyn Item` with
/// the associated vtable
pub type ItemRef<'a> = vtable::VRef<'a, ItemVTable>;

/// Alias for Option<Pin<&'a Property<T>>> to faciliate cbindgen.
pub type PinnedOptionalProp<'a, T> = Option<core::pin::Pin<&'a crate::Property<T>>>;

#[repr(C)]
#[derive(Default)]
/// WindowProperties is used to pass the references to properties of the instantiated
/// component that the run-time will keep up-to-date.
pub struct WindowProperties<'a> {
    /// A reference to the property that is supposed to be kept up-to-date with the width
    /// of the window.
    pub width: PinnedOptionalProp<'a, f32>,
    /// A reference to the property that is supposed to be kept up-to-date with the height
    /// of the window.
    pub height: PinnedOptionalProp<'a, f32>,

    /// A reference to the property that is supposed to be kept up-to-date with the current
    /// screen dpi / scale factor
    pub scale_factor: PinnedOptionalProp<'a, f32>,
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
