// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

// cSpell: ignore xffff unclipped subchildren subsubtree

//! This module contains the ItemTree and code that helps navigating it

use crate::SharedString;
use crate::accessibility::{
    AccessibilityAction, AccessibleStringProperty, SupportedAccessibilityAction,
};
use crate::items::{AccessibleRole, ItemRef, ItemVTable};
use crate::layout::{LayoutInfo, Orientation};
use crate::lengths::{ItemTransform, LogicalPoint, LogicalRect};
use crate::slice::Slice;
use crate::window::WindowAdapterRc;
use alloc::vec::Vec;
use core::ops::ControlFlow;
use core::pin::Pin;
// Only import num_traits::Float for no_std (MCU) builds where it's used.
#[cfg(not(feature = "std"))]
#[allow(unused)]
use num_traits::Float;
use vtable::*;

#[repr(C)]
#[derive(Debug, Clone, Copy)]
/// A range of indices
pub struct IndexRange {
    /// Start index
    pub start: usize,
    /// Index one past the last index
    pub end: usize,
}

impl From<core::ops::Range<usize>> for IndexRange {
    fn from(r: core::ops::Range<usize>) -> Self {
        Self { start: r.start, end: r.end }
    }
}
impl From<IndexRange> for core::ops::Range<usize> {
    fn from(r: IndexRange) -> Self {
        Self { start: r.start, end: r.end }
    }
}

/// A ItemTree is representing an unit that is allocated together
#[cfg_attr(not(feature = "ffi"), i_slint_core_macros::remove_extern)]
#[vtable]
#[repr(C)]
pub struct ItemTreeVTable {
    /// Visit the children of the item at index `index`.
    /// Note that the root item is at index 0, so passing 0 would visit the item under root (the children of root).
    /// If you want to visit the root item, you need to pass -1 as an index.
    pub visit_children_item: extern "C" fn(
        ::core::pin::Pin<VRef<ItemTreeVTable>>,
        index: isize,
        order: TraversalOrder,
        visitor: VRefMut<ItemVisitorVTable>,
    ) -> VisitChildrenResult,

    /// Return a reference to an item using the given index
    pub get_item_ref: extern "C" fn(
        ::core::pin::Pin<VRef<ItemTreeVTable>>,
        index: u32,
    ) -> ::core::pin::Pin<VRef<ItemVTable>>,

    /// Return the range of indices below the dynamic `ItemTreeNode` at `index`
    pub get_subtree_range:
        extern "C" fn(::core::pin::Pin<VRef<ItemTreeVTable>>, index: u32) -> IndexRange,

    /// Return the `ItemTreeRc` at `subindex` below the dynamic `ItemTreeNode` at `index`
    pub get_subtree: extern "C" fn(
        ::core::pin::Pin<VRef<ItemTreeVTable>>,
        index: u32,
        subindex: usize,
        result: &mut vtable::VWeak<ItemTreeVTable, Dyn>,
    ),

    /// Return the item tree that is defined by this `ItemTree`.
    pub get_item_tree: extern "C" fn(::core::pin::Pin<VRef<ItemTreeVTable>>) -> Slice<ItemTreeNode>,

    /// Return the node this ItemTree is a part of in the parent ItemTree.
    ///
    /// The return value is an item weak because it can be null if there is no parent.
    /// And the return value is passed by &mut because ItemWeak has a destructor
    /// Note that the returned value will typically point to a repeater node, which is
    /// strictly speaking not an Item at all!
    ///
    pub parent_node: extern "C" fn(::core::pin::Pin<VRef<ItemTreeVTable>>, result: &mut ItemWeak),

    /// This embeds this ItemTree into the item tree of another ItemTree
    ///
    /// Returns `true` if this ItemTree was embedded into the `parent`
    /// at `parent_item_tree_index`.
    pub embed_component: extern "C" fn(
        ::core::pin::Pin<VRef<ItemTreeVTable>>,
        parent: &VWeak<ItemTreeVTable>,
        parent_item_tree_index: u32,
    ) -> bool,

    /// Return the index of the current subtree or usize::MAX if this is not a subtree
    pub subtree_index: extern "C" fn(::core::pin::Pin<VRef<ItemTreeVTable>>) -> usize,

    /// Returns the layout info for the root of the ItemTree
    pub layout_info:
        extern "C" fn(::core::pin::Pin<VRef<ItemTreeVTable>>, Orientation) -> LayoutInfo,

    /// Recursively materialize every Repeater, Conditional, and
    /// ComponentContainer reachable from this ItemTree. Called at event-loop
    /// boundaries so init code runs outside any in-flight property evaluation.
    /// This is the "repeater instantiation pass".
    /// Returns `true` if any instance was created or removed.
    pub ensure_instantiated: extern "C" fn(::core::pin::Pin<VRef<ItemTreeVTable>>) -> bool,

