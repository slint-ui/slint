// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

// cSpell: ignore xffff

//! This module contains the ItemTree and code that helps navigating it

use crate::accessibility::{
    AccessibilityAction, AccessibleStringProperty, SupportedAccessibilityAction,
};
use crate::items::{AccessibleRole, ItemRef, ItemVTable};
use crate::layout::{LayoutInfo, Orientation};
use crate::lengths::{LogicalPoint, LogicalRect};
use crate::slice::Slice;
use crate::window::WindowAdapterRc;
use crate::SharedString;
use alloc::vec::Vec;
use core::ops::ControlFlow;
use core::pin::Pin;
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
#[vtable]
#[repr(C)]
pub struct ItemTreeVTable {
    /// Visit the children of the item at index `index`.
    /// Note that the root item is at index 0, so passing 0 would visit the item under root (the children of root).
    /// If you want to visit the root item, you need to pass -1 as an index.
    pub visit_children_item: extern "C" fn(
        core::pin::Pin<VRef<ItemTreeVTable>>,
        index: isize,
        order: TraversalOrder,
        visitor: VRefMut<ItemVisitorVTable>,
    ) -> VisitChildrenResult,

    /// Return a reference to an item using the given index
    pub get_item_ref: extern "C" fn(
        core::pin::Pin<VRef<ItemTreeVTable>>,
        index: u32,
    ) -> core::pin::Pin<VRef<ItemVTable>>,

    /// Return the range of indices below the dynamic `ItemTreeNode` at `index`
    pub get_subtree_range:
        extern "C" fn(core::pin::Pin<VRef<ItemTreeVTable>>, index: u32) -> IndexRange,

    /// Return the `ItemTreeRc` at `subindex` below the dynamic `ItemTreeNode` at `index`
    pub get_subtree: extern "C" fn(
        core::pin::Pin<VRef<ItemTreeVTable>>,
        index: u32,
        subindex: usize,
        result: &mut vtable::VWeak<ItemTreeVTable, Dyn>,
    ),

    /// Return the item tree that is defined by this `ItemTree`.
    /// The return value is an item weak because it can be null if there is no parent.
    /// And the return value is passed by &mut because ItemWeak has a destructor
    pub get_item_tree: extern "C" fn(core::pin::Pin<VRef<ItemTreeVTable>>) -> Slice<ItemTreeNode>,

    /// Return the node this ItemTree is a part of in the parent ItemTree.
    ///
    /// The return value is an item weak because it can be null if there is no parent.
    /// And the return value is passed by &mut because ItemWeak has a destructor
    /// Note that the returned value will typically point to a repeater node, which is
    /// strictly speaking not an Item at all!
    pub parent_node: extern "C" fn(core::pin::Pin<VRef<ItemTreeVTable>>, result: &mut ItemWeak),

    /// This embeds this ItemTree into the item tree of another ItemTree
    ///
    /// Returns `true` if this ItemTree was embedded into the `parent`
    /// at `parent_item_tree_index`.
    pub embed_component: extern "C" fn(
        core::pin::Pin<VRef<ItemTreeVTable>>,
        parent: &VWeak<ItemTreeVTable>,
        parent_item_tree_index: u32,
    ) -> bool,

    /// Return the index of the current subtree or usize::MAX if this is not a subtree
    pub subtree_index: extern "C" fn(core::pin::Pin<VRef<ItemTreeVTable>>) -> usize,

    /// Returns the layout info for the root of the ItemTree
    pub layout_info: extern "C" fn(core::pin::Pin<VRef<ItemTreeVTable>>, Orientation) -> LayoutInfo,

    /// Returns the item's geometry (relative to its parent item)
    pub item_geometry:
        extern "C" fn(core::pin::Pin<VRef<ItemTreeVTable>>, item_index: u32) -> LogicalRect,

    /// Returns the accessible role for a given item
    pub accessible_role:
        extern "C" fn(core::pin::Pin<VRef<ItemTreeVTable>>, item_index: u32) -> AccessibleRole,

    /// Returns the accessible property via the `result`. Returns true if such a property exists.
    pub accessible_string_property: extern "C" fn(
        core::pin::Pin<VRef<ItemTreeVTable>>,
        item_index: u32,
        what: AccessibleStringProperty,
        result: &mut SharedString,
    ) -> bool,

    /// Executes an accessibility action.
    pub accessibility_action: extern "C" fn(
        core::pin::Pin<VRef<ItemTreeVTable>>,
        item_index: u32,
        action: &AccessibilityAction,
    ),

    /// Returns the supported accessibility actions.
    pub supported_accessibility_actions: extern "C" fn(
        core::pin::Pin<VRef<ItemTreeVTable>>,
        item_index: u32,
    ) -> SupportedAccessibilityAction,

    /// Add the `ElementName::id` entries of the given item
    pub item_element_infos: extern "C" fn(
        core::pin::Pin<VRef<ItemTreeVTable>>,
        item_index: u32,
        result: &mut SharedString,
    ) -> bool,

    /// Returns a Window, creating a fresh one if `do_create` is true.
    pub window_adapter: extern "C" fn(
        core::pin::Pin<VRef<ItemTreeVTable>>,
        do_create: bool,
        result: &mut Option<WindowAdapterRc>,
    ),

    /// in-place destructor (for VRc)
    pub drop_in_place: unsafe fn(VRefMut<ItemTreeVTable>) -> vtable::Layout,

    /// dealloc function (for VRc)
    pub dealloc: unsafe fn(&ItemTreeVTable, ptr: *mut u8, layout: vtable::Layout),
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
        adapter.register_item_tree();
    }
}

/// Free the backend graphics resources allocated by the ItemTree's items.
pub fn unregister_item_tree<Base>(
    base: core::pin::Pin<&Base>,
    item_tree: ItemTreeRef,
    item_array: &[vtable::VOffset<Base, ItemVTable, vtable::AllowPin>],
    window_adapter: &WindowAdapterRc,
) {
    window_adapter.renderer().free_graphics_resources(
        item_tree,
        &mut item_array.iter().map(|item| item.apply_pin(base)),
    ).expect("Fatal error encountered when freeing graphics resources while destroying Slint component");
    if let Some(w) = window_adapter.internal(crate::InternalToken) {
        w.unregister_item_tree(item_tree, &mut item_array.iter().map(|item| item.apply_pin(base)));
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
                .map(|child_instance| wrap_around(ItemRc::new(child_instance, 0)))
        }
    }
}

/// A ItemRc is holding a reference to a ItemTree containing the item, and the index of this item
#[repr(C)]
#[derive(Clone, Debug)]
pub struct ItemRc {
    item_tree: vtable::VRc<ItemTreeVTable>,
    index: u32,
}

impl ItemRc {
    /// Create an ItemRc from a ItemTree and an index
    pub fn new(item_tree: vtable::VRc<ItemTreeVTable>, index: u32) -> Self {
        Self { item_tree, index }
    }

