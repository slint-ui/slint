// Copyright © SixtyFPS GmbH <info@slint.dev>
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

pub(crate) fn validate_assignments(root: &ElementRc, diag: &mut BuildDiagnostics) {
    recurse_elem(root, &(), &mut |parent, _| {
        let parent = parent.borrow();
        let ElementType::Component(component) = &parent.base_type else { return };
        for child in &parent.children {
            let name = child.borrow().slot_target.clone();
            if let Some(name) = name {
                validate_assignment(component, &name, child, diag);
            }
        }
    });
}

pub(super) fn validate_assignment(
    component: &Rc<Component>,
    name: &str,
    element: &ElementRc,
    diag: &mut BuildDiagnostics,
) {
    let Some(interface) = lookup_slot(component, name).and_then(|s| s.interface) else { return };
    if let Some(debug) = element.borrow().debug.first() {
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
    if let Some(existing) = insertion_points.get(name.as_str()) {
        diag.push_error(format!("The slot '{name}' can only appear once in an element"), node);
        diag.push_note(format!("The slot '{name}' is already used here"), &existing.node);
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
        return true;
    }
    let Some(interface) = source_interface else {
        diag.push_error(
            format!("Declare slot '{source}' with the interface required by '{target}'"),
            node,
        );
        if let Some(slot) = source_slot {
            diag.push_note(format!("The slot '{source}' is declared here"), &slot.name_node);
        }
        return false;
    };
    if target_interface.as_ref().is_some_and(|target| !Rc::ptr_eq(target, &interface)) {
        diag.push_error(format!("Forwarded slot '{source}' has an incompatible interface"), node);
        return false;
    }
    if let Some(existing) = insertion_points.get(source.as_str()) {
        diag.push_error(format!("The slot '{source}' can only appear once in an element"), node);
        diag.push_note(format!("The slot '{source}' is already used here"), &existing.node);
        return false;
    }
    if parent.borrow().children.iter().any(|c| c.borrow().slot_target.as_ref() == Some(target)) {
        diag.push_error(format!("Duplicate assignment to slot '{target}'"), node);
        return false;
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
            node: ChildInsertionPointNode::SlotForwarding(node.clone().into()),
        },
    );
    parent.borrow_mut().children.push(proxy);
    false
}

impl Element {
    pub(super) fn apply_slots(
        node: &syntax_nodes::Element,
        r: &ElementRc,
        component_child_insertion_points: &mut BTreeMap<String, ChildrenInsertionPoint>,
        declared_slots: &mut Vec<DeclaredSlot>,
        diag: &mut BuildDiagnostics,
        tr: &TypeRegister,
    ) {
        for declaration in node.SlotDeclaration() {
            Self::assert_experimental_slots(diag, &declaration, "named slots");
            declared_slots.push(self::declaration(declaration, tr, diag));
        }

        for se in node.children() {
            if se.kind() != SyntaxKind::SlotForwarding {
                continue;
            }
            if !Self::assert_experimental_slots(diag, &se, "slot forwarding") {
                continue;
            }

            let target_node = se.child_node(SyntaxKind::DeclaredIdentifier).unwrap();
            let target = parser::identifier_text(&target_node.clone()).unwrap_or_default();

            if target == "children" {
                diag.push_error(
                    format!(
                        "The name '{target}' is reserved for the default slot. Use @children instead"
                    ),
                    &target_node,
                );
                continue;
            }

            if r.borrow().forwarded_slots.iter().any(|f| f.target == target) {
                diag.push_error(format!("Duplicate assignment to slot '{target}'"), &target_node);
                continue;
            }

            match &r.borrow().base_type {
                ElementType::Component(component) if lookup_slot(component, &target).is_none() => {
                    diag.push_error(
                        format!("Unknown slot '{target}' in '{}'", component.id),
                        &target_node,
                    );
                    continue;
                }
                ElementType::Component(_) => {}
                _ => {
                    diag.push_error("Slot forwarding can only be used on components".into(), &se);
                    continue;
                }
            }

            let Some(expression_node) = se.child_node(SyntaxKind::Expression) else {
                diag.push_error(
                    "Slot forwarding requires a slot identifier on the right-hand side".into(),
                    &se,
                );
                continue;
            };
            let Some(source) = Self::slot_forwarding_expr_identifier(&expression_node) else {
                diag.push_error(
                    "Slot forwarding requires a slot identifier on the right-hand side".into(),
                    &expression_node,
                );
                continue;
            };

            if source == "children" {
                diag.push_error(
                    format!(
                        "The name '{source}' is reserved for the default slot. Use @children instead"
                    ),
                    &expression_node,
                );
                continue;
            }

            if create_forwarding(
                r,
                &target,
                &source,
                &expression_node,
                component_child_insertion_points,
                declared_slots,
                diag,
            ) {
                r.borrow_mut().forwarded_slots.push(SlotForwarding {
                    target,
                    source,
                    expression_node: expression_node.into(),
                });
            }
        }

        for forwarding in r.borrow().forwarded_slots.clone() {
            let source = forwarding.source.clone();
            if let Some(existing_cip) = component_child_insertion_points.get(source.as_str()) {
                if matches!(existing_cip.node, ChildInsertionPointNode::SlotPlaceholder(_)) {
                    diag.push_error(
                        format!(
                            "The slot '{source}' cannot be forwarded and used as a placeholder in the same component"
                        ),
                        &forwarding.expression_node,
                    );
                } else {
                    diag.push_error(
                        format!(
                            "{} can only appear once in an element",
                            slot_error_subject(&source)
                        ),
                        &forwarding.expression_node,
                    );
                }
                continue;
            }
            component_child_insertion_points.insert(
                source.to_string(),
                ChildrenInsertionPoint {
                    parent: r.clone(),
                    insertion_index: 0,
                    node: ChildInsertionPointNode::SlotForwarding(forwarding.expression_node),
                },
            );
        }
    }

