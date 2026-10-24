// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

// cSpell: ignore tabwidget

//! This pass lowers the TabWidget to create the tabbar.
//!
//! Must be done before inlining and many other passes because the lowered code must
//! be further inlined as it may expand to a native widget that needs inlining

use crate::diagnostics::BuildDiagnostics;
use crate::element_type::ElementType;
use crate::expression_tree::{BindingExpression, Callable, Expression, NamedReference};
use crate::expression_types::Unit;
use crate::langtype::Type;
use crate::object_tree::*;
use smol_str::{SmolStr, format_smolstr};
use std::collections::HashSet;
use std::rc::Rc;

pub async fn lower_tabwidget(
    doc: &Document,
    type_loader: &mut crate::typeloader::TypeLoader,
    diag: &mut BuildDiagnostics,
) {
    // Collect before lowering: lowering rewrites base_type, which would hide
    // other TabWidget elements from builtin_type() (an instance and the style
    // wrapper both match). Dedup a sub-component root visited more than once.
    // The empty check below also avoids loading std-widgets.slint when unused.
    let mut seen = HashSet::new();
    let mut tab_widgets = Vec::new();
    doc.visit_all_used_components(|component| {
        error_on_slot_in_inner_builtin(component, &["TabWidget"], diag);
        recurse_elem_including_sub_components_no_borrow(component, &(), &mut |elem, _| {
            if matches!(&elem.borrow().builtin_type(), Some(b) if b.name == "TabWidget")
                && seen.insert(Rc::as_ptr(elem))
            {
                tab_widgets.push(elem.clone());
            }
        })
    });

    if tab_widgets.is_empty() {
        return;
    }

    // If an element's base is itself a collected TabWidget that already has its own Tab
    // children (e.g. `component Xyz inherits TabWidget { Tab { ... } }`), it's just
    // instantiating Xyz rather than defining new Tabs, so skip lowering it here. Lowering it
    // too would overwrite its base_type and lose the Tabs already lowered on Xyz (issue
    // #8394); leaving it alone lets inlining pull Xyz's own Tabs in later.
    //
    // Any Tab children or an `orientation` override set directly at such an instantiation
    // site can't be folded into the base, since that would mean moving elements across
    // component boundaries. Diagnose both cases instead of silently dropping or ignoring them.
    let tab_widgets: Vec<_> = tab_widgets
        .into_iter()
        .filter(|elem| {
            if !base_chain_declares_children(&elem.borrow().base_type, &seen) {
                return true;
            }
            if !elem.borrow().children.is_empty() {
                diag.push_error(
                    "Cannot add Tab elements when instantiating a TabWidget subclass that already declares its own Tab; declare them on the subclass instead".to_owned(),
                    &*elem.borrow(),
                );
            }
            if let Some(orientation) = elem.borrow().binding("orientation") {
                diag.push_error(
                    "Cannot set orientation when instantiating a TabWidget subclass that already declares its own Tab; declare it on the subclass instead".to_owned(),
                    &orientation.span,
                );
            }
            false
        })
        .collect();

    // Ignore import errors
    let mut build_diags_to_ignore = BuildDiagnostics::default();
    let tabwidget_impl = type_loader
        .import_component("std-widgets-impl.slint", "TabWidgetImpl", &mut build_diags_to_ignore)
        .await
        .expect("can't load TabWidgetImpl from std-widgets-impl.slint");
    let tab_impl = type_loader
        .import_component("std-widgets-impl.slint", "TabImpl", &mut build_diags_to_ignore)
        .await
        .expect("can't load TabImpl from std-widgets-impl.slint");
    let tabbar_horizontal_impl = type_loader
        .import_component(
            "std-widgets-impl.slint",
            "TabBarHorizontalImpl",
            &mut build_diags_to_ignore,
        )
        .await
        .expect("can't load TabBarHorizontalImpl from std-widgets-impl.slint");
    let tabbar_vertical_impl = type_loader
        .import_component(
            "std-widgets-impl.slint",
            "TabBarVerticalImpl",
            &mut build_diags_to_ignore,
        )
        .await
        .expect("can't load TabBarVerticalImpl from std-widgets-impl.slint");
    let empty_type = type_loader.global_type_registry.borrow().empty_type();
    let grid_type =
        type_loader.global_type_registry.borrow().lookup_builtin_element("GridLayout").unwrap();

    for elem in &tab_widgets {
        process_tabwidget(
            elem,
            ElementType::Component(tabwidget_impl.clone()),
            ElementType::Component(tab_impl.clone()),
            ElementType::Component(tabbar_horizontal_impl.clone()),
            ElementType::Component(tabbar_vertical_impl.clone()),
            &empty_type,
            &grid_type,
            diag,
        );
    }
}

