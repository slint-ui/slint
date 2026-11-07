// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

// cSpell: ignore tabwidget

//! This pass lowers the TabWidget to create the tabbar.
//!
//! Must be done before inlining and many other passes because the lowered code must
//! be further inlined as it may expand to a native widget that needs inlining

use crate::diagnostics::BuildDiagnostics;
use crate::element_type::ElementType;
use crate::expression_tree::{
    BindingExpression, Callable, Expression, NamedReference, TwoWayBinding,
};
use crate::expression_types::{MinMaxOp, Unit};
use crate::langtype::Type;
use crate::object_tree::*;
use smol_str::{SmolStr, format_smolstr};
use std::collections::HashSet;
use std::rc::{Rc, Weak};

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
    let (timers, children): (Vec<_>, Vec<_>) = std::mem::take(&mut elem.borrow_mut().children)
        .into_iter()
        .partition(|child| {
            matches!(&child.borrow().base_type, ElementType::Builtin(b) if b.name == "Timer")
        });

    let num_tabs =
        crate::layout::create_new_prop(elem, SmolStr::new_static("num-tabs"), Type::Int32);
    let current_index = NamedReference::new(elem, SmolStr::new_static("current-index"));

    // The index of the first tab of the child being lowered
    let mut offset = Expression::NumberLiteral(0.0, Unit::None);
    let mut tabs = Vec::new();
    // Static panels are bound to the content area directly. The panels of `if` and `for`
    // are put in a GridLayout, which is the only way to get the minimum size of all instances.
    let mut static_panels = Vec::new();
    let mut repeated_panels = Vec::new();
    for child in children {
        error_on_slot_default(&child, "TabWidget", diag);
        if child.borrow().base_type.to_string() != "Tab" {
            assert!(diag.has_errors());
            static_panels.push(child);
            continue;
        }
        let tab_n = tabs.len();
        child.borrow_mut().base_type = empty_type.clone();
        child
            .borrow_mut()
            .property_declarations
            .insert(SmolStr::new_static("title"), Type::String.into());
        for prop in ["row", "col", "rowspan", "colspan"] {
            let old = child.borrow_mut().take_binding(prop);
            if let Some(old) = old {
                diag.push_error(
                    format!("The property '{prop}' cannot be set for Tabs inside a TabWidget"),
                    &old,
                );
            }
        }

        let repeated = child.borrow_mut().repeated.take();
        // The `if` or `for` moves to a wrapper that is the grid cell. The wrapper only takes
        // the minimum size of the Tab, so the Tab fills the content area like a static one.
        let repeated_panel = repeated.map(|mut repeated| {
            share_model(elem, &mut repeated, tab_n);
            let repeated_panel = Element::make_rc(Element {
                id: format_smolstr!("{}-panel{}", elem.borrow().id, tab_n),
                base_type: empty_type.clone(),
                enclosing_component: elem.borrow().enclosing_component.clone(),
                repeated: Some(repeated),
                children: vec![child.clone()],
                ..Default::default()
            });
            retarget_repeater_references(&child, &child, &repeated_panel);
            repeated_panel
        });

        let geometry = match &repeated_panel {
            None => ["x", "y", "width", "height"].map(|prop| {
                let content = NamedReference::new(elem, format_smolstr!("content-{prop}"));
                (prop, Expression::PropertyReference(content))
            }),
            Some(repeated_panel) => {
                let mut panel = repeated_panel.borrow_mut();
                for prop in ["row", "col"] {
                    panel.set_binding(
                        prop.into(),
                        Expression::NumberLiteral(0.0, Unit::None).into(),
                    );
                }
                for prop in ["min-width", "min-height"] {
                    panel.set_binding(
                        prop.into(),
                        Expression::PropertyReference(NamedReference::new(&child, prop.into()))
                            .into(),
                    );
                }
                drop(panel);
                let zero = Expression::NumberLiteral(0.0, Unit::Px);
                let parent_size = |prop: &str| {
                    Expression::PropertyReference(NamedReference::new(repeated_panel, prop.into()))
                };
                [
                    ("x", zero.clone()),
                    ("y", zero),
                    ("width", parent_size("width")),
                    ("height", parent_size("height")),
                ]
            }
        };
        for (prop, value) in geometry {
            let old = child.borrow_mut().set_binding(prop.into(), value.into());
            if let Some(old) = old {
                diag.push_error(
                    format!("The property '{prop}' cannot be set for Tabs inside a TabWidget"),
                    &old,
                );
            }
        }

        let is_for = repeated_panel.as_ref().is_some_and(|p| {
            p.borrow().repeated.as_ref().is_some_and(|r| !r.is_conditional_element)
        });
        let index_in_child = |repeated_element: &ElementRc| {
            if is_for {
                add(
                    offset.clone(),
                    Expression::RepeaterIndexReference { element: Rc::downgrade(repeated_element) },
                )
            } else {
                offset.clone()
            }
        };

        let condition = Expression::BinaryExpression {
            lhs: Expression::PropertyReference(current_index.clone()).into(),
            rhs: repeated_panel.as_ref().map_or(offset.clone(), index_in_child).into(),
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
            Expression::PropertyReference(NamedReference::new(&child, "title".into())).into();
        let old = child.borrow_mut().set_binding("accessible-label".into(), title_ref);
        if let Some(old) = old {
            diag.push_error(
                "The property 'accessible-label' cannot be set for Tabs inside a TabWidget"
                    .to_owned(),
                &old,
            );
        }

        let tab = Element::make_rc(Element {
            id: format_smolstr!("{}-tab{}", elem.borrow().id, tab_n),
            base_type: tab_impl.clone(),
            enclosing_component: elem.borrow().enclosing_component.clone(),
            repeated: repeated_panel.as_ref().and_then(|p| p.borrow().repeated.clone()),
            ..Default::default()
        });
        let (title, tab_index, count) = match &repeated_panel {
            None => (
                BindingExpression::new_two_way(
                    NamedReference::new(&child, SmolStr::new_static("title")).into(),
                ),
                offset.clone(),
                Expression::NumberLiteral(1.0, Unit::None),
            ),
            Some(repeated_panel) => {
                check_title_is_only_bound(&child, diag);
                let title = repeated_tab_title(&child, repeated_panel, &tab, diag);
                let count = tab_count(&repeated_panel.borrow());
                let count_prop = crate::layout::create_new_prop(
                    elem,
                    format_smolstr!("tab-count-{tab_n}"),
                    Type::Int32,
                );
                elem.borrow_mut().set_binding(count_prop.name().clone(), count.into());
                (title, index_in_child(&tab), Expression::PropertyReference(count_prop))
            }
        };
        {
            let mut tab = tab.borrow_mut();
            tab.set_binding(SmolStr::new_static("title"), title);
            tab.set_binding(
                SmolStr::new_static("current"),
                BindingExpression::new_two_way(current_index.clone().into()),
            );
            tab.set_binding(
                SmolStr::new_static("current-focused"),
                BindingExpression::new_two_way(
                    NamedReference::new(elem, SmolStr::new_static("current-focused")).into(),
                ),
            );
            tab.set_binding(SmolStr::new_static("tab-index"), tab_index.into());
            tab.set_binding(
                SmolStr::new_static("num-tabs"),
                Expression::PropertyReference(num_tabs.clone()).into(),
            );
        }
        tabs.push(tab);

        offset = add(offset, count);
        match repeated_panel {
            Some(repeated_panel) => repeated_panels.push(repeated_panel),
            None => static_panels.push(child),
        }
    }
    elem.borrow_mut().set_binding(num_tabs.name().clone(), offset.into());

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
    tabbar.borrow_mut().set_binding(
        SmolStr::new_static("num-tabs"),
        Expression::PropertyReference(num_tabs).into(),
    );
    tabbar.borrow_mut().set_binding(
        SmolStr::new_static("current"),
        BindingExpression::new_two_way(current_index.into()),
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

    let grid = (!repeated_panels.is_empty()).then(|| {
        let grid = Element::make_rc(Element {
            id: format_smolstr!("{}-content", elem.borrow().id),
            base_type: grid_type.clone(),
            enclosing_component: elem.borrow().enclosing_component.clone(),
            children: repeated_panels,
            ..Default::default()
        });
        for prop in ["x", "y", "width", "height"] {
            grid.borrow_mut().set_binding(
                prop.into(),
                Expression::PropertyReference(NamedReference::new(
                    elem,
                    format_smolstr!("content-{prop}"),
                ))
                .into(),
            );
        }
        grid
    });

    for prop in ["min-width", "min-height"] {
        if let Some(expr) = static_panels
            .iter()
            .chain(&grid)
            .map(|x| Expression::PropertyReference(NamedReference::new(x, prop.into())))
            .reduce(|lhs, rhs| crate::builtin_macros::min_max_expression(lhs, rhs, MinMaxOp::Max))
        {
            elem.borrow_mut().set_binding(format_smolstr!("content-{prop}"), expr.into());
        }
    }

    elem.borrow_mut().children =
        std::iter::once(tabbar).chain(static_panels).chain(grid).chain(timers).collect();
}

fn add(lhs: Expression, rhs: Expression) -> Expression {
    match (&lhs, &rhs) {
        (
            Expression::NumberLiteral(lhs, Unit::None),
            Expression::NumberLiteral(rhs, Unit::None),
        ) => Expression::NumberLiteral(lhs + rhs, Unit::None),
        _ => Expression::BinaryExpression {
            lhs: lhs.into(),
            rhs: rhs.into(),
            op: '+',
            source_location: None,
        },
    }
}

/// The number of tabs that the `if` or `for` of `repeated_panel` creates.
fn tab_count(repeated_panel: &Element) -> Expression {
    let repeated = repeated_panel.repeated.as_ref().unwrap();
    if repeated.is_conditional_element {
        Expression::Condition {
            condition: repeated.model.clone().into(),
            true_expr: Expression::NumberLiteral(1.0, Unit::None).into(),
            false_expr: Expression::NumberLiteral(0.0, Unit::None).into(),
            source_location: None,
        }
    } else {
        Expression::FunctionCall {
            function: Callable::Builtin(crate::builtin_functions::BuiltinFunction::ArrayLength),
            arguments: vec![repeated.model.clone()],
            source_location: None,
        }
    }
}

/// Moves the model of an `if` or `for` into a property of the TabWidget.
///
/// The tab bar entries and the panels are separate repeaters.
/// Without this, a model expression that creates a new model gives each repeater its own instance,
/// so changes to the model data through one of them don't reach the other.
fn share_model(tab_widget: &ElementRc, repeated: &mut RepeatedElementInfo, tab_n: usize) {
    let model = if repeated.is_conditional_element {
        &mut repeated.model
    } else if let Expression::Cast { from, to: Type::Model } = &mut repeated.model {
        &mut **from
    } else {
        return;
    };
    let ty = model.ty();
    if matches!(model, Expression::PropertyReference(_)) || !ty.is_property_type() {
        return;
    }
    let prop = crate::layout::create_new_prop(tab_widget, format_smolstr!("tab-model-{tab_n}"), ty);
    let model = std::mem::replace(model, Expression::PropertyReference(prop.clone()));
    tab_widget.borrow_mut().set_binding(prop.name().clone(), model.into());
}

/// Points the references to the model data and index of `from` in `root` and its children to `to`.
fn retarget_repeater_references(root: &ElementRc, from: &ElementRc, to: &ElementRc) {
    let from = Rc::downgrade(from);
    let to = Rc::downgrade(to);
    recurse_elem(root, &(), &mut |elem, _| {
        visit_element_expressions(elem, |expr, _, _| {
            expr.visit_recursive_mut(&mut |expr| match expr {
                Expression::RepeaterModelReference { element }
                | Expression::RepeaterIndexReference { element }
                    if Weak::ptr_eq(element, &from) =>
                {
                    *element = to.clone();
                }
                _ => {}
            })
        })
    });
}

/// The title binding of the tab bar entry `tab` of the Tab `panel` in an `if` or `for`.
///
/// `tab` and `repeated_panel` are separate repeaters, so `tab` can't access `panel` or its children.
fn repeated_tab_title(
    panel: &ElementRc,
    repeated_panel: &ElementRc,
    tab: &ElementRc,
    diag: &mut BuildDiagnostics,
) -> BindingExpression {
    let Some(binding) = panel.borrow().binding("title").map(|b| (*b).clone()) else {
        return Expression::StringLiteral(SmolStr::default()).into();
    };

    let mut inside = HashSet::new();
    recurse_elem(repeated_panel, &(), &mut |elem, _| {
        inside.insert(Rc::as_ptr(elem));
    });
    let is_inside = |elem: &ElementRc| inside.contains(&Rc::as_ptr(elem));
    let describe = |nr: &NamedReference| {
        let elem = nr.element();
        let id = elem.borrow().id.clone();
        if !id.is_empty() {
            format!("{id}.{}", nr.name())
        } else if Rc::ptr_eq(&elem, panel) {
            format!("self.{}", nr.name())
        } else {
            nr.name().to_string()
        }
    };

    let from = Rc::downgrade(repeated_panel);
    let to = Rc::downgrade(tab);
    let mut unreachable = None;
    let mut expression = binding.expression;
    expression.visit_recursive_mut(&mut |expr| match expr {
        Expression::RepeaterModelReference { element }
        | Expression::RepeaterIndexReference { element }
            if Weak::ptr_eq(element, &from) =>
        {
            *element = to.clone();
        }
        Expression::PropertyReference(nr)
        | Expression::FunctionCall {
            function: Callable::Callback(nr) | Callable::Function(nr),
            ..
        } if is_inside(&nr.element()) => {
            unreachable.get_or_insert_with(|| describe(nr));
        }
        Expression::ElementReference(elem)
            if elem.upgrade().is_some_and(|elem| is_inside(&elem)) =>
        {
            unreachable.get_or_insert_with(|| "an element".to_owned());
        }
        _ => {}
    });
    let mut two_way_bindings = binding.two_way_bindings;
    for two_way in &mut two_way_bindings {
        match two_way {
            TwoWayBinding::ModelData { repeated_element, .. }
                if Weak::ptr_eq(repeated_element, &from) =>
            {
                *repeated_element = to.clone();
            }
            TwoWayBinding::Property { property, .. } if is_inside(&property.element()) => {
                unreachable.get_or_insert_with(|| describe(property));
            }
            _ => {}
        }
    }

    if let Some(what) = unreachable {
        diag.push_error(
            format!("The 'title' of a Tab in an 'if' or 'for' can't use '{what}' from inside the Tab, use the model data or properties outside the Tab instead"),
            &binding.span,
        );
        return Expression::StringLiteral(SmolStr::default()).into();
    }
    let mut title = BindingExpression::from(expression);
    title.span = binding.span;
    title.two_way_bindings = two_way_bindings;
    title
}

/// Reports the changes to the title of the Tab `panel` in an `if` or `for` that happen
/// outside of its binding, since the tab bar entry can't follow them.
fn check_title_is_only_bound(panel: &ElementRc, diag: &mut BuildDiagnostics) {
    let title = NamedReference::new(panel, SmolStr::new_static("title"));
    recurse_elem(panel, &(), &mut |elem, _| {
        for (name, binding) in elem.borrow().real_bindings() {
            let binding = binding.borrow();
            if !(Rc::ptr_eq(elem, panel) && name == "title")
                && binding.two_way_bindings.iter().any(|two_way| two_way.property() == Some(&title))
            {
                diag.push_error(
                    "Can't use '<=>' with the 'title' of a Tab in an 'if' or 'for', use a one-way binding instead".into(),
                    &*binding,
                );
            }
        }
        for state in &elem.borrow().states {
            for (property, _, node) in &state.property_changes {
                if *property == title {
                    diag.push_error(
                        "A state can't change the 'title' of a Tab in an 'if' or 'for', use a condition in the 'title' binding instead".into(),
                        &node.QualifiedName(),
                    );
                }
            }
        }
        visit_element_expressions(elem, |expr, _, _| {
            expr.visit_recursive(&mut |expr| {
                if let Expression::SelfAssignment { lhs, node, .. } = expr
                    && matches!(&**lhs, Expression::PropertyReference(nr) if *nr == title)
                {
                    diag.push_error(
                        "Can't assign the 'title' of a Tab in an 'if' or 'for', bind 'title' to an expression instead".into(),
                        node,
                    );
                }
            })
        });
    });
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
