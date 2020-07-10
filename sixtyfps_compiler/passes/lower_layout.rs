//! Passe that compute the layout constraint

use crate::diagnostics::Diagnostics;
use crate::expression_tree::*;
use crate::layout::*;
use crate::object_tree::*;
use std::rc::Rc;

/// Currently this just removes the layout from the tree
pub fn lower_layouts(component: &Rc<Component>, diag: &mut Diagnostics) {
    recurse_elem(&component.root_element, &(), &mut |elem_, _| {
        let mut elem = elem_.borrow_mut();
        let new_children = Vec::with_capacity(elem.children.len());
        let old_children = std::mem::replace(&mut elem.children, new_children);

        for child in old_children {
            let is_grid_layout =
                if let crate::typeregister::Type::Builtin(be) = &child.borrow().base_type {
                    assert!(be.class_name != "Row"); // Caught at element lookup time
                    be.class_name == "GridLayout"
                } else {
                    false
                };

            let is_path_layout =
                if let crate::typeregister::Type::Builtin(be) = &child.borrow().base_type {
                    be.class_name == "PathLayout"
                } else {
                    false
                };

            if is_grid_layout {
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
                        component.optimized_elements.borrow_mut().push(cc.clone());
                    } else {
                        grid.add_element(cc.clone(), row, col, diag);
                        elem.children.push(cc);
                        col += 1;
                    }
                }
                component.optimized_elements.borrow_mut().push(child.clone());
                component.layout_constraints.borrow_mut().grids.push(grid);
                continue;
            } else if is_path_layout {
                let layout_elem = child;
                let layout_children = std::mem::take(&mut layout_elem.borrow_mut().children);
                elem.children.extend(layout_children.iter().cloned());
                component.optimized_elements.borrow_mut().push(layout_elem.clone());
                let path_elements_expr = match layout_elem.borrow_mut().bindings.remove("elements")
                {
                    Some(Expression::PathElements { elements }) => elements,
                    _ => {
                        diag.push_error("Internal error: elements binding in PathLayout does not contain path elements expression".into(), layout_elem.borrow().span());
                        return;
                    }
                };

                let x_reference = Box::new(Expression::PropertyReference(NamedReference {
                    element: Rc::downgrade(&layout_elem),
                    name: "x".into(),
                }));
                let y_reference = Box::new(Expression::PropertyReference(NamedReference {
                    element: Rc::downgrade(&layout_elem),
                    name: "y".into(),
                }));

                component.layout_constraints.borrow_mut().paths.push(PathLayout {
                    elements: layout_children,
                    path: path_elements_expr,
                    x_reference,
                    y_reference,
                });
                continue;
            } else {
                elem.children.push(child);
            }
        }
    });
}

impl GridLayout {
    fn add_element(&mut self, elem: ElementRc, row: usize, col: usize, diag: &mut Diagnostics) {
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
