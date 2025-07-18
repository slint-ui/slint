// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

use std::cell::RefCell;
use std::collections::{BTreeMap, HashMap};
use std::num::NonZeroUsize;
use std::rc::{Rc, Weak};

use itertools::Either;
use smol_str::{format_smolstr, SmolStr};

use super::lower_to_item_tree::{LoweredElement, LoweredSubComponentMapping, LoweringState};
use super::{Animation, PropertyIdx, PropertyReference, RepeatedElementIdx};
use crate::expression_tree::{BuiltinFunction, Callable, Expression as tree_Expression};
use crate::langtype::{EnumerationValue, Struct, Type};
use crate::layout::Orientation;
use crate::llr::Expression as llr_Expression;
use crate::namedreference::NamedReference;
use crate::object_tree::{Element, ElementRc, PropertyAnimation};
use crate::typeregister::BUILTIN;

pub struct ExpressionLoweringCtxInner<'a> {
    pub component: &'a Rc<crate::object_tree::Component>,
    /// The mapping for the current component
    pub mapping: &'a LoweredSubComponentMapping,
    pub parent: Option<&'a ExpressionLoweringCtxInner<'a>>,
}
#[derive(derive_more::Deref)]
pub struct ExpressionLoweringCtx<'a> {
    pub state: &'a mut LoweringState,
    #[deref]
    pub inner: ExpressionLoweringCtxInner<'a>,
}

impl ExpressionLoweringCtx<'_> {
    pub fn map_property_reference(&self, from: &NamedReference) -> PropertyReference {
        let element = from.element();
        let enclosing = &element.borrow().enclosing_component.upgrade().unwrap();
        if !enclosing.is_global() {
            let mut map = &self.inner;
            let mut level = 0;
            while !Rc::ptr_eq(enclosing, map.component) {
                map = map.parent.unwrap();
                level += 1;
            }
            if let Some(level) = NonZeroUsize::new(level) {
                return PropertyReference::InParent {
                    level,
                    parent_reference: Box::new(
                        map.mapping.map_property_reference(from, self.state),
                    ),
                };
            }
        }
        self.mapping.map_property_reference(from, self.state)
    }
}

impl super::TypeResolutionContext for ExpressionLoweringCtx<'_> {
    fn property_ty(&self, _: &PropertyReference) -> &Type {
        todo!()
    }
}

