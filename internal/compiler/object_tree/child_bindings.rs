// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

use super::*;

pub(super) fn create(
    node: syntax_nodes::ChildBindings,
    parent: &ElementRc,
    legacy: bool,
    diag: &mut BuildDiagnostics,
    tr: &TypeRegister,
) {
    if crate::reject_experimental_feature(
        diag,
        tr,
        crate::ExperimentalFeature::ChildBindings,
        "child bindings",
        &node,
    ) {
        return;
    }
    #[cfg(feature = "slint-sc")]
    diag.slint_sc_error("Child bindings are", &node);
    let body = node.Element();
    if matches!(parent.borrow().base_type, ElementType::Global | ElementType::Interface(_)) {
        return;
    }
    if !is_component_root(&parent.borrow().debug[0].node) {
        diag.push_error("Declare child-bindings on a component root".into(), &node);
        return;
    }
    if parent
        .borrow()
        .children
        .iter()
        .any(|child| child.borrow().child_bindings_interface.is_some())
    {
        diag.push_error("Declare child-bindings only once per component".into(), &node);
        return;
    }
    let ty = node.QualifiedName();
    let name = QualifiedTypeName::from_node(ty.clone()).to_smolstr();
    let interface = match tr.lookup_element(&name) {
        Ok(ElementType::Component(component)) if component.is_interface() => component,
        Ok(_) => {
            diag.push_error(format!("Child type '{name}' must be an interface"), &ty);
            return;
        }
        Err(error) => {
            diag.push_error(error, &ty);
            return;
        }
    };
    interface.used.set(true);
    for name in ["index", "count"] {
        if interfaces::declared_members(&interface.root_element).contains_key(name) {
            diag.push_error(
                format!("Child bindings reserve '{name}' for collection metadata"),
                &ty,
            );
            return;
        }
    }
    for child in body.children() {
        if matches!(child.kind(), SyntaxKind::Binding | SyntaxKind::TwoWayBinding)
            && child
                .child_token(SyntaxKind::Identifier)
                .is_some_and(|token| matches!(token.text(), "index" | "count"))
        {
            diag.push_error("Collection metadata is read-only".into(), &child);
        }
        if !matches!(
            child.kind(),
            SyntaxKind::Binding
                | SyntaxKind::CallbackConnection
                | SyntaxKind::TwoWayBinding
                | SyntaxKind::PropertyChangedCallback
        ) {
            diag.push_error(
                "Child bindings only accept property bindings and callback connections".into(),
                &child,
            );
        }
    }
    let id = parser::identifier_text(&node.DeclaredIdentifier()).unwrap_or_default();
    let Some((template, _, _)) = Element::element_without_children(
        &body,
        id,
        parent.borrow().base_type.clone(),
        legacy,
        diag,
        tr,
        Some(interface.clone()),
    ) else {
        return;
    };
    template.borrow_mut().child_bindings_interface = Some(interface);
    for name in ["index", "count"] {
        template.borrow_mut().property_declarations.insert(
            name.into(),
            PropertyDeclaration {
                property_type: Type::Int32,
                visibility: PropertyVisibility::Output,
                ..Default::default()
            },
        );
    }
    let count = crate::layout::create_new_prop(parent, "child-bindings-count".into(), Type::Int32);
    template.borrow_mut().property_declarations.get_mut("count").unwrap().is_alias = Some(count);
    parent.borrow_mut().children.push(template);
}
