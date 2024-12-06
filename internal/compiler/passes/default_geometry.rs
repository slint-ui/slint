// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

/*! Set the width and height of Rectangle, TouchArea, ... to 100%,
    the implicit width or aspect ratio preserving for Images.
    Also set the Image.image-fit default depending on the presence of a
    layout parent.

    This pass must be run after lower_layout
*/

use std::cell::RefCell;
use std::rc::Rc;

use crate::diagnostics::{BuildDiagnostics, DiagnosticLevel, Spanned};
use crate::expression_tree::{
    BindingExpression, BuiltinFunction, Expression, MinMaxOp, NamedReference, Unit,
};
use crate::langtype::{BuiltinElement, DefaultSizeBinding, Type};
use crate::layout::{implicit_layout_info_call, LayoutConstraints, Orientation};
use crate::object_tree::{Component, ElementRc};
use smol_str::{format_smolstr, SmolStr};
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
                        if elem.borrow().default_fill_parent.0 {
                            let e_width =
                                elem.borrow().geometry_props.as_ref().unwrap().width.clone();
                            let p_width =
                                parent.borrow().geometry_props.as_ref().unwrap().width.clone();
                            w100 |= make_default_100(&e_width, &p_width);
                        } else {
                            make_default_implicit(elem, "width");
                        }
                        if elem.borrow().default_fill_parent.1 {
                            let e_height =
                                elem.borrow().geometry_props.as_ref().unwrap().height.clone();
                            let p_height =
                                parent.borrow().geometry_props.as_ref().unwrap().height.clone();
                            h100 |= make_default_100(&e_height, &p_height);
                        } else {
                            make_default_implicit(elem, "height");
                        }
                    }
                    DefaultSizeBinding::ExpandsToParentGeometry => {
                        if !elem.borrow().child_of_layout {
                            let (e_width, e_height) = elem
                                .borrow()
                                .geometry_props
                                .as_ref()
                                .map(|g| (g.width.clone(), g.height.clone()))
                                .unwrap();
                            let (p_width, p_height) = parent
                                .borrow()
                                .geometry_props
                                .as_ref()
                                .map(|g| (g.width.clone(), g.height.clone()))
                                .unwrap();
                            w100 |= make_default_100(&e_width, &p_width);
                            h100 |= make_default_100(&e_height, &p_height);
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

                if !elem.borrow().child_of_layout
                    && !elem.borrow().is_legacy_syntax
                    && builtin_type.name != "Window"
                {
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
                .map(|(h, v)| {
                    (Some(Expression::PropertyReference(h)), Some(Expression::PropertyReference(v)))
                })
                .or_else(|| {
                    if c.borrow().is_legacy_syntax {
                        return None;
                    }
                    if c.borrow().repeated.is_some() {
                        // FIXME: we should ideally add runtime code to merge layout info of all elements that are repeated (same as #407)
                        return None;
                    }
                    let explicit_constraints =
                        LayoutConstraints::new(c, diag, DiagnosticLevel::Error);
                    let use_implicit_size = c.borrow().builtin_type().map_or(false, |b| {
                        b.default_size_binding == DefaultSizeBinding::ImplicitSize
                    });

                    let compute = |orientation| {
                        if !explicit_constraints.has_explicit_restrictions(orientation) {
                            use_implicit_size.then(|| implicit_layout_info_call(c, orientation))
                        } else {
                            Some(explicit_layout_info(c, orientation))
                        }
                    };
                    Some((compute(Orientation::Horizontal), compute(Orientation::Vertical)))
                        .filter(|(a, b)| a.is_some() || b.is_some())
                })
        })
        .collect::<Vec<_>>();

    if child_infos.is_empty() {
        return;
    }

    let li_v = crate::layout::create_new_prop(
        elem,
        SmolStr::new_static("layoutinfo-v"),
        crate::typeregister::layout_info_type(),
    );
    let li_h = crate::layout::create_new_prop(
        elem,
        SmolStr::new_static("layoutinfo-h"),
        crate::typeregister::layout_info_type(),
    );
    elem.borrow_mut().layout_info_prop = Some((li_h.clone(), li_v.clone()));
    let mut expr_h = implicit_layout_info_call(elem, Orientation::Horizontal);
    let mut expr_v = implicit_layout_info_call(elem, Orientation::Vertical);

    let explicit_constraints = LayoutConstraints::new(elem, diag, DiagnosticLevel::Warning);
    if !explicit_constraints.fixed_width {
        merge_explicit_constraints(&mut expr_h, &explicit_constraints, Orientation::Horizontal);
    }
    if !explicit_constraints.fixed_height {
        merge_explicit_constraints(&mut expr_v, &explicit_constraints, Orientation::Vertical);
    }

    for child_info in child_infos {
        if let Some(h) = child_info.0 {
            expr_h = Expression::BinaryExpression {
                lhs: Box::new(std::mem::take(&mut expr_h)),
                rhs: Box::new(h),
                op: '+',
            };
        }
        if let Some(v) = child_info.1 {
            expr_v = Expression::BinaryExpression {
                lhs: Box::new(std::mem::take(&mut expr_v)),
                rhs: Box::new(v),
                op: '+',
            };
        }
    }

    let expr_v = BindingExpression::new_with_span(expr_v, elem.borrow().to_source_location());
    li_v.element().borrow_mut().bindings.insert(li_v.name().clone(), expr_v.into());
    let expr_h = BindingExpression::new_with_span(expr_h, elem.borrow().to_source_location());
    li_h.element().borrow_mut().bindings.insert(li_h.name().clone(), expr_h.into());
}