pub fn lower_expression(
    expression: &tree_Expression,
    ctx: &mut ExpressionLoweringCtx<'_>,
) -> llr_Expression {
    match expression {
        tree_Expression::Invalid => {
            panic!("internal error, encountered invalid expression at code generation time")
        }
        tree_Expression::Uncompiled(_) => panic!(),
        tree_Expression::StringLiteral(s) => llr_Expression::StringLiteral(s.clone()),
        tree_Expression::NumberLiteral(n, unit) => {
            llr_Expression::NumberLiteral(unit.normalize(*n))
        }
        tree_Expression::BoolLiteral(b) => llr_Expression::BoolLiteral(*b),
        tree_Expression::PropertyReference(nr) => {
            llr_Expression::PropertyReference(ctx.map_property_reference(nr))
        }
        tree_Expression::ElementReference(e) => {
            let elem = e.upgrade().unwrap();
            let enclosing = elem.borrow().enclosing_component.upgrade().unwrap();
            // When within a ShowPopupMenu builtin function, this is a reference to the root of the menu item tree
            if Rc::ptr_eq(&elem, &enclosing.root_element) {
                if let Some(idx) = ctx
                    .component
                    .menu_item_tree
                    .borrow()
                    .iter()
                    .position(|c| Rc::ptr_eq(c, &enclosing))
                {
                    return llr_Expression::NumberLiteral(idx as _);
                }
            }

            // We map an element reference to a reference to the property "" inside that native item
            llr_Expression::PropertyReference(
                ctx.map_property_reference(&NamedReference::new(&elem, SmolStr::default())),
            )
        }
        tree_Expression::RepeaterIndexReference { element } => {
            repeater_special_property(element, ctx.component, 1usize.into())
        }
        tree_Expression::RepeaterModelReference { element } => {
            repeater_special_property(element, ctx.component, 0usize.into())
        }
        tree_Expression::FunctionParameterReference { index, .. } => {
            llr_Expression::FunctionParameterReference { index: *index }
        }
        tree_Expression::StoreLocalVariable { name, value } => llr_Expression::StoreLocalVariable {
            name: name.clone(),
            value: Box::new(lower_expression(value, ctx)),
        },
        tree_Expression::ReadLocalVariable { name, ty } => {
            llr_Expression::ReadLocalVariable { name: name.clone(), ty: ty.clone() }
        }
        tree_Expression::StructFieldAccess { base, name } => llr_Expression::StructFieldAccess {
            base: Box::new(lower_expression(base, ctx)),
            name: name.clone(),
        },
        tree_Expression::ArrayIndex { array, index } => llr_Expression::ArrayIndex {
            array: Box::new(lower_expression(array, ctx)),
            index: Box::new(lower_expression(index, ctx)),
        },
        tree_Expression::Cast { from, to } => {
            llr_Expression::Cast { from: Box::new(lower_expression(from, ctx)), to: to.clone() }
        }
        tree_Expression::CodeBlock(expr) => {
            llr_Expression::CodeBlock(expr.iter().map(|e| lower_expression(e, ctx)).collect::<_>())
        }
        tree_Expression::FunctionCall { function, arguments, .. } => match function {
            Callable::Builtin(BuiltinFunction::RestartTimer) => lower_restart_timer(arguments),
            Callable::Builtin(BuiltinFunction::ShowPopupWindow) => {
                lower_show_popup_window(arguments, ctx)
            }
            Callable::Builtin(BuiltinFunction::ClosePopupWindow) => {
                lower_close_popup_window(arguments, ctx)
            }
            Callable::Builtin(f) => {
                let mut arguments =
                    arguments.iter().map(|e| lower_expression(e, ctx)).collect::<Vec<_>>();
                if *f == BuiltinFunction::Translate {
                    if let llr_Expression::Array { as_model, .. } = &mut arguments[3] {
                        *as_model = false;
                    }
                    #[cfg(feature = "bundle-translations")]
                    if let Some(translation_builder) = ctx.state.translation_builder.as_mut() {
                        return translation_builder.lower_translate_call(arguments);
                    }
                }
                llr_Expression::BuiltinFunctionCall { function: f.clone(), arguments }
            }
            Callable::Callback(nr) => {
                let arguments = arguments.iter().map(|e| lower_expression(e, ctx)).collect::<_>();
                llr_Expression::CallBackCall { callback: ctx.map_property_reference(nr), arguments }
            }
            Callable::Function(nr)
                if nr
                    .element()
                    .borrow()
                    .native_class()
                    .is_some_and(|n| n.properties.contains_key(nr.name())) =>
            {
                llr_Expression::ItemMemberFunctionCall { function: ctx.map_property_reference(nr) }
            }
            Callable::Function(nr) => {
                let arguments = arguments.iter().map(|e| lower_expression(e, ctx)).collect::<_>();
                llr_Expression::FunctionCall { function: ctx.map_property_reference(nr), arguments }
            }
        },
        tree_Expression::SelfAssignment { lhs, rhs, op, .. } => {
            lower_assignment(lhs, rhs, *op, ctx)
        }
        tree_Expression::BinaryExpression { lhs, rhs, op } => llr_Expression::BinaryExpression {
            lhs: Box::new(lower_expression(lhs, ctx)),
            rhs: Box::new(lower_expression(rhs, ctx)),
            op: *op,
        },
        tree_Expression::UnaryOp { sub, op } => {
            llr_Expression::UnaryOp { sub: Box::new(lower_expression(sub, ctx)), op: *op }
        }
        tree_Expression::ImageReference { resource_ref, nine_slice, .. } => {
            llr_Expression::ImageReference {
                resource_ref: resource_ref.clone(),
                nine_slice: *nine_slice,
            }
        }
        tree_Expression::Condition { condition, true_expr, false_expr } => {
            let (true_ty, false_ty) = (true_expr.ty(), false_expr.ty());
            llr_Expression::Condition {
                condition: Box::new(lower_expression(condition, ctx)),
                true_expr: Box::new(lower_expression(true_expr, ctx)),
                false_expr: if false_ty == Type::Invalid
                    || false_ty == Type::Void
                    || true_ty == false_ty
                {
                    Box::new(lower_expression(false_expr, ctx))
                } else {
                    // Because the type of the Condition is based on the false expression, we need to insert a cast
                    Box::new(llr_Expression::Cast {
                        from: Box::new(lower_expression(false_expr, ctx)),
                        to: Type::Void,
                    })
                },
            }
        }
        tree_Expression::Array { element_ty, values } => llr_Expression::Array {
            element_ty: element_ty.clone(),
            values: values.iter().map(|e| lower_expression(e, ctx)).collect::<_>(),
            as_model: true,
        },
        tree_Expression::Struct { ty, values } => llr_Expression::Struct {
            ty: ty.clone(),
            values: values
                .iter()
                .map(|(s, e)| (s.clone(), lower_expression(e, ctx)))
                .collect::<_>(),
        },
        tree_Expression::PathData(data) => compile_path(data, ctx),
        tree_Expression::EasingCurve(x) => llr_Expression::EasingCurve(x.clone()),
        tree_Expression::LinearGradient { angle, stops } => llr_Expression::LinearGradient {
            angle: Box::new(lower_expression(angle, ctx)),
            stops: stops
                .iter()
                .map(|(a, b)| (lower_expression(a, ctx), lower_expression(b, ctx)))
                .collect::<_>(),
        },
        tree_Expression::RadialGradient { stops } => llr_Expression::RadialGradient {
            stops: stops
                .iter()
                .map(|(a, b)| (lower_expression(a, ctx), lower_expression(b, ctx)))
                .collect::<_>(),
        },
        tree_Expression::EnumerationValue(e) => llr_Expression::EnumerationValue(e.clone()),
        tree_Expression::ReturnStatement(..) => {
            panic!("The remove return pass should have removed all return")
        }
        tree_Expression::LayoutCacheAccess { layout_cache_prop, index, repeater_index } => {
            llr_Expression::LayoutCacheAccess {
                layout_cache_prop: ctx.map_property_reference(layout_cache_prop),
                index: *index,
                repeater_index: repeater_index.as_ref().map(|e| lower_expression(e, ctx).into()),
            }
        }
        tree_Expression::ComputeLayoutInfo(l, o) => compute_layout_info(l, *o, ctx),
        tree_Expression::SolveLayout(l, o) => solve_layout(l, *o, ctx),
        tree_Expression::MinMax { ty, op, lhs, rhs } => llr_Expression::MinMax {
            ty: ty.clone(),
            op: *op,
            lhs: Box::new(lower_expression(lhs, ctx)),
            rhs: Box::new(lower_expression(rhs, ctx)),
        },
        tree_Expression::EmptyComponentFactory => llr_Expression::EmptyComponentFactory,
        tree_Expression::DebugHook { expression, .. } => lower_expression(expression, ctx),
    }
}

