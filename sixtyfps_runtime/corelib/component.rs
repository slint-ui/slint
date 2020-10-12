/* LICENSE BEGIN
    This file is part of the SixtyFPS Project -- https://sixtyfps.io
    Copyright (c) 2020 Olivier Goffart <olivier.goffart@sixtyfps.io>
    Copyright (c) 2020 Simon Hausmann <simon.hausmann@sixtyfps.io>

    SPDX-License-Identifier: GPL-3.0-only
    This file is also available under commercial licensing terms.
    Please contact info@sixtyfps.io for more information.
LICENSE END */
#![warn(missing_docs)]

//! This module contains the basic datastructures that are exposed to the C API

use crate::eventloop::ComponentWindow;
use crate::input::{
    FocusEvent, FocusEventResult, InputEventResult, KeyEvent, KeyEventResult, MouseEvent,
};
use crate::item_tree::{ItemVisitorVTable, TraversalOrder, VisitChildrenResult};
use crate::layout::LayoutInfo;
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
    pub input_event: extern "C" fn(
        core::pin::Pin<VRef<ComponentVTable>>,
        MouseEvent,
        &ComponentWindow,
        &core::pin::Pin<VRef<ComponentVTable>>,
    ) -> InputEventResult,

    /// key event
    pub key_event: extern "C" fn(
        core::pin::Pin<VRef<ComponentVTable>>,
        &KeyEvent,
        &ComponentWindow,
    ) -> KeyEventResult,

    /// Event sent to transfer focus between items or to communicate window focus change.
    pub focus_event: extern "C" fn(
        core::pin::Pin<VRef<ComponentVTable>>,
        &FocusEvent,
        &ComponentWindow,
    ) -> FocusEventResult,
}

/// Alias for `vtable::VRef<ComponentVTable>` which represent a pointer to a `dyn Component` with
/// the associated vtable
pub type ComponentRef<'a> = vtable::VRef<'a, ComponentVTable>;

/// Type alias to the commonly use `Pin<VRef<ComponentVTable>>>`
pub type ComponentRefPin<'a> = core::pin::Pin<ComponentRef<'a>>;

/// Call init() on the ItemVTable for each item of the component.
pub fn init_component_items<Base>(
    base: core::pin::Pin<&Base>,
    item_tree: &[crate::item_tree::ItemTreeNode<Base>],
    window: &ComponentWindow,
) {
    item_tree.iter().for_each(|entry| match entry {
        crate::item_tree::ItemTreeNode::Item { item, .. } => {
            item.apply_pin(base).as_ref().init(window)
        }
        crate::item_tree::ItemTreeNode::DynamicTree { .. } => {}
    })
}

pub(crate) mod ffi {
    #![allow(unsafe_code)]

    use super::*;
    use crate::item_tree::*;
    use crate::slice::Slice;

    /// Call init() on the ItemVTable of each item of the component.
    #[no_mangle]
    pub unsafe extern "C" fn sixtyfps_component_init_items(
        component: ComponentRefPin,
        item_tree: Slice<ItemTreeNode<u8>>,
        window_handle: *const crate::eventloop::ffi::ComponentWindowOpaque,
    ) {
        let window = &*(window_handle as *const ComponentWindow);
        super::init_component_items(
            core::pin::Pin::new_unchecked(&*(component.as_ptr() as *const u8)),
            item_tree.as_slice(),
            window,
        )
    }
}