    pub(super) fn add_children_placeholder(
        se: SyntaxNode,
        r: &ElementRc,
        component_child_insertion_points: &mut BTreeMap<String, ChildrenInsertionPoint>,
        diag: &mut BuildDiagnostics,
    ) {
        #[cfg(feature = "slint-sc")]
        diag.slint_sc_error("The @children placeholder is", &se);
        if component_child_insertion_points.contains_key(DEFAULT_SLOT_NAME) {
            diag.push_error(
                format!(
                    "{} can only appear once in an element",
                    slot_error_subject(DEFAULT_SLOT_NAME)
                ),
                &se,
            );
        } else {
            component_child_insertion_points.insert(
                DEFAULT_SLOT_NAME.into(),
                ChildrenInsertionPoint {
                    parent: r.clone(),
                    insertion_index: r.borrow().children.len(),
                    node: ChildInsertionPointNode::ChildrenPlaceHolder(se.into()),
                },
            );
        }
    }

    pub(super) fn add_slot_assignment(
        se: SyntaxNode,
        r: &ElementRc,
        assigned_slots: &mut HashSet<SmolStr>,
        component_child_insertion_points: &mut BTreeMap<String, ChildrenInsertionPoint>,
        declared_slots: &mut Vec<DeclaredSlot>,
        is_legacy_syntax: bool,
        diag: &mut BuildDiagnostics,
        tr: &TypeRegister,
    ) {
        if !Self::assert_experimental_slots(diag, &se, "named slots") {
            return;
        }
        let name_node = se.child_node(SyntaxKind::DeclaredIdentifier).unwrap();
        let name = parser::identifier_text(&name_node).unwrap_or_default();
        if name == "children" {
            diag.push_error(
                format!(
                    "The name '{name}' is reserved for the default slot. Use @children instead"
                ),
                &name_node,
            );
        }
        if !assigned_slots.insert(name.clone()) {
            diag.push_error(format!("Duplicate assignment to slot '{name}'"), &name_node);
        }
        if r.borrow().forwarded_slots.iter().any(|f| f.target == name) {
            diag.push_error(format!("Duplicate assignment to slot '{name}'"), &name_node);
        }
        let sub_element_node = se.child_node(SyntaxKind::SubElement).unwrap();
        let parent_type = r.borrow().base_type.clone();
        match &parent_type {
            ElementType::Component(component) if lookup_slot(component, &name).is_none() => {
                diag.push_error(format!("Unknown slot '{name}' in '{}'", component.id), &name_node);
            }
            ElementType::Component(_) => {}
            _ => {
                diag.push_error("Slot assignments can only be used on components".to_string(), &se);
            }
        }
        let parent_type = match &parent_type {
            ElementType::Component(component)
                if lookup_slot(component, &name).is_some_and(|slot| slot.interface.is_some()) =>
            {
                tr.empty_type()
            }
            _ => parent_type,
        };
        let element = Element::from_sub_element_node(
            sub_element_node.into(),
            parent_type,
            component_child_insertion_points,
            declared_slots,
            is_legacy_syntax,
            diag,
            tr,
        );
        element.borrow_mut().slot_target = Some(name);
        r.borrow_mut().children.push(element);
    }

