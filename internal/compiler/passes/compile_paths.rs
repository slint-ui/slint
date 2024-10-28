// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

//! This pass converts the verbose markup used for paths, such as
//!    Path {
//!        LineTo { ... } ArcTo { ... }
//!    }
//! to a vector of path elements (PathData) that is assigned to the
//! elements property of the Path element. That way the generators have to deal
//! with path embedding only as part of the property assignment.

use crate::diagnostics::BuildDiagnostics;
use crate::expression_tree::*;
use crate::langtype::ElementType;
use crate::langtype::{Struct, Type};
use crate::object_tree::*;
use crate::EmbedResourcesKind;
use smol_str::SmolStr;
use std::cell::RefCell;
use std::rc::Rc;

pub fn compile_paths(
    component: &Rc<Component>,
    tr: &crate::typeregister::TypeRegister,
    _embed_resources: EmbedResourcesKind,
    diag: &mut BuildDiagnostics,
) {
    let path_type = tr.lookup_element("Path").unwrap();
    let path_type = path_type.as_builtin();

    recurse_elem(&component.root_element, &(), &mut |elem_, _| {
        let accepted_type = match &elem_.borrow().base_type {
            ElementType::Builtin(be)
                if be.native_class.class_name == path_type.native_class.class_name =>
            {
                path_type
            }
            _ => return,
        };

        #[cfg(feature = "software-renderer")]
        if _embed_resources == EmbedResourcesKind::EmbedTextures {
            diag.push_warning(
                "Path element is not supported with the software renderer".into(),
                &*elem_.borrow(),
            )
        }

        let element_types = &accepted_type.additional_accepted_child_types;

        let commands_binding =
            elem_.borrow_mut().bindings.remove("commands").map(RefCell::into_inner);

        let path_data_binding = if let Some(commands_expr) = commands_binding {
            if let Some(path_child) = elem_.borrow().children.iter().find(|child| {
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

            match &commands_expr.expression {
                Expression::StringLiteral(commands) => {
                    match compile_path_from_string_literal(commands) {
                        Ok(binding) => binding,
                        Err(e) => {
                            diag.push_error(
                                format!("Error parsing SVG commands ({e:?})"),
                                &commands_expr,
                            );
                            return;
                        }
                    }
                }
                expr if expr.ty() == Type::String => Expression::PathData(
                    crate::expression_tree::Path::Commands(Box::new(commands_expr.expression)),
                )
                .into(),
                _ => {
                    diag.push_error(
                        "The commands property only accepts strings".into(),
                        &*elem_.borrow(),
                    );
                    return;
                }
            }
        } else {
            let mut elem = elem_.borrow_mut();
            let enclosing_component = elem.enclosing_component.upgrade().unwrap();
            let new_children = Vec::with_capacity(elem.children.len());
            let old_children = std::mem::replace(&mut elem.children, new_children);

            let mut path_data = Vec::new();

            for child in old_children {
                let element_name =
                    &child.borrow().base_type.as_builtin().native_class.class_name.clone();

                if let Some(path_element) = element_types.get(element_name) {
                    let element_type = match path_element {
                        ElementType::Builtin(b) => b.clone(),
                        _ => panic!(
                            "Incorrect type registry -- expected built-in type for path element {}",
                            element_name
                        ),
                    };

                    if child.borrow().repeated.is_some() {
                        diag.push_error(
                            "Path elements are not supported with `for`-`in` syntax, yet (https://github.com/slint-ui/slint/issues/754)".into(),
                            &*child.borrow(),
                        );
                    } else {
                        let mut bindings = std::collections::BTreeMap::new();
                        {
                            let mut child = child.borrow_mut();
                            for k in element_type.properties.keys() {
                                if let Some(binding) = child.bindings.remove(k) {
                                    bindings.insert(k.clone(), binding);
                                }
                            }
                        }
                        path_data.push(PathElement { element_type, bindings });
                        enclosing_component.optimized_elements.borrow_mut().push(child);
                    }
                } else {
                    elem.children.push(child);
                }
            }
            Expression::PathData(crate::expression_tree::Path::Elements(path_data)).into()
        };

        elem_.borrow_mut().bindings.insert("elements".into(), RefCell::new(path_data_binding));
    });
}

fn compile_path_from_string_literal(
    commands: &str,
) -> Result<BindingExpression, lyon_extra::parser::ParseError> {
    let mut builder = lyon_path::Path::builder();
    let mut parser = lyon_extra::parser::PathParser::new();
    parser.parse(
        &lyon_extra::parser::ParserOptions::DEFAULT,
        &mut lyon_extra::parser::Source::new(commands.chars()),
        &mut builder,
    )?;
    let path = builder.build();

    let event_enum = crate::typeregister::BUILTIN.with(|e| e.enums.PathEvent.clone());
    let point_type = Type::Struct(Rc::new(Struct {
        fields: IntoIterator::into_iter([
            (SmolStr::new_static("x"), Type::Float32),
            (SmolStr::new_static("y"), Type::Float32),
        ])
        .collect(),
        name: Some("slint::private_api::Point".into()),
        node: None,
        rust_attributes: None,
    }));

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
                        event_enum.clone().try_value_from_string("end-closed").unwrap()
                    } else {
                        event_enum.clone().try_value_from_string("end-open").unwrap()
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
                (SmolStr::new_static("x"), Expression::NumberLiteral(point.x as _, Unit::None)),
                (SmolStr::new_static("y"), Expression::NumberLiteral(point.y as _, Unit::None)),
            ])
            .collect(),
        })
        .collect();

    Ok(Expression::PathData(Path::Events(events, points)).into())
}