fn lower_assignment(
    lhs: &tree_Expression,
    rhs: &tree_Expression,
    op: char,
    ctx: &mut ExpressionLoweringCtx,
) -> llr_Expression {
    match lhs {
        tree_Expression::PropertyReference(nr) => {
            let rhs = lower_expression(rhs, ctx);
            let property = ctx.map_property_reference(nr);
            let value = if op == '=' {
                rhs
            } else {
                llr_Expression::BinaryExpression {
                    lhs: llr_Expression::PropertyReference(property.clone()).into(),
                    rhs: rhs.into(),
                    op,
                }
            }
            .into();
            llr_Expression::PropertyAssignment { property, value }
        }
        tree_Expression::StructFieldAccess { base, name } => {
            let ty = base.ty();

            static COUNT: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
            let unique_name = format_smolstr!(
                "struct_assignment{}",
                COUNT.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
            );
            let s = tree_Expression::StoreLocalVariable {
                name: unique_name.clone(),
                value: base.clone(),
            };
            let lower_base =
                tree_Expression::ReadLocalVariable { name: unique_name, ty: ty.clone() };
            let mut values = HashMap::new();
            let Type::Struct(ty) = ty else { unreachable!() };

            for field in ty.fields.keys() {
                let e = if field != name {
                    tree_Expression::StructFieldAccess {
                        base: lower_base.clone().into(),
                        name: field.clone(),
                    }
                } else if op == '=' {
                    rhs.clone()
                } else {
                    tree_Expression::BinaryExpression {
                        lhs: tree_Expression::StructFieldAccess {
                            base: lower_base.clone().into(),
                            name: field.clone(),
                        }
                        .into(),
                        rhs: Box::new(rhs.clone()),
                        op,
                    }
                };
                values.insert(field.clone(), e);
            }

            let new_value =
                tree_Expression::CodeBlock(vec![s, tree_Expression::Struct { ty, values }]);
            lower_assignment(base, &new_value, '=', ctx)
        }
        tree_Expression::RepeaterModelReference { element } => {
            let rhs = lower_expression(rhs, ctx);
            let prop = repeater_special_property(element, ctx.component, 0usize.into());

            let level = match &prop {
                llr_Expression::PropertyReference(PropertyReference::InParent {
                    level, ..
                }) => (*level).into(),
                _ => 0,
            };

            let value = Box::new(if op == '=' {
                rhs
            } else {
                llr_Expression::BinaryExpression { lhs: prop.into(), rhs: rhs.into(), op }
            });

            llr_Expression::ModelDataAssignment { level, value }
        }
        tree_Expression::ArrayIndex { array, index } => {
            let rhs = lower_expression(rhs, ctx);
            let array = Box::new(lower_expression(array, ctx));
            let index = Box::new(lower_expression(index, ctx));
            let value = Box::new(if op == '=' {
                rhs
            } else {
                // FIXME: this will compute the index and the array twice:
                // Ideally we should store the index and the array in local variable
                llr_Expression::BinaryExpression {
                    lhs: llr_Expression::ArrayIndex { array: array.clone(), index: index.clone() }
                        .into(),
                    rhs: rhs.into(),
                    op,
                }
            });

            llr_Expression::ArrayIndexAssignment { array, index, value }
        }
        _ => panic!("not a rvalue"),
    }
}