fn merge_explicit_constraints(
    expr: &mut Expression,
    constraints: &LayoutConstraints,
    orientation: Orientation,
) {
    if constraints.has_explicit_restrictions(orientation) {
        static COUNT: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
        let unique_name = format_smolstr!(
            "layout_info_{}",
            COUNT.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
        );
        let ty = expr.ty();
        let store = Expression::StoreLocalVariable {
            name: unique_name.clone(),
            value: Box::new(std::mem::take(expr)),
        };
        let Type::Struct(s) = &ty else { unreachable!() };
        let mut values = s
            .fields
            .keys()
            .map(|p| {
                (
                    p.clone(),
                    Expression::StructFieldAccess {
                        base: Expression::ReadLocalVariable {
                            name: unique_name.clone(),
                            ty: ty.clone(),
                        }
                        .into(),
                        name: p.clone(),
                    },
                )
            })
            .collect::<HashMap<_, _>>();

        for (nr, s) in constraints.for_each_restrictions(orientation) {
            let e = nr
                .element()
                .borrow()
                .bindings
                .get(nr.name())
                .expect("constraint must have binding")
                .borrow()
                .expression
                .clone();
            debug_assert!(!matches!(e, Expression::Invalid));
            values.insert(s.into(), e);
        }
        *expr = Expression::CodeBlock([store, Expression::Struct { ty, values }].into());
    }
}

fn explicit_layout_info(e: &ElementRc, orientation: Orientation) -> Expression {
    let mut values = HashMap::new();
    let (size, orient) = match orientation {
        Orientation::Horizontal => ("width", "horizontal"),
        Orientation::Vertical => ("height", "vertical"),
    };
    for (k, v) in [
        ("min", format_smolstr!("min-{size}")),
        ("max", format_smolstr!("max-{size}")),
        ("preferred", format_smolstr!("preferred-{size}")),
        ("stretch", format_smolstr!("{orient}-stretch")),
    ] {
        values.insert(k.into(), Expression::PropertyReference(NamedReference::new(e, v)));
    }
    values.insert("min_percent".into(), Expression::NumberLiteral(0., Unit::None));
    values.insert("max_percent".into(), Expression::NumberLiteral(100., Unit::None));
    Expression::Struct { ty: crate::typeregister::layout_info_type(), values }
}

