/*!
The module responsible for the code generation.

There is one sub module for every language
*/

use crate::diagnostics::Diagnostics;
use crate::object_tree::{Component, Element};
use core::cell::RefCell;
use std::rc::Rc;

#[cfg(feature = "cpp")]
mod cpp;

pub fn generate(component: &Component, diag: &mut Diagnostics) {
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
    component: &Component,
    mut visit_item: impl FnMut(&Rc<RefCell<Element>>, u32),
) {
    visit_item(&component.root_element, 1);
    visit_children(&component.root_element.borrow(), 1, &mut visit_item);

    fn sub_children_count(item: &Element) -> usize {
        let mut count = item.children.len();
        for i in &item.children {
            count += sub_children_count(&i.borrow());
        }
        count
    }

    fn visit_children(
        item: &Element,
        children_offset: u32,
        visit_item: &mut impl FnMut(&Rc<RefCell<Element>>, u32),
    ) {
        let mut offset = children_offset + item.children.len() as u32;
        for i in &item.children {
            visit_item(i, offset);
            let child = &i.borrow();
            offset += sub_children_count(child) as u32;
        }

        let mut offset = children_offset + item.children.len() as u32;
        for i in &item.children {
            let child = &i.borrow();
            visit_children(child, offset, visit_item);
            offset += sub_children_count(child) as u32;
        }
    }
}
