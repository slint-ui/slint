use crate::abi::datastructures::{
    ComponentRef, ItemRef, ItemTreeNode, ItemVisitor, ItemVisitorVTable,
};

/// Visit each items recursively
///
/// The state parametter returned by the visitor is passed to each children.
pub fn visit_items<State>(
    component: ComponentRef,
    mut visitor: impl FnMut(ComponentRef, ItemRef, &State) -> State,
    state: State,
) {
    visit_internal(component, &mut visitor, -1, &state)
}

fn visit_internal<State>(
    component: ComponentRef,
    visitor: &mut impl FnMut(ComponentRef, ItemRef, &State) -> State,
    index: isize,
    state: &State,
) {
    let mut actual_visitor = |component: ComponentRef, index: isize, item: ItemRef| {
        let s = visitor(component, item, state);
        visit_internal(component, visitor, index, &s);
    };
    vtable::new_vref!(let mut actual_visitor : VRefMut<ItemVisitorVTable> for ItemVisitor = &mut actual_visitor);
    component.visit_children_item(index, actual_visitor);
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
    base: &Base,
    component: ComponentRef,
    item_tree: &[ItemTreeNode<Base>],
    index: isize,
    mut visitor: vtable::VRefMut<ItemVisitorVTable>,
    visit_dynamic: impl Fn(&Base, vtable::VRefMut<ItemVisitorVTable>, usize),
) {
    let mut visit_at_index = |idx: usize| match &item_tree[idx] {
        ItemTreeNode::Item { item, .. } => {
            visitor.visit_item(component, idx as isize, item.apply(base));
        }
        ItemTreeNode::DynamicTree { index } => visit_dynamic(base, visitor.borrow_mut(), *index),
    };
    if index == -1 {
        visit_at_index(0);
    } else {
        match &item_tree[index as usize] {
            ItemTreeNode::Item { children_index, chilren_count, .. } => {
                for c in *children_index..(*children_index + *chilren_count) {
                    visit_at_index(c as usize);
                }
            }
            ItemTreeNode::DynamicTree { .. } => todo!(),
        };
    };
}
