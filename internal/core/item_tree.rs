// Copyright © SixtyFPS GmbH <info@slint-ui.com>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-commercial

// cSpell: ignore xffff

//! This module contains code that helps navigating the tree of item

use crate::component::{ComponentRc, ComponentVTable};
use crate::items::{ItemRef, ItemVTable};
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
            if range.start <= component_index && component_index < range.end {
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

    pub fn downgrade(&self) -> ItemWeak {
        ItemWeak { component: VRc::downgrade(&self.component), index: self.index }
    }

    /// Return the parent Item in the item tree.
    /// This is weak because it can be null if there is no parent
    pub fn parent_item(&self) -> ItemWeak {
        let comp_ref_pin = vtable::VRc::borrow_pin(&self.component);
        let item_tree = crate::item_tree::ComponentItemTree::new(&comp_ref_pin);

        if let Some(parent_index) = item_tree.parent(self.index) {
            return ItemRc::new(self.component.clone(), parent_index).downgrade();
        }

        let mut r = ItemWeak::default();
        comp_ref_pin.as_ref().parent_node(&mut r);
        // parent_item returns the repeater node, go up one more level!
        if let Some(rc) = r.upgrade() {
            r = rc.parent_item();
        }
        r
    }

    // FIXME: This should be nicer/done elsewhere?
    pub fn is_visible(&self) -> bool {
        let item = self.borrow();
        let is_clipping = crate::item_rendering::is_enabled_clipping_item(item);
        let geometry = item.as_ref().geometry();

        if is_clipping && (geometry.width() <= 0.01 as _ || geometry.height() <= 0.01 as _) {
            return false;
        }

        if let Some(parent) = self.parent_item().upgrade() {
            parent.is_visible()
        } else {
            true
        }
    }

    /// Return the index of the item within the component
    pub fn index(&self) -> usize {
        self.index
    }
    /// Returns a reference to the component holding this item
    pub fn component(&self) -> vtable::VRc<ComponentVTable> {
        self.component.clone()
    }

    /// Returns the number of child items for this item. Returns None if
    /// the number is dynamically determined.
    /// TODO: Remove the option when the Subtree trait exists and allows querying
    pub fn children_count(&self) -> Option<u32> {
        let comp_ref_pin = vtable::VRc::borrow_pin(&self.component);
        let item_tree = comp_ref_pin.as_ref().get_item_tree();
        match item_tree.as_slice()[self.index] {
            crate::item_tree::ItemTreeNode::Item { children_count, .. } => Some(children_count),
            crate::item_tree::ItemTreeNode::DynamicTree { .. } => None,
        }
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
                    crate::item_tree::ItemTreeNode::DynamicTree { index, .. } => *index as usize,
                };

                let range = parent_ref_pin.as_ref().get_subtree_range(subtree_index);
                let next_subtree_index = subtree_step(current_component_subtree_index);

                if range.start <= next_subtree_index && next_subtree_index < range.end {
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
        let comp_ref_pin = vtable::VRc::borrow_pin(&self.component);
        let item_tree = crate::item_tree::ComponentItemTree::new(&comp_ref_pin);

        let mut to_focus = self.index();
        loop {
            if let Some(next) = focus_step(&item_tree, to_focus) {
                if let Some(item) = step_into_node(
                    &self.component(),
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
                let root = ItemRc::new(self.component(), 0);
                if let Some(item) = subtree_step(root.clone()) {
                    return step_in(item);
                } else {
                    // Go up a level!
                    if let Some(parent) = root.parent_item().upgrade() {
                        let parent_ref_pin = vtable::VRc::borrow_pin(&parent.component);
                        let parent_item_tree =
                            crate::item_tree::ComponentItemTree::new(&parent_ref_pin);
                        if let Some(next) = step_out(&parent_item_tree, parent.index()) {
                            if let Some(item) = step_into_node(
                                &parent.component(),
                                &parent_ref_pin,
                                next,
                                &parent_item_tree,
                                subtree_child,
                                step_in,
                            ) {
                                return item;
                            }
                            to_focus = next;
                        } else {
                            // Moving out
                            return step_in(root);
                        }
                    } else {
                        return step_in(root);
                    }
                }
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
            &|_, index| Some(index),
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
    visit_internal(
        component,
        order,
        &mut |component, item, index, state| (visitor(component, item, index, state), ()),
        &mut |_, _, _, r| r,
        -1,
        &state,
    )
}

/// Visit each items recursively
///
/// The state parameter returned by the visitor is passed to each child.
///
/// Returns the index of the item that cancelled, or -1 if nobody cancelled
pub fn visit_items_with_post_visit<State, PostVisitState>(
    component: &ComponentRc,
    order: TraversalOrder,
    mut visitor: impl FnMut(
        &ComponentRc,
        Pin<ItemRef>,
        usize,
        &State,
    ) -> (ItemVisitorResult<State>, PostVisitState),
    mut post_visitor: impl FnMut(
        &ComponentRc,
        Pin<ItemRef>,
        PostVisitState,
        VisitChildrenResult,
    ) -> VisitChildrenResult,
    state: State,
) -> VisitChildrenResult {
    visit_internal(component, order, &mut visitor, &mut post_visitor, -1, &state)
}

fn visit_internal<State, PostVisitState>(
    component: &ComponentRc,
    order: TraversalOrder,
    visitor: &mut impl FnMut(
        &ComponentRc,
        Pin<ItemRef>,
        usize,
        &State,
    ) -> (ItemVisitorResult<State>, PostVisitState),
    post_visitor: &mut impl FnMut(
        &ComponentRc,
        Pin<ItemRef>,
        PostVisitState,
        VisitChildrenResult,
    ) -> VisitChildrenResult,
    index: isize,
    state: &State,
) -> VisitChildrenResult {
    let mut actual_visitor = |component: &ComponentRc,
                              index: usize,
                              item: Pin<ItemRef>|
     -> VisitChildrenResult {
        match visitor(component, item, index, state) {
            (ItemVisitorResult::Continue(state), post_visit_state) => {
                let r =
                    visit_internal(component, order, visitor, post_visitor, index as isize, &state);
                post_visitor(component, item, post_visit_state, r)
            }
            (ItemVisitorResult::Abort, post_visit_state) => post_visitor(
                component,
                item,
                post_visit_state,
                VisitChildrenResult::abort(index, 0),
            ),
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
                        TraversalOrder::BackToFront => (*children_index + c),
                        TraversalOrder::FrontToBack => (*children_index + *children_count - c - 1),
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

    use crate::component::{Component, ComponentRc, ComponentVTable, ComponentWeak, IndexRange};
    use crate::layout::{LayoutInfo, Orientation};
    use crate::slice::Slice;

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

        fn layout_info(self: core::pin::Pin<&Self>, _1: Orientation) -> LayoutInfo {
            unimplemented!("Not needed for this test")
        }

        fn subtree_index(self: core::pin::Pin<&Self>) -> usize {
            self.subtree_index
        }

        fn get_subtree_range(self: core::pin::Pin<&Self>, subtree_index: usize) -> IndexRange {
            IndexRange { start: 0, end: self.subtrees.borrow()[subtree_index].len() }
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
    }

    crate::component::ComponentVTable_static!(static TEST_COMPONENT_VT for TestComponent);

    #[test]
    fn test_tree_traversal_one_node() {
        let component = VRc::new(TestComponent {
            parent_component: None,
            item_tree: vec![ItemTreeNode::Item {
                children_count: 0,
                children_index: 1,
                parent_index: 0,
                item_array_index: 0,
            }],
            subtrees: std::cell::RefCell::new(vec![]),
            subtree_index: core::usize::MAX,
        });
        let component = VRc::into_dyn(component);

        let item = ItemRc::new(component.clone(), 0);

        assert!(item.first_child().is_none());
        assert!(item.last_child().is_none());
        assert!(item.previous_sibling().is_none());
        assert!(item.next_sibling().is_none());

        // Wrap the focus around:
        assert!(item.previous_focus_item() == item);
        assert!(item.next_focus_item() == item);
    }

    #[test]
    fn test_tree_traversal_children_nodes() {
        let component = VRc::new(TestComponent {
            parent_component: None,
            item_tree: vec![
                ItemTreeNode::Item {
                    children_count: 3,
                    children_index: 1,
                    parent_index: 0,
                    item_array_index: 0,
                },
                ItemTreeNode::Item {
                    children_count: 0,
                    children_index: 4,
                    parent_index: 0,
                    item_array_index: 1,
                },
                ItemTreeNode::Item {
                    children_count: 0,
                    children_index: 4,
                    parent_index: 0,
                    item_array_index: 2,
                },
                ItemTreeNode::Item {
                    children_count: 0,
                    children_index: 4,
                    parent_index: 0,
                    item_array_index: 3,
                },
            ],
            subtrees: std::cell::RefCell::new(vec![]),
            subtree_index: core::usize::MAX,
        });
        let component = VRc::into_dyn(component);

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
        assert!(fc.parent_item().upgrade() == Some(item.clone()));

        // Examine item between first and last child:
        assert!(fcn == lcp);
        assert!(lcp.parent_item().upgrade() == Some(item.clone()));
        assert!(fcn.previous_sibling().unwrap() == fc);
        assert!(fcn.next_sibling().unwrap() == lc);

        // Examine last child:
        assert!(lc.first_child().is_none());
        assert!(lc.last_child().is_none());
        assert!(lc.next_sibling().is_none());
        assert!(lc.parent_item().upgrade() == Some(item.clone()));

        // Focus traversal:
        let mut cursor = item.clone();

        cursor = cursor.next_focus_item();
        assert!(cursor == fc);

        cursor = cursor.next_focus_item();
        assert!(cursor == fcn);

        cursor = cursor.next_focus_item();
        assert!(cursor == lc);

        cursor = cursor.next_focus_item();
        assert!(cursor == item);

        cursor = cursor.previous_focus_item();
        assert!(cursor == lc);

        cursor = cursor.previous_focus_item();
        assert!(cursor == fcn);

        cursor = cursor.previous_focus_item();
        assert!(cursor == fc);

        cursor = cursor.previous_focus_item();
        assert!(cursor == item);
    }

    #[test]
    fn test_tree_traversal_empty_subtree() {
        let component = vtable::VRc::new(TestComponent {
            parent_component: None,
            item_tree: vec![
                ItemTreeNode::Item {
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
        let component = vtable::VRc::into_dyn(component);

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
    fn test_tree_traversal_item_subtree_item() {
        let component = VRc::new(TestComponent {
            parent_component: None,
            item_tree: vec![
                ItemTreeNode::Item {
                    children_count: 3,
                    children_index: 1,
                    parent_index: 0,
                    item_array_index: 0,
                },
                ItemTreeNode::Item {
                    children_count: 0,
                    children_index: 4,
                    parent_index: 0,
                    item_array_index: 0,
                },
                ItemTreeNode::DynamicTree { index: 0, parent_index: 0 },
                ItemTreeNode::Item {
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
                children_count: 0,
                children_index: 1,
                parent_index: 2,
                item_array_index: 0,
            }],
            subtrees: std::cell::RefCell::new(vec![]),
            subtree_index: 0,
        })]]);

        let component = VRc::into_dyn(component);

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

        assert!(fcn == lcp);
        assert!(!VRc::ptr_eq(&fcn.component(), &item.component()));

        let last = fcn.next_sibling().unwrap();
        assert!(last == lc);

        let first = lcp.previous_sibling().unwrap();
        assert!(first == fc);

        // Focus traversal:
        let mut cursor = item.clone();

        cursor = cursor.next_focus_item();
        assert!(cursor == fc);

        cursor = cursor.next_focus_item();
        assert!(cursor == fcn);

        cursor = cursor.next_focus_item();
        assert!(cursor == lc);

        cursor = cursor.next_focus_item();
        assert!(cursor == item);

        cursor = cursor.previous_focus_item();
        assert!(cursor == lc);

        cursor = cursor.previous_focus_item();
        assert!(cursor == fcn);

        cursor = cursor.previous_focus_item();
        assert!(cursor == fc);

        cursor = cursor.previous_focus_item();
        assert!(cursor == item);
    }

    #[test]
    fn test_tree_traversal_subtrees_item() {
        let component = VRc::new(TestComponent {
            parent_component: None,
            item_tree: vec![
                ItemTreeNode::Item {
                    children_count: 2,
                    children_index: 1,
                    parent_index: 0,
                    item_array_index: 0,
                },
                ItemTreeNode::DynamicTree { index: 0, parent_index: 0 },
                ItemTreeNode::Item {
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
                    children_count: 0,
                    children_index: 1,
                    parent_index: 1,
                    item_array_index: 0,
                }],
                subtrees: std::cell::RefCell::new(vec![]),
                subtree_index: 2,
            }),
        ]]);

        let component = VRc::into_dyn(component);

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

        assert!(sub3.previous_sibling() == Some(sub2.clone()));

        let lc = item.last_child().unwrap();
        assert!(VRc::ptr_eq(&lc.component(), &item.component()));
        assert_eq!(lc.index(), 2);

        assert!(sub3.next_sibling() == Some(lc.clone()));
        assert!(lc.previous_sibling() == Some(sub3.clone()));

        // Focus traversal:
        let mut cursor = item.clone();

        cursor = cursor.next_focus_item();
        assert!(cursor == sub1);

        cursor = cursor.next_focus_item();
        assert!(cursor == sub2);

        cursor = cursor.next_focus_item();
        assert!(cursor == sub3);

        cursor = cursor.next_focus_item();
        assert!(cursor == lc);

        cursor = cursor.next_focus_item();
        assert!(cursor == item);

        cursor = cursor.previous_focus_item();
        assert!(cursor == lc);

        cursor = cursor.previous_focus_item();
        assert!(cursor == sub3);

        cursor = cursor.previous_focus_item();
        assert!(cursor == sub2);

        cursor = cursor.previous_focus_item();
        assert!(cursor == sub1);

        cursor = cursor.previous_focus_item();
        assert!(cursor == item);
    }

    #[test]
    fn test_tree_traversal_item_subtree_child() {
        let component = VRc::new(TestComponent {
            parent_component: None,
            item_tree: vec![
                ItemTreeNode::Item {
                    children_count: 2,
                    children_index: 1,
                    parent_index: 0,
                    item_array_index: 0,
                },
                ItemTreeNode::Item {
                    children_count: 1,
                    children_index: 3,
                    parent_index: 0,
                    item_array_index: 0,
                },
                ItemTreeNode::Item {
                    children_count: 0,
                    children_index: 4,
                    parent_index: 0,
                    item_array_index: 0,
                },
                ItemTreeNode::Item {
                    children_count: 1,
                    children_index: 4,
                    parent_index: 1,
                    item_array_index: 0,
                },
                ItemTreeNode::DynamicTree { index: 0, parent_index: 3 },
            ],
            subtrees: std::cell::RefCell::new(vec![]),
            subtree_index: core::usize::MAX,
        });

        component.as_pin_ref().subtrees.replace(vec![vec![VRc::new(TestComponent {
            parent_component: Some(VRc::into_dyn(component.clone())),
            item_tree: vec![ItemTreeNode::Item {
                children_count: 0,
                children_index: 1,
                parent_index: 4,
                item_array_index: 0,
            }],
            subtrees: std::cell::RefCell::new(vec![]),
            subtree_index: 0,
        })]]);

        let component = VRc::into_dyn(component);

        // Examine root node:
        let item = ItemRc::new(component.clone(), 0);
        assert!(item.previous_sibling().is_none());
        assert!(item.next_sibling().is_none());

        let c1 = item.first_child().unwrap();
        assert_eq!(c1.index(), 1);
        assert!(VRc::ptr_eq(&c1.component(), &item.component()));

        let c2 = c1.next_sibling().unwrap();
        assert_eq!(c2.index(), 2);
        assert!(VRc::ptr_eq(&c2.component(), &item.component()));

        let c1_1 = c1.first_child().unwrap();
        assert_eq!(c1_1.index(), 3);
        assert!(VRc::ptr_eq(&c1_1.component(), &item.component()));

        let sub = c1_1.first_child().unwrap();
        assert_eq!(sub.index(), 0);
        assert!(!VRc::ptr_eq(&sub.component(), &item.component()));

        // Focus traversal:
        let mut cursor = item.clone();

        cursor = cursor.next_focus_item();
        assert!(cursor == c1);

        cursor = cursor.next_focus_item();
        assert!(cursor == c1_1);

        cursor = cursor.next_focus_item();
        assert!(cursor == sub);

        cursor = cursor.next_focus_item();
        assert!(cursor == c2);

        cursor = cursor.next_focus_item();
        assert!(cursor == item);

        cursor = cursor.previous_focus_item();
        assert!(cursor == c2);

        cursor = cursor.previous_focus_item();
        assert!(cursor == sub);

        cursor = cursor.previous_focus_item();
        assert!(cursor == c1_1);

        cursor = cursor.previous_focus_item();
        assert!(cursor == c1);

        cursor = cursor.previous_focus_item();
        assert!(cursor == item);
    }

    #[test]
    fn test_component_item_tree_root_only() {
        let nodes = vec![ItemTreeNode::Item {
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
                children_count: 1,
                children_index: 1,
                parent_index: 0,
                item_array_index: 0,
            },
            ItemTreeNode::Item {
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
                children_count: 3,
                children_index: 1,
                parent_index: 0,
                item_array_index: 0,
            },
            ItemTreeNode::Item {
                children_count: 0,
                children_index: 4,
                parent_index: 0,
                item_array_index: 0,
            },
            ItemTreeNode::Item {
                children_count: 0,
                children_index: 4,
                parent_index: 0,
                item_array_index: 0,
            },
            ItemTreeNode::Item {
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
