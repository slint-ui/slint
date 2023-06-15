// Copyright © SixtyFPS GmbH <info@slint-ui.com>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-1.0 OR LicenseRef-Slint-commercial

// cSpell: ignore tabwidget

//! Passe lower the TabWidget to create the tabbar.
//!
//! Must be done before inlining and many other passes because the lowered code must
//! be further inlined as it may expends to native widget that needs inlining

use crate::diagnostics::BuildDiagnostics;
use crate::expression_tree::{BindingExpression, Expression, NamedReference, Unit};
use crate::langtype::{ElementType, Type};
use crate::object_tree::*;
use std::cell::RefCell;
use std::rc::Rc;

pub async fn lower_tabwidget(
    component: &Rc<Component>,
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
            assert!(diag.has_error());
            continue;
        }
        let index = tabs.len();
        child.borrow_mut().base_type = empty_type.clone();
        child.borrow_mut().property_declarations.insert("title".to_owned(), Type::String.into());
        set_geometry_prop(elem, child, "x", diag);
        set_geometry_prop(elem, child, "y", diag);
        set_geometry_prop(elem, child, "width", diag);
        set_geometry_prop(elem, child, "height", diag);
        let condition = Expression::BinaryExpression {
            lhs: Expression::PropertyReference(NamedReference::new(elem, "current-index")).into(),
            rhs: Expression::NumberLiteral(index as _, Unit::None).into(),
            op: '=',
        };
        let old = child
            .borrow_mut()
            .bindings
            .insert("visible".to_owned(), RefCell::new(condition.into()));
        if let Some(old) = old {
            diag.push_error(
                "The property 'visible' cannot be set for Tabs inside a TabWidget".to_owned(),
                &old.into_inner(),
            );
        }

        let mut tab = Element {
            id: format!("{}-tab{}", elem.borrow().id, index),
            base_type: tab_impl.clone(),
            enclosing_component: elem.borrow().enclosing_component.clone(),
            ..Default::default()
        };
        tab.bindings.insert(
            "title".to_owned(),
            BindingExpression::new_two_way(NamedReference::new(child, "title")).into(),
        );
        tab.bindings.insert(
            "current".to_owned(),
            BindingExpression::new_two_way(NamedReference::new(elem, "current-index")).into(),
        );
        tab.bindings.insert(
            "current-focused".to_owned(),
            BindingExpression::new_two_way(NamedReference::new(elem, "current-focused")).into(),
        );
        tab.bindings.insert(
            "tab-index".to_owned(),
            RefCell::new(Expression::NumberLiteral(index as _, Unit::None).into()),
        );
        tab.bindings.insert(
            "num-tabs".to_owned(),
            RefCell::new(Expression::NumberLiteral(num_tabs as _, Unit::None).into()),
        );
        tabs.push(Rc::new(RefCell::new(tab)));
    }

    let tabbar = Element {
        id: format!("{}-tabbar", elem.borrow().id),
        base_type: tabbar_impl,
        enclosing_component: elem.borrow().enclosing_component.clone(),
        children: tabs,
        ..Default::default()
    };
    let tabbar = Rc::new(RefCell::new(tabbar));
    set_tabbar_geometry_prop(elem, &tabbar, "x");
    set_tabbar_geometry_prop(elem, &tabbar, "y");
    set_tabbar_geometry_prop(elem, &tabbar, "width");
    set_tabbar_geometry_prop(elem, &tabbar, "height");
    tabbar.borrow_mut().bindings.insert(
        "num-tabs".to_owned(),
        RefCell::new(Expression::NumberLiteral(num_tabs as _, Unit::None).into()),
    );
    tabbar.borrow_mut().bindings.insert(
        "current".to_owned(),
        BindingExpression::new_two_way(NamedReference::new(elem, "current-index")).into(),
    );
    elem.borrow_mut().bindings.insert(
        "current-focused".to_owned(),
        BindingExpression::new_two_way(NamedReference::new(&tabbar, "current-focused")).into(),
    );
    elem.borrow_mut().bindings.insert(
        "tabbar-preferred-width".to_owned(),
        BindingExpression::new_two_way(NamedReference::new(&tabbar, "preferred-width")).into(),
    );
    elem.borrow_mut().bindings.insert(
        "tabbar-preferred-height".to_owned(),
        BindingExpression::new_two_way(NamedReference::new(&tabbar, "preferred-height")).into(),
    );

    if let Some(expr) = children
        .iter()
        .map(|x| Expression::PropertyReference(NamedReference::new(x, "min-width")))
        .reduce(|lhs, rhs| crate::builtin_macros::min_max_expression(lhs, rhs, '>'))
    {
        elem.borrow_mut().bindings.insert("content-min-width".into(), RefCell::new(expr.into()));
    };
    if let Some(expr) = children
        .iter()
        .map(|x| Expression::PropertyReference(NamedReference::new(x, "min-height")))
        .reduce(|lhs, rhs| crate::builtin_macros::min_max_expression(lhs, rhs, '>'))
    {
        elem.borrow_mut().bindings.insert("content-min-height".into(), RefCell::new(expr.into()));
    };

    elem.borrow_mut().children = std::iter::once(tabbar).chain(children.into_iter()).collect();
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
                &format!("content-{}", prop),
            ))
            .into(),
        ),
    );
    if let Some(old) = old.map(RefCell::into_inner) {
        diag.push_error(
            format!("The property '{}' cannot be set for Tabs inside a TabWidget", prop),
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
                &format!("tabbar-{}", prop),
            ))
            .into(),
        ),
    );
}
