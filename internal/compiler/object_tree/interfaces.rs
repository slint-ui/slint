// Copyright © 2026 Klarälvdalens Datakonsult AB, a KDAB Group company <info@kdab.com>, author Nathan Collins <nathan.collins@kdab.com>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

//! Module containing interfaces related types and functions.

use std::cell::RefCell;
use std::collections::BTreeMap;
use std::fmt::Display;
use std::rc::Rc;

use itertools::Itertools;
use smol_str::{SmolStr, ToSmolStr};

use crate::diagnostics::BuildDiagnostics;
use crate::expression_tree::{BindingExpression, Callable, Expression};
use crate::langtype::{ElementType, Function, PropertyLookupResult, Type};
use crate::namedreference::NamedReference;
use crate::object_tree::{
    Element, ElementRc, PropertyDeclaration, QualifiedTypeName, find_element_by_id,
    reject_experimental_feature,
};
use crate::parser;
use crate::parser::{SyntaxKind, syntax_nodes};
use crate::typeregister::TypeRegister;

enum InterfaceUseMode {
    Implements,
    Uses,
}

fn validate_property_declaration_for_interface(
    mode: InterfaceUseMode,
    result: &PropertyLookupResult,
    base_type: &ElementType,
    interface_name: &dyn Display,
) -> Result<(), String> {
    let usage = match mode {
        InterfaceUseMode::Implements => "implement",
        InterfaceUseMode::Uses => "use",
    };

    match result.property_type {
        Type::Invalid => Ok(()),
        Type::Callback { .. } => Err(format!(
            "Cannot {} interface '{}' because '{}' conflicts with an existing callback in '{}'",
            usage, interface_name, result.resolved_name, base_type
        )),
        Type::Function { .. } => Err(format!(
            "Cannot {} interface '{}' because '{}' conflicts with an existing function in '{}'",
            usage, interface_name, result.resolved_name, base_type
        )),
        _ => Err(format!(
            "Cannot {} interface '{}' because '{}' conflicts with an existing property in '{}'",
            usage, interface_name, result.resolved_name, base_type
        )),
    }
}

#[derive(Debug, PartialEq)]
pub(super) enum ImplementBinding {
    OnSelf,
    OnChild(SmolStr),
}

impl ImplementBinding {
    fn from_target(target_id: &SmolStr) -> ImplementBinding {
        if target_id.as_str() == "self" {
            ImplementBinding::OnSelf
        } else {
            ImplementBinding::OnChild(target_id.clone())
        }
    }
}

pub(super) struct ImplementedInterface {
    node: syntax_nodes::ImplementStatement,
    interface: ElementRc,
    interface_name: SmolStr,
    binding: ImplementBinding,
}

/// Resolve a single `implement` statement's interface. Emits diagnostics if the interface could not
/// be found, or was not actually an interface.
fn resolve_implement_statement(
    e: &Element,
    node: syntax_nodes::ImplementStatement,
    tr: &TypeRegister,
    diag: &mut BuildDiagnostics,
) -> Option<ImplementedInterface> {
    #[cfg(feature = "slint-sc")]
    diag.slint_sc_error("'implement' is", &node);

    if reject_experimental_feature(diag, tr, "implement", &node) {
        return None;
    }

    let qualified_name = node.QualifiedName();
    let interface_name = QualifiedTypeName::from_node(qualified_name.clone()).to_smolstr();
    let target_id = parser::identifier_text(&node.DeclaredIdentifier()).unwrap_or_default();

    if let Some(message) = match target_id.as_str() {
        "parent" => Some("Cannot implement an interface on a parent element"),
        "root" => Some("Cannot implement an interface on the root element; use 'self' instead"),
        _ => None,
    } {
        diag.push_error(message.into(), &node.DeclaredIdentifier());
        return None;
    }

    match e.base_type.lookup_type_for_child_element(&interface_name, tr) {
        Ok(ElementType::Component(c)) => {
            if !c.is_interface() {
                diag.push_error(
                    format!("Cannot implement {}. It is not an interface", interface_name),
                    &qualified_name,
                );
                return None;
            }

            c.used.set(true);
            Some(ImplementedInterface {
                node,
                interface: c.root_element.clone(),
                interface_name,
                binding: ImplementBinding::from_target(&target_id),
            })
        }
        Ok(_) => {
            // `lookup_type_for_child_element` resolves names like `Row` that are only valid
            // within a specific parent context (e.g. `GridLayout`), since it accounts for the
            // element's own base type. `tr.lookup_element` ignores that context and, for such
            // names, fails with a more specific diagnostic instead - reuse it here when it
            // applies, rather than the generic "not an interface" message.
            let message = match tr.lookup_element(&interface_name) {
                Err(context_restricted_message) => context_restricted_message,
                Ok(_) => format!("Cannot implement {}. It is not an interface", interface_name),
            };
            diag.push_error(message, &qualified_name);
            None
        }
        Err(err) => {
            diag.push_error(err, &qualified_name);
            None
        }
    }
}

