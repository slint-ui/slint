// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

//! This pass computes the layout constraint

use lyon_path::geom::euclid::approxeq::ApproxEq;

use crate::diagnostics::{BuildDiagnostics, DiagnosticLevel, Spanned};
use crate::expression_tree::*;
use crate::langtype::ElementType;
use crate::langtype::Type;
use crate::layout::*;
use crate::object_tree::*;
use crate::typeloader::TypeLoader;
use crate::typeregister::{TypeRegister, layout_info_type};
use smol_str::{SmolStr, format_smolstr};
use std::cell::RefCell;
use std::collections::HashSet;
use std::rc::Rc;

/// Add a `pure function layoutinfo-v-with-constraint(width: length) -> LayoutInfo`
/// to `elem` with the given `body`. The body reads
/// `FunctionParameterReference { index: 0 }` for the width.
fn synthesize_layoutinfo_v_with_constraint_on(
    elem: &ElementRc,
    span: crate::diagnostics::SourceLocation,
    body: Expression,
) {
    let function_ty = Type::Function(Rc::new(crate::langtype::Function {
        return_type: crate::typeregister::layout_info_type().into(),
        args: vec![Type::LogicalLength],
        arg_names: vec![SmolStr::new_static("width")],
    }));
    let prop_name = SmolStr::new_static("layoutinfo-v-with-constraint");
    let nr = crate::namedreference::NamedReference::new(elem, prop_name.clone());

    let mut elem_mut = elem.borrow_mut();
    elem_mut.property_declarations.insert(
        prop_name.clone(),
        PropertyDeclaration {
            property_type: function_ty,
            visibility: crate::object_tree::PropertyVisibility::Private,
            pure: Some(true),
            ..Default::default()
        },
    );
    elem_mut.bindings.insert(prop_name, BindingExpression::new_with_span(body, span).into());
    elem_mut.layout_info_v_with_constraint = Some(nr);
}

/// Rewrite a `layoutinfo-v` expression body to consume `width_param`
/// as its cross-axis constraint instead of reading the descendants'
/// width property.
fn rewrite_layoutinfo_v_for_constraint(expr: &mut Expression, width_param: &Expression) {
    expr.visit_recursive_mut(&mut |sub| match sub {
        Expression::ComputeBoxLayoutInfo {
            orientation: Orientation::Vertical,
            cross_axis_size,
            ..
        }
        | Expression::ComputeGridLayoutInfo {
            orientation: Orientation::Vertical,
            cross_axis_size,
            ..
        }
        | Expression::ComputeFlexboxLayoutInfo {
            orientation: Orientation::Vertical,
            cross_axis_size,
            ..
        } => {
            *cross_axis_size = Some(Box::new(width_param.clone()));
        }
        Expression::FunctionCall {
            function: Callable::Builtin(BuiltinFunction::ImplicitLayoutInfo(Orientation::Vertical)),
            arguments,
            ..
        } => {
            // Find the target element of the implicit layout-info query.
            let target = match arguments.first() {
                Some(Expression::ElementReference(weak)) => weak.upgrade(),
                _ => None,
            };
            if let Some(target) = target {
                // Target has the parametrized function: swap for the function call.
                if let Some(constrained_nr) =
                    target.borrow().inherited_layout_info_v_with_constraint()
                {
                    *sub = Expression::FunctionCall {
                        function: Callable::Function(crate::namedreference::NamedReference::new(
                            &target,
                            constrained_nr.name().clone(),
                        )),
                        arguments: vec![width_param.clone()],
                        source_location: None,
                    };
                    return;
                }
                // Builtin height-for-width: replace the default -1 with
                // the cross-axis size. The second arg is the
                // `cross_axis_constraint` of `ImplicitLayoutInfo`.
                if target.borrow().is_builtin_height_for_width() {
                    debug_assert!(arguments.len() >= 2);
                    if let Some(second) = arguments.get_mut(1) {
                        *second = width_param.clone();
                    }
                }
            }
        }
        Expression::PropertyReference(nr) => {
            // PropertyReference to an element's vertical layout-info prop
            // whose target has the parametrized function: swap for the function call.
            let target = nr.element();
            let is_vertical_layout_info = target
                .borrow()
                .layout_info_prop(Orientation::Vertical)
                .map(|prop_nr| {
                    prop_nr.name() == nr.name() && Rc::ptr_eq(&prop_nr.element(), &target)
                })
                .unwrap_or(false);
            if !is_vertical_layout_info {
                return;
            }
            if let Some(constrained_nr) = target.borrow().inherited_layout_info_v_with_constraint()
            {
                *sub = Expression::FunctionCall {
                    function: Callable::Function(crate::namedreference::NamedReference::new(
                        &target,
                        constrained_nr.name().clone(),
                    )),
                    arguments: vec![width_param.clone()],
                    source_location: None,
                };
            }
        }
        _ => {}
    });
}

/// Mirror of [`synthesize_layoutinfo_v_with_constraint_on`] for the horizontal axis.
fn synthesize_layoutinfo_h_with_constraint_on(
    elem: &ElementRc,
    span: crate::diagnostics::SourceLocation,
    body: Expression,
) {
    let function_ty = Type::Function(Rc::new(crate::langtype::Function {
        return_type: crate::typeregister::layout_info_type().into(),
        args: vec![Type::LogicalLength],
        arg_names: vec![SmolStr::new_static("height")],
    }));
    let prop_name = SmolStr::new_static("layoutinfo-h-with-constraint");
    let nr = crate::namedreference::NamedReference::new(elem, prop_name.clone());

    let mut elem_mut = elem.borrow_mut();
    elem_mut.property_declarations.insert(
        prop_name.clone(),
        PropertyDeclaration {
            property_type: function_ty,
            visibility: crate::object_tree::PropertyVisibility::Private,
            pure: Some(true),
            ..Default::default()
        },
    );
    elem_mut.bindings.insert(prop_name, BindingExpression::new_with_span(body, span).into());
    elem_mut.layout_info_h_with_constraint = Some(nr);
}

/// Same as `rewrite_layoutinfo_v_for_constraint`, but for the horizontal
/// axis. Only `ComputeFlexboxLayoutInfo` and `PropertyReference` are
/// rewritten — there's no width-for-height equivalent in box/grid
/// layouts, and `ImplicitLayoutInfo(Horizontal)` on a non-component
/// element doesn't depend on `self.height`.
fn rewrite_layoutinfo_h_for_constraint(expr: &mut Expression, height_param: &Expression) {
    expr.visit_recursive_mut(&mut |sub| match sub {
        Expression::ComputeFlexboxLayoutInfo {
            orientation: Orientation::Horizontal,
            cross_axis_size,
            ..
        } => {
            *cross_axis_size = Some(Box::new(height_param.clone()));
        }
        Expression::PropertyReference(nr) => {
            // PropertyReference to an element's horizontal layout-info
            // prop whose target has the parametrized function: swap for the function call.
            let target = nr.element();
            let is_horizontal_layout_info = target
                .borrow()
                .layout_info_prop(Orientation::Horizontal)
                .map(|prop_nr| {
                    prop_nr.name() == nr.name() && Rc::ptr_eq(&prop_nr.element(), &target)
                })
                .unwrap_or(false);
            if !is_horizontal_layout_info {
                return;
            }
            if let Some(constrained_nr) = target.borrow().inherited_layout_info_h_with_constraint()
            {
                *sub = Expression::FunctionCall {
                    function: Callable::Function(crate::namedreference::NamedReference::new(
                        &target,
                        constrained_nr.name().clone(),
                    )),
                    arguments: vec![height_param.clone()],
                    source_location: None,
                };
            }
        }
        _ => {}
    });
}

/// Same as `synthesize_layoutinfo_v_with_constraint`, but for the
/// horizontal axis. Fires on any element whose `layoutinfo-h` depends
/// (transitively) on a flex with horizontal cross-axis — directly
/// (column-direction flex) or via a descendant / base component.
pub fn synthesize_layoutinfo_h_with_constraint(component: &Rc<Component>) {
    /// Bottom-up walk, returns `true` if the subtree contains an
    /// h-cross-axis dependency (a flex with cross axis on horizontal,
    /// or a descendant / base component that has `layoutinfo-h-with-constraint`).
    fn walk(elem: &ElementRc) -> bool {
        let children = elem.borrow().children.clone();
        let mut has_h_cross = false;
        for c in &children {
            has_h_cross |= walk(c);
        }
        // Repeated elements moved their body into a sub-component;
        // recurse into it so we synthesize on the body's tree too.
        let repeated_body = {
            let elem_b = elem.borrow();
            if elem_b.repeated.is_some() {
                if let ElementType::Component(base_comp) = &elem_b.base_type {
                    Some(base_comp.root_element.clone())
                } else {
                    None
                }
            } else {
                None
            }
        };
        if let Some(body_root) = repeated_body {
            has_h_cross |= walk(&body_root);
        }

        let (already_synthesized, base_has_constraint, self_is_h_cross_flex, h_nr_clone) = {
            let elem_b = elem.borrow();
            let layout_type = elem_b.debug.first().and_then(|d| d.layout.as_ref()).cloned();
            let self_is = matches!(
                layout_type,
                Some(crate::layout::Layout::FlexboxLayout(ref l))
                    if !matches!(
                        l.axis_relation(Orientation::Horizontal),
                        crate::layout::FlexboxAxisRelation::MainAxis,
                    )
            );
            let base_has = matches!(
                &elem_b.base_type,
                ElementType::Component(base_comp)
                    if base_comp.root_element.borrow().layout_info_h_with_constraint.is_some()
            );
            (
                elem_b.layout_info_h_with_constraint.is_some(),
                base_has,
                self_is,
                elem_b.layout_info_prop(Orientation::Horizontal).cloned(),
            )
        };
        has_h_cross |= self_is_h_cross_flex | base_has_constraint;

        if !has_h_cross || already_synthesized {
            return has_h_cross;
        }
        let Some(h_nr) = h_nr_clone else { return has_h_cross };
        // `h_nr.element()` may be stale for repeater-body elements (their
        // bindings were moved to a new sub-component root by
        // `repeater_component`). Read from `elem` itself, which is the
        // current owner of the binding.
        let Some(h_binding) = elem.borrow().bindings.get(h_nr.name()).map(|b| b.borrow().clone())
        else {
            return has_h_cross;
        };

        let span = h_binding.span.clone().unwrap_or_else(|| elem.borrow().to_source_location());
        let mut body = h_binding.expression.clone();
        let height_param =
            Expression::FunctionParameterReference { index: 0, ty: Type::LogicalLength };
        rewrite_layoutinfo_h_for_constraint(&mut body, &height_param);

        synthesize_layoutinfo_h_with_constraint_on(elem, span, body);
        has_h_cross
    }
    walk(&component.root_element);
}

