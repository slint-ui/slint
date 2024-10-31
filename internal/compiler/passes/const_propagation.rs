// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

//! Try to simplify property bindings by propagating constant expressions

use crate::expression_tree::*;
use crate::langtype::ElementType;
use crate::langtype::Type;
use crate::object_tree::*;
use smol_str::{format_smolstr, ToSmolStr};

pub fn const_propagation(component: &Component) {
    visit_all_expressions(component, |expr, ty| {
        if matches!(ty(), Type::Callback { .. }) {
            return;
        }
        simplify_expression(expr);
    });
}

/// Returns false if the expression still contains a reference to an element
fn simplify_expression(expr: &mut Expression) -> bool {
    match expr {
        Expression::PropertyReference(nr) => {
            if nr.is_constant()
                && !match nr.ty() {
                    Type::Struct(s) => {
                        s.name.as_ref().map_or(false, |name| name.ends_with("::StateInfo"))
                    }
                    _ => false,
                }
            {
                // Inline the constant value
                if let Some(result) = extract_constant_property_reference(nr) {
                    *expr = result;
                    return true;
                }
            }
            false
        }
        Expression::BinaryExpression { lhs, op, rhs } => {
            let mut can_inline = simplify_expression(lhs);
            can_inline &= simplify_expression(rhs);

            let new = match (*op, &mut **lhs, &mut **rhs) {
                ('+', Expression::StringLiteral(a), Expression::StringLiteral(b)) => {
                    Some(Expression::StringLiteral(format_smolstr!("{}{}", a, b)))
                }
                ('+', Expression::NumberLiteral(a, un1), Expression::NumberLiteral(b, un2))
                    if un1 == un2 =>
                {
                    Some(Expression::NumberLiteral(*a + *b, *un1))
                }
                ('-', Expression::NumberLiteral(a, un1), Expression::NumberLiteral(b, un2))
                    if un1 == un2 =>
                {
                    Some(Expression::NumberLiteral(*a - *b, *un1))
                }
                ('*', Expression::NumberLiteral(a, un1), Expression::NumberLiteral(b, un2))
                    if *un1 == Unit::None || *un2 == Unit::None =>
                {
                    let preserved_unit = if *un1 == Unit::None { *un2 } else { *un1 };
                    Some(Expression::NumberLiteral(*a * *b, preserved_unit))
                }
                (
                    '/',
                    Expression::NumberLiteral(a, un1),
                    Expression::NumberLiteral(b, Unit::None),
                ) => Some(Expression::NumberLiteral(*a / *b, *un1)),
                // TODO: take care of * and / when both numbers have units
                ('=' | '!', Expression::NumberLiteral(a, _), Expression::NumberLiteral(b, _)) => {
                    Some(Expression::BoolLiteral((a == b) == (*op == '=')))
                }
                ('=' | '!', Expression::StringLiteral(a), Expression::StringLiteral(b)) => {
                    Some(Expression::BoolLiteral((a == b) == (*op == '=')))
                }
                ('=' | '!', Expression::EnumerationValue(a), Expression::EnumerationValue(b)) => {
                    Some(Expression::BoolLiteral((a == b) == (*op == '=')))
                }
                // TODO: more types and more comparison operators
                ('&', Expression::BoolLiteral(false), _) => {
                    can_inline = true;
                    Some(Expression::BoolLiteral(false))
                }
                ('&', _, Expression::BoolLiteral(false)) => {
                    can_inline = true;
                    Some(Expression::BoolLiteral(false))
                }
                ('&', Expression::BoolLiteral(true), e) => Some(std::mem::take(e)),
                ('&', e, Expression::BoolLiteral(true)) => Some(std::mem::take(e)),
                ('|', Expression::BoolLiteral(true), _) => {
                    can_inline = true;
                    Some(Expression::BoolLiteral(true))
                }
                ('|', _, Expression::BoolLiteral(true)) => {
                    can_inline = true;
                    Some(Expression::BoolLiteral(true))
                }
                ('|', Expression::BoolLiteral(false), e) => Some(std::mem::take(e)),
                ('|', e, Expression::BoolLiteral(false)) => Some(std::mem::take(e)),
                _ => None,
            };
            if let Some(new) = new {
                *expr = new;
            }
            can_inline
        }
        Expression::StructFieldAccess { base, name } => {
            let r = simplify_expression(base);
            if let Expression::Struct { values, .. } = &mut **base {
                if let Some(e) = values.remove(name) {
                    *expr = e;
                    return simplify_expression(expr);
                }
            }
            r
        }
        Expression::Cast { from, to } => {
            let can_inline = simplify_expression(from);
            let new = if from.ty() == *to {
                Some(std::mem::take(&mut **from))
            } else {
                match (&**from, to) {
                    (Expression::NumberLiteral(x, Unit::None), Type::String) => {
                        Some(Expression::StringLiteral((*x).to_smolstr()))
                    }
                    (Expression::Struct { values, .. }, to @ Type::Struct { .. }) => {
                        Some(Expression::Struct { ty: to.clone(), values: values.clone() })
                    }
                    _ => None,
                }
            };
            if let Some(new) = new {
                *expr = new;
            }
            can_inline
        }
        Expression::MinMax { op, lhs, rhs, ty: _ } => {
            let can_inline = simplify_expression(lhs) && simplify_expression(rhs);
            if let (Expression::NumberLiteral(lhs, u), Expression::NumberLiteral(rhs, _)) =
                (&**lhs, &**rhs)
            {
                let v = match op {
                    MinMaxOp::Min => lhs.min(*rhs),
                    MinMaxOp::Max => lhs.max(*rhs),
                };
                *expr = Expression::NumberLiteral(v, *u);
            }
            can_inline
        }
        Expression::CallbackReference { .. } => false,
        Expression::ElementReference { .. } => false,
        // FIXME
        Expression::FunctionReference { .. } => false,
        Expression::LayoutCacheAccess { .. } => false,
        Expression::SolveLayout { .. } => false,
        Expression::ComputeLayoutInfo { .. } => false,
        _ => {
            let mut result = true;
            expr.visit_mut(|expr| result &= simplify_expression(expr));
            result
        }
    }
}

