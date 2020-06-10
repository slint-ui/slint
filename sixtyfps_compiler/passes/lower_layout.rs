//! Passe that compute the layout constraint

use crate::diagnostics::Diagnostics;

use crate::layout::*;
use crate::object_tree::*;
use std::cell::RefCell;
use std::rc::Rc;

/// Currently this just removes the layout from the tree
pub fn lower_layouts(component: &Rc<Component>, diag: &mut Diagnostics) {
    fn lower_layouts_recursively(
        elem_: &Rc<RefCell<Element>>,
        component: &Rc<Component>,
        diag: &mut Diagnostics,
    ) {
        let mut elem = elem_.borrow_mut();
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
                let mut grid = GridLayout { within: elem_.clone(), elems: Default::default() };
                let mut row = 0;
                let mut col = 0;

                let child_children = std::mem::take(&mut child.borrow_mut().children);
                for cc in child_children {
                    let is_row =
                        if let crate::typeregister::Type::Builtin(be) = &cc.borrow().base_type {
                            be.class_name == "Row"
                        } else {
                            false
                        };
                    if is_row {
                        if col > 0 {
                            row += 1;
                            col = 0;
                        }
                        for x in &cc.borrow().children {
                            grid.add_element(x.clone(), row, col, diag);
                            col += 1;
                        }
                        elem.children.append(&mut cc.borrow_mut().children);
                    } else {
                        grid.add_element(cc.clone(), row, col, diag);
                        elem.children.push(cc);
                        col += 1;
                    }
                }
                component.optimized_elements.borrow_mut().push(child);
                component.layout_constraints.borrow_mut().0.push(grid);
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

impl GridLayout {
    fn add_element(
        &mut self,
        elem: Rc<RefCell<Element>>,
        row: usize,
        col: usize,
        diag: &mut Diagnostics,
    ) {
        fn index_checked<T: Default>(vec: &mut Vec<T>, idx: usize) -> &mut T {
            if vec.len() <= idx {
                vec.resize_with(idx + 1, T::default)
            }
            &mut vec[idx]
        };

        let row_vec = index_checked(&mut self.elems, row);
        let cell = index_checked(row_vec, col);
        if cell.is_some() {
            diag.push_error(format!("Multiple elements in the same cell"), elem.borrow().span())
        }
        *cell = Some(elem)
    }
}
