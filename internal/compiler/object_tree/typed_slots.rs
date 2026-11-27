// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

use super::*;

pub(super) fn declaration(
    node: syntax_nodes::SlotDeclaration,
    tr: &TypeRegister,
    diag: &mut BuildDiagnostics,
) -> DeclaredSlot {
    let is_collection = node.child_token(SyntaxKind::LBracket).is_some();
    let is_default =
        node.children_with_tokens().filter_map(|n| n.into_token()).any(|t| t.text() == "default");
    if is_default && !is_collection {
        diag.push_error(
            "A default slot must have a collection type, such as '<[*]>'".into(),
            &node,
        );
    }
    let mut component_type = None;
    let interface = node.QualifiedName().and_then(|ty| {
        let name = QualifiedTypeName::from_node(ty.clone()).to_smolstr();
        match tr.lookup_element(&name) {
            Ok(ElementType::Component(c)) if c.is_interface() => Some(c),
            Ok(element_type)
                if matches!(&element_type, ElementType::Component(c) if !c.is_global())
                    || matches!(&element_type, ElementType::Builtin(b) if !b.is_global) =>
            {
                component_type = Some(element_type);
                None
            }
            Ok(_) => {
                diag.push_error(
                    format!(
                        "Slot type '{name}' must be an interface, component, or built-in element"
                    ),
                    &ty,
                );
                None
            }
            Err(error) => {
                diag.push_error(error, &ty);
                None
            }
        }
    });
    let defaults = match (is_collection, node.SlotElementList(), node.SubElement()) {
        (true, Some(list), _) => list.SubElement().collect(),
        (true, None, Some(element)) => {
            diag.push_error("Use '[...]' for collection slot defaults".into(), &element);
            Vec::new()
        }
        (false, Some(list), _) => {
            diag.push_error(
                "A list of elements can only be used with a collection slot".into(),
                &list,
            );
            Vec::new()
        }
        (false, None, Some(element)) if node.QualifiedName().is_some() => {
            diag.push_error("Typed slots don't support default content yet".into(), &element);
            Vec::new()
        }
        (_, _, element) => element.into_iter().collect(),
    };
    let name_node = node.DeclaredIdentifier();
    DeclaredSlot {
        is_collection,
        is_default: is_default && is_collection,
        name: parser::identifier_text(&name_node).unwrap_or_default(),
        name_node,
        interface,
        component_type,
        defaults,
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
    let Some(slot) = lookup_slot(component, name) else { return };
    if slot.is_collection {
        return;
    }
    validate_child(&slot, element, diag);
}

pub(super) fn validate_child(
    slot: &DeclaredSlot,
    element: &ElementRc,
    diag: &mut BuildDiagnostics,
) -> bool {
    let element = element.borrow();
    if let Some(interface) = &slot.interface {
        return element.debug.first().is_none_or(|debug| {
            interfaces::validate_interface_implementation(
                &element,
                &interface.root_element,
                &interface.id,
                &debug.node.clone().into(),
                &interfaces::ImplementBinding::OnSelf,
                diag,
            )
        });
    }
    if let Some(expected) = &slot.component_type
        && !inherits_type(element.base_type.clone(), expected)
    {
        let subject = if slot.is_collection { "Collection slot" } else { "Slot" };
        diag.push_error(
            format!(
                "{subject} '{}' requires '{expected}' or a component inheriting it, but got '{}'",
                slot.name, element.base_type
            ),
            &*element,
        );
        return false;
    }
    true
}

fn inherits_type(mut actual: ElementType, expected: &ElementType) -> bool {
    while &actual != expected {
        let ElementType::Component(component) = actual else { return false };
        actual = component.root_element.borrow().base_type.clone();
    }
    true
}

pub(crate) fn missing_typed_slot_message(slot: &DeclaredSlot) -> String {
    let name = &slot.name;
    if let Some(interface) = &slot.interface {
        format!("Typed slot '{name}' requires a component that provides '{}'", interface.id)
    } else {
        let expected = slot.component_type.as_ref().unwrap();
        format!("Typed slot '{name}' requires '{expected}' or a component inheriting it")
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
    if slots.iter().any(|s| &s.name == name && s.is_collection) {
        diag.push_error(format!("Expand collection slot '{name}' with a 'for' loop"), node);
        return true;
    }
    let Some(constraint) =
        slots.iter().find(|s| &s.name == name).and_then(DeclaredSlot::constraint_type)
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
        Some(constraint),
    ) else {
        return true;
    };
    proxy.borrow_mut().is_typed_slot = true;
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
    slots: &mut [DeclaredSlot],
    diag: &mut BuildDiagnostics,
) -> bool {
    let source_slot = slots.iter().find(|slot| &slot.name == source);
    let target_slot = match &parent.borrow().base_type {
        ElementType::Component(c) => lookup_slot(c, target),
        _ => None,
    };
    if source_slot.is_some_and(|slot| slot.is_collection)
        || target_slot.as_ref().is_some_and(|slot| slot.is_collection)
    {
        diag.push_error("Forwarding collection slots is not supported yet".into(), node);
        Element::mark_placeholder_rejected(slots, source);
        return false;
    }
    let source_type = source_slot.and_then(DeclaredSlot::constraint_type);
    let target_slot = target_slot.as_ref().filter(|s| s.is_typed());
    if source_type.is_none() && target_slot.is_none() {
        return true;
    }
    let Some(source_type) = source_type else {
        let required =
            if target_slot.is_some_and(|s| s.interface.is_some()) { "interface" } else { "type" };
        diag.push_error(
            format!("Declare slot '{source}' with the {required} required by '{target}'"),
            node,
        );
        if let Some(slot) = source_slot {
            diag.push_note(format!("The slot '{source}' is declared here"), &slot.name_node);
        }
        return false;
    };
    if let Some(target_slot) = target_slot {
        if let Some(interface) = &target_slot.interface {
            if let Some(source_interface) = source_slot.and_then(|slot| slot.interface.as_ref()) {
                if !Rc::ptr_eq(source_interface, interface) {
                    diag.push_error(
                        format!("Forwarded slot '{source}' has an incompatible interface"),
                        node,
                    );
                    return false;
                }
            } else if !interfaces::validate_interface_implementation(
                &Element { base_type: source_type.clone(), ..Default::default() },
                &interface.root_element,
                &interface.id,
                &node.clone().into(),
                &interfaces::ImplementBinding::OnSelf,
                diag,
            ) {
                return false;
            }
        } else if !inherits_type(source_type.clone(), target_slot.component_type.as_ref().unwrap())
        {
            diag.push_error(format!("Forwarded slot '{source}' has an incompatible type"), node);
            Element::mark_placeholder_rejected(slots, source);
            return false;
        }
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
        base_type: source_type,
        is_typed_slot: true,
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
            crate::reject_experimental_feature(
                diag,
                tr,
                crate::ExperimentalFeature::NamedSlots,
                "named slots",
                &declaration,
            );
            let slot = self::declaration(declaration, tr, diag);
            if slot.is_collection {
                if !is_component_root(node) {
                    diag.push_error(
                        "Declare collection slots on a component root".into(),
                        &slot.name_node,
                    );
                    continue;
                }
                if slot.is_default
                    && (declared_slots.iter().any(|s| s.is_default)
                        || matches!(&r.borrow().base_type, ElementType::Component(base) if super::slot_collections::default_collection(base).is_some()))
                {
                    diag.push_error(
                        "A component can only declare one default slot".into(),
                        &slot.name_node,
                    );
                    continue;
                }
            }
            if is_component_root(node)
                && let ElementType::Component(base) = &r.borrow().base_type
                && lookup_slot(base, &slot.name).is_some()
            {
                diag.push_error(
                    format!(
                        "Remove the declaration of slot '{0}' inherited from '{1}'; assign with '{0} << ...' to replace its content",
                        slot.name, base.id
                    ),
                    &slot.name_node,
                );
                continue;
            }
            if slot.is_collection {
                super::slot_collections::declare(&slot, r, tr);
            }
            declared_slots.push(slot);
        }

        for se in node.children() {
            if se.kind() != SyntaxKind::SlotForwarding {
                continue;
            }
            if crate::reject_experimental_feature(
                diag,
                tr,
                crate::ExperimentalFeature::NamedSlots,
                "slot forwarding",
                &se,
            ) {
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
            let insertion_name = declared_slots
                .iter()
                .find(|slot| slot.name == source)
                .map_or(source.as_str(), |slot| slot.insertion_name());
            if let Some(existing_cip) = component_child_insertion_points.get(insertion_name) {
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
                insertion_name.to_string(),
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
        is_component_root: bool,
        diag: &mut BuildDiagnostics,
        tr: &TypeRegister,
    ) {
        if crate::reject_experimental_feature(
            diag,
            tr,
            crate::ExperimentalFeature::NamedSlots,
            "named slots",
            &se,
        ) {
            return;
        }
        let name_node = se.child_node(SyntaxKind::DeclaredIdentifier).unwrap();
        let name = parser::identifier_text(&name_node).unwrap_or_default();
        if name == "children" {
            diag.push_error(
                format!(
                    "The name '{name}' is reserved for the default slot; add the element as a direct child instead"
                ),
                &name_node,
            );
            return;
        }
        if r.borrow().forwarded_slots.iter().any(|f| f.target == name) {
            diag.push_error(format!("Duplicate assignment to slot '{name}'"), &name_node);
        }
        let parent_type = r.borrow().base_type.clone();
        let slot = match &parent_type {
            ElementType::Component(component) => {
                let slot = lookup_slot(component, &name);
                if slot.is_none() {
                    diag.push_error(
                        format!("Unknown slot '{name}' in '{}'", component.id),
                        &name_node,
                    );
                }
                slot
            }
            _ => {
                diag.push_error("Slot assignments can only be used on components".to_string(), &se);
                None
            }
        };
        let Some(slot) = slot else { return };
        if !assigned_slots.insert(name.clone()) {
            diag.push_error(format!("Duplicate assignment to slot '{name}'"), &name_node);
            return;
        }
        let se: syntax_nodes::SlotAssignment = se.into();
        let children: Vec<syntax_nodes::SubElement> =
            match (slot.is_collection, se.SlotElementList()) {
                (true, Some(list)) => list.SubElement().collect(),
                (true, None) => {
                    diag.push_error(
                        format!("Use '{name} << [...]' to assign a collection slot"),
                        &se,
                    );
                    return;
                }
                (false, Some(list)) => {
                    diag.push_error(
                        "A list of elements can only be used with a collection slot".into(),
                        &list,
                    );
                    return;
                }
                (false, None) => se.SubElement().into_iter().collect(),
            };
        if children.is_empty() {
            if slot.is_typed() && !slot.is_collection {
                diag.push_error(missing_typed_slot_message(&slot), &se);
            } else {
                r.borrow_mut().empty_slot_assignments.push(slot.insertion_name().into());
            }
        }
        let parent_type =
            if slot.is_collection || slot.is_typed() { tr.empty_type() } else { parent_type };
        for child in children {
            let element = Element::from_sub_element_node(
                child,
                parent_type.clone(),
                component_child_insertion_points,
                declared_slots,
                is_legacy_syntax,
                diag,
                tr,
            );
            if is_component_root && (slot.is_collection || !slot.is_typed()) {
                element.borrow_mut().slot_default = Some(slot.insertion_name().into());
            }
            element.borrow_mut().slot_target = Some(slot.insertion_name().into());
            r.borrow_mut().children.push(element);
        }
    }

    pub(super) fn component_slot_placeholder(
        node: &SyntaxNode,
        parent: &ElementRc,
        tr: &TypeRegister,
    ) -> Option<(SmolStr, SyntaxNode, Rc<Component>)> {
        if node.child_token(SyntaxKind::ColonEqual).is_some() {
            return None;
        }
        let qualified_name =
            node.child_node(SyntaxKind::Element)?.child_node(SyntaxKind::QualifiedName)?;
        if qualified_name.child_token(SyntaxKind::Dot).is_some() {
            return None;
        }
        let name = parser::identifier_text(&qualified_name)?;
        let parent_type = parent.borrow().base_type.clone();
        let ElementType::Component(base) = &parent_type else { return None };
        (parent_type.lookup_type_for_child_element(&name, tr).is_err()
            && lookup_slot(base, &name).is_some())
        .then(|| (name, qualified_name, base.clone()))
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
                    && (slot.is_typed()
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
    ) -> bool {
        crate::reject_experimental_feature(
            diagnostics,
            type_register,
            crate::ExperimentalFeature::NamedSlots,
            "named slots",
            node,
        );
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
            return false;
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
        true
    }

    pub(super) fn place_slot_default(
        slot_name: &SmolStr,
        parent: &ElementRc,
        declared_slots: &mut Vec<DeclaredSlot>,
        is_legacy_syntax: bool,
        diagnostics: &mut BuildDiagnostics,
        type_register: &TypeRegister,
    ) {
        let Some((defaults, insertion_name)) = declared_slots
            .iter()
            .find(|slot| &slot.name == slot_name)
            .map(|slot| (slot.defaults.clone(), SmolStr::new(slot.insertion_name())))
        else {
            return;
        };
        let mut nested_insertion_points = BTreeMap::new();
        let parent_type = parent.borrow().base_type.clone();
        for default in defaults {
            let element = Element::from_sub_element_node(
                default,
                parent_type.clone(),
                &mut nested_insertion_points,
                declared_slots,
                is_legacy_syntax,
                diagnostics,
                type_register,
            );
            element.borrow_mut().slot_default = Some(insertion_name.clone());
            parent.borrow_mut().children.push(element);
        }
        Self::reject_slot_placeholders(
            diagnostics,
            declared_slots,
            nested_insertion_points,
            &format!("the default content of slot '{slot_name}'"),
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
