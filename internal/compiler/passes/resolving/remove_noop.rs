// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

use crate::{diagnostics::BuildDiagnostics, expression_tree::Expression, parser::SyntaxNode};

/// Remove all expressions that are proven to have no effect from the given Expressions.
///
/// This function assumes that the given Expressions will form the an [Expression::CodeBlock], and
/// will therefore not modify the last Expression in the Vec, as that forms the result value of the
/// CodeBlock itself.
pub fn remove_from_codeblock(
    code_block: &mut Vec<(SyntaxNode, Expression)>,
    diagnostics: &mut BuildDiagnostics,
) {
    if code_block.len() > 1 {
        // In a code block, only the last expression returns a value.
        // Therefore all other expressions inside the block are only useful if they have side
        // effects.
        //
        // Remove all expressions without side effects (except for the last one) and emit a
        // warning.
        //
        // Note: Iterate over the indices in reverse, so that all to-be-iterated indices remain
        // valid when removing items from the vector.
        for index in (0..(code_block.len() - 1)).rev() {
            let (node, expression) = &code_block[index];
            if without_side_effects(expression) {
                diagnostics.push_warning("Expression has no effect!".to_owned(), node);
                code_block.remove(index);
            }
        }
    }
}

/// Returns whether the expression is certain to be without side effects.
/// This function is conservative and may still return `false`, even if a given expression
/// is without side effects.
/// It is only guaranteed that if this function returns `true`, the expression definitely does not
/// contain side effects.
fn without_side_effects(expression: &Expression) -> bool {
    match expression {
        Expression::Condition { condition, true_expr, false_expr } => {
            without_side_effects(condition)
                && without_side_effects(true_expr)
                && without_side_effects(false_expr)
        }
        Expression::NumberLiteral(_, _) => true,
        Expression::StringLiteral(_) => true,
        Expression::BoolLiteral(_) => true,
        Expression::CodeBlock(expressions) => expressions.iter().all(without_side_effects),
        Expression::FunctionParameterReference { .. } => true,
        // Invalid and uncompiled expressions are unknown at this point, so default to
        // `false`, because they may have side-efffects.
        Expression::Invalid => false,
        Expression::Uncompiled(_) => false,
        // A property reference may cause re-evaluation of a property, which may result in
        // side effects
        Expression::PropertyReference(_) => false,
        Expression::ElementReference(_) => false,
        Expression::RepeaterIndexReference { .. } => true,
        Expression::RepeaterModelReference { .. } => true,
        Expression::StoreLocalVariable { .. } => false,
        Expression::ReadLocalVariable { .. } => true,
        Expression::StructFieldAccess { base, name: _ } => without_side_effects(&*base),
        Expression::ArrayIndex { array, index } => {
            without_side_effects(&*array) && without_side_effects(&*index)
        }
        // Note: This assumes that the cast itself does not have any side effects, which may not be
        // the case if custom casting rules are implemented.
        Expression::Cast { from, to: _ } => without_side_effects(from),
        // Note: Calling a *pure* function is without side effects, however
        // just from the expression, the purity of the function is not known.
        // We would need to resolve the function to determine its purity.
        Expression::FunctionCall { .. } => false,
        Expression::SelfAssignment { .. } => false,
        Expression::BinaryExpression { lhs, rhs, .. } => {
            without_side_effects(&*lhs) && without_side_effects(&*rhs)
        }
        Expression::UnaryOp { sub, op: _ } => without_side_effects(&*sub),
        Expression::ImageReference { .. } => true,
        Expression::Array { element_ty: _, values } => values.iter().all(without_side_effects),
        Expression::Struct { ty: _, values } => values.values().all(without_side_effects),
        Expression::PathData(_) => true,
        Expression::EasingCurve(_) => true,
        Expression::LinearGradient { angle, stops } => {
            without_side_effects(&angle)
                && stops
                    .iter()
                    .all(|(start, end)| without_side_effects(start) && without_side_effects(end))
        }
        Expression::RadialGradient { stops } => stops
            .iter()
            .all(|(start, end)| without_side_effects(start) && without_side_effects(end)),
        Expression::ConicGradient { stops } => stops
            .iter()
            .all(|(start, end)| without_side_effects(start) && without_side_effects(end)),
        Expression::EnumerationValue(_) => true,
        // A return statement is never without side effects, as an important "side effect" is that
        // the current function stops at this point.
        Expression::ReturnStatement(_) => false,
        Expression::LayoutCacheAccess { .. } => false,
        Expression::ComputeLayoutInfo(_, _) => false,
        Expression::SolveLayout(_, _) => false,
        Expression::MinMax { ty: _, op: _, lhs, rhs } => {
            without_side_effects(lhs) && without_side_effects(rhs)
        }
        Expression::DebugHook { .. } => false,
        Expression::EmptyComponentFactory => false,
    }
}
