// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

// cSpell: ignore tabwidget

//! Passe lower the TabWidget to create the tabbar.
//!
//! Must be done before inlining and many other passes because the lowered code must
//! be further inlined as it may expends to native widget that needs inlining

use crate::diagnostics::BuildDiagnostics;
use crate::expression_tree::{BindingExpression, Expression, MinMaxOp, NamedReference, Unit};
use crate::langtype::{ElementType, Type};
use crate::object_tree::*;
use smol_str::{format_smolstr, SmolStr};
use std::cell::RefCell;

pub async fn lower_tabwidget(
    doc: &Document,
    type_loader: &mut crate::typeloader::TypeLoader,
    diag: &mut BuildDiagnostics,
) {
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
    let tabbar_impl = type_loader
        .import_component("std-widgets.slint", "TabBarImpl", &mut build_diags_to_ignore)
        .await
        .expect("can't load TabBarImpl from std-widgets.slint");
    let empty_type = type_loader.global_type_registry.borrow().empty_type();

    doc.visit_all_used_components(|component| {
        recurse_elem_including_sub_components_no_borrow(component, &(), &mut |elem, _| {
            if matches!(&elem.borrow().builtin_type(), Some(b) if b.name == "TabWidget") {
                process_tabwidget(
                    elem,
                    ElementType::Component(tabwidget_impl.clone()),
                    ElementType::Component(tab_impl.clone()),
                    ElementType::Component(tabbar_impl.clone()),
                    &empty_type,
                    diag,
                );
            }
        })
    });
}

fn process_tabwidget(
    elem: &ElementRc,
    tabwidget_impl: ElementType,
    tab_impl: ElementType,
    tabbar_impl: ElementType,
    empty_type: &ElementType,
    diag: &mut BuildDiagnostics,
) {
    if matches!(&elem.borrow_mut().base_type, ElementType::Builtin(_)) {
        // That's the TabWidget re-exported from the style, it doesn't need to be processed
        return;
    }

    elem.borrow_mut().base_type = tabwidget_impl;
    let mut children = std::mem::take(&mut elem.borrow_mut().children);
    let num_tabs = children.len();
    let mut tabs = Vec::new();
    for child in &mut children {
        if child.borrow().repeated.is_some() {
            diag.push_error(
                "dynamic tabs ('if' or 'for') are currently not supported".into(),
                &*child.borrow(),
            );
            continue;
        }
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
        set_geometry_prop(elem, child, "x", diag);
        set_geometry_prop(elem, child, "y", diag);
        set_geometry_prop(elem, child, "width", diag);
        set_geometry_prop(elem, child, "height", diag);
        let condition = Expression::BinaryExpression {
            lhs: Expression::PropertyReference(NamedReference::new(
                elem,
                SmolStr::new_static("current-index"),
            ))
            .into(),
            rhs: Expression::NumberLiteral(index as _, Unit::None).into(),
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

        let mut tab = Element {
            id: format_smolstr!("{}-tab{}", elem.borrow().id, index),
            base_type: tab_impl.clone(),
            enclosing_component: elem.borrow().enclosing_component.clone(),
            ..Default::default()
        };
        tab.bindings.insert(
            SmolStr::new_static("title"),
            BindingExpression::new_two_way(NamedReference::new(
                child,
                SmolStr::new_static("title"),
            ))
            .into(),
        );
        tab.bindings.insert(
            SmolStr::new_static("current"),
            BindingExpression::new_two_way(NamedReference::new(
                elem,
                SmolStr::new_static("current-index"),
            ))
            .into(),
        );
        tab.bindings.insert(
            SmolStr::new_static("current-focused"),
            BindingExpression::new_two_way(NamedReference::new(
                elem,
                SmolStr::new_static("current-focused"),
            ))
            .into(),
        );
        tab.bindings.insert(
            SmolStr::new_static("tab-index"),
            RefCell::new(Expression::NumberLiteral(index as _, Unit::None).into()),
        );
        tab.bindings.insert(
            SmolStr::new_static("num-tabs"),
            RefCell::new(Expression::NumberLiteral(num_tabs as _, Unit::None).into()),
        );
        tabs.push(Element::make_rc(tab));
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
    tabbar.borrow_mut().bindings.insert(
        SmolStr::new_static("num-tabs"),
        RefCell::new(Expression::NumberLiteral(num_tabs as _, Unit::None).into()),
    );
    tabbar.borrow_mut().bindings.insert(
        SmolStr::new_static("current"),
        BindingExpression::new_two_way(NamedReference::new(
            elem,
            SmolStr::new_static("current-index"),
        ))
        .into(),
    );
    elem.borrow_mut().bindings.insert(
        SmolStr::new_static("current-focused"),
        BindingExpression::new_two_way(NamedReference::new(
            &tabbar,
            SmolStr::new_static("current-focused"),
        ))
        .into(),
    );
    elem.borrow_mut().bindings.insert(
        SmolStr::new_static("tabbar-preferred-width"),
        BindingExpression::new_two_way(NamedReference::new(
            &tabbar,
            SmolStr::new_static("preferred-width"),
        ))
        .into(),
    );
    elem.borrow_mut().bindings.insert(
        SmolStr::new_static("tabbar-preferred-height"),
        BindingExpression::new_two_way(NamedReference::new(
            &tabbar,
            SmolStr::new_static("preferred-height"),
        ))
        .into(),
    );

    if let Some(expr) = children
        .iter()
        .map(|x| {
            Expression::PropertyReference(NamedReference::new(x, SmolStr::new_static("min-width")))
        })
        .reduce(|lhs, rhs| crate::builtin_macros::min_max_expression(lhs, rhs, MinMaxOp::Max))
    {
        elem.borrow_mut().bindings.insert("content-min-width".into(), RefCell::new(expr.into()));
    };
    if let Some(expr) = children
        .iter()
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
