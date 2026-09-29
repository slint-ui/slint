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
use crate::langtype::{BuiltinElement, BuiltinStruct, ElementType, Struct, Type};
use crate::object_tree::*;
use smol_str::SmolStr;
use std::rc::Rc;
use std::sync::Arc;

pub fn compile_paths(
    component: &Rc<Component>,
    tr: &crate::typeregister::TypeRegister,
    diag: &mut BuildDiagnostics,
) {
    let path_type = tr.lookup_element("Path").unwrap();
    let path_type = path_type.as_builtin();

    recurse_elem_including_sub_components_no_borrow(component, &(), &mut |elem_, _| {
        if elem_.borrow().builtin_type().is_none_or(|bt| bt.name != "Path") {
            return;
        }

        let commands_binding = elem_.borrow_mut().take_binding("commands");

        let path_data_binding = if let Some(commands_expr) = commands_binding {
            if let Some(path_child) = elem_
                .borrow()
                .children
                .iter()
                .find(|child| path_element_type(child, path_type).is_some())
            {
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
                                format!("Error parsing SVG commands ({e})"),
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
                if let Some(element_type) = path_element_type(&child, path_type).cloned() {
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
                                if let Some(binding) = child.take_binding(k) {
                                    bindings.insert(k.clone(), binding.into());
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

            if path_data.is_empty() {
                // Keep the elements a base component may have compiled already
                return;
            }

            Expression::PathData(crate::expression_tree::Path::Elements(path_data)).into()
        };

        elem_.borrow_mut().set_binding(SmolStr::new_static("elements"), path_data_binding);
    });
}

/// Reports path elements or `commands` given to an instance of a component whose `Path` base
/// already declares path elements, and a children placeholder in such a `Path`.
///
/// Runs before inlining, while the path elements of the base are still its children.
/// A `Path` is populated in one place only: a base that declares nothing can be filled by the
/// instance, but there is no appending to what the base declares.
pub fn check_derived_paths(
    component: &Rc<Component>,
    tr: &crate::typeregister::TypeRegister,
    diag: &mut BuildDiagnostics,
) {
    let path_type = tr.lookup_element("Path").unwrap();
    let path_type = path_type.as_builtin();

    for (name, cip) in component.child_insertion_points.borrow().iter() {
        if declares_path_elements(&cip.parent.borrow(), path_type) {
            diag.push_error(
                format!(
                    "{} cannot be placed in a Path that already has path elements",
                    slot_error_subject(name)
                ),
                &cip.node,
            );
        }
    }

    recurse_elem_including_sub_components_no_borrow(component, &(), &mut |elem, _| {
        let elem = elem.borrow();
        let ElementType::Component(base) = &elem.base_type else { return };
        if elem.children.is_empty() && elem.binding("commands").is_none() {
            return;
        }
        if base.child_insertion_points.borrow().contains_key(DEFAULT_SLOT_NAME) {
            // The children go to the placeholder, which is checked above
            return;
        }
        if declares_path_elements(&base.root_element.borrow(), path_type) {
            diag.push_error(
                "The Path was already populated in the base type and it can't be re-populated again"
                    .into(),
                &*elem,
            );
        }
    });
}

/// Whether `elem`, or the root of a component it derives from, has path element children
fn declares_path_elements(elem: &Element, path_type: &BuiltinElement) -> bool {
    elem.builtin_type().is_some_and(|builtin| builtin.name == path_type.name)
        && elem.any_in_inheritance_chain(|e| {
            e.children.iter().any(|child| path_element_type(child, path_type).is_some())
        })
}

/// The path element type of `child` when it is a `MoveTo`, `LineTo`, ... element
fn path_element_type<'a>(
    child: &ElementRc,
    path_type: &'a BuiltinElement,
) -> Option<&'a Rc<BuiltinElement>> {
    let builtin = child.borrow().builtin_type()?;
    path_type.additional_accepted_child_types.get(&builtin.native_class.class_name)
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

    let event_enum = crate::typeregister::BUILTIN.enums.PathEvent.clone();
    let point_type = Arc::new(Struct::new(
        IntoIterator::into_iter([
            (SmolStr::new_static("x"), Type::Float32),
            (SmolStr::new_static("y"), Type::Float32),
        ])
        .collect(),
        BuiltinStruct::Point,
    ));

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
