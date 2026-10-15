// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

//! Inline properties that are simple enough to be inlined
//!
//! If an expression does a single property access or less, it can be inlined
//! in the calling expression

use crate::builtin_functions::BuiltinFunction;
use crate::expression_types::ImageReference;
use crate::langtype::Type;
use crate::llr::{CompilationUnit, ContextMap, EvaluationContext, Expression};

const PROPERTY_ACCESS_COST: isize = 1000;
const ALLOC_COST: isize = 700;
const ARRAY_INDEX_COST: isize = 500;
/// The threshold from which we consider an expression to be worth inlining.
/// less than two allocations. (since property access usually cost one allocation)
const INLINE_THRESHOLD: isize = ALLOC_COST * 2 - 10;
/// Property that are used only once should almost always be inlined unless it is really expensive to compute and we want to cache the result
const INLINE_SINGLE_THRESHOLD: isize = ALLOC_COST * 10;

// The cost of an expression.
fn expression_cost(exp: &Expression, ctx: &EvaluationContext) -> isize {
    let mut cost = match exp {
        Expression::StringLiteral(_) => ALLOC_COST,
        Expression::NumberLiteral(_) => 0,
        Expression::BoolLiteral(_) => 0,
        Expression::KeysLiteral(_) => 0,
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
        Expression::SliceIndexAssignment { .. } => return isize::MAX,
        Expression::BinaryExpression { .. } => 1,
        Expression::UnaryOp { .. } => 1,
        // Avoid inlining calls to load the image from the cache, as in the worst case the image isn't cached
        // and repeated calls will load the image over and over again. It's better to keep the image cached in the
        // `property<image>` of the `Image` element, with the exception of embedded textures.
        Expression::ImageReference {
            resource_ref: ImageReference::EmbeddedTexture { .. }, ..
        } => 1,
        Expression::ImageReference { .. } => return isize::MAX,
        Expression::Condition { condition, true_expr, false_expr } => {
            return expression_cost(condition, ctx)
                .saturating_add(
                    expression_cost(true_expr, ctx).max(expression_cost(false_expr, ctx)),
                )
                .saturating_add(10);
        }
        // Never inline an array because it is a model and when shared it needs to keep its identity
        // (cf #5249)  (otherwise it would be `ALLOC_COST`)
        Expression::Array { .. } => return isize::MAX,
        Expression::Struct { .. } => 1,
        Expression::EasingCurve(_) => 1,
        Expression::MouseCursor(_) => 1,
        Expression::LinearGradient { .. } => ALLOC_COST,
        Expression::RadialGradient { .. } => ALLOC_COST,
        Expression::ConicGradient { .. } => ALLOC_COST,
        Expression::EnumerationValue(_) => 0,
        Expression::LayoutCacheAccess { .. } => PROPERTY_ACCESS_COST,
        Expression::GridRepeaterCacheAccess { .. } => PROPERTY_ACCESS_COST,
        Expression::WithLayoutItemInfo { .. } => return isize::MAX,
        Expression::WithFlexboxLayoutItemInfo { .. } => return isize::MAX,
        Expression::SolveFlexboxLayoutWithMeasure { .. } => return isize::MAX,
        Expression::BoxLayoutInfoOrthoWithMeasure { .. } => return isize::MAX,
        Expression::FlexboxLayoutInfoCrossAxisWithMeasure { .. } => return isize::MAX,
        Expression::WithGridInputData { .. } => return isize::MAX,
        Expression::MinMax { .. } => 10,
        Expression::EmptyComponentFactory => 10,
        Expression::EmptyDataTransfer => 10,
        Expression::TranslationReference { .. } => PROPERTY_ACCESS_COST + 2 * ALLOC_COST,
        // The body cost is added by the visit() walk below; returning the body
        // cost here would double-count it.
        Expression::Closure { .. } => 0,
        Expression::DashArray(_) => ALLOC_COST,
        // Don't inline: that could duplicate or relocate the hook.
        Expression::DebugHook { .. } => return isize::MAX,
        Expression::CornerShape(_) => 1,
    };

    exp.visit(|e| cost = cost.saturating_add(expression_cost(e, ctx)));

    cost
}

fn callback_cost(_callback: &crate::llr::MemberReference, _ctx: &EvaluationContext) -> isize {
    // TODO: lookup the callback and find out what it does
    isize::MAX
}

