// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

// cSpell: ignore tabwidget

//! This pass lowers the TabWidget to create the tabbar.
//!
//! Must be done before inlining and many other passes because the lowered code must
//! be further inlined as it may expand to a native widget that needs inlining.
//!
//! Supports both static tabs (defined inline) and dynamic tabs (using `for` or `if`).

use crate::diagnostics::BuildDiagnostics;
use crate::expression_tree::{BindingExpression, Expression, MinMaxOp, NamedReference, Unit};
use crate::langtype::{ElementType, Type};
use crate::object_tree::*;
use smol_str::{SmolStr, format_smolstr};
use std::cell::RefCell;
use std::rc::Rc;

pub async fn lower_tabwidget(
    doc: &Document,
    type_loader: &mut crate::typeloader::TypeLoader,
    diag: &mut BuildDiagnostics,
) {
    // First check if any TabWidget is used - avoid loading std-widgets.slint if not needed
    let mut has_tabwidget = false;
    doc.visit_all_used_components(|component| {
        recurse_elem_including_sub_components_no_borrow(component, &(), &mut |elem, _| {
            if matches!(&elem.borrow().builtin_type(), Some(b) if b.name == "TabWidget") {
                has_tabwidget = true;
            }
        })
    });

    if !has_tabwidget {
        return;
    }

    // Ignore import errors
    let mut build_diags_to_ignore = BuildDiagnostics::default();
    let tabwidget_impl = type_loader
        .import_component("std-widgets.slint", "TabWidgetImpl", &mut build_diags_to_ignore)
        .await
        .expect("can't load TabWidgetImpl from std-widgets.slint");
    let tab_impl = type_loader
        .import_component("std-widgets.slint", "TabImpl", &mut build_diags_to_ignore)
        .await
        .expect("can't load TabImpl from std-widgets.slint");
    let tabbar_horizontal_impl = type_loader
        .import_component("std-widgets.slint", "TabBarHorizontalImpl", &mut build_diags_to_ignore)
        .await
        .expect("can't load TabBarHorizontalImpl from std-widgets.slint");
    let tabbar_vertical_impl = type_loader
        .import_component("std-widgets.slint", "TabBarVerticalImpl", &mut build_diags_to_ignore)
        .await
        .expect("can't load TabBarVerticalImpl from std-widgets.slint");
    let empty_type = type_loader.global_type_registry.borrow().empty_type();

    doc.visit_all_used_components(|component| {
        recurse_elem_including_sub_components_no_borrow(component, &(), &mut |elem, _| {
            if matches!(&elem.borrow().builtin_type(), Some(b) if b.name == "TabWidget") {
                process_tabwidget(
                    elem,
                    ElementType::Component(tabwidget_impl.clone()),
                    ElementType::Component(tab_impl.clone()),
                    ElementType::Component(tabbar_horizontal_impl.clone()),
                    ElementType::Component(tabbar_vertical_impl.clone()),
                    &empty_type,
                    diag,
                );
            }
        })
    });
}

/// Represents the contribution of a child to the tab count and its starting offset
struct TabChildInfo {
    /// Expression for the starting index of this child's tabs
    offset_expr: Expression,
}

/// Compute the offset expressions for each child and the total num-tabs expression.
/// Static tabs contribute 1, repeated tabs contribute the model length.
fn compute_tab_offsets(children: &[ElementRc]) -> (Vec<TabChildInfo>, Expression) {
    let mut infos = Vec::new();
    let mut cumulative: Option<Expression> = None;

    for child in children {
        let offset = cumulative.clone().unwrap_or(Expression::NumberLiteral(0., Unit::None));

        let count = if let Some(repeated) = &child.borrow().repeated {
            if repeated.is_conditional_element {
                // if condition: contributes 0 or 1
                Expression::Condition {
                    condition: Box::new(repeated.model.clone()),
                    true_expr: Box::new(Expression::NumberLiteral(1., Unit::None)),
                    false_expr: Box::new(Expression::NumberLiteral(0., Unit::None)),
                }
            } else {
                // for loop: contributes model.length()
                Expression::FunctionCall {
                    function: crate::expression_tree::Callable::Builtin(
                        crate::expression_tree::BuiltinFunction::ArrayLength,
                    ),
                    arguments: vec![repeated.model.clone()],
                    source_location: None,
                }
            }
        } else {
            // Static tab: contributes exactly 1
            Expression::NumberLiteral(1., Unit::None)
        };

        cumulative = Some(if let Some(prev) = cumulative {
            Expression::BinaryExpression {
                lhs: Box::new(prev),
                rhs: Box::new(count.clone()),
                op: '+',
            }
        } else {
            count.clone()
        });

        infos.push(TabChildInfo { offset_expr: offset });
    }

    let total = cumulative.unwrap_or(Expression::NumberLiteral(0., Unit::None));
    (infos, total)
}

