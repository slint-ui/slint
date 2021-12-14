// Copyright © SixtyFPS GmbH <info@sixtyfps.io>
// SPDX-License-Identifier: (GPL-3.0-only OR LicenseRef-SixtyFPS-commercial)

use std::cell::RefCell;
use std::num::NonZeroUsize;
use std::rc::{Rc, Weak};

use super::lower_to_item_tree::{LoweredSubComponentMapping, LoweringState};
use super::{Animation, PropertyReference};
use crate::expression_tree::Expression as tree_Expression;
use crate::langtype::Type;
use crate::llr::Expression as llr_Expression;
use crate::namedreference::NamedReference;
use crate::object_tree::{Element, ElementRc, PropertyAnimation};

pub struct ExpressionContext<'a> {
    pub component: &'a Rc<crate::object_tree::Component>,
    pub mapping: &'a LoweredSubComponentMapping,
    pub state: &'a LoweringState,
    pub parent: Option<&'a ExpressionContext<'a>>,
}

impl ExpressionContext<'_> {
    pub fn map_property_reference(
        &self,
        from: &NamedReference,
        state: &LoweringState,
    ) -> Option<PropertyReference> {
        let element = from.element();
        let enclosing = &element.borrow().enclosing_component.upgrade().unwrap();
        if !enclosing.is_global() {
            let mut map = self;
            let mut level = 0;
            while !Rc::ptr_eq(&enclosing, map.component) {
                map = map.parent.unwrap();
                level += 1;
            }
            if let Some(level) = NonZeroUsize::new(level) {
                PropertyReference::InParent {
                    level,
                    parent_reference: Box::new(map.mapping.map_property_reference(from, state)?),
                };
            }
        }
        self.mapping.map_property_reference(from, state)
    }
}

pub fn lower_expression(
    expression: &tree_Expression,
    ctx: &ExpressionContext<'_>,
) -> Option<llr_Expression> {
    match expression {
        tree_Expression::Invalid => None,
        tree_Expression::Uncompiled(_) => None,
        tree_Expression::StringLiteral(s) => Some(llr_Expression::StringLiteral(s.clone())),
        tree_Expression::NumberLiteral(n, _) => Some(llr_Expression::NumberLiteral(*n)),
        tree_Expression::BoolLiteral(b) => Some(llr_Expression::BoolLiteral(*b)),
        tree_Expression::CallbackReference(nr) => Some(llr_Expression::PropertyReference(
            ctx.mapping.map_property_reference(nr, ctx.state)?,
        )),
        tree_Expression::PropertyReference(nr) => Some(llr_Expression::PropertyReference(
            ctx.mapping.map_property_reference(nr, ctx.state)?,
        )),
        tree_Expression::BuiltinFunctionReference(_, _) => todo!(),
        tree_Expression::MemberFunction { .. } => None,
        tree_Expression::BuiltinMacroReference(_, _) => None,
        tree_Expression::ElementReference(_) => todo!(),
        tree_Expression::RepeaterIndexReference { element } => {
            repeater_special_property(element, ctx.component, 1)
        }
        tree_Expression::RepeaterModelReference { element } => {
            repeater_special_property(element, ctx.component, 0)
        }
        tree_Expression::FunctionParameterReference { index, .. } => {
            Some(llr_Expression::FunctionParameterReference { index: *index })
        }
        tree_Expression::StoreLocalVariable { name, value } => {
            Some(llr_Expression::StoreLocalVariable {
                name: name.clone(),
                value: Box::new(lower_expression(value, ctx)?),
            })
        }
        tree_Expression::ReadLocalVariable { name, ty } => {
            Some(llr_Expression::ReadLocalVariable { name: name.clone(), ty: ty.clone() })
        }
        tree_Expression::StructFieldAccess { base, name } => {
            Some(llr_Expression::StructFieldAccess {
                base: Box::new(lower_expression(base, ctx)?),
                name: name.clone(),
            })
        }
        tree_Expression::Cast { from, to } => Some(llr_Expression::Cast {
            from: Box::new(lower_expression(from, ctx)?),
            to: to.clone(),
        }),
        tree_Expression::CodeBlock(expr) => Some(llr_Expression::CodeBlock(
            expr.iter().map(|e| lower_expression(e, ctx)).collect::<Option<_>>()?,
        )),
        tree_Expression::FunctionCall { function, arguments, .. } => {
            Some(llr_Expression::FunctionCall {
                function: Box::new(lower_expression(function, ctx)?),
                arguments: arguments
                    .iter()
                    .map(|e| lower_expression(e, ctx))
                    .collect::<Option<_>>()?,
            })
        }
        tree_Expression::SelfAssignment { lhs, rhs, op } => Some(llr_Expression::SelfAssignment {
            lhs: Box::new(lower_expression(lhs, ctx)?),
            rhs: Box::new(lower_expression(rhs, ctx)?),
            op: *op,
        }),
        tree_Expression::BinaryExpression { lhs, rhs, op } => {
            Some(llr_Expression::BinaryExpression {
                lhs: Box::new(lower_expression(lhs, ctx)?),
                rhs: Box::new(lower_expression(rhs, ctx)?),
                op: *op,
            })
        }
        tree_Expression::UnaryOp { sub, op } => {
            Some(llr_Expression::UnaryOp { sub: Box::new(lower_expression(sub, ctx)?), op: *op })
        }
        tree_Expression::ImageReference { resource_ref, .. } => {
            Some(llr_Expression::ImageReference { resource_ref: resource_ref.clone() })
        }
        tree_Expression::Condition { condition, true_expr, false_expr } => {
            Some(llr_Expression::Condition {
                condition: Box::new(lower_expression(condition, ctx)?),
                true_expr: Box::new(lower_expression(true_expr, ctx)?),
                false_expr: lower_expression(false_expr, ctx).map(Box::new),
            })
        }
        tree_Expression::Array { element_ty, values } => Some(llr_Expression::Array {
            element_ty: element_ty.clone(),
            values: values.iter().map(|e| lower_expression(e, ctx)).collect::<Option<_>>()?,
        }),
        tree_Expression::Struct { ty, values } => Some(llr_Expression::Struct {
            ty: ty.clone(),
            values: values
                .iter()
                .map(|(s, e)| Some((s.clone(), lower_expression(e, ctx)?)))
                .collect::<Option<_>>()?,
        }),
        tree_Expression::PathElements { elements } => match elements {
            crate::expression_tree::Path::Elements(_) => todo!(),
            crate::expression_tree::Path::Events(_) => todo!(),
        },
        tree_Expression::EasingCurve(x) => Some(llr_Expression::EasingCurve(x.clone())),
        tree_Expression::LinearGradient { angle, stops } => Some(llr_Expression::LinearGradient {
            angle: Box::new(lower_expression(angle, ctx)?),
            stops: stops
                .iter()
                .map(|(a, b)| Some((lower_expression(a, ctx)?, lower_expression(b, ctx)?)))
                .collect::<Option<_>>()?,
        }),
        tree_Expression::EnumerationValue(e) => Some(llr_Expression::EnumerationValue(e.clone())),
        tree_Expression::ReturnStatement(x) => Some(llr_Expression::ReturnStatement(
            x.as_ref().and_then(|e| lower_expression(e, ctx)).map(Box::new),
        )),
        tree_Expression::LayoutCacheAccess { layout_cache_prop, index, repeater_index } => {
            Some(llr_Expression::LayoutCacheAccess {
                layout_cache_prop: ctx
                    .mapping
                    .map_property_reference(layout_cache_prop, ctx.state)?,
                index: *index,
                repeater_index: repeater_index
                    .as_ref()
                    .and_then(|e| lower_expression(e, ctx))
                    .map(Box::new),
            })
        }
        tree_Expression::ComputeLayoutInfo(_, _) => todo!(),
        tree_Expression::SolveLayout(_, _) => todo!(),
    }
}

