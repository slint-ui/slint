/// This pass converts the verbose markup used for paths, such as
///    Path {
///        LineTo { ... } ArcTo { ... }
///    }
/// to a vector of path elements (PathData) that is assigned to the
/// elements property of the Path element. That way the generators have to deal
/// with path embedding only as part of the property assignment.
use crate::diagnostics::Diagnostics;
use crate::expression_tree::*;
use crate::object_tree::*;
use std::rc::Rc;

pub fn compile_paths(component: &Rc<Component> , diag: &mut Diagnostics) {
    recurse_elem(&component.root_element, &(), &mut |elem_, _| {
        let is_path = if let crate::typeregister::Type::Builtin(be) = &elem_.borrow().base_type {
            assert!(be.class_name != "Row"); // Caught at element lookup time
            be.class_name == "Path"
        } else {
            false
        };
        if !is_path {
            return;
        }

        let mut elem = elem_.borrow_mut();
        let children = std::mem::take(&mut elem.children);

        let path_data: Vec<_> = children
            .iter()
            .map(|child| {
                let child = child.borrow();
                let element_type = &child.base_type.as_builtin().class_name;

                let mut get_float_prop = |name: &str| match child.bindings.get(name) {
                    Some(Expression::NumberLiteral(n)) => *n as f32,
                    None => 0.,
                    _ => {
                        diag.push_error(format!("Property {} needs to be a numeric literal", name), child.span());
                        0.
                    }
                };

                match element_type.as_str() {
                    "LineTo" => {
                        // TODO: use rtti and fields to automate this mapping
                        let x = get_float_prop("x");
                        let y = get_float_prop("y");
                        PathElement::LineTo { x, y }
                    }

                    _ => panic!("Unexpected child {} in a Path element", element_type),
                }
            })
            .collect();

        elem.bindings.insert("elements".into(), Expression::PathElements { elements: path_data });
    });
}