/// Will extract the property binding from the given named reference
/// and propagate constant expression within it. If that's possible,
/// return the new expression
fn extract_constant_property_reference(nr: &NamedReference) -> Option<Expression> {
    debug_assert!(nr.is_constant());
    // find the binding.
    let mut element = nr.element();
    let mut expression = loop {
        if let Some(binding) = element.borrow().bindings.get(nr.name()) {
            let binding = binding.borrow();
            if !binding.two_way_bindings.is_empty() {
                // TODO: In practice, we should still find out what the real binding is
                // and solve that.
                return None;
            }
            if !matches!(binding.expression, Expression::Invalid) {
                break binding.expression.clone();
            }
        };
        if let Some(decl) = element.clone().borrow().property_declarations.get(nr.name()) {
            if let Some(alias) = &decl.is_alias {
                return extract_constant_property_reference(alias);
            }
        } else if let ElementType::Component(c) = &element.clone().borrow().base_type {
            element = c.root_element.clone();
            continue;
        }

        // There is no binding for this property, return the default value
        let ty = nr.ty();
        debug_assert!(!matches!(ty, Type::Invalid));
        return Some(Expression::default_value_for_type(&ty));
    };
    if !(simplify_expression(&mut expression)) {
        return None;
    }
    Some(expression)
}

#[test]
fn test() {
    let mut compiler_config =
        crate::CompilerConfiguration::new(crate::generator::OutputFormat::Interpreter);
    compiler_config.style = Some("fluent".into());
    let mut test_diags = crate::diagnostics::BuildDiagnostics::default();
    let doc_node = crate::parser::parse(
        r#"
/* ... */
struct Hello { s: string, v: float }
global G {
    property <float> p : 3 * 2 + 15 ;
    property <string> q: "foo " + 42;
    out property <Hello> out: { s: q, v: p };
}
export component Foo {
    out property<float> out: G.out.v;
}
"#
        .into(),
        Some(std::path::Path::new("HELLO")),
        &mut test_diags,
    );
    let (doc, diag, _) =
        spin_on::spin_on(crate::compile_syntax_node(doc_node, test_diags, compiler_config));
    assert!(!diag.has_errors());

    let out_binding = doc
        .inner_components
        .last()
        .unwrap()
        .root_element
        .borrow()
        .bindings
        .get("out")
        .unwrap()
        .borrow()
        .expression
        .clone();
    match &out_binding {
        Expression::NumberLiteral(n, _) => assert_eq!(*n, (3 * 2 + 15) as f64),
        _ => panic!("not number {out_binding:?}"),
    }
}
