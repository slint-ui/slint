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
        let mut elem = elem.borrow_mut();
        let new_children = Vec::with_capacity(elem.children.len());
        let old_children = std::mem::replace(&mut elem.children, new_children);

        for child in old_children {
            let is_layout =
                if let crate::typeregister::Type::Builtin(be) = &child.borrow().base_type {
                    if be.class_name == "Row" {
                        diag.push_error(
                            "Row can only be within a GridLayout element".to_owned(),
                            child.borrow().span(),
                        )
                    }
                    be.class_name == "GridLayout"
                } else {
                    false
                };

            if is_layout {
                let child_children = std::mem::take(&mut child.borrow_mut().children);
                for cc in child_children {
                    let is_row =
                        if let crate::typeregister::Type::Builtin(be) = &cc.borrow().base_type {
                            be.class_name == "Row"
                        } else {
                            false
                        };
                    if is_row {
                        // TODO: add the constraints
                        elem.children.append(&mut cc.borrow_mut().children);
                    } else {
                        // TODO: add the constraints
                        elem.children.push(cc);
                    }
                }
                component.optimized_elements.borrow_mut().push(child);
                continue;
            } else {
                elem.children.push(child);
            }
        }
        for e in &elem.children {
            lower_layouts_recursively(e, component, diag)
        }
    }
    lower_layouts_recursively(&component.root_element, component, diag)
}
