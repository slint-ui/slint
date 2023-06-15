// Copyright © SixtyFPS GmbH <info@slint-ui.com>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-1.0 OR LicenseRef-Slint-commercial

//! Inline properties that are simple enough to be inlined
//!
//! If an expression does a single property access or less, it can be inlined
//! in the calling expression

use crate::expression_tree::BuiltinFunction;
use crate::llr::{
    BindingExpression, EvaluationContext, Expression, Property, PropertyReference, PublicComponent,
    SubComponent,
};
use crate::object_tree::PropertyAnalysis;
use std::num::NonZeroUsize;

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
        Expression::ReturnStatement(_) => 1,
        Expression::LayoutCacheAccess { .. } => PROPERTY_ACCESS_COST,
        Expression::BoxLayoutFunction { .. } => return isize::MAX,
        Expression::ComputeDialogLayoutCells { .. } => return isize::MAX,
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
        BuiltinFunction::SetFocusItem => isize::MAX,
        BuiltinFunction::ShowPopupWindow | BuiltinFunction::ClosePopupWindow => isize::MAX,
        BuiltinFunction::ItemMemberFunction(..) => isize::MAX,
        BuiltinFunction::StringToFloat => 50,
        BuiltinFunction::StringIsFloat => 50,
        BuiltinFunction::ColorBrighter => 50,
        BuiltinFunction::ColorDarker => 50,
        BuiltinFunction::ColorTransparentize => 50,
        BuiltinFunction::ColorMix => 50,
        BuiltinFunction::ColorWithAlpha => 50,
        BuiltinFunction::ImageSize => 50,
        BuiltinFunction::ArrayLength => 50,
        BuiltinFunction::Rgb => 50,
        BuiltinFunction::ImplicitLayoutInfo(_) => isize::MAX,
        BuiltinFunction::ItemAbsolutePosition => isize::MAX,
        BuiltinFunction::RegisterCustomFontByPath => isize::MAX,
        BuiltinFunction::RegisterCustomFontByMemory => isize::MAX,
        BuiltinFunction::RegisterBitmapFont => isize::MAX,
        BuiltinFunction::DarkColorScheme => isize::MAX,
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
            property_binding_and_analysis(ctx, prop)
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

#[derive(Default)]
pub(crate) struct PropertyInfoResult<'a> {
    pub analysis: Option<&'a PropertyAnalysis>,
    pub binding: Option<(&'a BindingExpression, ContextMap)>,
    pub property_decl: Option<&'a Property>,
}

pub(crate) fn property_binding_and_analysis<'a>(
    ctx: &'a EvaluationContext,
    prop: &PropertyReference,
) -> PropertyInfoResult<'a> {
    fn match_in_sub_component<'a>(
        sc: &'a SubComponent,
        prop: &PropertyReference,
        map: ContextMap,
    ) -> PropertyInfoResult<'a> {
        let property_decl =
            if let PropertyReference::Local { property_index, sub_component_path } = &prop {
                let mut sc = sc;
                for i in sub_component_path {
                    sc = &sc.sub_components[*i].ty;
                }
                Some(&sc.properties[*property_index])
            } else {
                None
            };
        if let Some(a) = sc.prop_analysis.get(prop) {
            let binding = a.property_init.map(|i| (&sc.property_init[i].1, map));
            return PropertyInfoResult { analysis: Some(&a.analysis), binding, property_decl };
        }
        match prop {
            PropertyReference::Local { sub_component_path, property_index } => {
                if !sub_component_path.is_empty() {
                    let prop2 = PropertyReference::Local {
                        sub_component_path: sub_component_path[1..].to_vec(),
                        property_index: *property_index,
                    };
                    let idx = sub_component_path[0];
                    return match_in_sub_component(
                        &sc.sub_components[idx].ty,
                        &prop2,
                        map.deeper_in_sub_component(idx),
                    );
                }
            }
            PropertyReference::InNativeItem { item_index, sub_component_path, prop_name } => {
                if !sub_component_path.is_empty() {
                    let prop2 = PropertyReference::InNativeItem {
                        sub_component_path: sub_component_path[1..].to_vec(),
                        prop_name: prop_name.clone(),
                        item_index: *item_index,
                    };
                    let idx = sub_component_path[0];
                    return match_in_sub_component(
                        &sc.sub_components[idx].ty,
                        &prop2,
                        map.deeper_in_sub_component(idx),
                    );
                }
            }
            _ => unreachable!(),
        }
        return PropertyInfoResult { property_decl, ..Default::default() };
    }

    match prop {
        PropertyReference::Local { property_index, .. } => {
            if let Some(g) = ctx.current_global {
                return PropertyInfoResult {
                    analysis: Some(&g.prop_analysis[*property_index]),
                    binding: g.init_values[*property_index]
                        .as_ref()
                        .map(|b| (b, ContextMap::Identity)),
                    property_decl: Some(&g.properties[*property_index]),
                };
            } else if let Some(sc) = ctx.current_sub_component.as_ref() {
                return match_in_sub_component(sc, prop, ContextMap::Identity);
            } else {
                unreachable!()
            }
        }
        PropertyReference::InNativeItem { .. } => {
            return match_in_sub_component(
                ctx.current_sub_component.as_ref().unwrap(),
                prop,
                ContextMap::Identity,
            );
        }
        PropertyReference::Global { global_index, property_index } => {
            let g = &ctx.public_component.globals[*global_index];
            return PropertyInfoResult {
                analysis: Some(&g.prop_analysis[*property_index]),
                binding: g
                    .init_values
                    .get(*property_index)
                    .and_then(Option::as_ref)
                    .map(|b| (b, ContextMap::InGlobal(*global_index))),
                property_decl: Some(&g.properties[*property_index]),
            };
        }
        PropertyReference::InParent { level, parent_reference } => {
            let mut ctx = ctx;
            for _ in 0..level.get() {
                ctx = ctx.parent.as_ref().unwrap().ctx;
            }
            let mut ret = property_binding_and_analysis(ctx, parent_reference);
            match &mut ret.binding {
                Some((_, m @ ContextMap::Identity)) => {
                    *m = ContextMap::InSubElement { path: Default::default(), parent: level.get() };
                }
                Some((_, ContextMap::InSubElement { parent, .. })) => {
                    *parent += level.get();
                }
                _ => {}
            }
            ret
        }
        PropertyReference::Function { .. } | PropertyReference::GlobalFunction { .. } => {
            unreachable!()
        }
    }
}