fn builtin_function_cost(function: &BuiltinFunction) -> isize {
    match function {
        BuiltinFunction::GetWindowScaleFactor => PROPERTY_ACCESS_COST,
        BuiltinFunction::GetWindowDefaultFontSize => PROPERTY_ACCESS_COST,
        BuiltinFunction::AnimationTick => PROPERTY_ACCESS_COST,
        BuiltinFunction::DecimalSeparator => PROPERTY_ACCESS_COST,
        BuiltinFunction::DefaultWindowTitle => PROPERTY_ACCESS_COST,
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
        BuiltinFunction::ATan2 => 10,
        BuiltinFunction::Log => 10,
        BuiltinFunction::Ln => 10,
        BuiltinFunction::Pow => 10,
        BuiltinFunction::Exp => 10,
        BuiltinFunction::ToFixed => ALLOC_COST,
        BuiltinFunction::ToPrecision => ALLOC_COST,
        BuiltinFunction::ToStringUnlocalized => ALLOC_COST,
        BuiltinFunction::SetFocusItem | BuiltinFunction::ClearFocusItem => isize::MAX,
        BuiltinFunction::ShowPopupWindow
        | BuiltinFunction::ClosePopupWindow
        | BuiltinFunction::ShowPopupMenu
        | BuiltinFunction::ShowPopupMenuInternal => isize::MAX,
        BuiltinFunction::SetSelectionOffsets
        | BuiltinFunction::TextInputSelectAll
        | BuiltinFunction::TextInputClearSelection
        | BuiltinFunction::TextInputCut
        | BuiltinFunction::TextInputCopy
        | BuiltinFunction::TextInputPaste
        | BuiltinFunction::TextInputUndo
        | BuiltinFunction::TextInputRedo
        | BuiltinFunction::SwipeGestureHandlerCancel
        | BuiltinFunction::ContextMenuClose
        | BuiltinFunction::WindowClose
        | BuiltinFunction::WindowHide => isize::MAX,
        BuiltinFunction::HasSelection | BuiltinFunction::ContextMenuIsOpen => PROPERTY_ACCESS_COST,
        BuiltinFunction::ItemFontMetrics => PROPERTY_ACCESS_COST,
        BuiltinFunction::StringToFloat => 50,
        BuiltinFunction::StringIsFloat => 50,
        BuiltinFunction::StringIsEmpty => 50,
        BuiltinFunction::StringCharacterCount => 50,
        BuiltinFunction::StringStartsWith | BuiltinFunction::StringEndsWith => 50,
        BuiltinFunction::StringToLowercase | BuiltinFunction::StringToUppercase => ALLOC_COST,
        BuiltinFunction::StringReplaceAll => ALLOC_COST,
        BuiltinFunction::KeysToString => ALLOC_COST,
        BuiltinFunction::ColorRgbaStruct => 50,
        BuiltinFunction::ColorHsvaStruct => 50,
        BuiltinFunction::ColorOklchStruct => 50,
        BuiltinFunction::ColorBrighter => 50,
        BuiltinFunction::ColorDarker => 50,
        BuiltinFunction::ColorTransparentize => 50,
        BuiltinFunction::ColorMix => 50,
        BuiltinFunction::ColorWithAlpha => 50,
        BuiltinFunction::ImageSize => 50,
        BuiltinFunction::ArrayLength => 50,
        BuiltinFunction::ArrayPush
        | BuiltinFunction::ArrayRemove
        | BuiltinFunction::ArrayInsert => ALLOC_COST,
        BuiltinFunction::Rgb => 50,
        BuiltinFunction::Hsv => 50,
        BuiltinFunction::Oklch => 50,
        BuiltinFunction::CornerShapeSuperellipse => 50,
        BuiltinFunction::ImplicitLayoutInfo(_) => isize::MAX,
        BuiltinFunction::ItemAbsolutePosition => isize::MAX,
        BuiltinFunction::RegisterCustomFontByPath => isize::MAX,
        BuiltinFunction::RegisterCustomFontByMemory => isize::MAX,
        BuiltinFunction::RegisterBitmapFont => isize::MAX,
        BuiltinFunction::ColorScheme => PROPERTY_ACCESS_COST,
        BuiltinFunction::AccentColor => PROPERTY_ACCESS_COST,
        BuiltinFunction::SupportsNativeMenuBar => 10,
        BuiltinFunction::SetupMenuBar => isize::MAX,
        BuiltinFunction::SetupSystemTrayIcon => isize::MAX,
        BuiltinFunction::MonthDayCount => isize::MAX,
        BuiltinFunction::MonthOffset => isize::MAX,
        BuiltinFunction::FormatDate => isize::MAX,
        BuiltinFunction::DateNow => isize::MAX,
        BuiltinFunction::ValidDate => isize::MAX,
        BuiltinFunction::ParseDate => isize::MAX,
        BuiltinFunction::SetTextInputFocused => PROPERTY_ACCESS_COST,
        BuiltinFunction::TextInputFocused => PROPERTY_ACCESS_COST,
        BuiltinFunction::Translate => 2 * ALLOC_COST + PROPERTY_ACCESS_COST,
        BuiltinFunction::Use24HourFormat => 2 * ALLOC_COST + PROPERTY_ACCESS_COST,
        BuiltinFunction::UpdateTimers => 10,
        BuiltinFunction::DetectOperatingSystem => 10,
        BuiltinFunction::StartTimer => 10,
        BuiltinFunction::StopTimer => 10,
        BuiltinFunction::RestartTimer => 10,
        BuiltinFunction::ParseMarkdown => isize::MAX,
        BuiltinFunction::StringToStyledText => ALLOC_COST,
        BuiltinFunction::ColorToStyledText => ALLOC_COST,
        BuiltinFunction::OpenUrl => isize::MAX,
        BuiltinFunction::MacosBringAllWindowsToFront => isize::MAX,
        BuiltinFunction::PathPointAt => isize::MAX,
        BuiltinFunction::PathAngleAt => isize::MAX,
        // Iterating the model and running the closure is unbounded; never inline.
        BuiltinFunction::ArrayAny | BuiltinFunction::ArrayAll | BuiltinFunction::ArrayFindIndex => {
            isize::MAX
        }
        BuiltinFunction::ScrollTo => isize::MAX,
        BuiltinFunction::EnsureVisible => isize::MAX,
    }
}

