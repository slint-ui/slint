// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-1.2 OR LicenseRef-Slint-commercial

//! Inline properties that are simple enough to be inlined
//!
//! If an expression does a single property access or less, it can be inlined
//! in the calling expression

use crate::expression_tree::BuiltinFunction;
use crate::llr::{EvaluationContext, Expression, PropertyInfoResult, PublicComponent};

const PROPERTY_ACCESS_COST: isize = 1000;
const ALLOC_COST: isize = 700;
const ARRAY_INDEX_COST: isize = 500;
const INLINE_THRESHOLD: isize = ALLOC_COST * 2 - 10;

// The cost of an expression.
fn expression_cost(exp: &Expression, ctx: &EvaluationContext) -> isize {
    let mut cost = match exp {
        Expression::StringLiteral(_) => ALLOC_COST,
        Expression::NumberLiteral(_) => 0,
        Expression::BoolLiteral(_) => 0,
        Expression::PropertyReference(_) => PROPERTY_ACCESS_COST,
        Expression::FunctionParameterReference { .. } => return isize::MAX,
        Expression::StoreLocalVariable { .. } => 0,
        Expression::ReadLocalVariable { .. } => 1,
        Expression::StructFieldAccess { .. } => 1,
        Expression::ArrayIndex { .. } => ARRAY_INDEX_COST,
        Expression::Cast { .. } => 0,
        Expression::CodeBlock(_) => 0,
        Expression::BuiltinFunctionCall { function, .. } => builtin_function_cost(function),
        Expression::CallBackCall { callback, .. } => callback_cost(callback, ctx),
        Expression::FunctionCall { function, .. } => callback_cost(function, ctx),
        Expression::ExtraBuiltinFunctionCall { .. } => return isize::MAX,
        Expression::PropertyAssignment { .. } => return isize::MAX,
        Expression::ModelDataAssignment { .. } => return isize::MAX,
        Expression::ArrayIndexAssignment { .. } => return isize::MAX,
        Expression::BinaryExpression { .. } => 1,
        Expression::UnaryOp { .. } => 1,
        Expression::ImageReference { .. } => 1,
        Expression::Condition { .. } => 10,
        Expression::Array { .. } => ALLOC_COST,
        Expression::Struct { .. } => 1,
        Expression::EasingCurve(_) => 1,
        Expression::LinearGradient { .. } => ALLOC_COST,
        Expression::RadialGradient { .. } => ALLOC_COST,
        Expression::EnumerationValue(_) => 0,
        Expression::LayoutCacheAccess { .. } => PROPERTY_ACCESS_COST,
        Expression::BoxLayoutFunction { .. } => return isize::MAX,
        Expression::ComputeDialogLayoutCells { .. } => return isize::MAX,
        Expression::MinMax { .. } => 10,
    };

    exp.visit(|e| cost = cost.saturating_add(expression_cost(e, ctx)));

    cost
}

fn callback_cost(_callback: &crate::llr::PropertyReference, _ctx: &EvaluationContext) -> isize {
    // TODO: lookup the callback and find out what it does
    isize::MAX
}

fn builtin_function_cost(function: &BuiltinFunction) -> isize {
    match function {
        BuiltinFunction::GetWindowScaleFactor => PROPERTY_ACCESS_COST,
        BuiltinFunction::GetWindowDefaultFontSize => PROPERTY_ACCESS_COST,
        BuiltinFunction::AnimationTick => PROPERTY_ACCESS_COST,
        BuiltinFunction::Debug => isize::MAX,
        BuiltinFunction::Mod => 10,
        BuiltinFunction::Round => 10,
        BuiltinFunction::Ceil => 10,
        BuiltinFunction::Floor => 10,
        BuiltinFunction::Abs => 10,
        BuiltinFunction::Sqrt => 10,
        BuiltinFunction::Cos => 10,
        BuiltinFunction::Sin => 10,
        BuiltinFunction::Tan => 10,
        BuiltinFunction::ACos => 10,
        BuiltinFunction::ASin => 10,
        BuiltinFunction::ATan => 10,
        BuiltinFunction::Log => 10,
        BuiltinFunction::Pow => 10,
        BuiltinFunction::SetFocusItem | BuiltinFunction::ClearFocusItem => isize::MAX,
        BuiltinFunction::ShowPopupWindow | BuiltinFunction::ClosePopupWindow => isize::MAX,
        BuiltinFunction::SetSelectionOffsets => isize::MAX,
        BuiltinFunction::ItemMemberFunction(..) => isize::MAX,
        BuiltinFunction::StringToFloat => 50,
        BuiltinFunction::StringIsFloat => 50,
        BuiltinFunction::ColorRgbaStruct => 50,
        BuiltinFunction::ColorHsvaStruct => 50,
        BuiltinFunction::ColorBrighter => 50,
        BuiltinFunction::ColorDarker => 50,
        BuiltinFunction::ColorTransparentize => 50,
        BuiltinFunction::ColorMix => 50,
        BuiltinFunction::ColorWithAlpha => 50,
        BuiltinFunction::ImageSize => 50,
        BuiltinFunction::ArrayLength => 50,
        BuiltinFunction::Rgb => 50,
        BuiltinFunction::Hsv => 50,
        BuiltinFunction::ImplicitLayoutInfo(_) => isize::MAX,
        BuiltinFunction::ItemAbsolutePosition => isize::MAX,
        BuiltinFunction::RegisterCustomFontByPath => isize::MAX,
        BuiltinFunction::RegisterCustomFontByMemory => isize::MAX,
        BuiltinFunction::RegisterBitmapFont => isize::MAX,
        BuiltinFunction::ColorScheme => isize::MAX,
        BuiltinFunction::SetTextInputFocused => PROPERTY_ACCESS_COST,
        BuiltinFunction::TextInputFocused => PROPERTY_ACCESS_COST,
        BuiltinFunction::Translate => 2 * ALLOC_COST + PROPERTY_ACCESS_COST,
    }
}

pub fn inline_simple_expressions(root: &PublicComponent) {
    root.for_each_expression(&mut |e, ctx| {
        inline_simple_expressions_in_expression(&mut e.borrow_mut(), ctx)
    })
}

fn inline_simple_expressions_in_expression(expr: &mut Expression, ctx: &EvaluationContext) {
    if let Expression::PropertyReference(prop) = expr {
        if let PropertyInfoResult { analysis: Some(a), binding: Some((binding, map)), .. } =
            ctx.property_info(prop)
        {
            if !a.is_set
                && !a.is_set_externally
                // State info binding are special and the binding cannot be inlined or used.
                && !binding.is_state_info
                && binding.animation.is_none()
                && expression_cost(&binding.expression.borrow(), &map.map_context(ctx)) < INLINE_THRESHOLD
            {
                // Perform inlining
                *expr = binding.expression.borrow().clone();
                map.map_expression(expr);
            }
        }
    };

    expr.visit_mut(|e| inline_simple_expressions_in_expression(e, ctx));
}