    pub fn is_root_item_of(&self, item_tree: &VRc<ItemTreeVTable>) -> bool {
        self.index == 0 && VRc::ptr_eq(&self.item_tree, item_tree)
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

    /// Returns a `VRcMapped` of this item, to conveniently access specialized item API.
    pub fn downcast<T: HasStaticVTable<ItemVTable>>(&self) -> Option<VRcMapped<ItemTreeVTable, T>> {
        #![allow(unsafe_code)]
        let item = self.borrow();
        ItemRef::downcast_pin::<T>(item)?;

        Some(vtable::VRc::map_dyn(self.item_tree.clone(), |comp_ref_pin| {
            let result = comp_ref_pin.as_ref().get_item_ref(self.index);
            // Safety: we can expand the lifetime of the ItemRef because we know it lives for at least the
            // lifetime of the ItemTree, which is 'a.  Pin::as_ref removes the lifetime, but we can just put it back.
            let item =
                unsafe { core::mem::transmute::<Pin<ItemRef<'_>>, Pin<ItemRef<'_>>>(result) };
            ItemRef::downcast_pin::<T>(item).unwrap()
        }))
    }

    pub fn downgrade(&self) -> ItemWeak {
        ItemWeak { item_tree: VRc::downgrade(&self.item_tree), index: self.index }
    }

    /// Return the parent Item in the item tree.
    ///
    /// If the item is a the root on its Window or PopupWindow, then the parent is None.
    pub fn parent_item(&self) -> Option<ItemRc> {
        let comp_ref_pin = vtable::VRc::borrow_pin(&self.item_tree);
        let item_tree = crate::item_tree::ItemTreeNodeArray::new(&comp_ref_pin);

        if let Some(parent_index) = item_tree.parent(self.index) {
            return Some(ItemRc::new(self.item_tree.clone(), parent_index));
        }

        let mut r = ItemWeak::default();
        comp_ref_pin.as_ref().parent_node(&mut r);
        let parent = r.upgrade()?;
        let comp_ref_pin = vtable::VRc::borrow_pin(&parent.item_tree);
        let item_tree_array = crate::item_tree::ItemTreeNodeArray::new(&comp_ref_pin);
        if let Some(ItemTreeNode::DynamicTree { parent_index, .. }) =
            item_tree_array.get(parent.index())
        {
            // parent_node returns the repeater node, go up one more level!
            Some(ItemRc::new(parent.item_tree.clone(), *parent_index))
        } else {
            // the Item was most likely a PopupWindow and we don't want to return the item for the purpose of this call
            // (eg, focus/geometry/...)
            None
        }
    }

    /// Returns true if this item is visible from the root of the item tree. Note that this will return
    /// false for `Clip` elements with the `clip` property evaluating to true.
    pub fn is_visible(&self) -> bool {
        let (clip, geometry) = self.absolute_clip_rect_and_geometry();
        let intersection = geometry.intersection(&clip).unwrap_or_default();
        !intersection.is_empty() || (geometry.is_empty() && clip.contains(geometry.center()))
    }

    /// Returns the clip rect that applies to this item (in window coordinates) as well as the
    /// item's (unclipped) geometry (also in window coordinates).
    fn absolute_clip_rect_and_geometry(&self) -> (LogicalRect, LogicalRect) {
        let (mut clip, parent_geometry) = self.parent_item().map_or_else(
            || {
                (
                    LogicalRect::from_size((crate::Coord::MAX, crate::Coord::MAX).into()),
                    Default::default(),
                )
            },
            |parent| parent.absolute_clip_rect_and_geometry(),
        );

        let geometry = self.geometry().translate(parent_geometry.origin.to_vector());

        let item = self.borrow();
        if crate::item_rendering::is_clipping_item(item) {
            clip = geometry.intersection(&clip).unwrap_or_default();
        }

        (clip, geometry)
    }

    pub fn is_accessible(&self) -> bool {
        let comp_ref_pin = vtable::VRc::borrow_pin(&self.item_tree);
        let item_tree = crate::item_tree::ItemTreeNodeArray::new(&comp_ref_pin);

        if let Some(n) = &item_tree.get(self.index) {
            match n {
                ItemTreeNode::Item { is_accessible, .. } => *is_accessible,
                ItemTreeNode::DynamicTree { .. } => false,
            }
        } else {
            false
        }
    }

    pub fn accessible_role(&self) -> crate::items::AccessibleRole {
        let comp_ref_pin = vtable::VRc::borrow_pin(&self.item_tree);
        comp_ref_pin.as_ref().accessible_role(self.index)
    }

    pub fn accessible_string_property(
        &self,
        what: crate::accessibility::AccessibleStringProperty,
    ) -> Option<SharedString> {
        let comp_ref_pin = vtable::VRc::borrow_pin(&self.item_tree);
        let mut result = Default::default();
        let ok = comp_ref_pin.as_ref().accessible_string_property(self.index, what, &mut result);
        ok.then_some(result)
    }

    pub fn accessible_action(&self, action: &crate::accessibility::AccessibilityAction) {
        let comp_ref_pin = vtable::VRc::borrow_pin(&self.item_tree);
        comp_ref_pin.as_ref().accessibility_action(self.index, action);
    }

    pub fn supported_accessibility_actions(&self) -> SupportedAccessibilityAction {
        let comp_ref_pin = vtable::VRc::borrow_pin(&self.item_tree);
        comp_ref_pin.as_ref().supported_accessibility_actions(self.index)
    }

    pub fn element_count(&self) -> Option<usize> {
        let comp_ref_pin = vtable::VRc::borrow_pin(&self.item_tree);
        let mut result = SharedString::new();
        comp_ref_pin
            .as_ref()
            .item_element_infos(self.index, &mut result)
            .then(|| result.as_str().split("/").count())
    }

    pub fn element_type_names_and_ids(
        &self,
        element_index: usize,
    ) -> Option<Vec<(SharedString, SharedString)>> {
        let comp_ref_pin = vtable::VRc::borrow_pin(&self.item_tree);
        let mut result = SharedString::new();
        comp_ref_pin.as_ref().item_element_infos(self.index, &mut result).then(|| {
            result
                .as_str()
                .split("/")
                .nth(element_index)
                .unwrap()
                .split(";")
                .map(|encoded_elem_info| {
                    let mut decoder = encoded_elem_info.split(',');
                    let type_name = decoder.next().unwrap().into();
                    let id = decoder.next().map(Into::into).unwrap_or_default();
                    (type_name, id)
                })
                .collect()
        })
    }

    pub fn geometry(&self) -> LogicalRect {
        let comp_ref_pin = vtable::VRc::borrow_pin(&self.item_tree);
        comp_ref_pin.as_ref().item_geometry(self.index)
    }

    /// Returns an absolute position of `p` in the parent item coordinate system
    /// (does not add this item's x and y)
    pub fn map_to_window(&self, p: LogicalPoint) -> LogicalPoint {
        let mut current = self.clone();
        let mut result = p;
        while let Some(parent) = current.parent_item() {
            let geometry = parent.geometry();
            result += geometry.origin.to_vector();
            current = parent.clone();
        }
        result
    }

    /// Returns an absolute position of `p` in the `ItemTree`'s coordinate system
    /// (does not add this item's x and y)
    pub fn map_to_item_tree(
        &self,
        p: LogicalPoint,
        item_tree: &vtable::VRc<ItemTreeVTable>,
    ) -> LogicalPoint {
        let mut current = self.clone();
        let mut result = p;
        if current.is_root_item_of(item_tree) {
            return result;
        }
        while let Some(parent) = current.parent_item() {
            if parent.is_root_item_of(item_tree) {
                break;
            }
            let geometry = parent.geometry();
            result += geometry.origin.to_vector();
            current = parent.clone();
        }
        result
    }

    /// Return the index of the item within the ItemTree
    pub fn index(&self) -> u32 {
        self.index
    }
    /// Returns a reference to the ItemTree holding this item
    pub fn item_tree(&self) -> &vtable::VRc<ItemTreeVTable> {
        &self.item_tree
    }

    fn find_child(
        &self,
        child_access: &dyn Fn(&crate::item_tree::ItemTreeNodeArray, u32) -> Option<u32>,
        child_step: &dyn Fn(&crate::item_tree::ItemTreeNodeArray, u32) -> Option<u32>,
        subtree_child: &dyn Fn(usize, usize) -> usize,
    ) -> Option<Self> {
        let comp_ref_pin = vtable::VRc::borrow_pin(&self.item_tree);
        let item_tree = crate::item_tree::ItemTreeNodeArray::new(&comp_ref_pin);

        let mut current_child_index = child_access(&item_tree, self.index())?;
        loop {
            if let Some(item) = step_into_node(
                self.item_tree(),
                &comp_ref_pin,
                current_child_index,
                &item_tree,
                subtree_child,
                &core::convert::identity,
            ) {
                return Some(item);
            }
            current_child_index = child_step(&item_tree, current_child_index)?;
        }
    }

    /// The first child Item of this Item
    pub fn first_child(&self) -> Option<Self> {
        self.find_child(
            &|item_tree, index| item_tree.first_child(index),
            &|item_tree, index| item_tree.next_sibling(index),
            &|start, _| start,
        )
    }

    /// The last child Item of this Item
    pub fn last_child(&self) -> Option<Self> {
        self.find_child(
            &|item_tree, index| item_tree.last_child(index),
            &|item_tree, index| item_tree.previous_sibling(index),
            &|_, end| end.wrapping_sub(1),
        )
    }

    fn find_sibling(
        &self,
        sibling_step: &dyn Fn(&crate::item_tree::ItemTreeNodeArray, u32) -> Option<u32>,
        subtree_step: &dyn Fn(usize) -> usize,
        subtree_child: &dyn Fn(usize, usize) -> usize,
    ) -> Option<Self> {
        let comp_ref_pin = vtable::VRc::borrow_pin(&self.item_tree);
        if self.index == 0 {
            let mut parent_item = Default::default();
            comp_ref_pin.as_ref().parent_node(&mut parent_item);
            let current_component_subtree_index = comp_ref_pin.as_ref().subtree_index();
            if let Some(parent_item) = parent_item.upgrade() {
                let parent = parent_item.item_tree();
                let parent_ref_pin = vtable::VRc::borrow_pin(parent);
                let parent_item_index = parent_item.index();
                let parent_item_tree = crate::item_tree::ItemTreeNodeArray::new(&parent_ref_pin);

                let subtree_index = match parent_item_tree.get(parent_item_index)? {
                    crate::item_tree::ItemTreeNode::Item { .. } => {
                        // Popups can trigger this case!
                        return None;
                    }
                    crate::item_tree::ItemTreeNode::DynamicTree { index, .. } => *index,
                };

                let next_subtree_index = subtree_step(current_component_subtree_index);

                // Get next subtree from repeater!
                let mut next_subtree_instance = Default::default();
                parent_ref_pin.as_ref().get_subtree(
                    subtree_index,
                    next_subtree_index,
                    &mut next_subtree_instance,
                );
                if let Some(next_subtree_instance) = next_subtree_instance.upgrade() {
                    return Some(ItemRc::new(next_subtree_instance, 0));
                }

                // We need to leave the repeater:
                find_sibling_outside_repeater(
                    parent,
                    parent_ref_pin,
                    parent_item_index,
                    sibling_step,
                    subtree_child,
                )
            } else {
                None // At root if the item tree
            }
        } else {
            find_sibling_outside_repeater(
                self.item_tree(),
                comp_ref_pin,
                self.index(),
                sibling_step,
                subtree_child,
            )
        }
    }

    /// The previous sibling of this Item
    pub fn previous_sibling(&self) -> Option<Self> {
        self.find_sibling(
            &|item_tree, index| item_tree.previous_sibling(index),
            &|index| index.wrapping_sub(1),
            &|_, end| end.wrapping_sub(1),
        )
    }

    /// The next sibling of this Item
    pub fn next_sibling(&self) -> Option<Self> {
        self.find_sibling(
            &|item_tree, index| item_tree.next_sibling(index),
            &|index| index.saturating_add(1),
            &|start, _| start,
        )
    }

    fn move_focus(
        &self,
        focus_step: &dyn Fn(&crate::item_tree::ItemTreeNodeArray, u32) -> Option<u32>,
        subtree_step: &dyn Fn(ItemRc) -> Option<ItemRc>,
        subtree_child: &dyn Fn(usize, usize) -> usize,
        step_in: &dyn Fn(ItemRc) -> ItemRc,
        step_out: &dyn Fn(&crate::item_tree::ItemTreeNodeArray, u32) -> Option<u32>,
    ) -> Self {
        let mut component = self.item_tree().clone();
        let mut comp_ref_pin = vtable::VRc::borrow_pin(&self.item_tree);
        let mut item_tree = crate::item_tree::ItemTreeNodeArray::new(&comp_ref_pin);

        let mut to_focus = self.index();

        'in_tree: loop {
            if let Some(next) = focus_step(&item_tree, to_focus) {
                if let Some(item) = step_into_node(
                    &component,
                    &comp_ref_pin,
                    next,
                    &item_tree,
                    subtree_child,
                    step_in,
                ) {
                    return item;
                }
                to_focus = next;
                // Loop: We stepped into an empty repeater!
            } else {
                // Step out of this component:
                let mut root = ItemRc::new(component, 0);
                if let Some(item) = subtree_step(root.clone()) {
                    // Next component inside same repeater
                    return step_in(item);
                }

                // Step out of the repeater
                let root_component = root.item_tree();
                let root_comp_ref = vtable::VRc::borrow_pin(root_component);
                let mut parent_node = Default::default();
                root_comp_ref.as_ref().parent_node(&mut parent_node);

                while let Some(parent) = parent_node.upgrade() {
                    // .. not at the root of the item tree:
                    component = parent.item_tree().clone();
                    comp_ref_pin = vtable::VRc::borrow_pin(&component);
                    item_tree = crate::item_tree::ItemTreeNodeArray::new(&comp_ref_pin);

                    let index = parent.index();

                    if !matches!(item_tree.get(index), Some(ItemTreeNode::DynamicTree { .. })) {
                        // That was not a repeater (eg, a popup window)
                        break;
                    }

                    if let Some(next) = step_out(&item_tree, index) {
                        if let Some(item) = step_into_node(
                            parent.item_tree(),
                            &comp_ref_pin,
                            next,
                            &item_tree,
                            subtree_child,
                            step_in,
                        ) {
                            // Step into a dynamic node
                            return item;
                        } else {
                            // The dynamic node was empty, proceed in normal tree
                            to_focus = parent.index();
                            continue 'in_tree; // Find a node in the current (parent!) tree
                        }
                    }

                    root = ItemRc::new(component.clone(), 0);
                    if let Some(item) = subtree_step(root.clone()) {
                        return step_in(item);
                    }

                    // Go up one more level:
                    let root_component = root.item_tree();
                    let root_comp_ref = vtable::VRc::borrow_pin(root_component);
                    parent_node = Default::default();
                    root_comp_ref.as_ref().parent_node(&mut parent_node);
                }

                // Loop around after hitting the root node:
                return step_in(root);
            }
        }
    }

