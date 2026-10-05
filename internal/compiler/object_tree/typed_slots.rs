// Copyright © 2026 Slint Developers
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

use super::*;

pub(super) fn declaration(
    node: syntax_nodes::SlotDeclaration,
    tr: &TypeRegister,
    diag: &mut BuildDiagnostics,
) -> DeclaredSlot {
    let interface = node.QualifiedName().and_then(|ty| {
        let name = QualifiedTypeName::from_node(ty.clone()).to_smolstr();
        match tr.lookup_element(&name) {
            Ok(ElementType::Component(c)) if c.is_interface() => Some(c),
            Ok(_) => {
                diag.push_error(format!("Slot type '{name}' must be an interface"), &ty);
                None
            }
            Err(error) => {
                diag.push_error(error, &ty);
                None
            }
        }
    });
    let name_node = node.DeclaredIdentifier();
    DeclaredSlot {
        name: parser::identifier_text(&name_node).unwrap_or_default(),
        name_node,
        interface,
        has_rejected_placeholder: false,
    }
}

pub(crate) fn lookup_slot(component: &Rc<Component>, name: &str) -> Option<DeclaredSlot> {
    component.declared_slots.borrow().iter().find(|slot| slot.name == name).cloned().or_else(|| {
        if let ElementType::Component(base) = &component.root_element.borrow().base_type {
            lookup_slot(base, name)
        } else {
            None
        }
    })
}

fn implements(element: &Element, interface: &Rc<Component>) -> bool {
    element.typed_slot_interface.as_ref().is_some_and(|c| Rc::ptr_eq(c, interface))
        || element.implemented_interfaces.iter().any(|i| Rc::ptr_eq(i, &interface.root_element))
        || match &element.base_type {
            ElementType::Component(base) => implements(&base.root_element.borrow(), interface),
            _ => false,
        }
}

pub(super) fn validate_assignment(
    component: &Rc<Component>,
    name: &str,
    element: &ElementRc,
    diag: &mut BuildDiagnostics,
) {
    let Some(interface) = lookup_slot(component, name).and_then(|s| s.interface) else { return };
    if !implements(&element.borrow(), &interface) {
        diag.push_error(
            format!("Slot '{name}' requires a component implementing '{}'", interface.id),
            &*element.borrow(),
        );
    } else if let Some(debug) = element.borrow().debug.first() {
        interfaces::validate_interface_implementation(
            &element.borrow(),
            &interface.root_element,
            &interface.id,
            &debug.node.clone().into(),
            &interfaces::ImplementBinding::OnSelf,
            diag,
        );
    }
}

#[allow(clippy::too_many_arguments)]
pub(super) fn create_placeholder(
    node: &SyntaxNode,
    name: &SmolStr,
    parent: &ElementRc,
    insertion_points: &mut BTreeMap<String, ChildrenInsertionPoint>,
    slots: &[DeclaredSlot],
    legacy: bool,
    diag: &mut BuildDiagnostics,
    tr: &TypeRegister,
) -> bool {
    let Some(interface) = slots.iter().find(|s| &s.name == name).and_then(|s| s.interface.clone())
    else {
        return false;
    };
    if insertion_points.contains_key(name.as_str()) {
        diag.push_error(format!("The slot '{name}' can only appear once in an element"), node);
        return true;
    }
    let element_node: syntax_nodes::Element = node.child_node(SyntaxKind::Element).unwrap().into();
    for child in element_node.children() {
        if !matches!(
            child.kind(),
            SyntaxKind::QualifiedName
                | SyntaxKind::Binding
                | SyntaxKind::CallbackConnection
                | SyntaxKind::TwoWayBinding
                | SyntaxKind::PropertyChangedCallback
        ) {
            diag.push_error(
                "Typed slot placeholders only accept property bindings and callback connections"
                    .into(),
                &child,
            );
        }
    }
    let Some((proxy, _, _)) = Element::element_without_children(
        &element_node,
        name.clone(),
        parent.borrow().base_type.clone(),
        legacy,
        diag,
        tr,
        Some(interface.clone()),
    ) else {
        return true;
    };
    proxy.borrow_mut().typed_slot_interface = Some(interface);
    insertion_points.insert(
        name.to_string(),
        ChildrenInsertionPoint {
            parent: proxy.clone(),
            insertion_index: 0,
            default_children_count: 0,
            node: ChildInsertionPointNode::SlotPlaceholder(node.clone().into()),
        },
    );
    parent.borrow_mut().children.push(proxy);
    true
}

#[allow(clippy::too_many_arguments)]
pub(super) fn create_forwarding(
    parent: &ElementRc,
    target: &SmolStr,
    source: &SmolStr,
    node: &SyntaxNode,
    insertion_points: &mut BTreeMap<String, ChildrenInsertionPoint>,
    slots: &[DeclaredSlot],
    diag: &mut BuildDiagnostics,
) -> bool {
    let source_slot = slots.iter().find(|slot| &slot.name == source);
    let target_slot = match &parent.borrow().base_type {
        ElementType::Component(c) => lookup_slot(c, target),
        _ => None,
    };
    let source_interface = source_slot.and_then(|s| s.interface.clone());
    let target_interface = target_slot.and_then(|s| s.interface);
    if source_interface.is_none() && target_interface.is_none() {
        return false;
    }
    let Some(interface) = source_interface else {
        diag.push_error(
            format!("Declare slot '{source}' with the interface required by '{target}'"),
            node,
        );
        return true;
    };
    if target_interface.as_ref().is_some_and(|target| !Rc::ptr_eq(target, &interface)) {
        diag.push_error(format!("Forwarded slot '{source}' has an incompatible interface"), node);
        return true;
    }
    if insertion_points.contains_key(source.as_str()) {
        diag.push_error(format!("The slot '{source}' can only appear once in an element"), node);
        return true;
    }
    if parent.borrow().children.iter().any(|c| c.borrow().slot_target.as_ref() == Some(target)) {
        diag.push_error(format!("Duplicate assignment to slot '{target}'"), node);
        return true;
    }
    let proxy = Element {
        id: source.clone(),
        base_type: ElementType::Component(interface.clone()),
        typed_slot_interface: Some(interface),
        slot_target: Some(target.clone()),
        debug: parent.borrow().debug.clone(),
        ..Default::default()
    }
    .make_rc();
    insertion_points.insert(
        source.to_string(),
        ChildrenInsertionPoint {
            parent: proxy.clone(),
            insertion_index: 0,
            default_children_count: 0,
            node: ChildInsertionPointNode::SlotForwarding(node.clone().into()),
        },
    );
    parent.borrow_mut().children.push(proxy);
    true
}
