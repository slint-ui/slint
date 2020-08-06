use crate::abi::datastructures::{ComponentVTable, ItemRef, ItemVTable};
use crate::ComponentRefPin;
use core::pin::Pin;
use vtable::*;

/// The return value of the Component::visit_children_item function
///
/// Represents something like `enum { Continue, Aborted{aborted_at_item: isize} }`.
/// But this is just wrapping a int because it is easier to use ffi with isize than
/// complex enum.
///
/// -1 means the visitor will continue
/// otherwise this is the index of the item that aborted the visit.
#[repr(transparent)]
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub struct VisitChildrenResult(i64);
impl VisitChildrenResult {
    /// The result used for a visitor that want to continue the visit
    pub const CONTINUE: Self = Self(-1);

    /// Returns a result that means that the visitor must stop, and convey the item that caused the abort
    pub fn abort(item_index: usize, index_within_repeater: usize) -> Self {
        assert!(item_index < i32::MAX as usize);
        assert!(index_within_repeater < i32::MAX as usize);
        Self(item_index as i64 | (index_within_repeater as i64) << 32)
    }
    /// True if the visitor wants to abort the visit
    pub fn has_aborted(&self) -> bool {
        self.0 != -1
    }
    pub fn aborted_index(&self) -> Option<usize> {
        if self.0 != -1 {
            Some((self.0 & 0xffff_ffff) as usize)
        } else {
            None
        }
    }
}

/// The item tree is an array of ItemTreeNode representing a static tree of items
/// within a component.
#[repr(u8)]
pub enum ItemTreeNode<T> {
    /// Static item
    Item {
        /// byte offset where we can find the item (from the *ComponentImpl)
        item: vtable::VOffset<T, ItemVTable, vtable::PinnedFlag>,

        /// number of children
        chilren_count: u32,

        /// index of the first children within the item tree
        children_index: u32,
    },
    /// A placeholder for many instance of item in their own component which
    /// are instantiated according to a model.
    DynamicTree {
        /// the undex which is passed in the visit_dynamic callback.
        index: usize,
    },
}

#[repr(C)]
#[vtable]
/// Object to be passed in visit_item_children method of the Component.
pub struct ItemVisitorVTable {
    /// Called for each children of the visited item
    ///
    /// The `component` parameter is the component in which the item live which might not be the same
    /// as the parent's component.
    /// `index` is to be used again in the visit_item_children function of the Component (the one passed as parameter)
    /// and `item` is a reference to the item itself
    ///
    /// returns true to continue, or false to abort the visit
    visit_item: fn(
        VRefMut<ItemVisitorVTable>,
        component: Pin<VRef<ComponentVTable>>,
        index: isize,
        item: Pin<VRef<ItemVTable>>,
    ) -> bool,
    /// Destructor
    drop: fn(VRefMut<ItemVisitorVTable>),
}

/// Type alias to `vtable::VRefMut<ItemVisitorVTable>`
pub type ItemVisitorRefMut<'a> = vtable::VRefMut<'a, ItemVisitorVTable>;

impl<T: FnMut(crate::ComponentRefPin, isize, Pin<ItemRef>) -> bool> ItemVisitor for T {
    fn visit_item(
        &mut self,
        component: crate::ComponentRefPin,
        index: isize,
        item: Pin<ItemRef>,
    ) -> bool {
        self(component, index, item)
    }
}

pub(crate) mod ffi {
    #![allow(unsafe_code)]

    use super::*;
    use crate::slice::Slice;

    /// Expose `crate::item_tree::visit_item_tree` to C++
    ///
    /// Safety: Assume a correct implementation of the item_tree array
    #[no_mangle]
    pub unsafe extern "C" fn sixtyfps_visit_item_tree(
        component: Pin<VRef<ComponentVTable>>,
        item_tree: Slice<ItemTreeNode<u8>>,
        index: isize,
        visitor: VRefMut<ItemVisitorVTable>,
        visit_dynamic: extern "C" fn(
            base: &u8,
            visitor: vtable::VRefMut<ItemVisitorVTable>,
            dyn_index: usize,
        ) -> VisitChildrenResult,
    ) -> VisitChildrenResult {
        crate::item_tree::visit_item_tree(
            Pin::new_unchecked(&*(component.as_ptr() as *const u8)),
            component,
            item_tree.as_slice(),
            index,
            visitor,
            |a, b, c| visit_dynamic(a.get_ref(), b, c),
        )
    }
}

pub enum ItemVisitorResult<State> {
    Continue(State),
    Abort,
}

/// Visit each items recursively
///
/// The state parametter returned by the visitor is passed to each children.
///
/// Returns the index of the item that cancelled, or -1 if nobody cancelled
pub fn visit_items<State>(
    component: ComponentRefPin,
    mut visitor: impl FnMut(ComponentRefPin, Pin<ItemRef>, &State) -> ItemVisitorResult<State>,
    state: State,
) -> isize {
    visit_internal(component, &mut visitor, -1, &state)
}

fn visit_internal<State>(
    component: ComponentRefPin,
    visitor: &mut impl FnMut(ComponentRefPin, Pin<ItemRef>, &State) -> ItemVisitorResult<State>,
    index: isize,
    state: &State,
) -> isize {
    let mut result = -1;
    let mut actual_visitor =
        |component: ComponentRefPin, index: isize, item: Pin<ItemRef>| -> bool {
            match visitor(component, item, state) {
                ItemVisitorResult::Continue(state) => {
                    result = visit_internal(component, visitor, index, &state);
                    result == -1
                }
                ItemVisitorResult::Abort => {
                    result = index;
                    false
                }
            }
        };
    vtable::new_vref!(let mut actual_visitor : VRefMut<ItemVisitorVTable> for ItemVisitor = &mut actual_visitor);
    component.as_ref().visit_children_item(index, actual_visitor);
    result
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
    component: ComponentRefPin,
    item_tree: &[ItemTreeNode<Base>],
    index: isize,
    mut visitor: vtable::VRefMut<ItemVisitorVTable>,
    visit_dynamic: impl Fn(Pin<&Base>, vtable::VRefMut<ItemVisitorVTable>, usize) -> VisitChildrenResult,
) -> VisitChildrenResult {
    let mut visit_at_index = |idx: usize| -> VisitChildrenResult {
        match &item_tree[idx] {
            ItemTreeNode::Item { item, .. } => {
                if visitor.visit_item(component, idx as isize, item.apply_pin(base)) {
                    VisitChildrenResult::CONTINUE
                } else {
                    VisitChildrenResult::abort(idx, 0)
                }
            }
            ItemTreeNode::DynamicTree { index } => {
                if let Some(sub_idx) =
                    visit_dynamic(base, visitor.borrow_mut(), *index).aborted_index()
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
            ItemTreeNode::Item { children_index, chilren_count, .. } => {
                for c in *children_index..(*children_index + *chilren_count) {
                    let maybe_abort_index = visit_at_index(c as usize);
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