    /// Move tab focus to the previous item:
    pub fn previous_focus_item(&self) -> Self {
        self.move_focus(
            &|item_tree, index| {
                crate::item_focus::default_previous_in_local_focus_chain(index, item_tree)
            },
            &|root| root.previous_sibling(),
            &|_, end| end.wrapping_sub(1),
            &|root| {
                let mut current = root;
                loop {
                    if let Some(next) = current.last_child() {
                        current = next;
                    } else {
                        return current;
                    }
                }
            },
            &|item_tree, index| item_tree.parent(index),
        )
    }

    /// Move tab focus to the next item:
    pub fn next_focus_item(&self) -> Self {
        self.move_focus(
            &|item_tree, index| {
                crate::item_focus::default_next_in_local_focus_chain(index, item_tree)
            },
            &|root| root.next_sibling(),
            &|start, _| start,
            &core::convert::identity,
            &|item_tree, index| crate::item_focus::step_out_of_node(index, item_tree),
        )
    }

    pub fn window_adapter(&self) -> Option<WindowAdapterRc> {
        let comp_ref_pin = vtable::VRc::borrow_pin(&self.item_tree);
        let mut result = None;
        comp_ref_pin.as_ref().window_adapter(false, &mut result);
        result
    }

    /// Visit the children of this element and call the visitor to each of them, until the visitor returns [`ControlFlow::Break`].
    /// When the visitor breaks, the function returns the value. If it doesn't break, the function returns None.
    fn visit_descendants_impl<R>(
        &self,
        visitor: &mut impl FnMut(&ItemRc) -> ControlFlow<R>,
    ) -> Option<R> {
        let mut result = None;

        let mut actual_visitor = |item_tree: &ItemTreeRc,
                                  index: u32,
                                  _item_pin: core::pin::Pin<ItemRef>|
         -> VisitChildrenResult {
            let item_rc = ItemRc::new(item_tree.clone(), index);

            match visitor(&item_rc) {
                ControlFlow::Continue(_) => {
                    if let Some(x) = item_rc.visit_descendants_impl(visitor) {
                        result = Some(x);
                        return VisitChildrenResult::abort(index, 0);
                    }
                }
                ControlFlow::Break(x) => {
                    result = Some(x);
                    return VisitChildrenResult::abort(index, 0);
                }
            }

            VisitChildrenResult::CONTINUE
        };
        vtable::new_vref!(let mut actual_visitor : VRefMut<ItemVisitorVTable> for ItemVisitor = &mut actual_visitor);

        VRc::borrow_pin(self.item_tree()).as_ref().visit_children_item(
            self.index() as isize,
            TraversalOrder::BackToFront,
            actual_visitor,
        );

        result
    }

    /// Visit the children of this element and call the visitor to each of them, until the visitor returns [`ControlFlow::Break`].
    /// When the visitor breaks, the function returns the value. If it doesn't break, the function returns None.
    pub fn visit_descendants<R>(
        &self,
        mut visitor: impl FnMut(&ItemRc) -> ControlFlow<R>,
    ) -> Option<R> {
        self.visit_descendants_impl(&mut visitor)
    }
}

