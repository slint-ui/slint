// Copyright © SixtyFPS GmbH <info@slint-ui.com>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-commercial

/*! Set the width and height of Rectangle, TouchArea, ... to 100%,
    the implicit width or aspect ratio preserving for Images.
    Also set the Image.image-fit default depending on the presence of a
    layout parent.

    This pass must be run after lower_layout
*/

use std::cell::RefCell;
use std::rc::Rc;

use crate::diagnostics::{BuildDiagnostics, Spanned};
use crate::expression_tree::{
    BindingExpression, BuiltinFunction, Expression, NamedReference, Unit,
};
use crate::langtype::{
    BuiltinElement, DefaultSizeBinding, ElementType, PropertyLookupResult, Type,
};
use crate::layout::{implicit_layout_info_call, LayoutConstraints, Orientation};
use crate::object_tree::{Component, ElementRc};
use std::collections::HashMap;

pub fn default_geometry(root_component: &Rc<Component>, diag: &mut BuildDiagnostics) {
    crate::object_tree::recurse_elem_including_sub_components(
        root_component,
        &None,
        &mut |elem: &ElementRc, parent: &Option<ElementRc>| {
            if elem.borrow().repeated.is_some() {
                return None;
            }

            // whether the width, or height, is filling the parent
            let (mut w100, mut h100) = (false, false);

            w100 |= fix_percent_size(elem, parent, "width", diag);
            h100 |= fix_percent_size(elem, parent, "height", diag);

            gen_layout_info_prop(elem, diag);

            let builtin_type = match elem.borrow().builtin_type() {
                Some(b) => b,
                None => return Some(elem.clone()),
            };

            let is_image = builtin_type.name == "Image";
            if is_image {
                adjust_image_clip_rect(elem, &builtin_type);
            }

            if let Some(parent) = parent {
                match builtin_type.default_size_binding {
                    DefaultSizeBinding::None => {
                        let no_constraint_defined = !has_layout_info_prop(elem)
                            && !LayoutConstraints::new(elem, diag).has_explicit_restrictions();
                        if no_constraint_defined || elem.borrow().default_fill_parent.0 {
                            w100 |= make_default_100(elem, parent, "width");
                        } else {
                            make_default_implicit(elem, "width");
                        }
                        if no_constraint_defined || elem.borrow().default_fill_parent.1 {
                            h100 |= make_default_100(elem, parent, "height");
                        } else {
                            make_default_implicit(elem, "height");
                        }
                    }
                    DefaultSizeBinding::ExpandsToParentGeometry => {
                        if !elem.borrow().child_of_layout {
                            w100 |= make_default_100(elem, parent, "width");
                            h100 |= make_default_100(elem, parent, "height");
                        }
                    }
                    DefaultSizeBinding::ImplicitSize => {
                        let has_length_property_binding = |elem: &ElementRc, property: &str| {
                            debug_assert_eq!(
                                elem.borrow().lookup_property(property).property_type,
                                Type::LogicalLength
                            );

                            elem.borrow().is_binding_set(property, true)
                        };

                        let width_specified = has_length_property_binding(elem, "width");
                        let height_specified = has_length_property_binding(elem, "height");

                        if !elem.borrow().child_of_layout {
                            // Add aspect-ratio preserving width or height bindings
                            if is_image && width_specified && !height_specified {
                                make_default_aspect_ratio_preserving_binding(
                                    elem, "height", "width",
                                )
                            } else if is_image && height_specified && !width_specified {
                                make_default_aspect_ratio_preserving_binding(
                                    elem, "width", "height",
                                )
                            } else {
                                make_default_implicit(elem, "width");
                                make_default_implicit(elem, "height");
                            }
                        } else if is_image {
                            // If an image is in a layout and has no explicit width or height specified, change the default for image-fit
                            // to `contain`
                            if !width_specified || !height_specified {
                                let image_fit_lookup = elem.borrow().lookup_property("image-fit");

                                elem.borrow_mut().set_binding_if_not_set(
                                    image_fit_lookup.resolved_name.into(),
                                    || {
                                        Expression::EnumerationValue(
                                            image_fit_lookup
                                                .property_type
                                                .as_enum()
                                                .clone()
                                                .try_value_from_string("contain")
                                                .unwrap(),
                                        )
                                    },
                                );
                            }
                        }
                    }
                }

                if !elem.borrow().child_of_layout && !elem.borrow().is_legacy_syntax {
                    if !w100 {
                        maybe_center_in_parent(elem, parent, "x", "width");
                    }
                    if !h100 {
                        maybe_center_in_parent(elem, parent, "y", "height");
                    }
                }
            }

            Some(elem.clone())
        },
    )
}

