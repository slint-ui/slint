/*!
The module responsible for the code generation.

There is one sub module for every language
*/

use crate::lower::{LoweredComponent, LoweredItem};

#[cfg(feature = "cpp")]
mod cpp;

pub fn generate(component: &LoweredComponent) {
    #![allow(unused_variables)]
    #[cfg(feature = "cpp")]
    println!("{}", cpp::generate(component));
}

/// Visit each item in order in which they should appear in the children tree array.
/// The parameter of the visitor are the item, and the first_children_offset
#[allow(dead_code)]
pub fn build_array_helper(
    component: &LoweredComponent,
    mut visit_item: impl FnMut(&LoweredItem, u32),
) {
    let mut children_offset = 1;
    visit_item(&component.root_item, children_offset);
    visit_children(&component.root_item, &mut children_offset, &mut visit_item);

    fn visit_children(
        item: &LoweredItem,
        children_offset: &mut u32,
        visit_item: &mut impl FnMut(&LoweredItem, u32),
    ) {
        for i in &item.children {
            visit_item(i, *children_offset);
            *children_offset += i.children.len() as u32;
        }

        for i in &item.children {
            visit_children(i, children_offset, visit_item);
        }
    }
}
