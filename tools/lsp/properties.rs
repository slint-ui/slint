// Copyright © SixtyFPS GmbH <info@slint-ui.com>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-commercial

use crate::server_loop::{DocumentCache, OffsetToPositionMapper};
use crate::Error;

use i_slint_compiler::diagnostics::{BuildDiagnostics, Spanned};
use i_slint_compiler::langtype::{ElementType, Type};
use i_slint_compiler::object_tree::{Element, ElementRc, PropertyVisibility};
use i_slint_compiler::parser::{syntax_nodes, Language, SyntaxKind};
use lsp_types::WorkspaceEdit;

use std::collections::HashSet;

#[cfg(target_arch = "wasm32")]
use crate::wasm_prelude::*;

#[derive(serde::Deserialize, serde::Serialize, Clone, Debug, PartialEq)]
pub(crate) struct DefinitionInformation {
    property_definition_range: lsp_types::Range,
    expression_range: lsp_types::Range,
    delete_range: lsp_types::Range,
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

fn add_element_properties(
    element: &Element,
    offset_to_position: &mut OffsetToPositionMapper,
    group: &str,
    is_local_element: bool,
    result: &mut Vec<PropertyInformation>,
) {
    let file = source_file(element);

    result.extend(element.property_declarations.iter().filter_map(move |(name, value)| {
        if !value.property_type.is_property_type() {
            // Filter away the callbacks
            return None;
        }
        if matches!(value.visibility, PropertyVisibility::Output | PropertyVisibility::Private)
            && !is_local_element
        {
            // Skip properties that cannot be set because of visibility rules
            return None;
        }
        let type_node = value.type_node()?; // skip fake and materialized properties
        let declared_at = file.as_ref().map(|file| {
            let start_position = offset_to_position.map(type_node.text_range().start().into());
            let uri = lsp_types::Url::from_file_path(file).unwrap_or_else(|_| {
                lsp_types::Url::parse("file:///)").expect("That should have been valid as URL!")
            });

            DeclarationInformation { uri, start_position }
        });
        Some(PropertyInformation {
            name: name.clone(),
            type_name: format!("{}", value.property_type),
            declared_at,
            defined_at: None,
            group: group.to_string(),
        })
    }))
}

/// Move left from the start of a `token` to include white-space and comments that go with it.
fn left_extend(token: rowan::SyntaxToken<Language>) -> rowan::TextSize {
    let expression_position = token.text_range().start();
    let start_token = {
        let mut current_token = token.prev_token();
        let mut start_token = token.clone();

        // Walk backwards:
        while let Some(t) = current_token {
            if t.kind() != SyntaxKind::Whitespace && t.kind() != SyntaxKind::Comment {
                break;
            }
            start_token = t.clone();
            current_token = t.prev_token();
        }
        start_token
    };

    let start_position = start_token.text_range().start();
    let len = expression_position
        .checked_sub(start_position)
        .expect("start_position >= start of start_token");
    if len == 0.into() {
        return start_position;
    }

    // Collect white-space/comments into a string:
    let possible_preamble = {
        let mut tmp = String::with_capacity(len.into());
        let mut current_token = start_token.clone();
        while current_token.text_range().start() != expression_position {
            if current_token.kind() == SyntaxKind::Whitespace {
                tmp.push_str(current_token.text());
            } else {
                // Handle multiline comments by replacing forcing to single line:-)
                tmp.push_str(&current_token.text().replace('\n', " "));
            }
            current_token = current_token
                .next_token()
                .expect("We move between the start_token and the expression token");
        }
        tmp
    };
    let lines: Vec<&str> = possible_preamble.split('\n').rev().collect();
    if lines.len() == 0 || (lines.len() == 1 && lines[0].trim().is_empty()) {
        // just a bit of WS between expressions. Leave that alone:
        return expression_position;
    }

    let mut result_position = expression_position;

    let indent = {
        let last_line = lines.first().expect("len was != 0");
        let last_line_len = last_line.len();
        let trimmed_line_len = last_line.trim_start().len();
        if trimmed_line_len == 0 {
            last_line.to_string()
        } else {
            result_position = result_position
                .checked_sub(trimmed_line_len.try_into().expect("This is > 0"))
                .expect("This is safe");
            last_line[0..last_line_len - trimmed_line_len].to_string()
        }
    };

    for l in lines.into_iter().skip(1) {
        let trimmed = l.trim();
        if trimmed.is_empty() {
            // Empty lines separate comment sections from each other:
            return result_position;
        } else if l.starts_with(&indent) {
            // We have a comment, that is at least as widely indented as we are:
            // This line is a comment about us:
            // Move indent to the front, then one more for the `\n` and then `l.len() -
            // indent.len()`, which turns into:
            result_position = result_position
                .checked_sub((l.len() + 1).try_into().expect("This is fine!"))
                .expect("This, too");
        } else {
            // We had a comment less indented than ourselves, consider that unrelated:
            return result_position;
        }
    }
    result_position
}

/// Move right from the end of the `token` to include white-space and comments that go with it.
fn right_extend(token: rowan::SyntaxToken<Language>) -> rowan::TextSize {
    let expression_position = token.text_range().end();
    let (end_token, be_greedy) = {
        let mut current_token = token.next_token();
        let mut end_token = token.clone();
        let mut be_greedy = false;

        // Walk forward:
        while let Some(t) = current_token {
            if t.kind() == SyntaxKind::RBrace {
                be_greedy = true;
            }
            if t.kind() != SyntaxKind::Whitespace && t.kind() != SyntaxKind::Comment {
                break;
            }
            end_token = t.clone();
            current_token = t.next_token();
        }
        (end_token, be_greedy)
    };

    let end_position = end_token.text_range().end();
    let len = end_position.checked_sub(expression_position).expect("end_position >= end of token");
    if len == 0.into() {
        return end_position;
    }

    // Collect white-space/comments into a string:
    let possible_epilog = {
        let mut tmp = String::with_capacity(len.into());
        let mut current_token = token.next_token();
        while let Some(t) = current_token {
            if t.kind() == SyntaxKind::Whitespace {
                tmp.push_str(t.text());
            } else {
                // Handle multi-line comments by replacing forcing to single line:-)
                tmp.push_str(&t.text().replace('\n', " "));
            }
            if t.text_range().end() == end_position {
                break;
            }
            current_token = t.next_token();
        }
        tmp
    };
    let lines: Vec<&str> = possible_epilog.split('\n').collect();
    if lines.len() <= 1 {
        // Lines is either
        // 1. empty (nothing to do)
        // 2. Just WS (eat if greedy!)
        // 3. A comment (eat if greedy!)
        if be_greedy && !lines.is_empty() {
            return expression_position
                .checked_add(lines[0].len().try_into().expect("safe!"))
                .expect("Safe!");
        } else {
            return expression_position;
        }
    }

    let mut result_position = expression_position;
    for l in lines.into_iter() {
        let trimmed = l.trim();
        if trimmed.is_empty() {
            // Empty lines separate comment sections from each other:
            break;
        } else {
            result_position = result_position
                .checked_add((l.len() + 1).try_into().expect("This is fine!"))
                .expect("This, too");
        }
    }
    result_position
}

fn find_expression_range(
    element: &syntax_nodes::Element,
    offset: u32,
    offset_to_position: &mut OffsetToPositionMapper,
) -> Option<DefinitionInformation> {
    let mut property_definition_range = rowan::TextRange::default();
    let mut expression_range = rowan::TextRange::default();
    let mut delete_range = rowan::TextRange::default();
    let mut expression_value = None;

    if let Some(token) = element.token_at_offset(offset.into()).right_biased() {
        for ancestor in token.parent_ancestors() {
            if ancestor.kind() == SyntaxKind::BindingExpression {
                // The BindingExpression contains leading and trailing whitespace + `;`
                let expr =
                    &ancestor.first_child().expect("A BindingExpression needs to have a child");
                expression_range = expr.text_range();
                expression_value = Some(expr.text().to_string());
                continue;
            }
            if (ancestor.kind() == SyntaxKind::Binding)
                || (ancestor.kind() == SyntaxKind::PropertyDeclaration)
            {
                property_definition_range = ancestor.text_range();
                delete_range = rowan::TextRange::new(
                    left_extend(
                        ancestor.first_token().expect("A real node consists of at least one token"),
                    ),
                    right_extend(
                        ancestor.last_token().expect("A real node consists of at least one token"),
                    ),
                );
                break;
            }
            if ancestor.kind() == SyntaxKind::Element {
                // There should have been a binding before the element!
                break;
            }
        }
    }
    if let Some(expression_value) = expression_value {
        Some(DefinitionInformation {
            property_definition_range: offset_to_position.map_range(property_definition_range)?,
            delete_range: offset_to_position.map_range(delete_range)?,
            expression_range: offset_to_position.map_range(expression_range)?,
            expression_value,
        })
    } else {
        None
    }
}

fn insert_property_definitions(
    element: &Element,
    properties: &mut Vec<PropertyInformation>,
    offset_to_position: &mut OffsetToPositionMapper,
) {
    if let Some(element_node) = element.node.as_ref() {
        let element_range = element_node.text_range();

        for prop_info in properties {
            if let Some(v) = element.bindings.get(prop_info.name.as_str()) {
                if let Some(span) = &v.borrow().span {
                    let offset = span.span().offset as u32;
                    println!(
                        "Property {} is at {:?}",
                        prop_info.name,
                        offset_to_position.map(offset)
                    );
                    if element.source_file().map(|sf| sf.path())
                        == span.source_file.as_ref().map(|sf| sf.path())
                        && element_range.contains(offset.into())
                    {
                        if let Some(definition) =
                            find_expression_range(element_node, offset, offset_to_position)
                        {
                            prop_info.defined_at = Some(definition);
                        }
                    }
                }
            }
        }
    }
}

fn get_properties(
    element: &ElementRc,
    offset_to_position: &mut OffsetToPositionMapper,
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

fn get_element_information(
    element: &ElementRc,
    offset_to_position: &mut OffsetToPositionMapper,
) -> Option<ElementInformation> {
    let e = element.borrow();
    let range = e.node.as_ref().map(|n| n.text_range()).map(|r| lsp_types::Range {
        start: offset_to_position.map(r.start().into()),
        end: offset_to_position.map(r.end().into()),
    });
    Some(ElementInformation { id: e.id.clone(), type_name: format!("{}", e.base_type), range })
}

pub(crate) fn query_properties<'a>(
    document_cache: &mut DocumentCache,
    uri: &lsp_types::Url,
    source_version: i32,
    element: &ElementRc,
) -> Result<QueryPropertyResponse, crate::Error> {
    let mut mapper = document_cache.offset_to_position_mapper(uri);

    Ok(QueryPropertyResponse {
        properties: get_properties(&element, &mut mapper),
        element: get_element_information(&element, &mut mapper),
        source_uri: uri.to_string(),
        source_version,
    })
}

fn get_property_information<'a>(
    element: &ElementRc,
    offset_to_position: &mut OffsetToPositionMapper<'a>,
    property_name: &str,
) -> Result<PropertyInformation, Error> {
    if let Some(property) = get_properties(element, offset_to_position)
        .into_iter()
        .find(|pi| pi.name == property_name)
        .clone()
    {
        Ok(property.clone())
    } else {
        Err(format!("Element has no property with name {property_name}").into())
    }
}

