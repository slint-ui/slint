// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

use crate::diagnostics::BuildDiagnostics;
use crate::langtype::ElementType;
use crate::typeloader::TypeLoader;
use crate::{expression_tree, object_tree::*};
use std::cell::RefCell;
use std::rc::Rc;

pub fn lower_component_container(
    doc: &Document,
    type_loader: &mut TypeLoader,
    diag: &mut BuildDiagnostics,
) {
    let empty_type = type_loader.global_type_registry.borrow().empty_type();

    let mut new_components = vec![];

    doc.visit_all_used_components(|component| {
        recurse_elem_including_sub_components_no_borrow(component, &(), &mut |elem, _| {
            if matches!(&elem.borrow().builtin_type(), Some(b) if b.name == "ComponentContainer") {
                diagnose_component_container(elem, diag);
                process_component_container(elem, &empty_type, &mut new_components);
            }
        })
    });
}

fn diagnose_component_container(element: &ElementRc, diag: &mut BuildDiagnostics) {
    let elem = element.borrow();
    if !elem.children.is_empty() {
        diag.push_error("ComponentContainers may not have children".into(), &*element.borrow());
    }
    if let Some(cip) =
        elem.enclosing_component.upgrade().unwrap().child_insertion_point.borrow().clone()
    {
        if Rc::ptr_eq(&cip.parent, element) {
            diag.push_error(
                "The @children placeholder cannot appear in a ComponentContainer".into(),
                &*element.borrow(),
            );
        }
    }
}

fn process_component_container(
    element: &ElementRc,
    empty_type: &ElementType,
    new_components: &mut Vec<Rc<Component>>,
) {
    let component = Rc::new_cyclic(|component_weak| {
        let root_element = Rc::new(RefCell::new(Element {
            id: smol_str::format_smolstr!("component_container_internal_{}", new_components.len()),
            base_type: empty_type.clone(),
            enclosing_component: component_weak.clone(),
            ..Default::default()
        }));

        Component {
            node: element.borrow().debug.first().map(|n| n.node.clone().into()),
            id: smol_str::format_smolstr!("ComponentContainerInternal_{}", new_components.len()),
            root_element,
            ..Default::default()
        }
    });

    let mut elem = element.borrow_mut();

    let embedded_element = Element::make_rc(Element {
        base_type: ElementType::Component(component.clone()),
        id: smol_str::format_smolstr!("component_container_placeholder_{}", new_components.len()),
        debug: elem.debug.clone(),
        enclosing_component: elem.enclosing_component.clone(),
        default_fill_parent: (true, true),
        inline_depth: elem.inline_depth,
        repeated: Some(RepeatedElementInfo {
            model: expression_tree::Expression::BoolLiteral(false),
            model_data_id: Default::default(),
            index_id: Default::default(),
            is_conditional_element: true,
            is_listview: None,
        }),
        is_component_placeholder: true,
        ..Default::default()
    });
    elem.children.push(embedded_element);

    new_components.push(component);
}