/// Replace expression such as  `"width: 30%;` with `width: 0.3 * parent.width;`
///
/// Returns true if the expression was 100%
fn fix_percent_size(
    elem: &ElementRc,
    parent: &Option<ElementRc>,
    property: &'static str,
    diag: &mut BuildDiagnostics,
) -> bool {
    let elem = elem.borrow();
    let binding = match elem.bindings.get(property) {
        Some(b) => b,
        None => return false,
    };

    if binding.borrow().ty() != Type::Percent {
        let Some(parent) = parent.as_ref() else { return false };
        // Pattern match to check it was already parent.<property>
        return matches!(&binding.borrow().expression, Expression::PropertyReference(nr) if *nr.name() == property && Rc::ptr_eq(&nr.element(), parent));
    }
    let mut b = binding.borrow_mut();
    if let Some(mut parent) = parent.clone() {
        if parent.borrow().is_flickable_viewport {
            // the `%` in a flickable need to refer to the size of the flickable, not the size of the viewport
            parent = crate::object_tree::find_parent_element(&parent).unwrap_or(parent)
        }
        debug_assert_eq!(
            parent.borrow().lookup_property(&property).property_type,
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
            rhs: Box::new(Expression::PropertyReference(NamedReference::new(
                &parent,
                SmolStr::new_static(property),
            ))),
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
fn make_default_100(prop: &NamedReference, parent_prop: &NamedReference) -> bool {
    prop.element().borrow_mut().set_binding_if_not_set(prop.name().clone(), || {
        Expression::PropertyReference(parent_prop.clone())
    })
}

fn make_default_implicit(elem: &ElementRc, property: &str) {
    let e = crate::builtin_macros::min_max_expression(
        Expression::PropertyReference(NamedReference::new(
            elem,
            format_smolstr!("preferred-{}", property),
        )),
        Expression::PropertyReference(NamedReference::new(
            elem,
            format_smolstr!("min-{}", property),
        )),
        MinMaxOp::Max,
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
    missing_size_property: &'static str,
    given_size_property: &'static str,
) {
    if elem.borrow().is_binding_set(&missing_size_property, false) {
        return;
    }

    debug_assert_eq!(elem.borrow().lookup_property("source").property_type, Type::Image);

    let missing_size_property = SmolStr::new_static(missing_size_property);
    let given_size_property = SmolStr::new_static(given_size_property);

    let ratio = if elem.borrow().is_binding_set("source-clip-height", false) {
        Expression::BinaryExpression {
            lhs: Box::new(Expression::PropertyReference(NamedReference::new(
                elem,
                format_smolstr!("source-clip-{missing_size_property}"),
            ))),
            rhs: Box::new(Expression::PropertyReference(NamedReference::new(
                elem,
                format_smolstr!("source-clip-{given_size_property}"),
            ))),
            op: '/',
        }
    } else {
        let implicit_size_var = Box::new(Expression::ReadLocalVariable {
            name: "image_implicit_size".into(),
            ty: BuiltinFunction::ImageSize.ty().return_type.clone(),
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
                        elem,
                        SmolStr::new_static("source"),
                    ))],
                    source_location: None,
                }),
            },
            Expression::BinaryExpression {
                lhs: Box::new(Expression::StructFieldAccess {
                    base: implicit_size_var.clone(),
                    name: missing_size_property.clone(),
                }),
                rhs: Box::new(Expression::StructFieldAccess {
                    base: implicit_size_var,
                    name: given_size_property.clone(),
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

    elem.borrow_mut().bindings.insert(missing_size_property, RefCell::new(binding.into()));
}

fn maybe_center_in_parent(
    elem: &ElementRc,
    parent: &ElementRc,
    pos_prop: &'static str,
    size_prop: &'static str,
) {
    if elem.borrow().is_binding_set(&pos_prop, false) {
        return;
    }

    let size_prop = SmolStr::new_static(size_prop);
    let diff = Expression::BinaryExpression {
        lhs: Expression::PropertyReference(NamedReference::new(parent, size_prop.clone())).into(),
        op: '-',
        rhs: Expression::PropertyReference(NamedReference::new(elem, size_prop)).into(),
    };

    let pos_prop = SmolStr::new_static(pos_prop);
    elem.borrow_mut().set_binding_if_not_set(pos_prop, || Expression::BinaryExpression {
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
        let source = NamedReference::new(elem, SmolStr::new_static("source"));
        let x = NamedReference::new(elem, SmolStr::new_static("source-clip-x"));
        let y = NamedReference::new(elem, SmolStr::new_static("source-clip-y"));
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

#[test]
fn test_no_property_for_100pc() {
    //! Test that we don't generate x or y property to center elements if the size is filling the parent
    let mut compiler_config =
        crate::CompilerConfiguration::new(crate::generator::OutputFormat::Interpreter);
    compiler_config.style = Some("fluent".into());
    let mut test_diags = crate::diagnostics::BuildDiagnostics::default();
    let doc_node = crate::parser::parse(
        r#"
        export component Foo inherits Window {
            r1 := Rectangle {
                r2 := Rectangle {
                    width: 100%;
                    background: blue;
                }
                r3 := Rectangle {
                    height: parent.height;
                    width: 50%;
                    background: red;
                }
            }

            out property <length> r2x: r2.x;
            out property <length> r2y: r2.y;
            out property <length> r3x: r3.x;
            out property <length> r3y: r3.y;
        }
"#
        .into(),
        Some(std::path::Path::new("HELLO")),
        &mut test_diags,
    );
    let (doc, diag, _) =
        spin_on::spin_on(crate::compile_syntax_node(doc_node, test_diags, compiler_config));
    assert!(!diag.has_errors(), "{:?}", diag.to_string_vec());

    let root_elem = doc.inner_components.last().unwrap().root_element.borrow();

    // const propagation must have seen that the x and y property are literal 0
    assert!(matches!(
        &root_elem.bindings.get("r2x").unwrap().borrow().expression,
        Expression::NumberLiteral(v, _) if *v == 0.
    ));
    assert!(matches!(
        &root_elem.bindings.get("r2y").unwrap().borrow().expression,
        Expression::NumberLiteral(v, _) if *v == 0.
    ));
    assert!(matches!(
        &root_elem.bindings.get("r3y").unwrap().borrow().expression,
        Expression::NumberLiteral(v, _) if *v == 0.
    ));
    // this one is 50% so it should be set to be in the center
    assert!(!matches!(
        &root_elem.bindings.get("r3x").unwrap().borrow().expression,
        Expression::BinaryExpression { .. }
    ));
}
