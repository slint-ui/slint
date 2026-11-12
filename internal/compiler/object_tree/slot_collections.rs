// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

use super::*;

pub(super) fn has_default_collection(component: &Rc<Component>) -> bool {
    component.declared_slots.borrow().iter().any(|s| s.is_default)
        || matches!(&component.root_element.borrow().base_type, ElementType::Component(base) if has_default_collection(base))
}

pub(super) fn declare(slot: &DeclaredSlot, root: &ElementRc, tr: &TypeRegister) {
    let metadata = Element {
        id: slot.name.clone(),
        base_type: tr.empty_type(),
        slot_collection: true,
        debug: root.borrow().debug.clone(),
        ..Default::default()
    }
    .make_rc();
    let length = crate::layout::create_new_prop(root, "slot-length".into(), Type::Int32);
    metadata.borrow_mut().property_declarations.insert(
        "length".into(),
        PropertyDeclaration {
            property_type: Type::Int32,
            visibility: PropertyVisibility::Output,
            is_alias: Some(length),
            ..Default::default()
        },
    );
    root.borrow_mut().children.push(metadata);
}

#[allow(clippy::too_many_arguments)]
pub(super) fn create_expansion(
    node: syntax_nodes::RepeatedElement,
    parent: &ElementRc,
    insertion_points: &mut BTreeMap<String, ChildrenInsertionPoint>,
    slots: &mut Vec<DeclaredSlot>,
    legacy: bool,
    diag: &mut BuildDiagnostics,
    tr: &TypeRegister,
) -> bool {
    let Some(name) = Element::slot_forwarding_expr_identifier(&node.Expression()) else {
        return false;
    };
    let Some(slot) = slots.iter().find(|s| s.is_collection && s.name == name).cloned() else {
        return false;
    };
    let entry_id =
        node.DeclaredIdentifier().and_then(|n| parser::identifier_text(&n)).unwrap_or_default();
    let body = node.SubElement().Element();
    let supplied_name =
        body.QualifiedName().map(QualifiedTypeName::from_node).map(|n| n.to_smolstr());
    if entry_id.is_empty() {
        diag.push_error(
            format!("Expand slot '{name}' with 'for child[index] in {name}: child {{ ... }}'"),
            &node,
        );
        return true;
    }
    if supplied_name.as_ref() != Some(&entry_id) {
        let mut points = BTreeMap::new();
        let mut projection_slots = slots.to_vec();
        let template = Element::from_sub_element_node(
            node.SubElement(),
            parent.borrow().base_type.clone(),
            &mut points,
            &mut projection_slots,
            legacy,
            diag,
            tr,
        );
        Element::reject_slot_placeholders(
            diag,
            &mut projection_slots,
            points,
            "a collection projection",
        );
        let projection_slot: SmolStr =
            format!("$projection-{}-{}", name, insertion_points.len()).into();
        let proxy = Element {
            id: projection_slot.clone(),
            base_type: slot
                .interface
                .clone()
                .map(ElementType::Component)
                .or_else(|| slot.component_type.clone())
                .unwrap_or_else(|| tr.empty_type()),
            debug: template.borrow().debug.clone(),
            ..Default::default()
        }
        .make_rc();
        template.borrow_mut().children.push(proxy);
        let index_property =
            crate::layout::create_new_prop(&template, "slot-index".into(), Type::Int32)
                .name()
                .clone();
        let mut declaration = slot.clone();
        declaration.name = projection_slot.clone();
        declaration.is_collection = false;
        declaration.is_default = false;
        declaration.interface = None;
        declaration.component_type = None;
        slots.push(declaration);
        template.borrow_mut().slot_expansion = Some(SlotExpansion {
            node: node.clone(),
            slot_name: name,
            entry_id,
            index_id: node
                .RepeatedIndex()
                .and_then(|n| parser::identifier_text(&n))
                .unwrap_or_default(),
            index_property,
            projection_slot: Some(projection_slot.clone()),
        });
        insertion_points.insert(
            projection_slot.to_string(),
            ChildrenInsertionPoint {
                parent: parent.clone(),
                insertion_index: parent.borrow().children.len(),
                node: ChildInsertionPointNode::SlotPlaceholder(node.SubElement()),
            },
        );
        parent.borrow_mut().children.push(template);
        return true;
    }
    if node.SubElement().child_token(SyntaxKind::ColonEqual).is_some() {
        diag.push_error(
            "A collection placement cannot declare an element identifier".into(),
            &node,
        );
        return true;
    }
    let insertion_name = slot.insertion_name();
    if let Some(existing) = insertion_points.get(insertion_name) {
        diag.push_error(format!("The slot '{name}' can only appear once in an element"), &node);
        diag.push_note(format!("The slot '{name}' is already used here"), &existing.node);
        return true;
    }
    for child in body.children() {
        if !matches!(
            child.kind(),
            SyntaxKind::QualifiedName
                | SyntaxKind::Binding
                | SyntaxKind::TwoWayBinding
                | SyntaxKind::CallbackConnection
                | SyntaxKind::PropertyChangedCallback
        ) {
            diag.push_error(
                "Slot expansions only accept property bindings and callback connections".into(),
                &child,
            );
        }
    }
    let base = slot
        .interface
        .clone()
        .map(ElementType::Component)
        .or_else(|| slot.component_type.clone())
        .unwrap_or_else(|| tr.empty_type());
    let Some((template, _, _)) = Element::element_without_children(
        &body,
        SmolStr::default(),
        parent.borrow().base_type.clone(),
        legacy,
        diag,
        tr,
        Some(base),
    ) else {
        return true;
    };
    let index_property =
        crate::layout::create_new_prop(&template, "slot-index".into(), Type::Int32).name().clone();
    template.borrow_mut().property_declarations.get_mut(&index_property).unwrap().visibility =
        PropertyVisibility::Output;
    template.borrow_mut().slot_expansion = Some(SlotExpansion {
        node: node.clone(),
        slot_name: name,
        entry_id,
        index_id: node
            .RepeatedIndex()
            .and_then(|n| parser::identifier_text(&n))
            .unwrap_or_default(),
        index_property,
        projection_slot: None,
    });
    insertion_points.insert(
        insertion_name.into(),
        ChildrenInsertionPoint {
            parent: parent.clone(),
            insertion_index: parent.borrow().children.len(),
            node: ChildInsertionPointNode::SlotPlaceholder(node.SubElement()),
        },
    );
    parent.borrow_mut().children.push(template);
    true
}
