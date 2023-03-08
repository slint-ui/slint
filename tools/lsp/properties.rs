// Copyright © SixtyFPS GmbH <info@slint-ui.com>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-commercial

use crate::server_loop::{DocumentCache, OffsetToPositionMapper};
use crate::Error;

use i_slint_compiler::diagnostics::{BuildDiagnostics, Spanned};
use i_slint_compiler::langtype::{ElementType, Type};
use i_slint_compiler::object_tree::{Element, ElementRc, PropertyDeclaration, PropertyVisibility};
use i_slint_compiler::parser::{syntax_nodes, Language, SyntaxKind};
use rowan::TextRange;

use std::collections::HashSet;

#[cfg(target_arch = "wasm32")]
use crate::wasm_prelude::*;

#[derive(serde::Deserialize, serde::Serialize, Clone, Debug, PartialEq)]
pub(crate) struct DefinitionInformation {
    property_definition_range: lsp_types::Range,
    selection_range: lsp_types::Range,
    expression_range: lsp_types::Range,
    expression_value: String,
}

#[derive(serde::Deserialize, serde::Serialize, Clone, Debug, PartialEq)]
pub(crate) struct DeclarationInformation {
    uri: lsp_types::Url,
    start_position: lsp_types::Position,
}

#[derive(serde::Deserialize, serde::Serialize, Clone, Debug, PartialEq)]
pub(crate) struct PropertyInformation {
    name: String,
    type_name: String,
    declared_at: Option<DeclarationInformation>,
    defined_at: Option<DefinitionInformation>, // Range in the elements source file!
    group: String,
}

#[derive(serde::Deserialize, serde::Serialize, Debug)]
pub(crate) struct ElementInformation {
    id: String,
    type_name: String,
    range: Option<lsp_types::Range>,
}

#[derive(serde::Deserialize, serde::Serialize)]
pub(crate) struct QueryPropertyResponse {
    properties: Vec<PropertyInformation>,
    element: Option<ElementInformation>,
    source_uri: String,
    source_version: i32,
}

impl QueryPropertyResponse {
    pub fn no_element_response(source_uri: String, source_version: i32) -> Self {
        QueryPropertyResponse { properties: vec![], element: None, source_uri, source_version }
    }
}

#[derive(Debug, serde::Deserialize, serde::Serialize)]
pub(crate) struct SetBindingResponse {
    diagnostics: Vec<lsp_types::Diagnostic>,
}

// This gets defined accessibility properties...
fn get_reserved_properties<'a>(
    group: &'a str,
    properties: &'a [(&'a str, Type)],
) -> impl Iterator<Item = PropertyInformation> + 'a {
    properties.iter().map(|p| PropertyInformation {
        name: p.0.to_string(),
        type_name: format!("{}", p.1),
        declared_at: None,
        defined_at: None,
        group: group.to_string(),
    })
}

fn source_file(element: &Element) -> Option<String> {
    element.source_file().map(|sf| sf.path().to_string_lossy().to_string())
}

fn property_is_editable(property: &PropertyDeclaration, is_local_element: bool) -> bool {
    if !property.property_type.is_property_type() {
        // Filter away the callbacks
        return false;
    }
    if matches!(property.visibility, PropertyVisibility::Output | PropertyVisibility::Private)
        && !is_local_element
    {
        // Skip properties that cannot be set because of visibility rules
        return false;
    }
    if property.type_node().is_none() {
        return false;
    }

    true
}

fn add_element_properties(
    element: &Element,
    offset_to_position: &OffsetToPositionMapper,
    group: &str,
    is_local_element: bool,
    result: &mut Vec<PropertyInformation>,
) {
    let file = source_file(element);

    result.extend(element.property_declarations.iter().filter_map(move |(name, value)| {
        if !property_is_editable(value, is_local_element) {
            return None;
        }

        let type_node = value.type_node()?; // skip fake and materialized properties
        let declared_at = file.as_ref().map(|file| {
            let start_position = offset_to_position.map_node(&type_node).start;
            let uri = lsp_types::Url::from_file_path(file).unwrap_or_else(|_| {
                lsp_types::Url::parse("file:///)").expect("That should have been valid as URL!")
            });

            DeclarationInformation { uri, start_position }
        });
        Some(PropertyInformation {
            name: name.clone(),
            type_name: value.property_type.to_string(),
            declared_at,
            defined_at: None,
            group: group.to_string(),
        })
    }))
}

/// Move left from the start of a `token` to include white-space and comments that go with it.
fn left_extend(token: rowan::SyntaxToken<Language>) -> rowan::SyntaxToken<Language> {
    let mut current_token = token.prev_token();
    let mut start_token = token.clone();
    let mut last_comment = token;

    // Walk backwards:
    while let Some(t) = current_token {
        if t.kind() == SyntaxKind::Whitespace {
            let lbs = t.text().matches('\n').count();
            if lbs >= 1 {
                start_token = last_comment.clone();
            }
            if lbs >= 2 {
                break;
            }
            current_token = t.prev_token();
            continue;
        }
        if t.kind() == SyntaxKind::Comment {
            last_comment = t.clone();
            current_token = t.prev_token();
            continue;
        }
        break;
    }

    start_token
}

/// Move right from the end of the `token` to include white-space and comments that go with it.
fn right_extend(token: rowan::SyntaxToken<Language>) -> rowan::SyntaxToken<Language> {
    let mut current_token = token.next_token();
    let mut end_token = token.clone();
    let mut last_comment = token;

    // Walk forwards:
    while let Some(t) = current_token {
        if t.kind() == SyntaxKind::RBrace {
            // All comments between us and a `}` belong to us!
            end_token = last_comment;
            break;
        }
        if t.kind() == SyntaxKind::Whitespace {
            let lbs = t.text().matches('\n').count();
            if lbs > 0 {
                // comments in the current line belong to us, *if* there is a linebreak
                end_token = last_comment;
                break;
            }
            current_token = t.next_token();
            continue;
        }
        if t.kind() == SyntaxKind::Comment {
            last_comment = t.clone();
            current_token = t.next_token();
            continue;
        }

        // in all other cases: Leave the comment to the following token!
        break;
    }

    end_token
}