fn repeater_special_property(
    element: &Weak<RefCell<Element>>,
    component: &Rc<crate::object_tree::Component>,
    property_index: PropertyIdx,
) -> llr_Expression {
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
    if let Some(level) = NonZeroUsize::new(level - 1) {
        r = PropertyReference::InParent { level, parent_reference: Box::new(r) };
    }
    llr_Expression::PropertyReference(r)
}

fn lower_restart_timer(args: &[tree_Expression]) -> llr_Expression {
    if let [tree_Expression::ElementReference(e)] = args {
        let timer_element = e.upgrade().unwrap();
        let timer_comp = timer_element.borrow().enclosing_component.upgrade().unwrap();

        let timer_list = timer_comp.timers.borrow();
        let timer_index = timer_list
            .iter()
            .position(|t| Rc::ptr_eq(&t.element.upgrade().unwrap(), &timer_element))
            .unwrap();

        llr_Expression::BuiltinFunctionCall {
            function: BuiltinFunction::RestartTimer,
            arguments: vec![llr_Expression::NumberLiteral(timer_index as _)],
        }
    } else {
        panic!("invalid arguments to RestartTimer");
    }
}

fn lower_show_popup_window(
    args: &[tree_Expression],
    ctx: &mut ExpressionLoweringCtx,
) -> llr_Expression {
    if let [tree_Expression::ElementReference(e)] = args {
        let popup_window = e.upgrade().unwrap();
        let pop_comp = popup_window.borrow().enclosing_component.upgrade().unwrap();
        let parent_component = pop_comp
            .parent_element
            .upgrade()
            .unwrap()
            .borrow()
            .enclosing_component
            .upgrade()
            .unwrap();
        let popup_list = parent_component.popup_windows.borrow();
        let (popup_index, popup) = popup_list
            .iter()
            .enumerate()
            .find(|(_, p)| Rc::ptr_eq(&p.component, &pop_comp))
            .unwrap();
        let item_ref = lower_expression(
            &tree_Expression::ElementReference(Rc::downgrade(&popup.parent_element)),
            ctx,
        );

        llr_Expression::BuiltinFunctionCall {
            function: BuiltinFunction::ShowPopupWindow,
            arguments: vec![
                llr_Expression::NumberLiteral(popup_index as _),
                llr_Expression::EnumerationValue(popup.close_policy.clone()),
                item_ref,
            ],
        }
    } else {
        panic!("invalid arguments to ShowPopupWindow");
    }
}

fn lower_close_popup_window(
    args: &[tree_Expression],
    ctx: &mut ExpressionLoweringCtx,
) -> llr_Expression {
    if let [tree_Expression::ElementReference(e)] = args {
        let popup_window = e.upgrade().unwrap();
        let pop_comp = popup_window.borrow().enclosing_component.upgrade().unwrap();
        let parent_component = pop_comp
            .parent_element
            .upgrade()
            .unwrap()
            .borrow()
            .enclosing_component
            .upgrade()
            .unwrap();
        let popup_list = parent_component.popup_windows.borrow();
        let (popup_index, popup) = popup_list
            .iter()
            .enumerate()
            .find(|(_, p)| Rc::ptr_eq(&p.component, &pop_comp))
            .unwrap();
        let item_ref = lower_expression(
            &tree_Expression::ElementReference(Rc::downgrade(&popup.parent_element)),
            ctx,
        );

        llr_Expression::BuiltinFunctionCall {
            function: BuiltinFunction::ClosePopupWindow,
            arguments: vec![llr_Expression::NumberLiteral(popup_index as _), item_ref],
        }
    } else {
        panic!("invalid arguments to ShowPopupWindow");
    }
}