fn validate_property_information(
    property: &PropertyInformation,
    property_name: &str,
    new_expression_type: Type,
    diag: &mut BuildDiagnostics,
) {
    if property.defined_at.is_none() {
        diag.push_error_with_span(
            format!("Property \"{property_name}\" is declared but undefined"),
            i_slint_compiler::diagnostics::SourceLocation {
                source_file: None,
                span: i_slint_compiler::diagnostics::Span::new(0),
            },
        );
    }

    // Check return type match:
    if new_expression_type != i_slint_compiler::langtype::Type::Invalid
        && new_expression_type.to_string() != property.type_name
    {
        diag.push_error_with_span(
            format!(
                "return type mismatch in \"{property_name}\" (was: {}, expected: {})",
                new_expression_type.to_string(),
                property.type_name
            ),
            i_slint_compiler::diagnostics::SourceLocation {
                source_file: None,
                span: i_slint_compiler::diagnostics::Span::new(0),
            },
        );
    }
}

fn create_workspace_edit_for_set_binding<'a>(
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

pub(crate) fn set_binding<'a>(
    document_cache: &mut DocumentCache,
    uri: &lsp_types::Url,
    element: &ElementRc,
    property_name: &str,
    new_expression: String,
) -> Result<(SetBindingResponse, Option<WorkspaceEdit>), Error> {
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

    let property = match get_property_information(
        element,
        &mut &mut document_cache.offset_to_position_mapper(uri),
        property_name,
    ) {
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
                    diagnostics: diag
                        .iter()
                        .map(|d| crate::util::to_lsp_diag(d))
                        .collect::<Vec<_>>(),
                },
                None,
            ));
        }
    };

    validate_property_information(&property, property_name, new_expression_type, &mut diag);

    let workspace_edit = (!diag.has_error())
        .then(|| {
            create_workspace_edit_for_set_binding(
                uri,
                document_cache.document_version(uri)?,
                &property,
                new_expression,
            )
        })
        .flatten();

    Ok((
        SetBindingResponse {
            diagnostics: diag.iter().map(|d| crate::util::to_lsp_diag(d)).collect::<Vec<_>>(),
        },
        workspace_edit,
    ))
}

