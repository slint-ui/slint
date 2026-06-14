// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

use std::rc::Rc;

use smol_str::{SmolStr, ToSmolStr};

use crate::diagnostics::{BuildDiagnostics, Spanned};
use crate::expression_tree::{BindingExpression, Callable, Expression, ImageReference, Unit};
use crate::langtype::{DefaultSizeBinding, ElementType, Type};
use crate::namedreference::NamedReference;
use crate::object_tree::{Component, ElementRc};

pub fn warn_redundant_default_properties(
    root_component: &Rc<Component>,
    diag: &mut BuildDiagnostics,
) {
    crate::object_tree::recurse_elem_including_sub_components(
        root_component,
        &None,
        &mut |elem: &ElementRc, parent: &Option<ElementRc>| {
            let explicit_bindings = collect_explicit_bindings(elem);
            for (property_name, binding) in explicit_bindings {
                if base_has_explicit_binding(elem, &property_name) {
                    continue;
                }

                let builtin_default =
                    builtin_default_expression(elem, &property_name).is_some_and(|default_expr| {
                        same_expression(
                            super::ignore_debug_hooks(&binding.expression),
                            &default_expr,
                        )
                    });

                let geometry_default = parent.as_ref().is_some_and(|parent| {
                    redundant_geometry_binding(
                        elem,
                        parent,
                        &property_name,
                        super::ignore_debug_hooks(&binding.expression),
                    )
                });

                if builtin_default || geometry_default {
                    diag.push_info_with_span(
                        format!(
                            "Property '{}' is explicitly set to its default value and can be removed",
                            property_name
                        ),
                        binding.span.unwrap_or_default(),
                    );
                }
            }
            Some(elem.clone())
        },
    );
}

fn collect_explicit_bindings(elem: &ElementRc) -> Vec<(SmolStr, BindingExpression)> {
    elem.borrow()
        .bindings
        .iter()
        .filter_map(|(property_name, binding)| {
            let binding = binding.borrow();
            let span = binding.span.as_ref()?;
            if binding.priority <= 0
                || !binding.two_way_bindings.is_empty()
                || binding.animation.is_some()
                || span
                    .source_file()
                    .is_some_and(|sf| sf.path().to_string_lossy().starts_with("builtin:/"))
            {
                return None;
            }
            Some((property_name.clone(), binding.clone()))
        })
        .collect()
}

fn base_has_explicit_binding(elem: &ElementRc, property_name: &str) -> bool {
    let mut base_type = elem.borrow().base_type.clone();
    while let ElementType::Component(base) = base_type {
        let base_root = base.root_element.borrow();
        if base_root.bindings.get(property_name).is_some_and(|binding| {
            let binding = binding.borrow();
            binding.priority > 0 && binding.has_binding()
        }) {
            return true;
        }
        base_type = base_root.base_type.clone();
    }
    false
}

fn builtin_default_expression(elem: &ElementRc, property_name: &str) -> Option<Expression> {
    let builtin = elem.borrow().builtin_type()?;
    let info = builtin.properties.get(property_name)?;
    Some(
        info.default_value
            .expr(elem)
            .unwrap_or_else(|| Expression::default_value_for_type(&info.ty)),
    )
}

fn redundant_geometry_binding(
    elem: &ElementRc,
    parent: &ElementRc,
    property_name: &str,
    expression: &Expression,
) -> bool {
    let elem_borrow = elem.borrow();
    let Some(builtin) = elem_borrow.builtin_type() else { return false };

    match property_name {
        "width" | "height"
            if builtin.default_size_binding == DefaultSizeBinding::ExpandsToParentGeometry
                && !elem_borrow.child_of_layout =>
        {
            let parent_prop = NamedReference::new(parent, property_name.to_smolstr());
            is_fill_parent_expression(expression, &parent_prop)
        }
        "x" | "y"
            if !elem_borrow.child_of_layout
                && !elem_borrow.is_legacy_syntax
                && builtin.name != "Window"
                && !axis_fills_parent(elem, parent, property_name) =>
        {
            is_center_in_parent_expression(elem, parent, property_name, expression)
        }
        _ => false,
    }
}