/// Clone an expression, replacing any RepeaterModelReference or RepeaterIndexReference
/// that points to `from_elem` with one that points to `to_elem`.
/// This is used to duplicate a binding expression from a content repeater to a tabbar repeater.
fn remap_repeater_references(
    expr: &Expression,
    from_elem: &ElementRc,
    to_elem: &ElementRc,
) -> Expression {
    match expr {
        Expression::RepeaterModelReference { element }
            if element.upgrade().is_some_and(|e| Rc::ptr_eq(&e, from_elem)) =>
        {
            Expression::RepeaterModelReference { element: Rc::downgrade(to_elem) }
        }
        Expression::RepeaterIndexReference { element }
            if element.upgrade().is_some_and(|e| Rc::ptr_eq(&e, from_elem)) =>
        {
            Expression::RepeaterIndexReference { element: Rc::downgrade(to_elem) }
        }
        _ => {
            let mut cloned = expr.clone();
            cloned.visit_mut(|sub| {
                *sub = remap_repeater_references(sub, from_elem, to_elem);
            });
            cloned
        }
    }
}

fn process_tabwidget(
    elem: &ElementRc,
    tabwidget_impl: ElementType,
    tab_impl: ElementType,
    tabbar_horizontal_impl: ElementType,
    tabbar_vertical_impl: ElementType,
    empty_type: &ElementType,
    diag: &mut BuildDiagnostics,
) {
    if matches!(&elem.borrow_mut().base_type, ElementType::Builtin(_)) {
        // That's the TabWidget re-exported from the style, it doesn't need to be processed
        return;
    }

    elem.borrow_mut().base_type = tabwidget_impl;
    let mut children = std::mem::take(&mut elem.borrow_mut().children);

    // Validate that all children are Tabs
    children.retain(|child| {
        let base = child.borrow().base_type.to_string();
        if base != "Tab" {
            // If it has errors already, just skip
            if !diag.has_errors() {
                diag.push_error(
                    "Only Tab elements are allowed inside a TabWidget".into(),
                    &*child.borrow(),
                );
            }
            false
        } else {
            true
        }
    });

    let (tab_infos, num_tabs_expr) = compute_tab_offsets(&children);

    let mut tabs = Vec::new();

    for (child_idx, child) in children.iter_mut().enumerate() {
        let info = &tab_infos[child_idx];
        let is_repeated = child.borrow().repeated.is_some();

        // Transform the Tab into a content pane
        child.borrow_mut().base_type = empty_type.clone();
        child
            .borrow_mut()
            .property_declarations
            .insert(SmolStr::new_static("title"), Type::String.into());

        set_geometry_prop(elem, child, "x", diag);
        set_geometry_prop(elem, child, "y", diag);
        set_geometry_prop(elem, child, "width", diag);
        set_geometry_prop(elem, child, "height", diag);

        // Set visibility: current-index == (offset + repeater_index) for for-loops,
        // current-index == offset for static tabs and conditionals (which have at most 1 instance)
        let is_conditional =
            child.borrow().repeated.as_ref().is_some_and(|r| r.is_conditional_element);
        let index_expr = if is_repeated && !is_conditional {
            // For `for` repeated elements, the index within the repeater is available via
            // RepeaterIndexReference. The absolute index is offset + repeater_index.
            Expression::BinaryExpression {
                lhs: Box::new(info.offset_expr.clone()),
                rhs: Box::new(Expression::RepeaterIndexReference { element: Rc::downgrade(child) }),
                op: '+',
            }
        } else {
            // Static tabs and conditional tabs (which contribute at most 1 tab at offset)
            info.offset_expr.clone()
        };

        let condition = Expression::BinaryExpression {
            lhs: Expression::PropertyReference(NamedReference::new(
                elem,
                SmolStr::new_static("current-index"),
            ))
            .into(),
            rhs: Box::new(index_expr.clone()),
            op: '=',
        };
        let old = child
            .borrow_mut()
            .bindings
            .insert(SmolStr::new_static("visible"), RefCell::new(condition.into()));
        if let Some(old) = old {
            diag.push_error(
                "The property 'visible' cannot be set for Tabs inside a TabWidget".to_owned(),
                &old.into_inner(),
            );
        }

        let role = crate::typeregister::BUILTIN
            .with(|e| e.enums.AccessibleRole.clone())
            .try_value_from_string("tab-panel")
            .unwrap();
        let old = child.borrow_mut().bindings.insert(
            SmolStr::new_static("accessible-role"),
            RefCell::new(Expression::EnumerationValue(role).into()),
        );
        if let Some(old) = old {
            diag.push_error(
                "The property 'accessible-role' cannot be set for Tabs inside a TabWidget"
                    .to_owned(),
                &old.into_inner(),
            );
        }

        let title_ref = RefCell::new(
            Expression::PropertyReference(NamedReference::new(child, "title".into())).into(),
        );
        let old = child.borrow_mut().bindings.insert("accessible-label".into(), title_ref);
        if let Some(old) = old {
            diag.push_error(
                "The property 'accessible-label' cannot be set for Tabs inside a TabWidget"
                    .to_owned(),
                &old.into_inner(),
            );
        }

        // Create the corresponding tab bar item(s)
        if is_repeated && !is_conditional {
            // For `for` repeated tabs, create a repeated TabImpl element with the same model
            let repeated_info = child.borrow().repeated.as_ref().unwrap().clone();

            let tab = Element {
                id: format_smolstr!("{}-tab-repeated{}", elem.borrow().id, child_idx),
                base_type: tab_impl.clone(),
                enclosing_component: elem.borrow().enclosing_component.clone(),
                repeated: Some(RepeatedElementInfo {
                    model: repeated_info.model.clone(),
                    model_data_id: repeated_info.model_data_id.clone(),
                    index_id: repeated_info.index_id.clone(),
                    is_conditional_element: false,
                    is_listview: None,
                }),
                ..Default::default()
            };

            let tab_rc = Element::make_rc(tab);

            // tab-index = offset + repeater_index
            let tab_index_expr = Expression::BinaryExpression {
                lhs: Box::new(info.offset_expr.clone()),
                rhs: Box::new(Expression::RepeaterIndexReference {
                    element: Rc::downgrade(&tab_rc),
                }),
                op: '+',
            };

            // Clone the title binding from the content child and remap repeater
            // references to point to the tabbar tab element instead.
            let title_binding =
                child.borrow().bindings.get("title").map(|b| b.borrow().expression.clone());
            if let Some(title_expr) = title_binding {
                let remapped = remap_repeater_references(&title_expr, child, &tab_rc);
                tab_rc
                    .borrow_mut()
                    .bindings
                    .insert(SmolStr::new_static("title"), RefCell::new(remapped.into()));
            }
            tab_rc.borrow_mut().bindings.insert(
                SmolStr::new_static("current"),
                BindingExpression::new_two_way(
                    NamedReference::new(elem, SmolStr::new_static("current-index")).into(),
                )
                .into(),
            );
            tab_rc.borrow_mut().bindings.insert(
                SmolStr::new_static("current-focused"),
                BindingExpression::new_two_way(
                    NamedReference::new(elem, SmolStr::new_static("current-focused")).into(),
                )
                .into(),
            );
            tab_rc
                .borrow_mut()
                .bindings
                .insert(SmolStr::new_static("tab-index"), RefCell::new(tab_index_expr.into()));
            tab_rc.borrow_mut().bindings.insert(
                SmolStr::new_static("num-tabs"),
                RefCell::new(num_tabs_expr.clone().into()),
            );

            tabs.push(tab_rc);
        } else if is_conditional {
            // For conditional (`if`) tabs, create a conditional TabImpl element
            // with the same boolean model. The tab is at a fixed offset position.
            let repeated_info = child.borrow().repeated.as_ref().unwrap().clone();

            let tab = Element {
                id: format_smolstr!("{}-tab-cond{}", elem.borrow().id, child_idx),
                base_type: tab_impl.clone(),
                enclosing_component: elem.borrow().enclosing_component.clone(),
                repeated: Some(RepeatedElementInfo {
                    model: repeated_info.model.clone(),
                    model_data_id: repeated_info.model_data_id.clone(),
                    index_id: repeated_info.index_id.clone(),
                    is_conditional_element: true,
                    is_listview: None,
                }),
                ..Default::default()
            };

            let tab_rc = Element::make_rc(tab);

            // For a conditional, the title is a static expression (not model-dependent)
            let title_binding =
                child.borrow().bindings.get("title").map(|b| b.borrow().expression.clone());
            if let Some(title_expr) = title_binding {
                tab_rc
                    .borrow_mut()
                    .bindings
                    .insert(SmolStr::new_static("title"), RefCell::new(title_expr.into()));
            }
            tab_rc.borrow_mut().bindings.insert(
                SmolStr::new_static("current"),
                BindingExpression::new_two_way(
                    NamedReference::new(elem, SmolStr::new_static("current-index")).into(),
                )
                .into(),
            );
            tab_rc.borrow_mut().bindings.insert(
                SmolStr::new_static("current-focused"),
                BindingExpression::new_two_way(
                    NamedReference::new(elem, SmolStr::new_static("current-focused")).into(),
                )
                .into(),
            );
            tab_rc.borrow_mut().bindings.insert(
                SmolStr::new_static("tab-index"),
                RefCell::new(info.offset_expr.clone().into()),
            );
            tab_rc.borrow_mut().bindings.insert(
                SmolStr::new_static("num-tabs"),
                RefCell::new(num_tabs_expr.clone().into()),
            );

            tabs.push(tab_rc);
        } else {
            // Static tab
            let mut tab = Element {
                id: format_smolstr!("{}-tab{}", elem.borrow().id, child_idx),
                base_type: tab_impl.clone(),
                enclosing_component: elem.borrow().enclosing_component.clone(),
                ..Default::default()
            };
            tab.bindings.insert(
                SmolStr::new_static("title"),
                BindingExpression::new_two_way(
                    NamedReference::new(child, SmolStr::new_static("title")).into(),
                )
                .into(),
            );
            tab.bindings.insert(
                SmolStr::new_static("current"),
                BindingExpression::new_two_way(
                    NamedReference::new(elem, SmolStr::new_static("current-index")).into(),
                )
                .into(),
            );
            tab.bindings.insert(
                SmolStr::new_static("current-focused"),
                BindingExpression::new_two_way(
                    NamedReference::new(elem, SmolStr::new_static("current-focused")).into(),
                )
                .into(),
            );
            tab.bindings.insert(
                SmolStr::new_static("tab-index"),
                RefCell::new(info.offset_expr.clone().into()),
            );
            tab.bindings.insert(
                SmolStr::new_static("num-tabs"),
                RefCell::new(num_tabs_expr.clone().into()),
            );
            tabs.push(Element::make_rc(tab));
        }
    }

    let mut tabbar_impl = tabbar_horizontal_impl;
    if let Some(orientation) = elem.borrow().bindings.get("orientation") {
        if let Expression::EnumerationValue(val) =
            super::ignore_debug_hooks(&orientation.borrow().expression)
        {
            if val.value == 1 {
                tabbar_impl = tabbar_vertical_impl;
            }
        } else {
            diag.push_error(
                "The orientation property only supports constants at the moment".into(),
                &orientation.borrow().span,
            );
        }
    }
    let tabbar = Element {
        id: format_smolstr!("{}-tabbar", elem.borrow().id),
        base_type: tabbar_impl,
        enclosing_component: elem.borrow().enclosing_component.clone(),
        children: tabs,
        ..Default::default()
    };
    let tabbar = Element::make_rc(tabbar);
    set_tabbar_geometry_prop(elem, &tabbar, "x");
    set_tabbar_geometry_prop(elem, &tabbar, "y");
    set_tabbar_geometry_prop(elem, &tabbar, "width");
    set_tabbar_geometry_prop(elem, &tabbar, "height");
    tabbar
        .borrow_mut()
        .bindings
        .insert(SmolStr::new_static("num-tabs"), RefCell::new(num_tabs_expr.clone().into()));
    tabbar.borrow_mut().bindings.insert(
        SmolStr::new_static("current"),
        BindingExpression::new_two_way(
            NamedReference::new(elem, SmolStr::new_static("current-index")).into(),
        )
        .into(),
    );
    elem.borrow_mut().bindings.insert(
        SmolStr::new_static("current-focused"),
        BindingExpression::new_two_way(
            NamedReference::new(&tabbar, SmolStr::new_static("current-focused")).into(),
        )
        .into(),
    );
    elem.borrow_mut().bindings.insert(
        SmolStr::new_static("tabbar-preferred-width"),
        BindingExpression::new_two_way(
            NamedReference::new(&tabbar, SmolStr::new_static("preferred-width")).into(),
        )
        .into(),
    );
    elem.borrow_mut().bindings.insert(
        SmolStr::new_static("tabbar-preferred-height"),
        BindingExpression::new_two_way(
            NamedReference::new(&tabbar, SmolStr::new_static("preferred-height")).into(),
        )
        .into(),
    );

    // Only include static (non-repeated) children in content-min-width/height,
    // because repeated/conditional elements become sub-components whose properties
    // cannot be referenced directly from the parent component in the LLR.
    if let Some(expr) = children
        .iter()
        .filter(|x| x.borrow().repeated.is_none())
        .map(|x| {
            Expression::PropertyReference(NamedReference::new(x, SmolStr::new_static("min-width")))
        })
        .reduce(|lhs, rhs| crate::builtin_macros::min_max_expression(lhs, rhs, MinMaxOp::Max))
    {
        elem.borrow_mut().bindings.insert("content-min-width".into(), RefCell::new(expr.into()));
    };
    if let Some(expr) = children
        .iter()
        .filter(|x| x.borrow().repeated.is_none())
        .map(|x| {
            Expression::PropertyReference(NamedReference::new(x, SmolStr::new_static("min-height")))
        })
        .reduce(|lhs, rhs| crate::builtin_macros::min_max_expression(lhs, rhs, MinMaxOp::Max))
    {
        elem.borrow_mut().bindings.insert("content-min-height".into(), RefCell::new(expr.into()));
    };

    elem.borrow_mut().children = std::iter::once(tabbar).chain(children).collect();
}

fn set_geometry_prop(
    tab_widget: &ElementRc,
    content: &ElementRc,
    prop: &str,
    diag: &mut BuildDiagnostics,
) {
    let old = content.borrow_mut().bindings.insert(
        prop.into(),
        RefCell::new(
            Expression::PropertyReference(NamedReference::new(
                tab_widget,
                format_smolstr!("content-{}", prop),
            ))
            .into(),
        ),
    );
    if let Some(old) = old.map(RefCell::into_inner) {
        diag.push_error(
            format!("The property '{prop}' cannot be set for Tabs inside a TabWidget"),
            &old,
        );
    }
}

fn set_tabbar_geometry_prop(tab_widget: &ElementRc, tabbar: &ElementRc, prop: &str) {
    tabbar.borrow_mut().bindings.insert(
        prop.into(),
        RefCell::new(
            Expression::PropertyReference(NamedReference::new(
                tab_widget,
                format_smolstr!("tabbar-{}", prop),
            ))
            .into(),
        ),
    );
}