pub fn inline_simple_expressions(root: &CompilationUnit) {
    // Counter to give each inlined function's argument locals a unique name.
    let mut counter = 0usize;
    root.for_each_expression(&mut |e, ctx| {
        inline_simple_expressions_in_expression(&mut e.borrow_mut(), ctx, &mut counter)
    })
}

fn inline_simple_expressions_in_expression(
    expr: &mut Expression,
    ctx: &EvaluationContext,
    counter: &mut usize,
) {
    // Inline a call to a function that is called exactly once: move its body to the call site.
    if let Expression::FunctionCall { function, .. } = expr {
        let inline_target = ctx.function_info(function).and_then(|(f, map)| {
            if f.use_count.get() != 1 || !body_is_inline_safe(&f.code.borrow(), &map) {
                return None;
            }
            f.use_count.set(0);
            // count_property_use counts the body in the function's context; re-home the use
            // counts to the call site, where a reference can resolve to a parent-set binding
            // (e.g. `height: 100%`) that is invisible in the function and so under-counted.
            adjust_use_count(&f.code.borrow(), &map.map_context(ctx), -1);
            // Take the body out so it isn't also inlined in place when
            // `for_each_expression` reaches it, which would double-count uses.
            let body = f.code.replace(Expression::CodeBlock(Vec::new()));
            Some((body, f.args.clone(), map))
        });
        if let Some((mut body, arg_types, map)) = inline_target {
            let Expression::FunctionCall { arguments, .. } =
                std::mem::replace(expr, Expression::CodeBlock(Vec::new()))
            else {
                unreachable!()
            };
            let uid = *counter;
            *counter += 1;
            map.map_expression(&mut body);
            // Re-count in the call-site context (see above).
            adjust_use_count(&body, ctx, 1);
            substitute_function_parameters(&mut body, uid, &arg_types);
            *expr = if arguments.is_empty() {
                body
            } else {
                let mut stmts = arguments
                    .into_iter()
                    .enumerate()
                    .map(|(i, a)| Expression::StoreLocalVariable {
                        name: function_arg_local_name(uid, i),
                        value: Box::new(a),
                    })
                    .collect::<Vec<_>>();
                stmts.push(body);
                Expression::CodeBlock(stmts)
            };
            // Inline further within the freshly inlined body (nested single calls,
            // constant properties now visible at the call site, ...).
            inline_simple_expressions_in_expression(expr, ctx, counter);
            return;
        }
    }

    if let Expression::PropertyReference(prop) = expr {
        let prop_info = ctx.property_info(prop);
        if prop_info.analysis.as_ref().is_some_and(|a| !a.is_set && !a.is_set_externally) {
            if let Some((binding, map)) = prop_info.binding {
                if binding.animation.is_none() && binding.kind != super::super::BindingKind::State {
                    let mapped_ctx = map.map_context(ctx);
                    let cost = expression_cost(&binding.expression.borrow(), &mapped_ctx);
                    let use_count = binding.use_count.get();
                    debug_assert!(
                        use_count > 0,
                        "We use a property and its count is zero: {}",
                        crate::llr::pretty_print::DisplayPropertyRef(prop, ctx)
                    );
                    if cost <= INLINE_THRESHOLD
                        || (use_count == 1 && cost <= INLINE_SINGLE_THRESHOLD)
                    {
                        // Perform inlining
                        *expr = binding.expression.borrow().clone();
                        map.map_expression(expr);
                        // adjust use count
                        binding.use_count.set(use_count - 1);
                        if let Some(use_count) = prop_info.use_count {
                            use_count.set(use_count.get() - 1);
                        }
                        adjust_use_count(expr, ctx, 1);
                        if use_count == 1 {
                            adjust_use_count(&binding.expression.borrow(), &mapped_ctx, -1);
                            binding.expression.replace(Expression::CodeBlock(Vec::new()));
                        }
                    }
                }
            } else if let Some(use_count) = prop_info.use_count
                && let Some(e) = Expression::default_value_for_type(&prop_info.ty)
            {
                use_count.set(use_count.get() - 1);
                *expr = e;
            }
        }
    };

    expr.visit_mut(|e| inline_simple_expressions_in_expression(e, ctx, counter));

    fold_constant_expression(expr, ctx);
}