    /// Returns the item's geometry (relative to its parent item)
    pub item_geometry:
        extern "C" fn(::core::pin::Pin<VRef<ItemTreeVTable>>, item_index: u32) -> LogicalRect,

    /// Returns the accessible role for a given item
    pub accessible_role:
        extern "C" fn(::core::pin::Pin<VRef<ItemTreeVTable>>, item_index: u32) -> AccessibleRole,

    /// Returns the accessible property via the `result`. Returns true if such a property exists.
    pub accessible_string_property: extern "C" fn(
        ::core::pin::Pin<VRef<ItemTreeVTable>>,
        item_index: u32,
        what: AccessibleStringProperty,
        result: &mut SharedString,
    ) -> bool,

    /// Executes an accessibility action.
    pub accessibility_action: extern "C" fn(
        ::core::pin::Pin<VRef<ItemTreeVTable>>,
        item_index: u32,
        action: &AccessibilityAction,
    ),

    /// Returns the supported accessibility actions.
    pub supported_accessibility_actions: extern "C" fn(
        ::core::pin::Pin<VRef<ItemTreeVTable>>,
        item_index: u32,
    ) -> SupportedAccessibilityAction,

    /// Add the `ElementName::id` entries of the given item
    pub item_element_infos: extern "C" fn(
        ::core::pin::Pin<VRef<ItemTreeVTable>>,
        item_index: u32,
        result: &mut SharedString,
    ) -> bool,

    /// Returns a Window, creating a fresh one if `do_create` is true.
    pub window_adapter: extern "C" fn(
        ::core::pin::Pin<VRef<ItemTreeVTable>>,
        do_create: bool,
        result: &mut Option<WindowAdapterRc>,
    ),

    /// in-place destructor (for VRc)
    pub drop_in_place: unsafe extern "C" fn(VRefMut<ItemTreeVTable>) -> vtable::Layout,

    /// dealloc function (for VRc)
    pub dealloc: unsafe extern "C" fn(&ItemTreeVTable, ptr: *mut u8, layout: vtable::Layout),
}

#[cfg(test)]
pub(crate) use ItemTreeVTable_static;

/// Alias for `vtable::VRef<ItemTreeVTable>` which represent a pointer to a `dyn ItemTree` with
/// the associated vtable
pub type ItemTreeRef<'a> = vtable::VRef<'a, ItemTreeVTable>;

/// Type alias to the commonly used `Pin<VRef<ItemTreeVTable>>>`
pub type ItemTreeRefPin<'a> = core::pin::Pin<ItemTreeRef<'a>>;

/// Type alias to the commonly used VRc<ItemTreeVTable, Dyn>>
pub type ItemTreeRc = vtable::VRc<ItemTreeVTable, Dyn>;
/// Type alias to the commonly used VWeak<ItemTreeVTable, Dyn>>
pub type ItemTreeWeak = vtable::VWeak<ItemTreeVTable, Dyn>;

/// Ensure all repeaters and conditionals within the given item tree are
/// instantiated. Call this before non-rendering tree walks that use
/// `first_child` / `next_sibling`.
/// Returns `true` if any instance was created or removed.
pub fn ensure_item_tree_instantiated(item_tree: &vtable::VRc<ItemTreeVTable>) -> bool {
    vtable::VRc::borrow_pin(item_tree).as_ref().ensure_instantiated()
}

/// Call init() on the ItemVTable for each item of the ItemTree.
pub fn register_item_tree(item_tree_rc: &ItemTreeRc, window_adapter: Option<WindowAdapterRc>) {
    let c = vtable::VRc::borrow_pin(item_tree_rc);
    let item_tree = c.as_ref().get_item_tree();
    item_tree.iter().enumerate().for_each(|(tree_index, node)| {
        let tree_index = tree_index as u32;
        if let ItemTreeNode::Item { .. } = &node {
            let item = ItemRc::new(item_tree_rc.clone(), tree_index);
            c.as_ref().get_item_ref(tree_index).as_ref().init(&item);
        }
    });
    if let Some(adapter) = window_adapter.as_ref().and_then(|a| a.internal(crate::InternalToken)) {
        adapter.register_item_tree(ItemTreeRc::borrow_pin(item_tree_rc));
    }
}

/// Free the backend graphics resources allocated by the ItemTree's items.
/// This will be called  if an sub-tree gets destroyed or a popup gets closed, ...
/// It will be called only once not for every sub item
///
/// * `item_tree` - the item tree to unregister
pub fn unregister_item_tree<Base>(
    base: core::pin::Pin<&Base>,
    item_tree: ItemTreeRef,
    item_array: &[vtable::VOffset<Base, ItemVTable, vtable::AllowPin>],
    window_adapter: &WindowAdapterRc,
) {
    item_array.iter().for_each(|item| {
        item.apply_pin(base).as_ref().deinit(window_adapter);
    });
    window_adapter.renderer().free_graphics_resources(item_tree, &mut item_array.iter().map(|item| item.apply_pin(base))).expect(
        "Fatal error encountered when freeing graphics resources while destroying Slint component",
    );

    if let Some(w) = window_adapter.internal(crate::InternalToken) {
        w.unregister_item_tree(item_tree, &mut item_array.iter().map(|item| item.apply_pin(base)));
    }

    // Close popups that were part of a component that just got deleted
    let window_inner = crate::window::WindowInner::from_pub(window_adapter.window());
    let to_close_popups = window_inner
        .active_popups()
        .iter()
        .filter_map(|p| p.parent_item.upgrade().is_none().then_some(p.popup_id))
        .collect::<Vec<_>>();
    for popup_id in to_close_popups {
        window_inner.close_popup(popup_id);
    }
}