fn axis_fills_parent(elem: &ElementRc, parent: &ElementRc, property_name: &str) -> bool {
    let size_prop = match property_name {
        "x" => "width",
        "y" => "height",
        _ => return false,
    };

    if let Some(binding) = elem.borrow().bindings.get(size_prop) {
        return is_fill_parent_expression(
            super::ignore_debug_hooks(&binding.borrow().expression),
            &NamedReference::new(parent, size_prop.to_smolstr()),
        );
    }

    let Some(builtin) = elem.borrow().builtin_type() else { return false };
    builtin.default_size_binding == DefaultSizeBinding::ExpandsToParentGeometry
}

fn is_fill_parent_expression(expression: &Expression, parent_prop: &NamedReference) -> bool {
    if expression.ty() == Type::Percent && is_scalar_one(expression) {
        return true;
    }

    match expression {
        Expression::PropertyReference(nr) => nr == parent_prop,
        Expression::BinaryExpression { lhs, rhs, op: '*' } => {
            (is_scalar_one(lhs) && matches_property_reference(rhs, parent_prop))
                || (is_scalar_one(rhs) && matches_property_reference(lhs, parent_prop))
        }
        _ => false,
    }
}

fn is_center_in_parent_expression(
    elem: &ElementRc,
    parent: &ElementRc,
    property_name: &str,
    expression: &Expression,
) -> bool {
    let size_prop = match property_name {
        "x" => "width",
        "y" => "height",
        _ => return false,
    };

    let expected = Expression::BinaryExpression {
        lhs: Box::new(Expression::BinaryExpression {
            lhs: Box::new(Expression::PropertyReference(NamedReference::new(
                parent,
                SmolStr::new_static(size_prop),
            ))),
            rhs: Box::new(Expression::PropertyReference(NamedReference::new(
                elem,
                SmolStr::new_static(size_prop),
            ))),
            op: '-',
        }),
        rhs: Box::new(Expression::NumberLiteral(2., Unit::None)),
        op: '/',
    };

    same_expression(expression, &expected)
}

fn is_scalar_one(expression: &Expression) -> bool {
    scalar_value(expression).is_some_and(|value| (value - 1.).abs() < 0.001)
}

fn scalar_value(expression: &Expression) -> Option<f64> {
    match expression {
        Expression::NumberLiteral(value, Unit::None) => Some(*value),
        Expression::NumberLiteral(value, Unit::Percent) => Some(*value * 0.01),
        Expression::Cast { from, .. } => scalar_value(from),
        Expression::UnaryOp { sub, op: '+' } => scalar_value(sub),
        Expression::UnaryOp { sub, op: '-' } => scalar_value(sub).map(|value| -value),
        Expression::BinaryExpression { lhs, rhs, op: '*' } => {
            Some(scalar_value(lhs)? * scalar_value(rhs)?)
        }
        Expression::BinaryExpression { lhs, rhs, op: '/' } => {
            let rhs = scalar_value(rhs)?;
            if rhs.abs() < 0.001 { None } else { Some(scalar_value(lhs)? / rhs) }
        }
        _ => None,
    }
}

fn matches_property_reference(expression: &Expression, expected: &NamedReference) -> bool {
    matches!(expression, Expression::PropertyReference(actual) if actual == expected)
}