fn find_expression_range(
    element: &syntax_nodes::Element,
    offset: u32,
    offset_to_position: &OffsetToPositionMapper,
) -> Option<DefinitionInformation> {
    let mut selection_range = None;
    let mut expression_range = None;
    let mut expression_value = None;
    let mut property_definition_range = None;

    if let Some(token) = element.token_at_offset(offset.into()).right_biased() {
        for ancestor in token.parent_ancestors() {
            if ancestor.kind() == SyntaxKind::BindingExpression {
                // The BindingExpression contains leading and trailing whitespace + `;`
                let expr = &ancestor.first_child();
                expression_range = expr.as_ref().map(|e| e.text_range());
                expression_value = expr.as_ref().map(|e| e.text().to_string());
                continue;
            }
            if (ancestor.kind() == SyntaxKind::Binding)
                || (ancestor.kind() == SyntaxKind::PropertyDeclaration)
            {
                property_definition_range = Some(ancestor.text_range());
                selection_range = Some(rowan::TextRange::new(
                    left_extend(ancestor.first_token()?).text_range().start(),
                    right_extend(ancestor.last_token()?).text_range().end(),
                ))
                .or_else(|| property_definition_range.clone());
                break;
            }
            if ancestor.kind() == SyntaxKind::Element {
                // There should have been a binding before the element!
                break;
            }
        }
    }
    Some(DefinitionInformation {
        property_definition_range: offset_to_position.map_range(property_definition_range?),
        selection_range: offset_to_position.map_range(selection_range?),
        expression_range: offset_to_position.map_range(expression_range?),
        expression_value: expression_value?,
    })
}

fn find_property_binding_offset(element: &Element, property_name: &str) -> Option<u32> {
    let element_range = element.node.as_ref()?.text_range();

    if let Some(v) = element.bindings.get(property_name) {
        if let Some(span) = &v.borrow().span {
            let offset = span.span().offset as u32;
            if element.source_file().map(|sf| sf.path())
                == span.source_file.as_ref().map(|sf| sf.path())
                && element_range.contains(offset.into())
            {
                return Some(offset);
            }
        }
    }

    None
}

fn insert_property_definitions(
    element: &Element,
    properties: &mut Vec<PropertyInformation>,
    offset_to_position: &OffsetToPositionMapper,
) {
    if let Some(element_node) = element.node.as_ref() {
        for prop_info in properties {
            if let Some(offset) = find_property_binding_offset(element, prop_info.name.as_str()) {
                prop_info.defined_at =
                    find_expression_range(element_node, offset, offset_to_position);
            }
        }
    }
}

fn get_properties(
    element: &ElementRc,
    offset_to_position: &OffsetToPositionMapper,
) -> Vec<PropertyInformation> {
    let mut result = Vec::new();
    add_element_properties(&element.borrow(), offset_to_position, "", true, &mut result);

    let mut current_element = element.clone();

    let geometry_prop = HashSet::from(["x", "y", "width", "height"]);

    loop {
        let base_type = current_element.borrow().base_type.clone();
        match base_type {
            ElementType::Component(c) => {
                current_element = c.root_element.clone();
                add_element_properties(
                    &current_element.borrow(),
                    offset_to_position,
                    &c.id,
                    false,
                    &mut result,
                );
                continue;
            }
            ElementType::Builtin(b) => {
                result.extend(b.properties.iter().filter_map(|(k, t)| {
                    if geometry_prop.contains(k.as_str()) {
                        // skip geometry property because they are part of the reserved ones
                        return None;
                    }
                    if !t.ty.is_property_type() {
                        // skip callbacks and other functions
                        return None;
                    }
                    if t.property_visibility == PropertyVisibility::Output {
                        // Skip output-only properties
                        return None;
                    }

                    Some(PropertyInformation {
                        name: k.clone(),
                        type_name: t.ty.to_string(),
                        declared_at: None,
                        defined_at: None,
                        group: b.name.clone(),
                    })
                }));

                if b.name == "Rectangle" {
                    result.push(PropertyInformation {
                        name: "clip".into(),
                        type_name: Type::Bool.to_string(),
                        declared_at: None,
                        defined_at: None,
                        group: String::new(),
                    });
                }

                result.push(PropertyInformation {
                    name: "opacity".into(),
                    type_name: Type::Float32.to_string(),
                    declared_at: None,
                    defined_at: None,
                    group: String::new(),
                });
                result.push(PropertyInformation {
                    name: "visible".into(),
                    type_name: Type::Bool.to_string(),
                    declared_at: None,
                    defined_at: None,
                    group: String::new(),
                });

                if b.name == "Image" {
                    result.extend(get_reserved_properties(
                        "rotation",
                        i_slint_compiler::typeregister::RESERVED_ROTATION_PROPERTIES,
                    ));
                }

                if b.name == "Rectangle" {
                    result.extend(get_reserved_properties(
                        "drop-shadow",
                        i_slint_compiler::typeregister::RESERVED_DROP_SHADOW_PROPERTIES,
                    ));
                }
            }
            ElementType::Global => {
                break;
            }

            _ => {}
        }

        result.extend(get_reserved_properties(
            "geometry",
            i_slint_compiler::typeregister::RESERVED_GEOMETRY_PROPERTIES,
        ));
        result.extend(
            get_reserved_properties(
                "layout",
                i_slint_compiler::typeregister::RESERVED_LAYOUT_PROPERTIES,
            )
            // padding arbitrary items is not yet implemented
            .filter(|x| !x.name.starts_with("padding")),
        );
        result.push(PropertyInformation {
            name: "accessible-role".into(),
            type_name: Type::Enumeration(
                i_slint_compiler::typeregister::BUILTIN_ENUMS.with(|e| e.AccessibleRole.clone()),
            )
            .to_string(),
            declared_at: None,
            defined_at: None,
            group: "accessibility".into(),
        });
        if element.borrow().is_binding_set("accessible-role", true) {
            result.extend(get_reserved_properties(
                "accessibility",
                i_slint_compiler::typeregister::RESERVED_ACCESSIBILITY_PROPERTIES,
            ));
        }
        break;
    }

    insert_property_definitions(&element.borrow(), &mut result, offset_to_position);

    result
}

