use crate::abi::datastructures::{ComponentVTable, ItemRef, ItemVTable};
use crate::ComponentRefPin;
use core::pin::Pin;
use vtable::*;

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
        ) -> isize,
    ) -> isize {
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
    visit_dynamic: impl Fn(Pin<&Base>, vtable::VRefMut<ItemVisitorVTable>, usize) -> isize,
) -> isize {
    let mut visit_at_index = |idx: usize| -> isize {
        match &item_tree[idx] {
            ItemTreeNode::Item { item, .. } => {
                if visitor.visit_item(component, idx as isize, item.apply_pin(base)) {
                    -1
                } else {
                    idx as isize
                }
            }
            ItemTreeNode::DynamicTree { index } => {
                let sub_idx = visit_dynamic(base, visitor.borrow_mut(), *index);
                if sub_idx == -1 {
                    -1
                } else {
                    idx as isize | sub_idx << 16
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
                    if maybe_abort_index != -1 {
                        return maybe_abort_index;
                    }
                }
            }
            ItemTreeNode::DynamicTree { .. } => panic!("should not be called with dynamic items"),
        };
        -1
    }
}
