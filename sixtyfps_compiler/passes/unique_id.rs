//! This pass make sure that the id of the elements are unique
//!
//! It currently does so by adding a number to the existing id

use crate::object_tree::*;
use std::cell::RefCell;
use std::rc::Rc;

pub fn assign_unique_id(component: &Rc<Component>) {
    fn assign_unique_id_recursive(elem: &Rc<RefCell<Element>>, count: &mut usize) {
        *count += 1;
        {
            let mut elem_mut = elem.borrow_mut();
            let old_id = if !elem_mut.id.is_empty() { elem_mut.id.as_str() } else { "item" };
            elem_mut.id = format!("{}_{}", old_id, count);
        }
        for c in &elem.borrow().children {
            assign_unique_id_recursive(c, count);
        }
    }
    let mut count = 0;
    assign_unique_id_recursive(&component.root_element, &mut count)
}
