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
    Component, Element, ElementRc, PropertyDeclaration, QualifiedTypeName, find_element_by_id,
};
use crate::parser;
use crate::parser::{SyntaxKind, syntax_nodes};
use crate::typeregister::TypeRegister;

/// A parsed [syntax_nodes::UsesIdentifier].
#[derive(Clone, Debug)]
struct UsesStatement {
    interface_name: QualifiedTypeName,
    child_id: SmolStr,
    node: syntax_nodes::UsesIdentifier,
}

impl UsesStatement {
    /// Get the node representing the interface name.
    fn interface_name_node(&self) -> syntax_nodes::QualifiedName {
        self.node.QualifiedName()
    }

    /// Get the node representing the child identifier.
    fn child_id_node(&self) -> syntax_nodes::DeclaredIdentifier {
        self.node.DeclaredIdentifier()
    }

    /// Lookup the interface component for this uses statement. Emits an error if the iterface could not be found, or
    /// was not actually an interface.
    fn lookup_interface(
        &self,
        tr: &TypeRegister,
        diag: &mut BuildDiagnostics,
    ) -> Result<Rc<Component>, ()> {
        let interface_name = self.interface_name.to_smolstr();
        match tr.lookup_element(&interface_name) {
            Ok(element_type) => match element_type {
                ElementType::Component(component) => {
                    if !component.is_interface() {
                        diag.push_error(
                            format!("'{}' is not an interface", self.interface_name),
                            &self.interface_name_node(),
                        );
                        return Err(());
                    }

                    Ok(component)
                }
                _ => {
                    diag.push_error(
                        format!("'{}' is not an interface", self.interface_name),
                        &self.interface_name_node(),
                    );
                    Err(())
                }
            },
            Err(error) => {
                diag.push_error(error, &self.interface_name_node());
                Err(())
            }
        }
    }
}

impl From<&syntax_nodes::UsesIdentifier> for UsesStatement {
    fn from(node: &syntax_nodes::UsesIdentifier) -> UsesStatement {
        UsesStatement {
            interface_name: QualifiedTypeName::from_node(
                node.child_node(SyntaxKind::QualifiedName).unwrap().clone().into(),
            ),
            child_id: parser::identifier_text(&node.DeclaredIdentifier()).unwrap_or_default(),
            node: node.clone(),
        }
    }
}

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

/// An ImplementsSpecifier and the corresponding interface element.
pub(super) struct ImplementedInterface {
    implements_specifier: syntax_nodes::ImplementsSpecifier,
    interface: ElementRc,
    interface_name: SmolStr,
}

/// If the element implements a valid interface, return the corresponding ImplementedInterface. Otherwise return None.
/// Emits diagnostics if the implements specifier is invalid.
pub(super) fn get_implemented_interface(
    e: &Element,
    node: &syntax_nodes::Element,
    tr: &TypeRegister,
    diag: &mut BuildDiagnostics,
) -> Option<ImplementedInterface> {
    let parent: syntax_nodes::Component =
        node.parent().filter(|p| p.kind() == SyntaxKind::Component)?.into();

    let implements_specifier = parent.ImplementsSpecifier()?;

    if !diag.enable_experimental && !tr.expose_internal_types {
        diag.push_error("'implements' is an experimental feature".into(), &implements_specifier);
        return None;
    }

    let interface_name =
        QualifiedTypeName::from_node(implements_specifier.QualifiedName()).to_smolstr();

    match e.base_type.lookup_type_for_child_element(&interface_name, tr) {
        Ok(ElementType::Component(c)) => {
            if !c.is_interface() {
                diag.push_error(
                    format!("Cannot implement {}. It is not an interface", interface_name),
                    &implements_specifier.QualifiedName(),
                );
                return None;
            }

            c.used.set(true);
            Some(ImplementedInterface {
                implements_specifier,
                interface: c.root_element.clone(),
                interface_name,
            })
        }
        Ok(_) => {
            diag.push_error(
                format!("Cannot implement {}. It is not an interface", interface_name),
                &implements_specifier.QualifiedName(),
            );
            None
        }
        Err(err) => {
            diag.push_error(err, &implements_specifier.QualifiedName());
            None
        }
    }
}

pub(super) fn apply_implements_specifier(
    e: &mut Element,
    implemented_interface: &Option<ImplementedInterface>,
    diag: &mut BuildDiagnostics,
) {
    let Some(ImplementedInterface { interface, implements_specifier, interface_name }) =
        implemented_interface
    else {
        return;
    };

    for (unresolved_prop_name, prop_decl) in
        interface.borrow().property_declarations.iter().filter(|(_, prop_decl)| {
            // Functions are expected to be implemented manually, so we don't automatically add them.
            !matches!(prop_decl.property_type, Type::Function { .. })
        })
    {
        let lookup_result = e.lookup_property(unresolved_prop_name);
        if let Err(message) = validate_property_declaration_for_interface(
            InterfaceUseMode::Implements,
            &lookup_result,
            &e.base_type,
            &interface_name,
        ) {
            diag.push_error(message, &implements_specifier.QualifiedName());
            continue;
        }

        e.property_declarations.insert(unresolved_prop_name.clone(), prop_decl.clone());
    }
}