/// Synthesize `layoutinfo-v-with-constraint` on every element whose
/// vertical layout info depends on its width. The parameterized
/// function breaks the recursion that would otherwise occur when the
/// parent queries this element's vertical info.
pub fn synthesize_layoutinfo_v_with_constraint(component: &Rc<Component>) {
    /// Bottom-up walk, returns `true` if the subtree carries a v-cross-axis
    /// dependency (a height-for-width descendant, a row-direction flex, or
    /// a base component / descendant that already has `layoutinfo-v-with-constraint`).
    fn walk(elem: &ElementRc) -> bool {
        let children = elem.borrow().children.clone();
        let mut has_v_cross = false;
        for c in &children {
            has_v_cross |= walk(c);
        }
        // Repeater body: recurse into the moved-out sub-component.
        let repeated_body = {
            let elem_b = elem.borrow();
            if elem_b.repeated.is_some() {
                if let ElementType::Component(base_comp) = &elem_b.base_type {
                    Some(base_comp.root_element.clone())
                } else {
                    None
                }
            } else {
                None
            }
        };
        if let Some(body_root) = repeated_body {
            has_v_cross |= walk(&body_root);
        }

        let (already_synthesized, base_has_constraint, self_is_v_cross_flex, v_nr_clone) = {
            let elem_b = elem.borrow();
            has_v_cross |= elem_b.is_builtin_height_for_width();
            let layout_type = elem_b.debug.first().and_then(|d| d.layout.as_ref()).cloned();
            let self_is = matches!(
                layout_type,
                Some(crate::layout::Layout::FlexboxLayout(ref l))
                    if !matches!(
                        l.axis_relation(Orientation::Vertical),
                        crate::layout::FlexboxAxisRelation::MainAxis,
                    )
            );
            let base_has = matches!(
                &elem_b.base_type,
                ElementType::Component(base_comp)
                    if base_comp.root_element.borrow().layout_info_v_with_constraint.is_some()
            );
            (
                elem_b.layout_info_v_with_constraint.is_some(),
                base_has,
                self_is,
                elem_b.layout_info_prop(Orientation::Vertical).cloned(),
            )
        };
        has_v_cross |= self_is_v_cross_flex | base_has_constraint;

        if !has_v_cross || already_synthesized {
            return has_v_cross;
        }
        let Some(v_nr) = v_nr_clone else { return has_v_cross };
        let Some(v_binding) = elem.borrow().bindings.get(v_nr.name()).map(|b| b.borrow().clone())
        else {
            return has_v_cross;
        };

        let span = v_binding.span.clone().unwrap_or_else(|| elem.borrow().to_source_location());
        let mut body = v_binding.expression.clone();
        let width_param =
            Expression::FunctionParameterReference { index: 0, ty: Type::LogicalLength };
        rewrite_layoutinfo_v_for_constraint(&mut body, &width_param);

        synthesize_layoutinfo_v_with_constraint_on(elem, span, body);
        has_v_cross
    }
    walk(&component.root_element);
}

/// Lower all layouts and assign a LayoutConstraints to the component
pub fn lower_layouts(
    component: &Rc<Component>,
    type_loader: &mut TypeLoader,
    style_metrics: &Rc<Component>,
    diag: &mut BuildDiagnostics,
) {
    // lower the preferred-{width, height}: 100%;
    recurse_elem_including_sub_components(component, &(), &mut |elem, _| {
        if check_preferred_size_100(elem, "preferred-width", diag) {
            elem.borrow_mut().default_fill_parent.0 = true;
        }
        if check_preferred_size_100(elem, "preferred-height", diag) {
            elem.borrow_mut().default_fill_parent.1 = true;
        }
        let base = elem.borrow().sub_component().cloned();
        if let Some(base) = base {
            let base = base.root_element.borrow();
            let mut elem_mut = elem.borrow_mut();
            elem_mut.default_fill_parent.0 |= base.default_fill_parent.0;
            elem_mut.default_fill_parent.1 |= base.default_fill_parent.1;
        }
    });

    *component.root_constraints.borrow_mut() =
        LayoutConstraints::new(&component.root_element, Some((diag, DiagnosticLevel::Error)));

    recurse_elem_including_sub_components(
        component,
        &Option::default(),
        &mut |elem, parent_layout_type| {
            let component = elem.borrow().enclosing_component.upgrade().unwrap();

            // A popup is not visited as a component on its own (it can be nested in a sub-component),
            // so set the constraints of its root here, once per component when visiting its root. A
            // redundant size constraint on a popup root is only a warning (not an error like on a
            // window root) for compatibility with older versions of Slint that did not report it.
            if Rc::ptr_eq(elem, &component.root_element) {
                for popup in component.popup_windows.borrow().iter() {
                    *popup.component.root_constraints.borrow_mut() = LayoutConstraints::new(
                        &popup.component.root_element,
                        Some((&mut *diag, DiagnosticLevel::Warning)),
                    );
                }
            }

            lower_element_layout(
                &component,
                elem,
                &type_loader.global_type_registry.borrow(),
                style_metrics,
                parent_layout_type,
                diag,
            )
        },
    );
}

fn check_preferred_size_100(elem: &ElementRc, prop: &str, diag: &mut BuildDiagnostics) -> bool {
    let ret = if let Some(p) = elem.borrow().bindings.get(prop) {
        if p.borrow().expression.ty() == Type::Percent {
            if !matches!(p.borrow().expression.ignore_debug_hooks(), Expression::NumberLiteral(val, _) if *val == 100.)
            {
                diag.push_error(
                    format!("{prop} must either be a length, or the literal '100%'"),
                    &*p.borrow(),
                );
            }
            true
        } else {
            false
        }
    } else {
        false
    };
    if ret {
        elem.borrow_mut().bindings.remove(prop).unwrap();
        return true;
    }
    false
}

/// If the element is a layout, lower it to a Rectangle, and set the geometry property of the element inside it.
/// Returns the name of the layout type if the element was a layout and has been lowered
fn lower_element_layout(
    component: &Rc<Component>,
    elem: &ElementRc,
    type_register: &TypeRegister,
    style_metrics: &Rc<Component>,
    parent_layout_type: &Option<SmolStr>,
    diag: &mut BuildDiagnostics,
) -> Option<SmolStr> {
    let layout_type = if let ElementType::Builtin(base_type) = &elem.borrow().base_type {
        Some(base_type.name.clone())
    } else {
        None
    };

    check_no_layout_properties(elem, &layout_type, parent_layout_type, diag);

    match layout_type.as_ref()?.as_str() {
        "Row" => return layout_type,
        "GridLayout" => lower_grid_layout(component, elem, diag, type_register),
        "HorizontalLayout" => lower_box_layout(elem, diag, Orientation::Horizontal),
        "VerticalLayout" => lower_box_layout(elem, diag, Orientation::Vertical),
        "FlexboxLayout" => lower_flexbox_layout(elem, diag),
        "Dialog" => {
            lower_dialog_layout(elem, style_metrics, diag);
            // return now, the Dialog stays in the tree as a Dialog
            return layout_type;
        }
        _ => return None,
    };

    let mut elem = elem.borrow_mut();
    let elem = &mut *elem;
    let prev_base = std::mem::replace(&mut elem.base_type, type_register.empty_type());
    elem.default_fill_parent = (true, true);
    // Create fake properties for the layout properties
    // like alignment, spacing, spacing-horizontal, spacing-vertical
    for (p, ty) in prev_base.property_list() {
        if !elem.base_type.lookup_property(&p).is_valid()
            && !elem.property_declarations.contains_key(&p)
        {
            elem.property_declarations.insert(p, ty.into());
        }
    }

    layout_type
}

// to detect mixing auto and non-literal expressions in row/col values
#[derive(Debug, PartialEq, Eq, Clone, Copy)]
enum RowColExpressionType {
    Auto, // not specified
    Literal,
    RuntimeExpression,
}
impl RowColExpressionType {
    fn from_option_expr(
        expr: &Option<Expression>,
        is_number_literal: bool,
    ) -> RowColExpressionType {
        match expr {
            None => RowColExpressionType::Auto,
            Some(_) if is_number_literal => RowColExpressionType::Literal,
            Some(_) => RowColExpressionType::RuntimeExpression,
        }
    }
}

/// Two views of the running auto-vs-runtime classification as we walk a
/// GridLayout's children. See the call site in `lower_grid_layout` for why we
/// keep both: the lenient view is the only signal that a given conflict was
/// previously accepted (and so should be a warning rather than an error).
#[derive(Default)]
struct NumberingTypes {
    strict: Option<RowColExpressionType>,
    lenient: Option<RowColExpressionType>,
}

fn lower_grid_layout(
    component: &Rc<Component>,
    grid_layout_element: &ElementRc,
    diag: &mut BuildDiagnostics,
    type_register: &TypeRegister,
) {
    let mut grid = GridLayout {
        elems: Default::default(),
        geometry: LayoutGeometry::new(grid_layout_element),
        dialog_button_roles: None,
        uses_auto: false,
    };

    let layout_organized_data_prop = create_new_prop(
        grid_layout_element,
        SmolStr::new_static("layout-organized-data"),
        Type::ArrayOfU16,
    );
    let layout_cache_prop_h = create_new_prop(
        grid_layout_element,
        SmolStr::new_static("layout-cache-h"),
        Type::LayoutCache,
    );
    let layout_cache_prop_v = create_new_prop(
        grid_layout_element,
        SmolStr::new_static("layout-cache-v"),
        Type::LayoutCache,
    );
    let layout_info_prop_h = create_new_prop(
        grid_layout_element,
        SmolStr::new_static("layoutinfo-h"),
        layout_info_type().into(),
    );
    let layout_info_prop_v = create_new_prop(
        grid_layout_element,
        SmolStr::new_static("layoutinfo-v"),
        layout_info_type().into(),
    );

    let layout_children = std::mem::take(&mut grid_layout_element.borrow_mut().children);
    let mut collected_children = Vec::new();
    let mut new_row = false; // true until the first child of a Row, or the first item after an empty Row
    // The consistency check runs two classifications in parallel:
    //
    //   `strict`  — looks one level into a `for`/`if`'s sub-component for
    //               row/col bindings that `repeater_component` moved off the
    //               wrapper. This matches what the layout solver actually
    //               consumes.
    //   `lenient` — ignores those moved bindings, i.e. the same classification
    //               the consistency check used to perform.
    //
    // Some layouts that *should* have been rejected as auto-vs-runtime mixes
    // slipped through historically: the wrapper looked Auto from the outside
    // because its bindings had been moved into a sub-component, so the check
    // never saw the conflict. We can't simply error on every such input
    // because real `.slint` files written against the old behavior are out
    // there. Instead we keep both views and only emit a hard error when the
    // lenient view would also have flagged the mix; cases the lenient view
    // missed are downgraded to a warning so existing code keeps compiling.
    let mut numbering_type = NumberingTypes::default();
    let mut num_cached_items: usize = 0;
    for layout_child in layout_children {
        let is_repeated_row = {
            if layout_child.borrow().repeated.is_some()
                && let ElementType::Component(comp) = &layout_child.borrow().base_type
            {
                match &comp.root_element.borrow().base_type {
                    ElementType::Builtin(b) => b.name == "Row",
                    _ => false,
                }
            } else {
                false
            }
        };
        if is_repeated_row {
            grid.add_repeated_row(
                &layout_child,
                &layout_cache_prop_h,
                &layout_cache_prop_v,
                &layout_organized_data_prop,
                diag,
                &mut num_cached_items,
            );
            collected_children.push(layout_child);
            new_row = true;
        } else if layout_child.borrow().base_type.type_name() == Some("Row") {
            new_row = true;
            let row_children = std::mem::take(&mut layout_child.borrow_mut().children);
            for row_child in row_children {
                if let Some(binding) = row_child.borrow_mut().bindings.get("row") {
                    diag.push_warning(
                        "The 'row' property cannot be used for elements inside a Row. This was accepted by previous versions of Slint, but may become an error in the future".to_string(),
                        &*binding.borrow(),
                    );
                }
                grid.add_element(
                    &row_child,
                    new_row,
                    &layout_cache_prop_h,
                    &layout_cache_prop_v,
                    &layout_organized_data_prop,
                    &mut numbering_type,
                    diag,
                    &mut num_cached_items,
                );
                collected_children.push(row_child);
                new_row = false;
            }
            new_row = true; // the end of a Row means the next item is the first of a new row
            if layout_child.borrow().has_popup_child {
                // We need to keep that element otherwise the popup will malfunction
                layout_child.borrow_mut().base_type = type_register.empty_type();
                collected_children.push(layout_child);
            } else {
                component.optimized_elements.borrow_mut().push(layout_child);
            }
        } else {
            grid.add_element(
                &layout_child,
                new_row,
                &layout_cache_prop_h,
                &layout_cache_prop_v,
                &layout_organized_data_prop,
                &mut numbering_type,
                diag,
                &mut num_cached_items,
            );
            collected_children.push(layout_child);
            new_row = false;
        }
    }
    grid_layout_element.borrow_mut().children = collected_children;
    grid.uses_auto = numbering_type.strict == Some(RowColExpressionType::Auto);
    let span = grid_layout_element.borrow().to_source_location();

    layout_organized_data_prop.element().borrow_mut().bindings.insert(
        layout_organized_data_prop.name().clone(),
        BindingExpression::new_with_span(
            Expression::OrganizeGridLayout(grid.clone()),
            span.clone(),
        )
        .into(),
    );
    layout_cache_prop_h.element().borrow_mut().bindings.insert(
        layout_cache_prop_h.name().clone(),
        BindingExpression::new_with_span(
            Expression::SolveGridLayout {
                layout_organized_data_prop: layout_organized_data_prop.clone(),
                layout: grid.clone(),
                orientation: Orientation::Horizontal,
            },
            span.clone(),
        )
        .into(),
    );
    layout_cache_prop_v.element().borrow_mut().bindings.insert(
        layout_cache_prop_v.name().clone(),
        BindingExpression::new_with_span(
            Expression::SolveGridLayout {
                layout_organized_data_prop: layout_organized_data_prop.clone(),
                layout: grid.clone(),
                orientation: Orientation::Vertical,
            },
            span.clone(),
        )
        .into(),
    );
    layout_info_prop_h.element().borrow_mut().bindings.insert(
        layout_info_prop_h.name().clone(),
        BindingExpression::new_with_span(
            Expression::ComputeGridLayoutInfo {
                layout_organized_data_prop: layout_organized_data_prop.clone(),
                layout: grid.clone(),
                orientation: Orientation::Horizontal,
                cross_axis_size: None,
            },
            span.clone(),
        )
        .into(),
    );
    layout_info_prop_v.element().borrow_mut().bindings.insert(
        layout_info_prop_v.name().clone(),
        BindingExpression::new_with_span(
            Expression::ComputeGridLayoutInfo {
                layout_organized_data_prop: layout_organized_data_prop.clone(),
                layout: grid.clone(),
                orientation: Orientation::Vertical,
                cross_axis_size: None,
            },
            span,
        )
        .into(),
    );
    grid_layout_element.borrow_mut().layout_info_prop =
        Some((layout_info_prop_h, layout_info_prop_v));
    for d in grid_layout_element.borrow_mut().debug.iter_mut() {
        d.layout = Some(Layout::GridLayout(grid.clone()));
    }
}

