// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

//! Inline properties that are simple enough to be inlined
//!
//! If an expression does a single property access or less, it can be inlined
//! in the calling expression

use crate::expression_tree::{BuiltinFunction, ImageReference};
use crate::langtype::Type;
use crate::llr::{CompilationUnit, EvaluationContext, Expression};

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
        Expression::ItemMemberFunctionCall { function } => callback_cost(function, ctx),
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
        Expression::WithGridInputData { .. } => return isize::MAX,
        Expression::MinMax { .. } => 10,
        Expression::EmptyComponentFactory => 10,
        Expression::EmptyDataTransfer => 10,
        Expression::TranslationReference { .. } => PROPERTY_ACCESS_COST + 2 * ALLOC_COST,
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
        BuiltinFunction::SetSelectionOffsets => isize::MAX,
        BuiltinFunction::ItemFontMetrics => PROPERTY_ACCESS_COST,
        BuiltinFunction::StringToFloat => 50,
        BuiltinFunction::StringIsFloat => 50,
        BuiltinFunction::StringIsEmpty => 50,
        BuiltinFunction::StringCharacterCount => 50,
        BuiltinFunction::StringStartsWith | BuiltinFunction::StringEndsWith => 50,
        BuiltinFunction::StringToLowercase | BuiltinFunction::StringToUppercase => ALLOC_COST,
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
    // Inline a call to a function that is called exactly once: move its body to
    // the call site. Because it is the only call, the use counts of everything
    // in the body are preserved by the move, so no adjustment is needed.
    if let Expression::FunctionCall { function, .. } = expr {
        let inline_target = ctx.function_info(function).and_then(|(f, map)| {
            if f.use_count.get() != 1 || !body_is_inline_safe(&f.code.borrow()) {
                return None;
            }
            f.use_count.set(0);
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
                if binding.animation.is_none()
                    // State info binding are special and the binding cannot be inlined or used.
                    && !binding.is_state_info
                {
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
}

/// Whether a function body can be moved to its single call site.
///
/// Unsafe when it shows or closes a popup — the popup state lives in the
/// declaring component and is reached by an upward `parent_level`, so a caller
/// in an ancestor component cannot reach it — or reads a parameter more than
/// once: a real call clones each read (`args.N.clone()`), but the inlined body
/// reads a local that a second read would move.
fn body_is_inline_safe(exp: &Expression) -> bool {
    let mut params = std::collections::HashSet::new();
    let mut safe = true;
    exp.visit_recursive(&mut |e| match e {
        Expression::FunctionParameterReference { index } => safe &= params.insert(*index),
        Expression::BuiltinFunctionCall { function, .. } => {
            safe &= !matches!(
                function,
                BuiltinFunction::ShowPopupWindow
                    | BuiltinFunction::ClosePopupWindow
                    | BuiltinFunction::ShowPopupMenu
                    | BuiltinFunction::ShowPopupMenuInternal
            )
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
