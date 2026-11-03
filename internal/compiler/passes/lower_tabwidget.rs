// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

// cSpell: ignore tabwidget

//! This pass lowers the TabWidget to create the tabbar.
//!
//! Must be done before inlining and many other passes because the lowered code must
//! be further inlined as it may expand to a native widget that needs inlining

use crate::diagnostics::{BuildDiagnostics, Spanned};
use crate::element_type::{ElementType, PropertyLookupMode};
use crate::expression_tree::{BindingExpression, Callable, Expression, NamedReference};
use crate::expression_types::{MinMaxOp, Unit};
use crate::langtype::Type;
use crate::object_tree::*;
use smol_str::{SmolStr, format_smolstr};
use std::collections::HashSet;
use std::rc::Rc;

pub async fn lower_tabwidget(
    doc: &Document,
    type_loader: &mut crate::typeloader::TypeLoader,
    forwarded_references: &mut forward_inherited_expression::ForwardedReferenceCache,
    diag: &mut BuildDiagnostics,
) {
    doc.visit_all_used_components(|component| {
        error_on_slot_in_inner_builtin(component, &["TabWidget"], diag);
    });
    let mut tab_widgets = collect_tab_widgets(doc);
    if tab_widgets.is_empty() {
        return;
    }

    loop {
        let mut expanded = false;
        for elem in &tab_widgets {
            let bases = tabwidget_bases(elem);
            if !bases.iter().any(|base| base.root_element.borrow().children.iter().any(is_tab))
                && bases.iter().any(|base| base.root_element.borrow().children.iter().any(is_bar))
            {
                inline_bar_bases(elem, doc, type_loader, forwarded_references, diag);
                expanded = true;
            }
        }
        if !expanded {
            break;
        }
        // Inlining copies nested widgets, so the old collection doesn't contain every instance.
        tab_widgets = collect_tab_widgets(doc);
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
            if !tabwidget_bases(elem).iter().any(|base| {
                base.root_element.borrow().children.iter().any(is_tab)
            }) {
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
    let custom = tab_widgets.iter().any(|widget| {
        widget.borrow().children.iter().any(|child| {
            child.borrow().slot_target.as_deref() == Some("bar")
                || child
                    .borrow()
                    .children
                    .iter()
                    .any(|header| header.borrow().slot_target.as_deref() == Some("header"))
        })
    });
    let mut contracts = Vec::new();
    if custom {
        for name in ["TabHeaderInterface", "TabBarInterface"] {
            if let Some(interface) =
                type_loader.import_component("std-widget-interfaces.slint", name, diag).await
            {
                contracts.push(interface);
            }
        }
        if contracts.len() != 2 {
            return;
        }
    }

    for elem in &tab_widgets {
        let bar = elem
            .borrow()
            .children
            .iter()
            .find(|child| child.borrow().slot_target.as_deref() == Some("bar"))
            .cloned();
        if let Some(bar) = bar {
            if !validate_slot(&bar, &contracts[1], diag) {
                elem.borrow_mut().children.retain(|child| !Rc::ptr_eq(child, &bar));
            } else if bar
                .borrow()
                .base_type
                .lookup_type_for_child_element("Rectangle", &doc.local_registry)
                .is_err()
            {
                diag.push_error(
                    "Place '@children' in the custom tab bar's header layout".into(),
                    &*bar.borrow(),
                );
                elem.borrow_mut().children.retain(|child| !Rc::ptr_eq(child, &bar));
            }
        }
        process_tabwidget(
            elem,
            ElementType::Component(tabwidget_impl.clone()),
            ElementType::Component(tab_impl.clone()),
            ElementType::Component(tabbar_horizontal_impl.clone()),
            ElementType::Component(tabbar_vertical_impl.clone()),
            &empty_type,
            &contracts,
            &type_loader.symbol_counters,
            forwarded_references,
            diag,
        );
    }
}

fn collect_tab_widgets(doc: &Document) -> Vec<ElementRc> {
    let mut seen = HashSet::new();
    let mut widgets = Vec::new();
    doc.visit_all_used_components(|component| {
        recurse_elem_including_sub_components_no_borrow(component, &(), &mut |elem, _| {
            if matches!(elem.borrow().builtin_type(), Some(builtin) if builtin.name == "TabWidget")
                && seen.insert(Rc::as_ptr(elem))
            {
                widgets.push(elem.clone());
            }
        });
    });
    widgets
}

fn is_bar(elem: &ElementRc) -> bool {
    elem.borrow().slot_target.as_deref() == Some("bar")
}

fn is_tab(elem: &ElementRc) -> bool {
    matches!(elem.borrow().builtin_type(), Some(builtin) if builtin.name == "Tab")
}

fn tabwidget_bases(elem: &ElementRc) -> Vec<Rc<Component>> {
    let mut bases = Vec::new();
    let mut base_type = elem.borrow().base_type.clone();
    while let ElementType::Component(base) = base_type {
        base_type = base.root_element.borrow().base_type.clone();
        bases.push(base);
    }
    bases
}

fn inline_bar_bases(
    elem: &ElementRc,
    doc: &Document,
    type_loader: &crate::typeloader::TypeLoader,
    forwarded_references: &mut forward_inherited_expression::ForwardedReferenceCache,
    diag: &mut BuildDiagnostics,
) {
    let ElementType::Component(base) = elem.borrow().base_type.clone() else { return };
    inline_bar_bases(&base.root_element, doc, type_loader, forwarded_references, diag);
    super::lower_states::lower_states(
        &base,
        &doc.local_registry,
        &type_loader.symbol_counters,
        forwarded_references,
        type_loader.compiler_config.edition,
        diag,
    );
    super::collect_init_code::collect_init_code(&base);

    // Builtin TabWidget slots have no declared insertion points for the generic inliner.
    let bar = base.root_element.borrow().children.iter().position(is_bar);
    if let Some(index) = bar {
        base.root_element.borrow().children[index].borrow_mut().slot_default = Some("bar".into());
    }
    base.child_insertion_points.borrow_mut().insert(
        "bar".into(),
        ChildrenInsertionPoint {
            parent: base.root_element.clone(),
            insertion_index: bar.unwrap_or(0),
            node: ChildInsertionPointNode::DefaultChildrenPlaceHolder(
                base.node.as_ref().unwrap().clone().into(),
            ),
        },
    );
    let component = elem.borrow().enclosing_component.upgrade().unwrap();
    super::inlining::inline_element(elem, &base, &component, diag);
    base.child_insertion_points.borrow_mut().remove("bar");
    if let Some(index) = bar {
        base.root_element.borrow().children[index].borrow_mut().slot_default = None;
    }
    let mut init_code = component.init_code.borrow_mut();
    if let Some(inlined) = init_code.inlined_init_code.remove(&elem.borrow().span().offset) {
        init_code.constructor_code.extend(inlined);
    }
}

fn process_tabwidget(
    elem: &ElementRc,
    tabwidget_impl: ElementType,
    tab_impl: ElementType,
    tabbar_horizontal_impl: ElementType,
    tabbar_vertical_impl: ElementType,
    empty_type: &ElementType,
    contracts: &[Rc<Component>],
    symbol_counters: &crate::symbol_counters::SymbolCounters,
    forwarded_references: &mut forward_inherited_expression::ForwardedReferenceCache,
    diag: &mut BuildDiagnostics,
) {
    elem.borrow_mut().base_type = tabwidget_impl;
    let mut original = std::mem::take(&mut elem.borrow_mut().children);
    let custom_bar = take_slot(&mut original, "bar");
    let has_custom_bar = custom_bar.is_some();
    let (timers, mut children): (Vec<_>, Vec<_>) = original.into_iter().partition(
        |child| matches!(&child.borrow().base_type, ElementType::Builtin(b) if b.name == "Timer"),
    );
    let num_tabs = children.len();
    let mut tabs = Vec::new();
    let mut custom_headers = Vec::new();
    for child in &mut children {
        error_on_slot_default(child, "TabWidget", diag);
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
        let custom_header = take_slot(&mut child.borrow_mut().children, "header");
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

        if let Some(header) = custom_header {
            if !validate_slot(&header, &contracts[0], diag) {
                continue;
            }
            if !has_custom_bar {
                diag.push_error(
                    "Provide a custom 'bar' to use a custom 'header'".into(),
                    &*header.borrow(),
                );
                continue;
            }
            let title = member_reference(&header, "title");
            let selected = member_reference(&header, "selected");
            let focused = member_reference(&header, "focused");
            let mut h = header.borrow_mut();
            h.set_binding(
                title.name().clone(),
                Expression::PropertyReference(NamedReference::new(child, "title".into())).into(),
            );
            h.set_binding(
                selected.name().clone(),
                Expression::BinaryExpression {
                    lhs: Expression::PropertyReference(NamedReference::new(
                        elem,
                        "current-index".into(),
                    ))
                    .into(),
                    rhs: Expression::NumberLiteral(index as _, Unit::None).into(),
                    op: '=',
                    source_location: None,
                }
                .into(),
            );
            h.set_binding(
                focused.name().clone(),
                Expression::BinaryExpression {
                    lhs: Expression::PropertyReference(NamedReference::new(
                        elem,
                        "current-focused".into(),
                    ))
                    .into(),
                    rhs: Expression::NumberLiteral(index as _, Unit::None).into(),
                    op: '=',
                    source_location: None,
                }
                .into(),
            );
            h.set_binding(
                "accessible-item-index".into(),
                Expression::NumberLiteral(index as _, Unit::None).into(),
            );
            drop(h);
            custom_headers.push((header.clone(), child.clone(), index));
            tabs.push(header);
            continue;
        }
        let mut tab = Element {
            id: format_smolstr!("{}-tab{}", elem.borrow().id, index),
            base_type: tab_impl.clone(),
            enclosing_component: elem.borrow().enclosing_component.clone(),
            ..Default::default()
        };
        tab.set_binding(
            SmolStr::new_static("title"),
            BindingExpression::new_two_way(
                NamedReference::new(child, SmolStr::new_static("title")).into(),
            ),
        );
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
        tab.set_binding(
            SmolStr::new_static("tab-index"),
            Expression::NumberLiteral(index as _, Unit::None).into(),
        );
        tab.set_binding(
            SmolStr::new_static("num-tabs"),
            Expression::NumberLiteral(num_tabs as _, Unit::None).into(),
        );
        tabs.push(Element::make_rc(tab));
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
    let tabbar = if let Some(bar) = custom_bar {
        bar.borrow_mut().children.extend(tabs);
        bar
    } else {
        Element {
            id: format_smolstr!("{}-tabbar", elem.borrow().id),
            base_type: tabbar_impl,
            enclosing_component: elem.borrow().enclosing_component.clone(),
            children: tabs,
            ..Default::default()
        }
        .make_rc()
    };
    for (header, panel, index) in custom_headers {
        let current_index = NamedReference::new(elem, "current-index".into());
        current_index.mark_as_set();
        let activated = member_reference(&header, "activated");
        let handler = preserve_callback_handler(
            &activated,
            &mut HashSet::new(),
            symbol_counters,
            forwarded_references,
        );
        let mut activate = vec![
            Expression::SelfAssignment {
                lhs: Expression::PropertyReference(current_index).into(),
                rhs: Expression::NumberLiteral(index as _, Unit::None).into(),
                op: '=',
                node: None,
            },
            Expression::FunctionCall {
                function: Callable::Function(member_reference(&tabbar, "focus-tab")),
                arguments: vec![Expression::NumberLiteral(index as _, Unit::None)],
                source_location: None,
            },
        ];
        if let Some(handler) = handler {
            activate.push(handler);
        }
        let mut binding: BindingExpression = Expression::CodeBlock(activate).into();
        if let Some(original) = header.borrow().binding(activated.name()) {
            binding.two_way_bindings = original.two_way_bindings.clone();
        }
        header.borrow_mut().set_binding(activated.name().clone(), binding);
        let role = crate::typeregister::BUILTIN
            .enums
            .AccessibleRole
            .clone()
            .try_value_from_string("tab")
            .unwrap();
        let selected = member_reference(&header, "selected");
        let mut h = header.borrow_mut();
        h.set_binding("accessible-role".into(), Expression::EnumerationValue(role).into());
        h.set_binding(
            "accessible-label".into(),
            Expression::PropertyReference(NamedReference::new(&panel, "title".into())).into(),
        );
        h.set_binding("accessible-item-selectable".into(), Expression::BoolLiteral(true).into());
        h.set_binding(
            "accessible-item-selected".into(),
            Expression::PropertyReference(selected).into(),
        );
        h.set_binding_if_not_set("accessible-action-default".into(), || Expression::FunctionCall {
            function: Callable::Callback(activated),
            arguments: vec![],
            source_location: None,
        });
    }
    if has_custom_bar {
        let Type::Array(item_type) = tabbar
            .borrow()
            .lookup_property("tabs", PropertyLookupMode::ComponentLocal)
            .property_type
        else {
            return;
        };
        let Type::Struct(item_type) = &*item_type else { return };
        let model = Expression::Array {
            element_ty: Type::Struct(item_type.clone()),
            values: children
                .iter()
                .enumerate()
                .map(|(index, panel)| Expression::Struct {
                    ty: item_type.clone(),
                    values: [
                        ("index".into(), Expression::NumberLiteral(index as _, Unit::None)),
                        (
                            "title".into(),
                            Expression::PropertyReference(NamedReference::new(
                                panel,
                                "title".into(),
                            )),
                        ),
                    ]
                    .into_iter()
                    .collect(),
                })
                .collect(),
        };
        let tabs = member_reference(&tabbar, "tabs");
        let orientation = member_reference(&tabbar, "orientation");
        tabbar.borrow_mut().set_binding(tabs.name().clone(), model.into());
        tabbar.borrow_mut().set_binding(
            orientation.name().clone(),
            Expression::PropertyReference(NamedReference::new(elem, "orientation".into())).into(),
        );
    } else {
        tabbar.borrow_mut().set_binding(
            "num-tabs".into(),
            Expression::NumberLiteral(num_tabs as _, Unit::None).into(),
        );
    }
    set_tabbar_geometry_prop(elem, &tabbar, "x");
    set_tabbar_geometry_prop(elem, &tabbar, "y");
    set_tabbar_geometry_prop(elem, &tabbar, "width");
    set_tabbar_geometry_prop(elem, &tabbar, "height");
    let current =
        member_reference(&tabbar, if has_custom_bar { "current-index" } else { "current" });
    tabbar.borrow_mut().set_binding(
        current.name().clone(),
        BindingExpression::new_two_way(
            NamedReference::new(elem, SmolStr::new_static("current-index")).into(),
        ),
    );
    elem.borrow_mut().set_binding(
        SmolStr::new_static("current-focused"),
        BindingExpression::new_two_way(
            member_reference(
                &tabbar,
                if has_custom_bar { "focused-index" } else { "current-focused" },
            )
            .into(),
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

    if let Some(expr) = children
        .iter()
        .map(|x| {
            Expression::PropertyReference(NamedReference::new(x, SmolStr::new_static("min-width")))
        })
        .reduce(|lhs, rhs| crate::builtin_macros::min_max_expression(lhs, rhs, MinMaxOp::Max))
    {
        elem.borrow_mut().set_binding("content-min-width".into(), expr.into());
    };
    if let Some(expr) = children
        .iter()
        .map(|x| {
            Expression::PropertyReference(NamedReference::new(x, SmolStr::new_static("min-height")))
        })
        .reduce(|lhs, rhs| crate::builtin_macros::min_max_expression(lhs, rhs, MinMaxOp::Max))
    {
        elem.borrow_mut().set_binding("content-min-height".into(), expr.into());
    };

    elem.borrow_mut().children = std::iter::once(tabbar).chain(children).chain(timers).collect();
}

fn member_reference(element: &ElementRc, name: &str) -> NamedReference {
    let name = element
        .borrow()
        .lookup_property(name, PropertyLookupMode::ComponentLocal)
        .internal_or_resolved_name();
    NamedReference::new(element, name)
}

#[allow(clippy::mutable_key_type)]
fn preserve_callback_handler(
    reference: &NamedReference,
    visited: &mut HashSet<NamedReference>,
    symbol_counters: &crate::symbol_counters::SymbolCounters,
    forwarded_references: &mut forward_inherited_expression::ForwardedReferenceCache,
) -> Option<Expression> {
    if !visited.insert(reference.clone()) {
        return None;
    }
    let element = reference.element();
    let binding = element.borrow().binding(reference.name()).map(|binding| binding.clone());
    if let Some(binding) = binding {
        if !matches!(binding.value_expression(), Expression::Invalid) {
            let handler = crate::layout::create_new_prop(
                &element,
                "tab-header-activated".into(),
                reference.ty(),
            );
            element
                .borrow_mut()
                .set_binding(handler.name().clone(), binding.value_expression().clone().into());
            return Some(Expression::FunctionCall {
                function: Callable::Callback(handler),
                arguments: vec![],
                source_location: None,
            });
        }
        for alias in &binding.two_way_bindings {
            if let Some(alias) = alias.property()
                && let Some(handler) =
                    preserve_callback_handler(alias, visited, symbol_counters, forwarded_references)
            {
                return Some(handler);
            }
        }
    }
    let ElementType::Component(base) = element.borrow().base_type.clone() else { return None };
    let mut handler = preserve_callback_handler(
        &NamedReference::new(&base.root_element, reference.name().clone()),
        visited,
        symbol_counters,
        forwarded_references,
    )?;
    forward_inherited_expression::rebase_expression_to_instance(
        &mut handler,
        &base.root_element,
        &element,
        symbol_counters,
        forwarded_references,
    );
    Some(handler)
}

fn take_slot(children: &mut Vec<ElementRc>, name: &str) -> Option<ElementRc> {
    let index =
        children.iter().position(|child| child.borrow().slot_target.as_deref() == Some(name))?;
    let child = children.remove(index);
    child.borrow_mut().slot_target = None;
    Some(child)
}

fn validate_slot(
    element: &ElementRc,
    interface: &Rc<Component>,
    diag: &mut BuildDiagnostics,
) -> bool {
    let element = element.borrow();
    let Some(debug) = element.debug.first() else { return false };
    interfaces::validate_interface_implementation(
        &element,
        &interface.root_element,
        &interface.id,
        &debug.node.clone().into(),
        &interfaces::ImplementBinding::OnSelf,
        diag,
    )
}

fn set_geometry_prop(
    tab_widget: &ElementRc,
    content: &ElementRc,
    prop: &str,
    diag: &mut BuildDiagnostics,
) {
    let old = content.borrow_mut().set_binding(
        prop.into(),
        Expression::PropertyReference(NamedReference::new(
            tab_widget,
            format_smolstr!("content-{}", prop),
        ))
        .into(),
    );
    if let Some(old) = old {
        diag.push_error(
            format!("The property '{prop}' cannot be set for Tabs inside a TabWidget"),
            &old,
        );
    }
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
