// Copyright © SixtyFPS GmbH <info@sixtyfps.io>
// SPDX-License-Identifier: (GPL-3.0-only OR LicenseRef-SixtyFPS-commercial)

//! This pass converts the verbose markup used for paths, such as
//!    Path {
//!        LineTo { ... } ArcTo { ... }
//!    }
//! to a vector of path elements (PathData) that is assigned to the
//! elements property of the Path element. That way the generators have to deal
//! with path embedding only as part of the property assignment.

use crate::diagnostics::BuildDiagnostics;
use crate::expression_tree::*;
use crate::langtype::Type;
use crate::object_tree::*;
use std::cell::RefCell;
use std::rc::Rc;

pub fn compile_paths(
    component: &Rc<Component>,
    tr: &crate::typeregister::TypeRegister,
    diag: &mut BuildDiagnostics,
) {
    let path_type = tr.lookup("Path");
    let path_type = path_type.as_builtin();
    let pathlayout_type = tr.lookup("PathLayout");
    let pathlayout_type = pathlayout_type.as_builtin();

    recurse_elem(&component.root_element, &(), &mut |elem_, _| {
        let accepted_type = match &elem_.borrow().base_type {
            Type::Builtin(be)
                if be.native_class.class_name == path_type.native_class.class_name =>
            {
                path_type
            }
            Type::Builtin(be)
                if be.native_class.class_name == pathlayout_type.native_class.class_name =>
            {
                pathlayout_type
            }
            _ => return,
        };

        let element_types = &accepted_type.additional_accepted_child_types;

        let mut elem = elem_.borrow_mut();

        let path_data = if let Some(commands_expr) =
            elem.bindings.remove("commands").map(RefCell::into_inner)
        {
            if let Some(path_child) = elem.children.iter().find(|child| {
                element_types
                    .contains_key(&child.borrow().base_type.as_builtin().native_class.class_name)
            }) {
                diag.push_error(
                    "Path elements cannot be mixed with the use of the SVG commands property"
                        .into(),
                    &*path_child.borrow(),
                );
                return;
            }

            let commands = match &commands_expr.expression {
                Expression::StringLiteral(commands) => commands,
                _ => {
                    diag.push_error(
                        "The commands property only accepts string literals".into(),
                        &*elem,
                    );
                    return;
                }
            };

            let path_builder = lyon_path::Path::builder().with_svg();
            let path = lyon_svg::path_utils::build_path(path_builder, commands);
            match path {
                Ok(path) => {
                    let event_enum = crate::typeregister::PATH_EVENT_ENUM.with(|e| e.clone());
                    let point_type = Type::Struct {
                        fields: IntoIterator::into_iter([
                            ("x".to_owned(), Type::Float32),
                            ("y".to_owned(), Type::Float32),
                        ])
                        .collect(),
                        name: Some("Point".into()),
                        node: None,
                    };

                    let mut points = Vec::new();
                    let events = path
                        .into_iter()
                        .map(|event| {
                            Expression::EnumerationValue(match event {
                                lyon_path::Event::Begin { at } => {
                                    points.push(at);
                                    event_enum.clone().try_value_from_string("begin").unwrap()
                                }
                                lyon_path::Event::Line { from, to } => {
                                    points.push(from);
                                    points.push(to);

                                    event_enum.clone().try_value_from_string("line").unwrap()
                                }
                                lyon_path::Event::Quadratic { from, ctrl, to } => {
                                    points.push(from);
                                    points.push(ctrl);
                                    points.push(to);

                                    event_enum.clone().try_value_from_string("quadratic").unwrap()
                                }
                                lyon_path::Event::Cubic { from, ctrl1, ctrl2, to } => {
                                    points.push(from);
                                    points.push(ctrl1);
                                    points.push(ctrl2);
                                    points.push(to);
                                    event_enum.clone().try_value_from_string("cubic").unwrap()
                                }
                                lyon_path::Event::End { first: _, last: _, close } => {
                                    if close {
                                        event_enum
                                            .clone()
                                            .try_value_from_string("end_closed")
                                            .unwrap()
                                    } else {
                                        event_enum
                                            .clone()
                                            .try_value_from_string("end_open")
                                            .unwrap()
                                    }
                                }
                            })
                        })
                        .collect();

                    let points = points
                        .into_iter()
                        .map(|point| Expression::Struct {
                            ty: point_type.clone(),
                            values: IntoIterator::into_iter([
                                (
                                    "x".to_owned(),
                                    Expression::NumberLiteral(point.x as _, Unit::None),
                                ),
                                (
                                    "y".to_owned(),
                                    Expression::NumberLiteral(point.y as _, Unit::None),
                                ),
                            ])
                            .collect(),
                        })
                        .collect();

                    Path::Events(events, points)
                }
                Err(_) => {
                    diag.push_error("Error parsing SVG commands".into(), &commands_expr);
                    return;
                }
            }
        } else {
            let new_children = Vec::with_capacity(elem.children.len());
            let old_children = std::mem::replace(&mut elem.children, new_children);

            let mut path_data = Vec::new();

            for child in old_children {
                let element_name =
                    &child.borrow().base_type.as_builtin().native_class.class_name.clone();

                if let Some(path_element) = element_types.get(element_name) {
                    let element_type = match path_element {
                        Type::Builtin(b) => b.clone(),
                        _ => panic!(
                            "Incorrect type registry -- expected built-in type for path element {}",
                            element_name
                        ),
                    };

                    if child.borrow().repeated.is_some() {
                        diag.push_error(
                            "Path elements are not supported with `for`-`in` syntax, yet (https://github.com/sixtyfpsui/sixtyfps/issues/754)".into(),
                            &*child.borrow(),
                        );
                    } else {
                        let bindings = std::mem::take(&mut child.borrow_mut().bindings);
                        path_data.push(PathElement { element_type, bindings });
                    }
                } else {
                    elem.children.push(child);
                }
            }
            crate::expression_tree::Path::Elements(path_data)
        };

        elem.bindings
            .insert("elements".into(), RefCell::new(Expression::PathData(path_data).into()));
    });
}
