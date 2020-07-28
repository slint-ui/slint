//! Passe that compute the layout constraint

use crate::diagnostics::BuildDiagnostics;
use crate::expression_tree::*;
use crate::layout::*;
use crate::object_tree::*;
use std::rc::Rc;

/// Currently this just removes the layout from the tree
pub fn lower_layouts(component: &Rc<Component>, diag: &mut BuildDiagnostics) {
    recurse_elem(&component.root_element, &(), &mut |elem_, _| {
        let mut elem = elem_.borrow_mut();
        let new_children = Vec::with_capacity(elem.children.len());
        let old_children = std::mem::replace(&mut elem.children, new_children);

        for child in old_children {
            let is_grid_layout =
                if let crate::typeregister::Type::Builtin(be) = &child.borrow().base_type {
                    assert!(be.native_class.class_name != "Row"); // Caught at element lookup time
                    be.native_class.class_name == "GridLayout"
                } else {
                    false
                };

            let is_path_layout =
                if let crate::typeregister::Type::Builtin(be) = &child.borrow().base_type {
                    be.native_class.class_name == "PathLayout"
                } else {
                    false
                };

            let ref_child = child.clone();
            let prop_ref = move |name: &'static str| {
                Box::new(Expression::PropertyReference(NamedReference {
                    element: Rc::downgrade(&ref_child),
                    name: name.into(),
                }))
            };

            let (x_reference, y_reference) = if is_grid_layout || is_path_layout {
                (prop_ref("x"), prop_ref("y"))
            } else {
                (Box::new(Expression::Invalid), Box::new(Expression::Invalid))
            };

            if is_grid_layout {
                let mut grid = GridLayout {
                    within: elem_.clone(),
                    elems: Default::default(),
                    x_reference,
                    y_reference,
                };
                let mut row = 0;
                let mut col = 0;

                let child_children = std::mem::take(&mut child.borrow_mut().children);
                for cc in child_children {
                    let is_row =
                        if let crate::typeregister::Type::Builtin(be) = &cc.borrow().base_type {
                            be.native_class.class_name == "Row"
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
                if !grid.elems.is_empty() {
                    component.layout_constraints.borrow_mut().grids.push(grid);
                }
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
                        diag.push_error("Internal error: elements binding in PathLayout does not contain path elements expression".into(), &*layout_elem.borrow());
                        return;
                    }
                };

                if layout_children.is_empty() {
                    continue;
                }

                component.layout_constraints.borrow_mut().paths.push(PathLayout {
                    elements: layout_children,
                    path: path_elements_expr,
                    x_reference,
                    y_reference,
                    width_reference: prop_ref("width"),
                    height_reference: prop_ref("height"),
                    offset_reference: prop_ref("offset"),
                });
                continue;
            } else {
                elem.children.push(child);
            }
        }
    });
}

impl GridLayout {
    fn add_element(&mut self, item: ElementRc, row: u16, col: u16, _diag: &mut BuildDiagnostics) {
        self.elems.push(GridLayoutElement { col, row, colspan: 1, rowspan: 1, item });
    }
}