    pub(super) fn assert_experimental_slots(
        diagnostics: &mut BuildDiagnostics,
        node: &SyntaxNode,
        what: &str,
    ) -> bool {
        if diagnostics.enable_experimental {
            return true;
        }
        diagnostics.push_error(format!("'{what}' is an experimental feature"), node);
        false
    }

    pub(super) fn sub_element_slot_placeholder_name(
        node: &SyntaxNode,
        declared_slots: &[DeclaredSlot],
    ) -> Option<SmolStr> {
        if node.child_token(SyntaxKind::ColonEqual).is_some() {
            return None;
        }
        let element = node.child_node(SyntaxKind::Element)?;

        let qualified_name = element.child_node(SyntaxKind::QualifiedName)?;
        if qualified_name.child_token(SyntaxKind::Dot).is_some() {
            return None;
        }
        let name = parser::identifier_text(&qualified_name)?;
        declared_slots
            .iter()
            .any(|slot| {
                slot.name == name
                    && (slot.interface.is_some()
                        || !element.children().any(|c| c.kind() != SyntaxKind::QualifiedName))
            })
            .then_some(name)
    }

    pub(super) fn reject_slot_placeholders(
        diagnostics: &mut BuildDiagnostics,
        declared_slots: &mut [DeclaredSlot],
        insertion_points: BTreeMap<String, ChildrenInsertionPoint>,
        context: &str,
    ) {
        for (name, ChildrenInsertionPoint { node, .. }) in insertion_points {
            Self::mark_placeholder_rejected(declared_slots, &name);
            diagnostics.push_error(
                format!("{} cannot appear in {context}", slot_error_subject(&name)),
                &node,
            );
        }
    }

    pub(super) fn register_slot_placeholder(
        node: &SyntaxNode,
        slot_name: SmolStr,
        parent: &ElementRc,
        component_child_insertion_points: &mut BTreeMap<String, ChildrenInsertionPoint>,
        diagnostics: &mut BuildDiagnostics,
        type_register: &TypeRegister,
    ) {
        Self::assert_experimental_slots(diagnostics, node, "named slots");
        if let Some(existing) = component_child_insertion_points.get(slot_name.as_str()) {
            if matches!(existing.node, ChildInsertionPointNode::SlotForwarding(_)) {
                diagnostics.push_error(
                    format!(
                        "The slot '{slot_name}' cannot be forwarded and used as a placeholder in the same component"
                    ),
                    node,
                );
            } else {
                diagnostics.push_error(
                    format!(
                        "{} can only appear once in an element",
                        slot_error_subject(&slot_name)
                    ),
                    node,
                );
            }
            return;
        }
        if type_register.lookup_element(slot_name.as_str()).is_ok() {
            diagnostics.push_warning(
                format!(
                    "{} shadows an element type of the same name. This element is a slot placeholder, not an instance of '{slot_name}'",
                    slot_error_subject(&slot_name)
                ),
                node,
            );
        }
        let insertion_index = parent.borrow().children.len();
        component_child_insertion_points.insert(
            slot_name.to_string(),
            ChildrenInsertionPoint {
                parent: parent.clone(),
                insertion_index,
                node: ChildInsertionPointNode::SlotPlaceholder(node.clone().into()),
            },
        );
    }

    pub(super) fn slot_forwarding_expr_identifier(expression: &SyntaxNode) -> Option<SmolStr> {
        if expression.kind() != SyntaxKind::Expression {
            return None;
        }

        let mut expr_children = expression.children();
        let qualified_name = expr_children.find(|n| n.kind() == SyntaxKind::QualifiedName)?;
        if expr_children.next().is_some() {
            return None;
        }

        let mut identifiers = qualified_name
            .children_with_tokens()
            .filter(|n| n.kind() == SyntaxKind::Identifier)
            .filter_map(|n| n.into_token());
        let identifier = identifiers.next()?;
        if identifiers.next().is_some() {
            return None;
        }

        Some(crate::parser::normalize_identifier(identifier.text()))
    }
}