/// Fold a comparison of two literals to a `BoolLiteral`, and a `Condition` with
/// a literal condition to the branch it takes.
///
/// Inlining a property replaces a read with its constant value, which can leave
/// a comparison with no runtime input behind: in
/// `orientation == Orientation.vertical ? a : b` the read of `orientation`
/// becomes an `EnumerationValue`, so the comparison is now between two literals.
/// The generators emit it verbatim, and the C++ backend then contains `a == a`,
/// which `-Wtautological-compare` (part of `-Wall`, and the test driver builds
/// with `-Werror`) rejects. `const_propagation` folds the same shapes on the
/// element tree, but it runs before this pass and doesn't descend into layout
/// expressions.
fn fold_constant_expression(expr: &mut Expression, ctx: &EvaluationContext) {
    if let Some(folded) = fold_constant_comparison(expr) {
        *expr = folded;
    } else {
        fold_constant_condition(expr, ctx);
    }
}

/// Fold a comparison of two literals to a `BoolLiteral`.
fn fold_constant_comparison(expr: &Expression) -> Option<Expression> {
    let Expression::BinaryExpression { lhs, op, rhs } = expr else { return None };
    let value = match (&**lhs, *op, &**rhs) {
        (Expression::BoolLiteral(a), '=', Expression::BoolLiteral(b)) => a == b,
        (Expression::BoolLiteral(a), '!', Expression::BoolLiteral(b)) => a != b,
        (Expression::EnumerationValue(a), '=', Expression::EnumerationValue(b)) => a == b,
        (Expression::EnumerationValue(a), '!', Expression::EnumerationValue(b)) => a != b,
        (Expression::NumberLiteral(a), '=', Expression::NumberLiteral(b)) => a == b,
        (Expression::NumberLiteral(a), '!', Expression::NumberLiteral(b)) => a != b,
        (Expression::NumberLiteral(a), '<', Expression::NumberLiteral(b)) => a < b,
        (Expression::NumberLiteral(a), '>', Expression::NumberLiteral(b)) => a > b,
        (Expression::NumberLiteral(a), '≤', Expression::NumberLiteral(b)) => a <= b,
        (Expression::NumberLiteral(a), '≥', Expression::NumberLiteral(b)) => a >= b,
        _ => return None,
    };
    Some(Expression::BoolLiteral(value))
}