pub fn lower_animation(a: &PropertyAnimation, ctx: &mut ExpressionLoweringCtx<'_>) -> Animation {
    fn lower_animation_element(
        a: &ElementRc,
        ctx: &mut ExpressionLoweringCtx<'_>,
    ) -> llr_Expression {
        llr_Expression::Struct {
            values: animation_fields()
                .map(|(k, ty)| {
                    let e = a.borrow().bindings.get(&k).map_or_else(
                        || llr_Expression::default_value_for_type(&ty).unwrap(),
                        |v| lower_expression(&v.borrow().expression, ctx),
                    );
                    (k, e)
                })
                .collect::<_>(),
            ty: animation_ty(),
        }
    }

    fn animation_fields() -> impl Iterator<Item = (SmolStr, Type)> {
        IntoIterator::into_iter([
            (SmolStr::new_static("duration"), Type::Int32),
            (SmolStr::new_static("iteration-count"), Type::Float32),
            (
                SmolStr::new_static("direction"),
                Type::Enumeration(BUILTIN.with(|e| e.enums.AnimationDirection.clone())),
            ),
            (SmolStr::new_static("easing"), Type::Easing),
            (SmolStr::new_static("delay"), Type::Int32),
        ])
    }

    fn animation_ty() -> Rc<Struct> {
        Rc::new(Struct {
            fields: animation_fields().collect(),
            name: Some("slint::private_api::PropertyAnimation".into()),
            node: None,
            rust_attributes: None,
        })
    }

    match a {
        PropertyAnimation::Static(a) => Animation::Static(lower_animation_element(a, ctx)),
        PropertyAnimation::Transition { state_ref, animations } => {
            let set_state = llr_Expression::StoreLocalVariable {
                name: "state".into(),
                value: Box::new(lower_expression(state_ref, ctx)),
            };
            let animation_ty = Type::Struct(animation_ty());
            let mut get_anim = llr_Expression::default_value_for_type(&animation_ty).unwrap();
            for tr in animations.iter().rev() {
                let condition = lower_expression(
                    &tr.condition(tree_Expression::ReadLocalVariable {
                        name: "state".into(),
                        ty: state_ref.ty(),
                    }),
                    ctx,
                );
                get_anim = llr_Expression::Condition {
                    condition: Box::new(condition),
                    true_expr: Box::new(lower_animation_element(&tr.animation, ctx)),
                    false_expr: Box::new(get_anim),
                }
            }
            let result = llr_Expression::Struct {
                // This is going to be a tuple
                ty: Rc::new(Struct {
                    fields: IntoIterator::into_iter([
                        (SmolStr::new_static("0"), animation_ty),
                        // The type is an instant, which does not exist in our type system
                        (SmolStr::new_static("1"), Type::Invalid),
                    ])
                    .collect(),
                    name: None,
                    node: None,
                    rust_attributes: None,
                }),
                values: IntoIterator::into_iter([
                    (SmolStr::new_static("0"), get_anim),
                    (
                        SmolStr::new_static("1"),
                        llr_Expression::StructFieldAccess {
                            base: llr_Expression::ReadLocalVariable {
                                name: "state".into(),
                                ty: state_ref.ty(),
                            }
                            .into(),
                            name: "change_time".into(),
                        },
                    ),
                ])
                .collect(),
            };
            Animation::Transition(llr_Expression::CodeBlock(vec![set_state, result]))
        }
    }
}

fn compute_layout_info(
    l: &crate::layout::Layout,
    o: Orientation,
    ctx: &mut ExpressionLoweringCtx,
) -> llr_Expression {
    match l {
        crate::layout::Layout::GridLayout(layout) => {
            let (padding, spacing) = generate_layout_padding_and_spacing(&layout.geometry, o, ctx);
            let cells = grid_layout_cell_data(layout, o, ctx);
            llr_Expression::ExtraBuiltinFunctionCall {
                function: "grid_layout_info".into(),
                arguments: vec![cells, spacing, padding],
                return_ty: crate::typeregister::layout_info_type().into(),
            }
        }
        crate::layout::Layout::BoxLayout(layout) => {
            let (padding, spacing) = generate_layout_padding_and_spacing(&layout.geometry, o, ctx);
            let bld = box_layout_data(layout, o, ctx);
            let sub_expression = if o == layout.orientation {
                llr_Expression::ExtraBuiltinFunctionCall {
                    function: "box_layout_info".into(),
                    arguments: vec![bld.cells, spacing, padding, bld.alignment],
                    return_ty: crate::typeregister::layout_info_type().into(),
                }
            } else {
                llr_Expression::ExtraBuiltinFunctionCall {
                    function: "box_layout_info_ortho".into(),
                    arguments: vec![bld.cells, padding],
                    return_ty: crate::typeregister::layout_info_type().into(),
                }
            };
            match bld.compute_cells {
                Some((cells_variable, elements)) => llr_Expression::BoxLayoutFunction {
                    cells_variable,
                    repeater_indices: None,
                    elements,
                    orientation: o,
                    sub_expression: Box::new(sub_expression),
                },
                None => sub_expression,
            }
        }
    }
}

