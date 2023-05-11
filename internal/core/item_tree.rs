// Copyright © SixtyFPS GmbH <info@slint-ui.com>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-commercial

// cSpell: ignore xffff

//! This module contains code that helps navigating the tree of item

use crate::component::{ComponentRc, ComponentVTable};
use crate::items::{ItemRef, ItemVTable};
use crate::lengths::{LogicalPoint, LogicalRect};
use crate::SharedString;
use core::pin::Pin;
use vtable::*;

fn find_sibling_outside_repeater(
    component: crate::component::ComponentRc,
    comp_ref_pin: Pin<VRef<ComponentVTable>>,
    index: usize,
    sibling_step: &dyn Fn(&crate::item_tree::ComponentItemTree, usize) -> Option<usize>,
    subtree_child: &dyn Fn(usize, usize) -> usize,
) -> Option<ItemRc> {
    assert_ne!(index, 0);

    let item_tree = crate::item_tree::ComponentItemTree::new(&comp_ref_pin);

    let mut current_sibling = index;
    loop {
        current_sibling = sibling_step(&item_tree, current_sibling)?;

        if let Some(node) = step_into_node(
            &component,
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
    component: &crate::component::ComponentRc,
    comp_ref_pin: &Pin<VRef<ComponentVTable>>,
    node_index: usize,
    item_tree: &crate::item_tree::ComponentItemTree,
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
            if core::ops::Range::from(range).contains(&component_index) {
                let mut child_component = Default::default();
                comp_ref_pin.as_ref().get_subtree_component(
                    *index,
                    component_index,
                    &mut child_component,
                );
                let child_component = child_component.upgrade().unwrap();
                Some(wrap_around(ItemRc::new(child_component, 0)))
            } else {
                None
            }
        }
    }
}

/// A ItemRc is holding a reference to a component containing the item, and the index of this item
#[repr(C)]
#[derive(Clone, Debug)]
pub struct ItemRc {
    component: vtable::VRc<ComponentVTable>,
    index: usize,
}

impl ItemRc {
    /// Create an ItemRc from a component and an index
    pub fn new(component: vtable::VRc<ComponentVTable>, index: usize) -> Self {
        Self { component, index }
    }