fn same_expression(lhs: &Expression, rhs: &Expression) -> bool {
    match (lhs, rhs) {
        (Expression::Invalid, Expression::Invalid) => true,
        (Expression::StringLiteral(lhs), Expression::StringLiteral(rhs)) => lhs == rhs,
        (
            Expression::NumberLiteral(lhs_value, lhs_unit),
            Expression::NumberLiteral(rhs_value, rhs_unit),
        ) => lhs_unit == rhs_unit && (lhs_value - rhs_value).abs() < 0.001,
        (Expression::BoolLiteral(lhs), Expression::BoolLiteral(rhs)) => lhs == rhs,
        (Expression::PropertyReference(lhs), Expression::PropertyReference(rhs)) => lhs == rhs,
        (
            Expression::Cast { from: lhs_from, to: lhs_to },
            Expression::Cast { from: rhs_from, to: rhs_to },
        ) => lhs_to == rhs_to && same_expression(lhs_from, rhs_from),
        (
            Expression::BinaryExpression { lhs: lhs_lhs, rhs: lhs_rhs, op: lhs_op },
            Expression::BinaryExpression { lhs: rhs_lhs, rhs: rhs_rhs, op: rhs_op },
        ) => {
            lhs_op == rhs_op
                && same_expression(lhs_lhs, rhs_lhs)
                && same_expression(lhs_rhs, rhs_rhs)
        }
        (
            Expression::UnaryOp { sub: lhs_sub, op: lhs_op },
            Expression::UnaryOp { sub: rhs_sub, op: rhs_op },
        ) => lhs_op == rhs_op && same_expression(lhs_sub, rhs_sub),
        (Expression::EnumerationValue(lhs), Expression::EnumerationValue(rhs)) => lhs == rhs,
        (
            Expression::FunctionCall {
                function: Callable::Builtin(lhs_function),
                arguments: lhs_arguments,
                ..
            },
            Expression::FunctionCall {
                function: Callable::Builtin(rhs_function),
                arguments: rhs_arguments,
                ..
            },
        ) => {
            lhs_function == rhs_function
                && lhs_arguments.len() == rhs_arguments.len()
                && lhs_arguments
                    .iter()
                    .zip(rhs_arguments)
                    .all(|(lhs, rhs)| same_expression(lhs, rhs))
        }
        (
            Expression::Struct { ty: lhs_ty, values: lhs_values },
            Expression::Struct { ty: rhs_ty, values: rhs_values },
        ) => {
            lhs_ty.name == rhs_ty.name
                && lhs_ty.fields == rhs_ty.fields
                && lhs_values.len() == rhs_values.len()
                && lhs_values.iter().all(|(name, lhs_value)| {
                    rhs_values
                        .get(name)
                        .is_some_and(|rhs_value| same_expression(lhs_value, rhs_value))
                })
        }
        (
            Expression::Array { element_ty: lhs_ty, values: lhs_values },
            Expression::Array { element_ty: rhs_ty, values: rhs_values },
        ) => {
            lhs_ty == rhs_ty
                && lhs_values.len() == rhs_values.len()
                && lhs_values.iter().zip(rhs_values).all(|(lhs, rhs)| same_expression(lhs, rhs))
        }
        (
            Expression::ImageReference { resource_ref: lhs_ref, .. },
            Expression::ImageReference { resource_ref: rhs_ref, .. },
        ) => same_image_reference(lhs_ref, rhs_ref),
        _ => false,
    }
}

fn same_image_reference(lhs: &ImageReference, rhs: &ImageReference) -> bool {
    match (lhs, rhs) {
        (ImageReference::None, ImageReference::None) => true,
        (ImageReference::AbsolutePath(lhs), ImageReference::AbsolutePath(rhs)) => lhs == rhs,
        (
            ImageReference::EmbeddedData { resource_id: lhs_id, extension: lhs_extension },
            ImageReference::EmbeddedData { resource_id: rhs_id, extension: rhs_extension },
        ) => lhs_id == rhs_id && lhs_extension == rhs_extension,
        (
            ImageReference::EmbeddedTexture { resource_id: lhs_id },
            ImageReference::EmbeddedTexture { resource_id: rhs_id },
        ) => lhs_id == rhs_id,
        _ => false,
    }
}

#[test]
fn warns_for_builtin_defaults_when_enabled() {
    let mut compiler_config =
        crate::CompilerConfiguration::new(crate::generator::OutputFormat::Interpreter);
    compiler_config.warn_redundant_default_properties = true;
    let mut test_diags = crate::diagnostics::BuildDiagnostics::default();
    let doc_node = crate::parser::parse(
        r#"
        export component Test inherits Window {
            TouchArea {
                enabled: true;
                mouse-cursor: MouseCursor.default;
            }
        }
"#
        .into(),
        Some(std::path::Path::new("test.slint")),
        &mut test_diags,
    );
    let (_, diag, _) =
        spin_on::spin_on(crate::compile_syntax_node(doc_node, test_diags, compiler_config));
    assert!(!diag.has_errors(), "{:?}", diag.to_string_vec());

    let warnings = diag
        .iter()
        .filter(|diag| matches!(diag.level(), crate::diagnostics::DiagnosticLevel::Info))
        .map(|diag| diag.message().to_owned())
        .collect::<Vec<_>>();
    let mut warnings = warnings;
    warnings.sort();

    assert_eq!(
        warnings,
        vec![
            "Property 'enabled' is explicitly set to its default value and can be removed",
            "Property 'mouse-cursor' is explicitly set to its default value and can be removed",
        ]
    );
}

