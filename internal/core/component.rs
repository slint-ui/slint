// Copyright © SixtyFPS GmbH <info@slint-ui.com>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-commercial

#![warn(missing_docs)]

//! This module contains the basic datastructures that are exposed to the C API

use crate::item_tree::{ItemVisitorVTable, TraversalOrder, VisitChildrenResult};
use crate::items::{ItemVTable, ItemWeak};
use crate::layout::{LayoutInfo, Orientation};
use crate::window::WindowRc;
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

    /// Return a reference to an item using the given index
    pub get_item_ref: extern "C" fn(
        core::pin::Pin<VRef<ComponentVTable>>,
        index: usize,
    ) -> core::pin::Pin<VRef<ItemVTable>>,

    /// Return the parent item.
    /// The return value is an item weak because it can be null if there is no parent.
    /// And the return value is passed by &mut because ItemWeak has a destructor
    pub parent_item:
        extern "C" fn(core::pin::Pin<VRef<ComponentVTable>>, index: usize, result: &mut ItemWeak),

    /// Returns the layout info for this component
    pub layout_info:
        extern "C" fn(core::pin::Pin<VRef<ComponentVTable>>, Orientation) -> LayoutInfo,

    /// in-place destructor (for VRc)
    pub drop_in_place: unsafe fn(VRefMut<ComponentVTable>) -> vtable::Layout,
    /// dealloc function (for VRc)
    pub dealloc: unsafe fn(&ComponentVTable, ptr: *mut u8, layout: vtable::Layout),
}

/// Alias for `vtable::VRef<ComponentVTable>` which represent a pointer to a `dyn Component` with
/// the associated vtable
pub type ComponentRef<'a> = vtable::VRef<'a, ComponentVTable>;

/// Type alias to the commonly used `Pin<VRef<ComponentVTable>>>`
pub type ComponentRefPin<'a> = core::pin::Pin<ComponentRef<'a>>;

/// Type alias to the commonly used VRc<ComponentVTable, Dyn>>
pub type ComponentRc = vtable::VRc<ComponentVTable, Dyn>;
/// Type alias to the commonly used VWeak<ComponentVTable, Dyn>>
pub type ComponentWeak = vtable::VWeak<ComponentVTable, Dyn>;

/// Call init() on the ItemVTable for each item of the component.
pub fn init_component_items_array<Base>(
    base: core::pin::Pin<&Base>,
    item_array: &[vtable::VOffset<Base, ItemVTable, vtable::AllowPin>],
    window: &WindowRc,
) {
    item_array.iter().for_each(|item| item.apply_pin(base).as_ref().init(window));
}

/// Free the backend graphics resources allocated by the component's items.
pub fn free_component_item_array_graphics_resources<Base>(
    base: core::pin::Pin<&Base>,
    item_array: &[vtable::VOffset<Base, ItemVTable, vtable::AllowPin>],
    window: &WindowRc,
) {
    window.free_graphics_resources(&mut item_array.iter().map(|item| item.apply_pin(base)));
}

#[cfg(feature = "ffi")]
pub(crate) mod ffi {
    #![allow(unsafe_code)]

    use super::*;
    use crate::slice::Slice;

    /// Call init() on the ItemVTable of each item in the item array.
    #[no_mangle]
    pub unsafe extern "C" fn slint_component_init_items_array(
        component: ComponentRefPin,
        item_array: Slice<vtable::VOffset<u8, ItemVTable, vtable::AllowPin>>,
        window_handle: *const crate::window::ffi::WindowRcOpaque,
    ) {
        let window = &*(window_handle as *const WindowRc);
        super::init_component_items_array(
            core::pin::Pin::new_unchecked(&*(component.as_ptr() as *const u8)),
            item_array.as_slice(),
            window,
        )
    }

    /// Free the backend graphics resources allocated in the item array.
    #[no_mangle]
    pub unsafe extern "C" fn slint_component_free_item_array_graphics_resources(
        component: ComponentRefPin,
        item_array: Slice<vtable::VOffset<u8, ItemVTable, vtable::AllowPin>>,
        window_handle: *const crate::window::ffi::WindowRcOpaque,
    ) {
        let window = &*(window_handle as *const WindowRc);
        super::free_component_item_array_graphics_resources(
            core::pin::Pin::new_unchecked(&*(component.as_ptr() as *const u8)),
            item_array.as_slice(),
            window,
        )
    }
}
