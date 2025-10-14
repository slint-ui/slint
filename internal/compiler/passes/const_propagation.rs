// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

//! Try to simplify property bindings by propagating constant expressions

use super::GlobalAnalysis;
use crate::expression_tree::*;
use crate::langtype::ElementType;
use crate::langtype::Type;
use crate::object_tree::*;
use smol_str::{format_smolstr, ToSmolStr};

pub fn const_propagation(component: &Component, global_analysis: &GlobalAnalysis) {
    visit_all_expressions(component, |expr, ty| {
        if matches!(ty(), Type::Callback { .. }) {
            return;
        }
        simplify_expression(expr, global_analysis);
    });
}

/// Returns false if the expression still contains a reference to an element
fn simplify_expression(expr: &mut Expression, ga: &GlobalAnalysis) -> bool {
    match expr {
        Expression::PropertyReference(nr) => {
            if nr.is_constant()
                && !match nr.ty() {
                    Type::Struct(s) => {
                        s.name.as_ref().is_some_and(|name| name.ends_with("::StateInfo"))
                    }
                    _ => false,
                }
            {
                // Inline the constant value
                if let Some(result) = extract_constant_property_reference(nr, ga) {
                    *expr = result;
                    return true;
                }
            }
            false
        }
        Expression::BinaryExpression { lhs, op, rhs } => {
            let mut can_inline = simplify_expression(lhs, ga);
            can_inline &= simplify_expression(rhs, ga);

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
                ('>', Expression::NumberLiteral(a, un1), Expression::NumberLiteral(b, un2))
                    if un1 == un2 =>
                {
                    Some(Expression::BoolLiteral(*a > *b))
                }
                ('<', Expression::NumberLiteral(a, un1), Expression::NumberLiteral(b, un2))
                    if un1 == un2 =>
                {
                    Some(Expression::BoolLiteral(*a < *b))
                }
                _ => None,
            };
            if let Some(new) = new {
                *expr = new;
            }
            can_inline
        }
        Expression::UnaryOp { sub, op } => {
            let can_inline = simplify_expression(sub, ga);
            let new = match (*op, &mut **sub) {
                ('!', Expression::BoolLiteral(b)) => Some(Expression::BoolLiteral(!*b)),
                ('-', Expression::NumberLiteral(n, u)) => Some(Expression::NumberLiteral(-*n, *u)),
                ('+', Expression::NumberLiteral(n, u)) => Some(Expression::NumberLiteral(*n, *u)),
                _ => None,
            };
            if let Some(new) = new {
                *expr = new;
            }
            can_inline
        }
        Expression::StructFieldAccess { base, name } => {
            let r = simplify_expression(base, ga);
            if let Expression::Struct { values, .. } = &mut **base {
                if let Some(e) = values.remove(name) {
                    *expr = e;
                    return simplify_expression(expr, ga);
                }
            }
            r
        }
        Expression::Cast { from, to } => {
            let can_inline = simplify_expression(from, ga);
            let new = if from.ty() == *to {
                Some(std::mem::take(&mut **from))
            } else {
                match (&**from, to) {
                    (Expression::NumberLiteral(x, Unit::None), Type::String) => {
                        Some(Expression::StringLiteral(x.to_smolstr()))
                    }
                    (Expression::Struct { values, .. }, Type::Struct(ty)) => {
                        Some(Expression::Struct { ty: ty.clone(), values: values.clone() })
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
            let can_inline = simplify_expression(lhs, ga) & simplify_expression(rhs, ga);
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
        Expression::Condition { condition, true_expr, false_expr } => {
            let mut can_inline = simplify_expression(condition, ga);
            can_inline &= match &**condition {
                Expression::BoolLiteral(true) => {
                    *expr = *true_expr.clone();
                    simplify_expression(expr, ga)
                }
                Expression::BoolLiteral(false) => {
                    *expr = *false_expr.clone();
                    simplify_expression(expr, ga)
                }
                _ => simplify_expression(true_expr, ga) & simplify_expression(false_expr, ga),
            };
            can_inline
        }
        // disable this simplification for store local variable, as "let" is not an expression in rust
        Expression::CodeBlock(stmts)
            if stmts.len() == 1 && !matches!(stmts[0], Expression::StoreLocalVariable { .. }) =>
        {
            *expr = stmts[0].clone();
            simplify_expression(expr, ga)
        }
        Expression::FunctionCall { function, arguments, .. } => {
            let mut args_can_inline = true;
            for arg in arguments.iter_mut() {
                args_can_inline &= simplify_expression(arg, ga);
            }
            if args_can_inline {
                if let Some(inlined) = try_inline_function(function, arguments, ga) {
                    *expr = inlined;
                    return true;
                }
            }
            false
        }
        Expression::ElementReference { .. } => false,
        Expression::LayoutCacheAccess { .. } => false,
        Expression::SolveLayout { .. } => false,
        Expression::ComputeLayoutInfo { .. } => false,
        _ => {
            let mut result = true;
            expr.visit_mut(|expr| result &= simplify_expression(expr, ga));
            result
        }
    }
}

/// Will extract the property binding from the given named reference
/// and propagate constant expression within it. If that's possible,
/// return the new expression
fn extract_constant_property_reference(
    nr: &NamedReference,
    ga: &GlobalAnalysis,
) -> Option<Expression> {
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
                return extract_constant_property_reference(alias, ga);
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
    if !(simplify_expression(&mut expression, ga)) {
        return None;
    }
    Some(expression)
}

fn try_inline_function(
    function: &Callable,
    arguments: &[Expression],
    ga: &GlobalAnalysis,
) -> Option<Expression> {
    let function = match function {
        Callable::Function(function) => function,
        Callable::Builtin(b) => return try_inline_builtin_function(b, arguments, ga),
        _ => return None,
    };
    if !function.is_constant() {
        return None;
    }
    let mut body = extract_constant_property_reference(function, ga)?;

    fn substitute_arguments_recursive(e: &mut Expression, arguments: &[Expression]) {
        if let Expression::FunctionParameterReference { index, ty } = e {
            let e_new = arguments.get(*index).expect("reference to invalid arg").clone();
            debug_assert_eq!(e_new.ty(), *ty);
            *e = e_new;
        } else {
            e.visit_mut(|e| substitute_arguments_recursive(e, arguments));
        }
    }
    substitute_arguments_recursive(&mut body, arguments);

    if simplify_expression(&mut body, ga) {
        Some(body)
    } else {
        None
    }
}

fn try_inline_builtin_function(
    b: &BuiltinFunction,
    args: &[Expression],
    ga: &GlobalAnalysis,
) -> Option<Expression> {
    let a = |idx: usize| -> Option<f64> {
        match args.get(idx)? {
            Expression::NumberLiteral(n, Unit::None) => Some(*n),
            _ => None,
        }
    };
    let num = |n: f64| Some(Expression::NumberLiteral(n, Unit::None));

    match b {
        BuiltinFunction::GetWindowDefaultFontSize => match ga.default_font_size {
            crate::passes::binding_analysis::DefaultFontSize::LogicalValue(val) => {
                Some(Expression::NumberLiteral(val as _, Unit::Px))
            }
            _ => None,
        },
        BuiltinFunction::Mod => num(a(0)?.rem_euclid(a(1)?)),
        BuiltinFunction::Round => num(a(0)?.round()),
        BuiltinFunction::Ceil => num(a(0)?.ceil()),
        BuiltinFunction::Floor => num(a(0)?.floor()),
        BuiltinFunction::Abs => num(a(0)?.abs()),
        _ => None,
    }
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
enum Enum { aa, bb, cc }
global G {
    pure function complicated(a: float ) -> bool { if a > 5 { return true; }; if a < 1 { return true; }; uncomplicated() }
    pure function uncomplicated( ) -> bool { false }
    out property <float> p : 3 * 2 + 15 ;
    property <string> q: "foo " + 42;
    out property <float> w : -p / 2;
    out property <Hello> out: { s: q, v: complicated(w + 15) ? -123 : p };

    in-out property <Enum> e: Enum.bb;
}
export component Foo {
    in property <int> input;
    out property<float> out1: G.w;
    out property<float> out2: G.out.v;
    out property<bool> out3: false ? input == 12 : input > 0 ? input == 11 : G.e == Enum.bb;
}
"#
        .into(),
        Some(std::path::Path::new("HELLO")),
        &mut test_diags,
    );
    let (doc, diag, _) =
        spin_on::spin_on(crate::compile_syntax_node(doc_node, test_diags, compiler_config));
    assert!(!diag.has_errors(), "slint compile error {:#?}", diag.to_string_vec());

    let expected_p = 3.0 * 2.0 + 15.0;
    let expected_w = -expected_p / 2.0;
    let bindings = &doc.inner_components.last().unwrap().root_element.borrow().bindings;
    let out1_binding = bindings.get("out1").unwrap().borrow().expression.clone();
    match &out1_binding {
        Expression::NumberLiteral(n, _) => assert_eq!(*n, expected_w),
        _ => panic!("not number {out1_binding:?}"),
    }
    let out2_binding = bindings.get("out2").unwrap().borrow().expression.clone();
    match &out2_binding {
        Expression::NumberLiteral(n, _) => assert_eq!(*n, expected_p),
        _ => panic!("not number {out2_binding:?}"),
    }
    let out3_binding = bindings.get("out3").unwrap().borrow().expression.clone();
    match &out3_binding {
        // We have a code block because the first entry stores the value of `intput` in a local variable
        Expression::CodeBlock(stmts) => match &stmts[1] {
            Expression::Condition { condition: _, true_expr: _, false_expr } => match &**false_expr
            {
                Expression::BoolLiteral(b) => assert_eq!(*b, true),
                _ => panic!("false_expr not optimized in : {out3_binding:?}"),
            },
            _ => panic!("not condition:  {out3_binding:?}"),
        },
        _ => panic!("not code block: {out3_binding:?}"),
    };
}

#[test]
fn test_propagate_font_size() {
    struct Case {
        default_font_size: &'static str,
        another_window: &'static str,
        check_expression: fn(&Expression),
    }

    #[track_caller]
    fn assert_expr_is_mul(e: &Expression, l: f64, r: f64) {
        assert!(
            matches!(e, Expression::Cast { from, .. }
                        if matches!(from.as_ref(), Expression::BinaryExpression { lhs, rhs, op: '*'}
                        if matches!((lhs.as_ref(), rhs.as_ref()), (Expression::NumberLiteral(lhs, _), Expression::NumberLiteral(rhs, _)) if *lhs == l && *rhs == r ))),
            "Expression {e:?} is not a {l} * {r} expected"
        );
    }

    for Case { default_font_size, another_window, check_expression } in [
        Case {
            default_font_size: "default-font-size: 12px;",
            another_window: "",
            check_expression: |e| assert_expr_is_mul(e, 5.0, 12.0)
        },
        Case {
            default_font_size: "default-font-size: some-value;",
            another_window: "",
            check_expression: |e|  {
                assert!(!e.is_constant(None), "{e:?} should not be constant since some-value can vary at runtime");
            },
        },
        Case {
            default_font_size: "default-font-size: 25px;",
            another_window: "export component AnotherWindow inherits Window { default-font-size: 8px; }",
            check_expression: |e|  {
                assert!(e.is_constant(None) && !matches!(e, Expression::NumberLiteral(_,_ )), "{e:?} should be constant but not known at compile time since there are two windows");
            },
        },
        Case {
            default_font_size: "default-font-size: 25px;",
            another_window: "export component AnotherWindow inherits Window { }",
            check_expression: |e|  {
                assert!(!e.is_constant(None), "should not be const since at least one window has it unset");
            },
        },
        Case {
            default_font_size: "default-font-size: 20px;",
            another_window: "export component AnotherWindow inherits Window { default-font-size: 20px;  }",
            check_expression: |e| assert_expr_is_mul(e, 5.0, 20.0)
        },
        Case {
            default_font_size: "default-font-size: 20px;",
            another_window: "export component AnotherWindow inherits Window { in property <float> f: 1; default-font-size: 20px*f;  }",
            check_expression: |e| {
                assert!(!e.is_constant(None), "{e:?} should not be constant since 'f' can vary at runtime");
            },
        },

    ] {
        let source = format!(
            r#"
component SomeComponent {{
    in-out property <length> rem-prop: 5rem;
}}

{another_window}

export component Foo inherits Window {{
    in property <length> some-value: 45px;
    {default_font_size}
    sc1 := SomeComponent {{}}
    sc2 := SomeComponent {{}}

    out property <length> test: sc1.rem-prop;
}}
"#
        );

        let mut test_diags = crate::diagnostics::BuildDiagnostics::default();

        let doc_node = crate::parser::parse(
            source.clone(),
            Some(std::path::Path::new("HELLO")),
            &mut test_diags,
        );
        let mut compiler_config =
            crate::CompilerConfiguration::new(crate::generator::OutputFormat::Interpreter);
        compiler_config.style = Some("fluent".into());
        let (doc, diag, _) =
            spin_on::spin_on(crate::compile_syntax_node(doc_node, test_diags, compiler_config));
        assert!(!diag.has_errors(), "slint compile error {:#?}", diag.to_string_vec());

        let bindings = &doc.inner_components.last().unwrap().root_element.borrow().bindings;
        let out1_binding = bindings.get("test").unwrap().borrow().expression.clone();
        check_expression(&out1_binding);
    }
}