/// Fold a `Condition` whose condition is a literal to the branch it takes.
///
/// Only folds when both branches share a type: the LLR `Condition`'s type is the
/// false branch's, so folding one whose branches differ would change the type
/// (lowering inserted a cast to `void` in that case).
fn fold_constant_condition(expr: &mut Expression, ctx: &EvaluationContext) {
    let replacement = match expr {
        Expression::Condition { condition, true_expr, false_expr } => {
            let take_true = match &**condition {
                Expression::BoolLiteral(b) => *b,
                _ => return,
            };
            if true_expr.ty(ctx) != false_expr.ty(ctx) {
                return;
            }
            let branch = if take_true { &mut **true_expr } else { &mut **false_expr };
            std::mem::replace(branch, Expression::CodeBlock(Vec::new()))
        }
        _ => return,
    };
    *expr = replacement;
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::langtype::{Enumeration, EnumerationValue};
    use smol_str::SmolStr;
    use std::sync::Arc;

    /// The `Orientation` enum, as the LLR sees it. Comparisons of two
    /// `EnumerationValue`s compare the enum by `Arc` identity, so a single
    /// instance has to be shared.
    fn orientation_enum() -> Arc<Enumeration> {
        Arc::new(Enumeration {
            name: SmolStr::new_static("Orientation"),
            values: vec![SmolStr::new_static("horizontal"), SmolStr::new_static("vertical")],
            default_value: 0,
            node: None,
            public: true,
            rust_attributes: Vec::new(),
        })
    }

    fn orientation(enumeration: &Arc<Enumeration>, value: usize) -> Expression {
        Expression::EnumerationValue(EnumerationValue { value, enumeration: enumeration.clone() })
    }

    fn compare(lhs: Expression, op: char, rhs: Expression) -> Expression {
        Expression::BinaryExpression { lhs: Box::new(lhs), rhs: Box::new(rhs), op }
    }

    #[test]
    fn folds_constant_comparisons() {
        let orientation_ty = orientation_enum();
        let is = |e: &Expression, expected: bool| {
            assert!(
                matches!(fold_constant_comparison(e), Some(Expression::BoolLiteral(b)) if b == expected),
                "{e:?} did not fold to {expected}"
            );
        };
        is(&compare(orientation(&orientation_ty, 0), '=', orientation(&orientation_ty, 0)), true);
        is(&compare(orientation(&orientation_ty, 0), '=', orientation(&orientation_ty, 1)), false);
        is(&compare(orientation(&orientation_ty, 0), '!', orientation(&orientation_ty, 1)), true);
        is(&compare(Expression::BoolLiteral(false), '=', Expression::BoolLiteral(false)), true);
        is(&compare(Expression::NumberLiteral(1.), '<', Expression::NumberLiteral(2.)), true);

        // A comparison that still reads a local at runtime stays as it is.
        let dynamic = compare(
            Expression::ReadLocalVariable {
                name: SmolStr::new_static("x"),
                ty: Type::Int32.into(),
            },
            '=',
            orientation(&orientation_ty, 1),
        );
        assert!(fold_constant_comparison(&dynamic).is_none(), "{dynamic:?}");
    }
}

/// Whether a function body can be moved to its single call site through `map`.
///
/// Unsafe when it references state that only exists in the declaring component
/// and cannot be remapped by `ContextMap::map_expression`:
/// the menu item tree of a popup menu, and `UpdateTimers`,
/// refer to the enclosing component implicitly,
/// so they can only move within the same component.
/// Also unsafe when it reads a parameter more than once:
/// a real call clones each read (`args.N.clone()`),
/// but the inlined body reads a local that a second read would move.
fn body_is_inline_safe(exp: &Expression, map: &ContextMap) -> bool {
    let mut params = std::collections::HashSet::new();
    let mut safe = true;
    exp.visit_recursive(&mut |e| match e {
        Expression::FunctionParameterReference { index } => safe &= params.insert(*index),
        Expression::BuiltinFunctionCall { function, .. } => {
            safe &= match function {
                BuiltinFunction::ShowPopupMenu
                | BuiltinFunction::ShowPopupMenuInternal
                | BuiltinFunction::UpdateTimers => matches!(map, ContextMap::Identity),
                _ => true,
            }
        }
        _ => {}
    });
    safe
}

/// The local variable name holding argument `index` of the function inlined with `uid`.
fn function_arg_local_name(uid: usize, index: usize) -> smol_str::SmolStr {
    smol_str::format_smolstr!("inlined_fn_arg_{uid}_{index}")
}

/// Replace `FunctionParameterReference` with a read of the local that holds the argument.
fn substitute_function_parameters(expr: &mut Expression, uid: usize, arg_types: &[Type]) {
    expr.visit_recursive_mut(&mut |e| {
        if let Expression::FunctionParameterReference { index } = e {
            let index = *index;
            *e = Expression::ReadLocalVariable {
                name: function_arg_local_name(uid, index),
                ty: arg_types[index].clone(),
            };
        }
    });
}

fn adjust_use_count(expr: &Expression, ctx: &EvaluationContext, adjust: isize) {
    expr.visit_property_references(ctx, &mut |p, ctx| {
        let prop_info = ctx.property_info(p);
        if let Some(use_count) = prop_info.use_count {
            use_count.set(use_count.get().checked_add_signed(adjust).unwrap());
        }
        if let Some((binding, _)) = prop_info.binding {
            let use_count = binding.use_count.get().checked_add_signed(adjust).unwrap();
            binding.use_count.set(use_count);
        }
    });
}