fn solve_layout(
    l: &crate::layout::Layout,
    o: Orientation,
    ctx: &mut ExpressionLoweringCtx,
) -> llr_Expression {
    match l {
        crate::layout::Layout::GridLayout(layout) => {
            let (padding, spacing) = generate_layout_padding_and_spacing(&layout.geometry, o, ctx);
            let cells = grid_layout_cell_data(layout, o, ctx);
            let size = layout_geometry_size(&layout.geometry.rect, o, ctx);
            if let (Some(button_roles), Orientation::Horizontal) = (&layout.dialog_button_roles, o)
            {
                let cells_ty = cells.ty(ctx);
                let e = crate::typeregister::BUILTIN.with(|e| e.enums.DialogButtonRole.clone());
                let roles = button_roles
                    .iter()
                    .map(|r| {
                        llr_Expression::EnumerationValue(EnumerationValue {
                            value: e.values.iter().position(|x| x == r).unwrap() as _,
                            enumeration: e.clone(),
                        })
                    })
                    .collect();
                llr_Expression::CodeBlock(vec![
                    llr_Expression::ComputeDialogLayoutCells {
                        cells_variable: "cells".into(),
                        roles: llr_Expression::Array {
                            element_ty: Type::Enumeration(e),
                            values: roles,
                            as_model: false,
                        }
                        .into(),
                        unsorted_cells: Box::new(cells),
                    },
                    llr_Expression::ExtraBuiltinFunctionCall {
                        function: "solve_grid_layout".into(),
                        arguments: vec![make_struct(
                            "GridLayoutData",
                            [
                                ("size", Type::Float32, size),
                                ("spacing", Type::Float32, spacing),
                                ("padding", padding.ty(ctx), padding),
                                (
                                    "cells",
                                    cells_ty.clone(),
                                    llr_Expression::ReadLocalVariable {
                                        name: "cells".into(),
                                        ty: cells_ty,
                                    },
                                ),
                            ],
                        )],
                        return_ty: Type::LayoutCache,
                    },
                ])
            } else {
                llr_Expression::ExtraBuiltinFunctionCall {
                    function: "solve_grid_layout".into(),
                    arguments: vec![make_struct(
                        "GridLayoutData",
                        [
                            ("size", Type::Float32, size),
                            ("spacing", Type::Float32, spacing),
                            ("padding", padding.ty(ctx), padding),
                            ("cells", cells.ty(ctx), cells),
                        ],
                    )],
                    return_ty: Type::LayoutCache,
                }
            }
        }
        crate::layout::Layout::BoxLayout(layout) => {
            let (padding, spacing) = generate_layout_padding_and_spacing(&layout.geometry, o, ctx);
            let bld = box_layout_data(layout, o, ctx);
            let size = layout_geometry_size(&layout.geometry.rect, o, ctx);
            let data = make_struct(
                "BoxLayoutData",
                [
                    ("size", Type::Float32, size),
                    ("spacing", Type::Float32, spacing),
                    ("padding", padding.ty(ctx), padding),
                    (
                        "alignment",
                        crate::typeregister::BUILTIN
                            .with(|e| Type::Enumeration(e.enums.LayoutAlignment.clone())),
                        bld.alignment,
                    ),
                    ("cells", bld.cells.ty(ctx), bld.cells),
                ],
            );
            match bld.compute_cells {
                Some((cells_variable, elements)) => llr_Expression::BoxLayoutFunction {
                    cells_variable,
                    repeater_indices: Some("repeated_indices".into()),
                    elements,
                    orientation: o,
                    sub_expression: Box::new(llr_Expression::ExtraBuiltinFunctionCall {
                        function: "solve_box_layout".into(),
                        arguments: vec![
                            data,
                            llr_Expression::ReadLocalVariable {
                                name: "repeated_indices".into(),
                                ty: Type::Array(Type::Int32.into()),
                            },
                        ],
                        return_ty: Type::LayoutCache,
                    }),
                },
                None => llr_Expression::ExtraBuiltinFunctionCall {
                    function: "solve_box_layout".into(),
                    arguments: vec![
                        data,
                        llr_Expression::Array {
                            element_ty: Type::Int32,
                            values: vec![],
                            as_model: false,
                        },
                    ],
                    return_ty: Type::LayoutCache,
                },
            }
        }
    }
}

struct BoxLayoutDataResult {
    alignment: llr_Expression,
    cells: llr_Expression,
    /// When there are repeater involved, we need to do a BoxLayoutFunction with the
    /// given cell variable and elements
    compute_cells: Option<(String, Vec<Either<llr_Expression, RepeatedElementIdx>>)>,
}