fn find_block_range(element: &ElementRc) -> Option<TextRange> {
    let element = element.borrow();
    let node = element.node.as_ref();

    let open_brace = node?.child_token(SyntaxKind::LBrace)?;
    let close_brace = node?.child_token(SyntaxKind::RBrace)?;

    Some(TextRange::new(open_brace.text_range().start(), close_brace.text_range().end()))
}

fn get_element_information(
    element: &ElementRc,
    offset_to_position: &OffsetToPositionMapper,
) -> ElementInformation {
    let e = element.borrow();

    ElementInformation {
        id: e.id.clone(),
        type_name: e.base_type.to_string(),
        range: e.node.as_ref().map(|n| offset_to_position.map_node(n)),
    }
}

pub(crate) fn query_properties(
    document_cache: &mut DocumentCache,
    uri: &lsp_types::Url,
    source_version: i32,
    element: &ElementRc,
) -> Result<QueryPropertyResponse, crate::Error> {
    let mapper = document_cache.offset_to_position_mapper(uri)?;

    Ok(QueryPropertyResponse {
        properties: get_properties(element, &mapper),
        element: Some(get_element_information(element, &mapper)),
        source_uri: uri.to_string(),
        source_version,
    })
}

fn get_property_information(
    properties: &[PropertyInformation],
    property_name: &str,
) -> Result<PropertyInformation, Error> {
    if let Some(property) = properties.iter().find(|pi| pi.name == property_name) {
        Ok(property.clone())
    } else {
        Err(format!("Element has no property with name {property_name}").into())
    }
}

fn validate_property_expression_type(
    property: &PropertyInformation,
    new_expression_type: Type,
    diag: &mut BuildDiagnostics,
) {
    // Check return type match:
    if new_expression_type != i_slint_compiler::langtype::Type::Invalid
        && new_expression_type.to_string() != property.type_name
    {
        diag.push_error_with_span(
            format!(
                "return type mismatch in \"{}\" (was: {new_expression_type}, expected: {})",
                property.name, property.type_name
            ),
            i_slint_compiler::diagnostics::SourceLocation {
                source_file: None,
                span: i_slint_compiler::diagnostics::Span::new(0),
            },
        );
    }
}

fn create_workspace_edit_for_set_binding_on_existing_property(
    uri: &lsp_types::Url,
    version: i32,
    property: &PropertyInformation,
    new_expression: String,
) -> Option<lsp_types::WorkspaceEdit> {
    property.defined_at.as_ref().map(|defined_at| {
        let edit =
            lsp_types::TextEdit { range: defined_at.expression_range, new_text: new_expression };
        let edits = vec![lsp_types::OneOf::Left(edit)];
        let text_document_edits = vec![lsp_types::TextDocumentEdit {
            text_document: lsp_types::OptionalVersionedTextDocumentIdentifier::new(
                uri.clone(),
                version,
            ),
            edits,
        }];
        lsp_types::WorkspaceEdit {
            document_changes: Some(lsp_types::DocumentChanges::Edits(text_document_edits)),
            ..Default::default()
        }
    })
}

fn set_binding_on_existing_property(
    document_cache: &mut DocumentCache,
    uri: &lsp_types::Url,
    property: &PropertyInformation,
    new_expression: String,
    diag: &mut BuildDiagnostics,
) -> Result<(SetBindingResponse, Option<lsp_types::WorkspaceEdit>), Error> {
    let workspace_edit = (!diag.has_error())
        .then(|| {
            create_workspace_edit_for_set_binding_on_existing_property(
                uri,
                document_cache.document_version(uri)?,
                property,
                new_expression,
            )
        })
        .flatten();

    Ok((
        SetBindingResponse {
            diagnostics: diag.iter().map(crate::util::to_lsp_diag).collect::<Vec<_>>(),
        },
        workspace_edit,
    ))
}

enum InsertPosition {
    Before,
    After,
}

fn find_insert_position_relative_to_defined_properties(
    properties: &[PropertyInformation],
    property_name: &str,
) -> Option<(lsp_types::Range, InsertPosition)> {
    let mut previous_property = None;
    let mut property_index = usize::MAX;

    for (i, p) in properties.iter().enumerate() {
        if p.name == property_name {
            property_index = i;
        } else if let Some(defined_at) = &p.defined_at {
            if property_index == usize::MAX {
                previous_property = Some((i, defined_at.selection_range.end));
            } else {
                if let Some((pi, pp)) = previous_property {
                    if (i - property_index) >= (property_index - pi) {
                        return Some((lsp_types::Range::new(pp, pp), InsertPosition::After));
                    }
                }
                let p = defined_at.selection_range.start;
                return Some((lsp_types::Range::new(p, p), InsertPosition::Before));
            }
        }
    }

    None
}

fn find_insert_range_for_property(
    block_range: &Option<lsp_types::Range>,
    properties: &[PropertyInformation],
    property_name: &str,
) -> Option<(lsp_types::Range, InsertPosition)> {
    find_insert_position_relative_to_defined_properties(properties, property_name).or_else(|| {
        // No properties defined yet:
        block_range.map(|r| {
            // Right after the leading `{`...
            let r = lsp_types::Range::new(
                lsp_types::Position::new(r.start.line, r.start.character.saturating_add(1)),
                lsp_types::Position::new(r.start.line, r.start.character.saturating_add(1)),
            );
            (r, InsertPosition::After)
        })
    })
}

fn create_workspace_edit_for_set_binding_on_known_property(
    uri: &lsp_types::Url,
    version: i32,
    element: &ElementRc,
    offset_mapper: &OffsetToPositionMapper,
    properties: &[PropertyInformation],
    property_name: &str,
    new_expression: &str,
) -> Option<lsp_types::WorkspaceEdit> {
    let block_range = find_block_range(element).map(|r| offset_mapper.map_range(r));

    find_insert_range_for_property(&block_range, properties, property_name).and_then(
        |(range, insert_type)| {
            let indent = find_element_indent(element).unwrap_or_default();
            let edit = lsp_types::TextEdit {
                range,
                new_text: match insert_type {
                    InsertPosition::Before => {
                        format!("{property_name}: {new_expression};\n{indent}    ")
                    }
                    InsertPosition::After => {
                        format!("\n{indent}    {property_name}: {new_expression};")
                    }
                },
            };
            let edits = vec![lsp_types::OneOf::Left(edit)];
            let text_document_edits = vec![lsp_types::TextDocumentEdit {
                text_document: lsp_types::OptionalVersionedTextDocumentIdentifier::new(
                    uri.clone(),
                    version,
                ),
                edits,
            }];
            Some(lsp_types::WorkspaceEdit {
                document_changes: Some(lsp_types::DocumentChanges::Edits(text_document_edits)),
                ..Default::default()
            })
        },
    )
}

