// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

//! Try to simplify property bindings by propagating constant expressions

use std::collections::HashMap;

use super::GlobalAnalysis;
use crate::expression_tree::*;
use crate::langtype::{BuiltinStruct, ElementType, StructName, Type};
use crate::namedreference::NamedReference;
use crate::object_tree::*;
use smol_str::format_smolstr;

type ConstPropCache = HashMap<NamedReference, Option<Expression>>;

/// Fold constants in an expression that stands on its own, outside of a component.
///
/// This is used for expressions that cannot reference any properties or elements,
/// such as the default values of struct fields.
pub(crate) fn fold_const_expression(expr: &mut Expression) {
    simplify_expression(expr, &GlobalAnalysis::default(), &mut ConstPropCache::default());
}

pub fn const_propagation(component: &Component, global_analysis: &GlobalAnalysis) {
    let mut cache = ConstPropCache::new();
    visit_all_expressions(component, |expr, _ty| {
        simplify_expression(expr, global_analysis, &mut cache);
    });

    // The binding analysis classifies conversions such as float to string as non-constant
    // because their result depends on the locale's decimal separator. When the
    // simplification folded the conversion away, the binding is constant after all:
    // promote it back.
    recurse_elem_including_sub_components_no_borrow(component, &(), &mut |elem, _| {
        for binding in elem.borrow().bindings.values() {
            let Ok(mut binding) = binding.try_borrow_mut() else { continue };
            let Some(analysis) = binding.analysis.as_ref() else { continue };
            if analysis.is_const || matches!(binding.expression, Expression::Invalid) {
                continue;
            }
            if binding.expression.is_constant(Some(global_analysis))
                && binding.two_way_bindings.iter().all(|tw| tw.is_constant())
            {
                binding.analysis.as_mut().unwrap().is_const = true;
            }
        }
    });
}

/// Returns false if the expression still contains a reference to an element
///
/// The body of every non-trivial match arm lives in its own `#[inline(never)]`
/// helper function: this function recurses for nested expressions, and with all
/// arm bodies inlined, its stack frame in unoptimized builds becomes so large
/// that deeply nested expressions overflow the stack.
fn simplify_expression(
    expr: &mut Expression,
    ga: &GlobalAnalysis,
    cache: &mut ConstPropCache,
) -> bool {
    match expr {
        Expression::PropertyReference(..) => simplify_property_reference(expr, ga, cache),
        Expression::BinaryExpression { .. } => simplify_binary_expression(expr, ga, cache),
        Expression::UnaryOp { .. } => simplify_unary_op(expr, ga, cache),
        Expression::StructFieldAccess { .. } => simplify_struct_field_access(expr, ga, cache),
        Expression::Cast { .. } => simplify_cast(expr, ga, cache),
        Expression::MinMax { .. } => simplify_min_max(expr, ga, cache),
        Expression::Condition { .. } => simplify_condition(expr, ga, cache),
        // disable this simplification for store local variable, as "let" is not an expression in rust
        Expression::CodeBlock(stmts)
            if stmts.len() == 1 && !matches!(stmts[0], Expression::StoreLocalVariable { .. }) =>
        {
            simplify_single_statement_code_block(expr, ga, cache)
        }
        Expression::FunctionCall { .. } => simplify_function_call(expr, ga, cache),
        Expression::ElementReference { .. } => false,
        Expression::LayoutCacheAccess { .. } => false,
        Expression::OrganizeGridLayout { .. } => false,
        Expression::SolveBoxLayout { .. } => false,
        Expression::SolveGridLayout { .. } => false,
        Expression::SolveFlexboxLayout { .. } => false,
        Expression::ComputeBoxLayoutInfo { .. } => false,
        Expression::ComputeGridLayoutInfo { .. } => false,
        Expression::ComputeFlexboxLayoutInfo { .. } => false,
        _ => {
            let mut result = true;
            expr.visit_mut(|expr| result &= simplify_expression(expr, ga, cache));
            result
        }
    }
}

#[inline(never)]
fn simplify_property_reference(
    expr: &mut Expression,
    ga: &GlobalAnalysis,
    cache: &mut ConstPropCache,
) -> bool {
    let Expression::PropertyReference(nr) = expr else { unreachable!() };
    if nr.is_constant()
        && !match nr.ty() {
            Type::Struct(s) => {
                matches!(s.name, StructName::Builtin(BuiltinStruct::StateInfo))
            }
            _ => false,
        }
    {
        // Inline the constant value
        if let Some(result) = extract_constant_property_reference(nr, ga, cache) {
            *expr = result;
            return true;
        }
    }
    false
}