/// Filter out conflicting `implement` statements across the combined self+child list, emitting
/// diagnostics for each conflict. Two statements conflict if they target the same interface, or if
/// they introduce properties/callbacks/functions with the same name. In that case we keep the first
/// one and filter out the rest.
fn filter_conflicting_implement_statements(
    diag: &mut BuildDiagnostics,
    statements: Vec<ImplementedInterface>,
) -> Vec<ImplementedInterface> {
    let mut seen_interfaces: Vec<ElementRc> = Vec::new();
    let mut seen_interface_api: BTreeMap<SmolStr, SmolStr> = BTreeMap::new();
    statements
        .into_iter()
        .filter(|stmt| {
            // Interface identity is the resolved interface's root element, not the syntactic name,
            // so this also catches the same interface implemented twice under different aliases.
            if seen_interfaces.iter().any(|seen| Rc::ptr_eq(seen, &stmt.interface)) {
                diag.push_error(
                    format!("'{}' is implemented multiple times", stmt.interface_name),
                    &stmt.node,
                );
                return false;
            }
            seen_interfaces.push(stmt.interface.clone());

            let mut valid = true;
            for (prop_name, _) in stmt.interface.borrow().property_declarations.iter() {
                if let Some(existing_interface) = seen_interface_api.get(prop_name) {
                    diag.push_error(
                        format!(
                            "'{}' occurs in '{}' and '{}'",
                            prop_name, stmt.interface_name, existing_interface
                        ),
                        &stmt.node.QualifiedName(),
                    );
                    valid = false;
                } else {
                    seen_interface_api.insert(prop_name.clone(), stmt.interface_name.clone());
                }
            }
            valid
        })
        .collect()
}

/// Gather the `implement` statements on the element, resolve their interfaces, run the unified
/// conflict check across self- and child-targeted statements, and partition the survivors. Emits
/// diagnostics for invalid or conflicting statements.
pub(super) fn get_implemented_interfaces(
    e: &Element,
    node: &syntax_nodes::Element,
    tr: &TypeRegister,
    diag: &mut BuildDiagnostics,
) -> (Vec<ImplementedInterface>, Vec<ImplementedInterface>) {
    let resolved: Vec<ImplementedInterface> = node
        .ImplementStatement()
        .filter_map(|stmt| resolve_implement_statement(e, stmt, tr, diag))
        .collect();

    let filtered = filter_conflicting_implement_statements(diag, resolved);

    let mut self_interfaces = Vec::new();
    let mut child_implements = Vec::new();
    for stmt in filtered {
        if stmt.binding == ImplementBinding::OnSelf {
            self_interfaces.push(stmt);
        } else {
            child_implements.push(stmt);
        }
    }
    (self_interfaces, child_implements)
}

/// `implement` statements are only supported on the root element for now. Emits a diagnostic for
/// each one found on a non-root element.
pub(super) fn disallow_implement_in_non_root(
    node: &syntax_nodes::Element,
    tr: &TypeRegister,
    diag: &mut BuildDiagnostics,
) {
    for stmt in node.ImplementStatement() {
        if reject_experimental_feature(diag, tr, "implement", &stmt) {
            continue;
        }
        diag.push_error("'implement' is only allowed in the root element".into(), &stmt);
    }
}

/// Apply the properties declared in the interfaces to the element, emitting diagnostics if there are any conflicts.
/// Existing property declarations are permitted, provided they match the declaration from the interface.
pub(super) fn apply_properties(
    e: &mut Element,
    implemented_interfaces: &[ImplementedInterface],
    diag: &mut BuildDiagnostics,
) {
    for ImplementedInterface { interface, node, interface_name, .. } in implemented_interfaces {
        for (unresolved_prop_name, prop_decl) in
            interface.borrow().property_declarations.iter().filter(|(_, prop_decl)| {
                // Functions are expected to be implemented manually, so we don't automatically add them.
                !matches!(prop_decl.property_type, Type::Function { .. } | Type::Callback { .. })
            })
        {
            apply_interface_property_declaration(
                e,
                unresolved_prop_name,
                prop_decl,
                node,
                interface_name,
                diag,
            );
        }
    }
}

