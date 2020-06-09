//! Passe that compute the layout constraint

use crate::diagnostics::Diagnostics;

use crate::object_tree::*;
use std::cell::RefCell;
use std::rc::Rc;

/// Currently this just removes the layout from the tree
pub fn lower_layouts(component: &Rc<Component>, diag: &mut Diagnostics) {
    fn lower_layouts_recursively(
        elem: &Rc<RefCell<Element>>,
        component: &Rc<Component>,
        diag: &mut Diagnostics,
    ) {
        for e in &elem.borrow().children {
            lower_layouts_recursively(e, component, diag)
        }
        let mut elem = elem.borrow_mut();
        let new_children = Vec::with_capacity(elem.children.len());
        let old_children = std::mem::replace(&mut elem.children, new_children);

        for child in old_children {
            let is_layout =
                if let crate::typeregister::Type::Builtin(be) = &child.borrow().base_type {
                    be.class_name == "GridLayout" || be.class_name == "Row"
                } else {
                    false
                };

            if is_layout {
                elem.children.append(&mut child.borrow_mut().children);
                component.optimized_elements.borrow_mut().push(child);
                continue;
            } else {
                elem.children.push(child);
            }
        }
    }
    lower_layouts_recursively(&component.root_element, component, diag)
}