    /// Return a `Pin<ItemRef<'a>>`
    pub fn borrow<'a>(&'a self) -> Pin<ItemRef<'a>> {
        #![allow(unsafe_code)]
        let comp_ref_pin = vtable::VRc::borrow_pin(&self.component);
        let result = comp_ref_pin.as_ref().get_item_ref(self.index);
        // Safety: we can expand the lifetime of the ItemRef because we know it lives for at least the
        // lifetime of the component, which is 'a.  Pin::as_ref removes the lifetime, but we can just put it back.
        unsafe { core::mem::transmute::<Pin<ItemRef<'_>>, Pin<ItemRef<'a>>>(result) }
    }

    /// Returns a `VRcMapped` of this item, to conveniently access specialized item API.
    pub fn downcast<'a, T: HasStaticVTable<ItemVTable>>(
        &'a self,
    ) -> Option<VRcMapped<ComponentVTable, T>> {
        #![allow(unsafe_code)]
        let item = self.borrow();
        if ItemRef::downcast_pin::<T>(item).is_none() {
            return None;
        }

        Some(vtable::VRc::map_dyn(self.component.clone(), |comp_ref_pin| {
            let result = comp_ref_pin.as_ref().get_item_ref(self.index);
            // Safety: we can expand the lifetime of the ItemRef because we know it lives for at least the
            // lifetime of the component, which is 'a.  Pin::as_ref removes the lifetime, but we can just put it back.
            let item =
                unsafe { core::mem::transmute::<Pin<ItemRef<'_>>, Pin<ItemRef<'_>>>(result) };
            ItemRef::downcast_pin::<T>(item).unwrap()
        }))
    }

    pub fn downgrade(&self) -> ItemWeak {
        ItemWeak { component: VRc::downgrade(&self.component), index: self.index }
    }

    /// Return the parent Item in the item tree.
    pub fn parent_item(&self) -> Option<ItemRc> {
        let comp_ref_pin = vtable::VRc::borrow_pin(&self.component);
        let item_tree = crate::item_tree::ComponentItemTree::new(&comp_ref_pin);

        if let Some(parent_index) = item_tree.parent(self.index) {
            return Some(ItemRc::new(self.component.clone(), parent_index));
        }

        let mut r = ItemWeak::default();
        comp_ref_pin.as_ref().parent_node(&mut r);
        // parent_node returns the repeater node, go up one more level!
        r.upgrade()?.parent_item()
    }

    // FIXME: This should be nicer/done elsewhere?
    pub fn is_visible(&self) -> bool {
        let item = self.borrow();
        let is_clipping = crate::item_rendering::is_clipping_item(item);
        let geometry = item.as_ref().geometry();

        if is_clipping && (geometry.width() <= 0.01 as _ || geometry.height() <= 0.01 as _) {
            return false;
        }

        if let Some(parent) = self.parent_item() {
            parent.is_visible()
        } else {
            true
        }
    }

    pub fn is_accessible(&self) -> bool {
        let comp_ref_pin = vtable::VRc::borrow_pin(&self.component);
        let item_tree = crate::item_tree::ComponentItemTree::new(&comp_ref_pin);

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
        let comp_ref_pin = vtable::VRc::borrow_pin(&self.component);
        comp_ref_pin.as_ref().accessible_role(self.index)
    }

    pub fn accessible_string_property(
        &self,
        what: crate::accessibility::AccessibleStringProperty,
    ) -> SharedString {
        let comp_ref_pin = vtable::VRc::borrow_pin(&self.component);
        let mut result = Default::default();
        comp_ref_pin.as_ref().accessible_string_property(self.index, what, &mut result);
        result
    }

    pub fn geometry(&self) -> LogicalRect {
        self.borrow().as_ref().geometry()
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
        return result;
    }

    /// Return the index of the item within the component
    pub fn index(&self) -> usize {
        self.index
    }
    /// Returns a reference to the component holding this item
    pub fn component(&self) -> vtable::VRc<ComponentVTable> {
        self.component.clone()
    }

    fn find_child(
        &self,
        child_access: &dyn Fn(&crate::item_tree::ComponentItemTree, usize) -> Option<usize>,
        child_step: &dyn Fn(&crate::item_tree::ComponentItemTree, usize) -> Option<usize>,
        subtree_child: &dyn Fn(usize, usize) -> usize,
    ) -> Option<Self> {
        let comp_ref_pin = vtable::VRc::borrow_pin(&self.component);
        let item_tree = crate::item_tree::ComponentItemTree::new(&comp_ref_pin);

        let mut current_child_index = child_access(&item_tree, self.index())?;
        loop {
            if let Some(item) = step_into_node(
                &self.component(),
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
        sibling_step: &dyn Fn(&crate::item_tree::ComponentItemTree, usize) -> Option<usize>,
        subtree_step: &dyn Fn(usize) -> usize,
        subtree_child: &dyn Fn(usize, usize) -> usize,
    ) -> Option<Self> {
        let comp_ref_pin = vtable::VRc::borrow_pin(&self.component);
        if self.index == 0 {
            let mut parent_item = Default::default();
            comp_ref_pin.as_ref().parent_node(&mut parent_item);
            let current_component_subtree_index = comp_ref_pin.as_ref().subtree_index();
            if let Some(parent_item) = parent_item.upgrade() {
                let parent = parent_item.component();
                let parent_ref_pin = vtable::VRc::borrow_pin(&parent);
                let parent_item_index = parent_item.index();
                let parent_item_tree = crate::item_tree::ComponentItemTree::new(&parent_ref_pin);

                let subtree_index = match parent_item_tree.get(parent_item_index)? {
                    crate::item_tree::ItemTreeNode::Item { .. } => {
                        panic!("Got an Item, expected a repeater!")
                    }
                    crate::item_tree::ItemTreeNode::DynamicTree { index, .. } => *index,
                };

                let range = parent_ref_pin.as_ref().get_subtree_range(subtree_index);
                let next_subtree_index = subtree_step(current_component_subtree_index);

                if core::ops::Range::from(range).contains(&next_subtree_index) {
                    // Get next subtree from repeater!
                    let mut next_subtree_component = Default::default();
                    parent_ref_pin.as_ref().get_subtree_component(
                        subtree_index,
                        next_subtree_index,
                        &mut next_subtree_component,
                    );
                    let next_subtree_component = next_subtree_component.upgrade().unwrap();
                    return Some(ItemRc::new(next_subtree_component, 0));
                }

                // We need to leave the repeater:
                find_sibling_outside_repeater(
                    parent.clone(),
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
                self.component(),
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
        focus_step: &dyn Fn(&crate::item_tree::ComponentItemTree, usize) -> Option<usize>,
        subtree_step: &dyn Fn(ItemRc) -> Option<ItemRc>,
        subtree_child: &dyn Fn(usize, usize) -> usize,
        step_in: &dyn Fn(ItemRc) -> ItemRc,
        step_out: &dyn Fn(&crate::item_tree::ComponentItemTree, usize) -> Option<usize>,
    ) -> Self {
        let mut component = self.component();
        let mut comp_ref_pin = vtable::VRc::borrow_pin(&self.component);
        let mut item_tree = crate::item_tree::ComponentItemTree::new(&comp_ref_pin);

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
                let root_component = root.component();
                let root_comp_ref = vtable::VRc::borrow_pin(&root_component);
                let mut parent_node = Default::default();
                root_comp_ref.as_ref().parent_node(&mut parent_node);

                while let Some(parent) = parent_node.upgrade() {
                    // .. not at the root of the item tree:
                    component = parent.component();
                    comp_ref_pin = vtable::VRc::borrow_pin(&component);
                    item_tree = crate::item_tree::ComponentItemTree::new(&comp_ref_pin);

                    let index = parent.index();

                    if let Some(next) = step_out(&item_tree, index) {
                        if let Some(item) = step_into_node(
                            &parent.component(),
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

                    root = ItemRc::new(component, 0);
                    if let Some(item) = subtree_step(root.clone()) {
                        return step_in(item);
                    }

                    // Go up one more level:
                    let root_component = root.component();
                    let root_comp_ref = vtable::VRc::borrow_pin(&root_component);
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
}

impl PartialEq for ItemRc {
    fn eq(&self, other: &Self) -> bool {
        VRc::ptr_eq(&self.component, &other.component) && self.index == other.index
    }
}

impl Eq for ItemRc {}

/// A Weak reference to an item that can be constructed from an ItemRc.
#[derive(Clone, Default)]
#[repr(C)]
pub struct ItemWeak {
    component: crate::component::ComponentWeak,
    index: usize,
}

impl ItemWeak {
    pub fn upgrade(&self) -> Option<ItemRc> {
        self.component.upgrade().map(|c| ItemRc::new(c, self.index))
    }
}

impl PartialEq for ItemWeak {
    fn eq(&self, other: &Self) -> bool {
        VWeak::ptr_eq(&self.component, &other.component) && self.index == other.index
    }
}

impl Eq for ItemWeak {}

#[repr(u8)]
#[derive(Debug, Copy, Clone, Eq, PartialEq)]
pub enum TraversalOrder {
    BackToFront,
    FrontToBack,
}

/// The return value of the Component::visit_children_item function
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
    pub fn abort(item_index: usize, index_within_repeater: usize) -> Self {
        assert!(item_index < u32::MAX as usize);
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
/// within a component.
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
    /// A placeholder for many instance of item in their own component which
    /// are instantiated according to a model.
    DynamicTree {
        /// the index which is passed in the visit_dynamic callback.
        index: usize,

        /// The index of the parent item (not valid for the root)
        parent_index: u32,
    },
}

impl ItemTreeNode {
    pub fn parent_index(&self) -> usize {
        match self {
            ItemTreeNode::Item { parent_index, .. } => *parent_index as usize,
            ItemTreeNode::DynamicTree { parent_index, .. } => *parent_index as usize,
        }
    }
}

/// The `ComponentItemTree` provides tree walking code for the physical ItemTree stored in
/// a `Component` without stitching any inter-Component links together!
pub struct ComponentItemTree<'a> {
    item_tree: &'a [ItemTreeNode],
}

impl<'a> ComponentItemTree<'a> {
    /// Create a new `ItemTree` from its raw data.
    pub fn new(comp_ref_pin: &'a Pin<VRef<'a, ComponentVTable>>) -> Self {
        Self { item_tree: comp_ref_pin.as_ref().get_item_tree().as_slice() }
    }

    /// Get a ItemTreeNode
    pub fn get(&self, index: usize) -> Option<&ItemTreeNode> {
        self.item_tree.get(index)
    }

    /// Get the parent of a node, returns `None` if this is the root node of this item tree.
    pub fn parent(&self, index: usize) -> Option<usize> {
        (index < self.item_tree.len() && index != 0).then(|| self.item_tree[index].parent_index())
    }

    /// Returns the next sibling or `None` if this is the last sibling.
    pub fn next_sibling(&self, index: usize) -> Option<usize> {
        if let Some(parent_index) = self.parent(index) {
            match self.item_tree[parent_index] {
                ItemTreeNode::Item { children_index, children_count, .. } => (index
                    < (children_count as usize + children_index as usize - 1))
                    .then(|| index + 1),
                ItemTreeNode::DynamicTree { .. } => {
                    unreachable!("Parent in same item tree is a repeater.")
                }
            }
        } else {
            None // No parent, so we have no siblings either:-)
        }
    }

    /// Returns the previous sibling or `None` if this is the first sibling.
    pub fn previous_sibling(&self, index: usize) -> Option<usize> {
        if let Some(parent_index) = self.parent(index) {
            match self.item_tree[parent_index] {
                ItemTreeNode::Item { children_index, .. } => {
                    (index > children_index as usize).then(|| index - 1)
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
    pub fn first_child(&self, index: usize) -> Option<usize> {
        match self.item_tree.get(index)? {
            ItemTreeNode::Item { children_index, children_count, .. } => {
                (*children_count != 0).then(|| *children_index as _)
            }
            ItemTreeNode::DynamicTree { .. } => None,
        }
    }

    /// Returns the last child or `None` if this are no children or the `index`
    /// points to an `DynamicTree`.
    pub fn last_child(&self, index: usize) -> Option<usize> {
        match self.item_tree.get(index)? {
            ItemTreeNode::Item { children_index, children_count, .. } => (*children_count != 0)
                .then(|| *children_index as usize + *children_count as usize - 1),
            ItemTreeNode::DynamicTree { .. } => None,
        }
    }

    /// Returns the number of nodes in the `ComponentItemTree`
    pub fn node_count(&self) -> usize {
        self.item_tree.len()
    }
}

impl<'a> From<&'a [ItemTreeNode]> for ComponentItemTree<'a> {
    fn from(item_tree: &'a [ItemTreeNode]) -> Self {
        Self { item_tree }
    }
}

#[repr(C)]
#[vtable]
/// Object to be passed in visit_item_children method of the Component.
pub struct ItemVisitorVTable {
    /// Called for each child of the visited item
    ///
    /// The `component` parameter is the component in which the item live which might not be the same
    /// as the parent's component.
    /// `index` is to be used again in the visit_item_children function of the Component (the one passed as parameter)
    /// and `item` is a reference to the item itself
    visit_item: fn(
        VRefMut<ItemVisitorVTable>,
        component: &VRc<ComponentVTable, vtable::Dyn>,
        index: usize,
        item: Pin<VRef<ItemVTable>>,
    ) -> VisitChildrenResult,
    /// Destructor
    drop: fn(VRefMut<ItemVisitorVTable>),
}

/// Type alias to `vtable::VRefMut<ItemVisitorVTable>`
pub type ItemVisitorRefMut<'a> = vtable::VRefMut<'a, ItemVisitorVTable>;

impl<T: FnMut(&ComponentRc, usize, Pin<ItemRef>) -> VisitChildrenResult> ItemVisitor for T {
    fn visit_item(
        &mut self,
        component: &ComponentRc,
        index: usize,
        item: Pin<ItemRef>,
    ) -> VisitChildrenResult {
        self(component, index, item)
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
    component: &ComponentRc,
    order: TraversalOrder,
    mut visitor: impl FnMut(&ComponentRc, Pin<ItemRef>, usize, &State) -> ItemVisitorResult<State>,
    state: State,
) -> VisitChildrenResult {
    visit_internal(component, order, &mut visitor, -1, &state)
}

fn visit_internal<State>(
    component: &ComponentRc,
    order: TraversalOrder,
    visitor: &mut impl FnMut(&ComponentRc, Pin<ItemRef>, usize, &State) -> ItemVisitorResult<State>,
    index: isize,
    state: &State,
) -> VisitChildrenResult {
    let mut actual_visitor =
        |component: &ComponentRc, index: usize, item: Pin<ItemRef>| -> VisitChildrenResult {
            match visitor(component, item, index, state) {
                ItemVisitorResult::Continue(state) => {
                    visit_internal(component, order, visitor, index as isize, &state)
                }

                ItemVisitorResult::Abort => VisitChildrenResult::abort(index, 0),
            }
        };
    vtable::new_vref!(let mut actual_visitor : VRefMut<ItemVisitorVTable> for ItemVisitor = &mut actual_visitor);
    VRc::borrow_pin(component).as_ref().visit_children_item(index, order, actual_visitor)
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
    component: &ComponentRc,
    item_tree: &[ItemTreeNode],
    index: isize,
    order: TraversalOrder,
    mut visitor: vtable::VRefMut<ItemVisitorVTable>,
    visit_dynamic: impl Fn(
        Pin<&Base>,
        TraversalOrder,
        vtable::VRefMut<ItemVisitorVTable>,
        usize,
    ) -> VisitChildrenResult,
) -> VisitChildrenResult {
    let mut visit_at_index = |idx: usize| -> VisitChildrenResult {
        match &item_tree[idx] {
            ItemTreeNode::Item { .. } => {
                let item = crate::items::ItemRc::new(component.clone(), idx);
                visitor.visit_item(component, idx, item.borrow())
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
        match &item_tree[index as usize] {
            ItemTreeNode::Item { children_index, children_count, .. } => {
                for c in 0..*children_count {
                    let idx = match order {
                        TraversalOrder::BackToFront => *children_index + c,
                        TraversalOrder::FrontToBack => *children_index + *children_count - c - 1,
                    } as usize;
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
    use crate::slice::Slice;

    /// Expose `crate::item_tree::visit_item_tree` to C++
    ///
    /// Safety: Assume a correct implementation of the item_tree array
    #[no_mangle]
    pub unsafe extern "C" fn slint_visit_item_tree(
        component: &ComponentRc,
        item_tree: Slice<ItemTreeNode>,
        index: isize,
        order: TraversalOrder,
        visitor: VRefMut<ItemVisitorVTable>,
        visit_dynamic: extern "C" fn(
            base: &u8,
            order: TraversalOrder,
            visitor: vtable::VRefMut<ItemVisitorVTable>,
            dyn_index: usize,
        ) -> VisitChildrenResult,
    ) -> VisitChildrenResult {
        crate::item_tree::visit_item_tree(
            Pin::new_unchecked(&*(&**component as *const Dyn as *const u8)),
            component,
            item_tree.as_slice(),
            index,
            order,
            visitor,
            |a, b, c, d| visit_dynamic(a.get_ref(), b, c, d),
        )
    }
}

#[cfg(test)]
mod tests {
    #![allow(unsafe_code)]

    use super::*;

    use crate::accessibility::AccessibleStringProperty;
    use crate::component::{Component, ComponentRc, ComponentVTable, ComponentWeak, IndexRange};
    use crate::items::AccessibleRole;
    use crate::layout::{LayoutInfo, Orientation};
    use crate::slice::Slice;
    use crate::SharedString;

    use vtable::VRc;

    struct TestComponent {
        parent_component: Option<ComponentRc>,
        item_tree: Vec<ItemTreeNode>,
        subtrees: std::cell::RefCell<Vec<Vec<vtable::VRc<ComponentVTable, TestComponent>>>>,
        subtree_index: usize,
    }

    impl Component for TestComponent {
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
            _1: usize,
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

        fn set_parent_node(
            self: core::pin::Pin<&Self>,
            _parent_component: Option<&ComponentWeak>,
            _item_tree_index: usize,
        ) {
            unimplemented!();
        }

        fn layout_info(self: core::pin::Pin<&Self>, _1: Orientation) -> LayoutInfo {
            unimplemented!("Not needed for this test")
        }

        fn subtree_index(self: core::pin::Pin<&Self>) -> usize {
            self.subtree_index
        }

        fn get_subtree_range(self: core::pin::Pin<&Self>, subtree_index: usize) -> IndexRange {
            (0..self.subtrees.borrow()[subtree_index].len()).into()
        }

        fn get_subtree_component(
            self: core::pin::Pin<&Self>,
            subtree_index: usize,
            component_index: usize,
            result: &mut ComponentWeak,
        ) {
            *result = vtable::VRc::downgrade(&vtable::VRc::into_dyn(
                self.subtrees.borrow()[subtree_index][component_index].clone(),
            ))
        }

        fn accessible_role(self: Pin<&Self>, _: usize) -> AccessibleRole {
            unimplemented!("Not needed for this test")
        }

        fn accessible_string_property(
            self: Pin<&Self>,
            _: usize,
            _: AccessibleStringProperty,
            _: &mut SharedString,
        ) {
        }
    }

    crate::component::ComponentVTable_static!(static TEST_COMPONENT_VT for TestComponent);

    fn create_one_node_component() -> VRc<ComponentVTable, vtable::Dyn> {
        let component = VRc::new(TestComponent {
            parent_component: None,
            item_tree: vec![ItemTreeNode::Item {
                is_accessible: false,
                children_count: 0,
                children_index: 1,
                parent_index: 0,
                item_array_index: 0,
            }],
            subtrees: std::cell::RefCell::new(vec![]),
            subtree_index: core::usize::MAX,
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

    fn create_children_nodes() -> VRc<ComponentVTable, vtable::Dyn> {
        let component = VRc::new(TestComponent {
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
            subtree_index: core::usize::MAX,
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
        assert!(VRc::ptr_eq(&fc.component(), &item.component()));

        let fcn = fc.next_sibling().unwrap();
        assert_eq!(fcn.index(), 2);

        let lc = item.last_child().unwrap();
        assert_eq!(lc.index(), 3);
        assert!(VRc::ptr_eq(&lc.component(), &item.component()));

        let lcp = lc.previous_sibling().unwrap();
        assert!(VRc::ptr_eq(&lcp.component(), &item.component()));
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

    fn create_empty_subtree() -> VRc<ComponentVTable, vtable::Dyn> {
        let component = vtable::VRc::new(TestComponent {
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
            subtree_index: core::usize::MAX,
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

    fn create_item_subtree_item() -> VRc<ComponentVTable, vtable::Dyn> {
        let component = VRc::new(TestComponent {
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
            subtree_index: core::usize::MAX,
        });

        component.as_pin_ref().subtrees.replace(vec![vec![VRc::new(TestComponent {
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
        assert!(VRc::ptr_eq(&fc.component(), &item.component()));
        assert_eq!(fc.index(), 1);

        let lc = item.last_child().unwrap();
        assert!(VRc::ptr_eq(&lc.component(), &item.component()));
        assert_eq!(lc.index(), 3);

        let fcn = fc.next_sibling().unwrap();
        let lcp = lc.previous_sibling().unwrap();

        assert_eq!(fcn, lcp);
        assert!(!VRc::ptr_eq(&fcn.component(), &item.component()));

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

    fn create_nested_subtrees() -> VRc<ComponentVTable, vtable::Dyn> {
        let component = VRc::new(TestComponent {
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
            subtree_index: core::usize::MAX,
        });

        let sub_component1 = VRc::new(TestComponent {
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
            subtree_index: core::usize::MAX,
        });
        let sub_component2 = VRc::new(TestComponent {
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
            subtree_index: core::usize::MAX,
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
        assert!(VRc::ptr_eq(&fc.component(), &item.component()));
        assert_eq!(fc.index(), 1);

        let lc = item.last_child().unwrap();
        assert!(VRc::ptr_eq(&lc.component(), &item.component()));
        assert_eq!(lc.index(), 3);

        let fcn = fc.next_sibling().unwrap();
        let lcp = lc.previous_sibling().unwrap();

        assert_eq!(fcn, lcp);
        assert!(!VRc::ptr_eq(&fcn.component(), &item.component()));

        let last = fcn.next_sibling().unwrap();
        assert_eq!(last, lc);

        let first = lcp.previous_sibling().unwrap();
        assert_eq!(first, fc);

        // Nested component:
        let nested_root = fcn.first_child().unwrap();
        assert_eq!(nested_root, fcn.last_child().unwrap());
        assert!(nested_root.next_sibling().is_none());
        assert!(nested_root.previous_sibling().is_none());
        assert!(!VRc::ptr_eq(&nested_root.component(), &item.component()));
        assert!(!VRc::ptr_eq(&nested_root.component(), &fcn.component()));

        let nested_child = nested_root.first_child().unwrap();
        assert_eq!(nested_child, nested_root.last_child().unwrap());
        assert!(VRc::ptr_eq(&nested_root.component(), &nested_child.component()));
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

    fn create_subtrees_item() -> VRc<ComponentVTable, vtable::Dyn> {
        let component = VRc::new(TestComponent {
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
            subtree_index: core::usize::MAX,
        });

        component.as_pin_ref().subtrees.replace(vec![vec![
            VRc::new(TestComponent {
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
            VRc::new(TestComponent {
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
            VRc::new(TestComponent {
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
        assert!(!VRc::ptr_eq(&sub1.component(), &item.component()));

        // assert!(sub1.previous_sibling().is_none());

        let sub2 = sub1.next_sibling().unwrap();
        assert_eq!(sub2.index(), 0);
        assert!(!VRc::ptr_eq(&sub1.component(), &sub2.component()));
        assert!(!VRc::ptr_eq(&item.component(), &sub2.component()));

        assert!(sub2.previous_sibling() == Some(sub1.clone()));

        let sub3 = sub2.next_sibling().unwrap();
        assert_eq!(sub3.index(), 0);
        assert!(!VRc::ptr_eq(&sub1.component(), &sub2.component()));
        assert!(!VRc::ptr_eq(&sub2.component(), &sub3.component()));
        assert!(!VRc::ptr_eq(&item.component(), &sub3.component()));

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

        let tree: ComponentItemTree = (nodes.as_slice()).into();

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

        let tree: ComponentItemTree = (nodes.as_slice()).into();

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

        let tree: ComponentItemTree = (nodes.as_slice()).into();

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