fn set_binding_on_known_property(
    document_cache: &mut DocumentCache,
    uri: &lsp_types::Url,
    element: &ElementRc,
    properties: &[PropertyInformation],
    property_name: &str,
    new_expression: &str,
    diag: &mut BuildDiagnostics,
) -> Result<(SetBindingResponse, Option<lsp_types::WorkspaceEdit>), Error> {
    let workspace_edit = (!diag.has_error())
        .then(|| {
            create_workspace_edit_for_set_binding_on_known_property(
                uri,
                document_cache.document_version(uri)?,
                element,
                &document_cache
                    .offset_to_position_mapper(uri)
                    .expect("This URI is known at this point!"),
                properties,
                property_name,
                new_expression,
            )
        })
        .flatten();

    Ok((
        SetBindingResponse {
            diagnostics: diag.iter().map(crate::util::to_lsp_diag).collect::<Vec<_>>(),
        },
        workspace_edit,
    ))
}

// Find the indentation of the element itself as well as the indentation of properties inside the
// element. Returns the element indent followed by the block indent
fn find_element_indent(element: &ElementRc) -> Option<String> {
    element
        .borrow()
        .node
        .as_ref()
        .and_then(|n| n.first_token())
        .and_then(|t| t.prev_token())
        .and_then(|t| {
            if t.kind() != SyntaxKind::Whitespace {
                None
            } else {
                t.text().split('\n').last().map(|s| s.to_owned())
            }
        })
}

pub(crate) fn set_binding(
    document_cache: &mut DocumentCache,
    uri: &lsp_types::Url,
    element: &ElementRc,
    property_name: &str,
    new_expression: String,
) -> Result<(SetBindingResponse, Option<lsp_types::WorkspaceEdit>), Error> {
    let (mut diag, expression_node) = {
        let mut diagnostics = BuildDiagnostics::default();

        let syntax_node = i_slint_compiler::parser::parse_expression_as_bindingexpression(
            &new_expression,
            &mut diagnostics,
        );

        (diagnostics, syntax_node)
    };

    let new_expression_type = {
        let element = element.borrow();
        if let Some(node) = element.node.as_ref() {
            crate::util::with_property_lookup_ctx(document_cache, node, property_name, |ctx| {
                let expression =
                    i_slint_compiler::expression_tree::Expression::from_binding_expression_node(
                        expression_node,
                        ctx,
                    );
                expression.ty()
            })
            .unwrap_or(Type::Invalid)
        } else {
            Type::Invalid
        }
    };

    let offset_mapper = document_cache.offset_to_position_mapper(uri)?;
    let properties = get_properties(element, &offset_mapper);
    let property = match get_property_information(&properties, property_name) {
        Ok(p) => p,
        Err(e) => {
            diag.push_error_with_span(
                e.to_string(),
                i_slint_compiler::diagnostics::SourceLocation {
                    source_file: None,
                    span: i_slint_compiler::diagnostics::Span::new(0),
                },
            );
            return Ok((
                SetBindingResponse {
                    diagnostics: diag.iter().map(crate::util::to_lsp_diag).collect::<Vec<_>>(),
                },
                None,
            ));
        }
    };

    validate_property_expression_type(&property, new_expression_type, &mut diag);
    if property.defined_at.is_some() {
        // Change an already defined property:
        set_binding_on_existing_property(document_cache, uri, &property, new_expression, &mut diag)
    } else {
        // Add a new definition to a known property:
        set_binding_on_known_property(
            document_cache,
            uri,
            element,
            &properties,
            &property.name,
            &new_expression,
            &mut diag,
        )
    }
}

fn create_workspace_edit_for_remove_binding(
    uri: &lsp_types::Url,
    version: i32,
    range: lsp_types::Range,
) -> lsp_types::WorkspaceEdit {
    let edit = lsp_types::TextEdit { range, new_text: String::new() };
    let edits = vec![lsp_types::OneOf::Left(edit)];
    let text_document_edits = vec![lsp_types::TextDocumentEdit {
        text_document: lsp_types::OptionalVersionedTextDocumentIdentifier::new(
            uri.clone(),
            version,
        ),
        edits,
    }];
    lsp_types::WorkspaceEdit {
        document_changes: Some(lsp_types::DocumentChanges::Edits(text_document_edits)),
        ..Default::default()
    }
}

