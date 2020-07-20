use crate::diagnostics::FileDiagnostics;
/// This pass converts the verbose markup used for paths, such as
///    Path {
///        LineTo { ... } ArcTo { ... }
///    }
/// to a vector of path elements (PathData) that is assigned to the
/// elements property of the Path element. That way the generators have to deal
/// with path embedding only as part of the property assignment.
use crate::expression_tree::*;
use crate::object_tree::*;
use crate::typeregister::Type;
use std::rc::Rc;

pub fn compile_paths(
    component: &Rc<Component>,
    tr: &crate::typeregister::TypeRegister,
    diag: &mut FileDiagnostics,
) {
    let path_type = tr.lookup("Path");
    let path_type = path_type.as_builtin();
    let pathlayout_type = tr.lookup("PathLayout");
    let pathlayout_type = pathlayout_type.as_builtin();

    recurse_elem(&component.root_element, &(), &mut |elem_, _| {
        let accepted_type = match &elem_.borrow().base_type {
            crate::typeregister::Type::Builtin(be) if be.class_name == path_type.class_name => {
                path_type
            }
            crate::typeregister::Type::Builtin(be)
                if be.class_name == pathlayout_type.class_name =>
            {
                pathlayout_type
            }
            _ => return,
        };

        let element_types = &accepted_type.additional_accepted_child_types;

        let mut elem = elem_.borrow_mut();

        let path_data = if let Some(commands_expr) = elem.bindings.remove("commands") {
            if let Some(path_child) = elem.children.iter().find(|child| {
                element_types.contains_key(&child.borrow().base_type.as_builtin().class_name)
            }) {
                diag.push_error(
                    "Path elements cannot be mixed with the use of the SVG commands property."
                        .into(),
                    &*path_child.borrow(),
                );
                return;
            }

            let commands = match commands_expr {
                Expression::StringLiteral(commands) => commands,
                _ => {
                    diag.push_error(
                        "The commands property only accepts string literals.".into(),
                        &*elem,
                    );
                    return;
                }
            };

            let path_builder = lyon::path::Path::builder().with_svg();
            let path = lyon::svg::path_utils::build_path(path_builder, &commands);
            match path {
                Ok(path) => Path::Events(path.into_iter().collect()),
                Err(err) => {
                    diag.push_error(format!("Error parsing SVG commands: {:?}", err), &*elem);
                    return;
                }
            }
        } else {
            let new_children = Vec::with_capacity(elem.children.len());
            let old_children = std::mem::replace(&mut elem.children, new_children);

            let mut path_data = Vec::new();

            for child in old_children {
                let element_name = &child.borrow().base_type.as_builtin().class_name.clone();

                if let Some(path_element) = element_types.get(element_name) {
                    let element_type = match path_element {
                        Type::Builtin(b) => b.clone(),
                        _ => panic!(
                            "Incorrect type registry -- expected built-in type for path element {}",
                            element_name
                        ),
                    };
                    let bindings = std::mem::take(&mut child.borrow_mut().bindings);
                    path_data.push(PathElement { element_type, bindings });
                } else {
                    elem.children.push(child);
                }
            }
            crate::expression_tree::Path::Elements(path_data)
        };

        elem.bindings.insert("elements".into(), Expression::PathElements { elements: path_data });
    });
}