impl GridLayout {
    fn add_element(
        &mut self,
        item_element: &ElementRc,
        new_row: bool,
        layout_cache_prop_h: &NamedReference,
        layout_cache_prop_v: &NamedReference,
        organized_data_prop: &NamedReference,
        numbering_type: &mut NumberingTypes,
        diag: &mut BuildDiagnostics,
        num_cached_items: &mut usize,
    ) {
        // Some compile-time checks
        {
            // Returns (strict, lenient, is_number_literal):
            //
            //   `strict`  - the binding the layout solver will see at runtime,
            //               looking one level into a repeater wrapper's
            //               sub-component when needed.
            //   `lenient` - the binding visible directly on the wrapper, which
            //               is empty for repeater wrappers because their
            //               bindings have been moved into the sub-component.
            //
            // The two only differ for a repeater wrapper that had a row/col
            // binding on its body; everywhere else they are equal. We hand
            // both to the consistency check so it can tell apart cases that
            // were already wrong before this code changed from cases that were
            // only just unmasked.
            //
            // `is_number_literal` describes the strict expression. It is safe
            // to share one flag because either direct == strict (the binding
            // was on the wrapper) or direct is None (in which case the lenient
            // classification will be Auto regardless of the flag).
            let mut check_expr = |name: &str| {
                let mut is_number_literal = false;
                let mut read = |elem: &ElementRc, lit: &mut bool| -> Option<Expression> {
                    let b = elem.borrow().bindings.get(name).cloned()?;
                    let b_borrow = b.borrow();
                    if !b_borrow.has_binding() {
                        return None;
                    }
                    *lit = check_number_literal_is_positive_integer(
                        &b_borrow.expression,
                        name,
                        &*b_borrow,
                        diag,
                    );
                    Some(b_borrow.expression.clone())
                };
                let lenient = read(item_element, &mut is_number_literal);
                let strict = if lenient.is_some() {
                    lenient.clone()
                } else if item_element.borrow().repeated.is_some()
                    && let ElementType::Component(base) = item_element.borrow().base_type.clone()
                {
                    read(&base.root_element, &mut is_number_literal)
                } else {
                    None
                };
                (strict, lenient, is_number_literal)
            };

            let (row_strict, row_lenient, row_lit) = check_expr("row");
            let (col_strict, col_lenient, col_lit) = check_expr("col");
            check_expr("rowspan");
            check_expr("colspan");

            // Returns true iff a classification of `ty`, compared against the
            // already-recorded numbering `num`, would have errored under the
            // historical rule (set on the first non-Literal element; mismatch
            // after that is the mix).
            let would_conflict = |num: &Option<RowColExpressionType>,
                                  ty: &RowColExpressionType|
             -> bool {
                !matches!(ty, RowColExpressionType::Literal) && matches!(num, Some(t) if t != ty)
            };

            // Classify each axis into the diagnostic it would produce, and
            // immediately fold its non-Literal types into `numbering_type` so
            // an intra-element mix (row Runtime + col Auto on the same
            // wrapper) still trips when the second axis is checked.
            let mut classify_and_update =
                |strict: RowColExpressionType, lenient: RowColExpressionType| -> Option<bool> {
                    let diag = if would_conflict(&numbering_type.strict, &strict) {
                        // true ↔ strict-and-lenient conflict ↔ this was
                        // already wrong under the old check, so it stays an
                        // error; false ↔ strict-only conflict ↔ warning.
                        Some(would_conflict(&numbering_type.lenient, &lenient))
                    } else {
                        None
                    };
                    // Record the first non-Literal value seen for each view,
                    // even after a conflict — once set, never overwritten,
                    // matching the historical check's behavior.
                    if numbering_type.strict.is_none()
                        && !matches!(strict, RowColExpressionType::Literal)
                    {
                        numbering_type.strict = Some(strict);
                    }
                    if numbering_type.lenient.is_none()
                        && !matches!(lenient, RowColExpressionType::Literal)
                    {
                        numbering_type.lenient = Some(lenient);
                    }
                    diag
                };

            let row_strict_ty = RowColExpressionType::from_option_expr(&row_strict, row_lit);
            let row_lenient_ty = RowColExpressionType::from_option_expr(&row_lenient, row_lit);
            let col_strict_ty = RowColExpressionType::from_option_expr(&col_strict, col_lit);
            let col_lenient_ty = RowColExpressionType::from_option_expr(&col_lenient, col_lit);

            let row_diag = classify_and_update(row_strict_ty, row_lenient_ty);
            let col_diag = classify_and_update(col_strict_ty, col_lenient_ty);

            // Pick the most severe diagnostic across both axes. `Some(true)`
            // (error) wins over `Some(false)` (warning); ties prefer row for
            // a stable, source-order span.
            let report = match (row_diag, col_diag) {
                (Some(true), _) => Some(("row", true)),
                (_, Some(true)) => Some(("col", true)),
                (Some(false), _) => Some(("row", false)),
                (_, Some(false)) => Some(("col", false)),
                _ => None,
            };

            if let Some((prop_name, is_error)) = report {
                // Pick the tightest span we can: a binding on the wrapper if
                // there is one, otherwise the same binding inside the
                // repeater's sub-component root, otherwise the wrapper as a
                // whole.
                let element_ref = item_element.borrow();
                let inner_borrow = match &element_ref.base_type {
                    ElementType::Component(base) if element_ref.repeated.is_some() => {
                        Some(base.root_element.clone())
                    }
                    _ => None,
                };
                let direct_binding = element_ref.bindings.get(prop_name).cloned();
                let inner_binding =
                    inner_borrow.as_ref().and_then(|e| e.borrow().bindings.get(prop_name).cloned());
                let binding = direct_binding.or(inner_binding);
                let binding_borrow = binding.as_ref().map(|b| b.borrow());
                let span: &dyn Spanned = match &binding_borrow {
                    Some(b) => &**b,
                    None => &*element_ref,
                };
                if is_error {
                    diag.push_error(
                        format!("Cannot mix auto-numbering and runtime expressions for the '{prop_name}' property"),
                        span,
                    );
                } else {
                    diag.push_warning(
                        format!("Cannot mix auto-numbering and runtime expressions for the '{prop_name}' property. This was accepted by previous versions of Slint, but may become an error in the future"),
                        span,
                    );
                }
            }
        }

        let propref = |name: &'static str| -> Option<RowColExpr> {
            let nr = crate::layout::binding_reference(item_element, name).map(|nr| {
                // similar to adjust_references in repeater_component.rs (which happened before these references existed)
                let e = nr.element();
                let mut nr = nr.clone();
                if e.borrow().repeated.is_some()
                    && let crate::langtype::ElementType::Component(c) = e.borrow().base_type.clone()
                {
                    nr = NamedReference::new(&c.root_element, nr.name().clone())
                };
                nr
            });
            nr.map(RowColExpr::Named)
        };

        let row_expr = propref("row");
        let col_expr = propref("col");
        let rowspan_expr = propref("rowspan");
        let colspan_expr = propref("colspan");