pub(crate) fn remove_binding(
    document_cache: &mut DocumentCache,
    uri: &lsp_types::Url,
    element: &ElementRc,
    property_name: &str,
) -> Result<lsp_types::WorkspaceEdit, Error> {
    let offset_mapper = document_cache.offset_to_position_mapper(uri)?;
    let element = element.borrow();

    let range = find_property_binding_offset(&element, property_name)
        .and_then(|offset| element.node.as_ref()?.token_at_offset(offset.into()).right_biased())
        .and_then(|token| {
            for ancestor in token.parent_ancestors() {
                if (ancestor.kind() == SyntaxKind::Binding)
                    || (ancestor.kind() == SyntaxKind::PropertyDeclaration)
                {
                    let start = {
                        let token = left_extend(ancestor.first_token()?);
                        let start = token.text_range().start();
                        token
                            .prev_token()
                            .and_then(|t| {
                                if t.kind() == SyntaxKind::Whitespace && t.text().contains('\n') {
                                    let to_sub =
                                        t.text().split('\n').last().unwrap_or_default().len()
                                            as u32;
                                    start.checked_sub(to_sub.into())
                                } else {
                                    None
                                }
                            })
                            .unwrap_or(start)
                    };
                    let end = {
                        let token = right_extend(ancestor.last_token()?);
                        let end = token.text_range().end();
                        token
                            .next_token()
                            .and_then(|t| {
                                if t.kind() == SyntaxKind::Whitespace && t.text().contains('\n') {
                                    let to_add =
                                        t.text().split('\n').next().unwrap_or_default().len()
                                            as u32;
                                    end.checked_add((to_add + 1/* <cr> */).into())
                                } else {
                                    None
                                }
                            })
                            .unwrap_or(end)
                    };

                    return Some(offset_mapper.map_range(rowan::TextRange::new(start, end)));
                }
                if ancestor.kind() == SyntaxKind::Element {
                    // There should have been a binding before the element!
                    break;
                }
            }
            None
        })
        .ok_or_else(|| Into::<Error>::into("Could not find range to delete."))?;

    Ok(create_workspace_edit_for_remove_binding(
        uri,
        document_cache
            .document_version(uri)
            .ok_or_else(|| Into::<Error>::into("Document not found in cache"))?,
        range,
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    use crate::server_loop;

    use crate::test::{complex_document_cache, loaded_document_cache};

    fn find_property<'a>(
        properties: &'a [PropertyInformation],
        name: &'_ str,
    ) -> Option<&'a PropertyInformation> {
        properties.iter().find(|p| p.name == name)
    }

    fn properties_at_position_in_cache(
        line: u32,
        character: u32,
        dc: &mut DocumentCache,
        url: &lsp_types::Url,
    ) -> Option<(ElementRc, Vec<PropertyInformation>)> {
        let element =
            server_loop::element_at_position(dc, &url, &lsp_types::Position { line, character })?;
        Some((element.clone(), get_properties(&element, &dc.offset_to_position_mapper(url).ok()?)))
    }

    fn properties_at_position(
        line: u32,
        character: u32,
    ) -> Option<(ElementRc, Vec<PropertyInformation>, DocumentCache, lsp_types::Url)> {
        let (mut dc, url, _) = complex_document_cache("fluent");
        if let Some((e, p)) = properties_at_position_in_cache(line, character, &mut dc, &url) {
            Some((e, p, dc, url))
        } else {
            None
        }
    }

    #[test]
    fn test_get_properties() {
        let (_, result, _, _) = properties_at_position(6, 4).unwrap();

        // Property of element:
        assert_eq!(&find_property(&result, "elapsed-time").unwrap().type_name, "duration");
        // Property of base type:
        assert_eq!(&find_property(&result, "no-frame").unwrap().type_name, "bool");
        // reserved properties:
        assert_eq!(
            &find_property(&result, "accessible-role").unwrap().type_name,
            "enum AccessibleRole"
        );

        // Poke deeper:
        let (_, result, _, _) = properties_at_position(21, 30).unwrap();
        let property = find_property(&result, "background").unwrap();

        let def_at = property.defined_at.as_ref().unwrap();
        assert_eq!(def_at.expression_range.end.line, def_at.expression_range.start.line);
        // -1 because the lsp range end location is exclusive.
        assert_eq!(
            (def_at.expression_range.end.character - def_at.expression_range.start.character)
                as usize,
            "lightblue".len()
        );
    }

    #[test]
    fn test_element_information() {
        let (mut dc, url, _) = complex_document_cache("fluent");
        let element =
            server_loop::element_at_position(&mut dc, &url, &lsp_types::Position::new(33, 4))
                .unwrap();

        let result =
            get_element_information(&element, &dc.offset_to_position_mapper(&url).unwrap());

        let r = result.range.unwrap();
        assert_eq!(r.start.line, 32);
        assert_eq!(r.start.character, 12);
        assert_eq!(r.end.line, 35);
        assert_eq!(r.end.character, 13);
    }

    fn delete_range_test(
        content: String,
        pos_l: u32,
        pos_c: u32,
        sl: u32,
        sc: u32,
        el: u32,
        ec: u32,
    ) {
        for (i, l) in content.split('\n').enumerate() {
            println!("{i:2}: {l}");
        }
        println!("-------------------------------------------------------------------");
        println!("   :           1         2         3         4         5");
        println!("   : 012345678901234567890123456789012345678901234567890123456789");

        let (mut dc, url, _) = loaded_document_cache("fluent", content);

        let (_, result) = properties_at_position_in_cache(pos_l, pos_c, &mut dc, &url).unwrap();

        let p = find_property(&result, "text").unwrap();
        let definition = p.defined_at.as_ref().unwrap();

        assert_eq!(&definition.expression_value, "\"text\"");

        println!("Actual: (l: {}, c: {}) - (l: {}, c: {}) --- Expected: (l: {sl}, c: {sc}) - (l: {el}, c: {ec})",
            definition.selection_range.start.line,
            definition.selection_range.start.character,
            definition.selection_range.end.line,
            definition.selection_range.end.character,
        );

        assert_eq!(definition.selection_range.start.line, sl);
        assert_eq!(definition.selection_range.start.character, sc);
        assert_eq!(definition.selection_range.end.line, el);
        assert_eq!(definition.selection_range.end.character, ec);
    }

    #[test]
    fn test_get_property_delete_range_no_extend() {
        delete_range_test(
            r#"import { VerticalBox } from "std-widgets.slint";

component MainWindow inherits Window {
    VerticalBox {
        Text { text: "text"; }
    }
}
            "#
            .to_string(),
            4,
            12,
            4,
            15,
            4,
            28,
        );
    }

    #[test]
    fn test_get_property_delete_range_line_extend_left_extra_indent() {
        delete_range_test(
            r#"import { VerticalBox } from "std-widgets.slint";

component MainWindow inherits Window {
    VerticalBox {
        Text {
              // Cut
            text: "text";
        }
    }
}
            "#
            .to_string(),
            4,
            12,
            5,
            14,
            6,
            25,
        );
    }

    #[test]
    fn test_get_property_delete_range_line_extend_left_no_ws() {
        delete_range_test(
            r#"import { VerticalBox } from "std-widgets.slint";

component MainWindow inherits Window {
    VerticalBox {
        Text {
            /* Cut */text: "text";
        }
    }
}
            "#
            .to_string(),
            4,
            12,
            5,
            12,
            5,
            34,
        );
    }

    #[test]
    fn test_get_property_delete_range_extend_left_to_empty_line() {
        delete_range_test(
            r#"import { VerticalBox } from "std-widgets.slint";

component MainWindow inherits Window {
    VerticalBox {
        Text {
            font-size: 12px;
            // Keep

            // Cut
            text: "text";
        }
    }
}
            "#
            .to_string(),
            4,
            12,
            8,
            12,
            9,
            25,
        );
    }

    #[test]
    fn test_get_property_delete_range_extend_left_many_lines() {
        delete_range_test(
            r#"import { VerticalBox } from "std-widgets.slint";

component MainWindow inherits Window {
    VerticalBox {
        Text {
            font-size: 12px;
             // Keep

            // Cut
              // Cut
            // Cut
                  // Cut
            // Cut
            // Cut
            // Cut
            // Cut
            // Cut
            // Cut
            // Cut
            text: "text";
        }
    }
}
            "#
            .to_string(),
            4,
            12,
            8,
            12,
            19,
            25,
        );
    }

    #[test]
    fn test_get_property_delete_range_extend_left_multiline_comment() {
        delete_range_test(
            r#"import { VerticalBox } from "std-widgets.slint";

component MainWindow inherits Window {
    VerticalBox {
        Text {
            font-size: 12px;
          // Keep

            /* Cut
       Cut
            /* Cut
              ---  Cut */

            // Cut
            // Cut */
            text: "text";
        }
    }
}
            "#
            .to_string(),
            4,
            12,
            8,
            12,
            15,
            25,
        );
    }

    #[test]
    fn test_get_property_delete_range_extend_left_un_indented_property() {
        delete_range_test(
            r#"import { VerticalBox } from "std-widgets.slint";

component MainWindow inherits Window {
    VerticalBox {
        Text {
            font-size: 12px;

        /* Cut
       Cut

            /* Cut
              ---  Cut */
  Cut */
                // Cut
            // Cut
text: "text";
        }
    }
}
            "#
            .to_string(),
            4,
            12,
            7,
            8,
            15,
            13,
        );
    }

    #[test]
    fn test_get_property_delete_range_extend_left_leading_line_comment() {
        delete_range_test(
            r#"import { VerticalBox } from "std-widgets.slint";

component MainWindow inherits Window {
    VerticalBox {
        Text {
            font-size: 12px;
          // Cut
            /* Cut
       Cut

            /* Cut
              ---  Cut */
  Cut */
                // Cut
            // Cut
            /* cut */ text: "text";
        }
    }
}
            "#
            .to_string(),
            4,
            12,
            6,
            10,
            15,
            35,
        );
    }

    #[test]
    fn test_get_property_delete_range_right_extend() {
        delete_range_test(
            r#"import { VerticalBox } from "std-widgets.slint";

component MainWindow inherits Window {
    VerticalBox {
        Text {
            text: "text"; // Cut
                // Keep
        }
    }
}
            "#
            .to_string(),
            4,
            12,
            5,
            12,
            5,
            32,
        );
    }

    #[test]
    fn test_get_property_delete_range_right_extend_to_line_break() {
        delete_range_test(
            r#"import { VerticalBox } from "std-widgets.slint";

component MainWindow inherits Window {
    VerticalBox {
        Text {
            text: "text"; /* Cut
                // Cut
                   Cut
                 *   Cut */

            // Keep
            font-size: 12px;
        }
    }
}
            "#
            .to_string(),
            4,
            12,
            5,
            12,
            8,
            27,
        );
    }

    #[test]
    fn test_get_property_delete_range_no_right_extend() {
        delete_range_test(
            r#"import { VerticalBox } from "std-widgets.slint";

component MainWindow {
    VerticalBox {
        Text {
            text: "text";/*Keep*/ font_size: 12px;
        }
    }
}
            "#
            .to_string(),
            4,
            12,
            5,
            12,
            5,
            25,
        );
    }

    #[test]
    fn test_get_property_delete_range_no_right_extend_with_ws() {
        delete_range_test(
            r#"import { VerticalBox } from "std-widgets.slint";

component MainWindow {
    VerticalBox {
        Text {
            text: "text";  /*Keep*/ font_size: 12px;
        }
    }
}
            "#
            .to_string(),
            4,
            12,
            5,
            12,
            5,
            25,
        );
    }

    #[test]
    fn test_get_property_delete_range_right_extend_to_rbrace() {
        delete_range_test(
            r#"import { VerticalBox } from "std-widgets.slint";

component MainWindow {
    VerticalBox {
        Text { text: "text";/* Cut */}
        }
    }
}
            "#
            .to_string(),
            4,
            12,
            4,
            15,
            4,
            37,
        );
    }

    #[test]
    fn test_get_property_delete_range_right_extend_to_rbrace_ws() {
        delete_range_test(
            r#"import { VerticalBox } from "std-widgets.slint";

component MainWindow inherits Window {
    VerticalBox {
        Text { text: "text";   /* Cut */    /* Cut */ }
        }
    }
}
            "#
            .to_string(),
            4,
            12,
            4,
            15,
            4,
            53,
        );
    }

    #[test]
    fn test_get_property_definition() {
        let (mut dc, url, _) = loaded_document_cache("fluent",
            r#"import { LineEdit, Button, Slider, HorizontalBox, VerticalBox } from "std-widgets.slint";

component Base1 {
    in-out property<int> foo = 42;
}

component Base2 inherits Base1 {
    foo: 23;
}

component MainWindow inherits Window {
    property <duration> total-time: slider.value * 1s;
    property <duration> elapsed-time;

    callback tick(duration);
    tick(passed-time) => {
        elapsed-time += passed-time;
        elapsed-time = min(elapsed-time, total-time);
    }

    VerticalBox {
        HorizontalBox {
            padding-left: 0;
            Text { text: "Elapsed Time:"; }
            Base2 {
                foo: 15;
                min-width: 200px;
                max-height: 30px;
                background: gray;
                Rectangle {
                    height: 100%;
                    width: parent.width * (elapsed-time/total-time);
                    background: lightblue;
                }
            }
        }
        Text{
            text: (total-time / 1s) + "s";
        }
        HorizontalBox {
            padding-left: 0;
            Text {
                text: "Duration:";
                vertical-alignment: center;
            }
            slider := Slider {
                maximum: 30s / 1s;
                value: 10s / 1s;
                changed(new-duration) => {
                    root.total-time = new-duration * 1s;
                    root.elapsed-time = min(root.elapsed-time, root.total-time);
                }
            }
        }
        Button {
            text: "Reset";
            clicked => {
                elapsed-time = 0
            }
        }
    }
}
            "#.to_string());
        let file_url = url.clone();
        let (_, result) = properties_at_position_in_cache(28, 15, &mut dc, &url).unwrap();

        let foo_property = find_property(&result, "foo").unwrap();

        assert_eq!(foo_property.type_name, "int");

        let declaration = foo_property.declared_at.as_ref().unwrap();
        assert_eq!(declaration.uri, file_url);
        assert_eq!(declaration.start_position.line, 3);
        assert_eq!(declaration.start_position.character, 20); // This should probably point to the start of
                                                              // `property<int> foo = 42`, not to the `<`
        assert_eq!(foo_property.group, "Base1");
    }

    #[test]
    fn test_invalid_properties() {
        let (mut dc, url, _) = loaded_document_cache(
            "fluent",
            r#"
global SomeGlobal := {
    property <int> glob: 77;
}

component SomeRect inherits Rectangle {
    component foo inherits InvalidType {
        property <int> abcd: 41;
        width: 45px;
    }
}
            "#
            .to_string(),
        );

        let (_, result) = properties_at_position_in_cache(1, 25, &mut dc, &url).unwrap();

        let glob_property = find_property(&result, "glob").unwrap();
        assert_eq!(glob_property.type_name, "int");
        let declaration = glob_property.declared_at.as_ref().unwrap();
        assert_eq!(declaration.uri, url);
        assert_eq!(declaration.start_position.line, 2);
        assert_eq!(glob_property.group, "");
        assert_eq!(find_property(&result, "width"), None);

        let (_, result) = properties_at_position_in_cache(8, 4, &mut dc, &url).unwrap();
        let abcd_property = find_property(&result, "abcd").unwrap();
        assert_eq!(abcd_property.type_name, "int");
        let declaration = abcd_property.declared_at.as_ref().unwrap();
        assert_eq!(declaration.uri, url);
        assert_eq!(declaration.start_position.line, 7);
        assert_eq!(abcd_property.group, "");

        let x_property = find_property(&result, "x").unwrap();
        assert_eq!(x_property.type_name, "length");
        assert_eq!(x_property.defined_at, None);
        assert_eq!(x_property.group, "geometry");

        let width_property = find_property(&result, "width").unwrap();
        assert_eq!(width_property.type_name, "length");
        let definition = width_property.defined_at.as_ref().unwrap();
        assert_eq!(definition.expression_range.start.line, 8);
        assert_eq!(width_property.group, "geometry");
    }

    #[test]
    fn test_invalid_property_panic() {
        let (mut dc, url, _) = loaded_document_cache(
            "fluent",
            r#"export component Demo { Text { text: } }"#.to_string(),
        );

        let (_, result) = properties_at_position_in_cache(0, 35, &mut dc, &url).unwrap();

        let prop = find_property(&result, "text").unwrap();
        assert_eq!(prop.defined_at, None); // The property has no valid definition at this time
    }

    #[test]
    fn test_codeblock_property_declaration() {
        let (mut dc, url, _) = loaded_document_cache(
            "fluent",
            r#"
component Base {
    property <int> a1: { 1 + 1 }
    property <int> a2: { 1 + 2; }
    property <int> a3: { 1 + 3 };
    property <int> a4: { 1 + 4; };
    in property <int> b: {
        if (something) { return 42; }
        return 1 + 2;
    }
}
            "#
            .to_string(),
        );

        let (_, result) = properties_at_position_in_cache(3, 0, &mut dc, &url).unwrap();
        assert_eq!(find_property(&result, "a1").unwrap().type_name, "int");
        assert_eq!(
            find_property(&result, "a1").unwrap().defined_at.as_ref().unwrap().expression_value,
            "{ 1 + 1 }"
        );
        assert_eq!(find_property(&result, "a2").unwrap().type_name, "int");
        assert_eq!(
            find_property(&result, "a2").unwrap().defined_at.as_ref().unwrap().expression_value,
            "{ 1 + 2; }"
        );
        assert_eq!(find_property(&result, "a3").unwrap().type_name, "int");
        assert_eq!(
            find_property(&result, "a3").unwrap().defined_at.as_ref().unwrap().expression_value,
            "{ 1 + 3 }"
        );
        assert_eq!(find_property(&result, "a4").unwrap().type_name, "int");
        assert_eq!(
            find_property(&result, "a4").unwrap().defined_at.as_ref().unwrap().expression_value,
            "{ 1 + 4; }"
        );
        assert_eq!(find_property(&result, "b").unwrap().type_name, "int");
        assert_eq!(
            find_property(&result, "b").unwrap().defined_at.as_ref().unwrap().expression_value,
            "{\n        if (something) { return 42; }\n        return 1 + 2;\n    }"
        );
    }

    #[test]
    fn test_codeblock_property_definitions() {
        let (mut dc, url, _) = loaded_document_cache(
            "fluent",
            r#"
component Base {
    in property <int> a1;
    in property <int> a2;
    in property <int> a3;
    in property <int> a4;
    in property <int> b;
}
component MyComp {
    Base {
        a1: { 1 + 1 }
        a2: { 1 + 2; }
        a3: { 1 + 3 };
        a4: { 1 + 4; };
        b: {
            if (something) { return 42; }
            return 1 + 2;
        }
    }
}
            "#
            .to_string(),
        );

        let (_, result) = properties_at_position_in_cache(11, 0, &mut dc, &url).unwrap();
        assert_eq!(find_property(&result, "a1").unwrap().type_name, "int");
        assert_eq!(
            find_property(&result, "a1").unwrap().defined_at.as_ref().unwrap().expression_value,
            "{ 1 + 1 }"
        );
        assert_eq!(find_property(&result, "a2").unwrap().type_name, "int");
        assert_eq!(
            find_property(&result, "a2").unwrap().defined_at.as_ref().unwrap().expression_value,
            "{ 1 + 2; }"
        );
        assert_eq!(find_property(&result, "a3").unwrap().type_name, "int");
        assert_eq!(
            find_property(&result, "a3").unwrap().defined_at.as_ref().unwrap().expression_value,
            "{ 1 + 3 }"
        );
        assert_eq!(find_property(&result, "a4").unwrap().type_name, "int");
        assert_eq!(
            find_property(&result, "a4").unwrap().defined_at.as_ref().unwrap().expression_value,
            "{ 1 + 4; }"
        );
        assert_eq!(find_property(&result, "b").unwrap().type_name, "int");
        assert_eq!(
            find_property(&result, "b").unwrap().defined_at.as_ref().unwrap().expression_value,
            "{\n            if (something) { return 42; }\n            return 1 + 2;\n        }",
        );
    }

    #[test]
    fn test_output_properties() {
        let (mut dc, url, _) = loaded_document_cache(
            "fluent",
            r#"
component Base {
    property <int> a: 1;
    in property <int> b: 2;
    out property <int> c: 3;
    in-out property <int> d: 4;
}

component MyComp {
    Base {

    }
    TouchArea {

    }
}
            "#
            .to_string(),
        );

        let (_, result) = properties_at_position_in_cache(3, 0, &mut dc, &url).unwrap();
        assert_eq!(find_property(&result, "a").unwrap().type_name, "int");
        assert_eq!(find_property(&result, "b").unwrap().type_name, "int");
        assert_eq!(find_property(&result, "c").unwrap().type_name, "int");
        assert_eq!(find_property(&result, "d").unwrap().type_name, "int");

        let (_, result) = properties_at_position_in_cache(10, 0, &mut dc, &url).unwrap();
        assert_eq!(find_property(&result, "a"), None);
        assert_eq!(find_property(&result, "b").unwrap().type_name, "int");
        assert_eq!(find_property(&result, "c"), None);
        assert_eq!(find_property(&result, "d").unwrap().type_name, "int");

        let (_, result) = properties_at_position_in_cache(13, 0, &mut dc, &url).unwrap();
        assert_eq!(find_property(&result, "enabled").unwrap().type_name, "bool");
        assert_eq!(find_property(&result, "pressed"), None);
    }

    fn set_binding_helper(
        property_name: &str,
        new_value: &str,
    ) -> (SetBindingResponse, Option<lsp_types::WorkspaceEdit>) {
        let (element, _, mut dc, url) = properties_at_position(18, 15).unwrap();
        set_binding(&mut dc, &url, &element, property_name, new_value.to_string()).unwrap()
    }

    #[test]
    fn test_set_binding_valid_expression_unknown_property() {
        let (result, edit) = set_binding_helper("foobar", "1 + 2");

        assert_eq!(edit, None);
        assert_eq!(result.diagnostics.len(), 1_usize);
        assert_eq!(result.diagnostics[0].severity, Some(lsp_types::DiagnosticSeverity::ERROR));
        assert!(result.diagnostics[0].message.contains("no property"));
    }

    #[test]
    fn test_set_binding_valid_expression_undefined_property() {
        let (result, edit) = set_binding_helper("x", "30px");

        let edit = edit.unwrap();
        let dcs = if let Some(lsp_types::DocumentChanges::Edits(e)) = &edit.document_changes {
            e
        } else {
            unreachable!();
        };
        assert_eq!(dcs.len(), 1_usize);

        let tcs = &dcs[0].edits;
        assert_eq!(tcs.len(), 1_usize);

        let tc = if let lsp_types::OneOf::Left(tc) = &tcs[0] {
            tc
        } else {
            unreachable!();
        };
        assert_eq!(&tc.new_text, "x: 30px;\n                ");
        assert_eq!(tc.range.start, lsp_types::Position { line: 17, character: 16 });
        assert_eq!(tc.range.end, lsp_types::Position { line: 17, character: 16 });

        assert_eq!(result.diagnostics.len(), 0_usize);
    }

    #[test]
    fn test_set_binding_valid_expression_wrong_return_type() {
        let (result, edit) = set_binding_helper("min-width", "\"test\"");

        assert_eq!(edit, None);
        assert_eq!(result.diagnostics.len(), 1_usize);
        assert_eq!(result.diagnostics[0].severity, Some(lsp_types::DiagnosticSeverity::ERROR));
        assert!(result.diagnostics[0].message.contains("return type mismatch"));
    }

    #[test]
    fn test_set_binding_invalid_expression() {
        let (result, edit) = set_binding_helper("min-width", "?=///1 + 2");

        assert_eq!(edit, None);
        assert_eq!(result.diagnostics.len(), 1_usize);
        assert_eq!(result.diagnostics[0].severity, Some(lsp_types::DiagnosticSeverity::ERROR));
        assert!(result.diagnostics[0].message.contains("invalid expression"));
    }

    #[test]
    fn test_set_binding_trailing_garbage() {
        let (result, edit) = set_binding_helper("min-width", "1px;");

        assert_eq!(edit, None);
        assert_eq!(result.diagnostics.len(), 1_usize);
        assert_eq!(result.diagnostics[0].severity, Some(lsp_types::DiagnosticSeverity::ERROR));
        assert!(result.diagnostics[0].message.contains("end of string"));
    }

    #[test]
    fn test_set_binding_valid() {
        let (result, edit) = set_binding_helper("min-width", "5px");

        let edit = edit.unwrap();
        let dcs = if let Some(lsp_types::DocumentChanges::Edits(e)) = &edit.document_changes {
            e
        } else {
            unreachable!();
        };
        assert_eq!(dcs.len(), 1_usize);

        let tcs = &dcs[0].edits;
        assert_eq!(tcs.len(), 1_usize);

        let tc = if let lsp_types::OneOf::Left(tc) = &tcs[0] {
            tc
        } else {
            unreachable!();
        };
        assert_eq!(&tc.new_text, "5px");
        assert_eq!(tc.range.start, lsp_types::Position { line: 17, character: 27 });
        assert_eq!(tc.range.end, lsp_types::Position { line: 17, character: 32 });

        assert_eq!(result.diagnostics.len(), 0_usize);
    }
}