impl PartialEq for ItemRc {
    fn eq(&self, other: &Self) -> bool {
        VRc::ptr_eq(&self.item_tree, &other.item_tree) && self.index == other.index
    }
}

impl Eq for ItemRc {}

/// A Weak reference to an item that can be constructed from an ItemRc.
#[derive(Clone, Default)]
#[repr(C)]
pub struct ItemWeak {
    item_tree: crate::item_tree::ItemTreeWeak,
    index: u32,
}

impl ItemWeak {
    pub fn upgrade(&self) -> Option<ItemRc> {
        self.item_tree.upgrade().map(|c| ItemRc::new(c, self.index))
    }
}

impl PartialEq for ItemWeak {
    fn eq(&self, other: &Self) -> bool {
        VWeak::ptr_eq(&self.item_tree, &other.item_tree) && self.index == other.index
    }
}

impl Eq for ItemWeak {}

#[repr(u8)]
#[derive(Debug, Copy, Clone, Eq, PartialEq)]
pub enum TraversalOrder {
    BackToFront,
    FrontToBack,
}

/// The return value of the ItemTree::visit_children_item function
///
/// Represents something like `enum { Continue, Aborted{aborted_at_item: isize} }`.
/// But this is just wrapping a int because it is easier to use ffi with isize than
/// complex enum.
///
/// -1 means the visitor will continue
/// otherwise this is the index of the item that aborted the visit.
#[repr(transparent)]
#[derive(Copy, Clone, Eq, PartialEq)]
pub struct VisitChildrenResult(u64);
impl VisitChildrenResult {
    /// The result used for a visitor that want to continue the visit
    pub const CONTINUE: Self = Self(u64::MAX);

    /// Returns a result that means that the visitor must stop, and convey the item that caused the abort
    pub fn abort(item_index: u32, index_within_repeater: usize) -> Self {
        assert!(index_within_repeater < u32::MAX as usize);
        Self(item_index as u64 | (index_within_repeater as u64) << 32)
    }
    /// True if the visitor wants to abort the visit
    pub fn has_aborted(&self) -> bool {
        self.0 != Self::CONTINUE.0
    }
    pub fn aborted_index(&self) -> Option<usize> {
        if self.0 != Self::CONTINUE.0 {
            Some((self.0 & 0xffff_ffff) as usize)
        } else {
            None
        }
    }
    pub fn aborted_indexes(&self) -> Option<(usize, usize)> {
        if self.0 != Self::CONTINUE.0 {
            Some(((self.0 & 0xffff_ffff) as usize, (self.0 >> 32) as usize))
        } else {
            None
        }
    }
}
impl core::fmt::Debug for VisitChildrenResult {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        if self.0 == Self::CONTINUE.0 {
            write!(f, "CONTINUE")
        } else {
            write!(f, "({},{})", (self.0 & 0xffff_ffff) as usize, (self.0 >> 32) as usize)
        }
    }
}

/// The item tree is an array of ItemTreeNode representing a static tree of items
/// within a ItemTree.
#[repr(u8)]
#[derive(Debug)]
pub enum ItemTreeNode {
    /// Static item
    Item {
        /// True when the item has accessibility properties attached
        is_accessible: bool,

        /// number of children
        children_count: u32,

        /// index of the first children within the item tree
        children_index: u32,

        /// The index of the parent item (not valid for the root)
        parent_index: u32,

        /// The index in the extra item_array
        item_array_index: u32,
    },
    /// A placeholder for many instance of item in their own ItemTree which
    /// are instantiated according to a model.
    DynamicTree {
        /// the index which is passed in the visit_dynamic callback.
        index: u32,

        /// The index of the parent item (not valid for the root)
        parent_index: u32,
    },
}

impl ItemTreeNode {
    pub fn parent_index(&self) -> u32 {
        match self {
            ItemTreeNode::Item { parent_index, .. } => *parent_index,
            ItemTreeNode::DynamicTree { parent_index, .. } => *parent_index,
        }
    }
}

/// The `ItemTreeNodeArray` provides tree walking code for the physical ItemTree stored in
/// a `ItemTree` without stitching any inter-ItemTree links together!
pub struct ItemTreeNodeArray<'a> {
    node_array: &'a [ItemTreeNode],
}

impl<'a> ItemTreeNodeArray<'a> {
    /// Create a new `ItemTree` from its raw data.
    pub fn new(comp_ref_pin: &'a Pin<VRef<'a, ItemTreeVTable>>) -> Self {
        Self { node_array: comp_ref_pin.as_ref().get_item_tree().as_slice() }
    }

    /// Get a ItemTreeNode
    pub fn get(&self, index: u32) -> Option<&ItemTreeNode> {
        self.node_array.get(index as usize)
    }

    /// Get the parent of a node, returns `None` if this is the root node of this item tree.
    pub fn parent(&self, index: u32) -> Option<u32> {
        let index = index as usize;
        (index < self.node_array.len() && index != 0).then(|| self.node_array[index].parent_index())
    }

    /// Returns the next sibling or `None` if this is the last sibling.
    pub fn next_sibling(&self, index: u32) -> Option<u32> {
        if let Some(parent_index) = self.parent(index) {
            match self.node_array[parent_index as usize] {
                ItemTreeNode::Item { children_index, children_count, .. } => {
                    (index < (children_count + children_index - 1)).then_some(index + 1)
                }
                ItemTreeNode::DynamicTree { .. } => {
                    unreachable!("Parent in same item tree is a repeater.")
                }
            }
        } else {
            None // No parent, so we have no siblings either:-)
        }
    }

    /// Returns the previous sibling or `None` if this is the first sibling.
    pub fn previous_sibling(&self, index: u32) -> Option<u32> {
        if let Some(parent_index) = self.parent(index) {
            match self.node_array[parent_index as usize] {
                ItemTreeNode::Item { children_index, .. } => {
                    (index > children_index).then_some(index - 1)
                }
                ItemTreeNode::DynamicTree { .. } => {
                    unreachable!("Parent in same item tree is a repeater.")
                }
            }
        } else {
            None // No parent, so we have no siblings either:-)
        }
    }

    /// Returns the first child or `None` if this are no children or the `index`
    /// points to a `DynamicTree`.
    pub fn first_child(&self, index: u32) -> Option<u32> {
        match self.node_array.get(index as usize)? {
            ItemTreeNode::Item { children_index, children_count, .. } => {
                (*children_count != 0).then_some(*children_index as _)
            }
            ItemTreeNode::DynamicTree { .. } => None,
        }
    }

    /// Returns the last child or `None` if this are no children or the `index`
    /// points to an `DynamicTree`.
    pub fn last_child(&self, index: u32) -> Option<u32> {
        match self.node_array.get(index as usize)? {
            ItemTreeNode::Item { children_index, children_count, .. } => {
                if *children_count != 0 {
                    Some(*children_index + *children_count - 1)
                } else {
                    None
                }
            }
            ItemTreeNode::DynamicTree { .. } => None,
        }
    }

    /// Returns the number of nodes in the `ItemTreeNodeArray`
    pub fn node_count(&self) -> usize {
        self.node_array.len()
    }
}

impl<'a> From<&'a [ItemTreeNode]> for ItemTreeNodeArray<'a> {
    fn from(item_tree: &'a [ItemTreeNode]) -> Self {
        Self { node_array: item_tree }
    }
}

#[repr(C)]
#[vtable]
/// Object to be passed in visit_item_children method of the ItemTree.
pub struct ItemVisitorVTable {
    /// Called for each child of the visited item
    ///
    /// The `item_tree` parameter is the ItemTree in which the item live which might not be the same
    /// as the parent's ItemTree.
    /// `index` is to be used again in the visit_item_children function of the ItemTree (the one passed as parameter)
    /// and `item` is a reference to the item itself
    visit_item: fn(
        VRefMut<ItemVisitorVTable>,
        item_tree: &VRc<ItemTreeVTable, vtable::Dyn>,
        index: u32,
        item: Pin<VRef<ItemVTable>>,
    ) -> VisitChildrenResult,
    /// Destructor
    drop: fn(VRefMut<ItemVisitorVTable>),
}

/// Type alias to `vtable::VRefMut<ItemVisitorVTable>`
pub type ItemVisitorRefMut<'a> = vtable::VRefMut<'a, ItemVisitorVTable>;

