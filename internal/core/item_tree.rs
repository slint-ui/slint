// Copyright © SixtyFPS GmbH <info@slint-ui.com>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-commercial

// cSpell: ignore xffff

use crate::component::{ComponentRc, ComponentVTable};
use crate::items::{ItemRef, ItemVTable};
use core::pin::Pin;
use vtable::*;

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
