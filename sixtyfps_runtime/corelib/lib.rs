/*!

# SixtyFPS runtime library

**NOTE:** This library is an internal crate for the SixtyFPS project.
This crate should not be used directly by application using SixtyFPS.
You should use the `sixtyfps` crate instead
*/

#![deny(unsafe_code)]

/// The animation system
pub mod animations;
pub(crate) mod flickable;
pub mod font;
pub mod graphics;
pub mod input;
pub mod item_tree;
pub mod layout;

#[cfg(feature = "rtti")]
pub mod rtti;

pub mod items;
pub mod model;
pub mod properties;
pub mod sharedarray;
pub mod signals;
pub mod string;

#[doc(inline)]
pub use string::SharedString;

#[doc(inline)]
pub use sharedarray::SharedArray;

#[doc(inline)]
pub use graphics::Resource;

#[doc(inline)]
pub use properties::Property;

#[doc(inline)]
pub use signals::Signal;

#[doc(inline)]
pub use graphics::Color;

#[doc(inline)]
pub use graphics::PathData;

pub mod slice;

use input::{InputEventResult, MouseEvent};
use item_tree::{ItemVisitorVTable, TraversalOrder, VisitChildrenResult};
use layout::LayoutInfo;
use vtable::*;

/// A Component is representing an unit that is allocated together
#[vtable]
#[repr(C)]
pub struct ComponentVTable {
    /// Visit the children of the item at index `index`.
    /// Note that the root item is at index 0, so passing 0 would visit the item under root (the children of root).
    /// If you want to visit the root item, you need to pass -1 as an index.
    pub visit_children_item: extern "C" fn(
        core::pin::Pin<VRef<ComponentVTable>>,
        index: isize,
        order: TraversalOrder,
        visitor: VRefMut<ItemVisitorVTable>,
    ) -> VisitChildrenResult,

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

/// Type alias to the commonly use `Pin<VRef<ComponentVTable>>>`
pub type ComponentRefPin<'a> = core::pin::Pin<ComponentRef<'a>>;

pub mod eventloop;
pub mod item_rendering;
pub mod tests;

/// One need to use at least one function in each module in order to get them
/// exported in the final binary.
/// This only use functions from modules which are not otherwise used.
#[doc(hidden)]
#[cold]
pub fn use_modules() -> usize {
    tests::sixtyfps_mock_elapsed_time as usize
        + signals::ffi::sixtyfps_signal_init as usize
        + sharedarray::ffi::sixtyfps_shared_array_drop as usize
        + layout::solve_grid_layout as usize
        + item_tree::ffi::sixtyfps_visit_item_tree as usize
        + graphics::ffi::sixtyfps_new_path_elements as usize
        + properties::ffi::sixtyfps_property_init as usize
        + string::ffi::sixtyfps_shared_string_bytes as usize
        + eventloop::ffi::sixtyfps_component_window_drop as usize
        + input::ffi::sixtyfps_process_ungrabbed_mouse_event as usize
}