fn repeater_special_property(
    element: &Weak<RefCell<Element>>,
    component: &Rc<crate::object_tree::Component>,
    property_index: usize,
) -> Option<llr_Expression> {
    let mut r = PropertyReference::Local { sub_component_path: vec![], property_index };
    let enclosing = element.upgrade().unwrap().borrow().enclosing_component.upgrade().unwrap();
    let mut level = 0;
    let mut component = component.clone();
    while !Rc::ptr_eq(&enclosing, &component) {
        component = component
            .parent_element
            .upgrade()
            .unwrap()
            .borrow()
            .enclosing_component
            .upgrade()
            .unwrap();
        level += 1;
    }
    if let Some(level) = NonZeroUsize::new(level) {
        r = PropertyReference::InParent { level, parent_reference: Box::new(r) };
    }
    Some(llr_Expression::PropertyReference(r))
}

pub fn lower_animation(a: &PropertyAnimation, ctx: &ExpressionContext<'_>) -> Option<Animation> {
    fn lower_animation_element(
        a: &ElementRc,
        ctx: &ExpressionContext<'_>,
    ) -> Option<llr_Expression> {
        Some(llr_Expression::Struct {
            ty: animation_ty(),
            values: a
                .borrow()
                .bindings
                .iter()
                .map(|(k, v)| Some((k.clone(), lower_expression(&v.borrow().expression, ctx)?)))
                .collect::<Option<_>>()?,
        })
    }

    fn animation_ty() -> Type {
        Type::Struct {
            fields: IntoIterator::into_iter([
                ("duration".to_string(), Type::Int32),
                ("loop-count".to_string(), Type::Int32),
                ("easing".to_string(), Type::Easing),
            ])
            .collect(),
            name: Some("PropertyAnimation".into()),
            node: None,
        }
    }

    match a {
        PropertyAnimation::Static(a) => Some(Animation::Static(lower_animation_element(a, ctx)?)),
        PropertyAnimation::Transition { state_ref, animations } => {
            let set_state = llr_Expression::StoreLocalVariable {
                name: "state".into(),
                value: Box::new(lower_expression(state_ref, ctx)?),
            };
            let mut get_anim = llr_Expression::default_value_for_type(&animation_ty())?;
            for tr in animations.iter().rev() {
                let condition = lower_expression(
                    &tr.condition(tree_Expression::ReadLocalVariable {
                        name: "state".into(),
                        ty: state_ref.ty(),
                    }),
                    ctx,
                )?;
                get_anim = llr_Expression::Condition {
                    condition: Box::new(condition),
                    true_expr: Box::new(lower_animation_element(&tr.animation, ctx)?),
                    false_expr: Some(Box::new(get_anim)),
                }
            }
            Some(Animation::Transition(llr_Expression::CodeBlock(vec![set_state, get_anim])))
        }
    }
}