fn process_tabwidget(
    elem: &ElementRc,
    tabwidget_impl: ElementType,
    tab_impl: ElementType,
    tabbar_horizontal_impl: ElementType,
    tabbar_vertical_impl: ElementType,
    empty_type: &ElementType,
    grid_type: &ElementType,
    diag: &mut BuildDiagnostics,
) {
    elem.borrow_mut().base_type = tabwidget_impl;
    let (timers, mut children): (Vec<_>, Vec<_>) = std::mem::take(&mut elem.borrow_mut().children)
        .into_iter()
        .partition(|child| {
            matches!(&child.borrow().base_type, ElementType::Builtin(b) if b.name == "Timer")
        });

    // Every child contributes a count (1 / cond?1:0 / model length), stored in
    // a synthetic property on the TabWidget; offsets are running sums.
    let add = |lhs: Expression, rhs: Expression| Expression::BinaryExpression {
        lhs: lhs.into(),
        rhs: rhs.into(),
        op: '+',
        source_location: None,
    };
    let mut offsets = Vec::new();
    let mut running = Expression::NumberLiteral(0.0, Unit::None);
    for (i, child) in children.iter().enumerate() {
        offsets.push(running.clone());
        let count = match &child.borrow().repeated {
            None => Expression::NumberLiteral(1.0, Unit::None),
            Some(r) if r.is_conditional_element => Expression::Condition {
                condition: r.model.clone().into(),
                true_expr: Expression::NumberLiteral(1.0, Unit::None).into(),
                false_expr: Expression::NumberLiteral(0.0, Unit::None).into(),
                source_location: None,
            },
            Some(r) => Expression::FunctionCall {
                function: Callable::Builtin(crate::builtin_functions::BuiltinFunction::ArrayLength),
                arguments: vec![r.model.clone()],
                source_location: None,
            },
        };
        let count_prop =
            crate::layout::create_new_prop(elem, format_smolstr!("tab-count-{i}"), Type::Int32);
        elem.borrow_mut().set_binding(count_prop.name().clone(), count.into());
        running = add(running, Expression::PropertyReference(count_prop));
    }
    let num_tabs_expr = running;

    let mut tabs = Vec::new();
    for (position, child) in children.iter_mut().enumerate() {
        error_on_slot_default(child, "TabWidget", diag);
        if child.borrow().base_type.to_string() != "Tab" {
            assert!(diag.has_errors());
            continue;
        }
        let index = tabs.len();
        child.borrow_mut().base_type = empty_type.clone();
        child
            .borrow_mut()
            .property_declarations
            .insert(SmolStr::new_static("title"), Type::String.into());
        // Panels are cells (0,0) of a synthetic GridLayout instead of
        // being bound to content-{x,y,width,height} directly.
        for prop in ["x", "y", "width", "height", "row", "col", "rowspan", "colspan"] {
            if let Some(old) = child.borrow().binding(prop) {
                diag.push_error(
                    format!("The property '{prop}' cannot be set for Tabs inside a TabWidget"),
                    &*old,
                );
            }
        }
        for prop in ["row", "col"] {
            child.borrow_mut().set_binding(
                SmolStr::new_static(prop),
                Expression::NumberLiteral(0.0, Unit::None).into(),
            );
        }

        let is_for = child.borrow().repeated.as_ref().is_some_and(|r| !r.is_conditional_element);
        let index_expr = if is_for {
            add(
                offsets[position].clone(),
                Expression::RepeaterIndexReference { element: Rc::downgrade(child) },
            )
        } else {
            offsets[position].clone()
        };
        let condition = Expression::BinaryExpression {
            lhs: Expression::PropertyReference(NamedReference::new(
                elem,
                SmolStr::new_static("current-index"),
            ))
            .into(),
            rhs: index_expr.clone().into(),
            op: '=',
            source_location: None,
        };
        let old = child.borrow_mut().set_binding(SmolStr::new_static("visible"), condition.into());
        if let Some(old) = old {
            diag.push_error(
                "The property 'visible' cannot be set for Tabs inside a TabWidget".to_owned(),
                &old,
            );
        }
        let role = crate::typeregister::BUILTIN
            .enums
            .AccessibleRole
            .clone()
            .try_value_from_string("tab-panel")
            .unwrap();
        let old = child.borrow_mut().set_binding(
            SmolStr::new_static("accessible-role"),
            Expression::EnumerationValue(role).into(),
        );
        if let Some(old) = old {
            diag.push_error(
                "The property 'accessible-role' cannot be set for Tabs inside a TabWidget"
                    .to_owned(),
                &old,
            );
        }
        let title_ref =
            Expression::PropertyReference(NamedReference::new(child, "title".into())).into();
        let old = child.borrow_mut().set_binding("accessible-label".into(), title_ref);
        if let Some(old) = old {
            diag.push_error(
                "The property 'accessible-label' cannot be set for Tabs inside a TabWidget"
                    .to_owned(),
                &old,
            );
        }

        let tab_rc = Element::make_rc(Element {
            id: format_smolstr!("{}-tab{}", elem.borrow().id, index),
            base_type: tab_impl.clone(),
            enclosing_component: elem.borrow().enclosing_component.clone(),
            repeated: child.borrow().repeated.clone(),
            ..Default::default()
        });
        // For repeated tabs, copy the title expression into the header's own repeater
        // and retarget model/index references child -> tab.
        if child.borrow().repeated.is_some() {
            let title_expr = child
                .borrow()
                .binding("title")
                .map(|b| b.expression.clone())
                .unwrap_or(Expression::StringLiteral("".into()));
            let mut title_expr = title_expr;
            let child_weak = Rc::downgrade(child);
            let tab_weak = Rc::downgrade(&tab_rc);
            title_expr.visit_recursive_mut(&mut |e| match e {
                Expression::RepeaterModelReference { element }
                | Expression::RepeaterIndexReference { element }
                    if std::rc::Weak::ptr_eq(element, &child_weak) =>
                {
                    *element = tab_weak.clone();
                }
                _ => {}
            });
            tab_rc.borrow_mut().set_binding(SmolStr::new_static("title"), title_expr.into());
        } else {
            tab_rc.borrow_mut().set_binding(
                SmolStr::new_static("title"),
                BindingExpression::new_two_way(
                    NamedReference::new(child, SmolStr::new_static("title")).into(),
                ),
            );
        }
        let tab_index_expr = if is_for {
            add(
                offsets[position].clone(),
                Expression::RepeaterIndexReference { element: Rc::downgrade(&tab_rc) },
            )
        } else {
            offsets[position].clone()
        };
        let mut tab = tab_rc.borrow_mut();
        tab.set_binding(
            SmolStr::new_static("current"),
            BindingExpression::new_two_way(
                NamedReference::new(elem, SmolStr::new_static("current-index")).into(),
            ),
        );
        tab.set_binding(
            SmolStr::new_static("current-focused"),
            BindingExpression::new_two_way(
                NamedReference::new(elem, SmolStr::new_static("current-focused")).into(),
            ),
        );
        tab.set_binding(SmolStr::new_static("tab-index"), tab_index_expr.into());
        tab.set_binding(SmolStr::new_static("num-tabs"), num_tabs_expr.clone().into());
        drop(tab);
        tabs.push(tab_rc);
    }

    let mut tabbar_impl = tabbar_horizontal_impl;
    if let Some(orientation) = elem.borrow().binding("orientation") {
        if let Expression::EnumerationValue(val) = orientation.value_expression() {
            if val.value == 1 {
                tabbar_impl = tabbar_vertical_impl;
            }
        } else {
            diag.push_error(
                "The orientation property only supports constants at the moment".into(),
                &orientation.span,
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
    tabbar.borrow_mut().set_binding(SmolStr::new_static("num-tabs"), num_tabs_expr.clone().into());
    tabbar.borrow_mut().set_binding(
        SmolStr::new_static("current"),
        BindingExpression::new_two_way(
            NamedReference::new(elem, SmolStr::new_static("current-index")).into(),
        ),
    );
    elem.borrow_mut().set_binding(
        SmolStr::new_static("current-focused"),
        BindingExpression::new_two_way(
            NamedReference::new(&tabbar, SmolStr::new_static("current-focused")).into(),
        ),
    );
    elem.borrow_mut().set_binding(
        SmolStr::new_static("tabbar-preferred-width"),
        BindingExpression::new_two_way(
            NamedReference::new(&tabbar, SmolStr::new_static("preferred-width")).into(),
        ),
    );
    elem.borrow_mut().set_binding(
        SmolStr::new_static("tabbar-preferred-height"),
        BindingExpression::new_two_way(
            NamedReference::new(&tabbar, SmolStr::new_static("preferred-height")).into(),
        ),
    );

    // One GridLayout hosts all panels in cell (0,0); its layout info is
    // the max over all panels, including every instance of a `for`/`if`.
    let content = Element::make_rc(Element {
        id: format_smolstr!("{}-content", elem.borrow().id),
        base_type: grid_type.clone(),
        enclosing_component: elem.borrow().enclosing_component.clone(),
        children,
        ..Default::default()
    });
    for prop in ["x", "y", "width", "height"] {
        content.borrow_mut().set_binding(
            prop.into(),
            Expression::PropertyReference(NamedReference::new(
                elem,
                format_smolstr!("content-{}", prop),
            ))
            .into(),
        );
    }
    elem.borrow_mut().set_binding(
        "content-min-width".into(),
        Expression::PropertyReference(NamedReference::new(&content, "min-width".into())).into(),
    );
    elem.borrow_mut().set_binding(
        "content-min-height".into(),
        Expression::PropertyReference(NamedReference::new(&content, "min-height".into())).into(),
    );

    elem.borrow_mut().children = [tabbar, content].into_iter().chain(timers).collect();
}

fn set_tabbar_geometry_prop(tab_widget: &ElementRc, tabbar: &ElementRc, prop: &str) {
    tabbar.borrow_mut().set_binding(
        prop.into(),
        Expression::PropertyReference(NamedReference::new(
            tab_widget,
            format_smolstr!("tabbar-{}", prop),
        ))
        .into(),
    );
}