impl<T: FnMut(&ItemTreeRc, u32, Pin<ItemRef>) -> VisitChildrenResult> ItemVisitor for T {
    fn visit_item(
        &mut self,
        item_tree: &ItemTreeRc,
        index: u32,
        item: Pin<ItemRef>,
    ) -> VisitChildrenResult {
        self(item_tree, index, item)
    }
}
pub enum ItemVisitorResult<State> {
    Continue(State),
    Abort,
}

/// Visit each items recursively
///
/// The state parameter returned by the visitor is passed to each child.
///
/// Returns the index of the item that cancelled, or -1 if nobody cancelled
pub fn visit_items<State>(
    item_tree: &ItemTreeRc,
    order: TraversalOrder,
    mut visitor: impl FnMut(&ItemTreeRc, Pin<ItemRef>, u32, &State) -> ItemVisitorResult<State>,
    state: State,
) -> VisitChildrenResult {
    visit_internal(item_tree, order, &mut visitor, -1, &state)
}

fn visit_internal<State>(
    item_tree: &ItemTreeRc,
    order: TraversalOrder,
    visitor: &mut impl FnMut(&ItemTreeRc, Pin<ItemRef>, u32, &State) -> ItemVisitorResult<State>,
    index: isize,
    state: &State,
) -> VisitChildrenResult {
    let mut actual_visitor =
        |item_tree: &ItemTreeRc, index: u32, item: Pin<ItemRef>| -> VisitChildrenResult {
            match visitor(item_tree, item, index, state) {
                ItemVisitorResult::Continue(state) => {
                    visit_internal(item_tree, order, visitor, index as isize, &state)
                }

                ItemVisitorResult::Abort => VisitChildrenResult::abort(index, 0),
            }
        };
    vtable::new_vref!(let mut actual_visitor : VRefMut<ItemVisitorVTable> for ItemVisitor = &mut actual_visitor);
    VRc::borrow_pin(item_tree).as_ref().visit_children_item(index, order, actual_visitor)
}

/// Visit the children within an array of ItemTreeNode
///
/// The dynamic visitor is called for the dynamic nodes, its signature is
/// `fn(base: &Base, visitor: vtable::VRefMut<ItemVisitorVTable>, dyn_index: usize)`
///
/// FIXME: the design of this use lots of indirection and stack frame in recursive functions
/// Need to check if the compiler is able to optimize away some of it.
/// Possibly we should generate code that directly call the visitor instead
pub fn visit_item_tree<Base>(
    base: Pin<&Base>,
    item_tree: &ItemTreeRc,
    item_tree_array: &[ItemTreeNode],
    index: isize,
    order: TraversalOrder,
    mut visitor: vtable::VRefMut<ItemVisitorVTable>,
    visit_dynamic: impl Fn(
        Pin<&Base>,
        TraversalOrder,
        vtable::VRefMut<ItemVisitorVTable>,
        u32,
    ) -> VisitChildrenResult,
) -> VisitChildrenResult {
    let mut visit_at_index = |idx: u32| -> VisitChildrenResult {
        match &item_tree_array[idx as usize] {
            ItemTreeNode::Item { .. } => {
                let item = crate::items::ItemRc::new(item_tree.clone(), idx);
                visitor.visit_item(item_tree, idx, item.borrow())
            }
            ItemTreeNode::DynamicTree { index, .. } => {
                if let Some(sub_idx) =
                    visit_dynamic(base, order, visitor.borrow_mut(), *index).aborted_index()
                {
                    VisitChildrenResult::abort(idx, sub_idx)
                } else {
                    VisitChildrenResult::CONTINUE
                }
            }
        }
    };
    if index == -1 {
        visit_at_index(0)
    } else {
        match &item_tree_array[index as usize] {
            ItemTreeNode::Item { children_index, children_count, .. } => {
                for c in 0..*children_count {
                    let idx = match order {
                        TraversalOrder::BackToFront => *children_index + c,
                        TraversalOrder::FrontToBack => *children_index + *children_count - c - 1,
                    };
                    let maybe_abort_index = visit_at_index(idx);
                    if maybe_abort_index.has_aborted() {
                        return maybe_abort_index;
                    }
                }
            }
            ItemTreeNode::DynamicTree { .. } => panic!("should not be called with dynamic items"),
        };
        VisitChildrenResult::CONTINUE
    }
}

#[cfg(feature = "ffi")]
pub(crate) mod ffi {
    #![allow(unsafe_code)]

    use super::*;
    use core::ffi::c_void;

    /// Call init() on the ItemVTable of each item in the item array.
    #[no_mangle]
    pub unsafe extern "C" fn slint_register_item_tree(
        item_tree_rc: &ItemTreeRc,
        window_handle: *const crate::window::ffi::WindowAdapterRcOpaque,
    ) {
        let window_adapter = (window_handle as *const WindowAdapterRc).as_ref().cloned();
        super::register_item_tree(item_tree_rc, window_adapter)
    }

    /// Free the backend graphics resources allocated in the item array.
    #[no_mangle]
    pub unsafe extern "C" fn slint_unregister_item_tree(
        component: ItemTreeRefPin,
        item_array: Slice<vtable::VOffset<u8, ItemVTable, vtable::AllowPin>>,
        window_handle: *const crate::window::ffi::WindowAdapterRcOpaque,
    ) {
        let window_adapter = &*(window_handle as *const WindowAdapterRc);
        super::unregister_item_tree(
            core::pin::Pin::new_unchecked(&*(component.as_ptr() as *const u8)),
            core::pin::Pin::into_inner(component),
            item_array.as_slice(),
            window_adapter,
        )
    }