/// Generate a layout_info_prop based on the children layouts
fn gen_layout_info_prop(elem: &ElementRc, diag: &mut BuildDiagnostics) {
    if elem.borrow().layout_info_prop.is_some() || elem.borrow().is_flickable_viewport {
        return;
    }

    let child_infos = elem
        .borrow()
        .children
        .iter()
        .filter(|c| {
            !c.borrow().bindings.contains_key("x") && !c.borrow().bindings.contains_key("y")
        })
        .filter_map(|c| {
            gen_layout_info_prop(c, diag);
            c.borrow()
                .layout_info_prop
                .clone()
                .map(|(h, v)| (Expression::PropertyReference(h), Expression::PropertyReference(v)))
                .or_else(|| {
                    if c.borrow().is_legacy_syntax {
                        return None;
                    }
                    if c.borrow().repeated.is_some() {
                        // FIXME: we should ideally add runtime code to merge layout info of all elements that are repeated (same as #407)
                        return None;
                    }
                    let explicit_constraints = LayoutConstraints::new(c, diag);
                    if !explicit_constraints.has_explicit_restrictions() {
                        c.borrow()
                            .builtin_type()
                            .map_or(false, |b| {
                                b.default_size_binding == DefaultSizeBinding::ImplicitSize
                            })
                            .then(|| {
                                (
                                    implicit_layout_info_call(c, Orientation::Horizontal),
                                    implicit_layout_info_call(c, Orientation::Vertical),
                                )
                            })
                    } else {
                        Some((
                            explicit_layout_info(c, Orientation::Horizontal),
                            explicit_layout_info(c, Orientation::Vertical),
                        ))
                    }
                })
        })
        .collect::<Vec<_>>();

    if child_infos.is_empty() {
        return;
    }

    let li_v = super::lower_layout::create_new_prop(
        elem,
        "layoutinfo-v",
        crate::layout::layout_info_type(),
    );
    let li_h = super::lower_layout::create_new_prop(
        elem,
        "layoutinfo-h",
        crate::layout::layout_info_type(),
    );
    elem.borrow_mut().layout_info_prop = Some((li_h.clone(), li_v.clone()));
    let mut expr_h = implicit_layout_info_call(elem, Orientation::Horizontal);
    let mut expr_v = implicit_layout_info_call(elem, Orientation::Vertical);

    for child_info in child_infos {
        expr_h = Expression::BinaryExpression {
            lhs: Box::new(std::mem::take(&mut expr_h)),
            rhs: Box::new(child_info.0),
            op: '+',
        };
        expr_v = Expression::BinaryExpression {
            lhs: Box::new(std::mem::take(&mut expr_v)),
            rhs: Box::new(child_info.1),
            op: '+',
        };
    }

    let expr_v = BindingExpression::new_with_span(expr_v, elem.borrow().to_source_location());
    li_v.element().borrow_mut().bindings.insert(li_v.name().into(), expr_v.into());
    let expr_h = BindingExpression::new_with_span(expr_h, elem.borrow().to_source_location());
    li_h.element().borrow_mut().bindings.insert(li_h.name().into(), expr_h.into());
}