        self.add_element_with_coord_as_expr(
            item_element,
            new_row,
            (&row_expr, &col_expr),
            (&rowspan_expr, &colspan_expr),
            layout_cache_prop_h,
            layout_cache_prop_v,
            organized_data_prop,
            diag,
            num_cached_items,
        );
    }

    fn add_element_with_coord(
        &mut self,
        item_element: &ElementRc,
        (row, col): (u16, u16),
        (rowspan, colspan): (u16, u16),
        layout_cache_prop_h: &NamedReference,
        layout_cache_prop_v: &NamedReference,
        organized_data_prop: &NamedReference,
        diag: &mut BuildDiagnostics,
        num_cached_items: &mut usize,
    ) {
        self.add_element_with_coord_as_expr(
            item_element,
            false, // new_row
            (&Some(RowColExpr::Literal(row)), &Some(RowColExpr::Literal(col))),
            (&Some(RowColExpr::Literal(rowspan)), &Some(RowColExpr::Literal(colspan))),
            layout_cache_prop_h,
            layout_cache_prop_v,
            organized_data_prop,
            diag,
            num_cached_items,
        )
    }

    fn add_repeated_row(
        &mut self,
        item_element: &ElementRc,
        layout_cache_prop_h: &NamedReference,
        layout_cache_prop_v: &NamedReference,
        organized_data_prop: &NamedReference,
        diag: &mut BuildDiagnostics,
        num_cached_items: &mut usize,
    ) {
        let layout_item = create_layout_item(item_element, diag);
        if let ElementType::Component(comp) = &item_element.borrow().base_type {
            let mut children_layout_items = Vec::new();
            let jump_pos = *num_cached_items;

            // Determine whether any child is an inner repeater (dynamic stride)
            let children_ref = comp.root_element.borrow().children.clone();
            let has_inner_repeaters = children_ref.iter().any(|c| c.borrow().repeated.is_some());

            // Compute stride expressions for H/V coord caches and org-data cache.
            // For non-inner rows: stride is compile-time (step * entries_per_item).
            // For inner-repeater rows: stride is runtime, stored at cache[index+1] by
            // the layout solver (GridLayoutCacheGenerator / OrganizedDataGenerator).
            let step = children_ref.len() as f64;
            let (stride_h_expr, stride_v_expr, stride_org_expr): (
                Expression,
                Expression,
                Expression,
            ) = if has_inner_repeaters {
                // stride = step * entries_per_item, computed at runtime and stored at
                // cache[jump_pos*2+1] (coord) or cache[jump_pos*4+1] (org)
                (
                    Expression::LayoutCacheAccess {
                        layout_cache_prop: layout_cache_prop_h.clone(),
                        index: jump_pos * 2 + 1,
                        repeater_index: None,
                        entries_per_item: 1,
                    },
                    Expression::LayoutCacheAccess {
                        layout_cache_prop: layout_cache_prop_v.clone(),
                        index: jump_pos * 2 + 1,
                        repeater_index: None,
                        entries_per_item: 1,
                    },
                    Expression::LayoutCacheAccess {
                        layout_cache_prop: organized_data_prop.clone(),
                        index: jump_pos * 4 + 1,
                        repeater_index: None,
                        entries_per_item: 1,
                    },
                )
            } else {
                // stride = step * 2 for coord (pos+size per child), step * 4 for org (4 u16)
                (
                    Expression::NumberLiteral(step * 2.0, Unit::None), // pos+size
                    Expression::NumberLiteral(step * 2.0, Unit::None), // pos+size
                    Expression::NumberLiteral(step * 4.0, Unit::None), // row+col+rowspan+colspan
                )
            };

            // Track the cumulative position (as an Expression) of each child in the
            // flattened stride. For static children the position increments by 1; for
            // inner repeaters it increments by the model length (dynamic).
            //
            // Each child's position in the stride determines where its data lives in
            // the coordinate/organized-data caches. We encode this via
            // inner_repeater_index in GridRepeaterCacheAccess:
            //   data_idx = data_start + row_idx * stride + child_offset + inner_rep_idx * epi
            // Using child_offset=0 (for pos) / 1 (for size) and
            // inner_rep_idx = cumulative_position (+ model_index for inner items).
            let mut cumulative_pos: Option<Expression> = None;

            for child in children_ref.iter() {
                let is_nested_repeater = child.borrow().repeated.is_some();
                let sub_item = create_layout_item(child, diag);

                // Read colspan and rowspan from the child element
                let propref = |name: &'static str, elem: &ElementRc| -> Option<RowColExpr> {
                    let nr = crate::layout::binding_reference(elem, name).map(|nr| {
                        let e = nr.element();
                        let mut nr = nr.clone();
                        if e.borrow().repeated.is_some()
                            && let crate::langtype::ElementType::Component(c) =
                                e.borrow().base_type.clone()
                        {
                            nr = NamedReference::new(&c.root_element, nr.name().clone())
                        };
                        nr
                    });
                    nr.map(RowColExpr::Named)
                };
                let colspan_expr = propref("colspan", child);
                let rowspan_expr = propref("rowspan", child);
                let child_grid_cell = Rc::new(RefCell::new(GridLayoutCell {
                    new_row: false,
                    col_expr: RowColExpr::Auto,
                    row_expr: RowColExpr::Auto,
                    colspan_expr: colspan_expr.unwrap_or(RowColExpr::Literal(1)),
                    rowspan_expr: rowspan_expr.unwrap_or(RowColExpr::Literal(1)),
                    child_items: None,
                }));
                // Attach to the element the solver reads: the sub-component root for an inner
                // repeater (so it reports its own colspan/rowspan), or `child` for a static child.
                sub_item.elem.borrow_mut().grid_layout_cell = Some(child_grid_cell);

                // Compute the effective inner_rep_idx for this child:
                // - For inner repeater items: cumulative_pos + model_index
                // - For static children: cumulative_pos (their fixed position in stride)
                // When cumulative_pos is None (= 0), we simplify to avoid unnecessary
                // BinaryExpression nodes.
                let effective_inner_rep_idx = if is_nested_repeater {
                    // Inner repeater: position = cumulative_pos + model_index
                    let model_idx = sub_item.repeater_index.clone().unwrap();
                    Some(if let Some(ref base) = cumulative_pos {
                        Expression::BinaryExpression {
                            lhs: Box::new(base.clone()),
                            rhs: Box::new(model_idx),
                            op: '+',
                        }
                    } else {
                        model_idx
                    })
                } else {
                    // Static child: position = cumulative_pos
                    cumulative_pos.clone()
                };

                let repeater_params = RepeaterCacheParams {
                    index: jump_pos,
                    rep_idx: &layout_item.repeater_index,
                    child_offset: 0,
                    inner_rep_idx: &effective_inner_rep_idx,
                };
                // The layout engine will set x,y,width,height for each of the repeated children
                set_coord_prop_from_cache(
                    &sub_item.elem,
                    &sub_item.item.constraints,
                    layout_cache_prop_h,
                    layout_cache_prop_v,
                    &repeater_params,
                    Some(&stride_h_expr),
                    Some(&stride_v_expr),
                    diag,
                );
                // ... and their row and col properties
                set_grid_rowcol_from_cache(
                    &sub_item.elem,
                    organized_data_prop,
                    &repeater_params,
                    Some(&stride_org_expr),
                    (&None::<RowColExpr>, &None::<RowColExpr>),
                    diag,
                );

                // Update cumulative position for the next child
                if is_nested_repeater {
                    // Inner repeater: adds model.length() items to the position.
                    // For a conditional `if cond: element`, the model is a boolean expression,
                    // so the length is `cond ? 1 : 0`, not `ArrayLength(cond)`.
                    let (model_expr, is_conditional) = {
                        let b = child.borrow();
                        let r = b.repeated.as_ref().unwrap();
                        (r.model.clone(), r.is_conditional_element)
                    };
                    let len_expr = if is_conditional {
                        Expression::Condition {
                            condition: Box::new(model_expr),
                            true_expr: Box::new(Expression::NumberLiteral(1., Unit::None)),
                            false_expr: Box::new(Expression::NumberLiteral(0., Unit::None)),
                        }
                    } else {
                        Expression::FunctionCall {
                            function: Callable::Builtin(BuiltinFunction::ArrayLength),
                            arguments: vec![model_expr],
                            source_location: None,
                        }
                    };
                    cumulative_pos = Some(if let Some(prev) = cumulative_pos.take() {
                        Expression::BinaryExpression {
                            lhs: Box::new(prev),
                            rhs: Box::new(len_expr),
                            op: '+',
                        }
                    } else {
                        len_expr
                    });
                } else {
                    // Static child: adds 1 to the position
                    cumulative_pos = Some(if let Some(prev) = cumulative_pos.take() {
                        Expression::BinaryExpression {
                            lhs: Box::new(prev),
                            rhs: Box::new(Expression::NumberLiteral(1., Unit::None)),
                            op: '+',
                        }
                    } else {
                        Expression::NumberLiteral(1., Unit::None)
                    });
                }

                if is_nested_repeater {
                    children_layout_items.push(RowChildTemplate::Repeated {
                        item: sub_item.item,
                        repeated_element: child.clone(),
                    });
                } else {
                    children_layout_items.push(RowChildTemplate::Static(sub_item.item));
                }
            }

            // 1 jump cell per repeater
            *num_cached_items += 1;
            // Add a single GridLayoutElement for the repeated Row
            let grid_layout_cell = Rc::new(RefCell::new(GridLayoutCell {
                new_row: true,
                col_expr: RowColExpr::Auto,
                row_expr: RowColExpr::Auto,
                colspan_expr: RowColExpr::Literal(1),
                rowspan_expr: RowColExpr::Literal(1),
                child_items: Some(children_layout_items),
            }));
            let grid_layout_element = GridLayoutElement {
                cell: grid_layout_cell.clone(),
                item: layout_item.item.clone(),
            };
            comp.root_element.borrow_mut().grid_layout_cell = Some(grid_layout_cell);
            self.elems.push(grid_layout_element);
        }
    }

    fn add_element_with_coord_as_expr(
        &mut self,
        item_element: &ElementRc,
        new_row: bool,
        (row_expr, col_expr): (&Option<RowColExpr>, &Option<RowColExpr>),
        (rowspan_expr, colspan_expr): (&Option<RowColExpr>, &Option<RowColExpr>),
        layout_cache_prop_h: &NamedReference,
        layout_cache_prop_v: &NamedReference,
        organized_data_prop: &NamedReference,
        diag: &mut BuildDiagnostics,
        num_cached_items: &mut usize,
    ) {
        let layout_item = create_layout_item(item_element, diag);

        let has_repeater_indirection = layout_item.repeater_index.is_some();
        // For repeated single elements: stride=2 for coord, stride=4 for org
        let stride_coord =
            has_repeater_indirection.then(|| Expression::NumberLiteral(2.0, Unit::None));
        let stride_org =
            has_repeater_indirection.then(|| Expression::NumberLiteral(4.0, Unit::None));
        let repeater_params = RepeaterCacheParams {
            index: *num_cached_items,
            rep_idx: &layout_item.repeater_index,
            child_offset: 0,
            inner_rep_idx: &None,
        };
        set_coord_prop_from_cache(
            &layout_item.elem,
            &layout_item.item.constraints,
            layout_cache_prop_h,
            layout_cache_prop_v,
            &repeater_params,
            stride_coord.as_ref(),
            stride_coord.as_ref(),
            diag,
        );
        set_grid_rowcol_from_cache(
            &layout_item.elem,
            organized_data_prop,
            &repeater_params,
            stride_org.as_ref(),
            (row_expr, col_expr),
            diag,
        );

        let expr_or_default = |expr: &Option<RowColExpr>, default: RowColExpr| -> RowColExpr {
            match expr {
                Some(RowColExpr::Literal(v)) => RowColExpr::Literal(*v),
                Some(RowColExpr::Named(nr)) => RowColExpr::Named(nr.clone()),
                Some(RowColExpr::Auto) => RowColExpr::Auto,
                None => default,
            }
        };

        let grid_layout_cell = Rc::new(RefCell::new(GridLayoutCell {
            new_row,
            col_expr: expr_or_default(col_expr, RowColExpr::Auto),
            row_expr: expr_or_default(row_expr, RowColExpr::Auto),
            colspan_expr: expr_or_default(colspan_expr, RowColExpr::Literal(1)),
            rowspan_expr: expr_or_default(rowspan_expr, RowColExpr::Literal(1)),
            child_items: None,
        }));
        let grid_layout_element =
            GridLayoutElement { cell: grid_layout_cell.clone(), item: layout_item.item.clone() };
        layout_item.elem.borrow_mut().grid_layout_cell = Some(grid_layout_cell);
        self.elems.push(grid_layout_element);
        *num_cached_items += 1;
    }
}

/// Solve box layouts with a single non-repeated cell and constant alignment at
/// compile time: closed-form bindings replace the layout-cache property, the
/// runtime solver, and the layout info computation.
///
/// Runs after the default_geometry pass so that every cell's layout info
/// property exists.
pub fn optimize_single_cell_layouts(component: &Rc<Component>) {
    recurse_elem_including_sub_components(component, &(), &mut |elem, _| {
        // Collect first: the rewrite modifies the bindings.
        let solves = elem
            .borrow()
            .bindings
            .iter()
            .filter_map(|(name, b)| match &b.borrow().expression {
                Expression::SolveBoxLayout(l, o) if *o == l.orientation && l.elems.len() == 1 => {
                    Some((name.clone(), l.clone()))
                }
                _ => None,
            })
            .collect::<Vec<_>>();
        for (cache_name, layout) in solves {
            optimize_single_cell_layout(elem, &cache_name, &layout);
        }
    });
}

