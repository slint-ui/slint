//! This pass make sure that the id of the elements are unique
//!
//! It currently does so by adding a number to the existing id

use crate::object_tree::*;
use std::rc::Rc;

pub fn assign_unique_id(component: &Rc<Component>) {
    let mut count = 0;
    recurse_elem(&component.root_element, &(), &mut |elem, _| {
        count += 1;
        let mut elem_mut = elem.borrow_mut();
        let old_id = if !elem_mut.id.is_empty() { elem_mut.id.as_str() } else { "item" };
        elem_mut.id = format!("{}_{}", old_id, count);
    })
}