/// Maps between two evaluation context.
/// This allows to go from the current subcomponent's context, to the context
/// relative to the binding we want to inline
#[derive(Debug, Clone)]
pub(crate) enum ContextMap {
    Identity,
    InSubElement { path: Vec<usize>, parent: usize },
    InGlobal(usize),
}

impl ContextMap {
    fn deeper_in_sub_component(self, sub: usize) -> Self {
        match self {
            ContextMap::Identity => ContextMap::InSubElement { parent: 0, path: vec![sub] },
            ContextMap::InSubElement { mut path, parent } => {
                path.push(sub);
                ContextMap::InSubElement { path, parent }
            }
            ContextMap::InGlobal(_) => panic!(),
        }
    }

    pub fn map_property_reference(&self, p: &PropertyReference) -> PropertyReference {
        match self {
            ContextMap::Identity => p.clone(),
            ContextMap::InSubElement { path, parent } => {
                let map_sub_path = |sub_component_path: &[usize]| -> Vec<usize> {
                    path.iter().chain(sub_component_path.iter()).copied().collect()
                };

                let p2 = match p {
                    PropertyReference::Local { sub_component_path, property_index } => {
                        PropertyReference::Local {
                            sub_component_path: map_sub_path(sub_component_path),
                            property_index: *property_index,
                        }
                    }
                    PropertyReference::Function { sub_component_path, function_index } => {
                        PropertyReference::Function {
                            sub_component_path: map_sub_path(sub_component_path),
                            function_index: *function_index,
                        }
                    }
                    PropertyReference::InNativeItem {
                        sub_component_path,
                        item_index,
                        prop_name,
                    } => PropertyReference::InNativeItem {
                        item_index: *item_index,
                        prop_name: prop_name.clone(),
                        sub_component_path: map_sub_path(sub_component_path),
                    },
                    PropertyReference::InParent { level, parent_reference } => {
                        return PropertyReference::InParent {
                            level: (parent + level.get()).try_into().unwrap(),
                            parent_reference: parent_reference.clone(),
                        }
                    }
                    PropertyReference::Global { .. } | PropertyReference::GlobalFunction { .. } => {
                        return p.clone()
                    }
                };
                if let Some(level) = NonZeroUsize::new(*parent) {
                    PropertyReference::InParent { level, parent_reference: p2.into() }
                } else {
                    p2
                }
            }
            ContextMap::InGlobal(global_index) => match p {
                PropertyReference::Local { sub_component_path, property_index } => {
                    assert!(sub_component_path.is_empty());
                    PropertyReference::Global {
                        global_index: *global_index,
                        property_index: *property_index,
                    }
                }
                g @ PropertyReference::Global { .. } => g.clone(),
                _ => unreachable!(),
            },
        }
    }

    fn map_expression(&self, e: &mut Expression) {
        match e {
            Expression::PropertyReference(p)
            | Expression::CallBackCall { callback: p, .. }
            | Expression::PropertyAssignment { property: p, .. }
            | Expression::LayoutCacheAccess { layout_cache_prop: p, .. } => {
                *p = self.map_property_reference(p);
            }
            _ => (),
        }
        e.visit_mut(|e| self.map_expression(e))
    }

    pub fn map_context<'a>(&self, ctx: &EvaluationContext<'a>) -> EvaluationContext<'a> {
        match self {
            ContextMap::Identity => ctx.clone(),
            ContextMap::InSubElement { path, parent } => {
                let mut ctx = ctx;
                for _ in 0..*parent {
                    ctx = ctx.parent.unwrap().ctx;
                }
                if path.is_empty() {
                    ctx.clone()
                } else {
                    let mut e = ctx.current_sub_component.unwrap();
                    for i in path {
                        e = &e.sub_components[*i].ty;
                    }
                    EvaluationContext::new_sub_component(ctx.public_component, e, (), None)
                }
            }
            ContextMap::InGlobal(g) => EvaluationContext::new_global(
                ctx.public_component,
                &ctx.public_component.globals[*g],
                (),
            ),
        }
    }
}