/// Apply the callbacks declared in the interfaces to the element, emitting diagnostics if there are any conflicts.
/// Existing callback declarations are permitted, provided they match the declaration from the interface.
pub(super) fn apply_callbacks(
    e: &mut Element,
    implemented_interfaces: &[ImplementedInterface],
    diag: &mut BuildDiagnostics,
) {
    for ImplementedInterface { interface, node, interface_name, .. } in implemented_interfaces {
        for (unresolved_prop_name, prop_decl) in
            interface.borrow().property_declarations.iter().filter(|(_, prop_decl)| {
                // Functions are expected to be implemented manually, so we don't automatically add them.
                matches!(prop_decl.property_type, Type::Callback { .. })
            })
        {
            apply_interface_property_declaration(
                e,
                unresolved_prop_name,
                prop_decl,
                node,
                interface_name,
                diag,
            );
        }
    }
}

/// Apply a [PropertyDeclaration] from an interface to the element, emitting diagnostics if there are any conflicts. An
/// existing declaration with the same name is permitted, provided it matches the declaration from the interface.
fn apply_interface_property_declaration(
    e: &mut Element,
    unresolved_prop_name: &SmolStr,
    prop_decl: &PropertyDeclaration,
    node: &syntax_nodes::ImplementStatement,
    interface_name: &SmolStr,
    diag: &mut BuildDiagnostics,
) {
    if matches!(prop_decl.property_type, Type::Invalid) {
        // The interface's own declaration is invalid (e.g. an unknown property type). A diagnostic
        // was already emitted when the interface was parsed, so there is nothing meaningful to apply
        // or conflict-check here.
        return;
    }

    let lookup_result = e.lookup_property(unresolved_prop_name);

    fn find_conflicting_node(
        e: &mut Element,
        unresolved_prop_name: &SmolStr,
    ) -> Option<parser::SyntaxNode> {
        e.property_declarations.get(unresolved_prop_name).and_then(|decl| decl.node.clone())
    }

    if lookup_result.property_type != Type::Invalid {
        if lookup_result.is_local_to_component {
            let property_type_name = match prop_decl.property_type {
                Type::Callback { .. } => "callback",
                Type::Function { .. } => "function",
                _ => "property",
            };

            let local_property_node = find_conflicting_node(e, unresolved_prop_name)
                .expect("Expected local property to have a syntax node");

            diag.push_error(
                format!(
                    "Conflict with '{}' which declares a {} with the same name",
                    interface_name, property_type_name
                ),
                &local_property_node,
            );
            return;
        }

        match property_matches_interface(&lookup_result, prop_decl) {
            Ok(()) => {
                // The property already exists and matches the interface declaration, so we don't need to do anything.
                return;
            }
            Err(error) => {
                // Attempt to find a node for the existing property for better diagnostics. If the property is not local
                // to the component, we fall back to pointing at the implement statement below.
                if let Some(local_property_node) = find_conflicting_node(e, unresolved_prop_name) {
                    diag.push_error(
                        format!("Conflict with '{}' which {}", interface_name, error),
                        &local_property_node,
                    );
                    return;
                }
            }
        }
    }

    if let Err(message) = validate_property_declaration_for_interface(
        InterfaceUseMode::Implements,
        &lookup_result,
        &e.base_type,
        &interface_name,
    ) {
        diag.push_error(message, &node.QualifiedName());
        return;
    }

    e.property_declarations.insert(unresolved_prop_name.clone(), prop_decl.clone());
}