fn optimize_single_cell_layout(
    layout_element: &ElementRc,
    cache_name: &SmolStr,
    layout: &BoxLayout,
) {
    let Some(single_cell) = single_cell_box_layout(layout) else { return };
    let orientation = layout.orientation;
    let cell = &layout.elems[0].element;
    let (pos, size) = match orientation {
        Orientation::Horizontal => ("x", "width"),
        Orientation::Vertical => ("y", "height"),
    };
    let replace = |prop: &str, expr: Expression| {
        let elem = cell.borrow();
        let mut binding = elem.binding_mut(prop).expect("the layout has set the cell's geometry");
        let expression = &mut binding.expression;
        debug_assert!(matches!(
            expression.ignore_debug_hooks(),
            Expression::LayoutCacheAccess { .. }
        ));

        *expression.ignore_debug_hooks_mut() = expr;
    };
    let pads = layout.geometry.padding.begin_end(orientation);
    let available = || size_minus_padding(layout_element, size, pads);
    let mut pos_expr = pads.0.map_or(Expression::NumberLiteral(0., Unit::Px), |nr| {
        Expression::PropertyReference(nr.clone())
    });
    if single_cell.pos_factor != 0. {
        // Read the size through the cell's property so the size expression
        // isn't duplicated.
        let cell_size =
            Expression::PropertyReference(NamedReference::new(cell, SmolStr::new_static(size)));
        let leftover = min_max(
            MinMaxOp::Max,
            Expression::NumberLiteral(0., Unit::Px),
            bin('-', available(), cell_size),
        );
        let factor = Expression::NumberLiteral(single_cell.pos_factor, Unit::None);
        pos_expr = bin('+', pos_expr, bin('*', leftover, factor));
    }
    replace(pos, pos_expr);
    if let Some((min_expr, max_expr, pref_expr)) = &single_cell.size {
        let mut size_expr = available();
        if let Some(pref) = pref_expr {
            size_expr = min_max(MinMaxOp::Min, size_expr, pref.clone());
        }
        // Clamp like the runtime solver: the minimum wins over the maximum.
        size_expr = min_max(
            MinMaxOp::Max,
            min_max(MinMaxOp::Min, size_expr, max_expr.clone()),
            min_expr.clone(),
        );
        replace(size, size_expr);
    }
    layout_element.borrow_mut().bindings.remove(cache_name);
    layout_element.borrow_mut().property_declarations.remove(cache_name);
    for o in [Orientation::Horizontal, Orientation::Vertical] {
        let Some(nr) = layout_element.borrow().layout_info_prop(o).cloned() else { continue };
        let Some(info) =
            single_cell_layout_info_binding(layout, &layout.elems[0], o, single_cell.stretch)
        else {
            continue;
        };
        if let Some(binding) = nr.element().borrow().bindings.get(nr.name()) {
            binding.borrow_mut().expression = info;
        }
    }
}

/// Compile-time solution for a box layout with a single non-repeated cell and
/// constant alignment: `size = clamp(min(available, preferred), min, max)`
/// (no preferred term when stretching), `pos = padding + pos_factor * leftover`.
struct SingleCellBoxLayout {
    /// Whether the (constant) alignment is the default stretch.
    stretch: bool,
    /// Fraction of the leftover space placed before the cell:
    /// 0 for stretch/start/space-between, ½ for center/space-around/space-evenly, 1 for end.
    pos_factor: f64,
    /// The `(min, max, preferred)` expressions clamping the size; the preferred
    /// term is `None` when stretching. The whole option is `None` when the size
    /// is fixed by an explicit binding that stays in place.
    size: Option<(Expression, Expression, Option<Expression>)>,
}

fn single_cell_box_layout(layout: &BoxLayout) -> Option<SingleCellBoxLayout> {
    let orientation = layout.orientation;
    let [item] = layout.elems.as_slice() else { return None };
    if item.element.borrow().repeated.is_some() {
        return None;
    }
    // Cells with a cross-axis-parametrized layout info need the runtime cells machinery.
    if orientation == Orientation::Horizontal
        && item.element.borrow().inherited_layout_info_h_with_constraint().is_some()
    {
        return None;
    }
    // The alignment must be a compile-time constant. A state-dependent
    // alignment was already rewritten into a condition by the lower_states pass.
    let alignment = match &layout.geometry.alignment {
        None => None,
        Some(nr) => {
            let elem = nr.element();
            let elem = elem.borrow();
            let analysis = elem.property_analysis.borrow();
            if analysis.get(nr.name()).is_some_and(|a| a.is_set || a.is_set_externally) {
                return None;
            }
            let binding = elem.bindings.get(nr.name())?;
            let binding = binding.borrow();
            if !binding.two_way_bindings.is_empty() {
                return None;
            }
            let Expression::EnumerationValue(ev) = binding.expression.ignore_debug_hooks() else {
                return None;
            };
            Some(ev.enumeration.values[ev.value].clone())
        }
    };
    let (stretch, pos_factor) = match alignment.as_deref() {
        None | Some("stretch") => (true, 0.),
        Some("start" | "space-between") => (false, 0.),
        Some("center" | "space-around" | "space-evenly") => (false, 0.5),
        Some("end") => (false, 1.),
        _ => return None,
    };
    let c = item.constraints.for_orientation(orientation);
    // Percent constraints scale with the available size; leave them to the solver.
    if [c.min, c.max, c.preferred].into_iter().flatten().any(|nr| nr.ty() == Type::Percent) {
        return None;
    }
    if c.fixed {
        return Some(SingleCellBoxLayout { stretch, pos_factor, size: None });
    }
    let implicit = cell_implicit_info(&item.element, orientation);
    let side = |explicit: &Option<NamedReference>, name: &str| {
        explicit
            .clone()
            .map(Expression::PropertyReference)
            .or_else(|| implicit.as_ref().and_then(|info| implicit_info_field(info, name)))
    };
    let pref = if stretch { None } else { Some(side(c.preferred, "preferred")?) };
    Some(SingleCellBoxLayout {
        stretch,
        pos_factor,
        size: Some((side(c.min, "min")?, side(c.max, "max")?, pref)),
    })
}

fn bin(op: char, lhs: Expression, rhs: Expression) -> Expression {
    Expression::BinaryExpression { lhs: Box::new(lhs), rhs: Box::new(rhs), op }
}

fn min_max(op: MinMaxOp, lhs: Expression, rhs: Expression) -> Expression {
    crate::builtin_macros::min_max_expression(lhs, rhs, op)
}

fn size_minus_padding(
    layout_element: &ElementRc,
    size_prop: &'static str,
    (begin_padding, end_padding): (Option<&NamedReference>, Option<&NamedReference>),
) -> Expression {
    let mut e = Expression::PropertyReference(NamedReference::new(
        layout_element,
        SmolStr::new_static(size_prop),
    ));
    for p in [begin_padding, end_padding].into_iter().flatten() {
        e = bin('-', e, Expression::PropertyReference(p.clone()));
    }
    e
}

/// Closed-form `layoutinfo-{h,v}` binding for a single-cell box layout: what
/// `box_layout_info` / `box_layout_info_ortho` compute at runtime. `None` when
/// a needed side of the cell's implicit layout info requires a runtime call.
fn single_cell_layout_info_binding(
    layout: &BoxLayout,
    item: &LayoutItem,
    o: Orientation,
    stretch: bool,
) -> Option<Expression> {
    let c = item.constraints.for_orientation(o);
    let size_prop = match o {
        Orientation::Horizontal => "width",
        Orientation::Vertical => "height",
    };
    let info_ty = crate::typeregister::layout_info_type();
    // Literal fields for built-ins with static info; otherwise a local holding
    // the cell's layout info property, stored only when a field is read.
    enum Implicit {
        Literal(std::collections::BTreeMap<SmolStr, Expression>),
        Prop(Expression),
        Expensive,
    }
    let implicit = match cell_implicit_info(&item.element, o) {
        Some(Expression::Struct { values, .. }) => Implicit::Literal(values),
        Some(base @ Expression::PropertyReference(_)) => Implicit::Prop(base),
        _ => Implicit::Expensive,
    };
    let mut implicit_used = false;
    let mut implicit_field = |name: &str| match &implicit {
        Implicit::Literal(values) => values.get(name).cloned(),
        Implicit::Prop(_) => {
            implicit_used = true;
            Some(Expression::StructFieldAccess {
                base: Box::new(Expression::ReadLocalVariable {
                    name: "cell_layout_info".into(),
                    ty: info_ty.clone().into(),
                }),
                name: name.into(),
            })
        }
        Implicit::Expensive => None,
    };
    // Percent constraints don't restrict the reported layout info.
    let explicit = |nr: &Option<NamedReference>| {
        nr.clone().filter(|nr| nr.ty() != Type::Percent).map(Expression::PropertyReference)
    };
    let (cell_min, cell_max, cell_pref) = if c.fixed {
        let sz =
            Expression::ReadLocalVariable { name: "cell_size".into(), ty: Type::LogicalLength };
        (sz.clone(), sz.clone(), sz)
    } else {
        (
            explicit(c.min).or_else(|| implicit_field("min"))?,
            explicit(c.max).or_else(|| implicit_field("max"))?,
            explicit(c.preferred).or_else(|| implicit_field("preferred"))?,
        )
    };
    let cell_stretch = c
        .stretch
        .clone()
        .map(Expression::PropertyReference)
        .or_else(|| implicit_field("stretch"))
        .or_else(|| static_native_stretch(&item.element))?;

    let mut prelude = Vec::new();
    if implicit_used && let Implicit::Prop(base) = implicit {
        prelude.push(Expression::StoreLocalVariable {
            name: "cell_layout_info".into(),
            value: Box::new(base),
        });
    }
    if c.fixed {
        prelude.push(Expression::StoreLocalVariable {
            name: "cell_size".into(),
            value: Box::new(Expression::PropertyReference(NamedReference::new(
                &item.element,
                SmolStr::new_static(size_prop),
            ))),
        });
    }
    let (pad_begin, pad_end) = layout.geometry.padding.begin_end(o);
    let pad_sum = [pad_begin, pad_end]
        .into_iter()
        .flatten()
        .map(|nr| Expression::PropertyReference(nr.clone()))
        .reduce(|lhs, rhs| bin('+', lhs, rhs));
    if let Some(pad_sum) = pad_sum.clone() {
        prelude.push(Expression::StoreLocalVariable {
            name: "layout_padding".into(),
            value: Box::new(pad_sum),
        });
    }
    let plus_pads = |e: Expression| {
        if pad_sum.is_none() {
            e
        } else {
            let pads = Expression::ReadLocalVariable {
                name: "layout_padding".into(),
                ty: Type::LogicalLength,
            };
            bin('+', e, pads)
        }
    };
    let (min, max, preferred) = if o == layout.orientation {
        let min = plus_pads(cell_min.clone());
        let max = if stretch {
            min_max(MinMaxOp::Max, plus_pads(cell_max.clone()), min.clone())
        } else {
            Expression::NumberLiteral(f32::MAX as f64, Unit::Px)
        };
        let pref = plus_pads(min_max(
            MinMaxOp::Max,
            min_max(MinMaxOp::Min, cell_pref, cell_max),
            cell_min,
        ));
        (min, max, pref)
    } else {
        let bounded_max = min_max(MinMaxOp::Max, cell_max, cell_min.clone());
        let pref = plus_pads(min_max(
            MinMaxOp::Max,
            min_max(MinMaxOp::Min, cell_pref, bounded_max.clone()),
            cell_min.clone(),
        ));
        (plus_pads(cell_min), plus_pads(bounded_max), pref)
    };
    let values = [
        ("min", min),
        ("max", max),
        ("min_percent", Expression::NumberLiteral(0., Unit::None)),
        ("max_percent", Expression::NumberLiteral(100., Unit::None)),
        ("preferred", preferred),
        ("stretch", cell_stretch),
    ]
    .into_iter()
    .map(|(name, e)| (SmolStr::new_static(name), e))
    .collect();
    let info = Expression::Struct { ty: info_ty, values };
    Some(if prelude.is_empty() {
        info
    } else {
        prelude.push(info);
        Expression::CodeBlock(prelude)
    })
}

/// The implicit layout info of a layout cell, resolved like `get_layout_info`
/// in the LLR lowering: the element's own layoutinfo property when it has one,
/// otherwise [`implicit_layout_info_call`].
fn cell_implicit_info(elem: &ElementRc, orientation: Orientation) -> Option<Expression> {
    let own_info = elem.borrow().layout_info_prop(orientation).cloned();
    match own_info {
        Some(nr) => Some(Expression::PropertyReference(nr)),
        None => implicit_layout_info_call(elem, orientation, BuiltinFilter::All, None),
    }
}