fn box_layout_data(
    layout: &crate::layout::BoxLayout,
    orientation: Orientation,
    ctx: &mut ExpressionLoweringCtx,
) -> BoxLayoutDataResult {
    let alignment = if let Some(expr) = &layout.geometry.alignment {
        llr_Expression::PropertyReference(ctx.map_property_reference(expr))
    } else {
        let e = crate::typeregister::BUILTIN.with(|e| e.enums.LayoutAlignment.clone());
        llr_Expression::EnumerationValue(EnumerationValue {
            value: e.default_value,
            enumeration: e,
        })
    };

    let repeater_count =
        layout.elems.iter().filter(|i| i.element.borrow().repeated.is_some()).count();

    let element_ty = crate::typeregister::box_layout_cell_data_type();

    if repeater_count == 0 {
        let cells = llr_Expression::Array {
            values: layout
                .elems
                .iter()
                .map(|li| {
                    let layout_info =
                        get_layout_info(&li.element, ctx, &li.constraints, orientation);
                    make_struct(
                        "BoxLayoutCellData",
                        [(
                            "constraint",
                            crate::typeregister::layout_info_type().into(),
                            layout_info,
                        )],
                    )
                })
                .collect(),
            element_ty,
            as_model: false,
        };
        BoxLayoutDataResult { alignment, cells, compute_cells: None }
    } else {
        let mut elements = vec![];
        for item in &layout.elems {
            if item.element.borrow().repeated.is_some() {
                let repeater_index =
                    match ctx.mapping.element_mapping.get(&item.element.clone().into()).unwrap() {
                        LoweredElement::Repeated { repeated_index } => *repeated_index,
                        _ => panic!(),
                    };
                elements.push(Either::Right(repeater_index))
            } else {
                let layout_info =
                    get_layout_info(&item.element, ctx, &item.constraints, orientation);
                elements.push(Either::Left(make_struct(
                    "BoxLayoutCellData",
                    [("constraint", crate::typeregister::layout_info_type().into(), layout_info)],
                )));
            }
        }
        let cells = llr_Expression::ReadLocalVariable {
            name: "cells".into(),
            ty: Type::Array(Rc::new(crate::typeregister::layout_info_type().into())),
        };
        BoxLayoutDataResult { alignment, cells, compute_cells: Some(("cells".into(), elements)) }
    }
}

fn grid_layout_cell_data(
    layout: &crate::layout::GridLayout,
    orientation: Orientation,
    ctx: &mut ExpressionLoweringCtx,
) -> llr_Expression {
    llr_Expression::Array {
        element_ty: grid_layout_cell_data_ty(),
        values: layout
            .elems
            .iter()
            .map(|c| {
                let (col_or_row, span) = c.col_or_row_and_span(orientation);
                let layout_info =
                    get_layout_info(&c.item.element, ctx, &c.item.constraints, orientation);

                make_struct(
                    "GridLayoutCellData",
                    [
                        ("constraint", crate::typeregister::layout_info_type().into(), layout_info),
                        ("col_or_row", Type::Int32, llr_Expression::NumberLiteral(col_or_row as _)),
                        ("span", Type::Int32, llr_Expression::NumberLiteral(span as _)),
                    ],
                )
            })
            .collect(),
        as_model: false,
    }
}

pub(super) fn grid_layout_cell_data_ty() -> Type {
    Type::Struct(Rc::new(Struct {
        fields: IntoIterator::into_iter([
            (SmolStr::new_static("col_or_row"), Type::Int32),
            (SmolStr::new_static("span"), Type::Int32),
            (SmolStr::new_static("constraint"), crate::typeregister::layout_info_type().into()),
        ])
        .collect(),
        name: Some("GridLayoutCellData".into()),
        node: None,
        rust_attributes: None,
    }))
}

fn generate_layout_padding_and_spacing(
    layout_geometry: &crate::layout::LayoutGeometry,
    orientation: Orientation,
    ctx: &ExpressionLoweringCtx,
) -> (llr_Expression, llr_Expression) {
    let padding_prop = |expr| {
        if let Some(expr) = expr {
            llr_Expression::PropertyReference(ctx.map_property_reference(expr))
        } else {
            llr_Expression::NumberLiteral(0.)
        }
    };
    let spacing = padding_prop(layout_geometry.spacing.orientation(orientation));
    let (begin, end) = layout_geometry.padding.begin_end(orientation);

    let padding = make_struct(
        "Padding",
        [("begin", Type::Float32, padding_prop(begin)), ("end", Type::Float32, padding_prop(end))],
    );

    (padding, spacing)
}

fn layout_geometry_size(
    rect: &crate::layout::LayoutRect,
    orientation: Orientation,
    ctx: &ExpressionLoweringCtx,
) -> llr_Expression {
    match rect.size_reference(orientation) {
        Some(nr) => llr_Expression::PropertyReference(ctx.map_property_reference(nr)),
        None => llr_Expression::NumberLiteral(0.),
    }
}