fn explicit_layout_info(e: &ElementRc, orientation: Orientation) -> Expression {
    let mut values = HashMap::new();
    let (size, orient) = match orientation {
        Orientation::Horizontal => ("width", "horizontal"),
        Orientation::Vertical => ("height", "vertical"),
    };
    for (k, v) in [
        ("min", format!("min-{size}")),
        ("max", format!("max-{size}")),
        ("preferred", format!("preferred-{size}")),
        ("stretch", format!("{orient}-stretch")),
    ] {
        values.insert(k.into(), Expression::PropertyReference(NamedReference::new(e, &v)));
    }
    values.insert("min_percent".into(), Expression::NumberLiteral(0., Unit::None));
    values.insert("max_percent".into(), Expression::NumberLiteral(100., Unit::None));
    Expression::Struct { ty: crate::layout::layout_info_type(), values }
}

/// Replace expression such as  `"width: 30%;` with `width: 0.3 * parent.width;`
///
/// Returns true if the expression was 100%
fn fix_percent_size(
    elem: &ElementRc,
    parent: &Option<ElementRc>,
    property: &str,
    diag: &mut BuildDiagnostics,
) -> bool {
    let elem = elem.borrow();
    let binding = match elem.bindings.get(property) {
        Some(b) => b,
        None => return false,
    };

    if binding.borrow().ty() != Type::Percent {
        return false;
    }
    let mut b = binding.borrow_mut();
    if let Some(parent) = parent {
        debug_assert_eq!(
            parent.borrow().lookup_property(property).property_type,
            Type::LogicalLength
        );
        let fill =
            matches!(b.expression, Expression::NumberLiteral(x, _) if (x - 100.).abs() < 0.001);
        b.expression = Expression::BinaryExpression {
            lhs: Box::new(std::mem::take(&mut b.expression).maybe_convert_to(
                Type::Float32,
                &b.span,
                diag,
            )),
            rhs: Box::new(Expression::PropertyReference(NamedReference::new(parent, property))),
            op: '*',
        };
        fill
    } else {
        diag.push_error("Cannot find parent property to apply relative length".into(), &b.span);
        false
    }
}

/// Generate a size property that covers the parent.
/// Return true if it was changed
fn make_default_100(elem: &ElementRc, parent_element: &ElementRc, property: &str) -> bool {
    let PropertyLookupResult { resolved_name, property_type, .. } =
        parent_element.borrow().lookup_property(property);
    if property_type != Type::LogicalLength {
        return false;
    }

    elem.borrow_mut().set_binding_if_not_set(resolved_name.to_string(), || {
        Expression::PropertyReference(NamedReference::new(parent_element, resolved_name.as_ref()))
    })
}

fn make_default_implicit(elem: &ElementRc, property: &str) {
    let e = crate::builtin_macros::min_max_expression(
        Expression::PropertyReference(NamedReference::new(
            elem,
            &format!("preferred-{}", property),
        )),
        Expression::PropertyReference(NamedReference::new(elem, &format!("min-{}", property))),
        '>',
    );
    elem.borrow_mut().set_binding_if_not_set(property.into(), || e);
}