/// A cheap expression for one field of a [`cell_implicit_info`] result.
/// `None` when the info needs a runtime call.
fn implicit_info_field(info: &Expression, name: &str) -> Option<Expression> {
    match info {
        Expression::Struct { values, .. } => values.get(name).cloned(),
        base @ Expression::PropertyReference(_) => {
            Some(Expression::StructFieldAccess { base: Box::new(base.clone()), name: name.into() })
        }
        _ => None,
    }
}

/// Clamp a cross-axis stretch size to the cell's explicit min/max constraints,
/// like the runtime solver: percent constraints scale with the available size,
/// and the minimum wins over the maximum.
///
/// Implicit constraints are not consulted: an element's layout info may depend
/// on its own geometry (a percent spacing, for example), and reading it from
/// the geometry binding would create a binding loop.
fn clamp_cross_stretch_size(
    available: &Expression,
    c: &LayoutConstraints,
    ortho: Orientation,
) -> Expression {
    let c = c.for_orientation(ortho);
    let side = |explicit: &Option<NamedReference>| match explicit {
        Some(nr) if nr.ty() == Type::Percent => Some(bin(
            '/',
            bin('*', available.clone(), Expression::PropertyReference(nr.clone())),
            Expression::NumberLiteral(100., Unit::None),
        )),
        Some(nr) => Some(Expression::PropertyReference(nr.clone())),
        None => None,
    };
    let mut size_expr = available.clone();
    if let Some(max_expr) = side(c.max) {
        size_expr = min_max(MinMaxOp::Min, size_expr, max_expr);
    }
    if let Some(min_expr) = side(c.min) {
        size_expr = min_max(MinMaxOp::Max, size_expr, min_expr);
    }
    size_expr
}

fn lower_box_layout(
    layout_element: &ElementRc,
    diag: &mut BuildDiagnostics,
    orientation: Orientation,
) {
    let mut layout = BoxLayout {
        orientation,
        elems: Default::default(),
        geometry: LayoutGeometry::new(layout_element),
        cross_alignment: binding_reference(layout_element, "cross-axis-alignment"),
    };

    let layout_cache_ortho_prop = layout.cross_alignment.is_some().then(|| {
        create_new_prop(
            layout_element,
            SmolStr::new_static("layout-cache-ortho"),
            Type::LayoutCache,
        )
    });
    let layout_info_prop_v = create_new_prop(
        layout_element,
        SmolStr::new_static("layoutinfo-v"),
        layout_info_type().into(),
    );
    let layout_info_prop_h = create_new_prop(
        layout_element,
        SmolStr::new_static("layoutinfo-h"),
        layout_info_type().into(),
    );

    let layout_children = std::mem::take(&mut layout_element.borrow_mut().children);

    let (pos, size, pad, ortho) = match orientation {
        Orientation::Horizontal => ("x", "width", "y", "height"),
        Orientation::Vertical => ("y", "height", "x", "width"),
    };

    let layout_cache_prop =
        create_new_prop(layout_element, SmolStr::new_static("layout-cache"), Type::LayoutCache);
    // Default stretch bindings, only used when there is no `cross-axis-alignment`.
    let stretch_bindings = layout_cache_ortho_prop.is_none().then(|| {
        let pads = layout.geometry.padding.begin_end(orientation.orthogonal());
        let pad_expr = pads.0.map(|nr| Expression::PropertyReference(nr.clone()));
        (pad_expr, size_minus_padding(layout_element, ortho, pads))
    });

    for layout_child in &layout_children {
        let item = create_layout_item(layout_child, diag);
        let index = layout.elems.len() * BOX_LAYOUT_CACHE_ENTRIES_PER_CELL;
        let rep_idx = &item.repeater_index;
        let (fixed_size, fixed_ortho) = match orientation {
            Orientation::Horizontal => {
                (item.item.constraints.fixed_width, item.item.constraints.fixed_height)
            }
            Orientation::Vertical => {
                (item.item.constraints.fixed_height, item.item.constraints.fixed_width)
            }
        };
        let actual_elem = &item.elem;
        let entries = BOX_LAYOUT_CACHE_ENTRIES_PER_CELL;
        set_prop_from_cache(actual_elem, pos, &layout_cache_prop, index, rep_idx, entries, diag);
        if !fixed_size {
            set_prop_from_cache(
                actual_elem,
                size,
                &layout_cache_prop,
                index + 1,
                rep_idx,
                entries,
                diag,
            );
        }
        if let Some(cache_ortho) = &layout_cache_ortho_prop {
            set_prop_from_cache(actual_elem, pad, cache_ortho, index, rep_idx, entries, diag);
            if !fixed_ortho {
                set_prop_from_cache(
                    actual_elem,
                    ortho,
                    cache_ortho,
                    index + 1,
                    rep_idx,
                    entries,
                    diag,
                );
            }
        } else {
            let (pad_expr, size_expr) = stretch_bindings.as_ref().unwrap();
            if let Some(pad_expr) = pad_expr {
                actual_elem
                    .borrow_mut()
                    .bindings
                    .insert(pad.into(), RefCell::new(pad_expr.clone().into()));
            }
            if !fixed_ortho {
                let clamped = clamp_cross_stretch_size(
                    size_expr,
                    &item.item.constraints,
                    orientation.orthogonal(),
                );
                actual_elem
                    .borrow_mut()
                    .bindings
                    .insert(ortho.into(), RefCell::new(clamped.into()));
            }
        }
        layout.elems.push(item.item);
    }
    layout_element.borrow_mut().children = layout_children;
    let span = layout_element.borrow().to_source_location();
    layout_cache_prop.element().borrow_mut().bindings.insert(
        layout_cache_prop.name().clone(),
        BindingExpression::new_with_span(
            Expression::SolveBoxLayout(layout.clone(), orientation),
            span.clone(),
        )
        .into(),
    );
    if let Some(cache_ortho) = &layout_cache_ortho_prop {
        cache_ortho.element().borrow_mut().bindings.insert(
            cache_ortho.name().clone(),
            BindingExpression::new_with_span(
                Expression::SolveBoxLayout(layout.clone(), orientation.orthogonal()),
                span.clone(),
            )
            .into(),
        );
    }
    layout_info_prop_h.element().borrow_mut().bindings.insert(
        layout_info_prop_h.name().clone(),
        BindingExpression::new_with_span(
            Expression::ComputeBoxLayoutInfo {
                layout: layout.clone(),
                orientation: Orientation::Horizontal,
                cross_axis_size: None,
            },
            span.clone(),
        )
        .into(),
    );
    layout_info_prop_v.element().borrow_mut().bindings.insert(
        layout_info_prop_v.name().clone(),
        BindingExpression::new_with_span(
            Expression::ComputeBoxLayoutInfo {
                layout: layout.clone(),
                orientation: Orientation::Vertical,
                cross_axis_size: None,
            },
            span,
        )
        .into(),
    );
    layout_element.borrow_mut().layout_info_prop = Some((layout_info_prop_h, layout_info_prop_v));
    for d in layout_element.borrow_mut().debug.iter_mut() {
        d.layout = Some(Layout::BoxLayout(layout.clone()));
    }
}

fn lower_flexbox_layout(layout_element: &ElementRc, diag: &mut BuildDiagnostics) {
    // Warn if alignment is set to stretch, which behaves like start in flexbox
    // (CSS spec: justify-content:stretch acts as flex-start for flex items)
    if let Some(binding) = layout_element.borrow().bindings.get("alignment") {
        let binding = binding.borrow();
        if matches!(binding.expression.ignore_debug_hooks(),
            Expression::EnumerationValue(v) if v.enumeration.name == "LayoutAlignment"
                && v.enumeration.values[v.value] == "stretch")
        {
            diag.push_warning(
                "alignment: stretch has no effect on FlexboxLayout".into(),
                &*binding,
            );
        }
    }

    let direction = crate::layout::binding_reference(layout_element, "flex-direction");
    let align_content = crate::layout::binding_reference(layout_element, "align-content");
    let cross_axis_alignment =
        crate::layout::binding_reference(layout_element, "cross-axis-alignment");
    let flex_wrap = crate::layout::binding_reference(layout_element, "flex-wrap");

    let mut layout = crate::layout::FlexboxLayout {
        elems: Default::default(),
        geometry: LayoutGeometry::new(layout_element),
        direction,
        align_content,
        cross_axis_alignment,
        flex_wrap,
    };

    // FlexboxLayout needs 4 values per item: x, y, width, height
    let layout_cache_prop =
        create_new_prop(layout_element, SmolStr::new_static("layout-cache"), Type::LayoutCache);
    let layout_info_prop_v = create_new_prop(
        layout_element,
        SmolStr::new_static("layoutinfo-v"),
        layout_info_type().into(),
    );
    let layout_info_prop_h = create_new_prop(
        layout_element,
        SmolStr::new_static("layoutinfo-h"),
        layout_info_type().into(),
    );

    let layout_children = std::mem::take(&mut layout_element.borrow_mut().children);

    for layout_child in &layout_children {
        let item = create_layout_item(layout_child, diag);
        let index = layout.elems.len() * 4; // 4 values per item: x, y, width, height
        let rep_idx = &item.repeater_index;
        let actual_elem = &item.elem;
        actual_elem.borrow_mut().child_of_flexbox = true;

        // Set x from cache[index]
        set_prop_from_cache(actual_elem, "x", &layout_cache_prop, index, rep_idx, 4, diag);
        // Set y from cache[index + 1]
        set_prop_from_cache(actual_elem, "y", &layout_cache_prop, index + 1, rep_idx, 4, diag);
        // Set width from cache[index + 2] if not fixed
        if !item.item.constraints.fixed_width {
            set_prop_from_cache(
                actual_elem,
                "width",
                &layout_cache_prop,
                index + 2,
                rep_idx,
                4,
                diag,
            );
        }
        // Set height from cache[index + 3] if not fixed
        if !item.item.constraints.fixed_height {
            set_prop_from_cache(
                actual_elem,
                "height",
                &layout_cache_prop,
                index + 3,
                rep_idx,
                4,
                diag,
            );
        }
        let flex_grow = crate::layout::binding_reference(actual_elem, "flex-grow");
        let flex_shrink = crate::layout::binding_reference(actual_elem, "flex-shrink");
        let flex_basis = crate::layout::binding_reference(actual_elem, "flex-basis");
        let align_self = crate::layout::binding_reference(actual_elem, "flex-align-self");
        let order = crate::layout::binding_reference(actual_elem, "flex-order");
        layout.elems.push(crate::layout::FlexboxLayoutItem {
            item: item.item,
            flex_grow,
            flex_shrink,
            flex_basis,
            align_self,
            order,
        });
    }
    layout_element.borrow_mut().children = layout_children;
    let span = layout_element.borrow().to_source_location();

    layout_cache_prop.element().borrow_mut().bindings.insert(
        layout_cache_prop.name().clone(),
        BindingExpression::new_with_span(
            Expression::SolveFlexboxLayout(layout.clone()),
            span.clone(),
        )
        .into(),
    );
    layout_info_prop_h.element().borrow_mut().bindings.insert(
        layout_info_prop_h.name().clone(),
        BindingExpression::new_with_span(
            Expression::ComputeFlexboxLayoutInfo {
                layout: layout.clone(),
                orientation: Orientation::Horizontal,
                cross_axis_size: None,
            },
            span.clone(),
        )
        .into(),
    );
    layout_info_prop_v.element().borrow_mut().bindings.insert(
        layout_info_prop_v.name().clone(),
        BindingExpression::new_with_span(
            Expression::ComputeFlexboxLayoutInfo {
                layout: layout.clone(),
                orientation: Orientation::Vertical,
                cross_axis_size: None,
            },
            span,
        )
        .into(),
    );
    layout_element.borrow_mut().layout_info_prop = Some((layout_info_prop_h, layout_info_prop_v));
    for d in layout_element.borrow_mut().debug.iter_mut() {
        d.layout = Some(Layout::FlexboxLayout(layout.clone()));
    }
}