pub fn get_layout_info(
    elem: &ElementRc,
    ctx: &mut ExpressionLoweringCtx,
    constraints: &crate::layout::LayoutConstraints,
    orientation: Orientation,
) -> llr_Expression {
    let layout_info = if let Some(layout_info_prop) = &elem.borrow().layout_info_prop(orientation) {
        llr_Expression::PropertyReference(ctx.map_property_reference(layout_info_prop))
    } else {
        lower_expression(&crate::layout::implicit_layout_info_call(elem, orientation), ctx)
    };

    if constraints.has_explicit_restrictions(orientation) {
        let store = llr_Expression::StoreLocalVariable {
            name: "layout_info".into(),
            value: layout_info.into(),
        };
        let ty = crate::typeregister::layout_info_type();
        let mut values = ty
            .fields
            .keys()
            .map(|p| {
                (
                    p.clone(),
                    llr_Expression::StructFieldAccess {
                        base: llr_Expression::ReadLocalVariable {
                            name: "layout_info".into(),
                            ty: ty.clone().into(),
                        }
                        .into(),
                        name: p.clone(),
                    },
                )
            })
            .collect::<BTreeMap<_, _>>();

        for (nr, s) in constraints.for_each_restrictions(orientation) {
            values.insert(
                s.into(),
                llr_Expression::PropertyReference(ctx.map_property_reference(nr)),
            );
        }
        llr_Expression::CodeBlock([store, llr_Expression::Struct { ty, values }].into())
    } else {
        layout_info
    }
}

fn compile_path(
    path: &crate::expression_tree::Path,
    ctx: &mut ExpressionLoweringCtx,
) -> llr_Expression {
    fn llr_path_elements(elements: Vec<llr_Expression>) -> llr_Expression {
        llr_Expression::Cast {
            from: llr_Expression::Array {
                element_ty: crate::typeregister::path_element_type(),
                values: elements,
                as_model: false,
            }
            .into(),
            to: Type::PathData,
        }
    }

    match path {
        crate::expression_tree::Path::Elements(elements) => {
            let converted_elements = elements
                .iter()
                .map(|element| {
                    let element_type = Rc::new(Struct {
                        fields: element
                            .element_type
                            .properties
                            .iter()
                            .map(|(k, v)| (k.clone(), v.ty.clone()))
                            .collect(),
                        name: element.element_type.native_class.cpp_type.clone(),
                        node: None,
                        rust_attributes: None,
                    });

                    llr_Expression::Struct {
                        ty: element_type,
                        values: element
                            .element_type
                            .properties
                            .iter()
                            .map(|(element_field_name, element_property)| {
                                (
                                    element_field_name.clone(),
                                    element.bindings.get(element_field_name).map_or_else(
                                        || {
                                            llr_Expression::default_value_for_type(
                                                &element_property.ty,
                                            )
                                            .unwrap()
                                        },
                                        |expr| lower_expression(&expr.borrow().expression, ctx),
                                    ),
                                )
                            })
                            .collect(),
                    }
                })
                .collect();
            llr_path_elements(converted_elements)
        }
        crate::expression_tree::Path::Events(events, points) => {
            if events.is_empty() || points.is_empty() {
                return llr_path_elements(vec![]);
            }

            let events: Vec<_> = events.iter().map(|event| lower_expression(event, ctx)).collect();

            let event_type = events.first().unwrap().ty(ctx);

            let points: Vec<_> = points.iter().map(|point| lower_expression(point, ctx)).collect();

            let point_type = points.first().unwrap().ty(ctx);

            llr_Expression::Cast {
                from: llr_Expression::Struct {
                    ty: Rc::new(Struct {
                        fields: IntoIterator::into_iter([
                            (SmolStr::new_static("events"), Type::Array(event_type.clone().into())),
                            (SmolStr::new_static("points"), Type::Array(point_type.clone().into())),
                        ])
                        .collect(),
                        name: None,
                        node: None,
                        rust_attributes: None,
                    }),
                    values: IntoIterator::into_iter([
                        (
                            SmolStr::new_static("events"),
                            llr_Expression::Array {
                                element_ty: event_type,
                                values: events,
                                as_model: false,
                            },
                        ),
                        (
                            SmolStr::new_static("points"),
                            llr_Expression::Array {
                                element_ty: point_type,
                                values: points,
                                as_model: false,
                            },
                        ),
                    ])
                    .collect(),
                }
                .into(),
                to: Type::PathData,
            }
        }
        crate::expression_tree::Path::Commands(commands) => llr_Expression::Cast {
            from: lower_expression(commands, ctx).into(),
            to: Type::PathData,
        },
    }
}

pub fn make_struct(
    name: &str,
    it: impl IntoIterator<Item = (&'static str, Type, llr_Expression)>,
) -> llr_Expression {
    let mut fields = BTreeMap::<SmolStr, Type>::new();
    let mut values = BTreeMap::<SmolStr, llr_Expression>::new();
    for (name, ty, expr) in it {
        fields.insert(SmolStr::new(name), ty);
        values.insert(SmolStr::new(name), expr);
    }

    llr_Expression::Struct {
        ty: Rc::new(Struct {
            fields,
            name: Some(format_smolstr!("slint::private_api::{name}")),
            node: None,
            rust_attributes: None,
        }),
        values,
    }
}