#[inline(never)]
fn simplify_binary_expression(
    expr: &mut Expression,
    ga: &GlobalAnalysis,
    cache: &mut ConstPropCache,
) -> bool {
    let Expression::BinaryExpression { lhs, op, rhs } = expr else { unreachable!() };
    let mut can_inline = simplify_expression(lhs, ga, cache);
    can_inline &= simplify_expression(rhs, ga, cache);

    // The folding lives in a separate function: in unoptimized builds its many
    // `Expression` temporaries would otherwise be part of this function's stack
    // frame, which is live during the recursion above.
    let new = fold_binary_expression(*op, lhs, rhs, &mut can_inline);
    if let Some(new) = new {
        *expr = new;
    }
    can_inline
}

#[inline(never)]
fn fold_binary_expression(
    op: char,
    lhs: &mut Expression,
    rhs: &mut Expression,
    can_inline: &mut bool,
) -> Option<Expression> {
    match (op, lhs, rhs) {
        // constant folding
        ('+', Expression::StringLiteral(a), Expression::StringLiteral(b)) => {
            Some(Expression::StringLiteral(format_smolstr!("{}{}", a, b)))
        }
        ('+', Expression::NumberLiteral(a, un1), Expression::NumberLiteral(b, _)) => {
            Some(Expression::NumberLiteral(*a + *b, *un1))
        }
        ('-', Expression::NumberLiteral(a, un1), Expression::NumberLiteral(b, _)) => {
            Some(Expression::NumberLiteral(*a - *b, *un1))
        }
        ('*', Expression::NumberLiteral(a, un1), Expression::NumberLiteral(b, un2))
            if *un1 == Unit::None || *un2 == Unit::None =>
        {
            let preserved_unit = if *un1 == Unit::None { *un2 } else { *un1 };
            Some(Expression::NumberLiteral(*a * *b, preserved_unit))
        }
        ('/', Expression::NumberLiteral(a, un1), Expression::NumberLiteral(b, Unit::None)) => {
            Some(Expression::NumberLiteral(*a / *b, *un1))
        }
        ('/', Expression::NumberLiteral(a, un1), Expression::NumberLiteral(b, un2))
            if un1 == un2 =>
        {
            Some(Expression::NumberLiteral(*a / *b, Unit::None))
        }
        // TODO: fold * and / that produce a unit product

        // arithmetic identities
        ('+', e, Expression::NumberLiteral(n, _))
        | ('+', Expression::NumberLiteral(n, _), e)
        | ('-', e, Expression::NumberLiteral(n, _))
            if *n == 0. =>
        {
            Some(std::mem::take(e))
        }
        ('*', e, Expression::NumberLiteral(n, Unit::None))
        | ('*', Expression::NumberLiteral(n, Unit::None), e)
        | ('/', e, Expression::NumberLiteral(n, Unit::None))
            if *n == 1. =>
        {
            Some(std::mem::take(e))
        }

        // comparisons
        (
            '=' | '!' | '<' | '>' | '≤' | '≥',
            Expression::NumberLiteral(a, _),
            Expression::NumberLiteral(b, _),
        ) => Some(Expression::BoolLiteral(match op {
            '=' => a == b,
            '!' => a != b,
            '<' => a < b,
            '>' => a > b,
            '≤' => a <= b,
            _ => a >= b,
        })),
        ('=' | '!', Expression::StringLiteral(a), Expression::StringLiteral(b)) => {
            Some(Expression::BoolLiteral((a == b) == (op == '=')))
        }
        ('=' | '!', Expression::EnumerationValue(a), Expression::EnumerationValue(b)) => {
            Some(Expression::BoolLiteral((a == b) == (op == '=')))
        }
        ('=' | '!', Expression::BoolLiteral(a), Expression::BoolLiteral(b)) => {
            Some(Expression::BoolLiteral((a == b) == (op == '=')))
        }
        // TODO: more types and more comparison operators

        // boolean logic
        ('&', Expression::BoolLiteral(a), Expression::BoolLiteral(b)) => {
            Some(Expression::BoolLiteral(*a && *b))
        }
        ('|', Expression::BoolLiteral(a), Expression::BoolLiteral(b)) => {
            Some(Expression::BoolLiteral(*a || *b))
        }
        ('&', Expression::BoolLiteral(false), _) => {
            *can_inline = true;
            Some(Expression::BoolLiteral(false))
        }
        ('|', Expression::BoolLiteral(true), _) => {
            *can_inline = true;
            Some(Expression::BoolLiteral(true))
        }
        ('&', Expression::BoolLiteral(true), e)
        | ('&', e, Expression::BoolLiteral(true))
        | ('|', Expression::BoolLiteral(false), e)
        | ('|', e, Expression::BoolLiteral(false)) => Some(std::mem::take(e)),
        _ => None,
    }
}