    /// Expose `crate::item_tree::visit_item_tree` to C++
    ///
    /// Safety: Assume a correct implementation of the item_tree array
    #[no_mangle]
    pub unsafe extern "C" fn slint_visit_item_tree(
        item_tree: &ItemTreeRc,
        item_tree_array: Slice<ItemTreeNode>,
        index: isize,
        order: TraversalOrder,
        visitor: VRefMut<ItemVisitorVTable>,
        visit_dynamic: extern "C" fn(
            base: *const c_void,
            order: TraversalOrder,
            visitor: vtable::VRefMut<ItemVisitorVTable>,
            dyn_index: u32,
        ) -> VisitChildrenResult,
    ) -> VisitChildrenResult {
        crate::item_tree::visit_item_tree(
            VRc::as_pin_ref(item_tree),
            item_tree,
            item_tree_array.as_slice(),
            index,
            order,
            visitor,
            |a, b, c, d| visit_dynamic(a.get_ref() as *const vtable::Dyn as *const c_void, b, c, d),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct TestItemTree {
        parent_component: Option<ItemTreeRc>,
        item_tree: Vec<ItemTreeNode>,
        subtrees: std::cell::RefCell<Vec<Vec<vtable::VRc<ItemTreeVTable, TestItemTree>>>>,
        subtree_index: usize,
    }

    impl ItemTree for TestItemTree {
        fn visit_children_item(
            self: core::pin::Pin<&Self>,
            _1: isize,
            _2: crate::item_tree::TraversalOrder,
            _3: vtable::VRefMut<crate::item_tree::ItemVisitorVTable>,
        ) -> crate::item_tree::VisitChildrenResult {
            unimplemented!("Not needed for this test")
        }

        fn get_item_ref(
            self: core::pin::Pin<&Self>,
            _1: u32,
        ) -> core::pin::Pin<vtable::VRef<super::ItemVTable>> {
            unimplemented!("Not needed for this test")
        }

        fn get_item_tree(self: core::pin::Pin<&Self>) -> Slice<ItemTreeNode> {
            Slice::from_slice(&self.get_ref().item_tree)
        }

        fn parent_node(self: core::pin::Pin<&Self>, result: &mut ItemWeak) {
            if let Some(parent_item) = self.parent_component.clone() {
                *result =
                    ItemRc::new(parent_item.clone(), self.item_tree[0].parent_index()).downgrade();
            }
        }

        fn embed_component(
            self: core::pin::Pin<&Self>,
            _parent_component: &ItemTreeWeak,
            _item_tree_index: u32,
        ) -> bool {
            false
        }

        fn layout_info(self: core::pin::Pin<&Self>, _1: Orientation) -> LayoutInfo {
            unimplemented!("Not needed for this test")
        }

        fn subtree_index(self: core::pin::Pin<&Self>) -> usize {
            self.subtree_index
        }

        fn get_subtree_range(self: core::pin::Pin<&Self>, subtree_index: u32) -> IndexRange {
            (0..self.subtrees.borrow()[subtree_index as usize].len()).into()
        }

        fn get_subtree(
            self: core::pin::Pin<&Self>,
            subtree_index: u32,
            component_index: usize,
            result: &mut ItemTreeWeak,
        ) {
            if let Some(vrc) = self.subtrees.borrow()[subtree_index as usize].get(component_index) {
                *result = vtable::VRc::downgrade(&vtable::VRc::into_dyn(vrc.clone()))
            }
        }

        fn accessible_role(self: Pin<&Self>, _: u32) -> AccessibleRole {
            unimplemented!("Not needed for this test")
        }

        fn accessible_string_property(
            self: Pin<&Self>,
            _: u32,
            _: AccessibleStringProperty,
            _: &mut SharedString,
        ) -> bool {
            false
        }

        fn item_element_infos(self: Pin<&Self>, _: u32, _: &mut SharedString) -> bool {
            false
        }

        fn window_adapter(
            self: Pin<&Self>,
            _do_create: bool,
            _result: &mut Option<WindowAdapterRc>,
        ) {
            unimplemented!("Not needed for this test")
        }

        fn item_geometry(self: Pin<&Self>, _: u32) -> LogicalRect {
            unimplemented!("Not needed for this test")
        }

        fn accessibility_action(self: core::pin::Pin<&Self>, _: u32, _: &AccessibilityAction) {
            unimplemented!("Not needed for this test")
        }

        fn supported_accessibility_actions(
            self: core::pin::Pin<&Self>,
            _: u32,
        ) -> SupportedAccessibilityAction {
            unimplemented!("Not needed for this test")
        }
    }

    crate::item_tree::ItemTreeVTable_static!(static TEST_COMPONENT_VT for TestItemTree);

    fn create_one_node_component() -> VRc<ItemTreeVTable, vtable::Dyn> {
        let component = VRc::new(TestItemTree {
            parent_component: None,
            item_tree: vec![ItemTreeNode::Item {
                is_accessible: false,
                children_count: 0,
                children_index: 1,
                parent_index: 0,
                item_array_index: 0,
            }],
            subtrees: std::cell::RefCell::new(vec![]),
            subtree_index: usize::MAX,
        });
        VRc::into_dyn(component)
    }

    #[test]
    fn test_tree_traversal_one_node_structure() {
        let component = create_one_node_component();

        let item = ItemRc::new(component.clone(), 0);

        assert!(item.first_child().is_none());
        assert!(item.last_child().is_none());
        assert!(item.previous_sibling().is_none());
        assert!(item.next_sibling().is_none());
    }

    #[test]
    fn test_tree_traversal_one_node_forward_focus() {
        let component = create_one_node_component();

        let item = ItemRc::new(component.clone(), 0);

        // Wrap the focus around:
        assert_eq!(item.next_focus_item(), item);
    }

    #[test]
    fn test_tree_traversal_one_node_backward_focus() {
        let component = create_one_node_component();

        let item = ItemRc::new(component.clone(), 0);

        // Wrap the focus around:
        assert_eq!(item.previous_focus_item(), item);
    }

    fn create_children_nodes() -> VRc<ItemTreeVTable, vtable::Dyn> {
        let component = VRc::new(TestItemTree {
            parent_component: None,
            item_tree: vec![
                ItemTreeNode::Item {
                    is_accessible: false,
                    children_count: 3,
                    children_index: 1,
                    parent_index: 0,
                    item_array_index: 0,
                },
                ItemTreeNode::Item {
                    is_accessible: false,
                    children_count: 0,
                    children_index: 4,
                    parent_index: 0,
                    item_array_index: 1,
                },
                ItemTreeNode::Item {
                    is_accessible: false,
                    children_count: 0,
                    children_index: 4,
                    parent_index: 0,
                    item_array_index: 2,
                },
                ItemTreeNode::Item {
                    is_accessible: false,
                    children_count: 0,
                    children_index: 4,
                    parent_index: 0,
                    item_array_index: 3,
                },
            ],
            subtrees: std::cell::RefCell::new(vec![]),
            subtree_index: usize::MAX,
        });
        VRc::into_dyn(component)
    }

    #[test]
    fn test_tree_traversal_children_nodes_structure() {
        let component = create_children_nodes();

        // Examine root node:
        let item = ItemRc::new(component.clone(), 0);
        assert!(item.previous_sibling().is_none());
        assert!(item.next_sibling().is_none());

        let fc = item.first_child().unwrap();
        assert_eq!(fc.index(), 1);
        assert!(VRc::ptr_eq(fc.item_tree(), item.item_tree()));

        let fcn = fc.next_sibling().unwrap();
        assert_eq!(fcn.index(), 2);

        let lc = item.last_child().unwrap();
        assert_eq!(lc.index(), 3);
        assert!(VRc::ptr_eq(lc.item_tree(), item.item_tree()));

        let lcp = lc.previous_sibling().unwrap();
        assert!(VRc::ptr_eq(lcp.item_tree(), item.item_tree()));
        assert_eq!(lcp.index(), 2);

        // Examine first child:
        assert!(fc.first_child().is_none());
        assert!(fc.last_child().is_none());
        assert!(fc.previous_sibling().is_none());
        assert_eq!(fc.parent_item().unwrap(), item);

        // Examine item between first and last child:
        assert_eq!(fcn, lcp);
        assert_eq!(lcp.parent_item().unwrap(), item);
        assert_eq!(fcn.previous_sibling().unwrap(), fc);
        assert_eq!(fcn.next_sibling().unwrap(), lc);

        // Examine last child:
        assert!(lc.first_child().is_none());
        assert!(lc.last_child().is_none());
        assert!(lc.next_sibling().is_none());
        assert_eq!(lc.parent_item().unwrap(), item);
    }

    #[test]
    fn test_tree_traversal_children_nodes_forward_focus() {
        let component = create_children_nodes();

        let item = ItemRc::new(component.clone(), 0);
        let fc = item.first_child().unwrap();
        let fcn = fc.next_sibling().unwrap();
        let lc = item.last_child().unwrap();

        let mut cursor = item.clone();

        cursor = cursor.next_focus_item();
        assert_eq!(cursor, fc);

        cursor = cursor.next_focus_item();
        assert_eq!(cursor, fcn);

        cursor = cursor.next_focus_item();
        assert_eq!(cursor, lc);

        cursor = cursor.next_focus_item();
        assert_eq!(cursor, item);
    }

    #[test]
    fn test_tree_traversal_children_nodes_backward_focus() {
        let component = create_children_nodes();

        let item = ItemRc::new(component.clone(), 0);
        let fc = item.first_child().unwrap();
        let fcn = fc.next_sibling().unwrap();
        let lc = item.last_child().unwrap();

        let mut cursor = item.clone();

        cursor = cursor.previous_focus_item();
        assert_eq!(cursor, lc);

        cursor = cursor.previous_focus_item();
        assert_eq!(cursor, fcn);

        cursor = cursor.previous_focus_item();
        assert_eq!(cursor, fc);

        cursor = cursor.previous_focus_item();
        assert_eq!(cursor, item);
    }

    fn create_empty_subtree() -> VRc<ItemTreeVTable, vtable::Dyn> {
        let component = vtable::VRc::new(TestItemTree {
            parent_component: None,
            item_tree: vec![
                ItemTreeNode::Item {
                    is_accessible: false,
                    children_count: 1,
                    children_index: 1,
                    parent_index: 0,
                    item_array_index: 0,
                },
                ItemTreeNode::DynamicTree { index: 0, parent_index: 0 },
            ],
            subtrees: std::cell::RefCell::new(vec![vec![]]),
            subtree_index: usize::MAX,
        });
        vtable::VRc::into_dyn(component)
    }

    #[test]
    fn test_tree_traversal_empty_subtree_structure() {
        let component = create_empty_subtree();

        // Examine root node:
        let item = ItemRc::new(component.clone(), 0);
        assert!(item.previous_sibling().is_none());
        assert!(item.next_sibling().is_none());
        assert!(item.first_child().is_none());
        assert!(item.last_child().is_none());

        // Wrap the focus around:
        assert!(item.previous_focus_item() == item);
        assert!(item.next_focus_item() == item);
    }

    #[test]
    fn test_tree_traversal_empty_subtree_forward_focus() {
        let component = create_empty_subtree();

        // Examine root node:
        let item = ItemRc::new(component.clone(), 0);

        assert!(item.next_focus_item() == item);
    }

    #[test]
    fn test_tree_traversal_empty_subtree_backward_focus() {
        let component = create_empty_subtree();

        // Examine root node:
        let item = ItemRc::new(component.clone(), 0);

        assert!(item.previous_focus_item() == item);
    }

    fn create_item_subtree_item() -> VRc<ItemTreeVTable, vtable::Dyn> {
        let component = VRc::new(TestItemTree {
            parent_component: None,
            item_tree: vec![
                ItemTreeNode::Item {
                    is_accessible: false,
                    children_count: 3,
                    children_index: 1,
                    parent_index: 0,
                    item_array_index: 0,
                },
                ItemTreeNode::Item {
                    is_accessible: false,
                    children_count: 0,
                    children_index: 4,
                    parent_index: 0,
                    item_array_index: 0,
                },
                ItemTreeNode::DynamicTree { index: 0, parent_index: 0 },
                ItemTreeNode::Item {
                    is_accessible: false,
                    children_count: 0,
                    children_index: 4,
                    parent_index: 0,
                    item_array_index: 0,
                },
            ],
            subtrees: std::cell::RefCell::new(vec![]),
            subtree_index: usize::MAX,
        });

        component.as_pin_ref().subtrees.replace(vec![vec![VRc::new(TestItemTree {
            parent_component: Some(VRc::into_dyn(component.clone())),
            item_tree: vec![ItemTreeNode::Item {
                is_accessible: false,
                children_count: 0,
                children_index: 1,
                parent_index: 2,
                item_array_index: 0,
            }],
            subtrees: std::cell::RefCell::new(vec![]),
            subtree_index: 0,
        })]]);

        VRc::into_dyn(component)
    }

    #[test]
    fn test_tree_traversal_item_subtree_item_structure() {
        let component = create_item_subtree_item();

        // Examine root node:
        let item = ItemRc::new(component.clone(), 0);
        assert!(item.previous_sibling().is_none());
        assert!(item.next_sibling().is_none());

        let fc = item.first_child().unwrap();
        assert!(VRc::ptr_eq(fc.item_tree(), item.item_tree()));
        assert_eq!(fc.index(), 1);

        let lc = item.last_child().unwrap();
        assert!(VRc::ptr_eq(lc.item_tree(), item.item_tree()));
        assert_eq!(lc.index(), 3);

        let fcn = fc.next_sibling().unwrap();
        let lcp = lc.previous_sibling().unwrap();

        assert_eq!(fcn, lcp);
        assert!(!VRc::ptr_eq(fcn.item_tree(), item.item_tree()));

        let last = fcn.next_sibling().unwrap();
        assert_eq!(last, lc);

        let first = lcp.previous_sibling().unwrap();
        assert_eq!(first, fc);
    }

    #[test]
    fn test_tree_traversal_item_subtree_item_forward_focus() {
        let component = create_item_subtree_item();

        let item = ItemRc::new(component.clone(), 0);
        let fc = item.first_child().unwrap();
        let lc = item.last_child().unwrap();
        let fcn = fc.next_sibling().unwrap();

        let mut cursor = item.clone();

        cursor = cursor.next_focus_item();
        assert_eq!(cursor, fc);

        cursor = cursor.next_focus_item();
        assert_eq!(cursor, fcn);

        cursor = cursor.next_focus_item();
        assert_eq!(cursor, lc);

        cursor = cursor.next_focus_item();
        assert_eq!(cursor, item);
    }

    #[test]
    fn test_tree_traversal_item_subtree_item_backward_focus() {
        let component = create_item_subtree_item();

        let item = ItemRc::new(component.clone(), 0);
        let fc = item.first_child().unwrap();
        let lc = item.last_child().unwrap();
        let fcn = fc.next_sibling().unwrap();

        let mut cursor = item.clone();

        cursor = cursor.previous_focus_item();
        assert_eq!(cursor, lc);

        cursor = cursor.previous_focus_item();
        assert_eq!(cursor, fcn);

        cursor = cursor.previous_focus_item();
        assert_eq!(cursor, fc);

        cursor = cursor.previous_focus_item();
        assert_eq!(cursor, item);
    }

    fn create_nested_subtrees() -> VRc<ItemTreeVTable, vtable::Dyn> {
        let component = VRc::new(TestItemTree {
            parent_component: None,
            item_tree: vec![
                ItemTreeNode::Item {
                    is_accessible: false,
                    children_count: 3,
                    children_index: 1,
                    parent_index: 0,
                    item_array_index: 0,
                },
                ItemTreeNode::Item {
                    is_accessible: false,
                    children_count: 0,
                    children_index: 4,
                    parent_index: 0,
                    item_array_index: 0,
                },
                ItemTreeNode::DynamicTree { index: 0, parent_index: 0 },
                ItemTreeNode::Item {
                    is_accessible: false,
                    children_count: 0,
                    children_index: 4,
                    parent_index: 0,
                    item_array_index: 0,
                },
            ],
            subtrees: std::cell::RefCell::new(vec![]),
            subtree_index: usize::MAX,
        });

        let sub_component1 = VRc::new(TestItemTree {
            parent_component: Some(VRc::into_dyn(component.clone())),
            item_tree: vec![
                ItemTreeNode::Item {
                    is_accessible: false,
                    children_count: 1,
                    children_index: 1,
                    parent_index: 2,
                    item_array_index: 0,
                },
                ItemTreeNode::DynamicTree { index: 0, parent_index: 0 },
            ],
            subtrees: std::cell::RefCell::new(vec![]),
            subtree_index: usize::MAX,
        });
        let sub_component2 = VRc::new(TestItemTree {
            parent_component: Some(VRc::into_dyn(sub_component1.clone())),
            item_tree: vec![
                ItemTreeNode::Item {
                    is_accessible: false,
                    children_count: 1,
                    children_index: 1,
                    parent_index: 1,
                    item_array_index: 0,
                },
                ItemTreeNode::Item {
                    is_accessible: false,
                    children_count: 0,
                    children_index: 2,
                    parent_index: 0,
                    item_array_index: 0,
                },
            ],
            subtrees: std::cell::RefCell::new(vec![]),
            subtree_index: usize::MAX,
        });

        sub_component1.as_pin_ref().subtrees.replace(vec![vec![sub_component2]]);
        component.as_pin_ref().subtrees.replace(vec![vec![sub_component1]]);

        VRc::into_dyn(component)
    }

    #[test]
    fn test_tree_traversal_nested_subtrees_structure() {
        let component = create_nested_subtrees();

        // Examine root node:
        let item = ItemRc::new(component.clone(), 0);
        assert!(item.previous_sibling().is_none());
        assert!(item.next_sibling().is_none());

        let fc = item.first_child().unwrap();
        assert!(VRc::ptr_eq(fc.item_tree(), item.item_tree()));
        assert_eq!(fc.index(), 1);

        let lc = item.last_child().unwrap();
        assert!(VRc::ptr_eq(lc.item_tree(), item.item_tree()));
        assert_eq!(lc.index(), 3);

        let fcn = fc.next_sibling().unwrap();
        let lcp = lc.previous_sibling().unwrap();

        assert_eq!(fcn, lcp);
        assert!(!VRc::ptr_eq(fcn.item_tree(), item.item_tree()));

        let last = fcn.next_sibling().unwrap();
        assert_eq!(last, lc);

        let first = lcp.previous_sibling().unwrap();
        assert_eq!(first, fc);

        // Nested component:
        let nested_root = fcn.first_child().unwrap();
        assert_eq!(nested_root, fcn.last_child().unwrap());
        assert!(nested_root.next_sibling().is_none());
        assert!(nested_root.previous_sibling().is_none());
        assert!(!VRc::ptr_eq(nested_root.item_tree(), item.item_tree()));
        assert!(!VRc::ptr_eq(nested_root.item_tree(), fcn.item_tree()));

        let nested_child = nested_root.first_child().unwrap();
        assert_eq!(nested_child, nested_root.last_child().unwrap());
        assert!(VRc::ptr_eq(nested_root.item_tree(), nested_child.item_tree()));
    }

    #[test]
    fn test_tree_traversal_nested_subtrees_forward_focus() {
        let component = create_nested_subtrees();

        // Examine root node:
        let item = ItemRc::new(component.clone(), 0);
        let fc = item.first_child().unwrap();
        let fcn = fc.next_sibling().unwrap();
        let lc = item.last_child().unwrap();
        let nested_root = fcn.first_child().unwrap();
        let nested_child = nested_root.first_child().unwrap();

        // Focus traversal:
        let mut cursor = item.clone();

        cursor = cursor.next_focus_item();
        assert_eq!(cursor, fc);

        cursor = cursor.next_focus_item();
        assert_eq!(cursor, fcn);

        cursor = cursor.next_focus_item();
        assert_eq!(cursor, nested_root);

        cursor = cursor.next_focus_item();
        assert_eq!(cursor, nested_child);

        cursor = cursor.next_focus_item();
        assert_eq!(cursor, lc);

        cursor = cursor.next_focus_item();
        assert_eq!(cursor, item);
    }

    #[test]
    fn test_tree_traversal_nested_subtrees_backward_focus() {
        let component = create_nested_subtrees();

        // Examine root node:
        let item = ItemRc::new(component.clone(), 0);
        let fc = item.first_child().unwrap();
        let fcn = fc.next_sibling().unwrap();
        let lc = item.last_child().unwrap();
        let nested_root = fcn.first_child().unwrap();
        let nested_child = nested_root.first_child().unwrap();

        // Focus traversal:
        let mut cursor = item.clone();

        cursor = cursor.previous_focus_item();
        assert_eq!(cursor, lc);

        cursor = cursor.previous_focus_item();
        assert_eq!(cursor, nested_child);

        cursor = cursor.previous_focus_item();
        assert_eq!(cursor, nested_root);

        cursor = cursor.previous_focus_item();
        assert_eq!(cursor, fcn);

        cursor = cursor.previous_focus_item();
        assert_eq!(cursor, fc);

        cursor = cursor.previous_focus_item();
        assert_eq!(cursor, item);
    }

    fn create_subtrees_item() -> VRc<ItemTreeVTable, vtable::Dyn> {
        let component = VRc::new(TestItemTree {
            parent_component: None,
            item_tree: vec![
                ItemTreeNode::Item {
                    is_accessible: false,
                    children_count: 2,
                    children_index: 1,
                    parent_index: 0,
                    item_array_index: 0,
                },
                ItemTreeNode::DynamicTree { index: 0, parent_index: 0 },
                ItemTreeNode::Item {
                    is_accessible: false,
                    children_count: 0,
                    children_index: 4,
                    parent_index: 0,
                    item_array_index: 0,
                },
            ],
            subtrees: std::cell::RefCell::new(vec![]),
            subtree_index: usize::MAX,
        });

        component.as_pin_ref().subtrees.replace(vec![vec![
            VRc::new(TestItemTree {
                parent_component: Some(VRc::into_dyn(component.clone())),
                item_tree: vec![ItemTreeNode::Item {
                    is_accessible: false,
                    children_count: 0,
                    children_index: 1,
                    parent_index: 1,
                    item_array_index: 0,
                }],
                subtrees: std::cell::RefCell::new(vec![]),
                subtree_index: 0,
            }),
            VRc::new(TestItemTree {
                parent_component: Some(VRc::into_dyn(component.clone())),
                item_tree: vec![ItemTreeNode::Item {
                    is_accessible: false,
                    children_count: 0,
                    children_index: 1,
                    parent_index: 1,
                    item_array_index: 0,
                }],
                subtrees: std::cell::RefCell::new(vec![]),
                subtree_index: 1,
            }),
            VRc::new(TestItemTree {
                parent_component: Some(VRc::into_dyn(component.clone())),
                item_tree: vec![ItemTreeNode::Item {
                    is_accessible: false,
                    children_count: 0,
                    children_index: 1,
                    parent_index: 1,
                    item_array_index: 0,
                }],
                subtrees: std::cell::RefCell::new(vec![]),
                subtree_index: 2,
            }),
        ]]);

        VRc::into_dyn(component)
    }

    #[test]
    fn test_tree_traversal_subtrees_item_structure() {
        let component = create_subtrees_item();

        // Examine root node:
        let item = ItemRc::new(component.clone(), 0);
        assert!(item.previous_sibling().is_none());
        assert!(item.next_sibling().is_none());

        let sub1 = item.first_child().unwrap();
        assert_eq!(sub1.index(), 0);
        assert!(!VRc::ptr_eq(sub1.item_tree(), item.item_tree()));

        // assert!(sub1.previous_sibling().is_none());

        let sub2 = sub1.next_sibling().unwrap();
        assert_eq!(sub2.index(), 0);
        assert!(!VRc::ptr_eq(sub1.item_tree(), sub2.item_tree()));
        assert!(!VRc::ptr_eq(item.item_tree(), sub2.item_tree()));

        assert!(sub2.previous_sibling() == Some(sub1.clone()));

        let sub3 = sub2.next_sibling().unwrap();
        assert_eq!(sub3.index(), 0);
        assert!(!VRc::ptr_eq(sub1.item_tree(), sub2.item_tree()));
        assert!(!VRc::ptr_eq(sub2.item_tree(), sub3.item_tree()));
        assert!(!VRc::ptr_eq(item.item_tree(), sub3.item_tree()));

        assert_eq!(sub3.previous_sibling().unwrap(), sub2.clone());
    }

    #[test]
    fn test_component_item_tree_root_only() {
        let nodes = vec![ItemTreeNode::Item {
            is_accessible: false,
            children_count: 0,
            children_index: 1,
            parent_index: 0,
            item_array_index: 0,
        }];

        let tree: ItemTreeNodeArray = (nodes.as_slice()).into();

        assert_eq!(tree.first_child(0), None);
        assert_eq!(tree.last_child(0), None);
        assert_eq!(tree.previous_sibling(0), None);
        assert_eq!(tree.next_sibling(0), None);
        assert_eq!(tree.parent(0), None);
    }

    #[test]
    fn test_component_item_tree_one_child() {
        let nodes = vec![
            ItemTreeNode::Item {
                is_accessible: false,
                children_count: 1,
                children_index: 1,
                parent_index: 0,
                item_array_index: 0,
            },
            ItemTreeNode::Item {
                is_accessible: false,
                children_count: 0,
                children_index: 2,
                parent_index: 0,
                item_array_index: 0,
            },
        ];

        let tree: ItemTreeNodeArray = (nodes.as_slice()).into();

        assert_eq!(tree.first_child(0), Some(1));
        assert_eq!(tree.last_child(0), Some(1));
        assert_eq!(tree.previous_sibling(0), None);
        assert_eq!(tree.next_sibling(0), None);
        assert_eq!(tree.parent(0), None);
        assert_eq!(tree.previous_sibling(1), None);
        assert_eq!(tree.next_sibling(1), None);
        assert_eq!(tree.parent(1), Some(0));
    }

    #[test]
    fn test_component_item_tree_tree_children() {
        let nodes = vec![
            ItemTreeNode::Item {
                is_accessible: false,
                children_count: 3,
                children_index: 1,
                parent_index: 0,
                item_array_index: 0,
            },
            ItemTreeNode::Item {
                is_accessible: false,
                children_count: 0,
                children_index: 4,
                parent_index: 0,
                item_array_index: 0,
            },
            ItemTreeNode::Item {
                is_accessible: false,
                children_count: 0,
                children_index: 4,
                parent_index: 0,
                item_array_index: 0,
            },
            ItemTreeNode::Item {
                is_accessible: false,
                children_count: 0,
                children_index: 4,
                parent_index: 0,
                item_array_index: 0,
            },
        ];

        let tree: ItemTreeNodeArray = (nodes.as_slice()).into();

        assert_eq!(tree.first_child(0), Some(1));
        assert_eq!(tree.last_child(0), Some(3));
        assert_eq!(tree.previous_sibling(0), None);
        assert_eq!(tree.next_sibling(0), None);
        assert_eq!(tree.parent(0), None);

        assert_eq!(tree.previous_sibling(1), None);
        assert_eq!(tree.next_sibling(1), Some(2));
        assert_eq!(tree.parent(1), Some(0));

        assert_eq!(tree.previous_sibling(2), Some(1));
        assert_eq!(tree.next_sibling(2), Some(3));
        assert_eq!(tree.parent(2), Some(0));

        assert_eq!(tree.previous_sibling(3), Some(2));
        assert_eq!(tree.next_sibling(3), None);
        assert_eq!(tree.parent(3), Some(0));
    }
}