fn find_sibling_outside_repeater(
    component: &ItemTreeRc,
    comp_ref_pin: Pin<VRef<ItemTreeVTable>>,
    index: u32,
    sibling_step: &dyn Fn(&crate::item_tree::ItemTreeNodeArray, u32) -> Option<u32>,
    subtree_child: &dyn Fn(usize, usize) -> usize,
) -> Option<ItemRc> {
    assert_ne!(index, 0);

    let item_tree = crate::item_tree::ItemTreeNodeArray::new(&comp_ref_pin);

    let mut current_sibling = index;
    loop {
        current_sibling = sibling_step(&item_tree, current_sibling)?;

        if let Some(node) = step_into_node(
            component,
            &comp_ref_pin,
            current_sibling,
            &item_tree,
            subtree_child,
            &core::convert::identity,
        ) {
            return Some(node);
        }
    }
}

fn step_into_node(
    component: &ItemTreeRc,
    comp_ref_pin: &Pin<VRef<ItemTreeVTable>>,
    node_index: u32,
    item_tree: &crate::item_tree::ItemTreeNodeArray,
    subtree_child: &dyn Fn(usize, usize) -> usize,
    wrap_around: &dyn Fn(ItemRc) -> ItemRc,
) -> Option<ItemRc> {
    match item_tree.get(node_index).expect("Invalid index passed to item tree") {
        crate::item_tree::ItemTreeNode::Item { .. } => {
            Some(ItemRc::new(component.clone(), node_index))
        }
        crate::item_tree::ItemTreeNode::DynamicTree { index, .. } => {
            let range = comp_ref_pin.as_ref().get_subtree_range(*index);
            let component_index = subtree_child(range.start, range.end);
            let mut child_instance = Default::default();
            comp_ref_pin.as_ref().get_subtree(*index, component_index, &mut child_instance);
            child_instance
                .upgrade()
                .map(|child_instance| wrap_around(ItemRc::new_root(child_instance)))
        }
    }
}

pub enum ParentItemTraversalMode {
    FindAllParents,
    StopAtPopups,
}

/// A ItemRc is holding a reference to an ItemTree containing the item, and the index of this item
#[repr(C)]
#[derive(Clone)]
pub struct ItemRc {
    item_tree: vtable::VRc<ItemTreeVTable>,
    index: u32,
}

impl core::fmt::Debug for ItemRc {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        let comp_ref_pin = vtable::VRc::borrow_pin(&self.item_tree);
        let mut debug = SharedString::new();
        comp_ref_pin.as_ref().item_element_infos(self.index, &mut debug);

        write!(f, "ItemRc{{ {:p}, {:?} {debug}}}", comp_ref_pin.as_ptr(), self.index)
    }
}

impl ItemRc {
    /// Create an ItemRc from a ItemTree and an index
    pub fn new(item_tree: vtable::VRc<ItemTreeVTable>, index: u32) -> Self {
        Self { item_tree, index }
    }

    pub fn new_root(item_tree: vtable::VRc<ItemTreeVTable>) -> Self {
        Self { item_tree, index: Self::root_index() }
    }

    #[inline(always)]
    pub const fn root_index() -> u32 {
        0
    }

    #[inline(always)]
    pub fn is_root(&self) -> bool {
        self.index == Self::root_index()
    }

    /// Root within the self item tree and not considering dynamic items
    pub fn is_root_item_of(&self, item_tree: &VRc<ItemTreeVTable>) -> bool {
        self.is_root() && VRc::ptr_eq(&self.item_tree, item_tree)
    }

    /// Return a `Pin<ItemRef<'a>>`
    pub fn borrow<'a>(&'a self) -> Pin<ItemRef<'a>> {
        #![allow(unsafe_code)]
        let comp_ref_pin = vtable::VRc::borrow_pin(&self.item_tree);
        let result = comp_ref_pin.as_ref().get_item_ref(self.index);
        // Safety: we can expand the lifetime of the ItemRef because we know it lives for at least the
        // lifetime of the ItemTree, which is 'a.  Pin::as_ref removes the lifetime, but we can just put it back.
        unsafe { core::mem::transmute::<Pin<ItemRef<'_>>, Pin<ItemRef<'a>>>(result) }
    }