#[inline(never)]
fn simplify_unary_op(
    expr: &mut Expression,
    ga: &GlobalAnalysis,
    cache: &mut ConstPropCache,
) -> bool {
    let Expression::UnaryOp { sub, op } = expr else { unreachable!() };
    let can_inline = simplify_expression(sub, ga, cache);
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

#[inline(never)]
fn simplify_struct_field_access(
    expr: &mut Expression,
    ga: &GlobalAnalysis,
    cache: &mut ConstPropCache,
) -> bool {
    let Expression::StructFieldAccess { base, name } = expr else { unreachable!() };
    if let Expression::PropertyReference(nr) = &**base
        && nr.is_constant()
        && let Some(field_expr) = extract_struct_field_from_constant(nr, name, ga, cache)
    {
        *expr = field_expr;
        return simplify_expression(expr, ga, cache);
    }
    let r = simplify_expression(base, ga, cache);
    if let Expression::Struct { values, .. } = &mut **base
        && let Some(e) = values.remove(name)
    {
        *expr = e;
        return simplify_expression(expr, ga, cache);
    }
    r
}

#[inline(never)]
fn simplify_cast(expr: &mut Expression, ga: &GlobalAnalysis, cache: &mut ConstPropCache) -> bool {
    let Expression::Cast { from, to } = expr else { unreachable!() };
    let can_inline = simplify_expression(from, ga, cache);
    let new = if from.ty() == *to {
        Some(std::mem::take(&mut **from))
    } else {
        match (&**from, &*to) {
            (Expression::NumberLiteral(x, Unit::None), Type::String) => {
                locale_independent_number_to_string(*x).map(Expression::StringLiteral)
            }
            (Expression::NumberLiteral(x, _), Type::Float32) => {
                Some(Expression::NumberLiteral(*x, Unit::None))
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

#[inline(never)]
fn simplify_min_max(
    expr: &mut Expression,
    ga: &GlobalAnalysis,
    cache: &mut ConstPropCache,
) -> bool {
    let Expression::MinMax { op, lhs, rhs, ty: _ } = expr else { unreachable!() };
    let can_inline = simplify_expression(lhs, ga, cache) & simplify_expression(rhs, ga, cache);
    if let (Expression::NumberLiteral(lhs, u), Expression::NumberLiteral(rhs, _)) = (&**lhs, &**rhs)
    {
        let v = match op {
            MinMaxOp::Min => lhs.min(*rhs),
            MinMaxOp::Max => lhs.max(*rhs),
        };
        *expr = Expression::NumberLiteral(v, *u);
    }
    can_inline
}

#[inline(never)]
fn simplify_condition(
    expr: &mut Expression,
    ga: &GlobalAnalysis,
    cache: &mut ConstPropCache,
) -> bool {
    let Expression::Condition { condition, true_expr, false_expr } = expr else { unreachable!() };
    let mut can_inline = simplify_expression(condition, ga, cache);
    can_inline &= match &**condition {
        Expression::BoolLiteral(true) => {
            *expr = *true_expr.clone();
            simplify_expression(expr, ga, cache)
        }
        Expression::BoolLiteral(false) => {
            *expr = *false_expr.clone();
            simplify_expression(expr, ga, cache)
        }
        _ => simplify_expression(true_expr, ga, cache) & simplify_expression(false_expr, ga, cache),
    };
    can_inline
}

#[inline(never)]
fn simplify_single_statement_code_block(
    expr: &mut Expression,
    ga: &GlobalAnalysis,
    cache: &mut ConstPropCache,
) -> bool {
    let Expression::CodeBlock(stmts) = expr else { unreachable!() };
    *expr = stmts[0].clone();
    simplify_expression(expr, ga, cache)
}

#[inline(never)]
fn simplify_function_call(
    expr: &mut Expression,
    ga: &GlobalAnalysis,
    cache: &mut ConstPropCache,
) -> bool {
    let Expression::FunctionCall { function, arguments, .. } = expr else { unreachable!() };
    let mut args_can_inline = true;
    for arg in arguments.iter_mut() {
        args_can_inline &= simplify_expression(arg, ga, cache);
    }
    if args_can_inline && let Some(inlined) = try_inline_function(function, arguments, ga, cache) {
        *expr = inlined;
        return true;
    }
    false
}

/// Will extract the property binding from the given named reference
/// and propagate constant expression within it. If that's possible,
/// return the new expression. Results are cached per NamedReference.
fn extract_constant_property_reference(
    nr: &NamedReference,
    ga: &GlobalAnalysis,
    cache: &mut ConstPropCache,
) -> Option<Expression> {
    debug_assert!(nr.is_constant());
    if let Some(cached) = cache.get(nr) {
        return cached.clone();
    }
    let result = extract_constant_property_reference_impl(nr, ga, cache);
    cache.insert(nr.clone(), result.clone());
    result
}

/// Extract just one field from a constant struct property, cloning only that
/// field instead of the entire struct expression.
fn extract_struct_field_from_constant(
    nr: &NamedReference,
    field_name: &str,
    ga: &GlobalAnalysis,
    cache: &mut ConstPropCache,
) -> Option<Expression> {
    // Populate the cache via the canonical path (result itself is discarded)
    let _ = extract_constant_property_reference(nr, ga, cache);
    if let Some(Some(Expression::Struct { values, .. })) = cache.get(nr) {
        values.get(field_name).cloned()
    } else {
        None
    }
}

fn extract_constant_property_reference_impl(
    nr: &NamedReference,
    ga: &GlobalAnalysis,
    cache: &mut ConstPropCache,
) -> Option<Expression> {
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
                return extract_constant_property_reference(alias, ga, cache);
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
    if !(simplify_expression(&mut expression, ga, cache)) {
        return None;
    }
    Some(expression)
}

fn try_inline_function(
    function: &Callable,
    arguments: &[Expression],
    ga: &GlobalAnalysis,
    cache: &mut ConstPropCache,
) -> Option<Expression> {
    let function = match function {
        Callable::Function(function) => function,
        Callable::Builtin(b) => return try_inline_builtin_function(b, arguments, ga),
        _ => return None,
    };
    if !function.is_constant() {
        return None;
    }
    let mut body = extract_constant_property_reference(function, ga, cache)?;

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

    if simplify_expression(&mut body, ga, cache) { Some(body) } else { None }
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
        BuiltinFunction::GetWindowScaleFactor => {
            ga.const_scale_factor.map(|factor| Expression::NumberLiteral(factor as _, Unit::None))
        }
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
        BuiltinFunction::StringToFloat | BuiltinFunction::StringIsFloat => {
            let Some(Expression::StringLiteral(s)) = args.first() else { return None };
            // Only fold when the string can't contain the decimal separator of any locale,
            // so that parsing gives the same result regardless of the locale.
            if !s.chars().all(|c| c.is_ascii_digit() || matches!(c, '+' | '-' | 'e' | 'E')) {
                return None;
            }
            let value = s.parse::<f32>().ok();
            Some(match b {
                BuiltinFunction::StringToFloat => {
                    Expression::NumberLiteral(value.unwrap_or(0.) as f64, Unit::None)
                }
                _ => Expression::BoolLiteral(value.is_some()),
            })
        }
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
        // We have a code block because the first entry stores the value of `input` in a local variable
        Expression::CodeBlock(stmts) => match &stmts[1] {
            Expression::Condition { condition: _, true_expr: _, false_expr } => match &**false_expr
            {
                Expression::BoolLiteral(b) => assert!(*b),
                _ => panic!("false_expr not optimized in : {out3_binding:?}"),
            },
            _ => panic!("not condition:  {out3_binding:?}"),
        },
        _ => panic!("not code block: {out3_binding:?}"),
    };
}

#[test]
fn test_locale_dependent_string_conversion() {
    let mut compiler_config =
        crate::CompilerConfiguration::new(crate::generator::OutputFormat::Interpreter);
    compiler_config.style = Some("fluent".into());
    let mut test_diags = crate::diagnostics::BuildDiagnostics::default();
    let doc_node = crate::parser::parse(
        r#"
export component Foo {
    out property <string> int-str: "n=" + 42;
    out property <string> calc-str: "n=" + (6 * 7);
    out property <string> float-str: "n=" + 4.5;
    out property <float> int-float: "42".to-float();
    out property <bool> int-is-float: "42".is-float();
    out property <float> frac-float: "4,2".to-float();
}
"#
        .into(),
        Some(std::path::Path::new("HELLO")),
        &mut test_diags,
    );
    let (doc, diag, _) =
        spin_on::spin_on(crate::compile_syntax_node(doc_node, test_diags, compiler_config));
    assert!(!diag.has_errors(), "slint compile error {:#?}", diag.to_string_vec());

    let bindings = &doc.inner_components.last().unwrap().root_element.borrow().bindings;
    let binding = |name: &str| bindings.get(name).unwrap().borrow().clone();
    let is_const =
        |name: &str| bindings.get(name).unwrap().borrow().analysis.as_ref().unwrap().is_const;

    // Conversions whose result contains no decimal separator are folded and stay constant
    assert!(
        matches!(&binding("int-str").expression, Expression::StringLiteral(s) if s == "n=42"),
        "{:?}",
        binding("int-str").expression
    );
    assert!(is_const("int-str"));
    assert!(matches!(&binding("calc-str").expression, Expression::StringLiteral(s) if s == "n=42"));
    assert!(is_const("calc-str"));
    assert!(
        matches!(&binding("int-float").expression, Expression::NumberLiteral(n, _) if *n == 42.)
    );
    assert!(is_const("int-float"));
    assert!(matches!(&binding("int-is-float").expression, Expression::BoolLiteral(true)));
    assert!(is_const("int-is-float"));

    // Locale-dependent conversions are not folded and their bindings are no longer constant,
    // so that they are re-evaluated when the locale changes at runtime
    assert!(
        !matches!(&binding("float-str").expression, Expression::StringLiteral(_)),
        "{:?}",
        binding("float-str").expression
    );
    assert!(!is_const("float-str"));
    assert!(matches!(&binding("frac-float").expression, Expression::FunctionCall { .. }));
    assert!(!is_const("frac-float"));
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
            check_expression: |e| assert_expr_is_mul(e, 5.0, 12.0),
        },
        Case {
            default_font_size: "default-font-size: some-value;",
            another_window: "",
            check_expression: |e| {
                assert!(
                    !e.is_constant(None),
                    "{e:?} should not be constant since some-value can vary at runtime"
                );
            },
        },
        Case {
            default_font_size: "default-font-size: 25px;",
            another_window: "export component AnotherWindow inherits Window { default-font-size: 8px; }",
            check_expression: |e| {
                assert!(
                    e.is_constant(None) && !matches!(e, Expression::NumberLiteral(_, _)),
                    "{e:?} should be constant but not known at compile time since there are two windows"
                );
            },
        },
        Case {
            default_font_size: "default-font-size: 25px;",
            another_window: "export component AnotherWindow inherits Window { }",
            check_expression: |e| {
                assert!(
                    !e.is_constant(None),
                    "should not be const since at least one window has it unset"
                );
            },
        },
        Case {
            default_font_size: "default-font-size: 20px;",
            another_window: "export component AnotherWindow inherits Window { default-font-size: 20px;  }",
            check_expression: |e| assert_expr_is_mul(e, 5.0, 20.0),
        },
        Case {
            default_font_size: "default-font-size: 20px;",
            another_window: "export component AnotherWindow inherits Window { in property <float> f: 1; default-font-size: 20px*f;  }",
            check_expression: |e| {
                assert!(
                    !e.is_constant(None),
                    "{e:?} should not be constant since 'f' can vary at runtime"
                );
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

#[test]
fn test_const_scale_factor() {
    let source = r#"
export component Foo inherits Window {
    out property <length> test: 10phx;
}"#;

    let mut test_diags = crate::diagnostics::BuildDiagnostics::default();
    let doc_node = crate::parser::parse(
        source.to_string(),
        Some(std::path::Path::new("HELLO")),
        &mut test_diags,
    );
    let mut compiler_config =
        crate::CompilerConfiguration::new(crate::generator::OutputFormat::Interpreter);
    compiler_config.style = Some("fluent".into());
    compiler_config.const_scale_factor = Some(2.);
    let (doc, diag, _) =
        spin_on::spin_on(crate::compile_syntax_node(doc_node, test_diags, compiler_config));
    assert!(!diag.has_errors(), "slint compile error {:#?}", diag.to_string_vec());

    let bindings = &doc.inner_components.last().unwrap().root_element.borrow().bindings;
    let mut test_binding = bindings.get("test").unwrap().borrow().expression.clone();
    if let Expression::Cast { from, to: _ } = test_binding {
        test_binding = *from;
    }
    assert!(
        matches!(test_binding, Expression::NumberLiteral(val, _) if val == 5.0),
        "Expression should be 5.0: {test_binding:?}"
    );
}

#[test]
fn test_unit_normalization() {
    // Compile `out property <ty> a: expr;` and return the folded binding of `a`.
    fn fold(ty: &str, expr: &str) -> Expression {
        let mut config =
            crate::CompilerConfiguration::new(crate::generator::OutputFormat::Interpreter);
        config.style = Some("fluent".into());
        let mut diags = crate::diagnostics::BuildDiagnostics::default();
        let doc_node = crate::parser::parse(
            format!("export component Foo {{ out property <{ty}> a: {expr}; }}").into(),
            Some(std::path::Path::new("HELLO")),
            &mut diags,
        );
        let (doc, diag, _) = spin_on::spin_on(crate::compile_syntax_node(doc_node, diags, config));
        assert!(!diag.has_errors(), "{expr}: {:#?}", diag.to_string_vec());
        doc.inner_components.last().unwrap().root_element.borrow().bindings["a"]
            .borrow()
            .expression
            .clone()
    }

    // A literal is stored in its type's canonical unit, not the one it was written in.
    assert!(matches!(fold("length", "1in"), Expression::NumberLiteral(v, Unit::Px) if v == 96.0));
    assert!(
        matches!(fold("duration", "2s"), Expression::NumberLiteral(v, Unit::Ms) if v == 2000.0)
    );

    // Mixed units of one type now fold, because they share a canonical unit.
    assert!(
        matches!(fold("length", "5px + 5cm"), Expression::NumberLiteral(v, Unit::Px) if v == 194.0),
        "{:?}",
        fold("length", "5px + 5cm")
    );
    assert!(
        matches!(fold("duration", "1s - 500ms"), Expression::NumberLiteral(v, Unit::Ms) if v == 500.0)
    );
    assert!(
        matches!(fold("length", "max(12cm, 12px)"), Expression::NumberLiteral(v, Unit::Px) if v == 12.0 * 37.8),
        "{:?}",
        fold("length", "max(12cm, 12px)")
    );

    // Comparisons fold on the normalized values, so different units compare correctly.
    assert!(matches!(fold("bool", "12cm == 12px"), Expression::BoolLiteral(false)));
    assert!(matches!(fold("bool", "1s == 1000ms"), Expression::BoolLiteral(true)));
    assert!(matches!(fold("bool", "12cm < 12px"), Expression::BoolLiteral(false)));
    assert!(matches!(fold("bool", "1turn == 360deg"), Expression::BoolLiteral(true)));

    // Multiplying by a unitless 0 yields 0 in the other factor's unit.
    assert!(
        matches!(fold("length", "10px * 0.0"), Expression::NumberLiteral(v, Unit::Px) if v == 0.0)
    );
    assert!(
        matches!(fold("float", "10 * 0.0"), Expression::NumberLiteral(v, Unit::None) if v == 0.0)
    );

    // Dividing equal units cancels to a unitless ratio.
    assert!(
        matches!(fold("float", "3px / 6px"), Expression::NumberLiteral(v, Unit::None) if v == 0.5)
    );
    // Equality folds for numbers (now via the ordering arm) and bools.
    assert!(matches!(fold("bool", "1px != 2px"), Expression::BoolLiteral(true)));
    assert!(matches!(fold("bool", "true == false"), Expression::BoolLiteral(false)));
}
