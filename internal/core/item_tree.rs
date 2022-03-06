// Copyright © SixtyFPS GmbH <info@slint-ui.com>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-commercial

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
pub enum ItemTreeNode<T> {
    /// Static item
    Item {
        /// byte offset where we can find the item (from the *ComponentImpl)
        item: vtable::VOffset<T, ItemVTable, vtable::AllowPin>,

        /// number of children
        children_count: u32,

        /// index of the first children within the item tree
        children_index: u32,

        /// The index of the parent item (not valid for the root)
        parent_index: u32,
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

impl<T> ItemTreeNode<T> {
    pub fn parent_index(&self) -> usize {
        match self {
            ItemTreeNode::Item { parent_index, .. } => *parent_index as usize,
            ItemTreeNode::DynamicTree { parent_index, .. } => *parent_index as usize,
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
