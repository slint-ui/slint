/*!
The module responsible for the code generation.

There is one sub module for every language
*/

use crate::diagnostics::Diagnostics;
use crate::lower::{LoweredComponent, LoweredItem};

#[cfg(feature = "cpp")]
mod cpp;

pub fn generate(component: &LoweredComponent, diag: &mut Diagnostics) {
    #![allow(unused_variables)]
    #[cfg(feature = "cpp")]
    {
        if let Some(output) = cpp::generate(component, diag) {
            println!("{}", output);
        }
    }
}

/// Visit each item in order in which they should appear in the children tree array.
/// The parameter of the visitor are the item, and the first_children_offset
#[allow(dead_code)]
pub fn build_array_helper(
    component: &LoweredComponent,
    mut visit_item: impl FnMut(&LoweredItem, u32),
) {
    visit_item(&component.root_item, 1);
    visit_children(&component.root_item, 1, &mut visit_item);

    fn sub_children_count(item: &LoweredItem) -> usize {
        let mut count = item.children.len();
        for i in &item.children {
            count += sub_children_count(i);
        }
        count
    }

    fn visit_children(
        item: &LoweredItem,
        children_offset: u32,
        visit_item: &mut impl FnMut(&LoweredItem, u32),
    ) {
        let mut offset = children_offset + item.children.len() as u32;
        for i in &item.children {
            visit_item(i, offset);
            offset += sub_children_count(i) as u32;
        }

        let mut offset = children_offset + item.children.len() as u32;
        for i in &item.children {
            visit_children(i, offset, visit_item);
            offset += sub_children_count(i) as u32;
        }
    }
}