fn lower_dialog_layout(
    dialog_element: &ElementRc,
    style_metrics: &Rc<Component>,
    diag: &mut BuildDiagnostics,
) {
    let mut grid = GridLayout {
        elems: Default::default(),
        geometry: LayoutGeometry::new(dialog_element),
        dialog_button_roles: None,
        uses_auto: true,
    };
    let metrics = &style_metrics.root_element;
    grid.geometry
        .padding
        .bottom
        .get_or_insert(NamedReference::new(metrics, SmolStr::new_static("layout-padding")));
    grid.geometry
        .padding
        .top
        .get_or_insert(NamedReference::new(metrics, SmolStr::new_static("layout-padding")));
    grid.geometry
        .padding
        .left
        .get_or_insert(NamedReference::new(metrics, SmolStr::new_static("layout-padding")));
    grid.geometry
        .padding
        .right
        .get_or_insert(NamedReference::new(metrics, SmolStr::new_static("layout-padding")));
    grid.geometry
        .spacing
        .horizontal
        .get_or_insert(NamedReference::new(metrics, SmolStr::new_static("layout-spacing")));
    grid.geometry
        .spacing
        .vertical
        .get_or_insert(NamedReference::new(metrics, SmolStr::new_static("layout-spacing")));

    let layout_organized_data_prop = create_new_prop(
        dialog_element,
        SmolStr::new_static("layout-organized-data"),
        Type::ArrayOfU16,
    );
    let layout_cache_prop_h =
        create_new_prop(dialog_element, SmolStr::new_static("layout-cache-h"), Type::LayoutCache);
    let layout_cache_prop_v =
        create_new_prop(dialog_element, SmolStr::new_static("layout-cache-v"), Type::LayoutCache);
    let layout_info_prop_h = create_new_prop(
        dialog_element,
        SmolStr::new_static("layoutinfo-h"),
        layout_info_type().into(),
    );
    let layout_info_prop_v = create_new_prop(
        dialog_element,
        SmolStr::new_static("layoutinfo-v"),
        layout_info_type().into(),
    );

    let mut main_widget = None;
    let mut button_roles = Vec::new();
    let mut seen_buttons = HashSet::new();
    let mut num_cached_items: usize = 0;
    let layout_children = std::mem::take(&mut dialog_element.borrow_mut().children);
    for layout_child in &layout_children {
        let dialog_button_role_binding =
            layout_child.borrow_mut().bindings.remove("dialog-button-role");
        let is_button = if let Some(role_binding) = dialog_button_role_binding {
            let role_binding = role_binding.into_inner();
            if let Expression::EnumerationValue(val) =
                super::ignore_debug_hooks(&role_binding.expression)
            {
                let en = &val.enumeration;
                debug_assert_eq!(en.name, "DialogButtonRole");
                button_roles.push(en.values[val.value].clone());
                if val.value == 0 {
                    diag.push_error(
                        "The `dialog-button-role` cannot be set explicitly to none".into(),
                        &role_binding,
                    );
                }
            } else {
                diag.push_error(
                    "The `dialog-button-role` property must be known at compile-time".into(),
                    &role_binding,
                );
            }
            true
        } else if matches!(&layout_child.borrow().lookup_property("kind").property_type, Type::Enumeration(e) if e.name == "StandardButtonKind")
        {
            // layout_child is a StandardButton
            match layout_child.borrow().bindings.get("kind") {
                None => diag.push_error(
                    "The `kind` property of the StandardButton in a Dialog must be set".into(),
                    &*layout_child.borrow(),
                ),
                Some(binding) => {
                    let binding = &*binding.borrow();
                    if let Expression::EnumerationValue(val) =
                        super::ignore_debug_hooks(&binding.expression)
                    {
                        let en = &val.enumeration;
                        debug_assert_eq!(en.name, "StandardButtonKind");
                        let kind = &en.values[val.value];
                        let role = match kind.as_str() {
                            "ok" => "accept",
                            "cancel" => "reject",
                            "apply" => "apply",
                            "close" => "reject",
                            "reset" => "reset",
                            "help" => "help",
                            "yes" => "accept",
                            "no" => "reject",
                            "abort" => "reject",
                            "retry" => "accept",
                            "ignore" => "accept",
                            _ => unreachable!(),
                        };
                        button_roles.push(role.into());
                        if !seen_buttons.insert(val.value) {
                            diag.push_error("Duplicated `kind`: There are two StandardButton in this Dialog with the same kind".into(), binding);
                        } else if Rc::ptr_eq(
                            dialog_element,
                            &dialog_element
                                .borrow()
                                .enclosing_component
                                .upgrade()
                                .unwrap()
                                .root_element,
                        ) {
                            let clicked_ty =
                                layout_child.borrow().lookup_property("clicked").property_type;
                            if matches!(&clicked_ty, Type::Callback { .. })
                                && layout_child.borrow().bindings.get("clicked").is_none_or(|c| {
                                    matches!(c.borrow().expression, Expression::Invalid)
                                })
                            {
                                dialog_element
                                    .borrow_mut()
                                    .property_declarations
                                    .entry(format_smolstr!("{}-clicked", kind))
                                    .or_insert_with(|| PropertyDeclaration {
                                        property_type: clicked_ty,
                                        node: None,
                                        expose_in_public_api: true,
                                        is_alias: Some(NamedReference::new(
                                            layout_child,
                                            SmolStr::new_static("clicked"),
                                        )),
                                        visibility: PropertyVisibility::InOut,
                                        pure: None,
                                        shadows_builtin: false,
                                        deprecated: None,
                                    });
                            }
                        }
                    } else {
                        diag.push_error(
                            "The `kind` property of the StandardButton in a Dialog must be known at compile-time"
                                .into(),
                            binding,
                        );
                    }
                }
            }
            true
        } else {
            false
        };

        if is_button {
            grid.add_element_with_coord(
                layout_child,
                (1, button_roles.len() as u16),
                (1, 1),
                &layout_cache_prop_h,
                &layout_cache_prop_v,
                &layout_organized_data_prop,
                diag,
                &mut num_cached_items,
            );
        } else if main_widget.is_some() {
            diag.push_error(
                "A Dialog can have only one child element that is not a StandardButton".into(),
                &*layout_child.borrow(),
            );
        } else {
            main_widget = Some(layout_child.clone())
        }
    }
    dialog_element.borrow_mut().children = layout_children;

    if let Some(main_widget) = main_widget {
        grid.add_element_with_coord(
            &main_widget,
            (0, 0),
            (1, button_roles.len() as u16 + 1),
            &layout_cache_prop_h,
            &layout_cache_prop_v,
            &layout_organized_data_prop,
            diag,
            &mut num_cached_items,
        );
    } else {
        diag.push_error(
            "A Dialog must have a single child element that is not StandardButton".into(),
            &*dialog_element.borrow(),
        );
    }
    grid.dialog_button_roles = Some(button_roles);

    let span = dialog_element.borrow().to_source_location();
    layout_organized_data_prop.element().borrow_mut().bindings.insert(
        layout_organized_data_prop.name().clone(),
        BindingExpression::new_with_span(
            Expression::OrganizeGridLayout(grid.clone()),
            span.clone(),
        )
        .into(),
    );
    layout_cache_prop_h.element().borrow_mut().bindings.insert(
        layout_cache_prop_h.name().clone(),
        BindingExpression::new_with_span(
            Expression::SolveGridLayout {
                layout_organized_data_prop: layout_organized_data_prop.clone(),
                layout: grid.clone(),
                orientation: Orientation::Horizontal,
            },
            span.clone(),
        )
        .into(),
    );
    layout_cache_prop_v.element().borrow_mut().bindings.insert(
        layout_cache_prop_v.name().clone(),
        BindingExpression::new_with_span(
            Expression::SolveGridLayout {
                layout_organized_data_prop: layout_organized_data_prop.clone(),
                layout: grid.clone(),
                orientation: Orientation::Vertical,
            },
            span.clone(),
        )
        .into(),
    );
    layout_info_prop_h.element().borrow_mut().bindings.insert(
        layout_info_prop_h.name().clone(),
        BindingExpression::new_with_span(
            Expression::ComputeGridLayoutInfo {
                layout_organized_data_prop: layout_organized_data_prop.clone(),
                layout: grid.clone(),
                orientation: Orientation::Horizontal,
                cross_axis_size: None,
            },
            span.clone(),
        )
        .into(),
    );
    layout_info_prop_v.element().borrow_mut().bindings.insert(
        layout_info_prop_v.name().clone(),
        BindingExpression::new_with_span(
            Expression::ComputeGridLayoutInfo {
                layout_organized_data_prop: layout_organized_data_prop.clone(),
                layout: grid.clone(),
                orientation: Orientation::Vertical,
                cross_axis_size: None,
            },
            span,
        )
        .into(),
    );
    dialog_element.borrow_mut().layout_info_prop = Some((layout_info_prop_h, layout_info_prop_v));
    for d in dialog_element.borrow_mut().debug.iter_mut() {
        d.layout = Some(Layout::GridLayout(grid.clone()));
    }
}

struct CreateLayoutItemResult {
    item: LayoutItem,
    elem: ElementRc,
    repeater_index: Option<Expression>,
}

/// Create a LayoutItem for the given `item_element`  returns None is the layout is empty
fn create_layout_item(
    item_element: &ElementRc,
    diag: &mut BuildDiagnostics,
) -> CreateLayoutItemResult {
    let fix_explicit_percent = |prop: &str, item: &ElementRc| {
        if !item.borrow().bindings.get(prop).is_some_and(|b| b.borrow().ty() == Type::Percent) {
            return;
        }
        let min_name = format_smolstr!("min-{}", prop);
        let max_name = format_smolstr!("max-{}", prop);
        let mut min_ref = BindingExpression::from(Expression::PropertyReference(
            NamedReference::new(item, min_name.clone()),
        ));
        let mut item = item.borrow_mut();
        let b = item.bindings.remove(prop).unwrap().into_inner();
        min_ref.span = b.span.clone();
        min_ref.priority = b.priority;
        item.bindings.insert(max_name.clone(), min_ref.into());
        item.bindings.insert(min_name.clone(), b.into());
        item.property_declarations.insert(
            min_name,
            PropertyDeclaration { property_type: Type::Percent, ..PropertyDeclaration::default() },
        );
        item.property_declarations.insert(
            max_name,
            PropertyDeclaration { property_type: Type::Percent, ..PropertyDeclaration::default() },
        );
    };
    fix_explicit_percent("width", item_element);
    fix_explicit_percent("height", item_element);

    item_element.borrow_mut().child_of_layout = true;
    let (repeater_index, actual_elem) = if let Some(r) = &item_element.borrow().repeated {
        let rep_comp = item_element.borrow().base_type.as_component().clone();
        fix_explicit_percent("width", &rep_comp.root_element);
        fix_explicit_percent("height", &rep_comp.root_element);

        *rep_comp.root_constraints.borrow_mut() = LayoutConstraints::new(
            &rep_comp.root_element,
            Some((&mut *diag, DiagnosticLevel::Error)),
        );
        rep_comp.root_element.borrow_mut().child_of_layout = true;
        (
            Some(if r.is_conditional_element {
                Expression::NumberLiteral(0., Unit::None)
            } else {
                Expression::RepeaterIndexReference { element: Rc::downgrade(item_element) }
            }),
            rep_comp.root_element.clone(),
        )
    } else {
        (None, item_element.clone())
    };

    let constraints = LayoutConstraints::new(&actual_elem, Some((diag, DiagnosticLevel::Error)));
    CreateLayoutItemResult {
        item: LayoutItem { element: item_element.clone(), constraints },
        elem: actual_elem,
        repeater_index,
    }
}