/// Validate that the functions declared in the interface are correctly implemented in the element. Emits diagnostics if not.
pub(super) fn validate_function_implementations(
    e: &Element,
    implemented_interfaces: &[ImplementedInterface],
    diag: &mut BuildDiagnostics,
) {
    for ImplementedInterface { interface, node, interface_name, .. } in implemented_interfaces {
        for (function_name, function_property_decl) in interface
            .borrow()
            .property_declarations
            .iter()
            .filter(|(_, prop_decl)| matches!(prop_decl.property_type, Type::Function { .. }))
        {
            let Type::Function(ref function_declaration) = function_property_decl.property_type
            else {
                debug_assert!(false, "Non-functions should have been filtered out already");
                continue;
            };

            let push_interface_error =
                |diag: &mut BuildDiagnostics, is_local_to_component, error| {
                    if is_local_to_component {
                        let source = e
                            .property_declarations
                            .get(function_name)
                            .and_then(|decl| decl.node.clone())
                            .map_or_else(
                                || parser::NodeOrToken::Node(node.QualifiedName().into()),
                                parser::NodeOrToken::Node,
                            );
                        diag.push_error(error, &source);
                    } else {
                        diag.push_error(error, &node.QualifiedName());
                    }
                };

            let found_function = e.lookup_property(function_name);
            let function_impl = match found_function.property_type {
                Type::Invalid => {
                    diag.push_error(
                        format!("Missing implementation of function '{}'", function_name),
                        &node.QualifiedName(),
                    );
                    None
                }
                Type::Function(function) => Some(function.clone()),
                _ => {
                    push_interface_error(
                        diag,
                        found_function.is_local_to_component,
                        format!(
                            "Cannot override '{}' from interface '{}'",
                            function_name, interface_name
                        ),
                    );
                    None
                }
            };
            let Some(function_impl) = function_impl else { continue };

            match (function_property_decl.pure, found_function.declared_pure) {
                (Some(true), Some(false)) | (Some(true), None) => push_interface_error(
                    diag,
                    found_function.is_local_to_component,
                    format!(
                        "Implementation of pure function '{}' from interface '{}' cannot be impure",
                        function_name, interface_name
                    ),
                ),
                _ => {
                    // If the implementation is pure but the declaration is not, we allow it.
                }
            }

            if function_property_decl.visibility != found_function.property_visibility {
                push_interface_error(
                    diag,
                    found_function.is_local_to_component,
                    format!(
                        "Incorrect visibility for implementation of '{}' from interface '{}'. Expected '{}'",
                        function_name, interface_name, function_property_decl.visibility,
                    ),
                );
            }

            if function_impl.args != function_declaration.args {
                let display_args = |args: &Vec<Type>| -> SmolStr {
                    args.iter().map(|t| t.to_string()).join(", ").into()
                };

                push_interface_error(
                    diag,
                    found_function.is_local_to_component,
                    format!(
                        "Incorrect arguments for implementation of '{}' from interface '{}'. Expected ({}) but got ({})",
                        function_name,
                        interface_name,
                        display_args(&function_declaration.args),
                        display_args(&function_impl.args),
                    ),
                );
            }

            if function_impl.return_type != function_declaration.return_type {
                push_interface_error(
                    diag,
                    found_function.is_local_to_component,
                    format!(
                        "Incorrect return type for implementation of '{}' from interface '{}'. Expected '{}' but got '{}'",
                        function_name,
                        interface_name,
                        function_declaration.return_type,
                        function_impl.return_type,
                    ),
                );
            }
        }
    }
}

/// Apply the child-targeted `implement` statements, forwarding each interface's members from the
/// element onto the named child (today's `uses`). Emits diagnostics for invalid statements.
pub(super) fn apply_child_implement_statements(
    e: &ElementRc,
    child_implements: Vec<ImplementedInterface>,
    diag: &mut BuildDiagnostics,
) {
    for ImplementedInterface { node, interface, interface_name, binding } in child_implements {
        debug_assert_ne!(binding, ImplementBinding::OnSelf);
        let ImplementBinding::OnChild(child_id) = binding else {
            continue;
        };
        let Some(child) = find_element_by_id(e, &child_id) else {
            diag.push_error(format!("'{}' does not exist", child_id), &node.DeclaredIdentifier());
            continue;
        };

        if !element_implements_interface(
            &child,
            &interface,
            &child_id,
            &interface_name,
            &node,
            diag,
        ) {
            continue;
        }

        for (name, prop_decl) in interface.borrow().property_declarations.iter() {
            let lookup_result = e.borrow().base_type.lookup_property(name);
            if let Err(message) = validate_property_declaration_for_interface(
                InterfaceUseMode::Uses,
                &lookup_result,
                &e.borrow().base_type,
                &interface_name,
            ) {
                diag.push_error(message, &node.QualifiedName());
                continue;
            }

            // Replace the node with the interface name for better diagnostics later, since the declaration won't have a
            // node in this element.
            let mut prop_decl = prop_decl.clone();
            prop_decl.node = Some(node.QualifiedName().into());

            if let Some(existing_property) =
                e.borrow_mut().property_declarations.insert(name.clone(), prop_decl.clone())
            {
                let source = existing_property
                    .node
                    .as_ref()
                    .and_then(|node| node.child_node(SyntaxKind::DeclaredIdentifier))
                    .and_then(|node| node.child_token(SyntaxKind::Identifier))
                    .map_or_else(
                        || parser::NodeOrToken::Node(node.DeclaredIdentifier().into()),
                        parser::NodeOrToken::Token,
                    );

                diag.push_error(
                    format!("Cannot override '{}' from '{}'", name, interface_name),
                    &source,
                );
                continue;
            }

            let existing_binding = match &prop_decl.property_type {
                Type::Function(func) => {
                    apply_uses_statement_function_binding(e, &child, name, func)
                }
                _ => e.borrow_mut().bindings.insert(
                    name.clone(),
                    BindingExpression::new_two_way(
                        NamedReference::new(&child, name.clone()).into(),
                    )
                    .into(),
                ),
            };

            if let Some(existing_binding) = existing_binding {
                let message = format!(
                    "Cannot override binding for '{}' from interface '{}'",
                    name, interface_name
                );
                if let Some(location) = &existing_binding.borrow().span {
                    diag.push_error(message, location);
                } else {
                    diag.push_error(message, &node.QualifiedName());
                }
            }
        }
    }
}