fn create_workspace_edit_for_remove_binding<'a>(
    uri: &lsp_types::Url,
    version: i32,
    property: &PropertyInformation,
) -> Option<lsp_types::WorkspaceEdit> {
    property.defined_at.as_ref().map(|defined_at| {
        let edit = lsp_types::TextEdit { range: defined_at.delete_range, new_text: String::new() };
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

pub(crate) fn remove_binding<'a>(
    document_cache: &mut DocumentCache,
    uri: &lsp_types::Url,
    element: &ElementRc,
    property_name: &str,
) -> Result<WorkspaceEdit, Error> {
    let property = get_property_information(
        element,
        &mut &mut document_cache.offset_to_position_mapper(uri),
        property_name,
    )?;

    let workspace_edit = create_workspace_edit_for_remove_binding(
        uri,
        document_cache
            .document_version(uri)
            .ok_or_else(|| Into::<Error>::into("Document not found in cache"))?,
        &property,
    )
    .ok_or_else(|| Into::<Error>::into("Failed to create workspace edit to remove property"))?;

    Ok(workspace_edit)
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
        Some((element.clone(), get_properties(&element, &mut dc.offset_to_position_mapper(url))))
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
            definition.delete_range.start.line,
            definition.delete_range.start.character,
            definition.delete_range.end.line,
            definition.delete_range.end.character,
        );

        assert_eq!(definition.delete_range.start.line, sl);
        assert_eq!(definition.delete_range.start.character, sc);
        assert_eq!(definition.delete_range.end.line, el);
        assert_eq!(definition.delete_range.end.character, ec);
    }

    #[test]
    fn test_get_property_delete_range_no_extend() {
        delete_range_test(
            r#"import { VerticalBox } from "std-widgets.slint";

MainWindow := Window {
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
            29, // This is greedy!
        );
    }

    #[test]
    fn test_get_property_delete_range_line_extend_left_extra_indent() {
        delete_range_test(
            r#"import { VerticalBox } from "std-widgets.slint";

MainWindow := Window {
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
            12,
            6,
            25,
        );
    }

    #[test]
    fn test_get_property_delete_range_line_extend_left_no_ws() {
        delete_range_test(
            r#"import { VerticalBox } from "std-widgets.slint";

MainWindow := Window {
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

MainWindow := Window {
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
    fn test_get_property_delete_range_extend_left_many_lines_to_de_indent() {
        delete_range_test(
            r#"import { VerticalBox } from "std-widgets.slint";

MainWindow := Window {
    VerticalBox {
        Text {
            font-size: 12px;
             // Keep
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

MainWindow := Window {
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
            7,
            12,
            14,
            25,
        );
    }

    #[test]
    fn test_get_property_delete_range_extend_left_un_indented_property() {
        delete_range_test(
            r#"import { VerticalBox } from "std-widgets.slint";

MainWindow := Window {
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
            0,
            15,
            13,
        );
    }

    #[test]
    fn test_get_property_delete_range_extend_left_leading_line_comment() {
        delete_range_test(
            r#"import { VerticalBox } from "std-widgets.slint";

MainWindow := Window {
    VerticalBox {
        Text {
            font-size: 12px;
          // Keep
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
            7,
            12,
            15,
            35,
        );
    }

    #[test]
    fn test_get_property_delete_range_right_extend() {
        delete_range_test(
            r#"import { VerticalBox } from "std-widgets.slint";

MainWindow := Window {
    VerticalBox {
        Text {
            text: "text"; // Cut
                // Cut
        }
    }
}
            "#
            .to_string(),
            4,
            12,
            5,
            12,
            7,
            0,
        );
    }

    #[test]
    fn test_get_property_delete_range_right_extend_to_empty_line() {
        delete_range_test(
            r#"import { VerticalBox } from "std-widgets.slint";

MainWindow := Window {
    VerticalBox {
        Text {
            text: "text"; // Cut
                // Cut
                /*   Cut
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
            9,
            0,
        );
    }

    #[test]
    fn test_get_property_delete_range_no_right_extend() {
        delete_range_test(
            r#"import { VerticalBox } from "std-widgets.slint";

MainWindow := Window {
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

MainWindow := Window {
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

MainWindow := Window {
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

MainWindow := Window {
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
            54,
        );
    }

    #[test]
    fn test_get_property_definition() {
        let (mut dc, url, _) = loaded_document_cache("fluent",
            r#"import { LineEdit, Button, Slider, HorizontalBox, VerticalBox } from "std-widgets.slint";

Base1 := Rectangle {
    property<int> foo = 42;
}

Base2 := Base1 {
    foo: 23;
}

MainWindow := Window {
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
        assert_eq!(declaration.start_position.character, 13); // This should probably point to the start of
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

SomeRect := Rectangle {
    foo := InvalidType {
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
    ) -> (SetBindingResponse, Option<WorkspaceEdit>) {
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

        assert_eq!(edit, None);
        assert_eq!(result.diagnostics.len(), 1_usize);
        assert_eq!(result.diagnostics[0].severity, Some(lsp_types::DiagnosticSeverity::ERROR));
        assert!(result.diagnostics[0].message.contains("undefined"));
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