/// Apply default property values defined in the interface to the element.
pub(super) fn apply_interface_default_property_values(
    e: &mut Element,
    implemented_interface: &Option<ImplementedInterface>,
) {
    let Some(ImplementedInterface { interface, .. }) = implemented_interface else {
        return;
    };

    for (property_name, _) in
        interface.borrow().property_declarations.iter().filter(|(_, prop_decl)| {
            // Only apply default bindings for properties
            !matches!(prop_decl.property_type, Type::Function { .. } | Type::Callback { .. })
        })
    {
        if let Some(binding) = interface.borrow().bindings.get(property_name) {
            e.bindings.entry(property_name.clone()).or_insert_with(|| binding.clone());
        }
    }
}

/// Validate that the functions declared in the interface are correctly implemented in the element. Emits diagnostics if not.
pub(super) fn validate_function_implementations_for_interface(
    e: &Element,
    implemented_interface: &Option<ImplementedInterface>,
    diag: &mut BuildDiagnostics,
) {
    let Some(ImplementedInterface { interface, implements_specifier, interface_name }) =
        implemented_interface
    else {
        return;
    };

    for (function_name, function_property_decl) in interface
        .borrow()
        .property_declarations
        .iter()
        .filter(|(_, prop_decl)| matches!(prop_decl.property_type, Type::Function { .. }))
    {
        let Type::Function(ref function_declaration) = function_property_decl.property_type else {
            debug_assert!(false, "Non-functions should have been filtered out already");
            continue;
        };

        let push_interface_error = |diag: &mut BuildDiagnostics, is_local_to_component, error| {
            if is_local_to_component {
                let source = e
                    .property_declarations
                    .get(function_name)
                    .and_then(|decl| decl.node.clone())
                    .map_or_else(
                        || parser::NodeOrToken::Node(implements_specifier.QualifiedName().into()),
                        parser::NodeOrToken::Node,
                    );
                diag.push_error(error, &source);
            } else {
                diag.push_error(error, &implements_specifier.QualifiedName());
            }
        };

        let found_function = e.lookup_property(function_name);
        let function_impl = match found_function.property_type {
            Type::Invalid => {
                diag.push_error(
                    format!("Missing implementation of function '{}'", function_name),
                    &implements_specifier.QualifiedName(),
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

pub(super) fn apply_uses_statement(
    e: &ElementRc,
    uses_specifier: Option<syntax_nodes::UsesSpecifier>,
    tr: &TypeRegister,
    diag: &mut BuildDiagnostics,
) {
    let Some(uses_specifier) = uses_specifier else {
        return;
    };

    if !diag.enable_experimental && !tr.expose_internal_types {
        diag.push_error("'uses' is an experimental feature".into(), &uses_specifier);
        return;
    }

    let uses_statements = gather_valid_uses_statements(e, tr, diag, uses_specifier);
    let uses_statements = filter_conflicting_uses_statements(diag, uses_statements);

    for ValidUsesStatement { uses_statement, interface, child } in uses_statements {
        for (name, prop_decl) in interface.borrow().property_declarations.iter() {
            let lookup_result = e.borrow().base_type.lookup_property(name);
            if let Err(message) = validate_property_declaration_for_interface(
                InterfaceUseMode::Uses,
                &lookup_result,
                &e.borrow().base_type,
                &uses_statement.interface_name,
            ) {
                diag.push_error(message, &uses_statement.interface_name_node());
                continue;
            }

            // Replace the node with the interface name for better diagnostics later, since the declaration won't have a
            // node in this element.
            let mut prop_decl = prop_decl.clone();
            prop_decl.node = Some(uses_statement.interface_name_node().into());

            if let Some(existing_property) =
                e.borrow_mut().property_declarations.insert(name.clone(), prop_decl.clone())
            {
                let source = existing_property
                    .node
                    .as_ref()
                    .and_then(|node| node.child_node(SyntaxKind::DeclaredIdentifier))
                    .and_then(|node| node.child_token(SyntaxKind::Identifier))
                    .map_or_else(
                        || parser::NodeOrToken::Node(uses_statement.child_id_node().into()),
                        parser::NodeOrToken::Token,
                    );

                diag.push_error(
                    format!("Cannot override '{}' from '{}'", name, uses_statement.interface_name),
                    &source,
                );
                continue;
            }

            let exisitng_binding = match &prop_decl.property_type {
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

            if let Some(existing_binding) = exisitng_binding {
                let message = format!(
                    "Cannot override binding for '{}' from interface '{}'",
                    name, uses_statement.interface_name
                );
                if let Some(location) = &existing_binding.borrow().span {
                    diag.push_error(message, location);
                } else {
                    diag.push_error(message, &uses_statement.interface_name_node());
                }
            }
        }
    }
}

/// A valid `uses` statement, containing the looked up interface and child element.
struct ValidUsesStatement {
    uses_statement: UsesStatement,
    interface: ElementRc,
    child: ElementRc,
}

/// Gather valid `uses` statements, emitting diagnostics for invalid ones. A valid `uses` statement is one where the
/// interface can be found, the child element can be found, and the child element implements the interface.
fn gather_valid_uses_statements(
    e: &Rc<RefCell<Element>>,
    tr: &TypeRegister,
    diag: &mut BuildDiagnostics,
    uses_specifier: syntax_nodes::UsesSpecifier,
) -> Vec<ValidUsesStatement> {
    let mut valid_uses_statements: Vec<ValidUsesStatement> = Vec::new();

    for uses_identifier_node in uses_specifier.UsesIdentifier() {
        let uses_statement: UsesStatement = (&uses_identifier_node).into();
        let Ok(interface_component) = uses_statement.lookup_interface(tr, diag) else {
            continue;
        };

        let Some(child) = find_element_by_id(e, &uses_statement.child_id) else {
            diag.push_error(
                format!("'{}' does not exist", uses_statement.child_id),
                &uses_statement.child_id_node(),
            );
            continue;
        };

        let interface = interface_component.root_element.clone();
        if !element_implements_interface(&child, &interface, &uses_statement, diag) {
            continue;
        }

        valid_uses_statements.push(ValidUsesStatement { uses_statement, interface, child });
    }
    valid_uses_statements
}

/// Filter out conflicting `uses` statements, emitting diagnostics for each conflict. Two `uses` statements conflict if
/// they introduce properties/callbacks/functions with the same name. In that case we keep the first one and filter out
/// the rest.
fn filter_conflicting_uses_statements(
    diag: &mut BuildDiagnostics,
    uses_statements: Vec<ValidUsesStatement>,
) -> Vec<ValidUsesStatement> {
    let mut seen_interfaces: Vec<SmolStr> = Vec::new();
    let mut seen_interface_api: BTreeMap<SmolStr, SmolStr> = BTreeMap::new();
    let valid_uses_statements: Vec<ValidUsesStatement> = uses_statements
        .into_iter()
        .filter(|vus| {
            let interface_name = vus.uses_statement.interface_name.to_smolstr();
            if seen_interfaces.contains(&interface_name) {
                diag.push_error(
                    format!("'{}' is used multiple times", vus.uses_statement.interface_name),
                    &vus.uses_statement.interface_name_node(),
                );
                return false;
            }
            seen_interfaces.push(interface_name.clone());

            let mut valid = true;
            for (prop_name, _) in vus.interface.borrow().property_declarations.iter() {
                if let Some(existing_interface) = seen_interface_api.get(prop_name) {
                    diag.push_error(
                        format!(
                            "'{}' occurs in '{}' and '{}'",
                            prop_name, vus.uses_statement.interface_name, existing_interface
                        ),
                        &vus.uses_statement.interface_name_node(),
                    );
                    valid = false;
                } else {
                    seen_interface_api.insert(prop_name.clone(), interface_name.clone());
                }
            }
            valid
        })
        .collect();
    valid_uses_statements
}

/// Check that the given element implements the given interface. Emits a diagnostic if the interface is not implemented.
fn element_implements_interface(
    element: &ElementRc,
    interface: &ElementRc,
    uses_statement: &UsesStatement,
    diag: &mut BuildDiagnostics,
) -> bool {
    let property_matches_interface = |property: &PropertyLookupResult,
                                      interface_declaration: &PropertyDeclaration|
     -> Result<(), String> {
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
            errors.push(format!(
                "purity declaration: '{}'",
                interface_declaration.pure.unwrap_or(false)
            ));
        }

        if errors.is_empty() {
            Ok(())
        } else {
            Err(format!("expected {}", errors.into_iter().join(", ")))
        }
    };

    let mut valid = true;
    let mut check = |property_name: &SmolStr, property_declaration: &PropertyDeclaration| {
        let lookup_result = element.borrow().lookup_property(property_name);
        if let Err(e) = property_matches_interface(&lookup_result, property_declaration) {
            diag.push_error(
                format!(
                    "'{}' does not implement '{}' from '{}' - {}",
                    uses_statement.child_id, property_name, uses_statement.interface_name, e
                ),
                &uses_statement.child_id_node(),
            );
            valid = false;
        }
    };

    for (property_name, property_declaration) in interface.borrow().property_declarations.iter() {
        check(property_name, property_declaration);
    }

    valid
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