#[test]
fn warns_for_geometry_defaults_when_enabled() {
    let mut compiler_config =
        crate::CompilerConfiguration::new(crate::generator::OutputFormat::Interpreter);
    compiler_config.warn_redundant_default_properties = true;
    let mut test_diags = crate::diagnostics::BuildDiagnostics::default();
    let doc_node = crate::parser::parse(
        r#"
        export component Test inherits Window {
            TouchArea {
                width: 100%;
                height: parent.height;
            }

            Rectangle {
                width: 100px;
                height: 50px;
                x: (parent.width - self.width) / 2;
                y: (parent.height - self.height) / 2;
            }
        }
"#
        .into(),
        Some(std::path::Path::new("test.slint")),
        &mut test_diags,
    );
    let (_, diag, _) =
        spin_on::spin_on(crate::compile_syntax_node(doc_node, test_diags, compiler_config));
    assert!(!diag.has_errors(), "{:?}", diag.to_string_vec());

    let warnings = diag
        .iter()
        .filter(|diag| matches!(diag.level(), crate::diagnostics::DiagnosticLevel::Info))
        .map(|diag| diag.message().to_owned())
        .collect::<Vec<_>>();
    let mut warnings = warnings;
    warnings.sort();

    assert_eq!(
        warnings,
        vec![
            "Property 'height' is explicitly set to its default value and can be removed",
            "Property 'width' is explicitly set to its default value and can be removed",
            "Property 'x' is explicitly set to its default value and can be removed",
            "Property 'y' is explicitly set to its default value and can be removed",
        ]
    );
}

#[test]
fn does_not_warn_when_restoring_a_base_override() {
    let mut compiler_config =
        crate::CompilerConfiguration::new(crate::generator::OutputFormat::Interpreter);
    compiler_config.warn_redundant_default_properties = true;
    let mut test_diags = crate::diagnostics::BuildDiagnostics::default();
    let doc_node = crate::parser::parse(
        r#"
        component BaseTouchArea inherits TouchArea {
            enabled: false;
        }

        export component Test inherits Window {
            BaseTouchArea {
                enabled: true;
            }
        }
"#
        .into(),
        Some(std::path::Path::new("test.slint")),
        &mut test_diags,
    );
    let (_, diag, _) =
        spin_on::spin_on(crate::compile_syntax_node(doc_node, test_diags, compiler_config));
    assert!(!diag.has_errors(), "{:?}", diag.to_string_vec());
    assert!(
        diag.iter().all(|diag| !matches!(diag.level(), crate::diagnostics::DiagnosticLevel::Info)),
        "{:?}",
        diag.to_string_vec()
    );
}

#[test]
fn warns_for_equivalent_fill_parent_expression() {
    let mut compiler_config =
        crate::CompilerConfiguration::new(crate::generator::OutputFormat::Interpreter);
    compiler_config.warn_redundant_default_properties = true;
    let mut test_diags = crate::diagnostics::BuildDiagnostics::default();
    let doc_node = crate::parser::parse(
        r#"
        export component Test inherits Window {
            TouchArea {
                width: 1 * parent.width;
            }
        }
"#
        .into(),
        Some(std::path::Path::new("test.slint")),
        &mut test_diags,
    );
    let (_, diag, _) =
        spin_on::spin_on(crate::compile_syntax_node(doc_node, test_diags, compiler_config));
    assert!(!diag.has_errors(), "{:?}", diag.to_string_vec());

    let infos = diag
        .iter()
        .filter(|diag| matches!(diag.level(), crate::diagnostics::DiagnosticLevel::Info))
        .map(|diag| diag.message().to_owned())
        .collect::<Vec<_>>();

    assert_eq!(
        infos,
        vec!["Property 'width' is explicitly set to its default value and can be removed"]
    );
}

#[test]
fn does_not_warn_for_layout_managed_geometry() {
    let mut compiler_config =
        crate::CompilerConfiguration::new(crate::generator::OutputFormat::Interpreter);
    compiler_config.warn_redundant_default_properties = true;
    let mut test_diags = crate::diagnostics::BuildDiagnostics::default();
    let doc_node = crate::parser::parse(
        r#"
        export component Test inherits Window {
            HorizontalLayout {
                TouchArea {
                    width: 100%;
                    height: 20px;
                }
            }
        }
"#
        .into(),
        Some(std::path::Path::new("test.slint")),
        &mut test_diags,
    );
    let (_, diag, _) =
        spin_on::spin_on(crate::compile_syntax_node(doc_node, test_diags, compiler_config));
    assert!(!diag.has_errors(), "{:?}", diag.to_string_vec());
    assert!(
        diag.iter().all(|diag| !matches!(diag.level(), crate::diagnostics::DiagnosticLevel::Info)),
        "{:?}",
        diag.to_string_vec()
    );
}
