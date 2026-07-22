// Copyright © 2026 Klarälvdalens Datakonsult AB, a KDAB Group company <info@kdab.com>, author Nathan Collins <nathan.collins@kdab.com>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

//! Module containing interfaces related types and functions.

use std::cell::RefCell;
use std::collections::BTreeMap;
use std::rc::Rc;

use itertools::Itertools;
use smol_str::{SmolStr, ToSmolStr};

use crate::diagnostics::BuildDiagnostics;
use crate::expression_tree::{BindingExpression, Callable, Expression};
use crate::langtype::{ElementType, Function, PropertyLookupResult, Type};
use crate::namedreference::NamedReference;
use crate::object_tree::{
    Element, ElementRc, PropertyDeclaration, QualifiedTypeName, find_element_by_id,
};
use crate::parser;
use crate::parser::{SyntaxKind, syntax_nodes};
use crate::reject_experimental_feature;
use crate::typeregister::TypeRegister;

fn check_property_declaration_conflicts(
    result: &PropertyLookupResult,
    base_type: &ElementType,
) -> Result<(), String> {
    match result.property_type {
        Type::Invalid => Ok(()),
        Type::Callback { .. } => Err(format!(
            "- '{}' conflicts with an existing callback in '{}'",
            result.resolved_name, base_type
        )),
        Type::Function { .. } => Err(format!(
            "- '{}' conflicts with an existing function in '{}'",
            result.resolved_name, base_type
        )),
        _ => Err(format!(
            "- '{}' conflicts with an existing property in '{}'",
            result.resolved_name, base_type
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

fn resolve_implement_statement(
    element: &Element,
    node: syntax_nodes::ImplementStatement,
    type_register: &TypeRegister,
    diagnostics: &mut BuildDiagnostics,
) -> Option<ImplementedInterface> {
    #[cfg(feature = "slint-sc")]
    diagnostics.slint_sc_error("'implement' is", &node);

    if reject_experimental_feature(diagnostics, type_register, "implement", &node) {
        return None;
    }

    let qualified_name = node.QualifiedName();
    let interface_name = QualifiedTypeName::from_node(qualified_name.clone()).to_smolstr();
    let target_id = parser::identifier_text(&node.DeclaredIdentifier()).unwrap_or_default();

    if let Some(target) = match target_id.as_str() {
        "parent" => Some("a parent element"),
        "root" => Some("the root element; use 'self' instead"),
        _ => None,
    } {
        diagnostics.push_error(
            format!("Cannot implement an interface based on {}", target),
            &node.DeclaredIdentifier(),
        );
        return None;
    }

    match element.base_type.lookup_type_for_child_element(&interface_name, type_register) {
        Ok(ElementType::Component(c)) => {
            if !c.is_interface() {
                diagnostics.push_error(
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
            let message = match type_register.lookup_element(&interface_name) {
                Err(context_restricted_message) => context_restricted_message,
                Ok(_) => format!("Cannot implement {}. It is not an interface", interface_name),
            };
            diagnostics.push_error(message, &qualified_name);
            None
        }
        Err(err) => {
            diagnostics.push_error(err, &qualified_name);
            None
        }
    }
}

fn filter_conflicting_implement_statements(
    diagnostics: &mut BuildDiagnostics,
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
                diagnostics.push_error(
                    format!("'{}' is implemented multiple times", stmt.interface_name),
                    &stmt.node,
                );
                return false;
            }
            seen_interfaces.push(stmt.interface.clone());

            let mut valid = true;
            for (prop_name, _) in stmt.interface.borrow().property_declarations.iter() {
                if let Some(existing_interface) = seen_interface_api.get(prop_name) {
                    diagnostics.push_error(
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

pub(super) fn get_implemented_interfaces(
    element: &Element,
    node: &syntax_nodes::Element,
    type_register: &TypeRegister,
    diagnostics: &mut BuildDiagnostics,
) -> (Vec<ImplementedInterface>, Vec<ImplementedInterface>) {
    let resolved: Vec<ImplementedInterface> = node
        .ImplementStatement()
        .filter_map(|stmt| resolve_implement_statement(element, stmt, type_register, diagnostics))
        .collect();

    let filtered = filter_conflicting_implement_statements(diagnostics, resolved);

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

pub(super) fn disallow_implement_in_non_root(
    node: &syntax_nodes::Element,
    type_register: &TypeRegister,
    diagnostics: &mut BuildDiagnostics,
) {
    for stmt in node.ImplementStatement() {
        if reject_experimental_feature(diagnostics, type_register, "implement", &stmt) {
            continue;
        }
        diagnostics.push_error("'implement' is only allowed in the root element".into(), &stmt);
    }
}

pub(super) fn validate_properties_and_callbacks(
    element: &Element,
    implemented_interfaces: &[ImplementedInterface],
    diagnostics: &mut BuildDiagnostics,
) {
    for ImplementedInterface { interface, node, interface_name, .. } in implemented_interfaces {
        let mut errors = Vec::new();
        for (member_name, member_declaration) in
            interface.borrow().property_declarations.iter().filter(|(_, property_declaration)| {
                !matches!(property_declaration.property_type, Type::Function { .. })
            })
        {
            if let Some(message) = validate_interface_member_implementation(
                element,
                member_name,
                member_declaration,
                interface_name,
                diagnostics,
            ) {
                errors.push(message);
            };
        }

        if !errors.is_empty() {
            diagnostics.push_error(
                format!("Cannot implement '{interface_name}'.\n{}", errors.join("\n")),
                &node.QualifiedName(),
            );
        }
    }
}

fn validate_interface_member_implementation(
    element: &Element,
    member_name: &SmolStr,
    interface_member: &PropertyDeclaration,
    interface_name: &SmolStr,
    diagnostics: &mut BuildDiagnostics,
) -> Option<String> {
    if matches!(interface_member.property_type, Type::Invalid) {
        // The interface's own declaration is invalid (e.g. an unknown property type). A diagnostic
        // was already emitted when the interface was parsed, so there is nothing meaningful to
        // validate here.
        return None;
    }

    let lookup_result = element.lookup_property(member_name);
    if lookup_result.property_type == Type::Invalid {
        return Some(missing_type_description(member_name, interface_member));
    }

    let Err(conflicts) =
        property_matches_interface(&lookup_result, interface_member, member_name, None)
    else {
        return None;
    };

    if !lookup_result.is_local_to_component {
        if let Err(message) =
            check_property_declaration_conflicts(&lookup_result, &element.base_type)
        {
            return Some(message);
        }
        return None;
    }

    let source = element
        .property_declarations
        .get(member_name)
        .and_then(|declaration| declaration.node.clone());

    if let Some(source) = source {
        let error = format!("Cannot implement '{interface_name}'.\n{conflicts}");
        diagnostics.push_error(error, &source);
        return None;
    }
    return Some(conflicts);
}

pub(super) fn validate_function_implementations(
    element: &Element,
    implemented_interfaces: &[ImplementedInterface],
    diagnostics: &mut BuildDiagnostics,
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
                        let source = element
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

            let found_function = element.lookup_property(function_name);
            let function_impl = match found_function.property_type {
                Type::Invalid => {
                    diagnostics.push_error(
                        format!("Missing implementation of function '{}'", function_name),
                        &node.QualifiedName(),
                    );
                    None
                }
                Type::Function(function) => Some(function.clone()),
                _ => {
                    push_interface_error(
                        diagnostics,
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
                    diagnostics,
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
                    diagnostics,
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
                    diagnostics,
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
                    diagnostics,
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

pub(super) fn apply_child_implement_statements(
    element: &ElementRc,
    child_implements: Vec<ImplementedInterface>,
    diagnostics: &mut BuildDiagnostics,
) {
    for ImplementedInterface { node, interface, interface_name, binding } in child_implements {
        debug_assert_ne!(binding, ImplementBinding::OnSelf);
        let ImplementBinding::OnChild(child_id) = binding else {
            continue;
        };
        let Some(child) = find_element_by_id(element, &child_id) else {
            diagnostics
                .push_error(format!("'{}' does not exist", child_id), &node.DeclaredIdentifier());
            continue;
        };

        if !element_implements_interface(
            &child,
            &interface,
            &child_id,
            &interface_name,
            &node,
            diagnostics,
        ) {
            continue;
        }

        let mut conflicts = Vec::new();
        for (name, prop_decl) in interface.borrow().property_declarations.iter() {
            let lookup_result = element.borrow().base_type.lookup_property(name);
            if let Err(message) =
                check_property_declaration_conflicts(&lookup_result, &element.borrow().base_type)
            {
                conflicts.push(message);
                continue;
            }

            // Replace the node with the interface name for better diagnostics later, since the declaration won't have a
            // node in this element.
            let mut prop_decl = prop_decl.clone();
            prop_decl.node = Some(node.QualifiedName().into());

            if let Some(existing_property) =
                element.borrow_mut().property_declarations.insert(name.clone(), prop_decl.clone())
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

                diagnostics.push_error(
                    format!("Cannot override '{}' from '{}'", name, interface_name),
                    &source,
                );
                continue;
            }

            let existing_binding = match &prop_decl.property_type {
                Type::Function(func) => {
                    apply_uses_statement_function_binding(element, &child, name, func)
                }
                _ => element.borrow_mut().bindings.insert(
                    name.clone(),
                    BindingExpression::new_two_way(
                        NamedReference::new(&child, name.clone()).into(),
                    )
                    .into(),
                ),
            };
            debug_assert!(
                existing_binding.is_none(),
                "Duplicate bindings should have been caught earlier"
            );
        }

        if !conflicts.is_empty() {
            diagnostics.push_error(
                format!(
                    "Cannot implement '{interface_name}' based on '{child_id}'.\n{}",
                    conflicts.join("\n")
                ),
                &node.QualifiedName(),
            );
        }
    }
}

fn element_implements_interface(
    element: &ElementRc,
    interface: &ElementRc,
    child_id: &SmolStr,
    interface_name: &SmolStr,
    implement_node: &syntax_nodes::ImplementStatement,
    diagnostics: &mut BuildDiagnostics,
) -> bool {
    let mut errors = Vec::new();
    let mut check = |property_name: &SmolStr, property_declaration: &PropertyDeclaration| {
        let lookup_result = element.borrow().lookup_property(property_name);
        if let Err(conflicts) = property_matches_interface(
            &lookup_result,
            property_declaration,
            property_name,
            Some(child_id),
        ) {
            errors.push(conflicts);
        }
    };

    for (property_name, property_declaration) in interface.borrow().property_declarations.iter() {
        check(property_name, property_declaration);
    }

    if !errors.is_empty() {
        let errors = errors.join("\n");
        diagnostics.push_error(
            format!("Cannot implement '{}' based on '{}'.\n{}", interface_name, child_id, errors),
            &implement_node.DeclaredIdentifier(),
        );
    }

    errors.is_empty()
}

fn missing_type_description(name: &SmolStr, interface_declaration: &PropertyDeclaration) -> String {
    let purity_description = |purity: &Option<bool>| {
        if purity.unwrap_or(false) { "pure " } else { "" }
    };
    let type_description = match interface_declaration.property_type {
        Type::Callback(..) => {
            format!(
                "a '{}{}'",
                purity_description(&interface_declaration.pure),
                interface_declaration.property_type
            )
        }
        Type::Function(..) => {
            format!(
                "a 'public {}{}'",
                purity_description(&interface_declaration.pure),
                interface_declaration.property_type
            )
        }
        _ => {
            format!(
                "an {} '{}' property",
                interface_declaration.visibility, interface_declaration.property_type
            )
        }
    };

    format!("- missing '{name}', {type_description}")
}

fn property_matches_interface(
    property: &PropertyLookupResult,
    interface_declaration: &PropertyDeclaration,
    name: &SmolStr,
    child_id: Option<&SmolStr>,
) -> Result<(), String> {
    if property.property_type == Type::Invalid {
        return Err(missing_type_description(name, interface_declaration));
    }

    let mut errors = Vec::new();

    let member_name =
        if let Some(child_id) = child_id { format!("{child_id}.{name}") } else { name.to_string() };

    if property.property_type != interface_declaration.property_type {
        let type_description = |property_type: &Type| match property_type {
            Type::Callback(..) => {
                format!("a '{}'", property_type)
            }
            Type::Function(..) => {
                format!("a '{}'", property_type)
            }
            _ => {
                format!("a '{}' property", property_type)
            }
        };

        let expected = type_description(&interface_declaration.property_type);
        let actual = type_description(&property.property_type);
        errors.push(format!("- '{member_name}' must be {expected} (found {actual})"));
    }

    if property.property_visibility != interface_declaration.visibility {
        errors.push(format!(
            "- '{member_name}' must be '{}' (found '{}')",
            interface_declaration.visibility, property.property_visibility
        ));
    }

    // The implementation can be "more pure" than the interface, but never less pure.
    if interface_declaration.pure.unwrap_or(false) && !property.declared_pure.unwrap_or(false) {
        errors.push(format!("- '{member_name}' must be 'pure'"));
    }

    if errors.is_empty() { Ok(()) } else { Err(errors.into_iter().join("\n")) }
}

fn apply_uses_statement_function_binding(
    element: &ElementRc,
    child: &ElementRc,
    name: &SmolStr,
    function: &Rc<Function>,
) -> Option<RefCell<BindingExpression>> {
    let args_expr: Vec<Expression> = function
        .args
        .iter()
        .enumerate()
        .map(|(i, ty)| Expression::FunctionParameterReference { index: i, ty: ty.clone() })
        .collect();

    let call_expr = Expression::FunctionCall {
        function: Callable::Function(NamedReference::new(child, name.clone())),
        arguments: args_expr,
        source_location: None,
    };

    let body = Expression::CodeBlock(vec![call_expr]);
    element.borrow_mut().bindings.insert(name.clone(), RefCell::new(BindingExpression::from(body)))
}