/// Check that the given element implements the given interface. Emits a diagnostic if the interface is not implemented.
fn element_implements_interface(
    element: &ElementRc,
    interface: &ElementRc,
    child_id: &SmolStr,
    interface_name: &SmolStr,
    node: &syntax_nodes::ImplementStatement,
    diag: &mut BuildDiagnostics,
) -> bool {
    let mut valid = true;
    let mut check = |property_name: &SmolStr, property_declaration: &PropertyDeclaration| {
        let lookup_result = element.borrow().lookup_property(property_name);
        if let Err(e) = property_matches_interface(&lookup_result, property_declaration) {
            diag.push_error(
                format!(
                    "'{}' does not implement '{}' from '{}' - {}",
                    child_id, property_name, interface_name, e
                ),
                &node.DeclaredIdentifier(),
            );
            valid = false;
        }
    };

    for (property_name, property_declaration) in interface.borrow().property_declarations.iter() {
        check(property_name, property_declaration);
    }

    valid
}

/// Check that the given property matches the declaration from the interface. Emits a diagnostic if it doesn't match.
fn property_matches_interface(
    property: &PropertyLookupResult,
    interface_declaration: &PropertyDeclaration,
) -> Result<(), String> {
    if property.property_type == Type::Invalid {
        return Err("not found".into());
    }

    let mut errors = Vec::new();

    if property.property_type != interface_declaration.property_type {
        errors.push(format!("type: '{}'", interface_declaration.property_type));
    }

    if property.property_visibility != interface_declaration.visibility {
        errors.push(format!("visibility: '{}'", interface_declaration.visibility));
    }

    if property.declared_pure.unwrap_or(false) != interface_declaration.pure.unwrap_or(false) {
        errors
            .push(format!("purity declaration: '{}'", interface_declaration.pure.unwrap_or(false)));
    }

    if errors.is_empty() {
        Ok(())
    } else {
        Err(format!("expected {}", errors.into_iter().join(", ")))
    }
}

/// Apply the function from the interface to the element, creating a forwarding bindings to the function on the child
/// element. Emits diagnostics if there are conflicting functions.
fn apply_uses_statement_function_binding(
    e: &ElementRc,
    child: &ElementRc,
    name: &SmolStr,
    func: &Rc<Function>,
) -> Option<RefCell<BindingExpression>> {
    // Create forwarding call expression: child.function_name(arg0, arg1, ...)
    let args_expr: Vec<Expression> = func
        .args
        .iter()
        .enumerate()
        .map(|(i, ty)| Expression::FunctionParameterReference { index: i, ty: ty.clone() })
        .collect();

    // Use Callable::Function with a NamedReference to the child's function
    let call_expr = Expression::FunctionCall {
        function: Callable::Function(NamedReference::new(child, name.clone())),
        arguments: args_expr,
        source_location: None,
    };

    // The function body is just the forwarding call. CodeBlock handles the return implicitly for the last expression
    let body = Expression::CodeBlock(vec![call_expr]);
    e.borrow_mut().bindings.insert(name.clone(), RefCell::new(BindingExpression::from(body)))
}