// For an element with `width`, `height`, `preferred-width` and `preferred-height`, make an aspect
// ratio preserving binding. This is currently only called for Image elements. For example when for an
// image the `width` is specified and there is no `height` binding, it is called with `missing_size_property = height`
// and `given_size_property = width` and install a binding like this:
//
//    height: self.width * self.preferred_height / self.preferred_width;
//
fn make_default_aspect_ratio_preserving_binding(
    elem: &ElementRc,
    missing_size_property: &str,
    given_size_property: &str,
) {
    if elem.borrow().is_binding_set(missing_size_property, false) {
        return;
    }

    debug_assert_eq!(elem.borrow().lookup_property("source").property_type, Type::Image);

    let ratio = if elem.borrow().is_binding_set("source-clip-height", false) {
        Expression::BinaryExpression {
            lhs: Box::new(Expression::PropertyReference(NamedReference::new(
                elem,
                &format!("source-clip-{missing_size_property}"),
            ))),
            rhs: Box::new(Expression::PropertyReference(NamedReference::new(
                elem,
                &format!("source-clip-{given_size_property}"),
            ))),
            op: '/',
        }
    } else {
        let implicit_size_var = Box::new(Expression::ReadLocalVariable {
            name: "image_implicit_size".into(),
            ty: match BuiltinFunction::ImageSize.ty() {
                Type::Function { return_type, .. } => *return_type,
                _ => panic!("invalid type for ImplicitItemSize built-in function"),
            },
        });

        Expression::CodeBlock(vec![
            Expression::StoreLocalVariable {
                name: "image_implicit_size".into(),
                value: Box::new(Expression::FunctionCall {
                    function: Box::new(Expression::BuiltinFunctionReference(
                        BuiltinFunction::ImageSize,
                        None,
                    )),
                    arguments: vec![Expression::PropertyReference(NamedReference::new(
                        elem, "source",
                    ))],
                    source_location: None,
                }),
            },
            Expression::BinaryExpression {
                lhs: Box::new(Expression::StructFieldAccess {
                    base: implicit_size_var.clone(),
                    name: missing_size_property.into(),
                }),
                rhs: Box::new(Expression::StructFieldAccess {
                    base: implicit_size_var,
                    name: given_size_property.into(),
                }),
                op: '/',
            },
        ])
    };
    let binding = Expression::BinaryExpression {
        lhs: Box::new(ratio),
        rhs: Expression::PropertyReference(NamedReference::new(elem, given_size_property)).into(),
        op: '*',
    };

    elem.borrow_mut()
        .bindings
        .insert(missing_size_property.to_string(), RefCell::new(binding.into()));
}

fn maybe_center_in_parent(elem: &ElementRc, parent: &ElementRc, pos_prop: &str, size_prop: &str) {
    if elem.borrow().is_binding_set(pos_prop, false) {
        return;
    }
    if elem.borrow().lookup_property(pos_prop).property_type != Type::LogicalLength
        || elem.borrow().lookup_property(size_prop).property_type != Type::LogicalLength
    {
        return;
    }

    let diff = Expression::BinaryExpression {
        lhs: Expression::PropertyReference(NamedReference::new(parent, size_prop)).into(),
        op: '-',
        rhs: Expression::PropertyReference(NamedReference::new(elem, size_prop)).into(),
    };

    elem.borrow_mut().set_binding_if_not_set(pos_prop.into(), || Expression::BinaryExpression {
        lhs: diff.into(),
        op: '/',
        rhs: Expression::NumberLiteral(2., Unit::None).into(),
    });
}

fn adjust_image_clip_rect(elem: &ElementRc, builtin: &Rc<BuiltinElement>) {
    debug_assert_eq!(builtin.native_class.class_name, "ClippedImage");

    if builtin.native_class.properties.keys().any(|p| {
        elem.borrow().bindings.contains_key(p)
            || elem.borrow().property_analysis.borrow().get(p).map_or(false, |a| a.is_used())
    }) {
        let source = NamedReference::new(elem, "source");
        let x = NamedReference::new(elem, "source-clip-x");
        let y = NamedReference::new(elem, "source-clip-y");
        let make_expr = |dim: &str, prop: NamedReference| Expression::BinaryExpression {
            lhs: Box::new(Expression::StructFieldAccess {
                base: Box::new(Expression::FunctionCall {
                    function: Box::new(Expression::BuiltinFunctionReference(
                        BuiltinFunction::ImageSize,
                        None,
                    )),
                    arguments: vec![Expression::PropertyReference(source.clone())],
                    source_location: None,
                }),
                name: dim.into(),
            }),
            rhs: Expression::PropertyReference(prop).into(),
            op: '-',
        };

        elem.borrow_mut()
            .set_binding_if_not_set("source-clip-width".into(), || make_expr("width", x));
        elem.borrow_mut()
            .set_binding_if_not_set("source-clip-height".into(), || make_expr("height", y));
    }
}

// return true if the element of its component base has a layout_info_prop define
fn has_layout_info_prop(elem: &ElementRc) -> bool {
    if elem.borrow().layout_info_prop.is_some() {
        return true;
    };
    if let ElementType::Component(base) = &elem.borrow().base_type {
        has_layout_info_prop(&base.root_element)
    } else {
        false
    }
}