fn set_grid_prop_from_cache(
    elem: &ElementRc,
    prop: &str,
    layout_cache_prop: &NamedReference,
    index: usize,
    repeater_index: &Option<Expression>,
    child_offset: usize,
    // If Some, use GridRepeaterCacheAccess (repeater indirection). None = LayoutCacheAccess.
    stride_expr: Option<&Expression>,
    inner_repeater_index: Option<Expression>,
    entries_per_item: usize,
    diag: &mut BuildDiagnostics,
) {
    if let Some(stride) = stride_expr {
        // Repeater indirection mode: cache[cache[index] + ri * stride + child_offset]
        let repeater_index_boxed = repeater_index.as_ref().map(|x| Box::new(x.clone()));
        let expr = Expression::GridRepeaterCacheAccess {
            layout_cache_prop: layout_cache_prop.clone(),
            index,
            repeater_index: repeater_index_boxed.unwrap(),
            stride: Box::new(stride.clone()),
            child_offset,
            inner_repeater_index: inner_repeater_index.map(Box::new),
            entries_per_item,
        };
        insert_cache_prop_binding(expr, elem, prop, layout_cache_prop, diag);
    } else {
        // Standard mode
        set_prop_from_cache(
            elem,
            prop,
            layout_cache_prop,
            index,
            repeater_index,
            entries_per_item,
            diag,
        );
    }
}

fn set_prop_from_cache(
    elem: &ElementRc,
    prop: &str,
    layout_cache_prop: &NamedReference,
    index: usize,
    repeater_index: &Option<Expression>,
    entries_per_item: usize,
    diag: &mut BuildDiagnostics,
) {
    let expr = Expression::LayoutCacheAccess {
        layout_cache_prop: layout_cache_prop.clone(),
        index,
        repeater_index: repeater_index.as_ref().map(|x| Box::new(x.clone())),
        entries_per_item,
    };
    insert_cache_prop_binding(expr, elem, prop, layout_cache_prop, diag);
}

fn insert_cache_prop_binding(
    expr: Expression,
    elem: &ElementRc,
    prop: &str,
    layout_cache_prop: &NamedReference,
    diag: &mut BuildDiagnostics,
) {
    let new_binding = BindingExpression::new_with_span(
        expr,
        layout_cache_prop.element().borrow().to_source_location(),
    );
    if let Some(old) = elem.borrow_mut().set_binding(prop.into(), new_binding) {
        diag.push_error(
            format!("The property '{prop}' cannot be set for elements placed in this layout, because the layout is already setting it"),
            &old,
        );
    }
}

/// Common cache-access parameters for repeater indirection in layout caches.
#[derive(Copy, Clone)]
struct RepeaterCacheParams<'a> {
    /// Logical index into the cache (base position for this item).
    index: usize,
    /// Repeater index expression (outer repeater iteration).
    rep_idx: &'a Option<Expression>,
    /// Offset for child items within a repeated row.
    child_offset: usize,
    /// Inner repeater index (for nested repeaters within repeated rows).
    inner_rep_idx: &'a Option<Expression>,
}

/// GridLayout: set properties (x, y, width, height) from the coordinate cache.
fn set_coord_prop_from_cache(
    elem: &ElementRc,
    constraints: &LayoutConstraints,
    layout_cache_prop_h: &NamedReference,
    layout_cache_prop_v: &NamedReference,
    repeater_params: &RepeaterCacheParams<'_>,
    stride_h: Option<&Expression>,
    stride_v: Option<&Expression>,
    diag: &mut BuildDiagnostics,
) {
    let has_repeater_indirection = stride_h.is_some();
    let cache_idx = repeater_params.index * 2;
    let pos_offset = repeater_params.child_offset;
    let size_offset = repeater_params.child_offset + 1;
    let inner_idx_clone = repeater_params.inner_rep_idx.clone();

    // In repeater indirection mode, width/height use the same cache_idx; in standard mode, they use cache_idx + 1
    let size_cache_idx = if has_repeater_indirection { cache_idx } else { cache_idx + 1 };

    set_grid_prop_from_cache(
        elem,
        "x",
        layout_cache_prop_h,
        cache_idx,
        repeater_params.rep_idx,
        pos_offset,
        stride_h,
        inner_idx_clone.clone(),
        2,
        diag,
    );
    if !constraints.fixed_width {
        set_grid_prop_from_cache(
            elem,
            "width",
            layout_cache_prop_h,
            size_cache_idx,
            repeater_params.rep_idx,
            size_offset,
            stride_h,
            inner_idx_clone.clone(),
            2,
            diag,
        );
    }
    set_grid_prop_from_cache(
        elem,
        "y",
        layout_cache_prop_v,
        cache_idx,
        repeater_params.rep_idx,
        pos_offset,
        stride_v,
        inner_idx_clone.clone(),
        2,
        diag,
    );
    if !constraints.fixed_height {
        set_grid_prop_from_cache(
            elem,
            "height",
            layout_cache_prop_v,
            size_cache_idx,
            repeater_params.rep_idx,
            size_offset,
            stride_v,
            inner_idx_clone,
            2,
            diag,
        );
    }
}

/// Set organized-data properties (col, row) from the organized data cache.
/// `stride`: Some = Repeater indirection mode. None = LayoutCacheAccess mode.
fn set_grid_rowcol_from_cache(
    elem: &ElementRc,
    organized_data_prop: &NamedReference,
    repeater_params: &RepeaterCacheParams<'_>,
    stride: Option<&Expression>,
    (row_expr, col_expr): (&Option<RowColExpr>, &Option<RowColExpr>),
    diag: &mut BuildDiagnostics,
) {
    let has_repeater_indirection = stride.is_some();
    let org_cache_idx = repeater_params.index * 4;

    // In repeater indirection mode, both col and row use the same cache_idx but different offsets
    // In standard mode, they use different cache_idx values with zero offsets
    let col_cache_idx = org_cache_idx;
    let col_offset = if has_repeater_indirection { repeater_params.child_offset * 4 } else { 0 };

    let (row_cache_idx, row_offset) = if has_repeater_indirection {
        (org_cache_idx, repeater_params.child_offset * 4 + 2)
    } else {
        (org_cache_idx + 2, 0)
    };

    if col_expr.is_none() {
        set_grid_prop_from_cache(
            elem,
            "col",
            organized_data_prop,
            col_cache_idx,
            repeater_params.rep_idx,
            col_offset,
            stride,
            repeater_params.inner_rep_idx.clone(),
            4,
            diag,
        );
    }
    if row_expr.is_none() {
        set_grid_prop_from_cache(
            elem,
            "row",
            organized_data_prop,
            row_cache_idx,
            repeater_params.rep_idx,
            row_offset,
            stride,
            repeater_params.inner_rep_idx.clone(),
            4,
            diag,
        );
    }
}

// If it's a number literal, it must be a positive integer
// But also allow any other kind of expression
// Returns true for literals, false for other kinds of expressions
fn check_number_literal_is_positive_integer(
    expression: &Expression,
    name: &str,
    span: &dyn crate::diagnostics::Spanned,
    diag: &mut BuildDiagnostics,
) -> bool {
    match super::ignore_debug_hooks(expression) {
        Expression::NumberLiteral(v, Unit::None) => {
            if *v > u16::MAX as f64 || !v.trunc().approx_eq(v) {
                diag.push_error(format!("'{name}' must be a positive integer"), span);
            }
            true
        }
        Expression::UnaryOp { op: '-', sub } => {
            if let Expression::NumberLiteral(_, Unit::None) = super::ignore_debug_hooks(sub) {
                diag.push_error(format!("'{name}' must be a positive integer"), span);
            }
            true
        }
        Expression::Cast { from, .. } => {
            check_number_literal_is_positive_integer(from, name, span, diag)
        }
        _ => false,
    }
}

fn recognized_layout_types() -> &'static [&'static str] {
    &["Row", "GridLayout", "HorizontalLayout", "VerticalLayout", "FlexboxLayout", "Dialog"]
}

/// Checks that there are no grid-layout specific properties used wrongly
fn check_no_layout_properties(
    item: &ElementRc,
    layout_type: &Option<SmolStr>,
    parent_layout_type: &Option<SmolStr>,
    diag: &mut BuildDiagnostics,
) {
    let elem = item.borrow();
    for (prop, expr) in elem.bindings.iter() {
        if !matches!(parent_layout_type.as_deref(), Some("GridLayout") | Some("Row"))
            && matches!(prop.as_ref(), "col" | "row" | "colspan" | "rowspan")
        {
            diag.push_error(format!("{prop} used outside of a GridLayout's cell"), &*expr.borrow());
        }
        if parent_layout_type.as_deref() != Some("FlexboxLayout")
            && matches!(
                prop.as_ref(),
                "flex-grow" | "flex-shrink" | "flex-basis" | "flex-align-self" | "flex-order"
            )
        {
            diag.push_error(format!("{prop} used outside of a FlexboxLayout"), &*expr.borrow());
        }
        if parent_layout_type.as_deref() != Some("Dialog")
            && matches!(prop.as_ref(), "dialog-button-role")
        {
            diag.push_error(
                format!("{prop} used outside of a Dialog's direct child"),
                &*expr.borrow(),
            );
        }
        if (layout_type.is_none()
            || !recognized_layout_types().contains(&layout_type.as_ref().unwrap().as_str()))
            && matches!(
                prop.as_ref(),
                "padding" | "padding-left" | "padding-right" | "padding-top" | "padding-bottom"
            )
            && !check_inherits_layout(item)
        {
            diag.push_warning(
                format!("{prop} only has effect on layout elements"),
                &*expr.borrow(),
            );
        }
    }

    /// Check if the element inherits from a layout that was lowered
    fn check_inherits_layout(item: &ElementRc) -> bool {
        if let ElementType::Component(c) = &item.borrow().base_type {
            c.root_element.borrow().debug.iter().any(|d| d.layout.is_some())
                || check_inherits_layout(&c.root_element)
        } else {
            false
        }
    }
}

/// For fixed layout, we need to dissociate the width and the height property of the WindowItem from width and height property
/// in slint such that the width and height property are actually constants.
///
/// The Slint runtime will change the width and height property of the native WindowItem to match those of the actual
/// window, but we don't want that to happen if we have a fixed layout.
pub fn check_window_layout(component: &Rc<Component>) {
    if component.root_constraints.borrow().fixed_height {
        adjust_window_layout(component, "height");
    }
    if component.root_constraints.borrow().fixed_width {
        adjust_window_layout(component, "width");
    }
}

pub fn check_popup_layout(component: &Rc<Component>) {
    component.popup_windows.borrow().iter().for_each(|p| {
        if p.component.root_constraints.borrow().fixed_height {
            adjust_window_layout(&p.component, "height");
        }

        if p.component.root_constraints.borrow().fixed_width {
            adjust_window_layout(&p.component, "width");
        }
    });
}

fn adjust_window_layout(component: &Rc<Component>, prop: &'static str) {
    let new_prop = crate::layout::create_new_prop(
        &component.root_element,
        format_smolstr!("fixed-{prop}"),
        Type::LogicalLength,
    );
    {
        let mut root = component.root_element.borrow_mut();
        if let Some(b) = root.bindings.remove(prop) {
            root.bindings.insert(new_prop.name().clone(), b);
        };
        let mut analysis = root.property_analysis.borrow_mut();
        if let Some(a) = analysis.remove(prop) {
            analysis.insert(new_prop.name().clone(), a);
        };
        drop(analysis);
        root.bindings.insert(
            prop.into(),
            RefCell::new(Expression::PropertyReference(new_prop.clone()).into()),
        );
    }

    let old_prop = NamedReference::new(&component.root_element, SmolStr::new_static(prop));
    crate::object_tree::visit_all_named_references(component, &mut |nr| {
        if nr == &old_prop {
            *nr = new_prop.clone()
        }
    });
}
