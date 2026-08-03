// Copyright © 2026 Klarälvdalens Datakonsult AB, a KDAB Group company <info@kdab.com>, author Nathan Collins <nathan.collins@kdab.com>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

//! Module containing interfaces related types and functions.

use std::collections::BTreeMap;
use std::rc::Rc;
use std::sync::Arc;

use itertools::Itertools;
use smol_str::SmolStr;

use crate::diagnostics::{BuildDiagnostics, SourceLocation, Spanned};
use crate::expression_tree::{BindingExpression, Callable, Expression};
use crate::langtype::{ElementType, Function, PropertyLookupResult, Type};
use crate::namedreference::NamedReference;
use crate::object_tree::{
    Element, ElementRc, PropertyDeclaration, QualifiedTypeName, find_element_by_id,
};
use crate::parser::{self, SyntaxNode, SyntaxToken};
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
            for prop_name in stmt.interface.borrow().property_declarations.keys() {
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

pub(super) fn validate_self_implement_statements(
    element: &Element,
    implemented_interfaces: &[ImplementedInterface],
    diagnostics: &mut BuildDiagnostics,
) {
    for ImplementedInterface { interface, node, interface_name, .. } in implemented_interfaces {
        validate_interface_implementation(
            element,
            interface,
            interface_name,
            &node.QualifiedName(),
            None,
            diagnostics,
        );
    }
}

struct NoteWithSource {
    note: String,
    source: SourceLocation,
}

struct InterfaceMemberDiagnostics {
    error: String,
    notes: Vec<NoteWithSource>,
}

impl From<String> for InterfaceMemberDiagnostics {
    fn from(error: String) -> Self {
        Self { error, notes: Default::default() }
    }
}

enum DeclarationAnchor {
    Name,
    PropertyType,
    /// The n-th parameter of a callback or function.
    Argument(usize),
    ReturnType,
    Visibility,
    Purity,
}

const VISIBILITY_KEYWORDS: &[&str] =
    &["in", "out", "in-out", "in_out", "private", "public", "protected"];

impl DeclarationAnchor {
    fn source_location(&self, declaration: &SyntaxNode) -> SourceLocation {
        self.narrow(declaration)
            .or_else(|| {
                Some(declaration.child_node(SyntaxKind::DeclaredIdentifier)?.to_source_location())
            })
            .unwrap_or_else(|| declaration.to_source_location())
    }

    fn narrow(&self, declaration: &SyntaxNode) -> Option<SourceLocation> {
        let node = match self {
            Self::Name => return None,
            Self::PropertyType => declaration.child_node(SyntaxKind::Type)?,
            Self::Argument(index) => parameter_type(declaration, *index)?,
            Self::ReturnType => declaration.child_node(SyntaxKind::ReturnType)?,
            Self::Visibility => {
                return Some(keyword_token(declaration, VISIBILITY_KEYWORDS)?.to_source_location());
            }
            Self::Purity => {
                return Some(keyword_token(declaration, &["pure"])?.to_source_location());
            }
        };
        Some(node.to_source_location())
    }
}

fn parameter_type(declaration: &SyntaxNode, index: usize) -> Option<SyntaxNode> {
    let parameter_kind = match declaration.kind() {
        SyntaxKind::Function => SyntaxKind::ArgumentDeclaration,
        SyntaxKind::CallbackDeclaration => SyntaxKind::CallbackDeclarationParameter,
        _ => return None,
    };
    declaration
        .children()
        .filter(|child| child.kind() == parameter_kind)
        .nth(index)?
        .child_node(SyntaxKind::Type)
}

/// Visibility and purity are plain identifier tokens rather than syntax nodes, so they can only be
/// located by their text - the inverse of how [`Element::from_node`] reads them.
fn keyword_token(declaration: &SyntaxNode, keywords: &[&str]) -> Option<SyntaxToken> {
    declaration
        .children_with_tokens()
        .filter_map(|child| child.into_token())
        .find(|token| token.kind() == SyntaxKind::Identifier && keywords.contains(&token.text()))
}

struct MemberViolation {
    error: String,
    note: String,
    anchor: DeclarationAnchor,
}

fn validate_interface_implementation(
    element: &Element,
    interface: &ElementRc,
    interface_name: &SmolStr,
    node: &SyntaxNode,
    child_id: Option<&SmolStr>,
    diagnostics: &mut BuildDiagnostics,
) -> bool {
    let mut errors = Vec::new();
    let mut notes = Vec::new();
    for (member_name, member_declaration) in interface.borrow().property_declarations.iter() {
        if let Some(mut conflict) = validate_interface_member_implementation(
            element,
            member_name,
            member_declaration,
            interface_name,
            child_id,
        ) {
            errors.push(conflict.error);
            notes.append(&mut conflict.notes);
        };
    }

    if !errors.is_empty() {
        let based_on = match child_id {
            Some(child_id) => format!(" based on '{child_id}'"),
            None => String::new(),
        };
        diagnostics.push_error(
            format!("Cannot implement '{interface_name}'{based_on}.\n{}", errors.join("\n")),
            node,
        );

        for note in notes {
            diagnostics.push_note_with_span(note.note, note.source);
        }
    }
    errors.is_empty()
}

fn validate_interface_member_implementation(
    element: &Element,
    member_name: &SmolStr,
    interface_member: &PropertyDeclaration,
    interface_name: &SmolStr,
    child_id: Option<&SmolStr>,
) -> Option<InterfaceMemberDiagnostics> {
    if matches!(interface_member.property_type, Type::Invalid) {
        // The interface's own declaration is invalid (e.g. an unknown property type). A diagnostic
        // was already emitted when the interface was parsed, so there is nothing meaningful to
        // validate here.
        return None;
    }

    let lookup_result = element.lookup_property(member_name);
    let Err(violations) = property_matches_interface(
        &lookup_result,
        interface_member,
        interface_name,
        member_name,
        child_id,
    ) else {
        return None;
    };

    let joined_errors =
        |violations: &[MemberViolation]| violations.iter().map(|v| v.error.as_str()).join("\n");

    if !lookup_result.is_valid() {
        return Some(InterfaceMemberDiagnostics::from(joined_errors(&violations)));
    }

    let base_conflict = (!lookup_result.is_local_to_component && child_id.is_none())
        .then(|| check_property_declaration_conflicts(&lookup_result, &element.base_type).err())
        .flatten();

    let error = match base_conflict {
        Some(error) => error,
        None => joined_errors(&violations),
    };

    let mut conflicts = InterfaceMemberDiagnostics::from(error);
    if let Some(source) = element.property_declaration_node(member_name) {
        conflicts.notes = violations
            .into_iter()
            .map(|violation| NoteWithSource {
                note: violation.note,
                source: violation.anchor.source_location(&source),
            })
            .collect();
    }
    Some(conflicts)
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

        if !validate_interface_implementation(
            &child.borrow(),
            &interface,
            &interface_name,
            &node.DeclaredIdentifier(),
            Some(&child_id),
            diagnostics,
        ) {
            continue;
        }

        let mut conflicts = Vec::new();
        let mut notes = Vec::new();
        for (name, prop_decl) in interface.borrow().property_declarations.iter() {
            let lookup_result = element.borrow().base_type.lookup_property(name);
            if let Err(message) =
                check_property_declaration_conflicts(&lookup_result, &element.borrow().base_type)
            {
                conflicts.push(message);
                if let Some(source) = element.borrow().property_declaration_node(name) {
                    notes.push(NoteWithSource {
                        note: declares_as_note(&interface_name, name, prop_decl),
                        source: DeclarationAnchor::Name.source_location(&source),
                    });
                }
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
                diagnostics.push_note(
                    declares_as_note(&interface_name, name, &prop_decl),
                    &node.QualifiedName(),
                );
                continue;
            }

            let existing_binding = match &prop_decl.property_type {
                Type::Function(func) => {
                    apply_uses_statement_function_binding(element, &child, name, func)
                }
                _ => element.borrow_mut().set_binding(
                    name.clone(),
                    BindingExpression::new_two_way(
                        NamedReference::new(&child, name.clone()).into(),
                    ),
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
            for note in notes {
                diagnostics.push_note(note.note, &note.source);
            }
        }
    }
}

fn purity_description(purity: &Option<bool>) -> &str {
    if purity.unwrap_or(false) { "pure " } else { "" }
}

fn missing_type_description(interface_declaration: &PropertyDeclaration) -> String {
    match interface_declaration.property_type {
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
    }
}

fn syntax_for(interface_declaration: &PropertyDeclaration, name: &SmolStr) -> String {
    let display_args =
        |arguments: &Vec<Type>| -> String { arguments.iter().map(|t| t.to_string()).join(", ") };
    let return_type = |return_type: &Type| -> String {
        if *return_type == Type::Void { String::new() } else { format!(" -> {return_type}") }
    };
    match &interface_declaration.property_type {
        Type::Function(function) => format!(
            "{}{} function {name}({}){} {{ }}",
            purity_description(&interface_declaration.pure),
            interface_declaration.visibility,
            display_args(&function.args),
            return_type(&function.return_type)
        ),
        Type::Callback(function) => format!(
            "{}callback {name}({}){};",
            purity_description(&interface_declaration.pure),
            display_args(&function.args),
            return_type(&function.return_type)
        ),
        _ if interface_declaration.property_type.is_property_type() => format!(
            "{} property <{}> {name};",
            interface_declaration.visibility, interface_declaration.property_type
        ),
        _ => name.to_string(),
    }
}

fn missing_type_error(name: &SmolStr, interface_declaration: &PropertyDeclaration) -> String {
    format!("- missing '{}'", syntax_for(interface_declaration, name))
}

fn declares_as_note(
    interface_name: &SmolStr,
    name: &SmolStr,
    interface_declaration: &PropertyDeclaration,
) -> String {
    format!("'{interface_name}' declares '{name}' as '{}'", syntax_for(interface_declaration, name))
}

fn signature_anchor(interface_declaration: &Function, declaration: &Function) -> DeclarationAnchor {
    if let Some(index) = interface_declaration
        .args
        .iter()
        .zip(declaration.args.iter())
        .position(|(expected, declared)| expected != declared)
    {
        DeclarationAnchor::Argument(index)
    } else if declaration.args.len() > interface_declaration.args.len() {
        DeclarationAnchor::Argument(interface_declaration.args.len())
    } else if declaration.args.len() < interface_declaration.args.len() {
        DeclarationAnchor::Name
    } else {
        DeclarationAnchor::ReturnType
    }
}

/// [PartialEq] for [Function] means that the argument names must match. That is not required for a valid interface implementation.
fn function_matches_for_interface(lhs: &Function, rhs: &Function) -> bool {
    lhs.return_type == rhs.return_type && lhs.args == rhs.args
}

fn property_type_matches_for_interface(lhs: &Type, rhs: &Type) -> bool {
    match (lhs, rhs) {
        (Type::Callback(lhs), Type::Callback(rhs)) => function_matches_for_interface(lhs, rhs),
        (Type::Function(lhs), Type::Function(rhs)) => function_matches_for_interface(lhs, rhs),
        _ => lhs == rhs,
    }
}

fn property_matches_interface(
    property: &PropertyLookupResult,
    interface_declaration: &PropertyDeclaration,
    interface_name: &SmolStr,
    name: &SmolStr,
    child_id: Option<&SmolStr>,
) -> Result<(), Vec<MemberViolation>> {
    if property.property_type == Type::Invalid {
        return Err(vec![MemberViolation {
            error: missing_type_error(name, interface_declaration),
            note: declares_as_note(interface_name, name, interface_declaration),
            anchor: DeclarationAnchor::Name,
        }]);
    }

    let mut errors = Vec::new();

    let member_name =
        if let Some(child_id) = child_id { format!("{child_id}.{name}") } else { name.to_string() };

    if !property_type_matches_for_interface(
        &property.property_type,
        &interface_declaration.property_type,
    ) {
        let is_same_type = match (&interface_declaration.property_type, &property.property_type) {
            (Type::Callback(..), Type::Callback(..)) | (Type::Function(..), Type::Function(..)) => {
                true
            }
            (lhs, rhs) => lhs.is_property_type() && rhs.is_property_type(),
        };

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

        let expected = if !is_same_type {
            missing_type_description(interface_declaration)
        } else {
            type_description(&interface_declaration.property_type)
        };

        let actual = type_description(&property.property_type);
        let error = format!("- '{member_name}' must be {expected} (found {actual})");

        if !is_same_type {
            // Visibility and purity are unlikely to make sense, so return early in this case.
            return Err(vec![MemberViolation {
                error,
                note: declares_as_note(interface_name, name, interface_declaration),
                anchor: DeclarationAnchor::Name,
            }]);
        }

        let (note, anchor) = match (&interface_declaration.property_type, &property.property_type) {
            (Type::Callback(expected), Type::Callback(declared))
            | (Type::Function(expected), Type::Function(declared)) => (
                declares_as_note(interface_name, name, interface_declaration),
                signature_anchor(expected, declared),
            ),
            (_, _) => (
                declares_as_note(interface_name, name, interface_declaration),
                DeclarationAnchor::PropertyType,
            ),
        };
        errors.push(MemberViolation { error, note, anchor });
    }

    if property.property_visibility != interface_declaration.visibility {
        errors.push(MemberViolation {
            error: format!(
                "- '{member_name}' must be '{}' (found '{}')",
                interface_declaration.visibility, property.property_visibility
            ),
            note: declares_as_note(interface_name, name, interface_declaration),
            anchor: DeclarationAnchor::Visibility,
        });
    }

    // The implementation can be "more pure" than the interface, but never less pure.
    if interface_declaration.pure.unwrap_or(false) && !property.declared_pure.unwrap_or(false) {
        errors.push(MemberViolation {
            error: format!("- '{member_name}' must be 'pure'"),
            note: declares_as_note(interface_name, name, interface_declaration),
            anchor: DeclarationAnchor::Purity,
        });
    }

    if errors.is_empty() { Ok(()) } else { Err(errors) }
}

fn apply_uses_statement_function_binding(
    element: &ElementRc,
    child: &ElementRc,
    name: &SmolStr,
    function: &Arc<Function>,
) -> Option<BindingExpression> {
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
    element.borrow_mut().set_binding(name.clone(), BindingExpression::from(body))
}
