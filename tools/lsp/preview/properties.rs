// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

use crate::common::{self, Result};
use crate::util;

use i_slint_compiler::diagnostics::{SourceFileVersion, Spanned};
use i_slint_compiler::langtype::{ElementType, Type};
use i_slint_compiler::object_tree::{Element, PropertyDeclaration, PropertyVisibility};
use i_slint_compiler::parser::{syntax_nodes, Language, SyntaxKind};

use std::collections::HashSet;

#[derive(Clone, Debug, PartialEq)]
pub struct DefinitionInformation {
    pub property_definition_range: lsp_types::Range,
    pub selection_range: lsp_types::Range,
    pub expression_range: lsp_types::Range,
    pub expression_value: String,
}

#[derive(Clone, Debug, PartialEq)]
pub struct DeclarationInformation {
    pub uri: lsp_types::Url,
    pub start_position: lsp_types::Position,
}

#[derive(Clone, Debug, PartialEq)]
pub struct PropertyInformation {
    pub name: String,
    pub type_name: String,
    pub declared_at: Option<DeclarationInformation>,
    pub defined_at: Option<DefinitionInformation>, // Range in the elements source file!
    pub group: String,
}

#[derive(Clone, Debug)]
pub struct ElementInformation {
    pub id: String,
    pub type_name: String,
    pub range: Option<lsp_types::Range>,
}

#[derive(Clone, Debug)]
pub struct QueryPropertyResponse {
    pub properties: Vec<PropertyInformation>,
    pub element: Option<ElementInformation>,
    pub source_uri: String,
    pub source_version: i32,
}

// This gets defined accessibility properties...
fn get_reserved_properties<'a>(
    group: &'a str,
    properties: impl Iterator<Item = (&'static str, Type)> + 'a,
) -> impl Iterator<Item = PropertyInformation> + 'a {
    properties.filter(|(_, t)| !matches!(t, Type::Callback { .. })).map(|p| PropertyInformation {
        name: p.0.to_string(),
        type_name: p.1.to_string(),
        declared_at: None,
        defined_at: None,
        group: group.to_string(),
    })
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
    group: &str,
    is_local_element: bool,
    result: &mut Vec<PropertyInformation>,
) {
    result.extend(element.property_declarations.iter().filter_map(move |(name, value)| {
        if !property_is_editable(value, is_local_element) {
            return None;
        }

        let declared_at = value
            .type_node()
            .as_ref()
            .and_then(util::map_node_and_url)
            .map(|(uri, range)| DeclarationInformation { uri, start_position: range.start });
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
) -> Option<DefinitionInformation> {
    let mut selection_range = None;
    let mut expression_range = None;
    let mut expression_value = None;
    let mut property_definition_range = None;

    let source_file = element.source_file()?;

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
                .or(property_definition_range);
                break;
            }
            if ancestor.kind() == SyntaxKind::Element {
                // There should have been a binding before the element!
                break;
            }
        }
    }
    Some(DefinitionInformation {
        property_definition_range: util::map_range(source_file, property_definition_range?),
        selection_range: util::map_range(source_file, selection_range?),
        expression_range: util::map_range(source_file, expression_range?),
        expression_value: expression_value?,
    })
}

fn find_property_binding_offset(
    element: &common::ElementRcNode,
    property_name: &str,
) -> Option<u32> {
    let element_range = element.with_element_node(|node| node.text_range());

    let element = element.element.borrow();

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
    element: &common::ElementRcNode,
    mut properties: Vec<PropertyInformation>,
) -> Vec<PropertyInformation> {
    for prop_info in properties.iter_mut() {
        if let Some(offset) = find_property_binding_offset(element, prop_info.name.as_str()) {
            prop_info.defined_at =
                element.with_element_node(|node| find_expression_range(node, offset));
        }
    }
    properties
}

fn get_properties(element: &common::ElementRcNode) -> Vec<PropertyInformation> {
    let mut result = Vec::new();
    add_element_properties(&element.element.borrow(), "", true, &mut result);

    let mut current_element = element.element.clone();

    let geometry_prop = HashSet::from(["x", "y", "width", "height"]);

    loop {
        let base_type = current_element.borrow().base_type.clone();
        match base_type {
            ElementType::Component(c) => {
                current_element = c.root_element.clone();
                add_element_properties(&current_element.borrow(), &c.id, false, &mut result);
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
                        i_slint_compiler::typeregister::RESERVED_ROTATION_PROPERTIES
                            .iter()
                            .cloned(),
                    ));
                }

                if b.name == "Rectangle" {
                    result.extend(get_reserved_properties(
                        "drop-shadow",
                        i_slint_compiler::typeregister::RESERVED_DROP_SHADOW_PROPERTIES
                            .iter()
                            .cloned(),
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
            i_slint_compiler::typeregister::RESERVED_GEOMETRY_PROPERTIES.iter().cloned(),
        ));
        result.extend(
            get_reserved_properties(
                "layout",
                i_slint_compiler::typeregister::RESERVED_LAYOUT_PROPERTIES.iter().cloned(),
            )
            // padding arbitrary items is not yet implemented
            .filter(|x| !x.name.starts_with("padding")),
        );
        // FIXME: ideally only if parent is a grid layout
        result.extend(get_reserved_properties(
            "layout",
            i_slint_compiler::typeregister::RESERVED_GRIDLAYOUT_PROPERTIES.iter().cloned(),
        ));
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
        if current_element.borrow().is_binding_set("accessible-role", true) {
            result.extend(get_reserved_properties(
                "accessibility",
                i_slint_compiler::typeregister::reserved_accessibility_properties(),
            ));
        }
        break;
    }

    insert_property_definitions(element, result)
}

fn find_block_range(element: &common::ElementRcNode) -> Option<lsp_types::Range> {
    element.with_element_node(|node| {
        let open_brace = node.child_token(SyntaxKind::LBrace)?;
        let close_brace = node.child_token(SyntaxKind::RBrace)?;

        Some(lsp_types::Range::new(
            util::map_position(node.source_file()?, open_brace.text_range().start()),
            util::map_position(node.source_file()?, close_brace.text_range().end()),
        ))
    })
}

fn get_element_information(element: &common::ElementRcNode) -> ElementInformation {
    let range = element.with_decorated_node(|node| util::map_node(&node));
    let e = element.element.borrow();
    let type_name = if matches!(&e.base_type, ElementType::Builtin(b) if b.name == "Empty") {
        String::new()
    } else {
        e.base_type.to_string()
    };
    ElementInformation { id: e.id.clone(), type_name, range }
}

pub(crate) fn query_properties(
    uri: &lsp_types::Url,
    source_version: SourceFileVersion,
    element: &common::ElementRcNode,
) -> Result<QueryPropertyResponse> {
    Ok(QueryPropertyResponse {
        properties: get_properties(element),
        element: Some(get_element_information(element)),
        source_uri: uri.to_string(),
        source_version: source_version.unwrap_or(i32::MIN),
    })
}

fn get_property_information(
    properties: &[PropertyInformation],
    property_name: &str,
) -> Result<PropertyInformation> {
    if let Some(property) = properties.iter().find(|pi| pi.name == property_name) {
        Ok(property.clone())
    } else {
        Err(format!("Element has no property with name {property_name}").into())
    }
}

fn create_text_document_edit_for_set_binding_on_existing_property(
    uri: lsp_types::Url,
    version: SourceFileVersion,
    property: &PropertyInformation,
    new_expression: String,
) -> Option<lsp_types::TextDocumentEdit> {
    property.defined_at.as_ref().map(|defined_at| {
        let edit =
            lsp_types::TextEdit { range: defined_at.expression_range, new_text: new_expression };
        common::create_text_document_edit(uri, version, vec![edit])
    })
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

fn create_text_document_edit_for_set_binding_on_known_property(
    uri: lsp_types::Url,
    version: SourceFileVersion,
    element: &common::ElementRcNode,
    properties: &[PropertyInformation],
    property_name: &str,
    new_expression: &str,
) -> Option<lsp_types::TextDocumentEdit> {
    let block_range = find_block_range(element);

    find_insert_range_for_property(&block_range, properties, property_name).map(
        |(range, insert_type)| {
            let indent = util::find_element_indent(element).unwrap_or_default();
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
            common::create_text_document_edit(uri, version, vec![edit])
        },
    )
}

pub fn set_binding(
    uri: lsp_types::Url,
    version: SourceFileVersion,
    element: &common::ElementRcNode,
    property_name: &str,
    new_expression: String,
) -> Option<lsp_types::WorkspaceEdit> {
    set_binding_impl(uri, version, element, property_name, new_expression)
        .map(|edit| common::create_workspace_edit_from_text_document_edits(vec![edit]))
}

pub fn set_binding_impl(
    uri: lsp_types::Url,
    version: SourceFileVersion,
    element: &common::ElementRcNode,
    property_name: &str,
    new_expression: String,
) -> Option<lsp_types::TextDocumentEdit> {
    let properties = get_properties(element);
    let property = get_property_information(&properties, property_name).ok()?;

    if property.defined_at.is_some() {
        // Change an already defined property:
        create_text_document_edit_for_set_binding_on_existing_property(
            uri,
            version,
            &property,
            new_expression,
        )
    } else {
        // Add a new definition to a known property:
        create_text_document_edit_for_set_binding_on_known_property(
            uri,
            version,
            element,
            &properties,
            property_name,
            &new_expression,
        )
    }
}

#[cfg(any(feature = "preview-external", feature = "preview-engine"))]
pub fn set_bindings(
    uri: lsp_types::Url,
    version: SourceFileVersion,
    element: &common::ElementRcNode,
    properties: &[crate::common::PropertyChange],
) -> Option<lsp_types::WorkspaceEdit> {
    let edits = properties
        .iter()
        .filter_map(|p| set_binding_impl(uri.clone(), version, element, &p.name, p.value.clone()))
        .collect::<Vec<_>>();

    (edits.len() != properties.len())
        .then_some(common::create_workspace_edit_from_text_document_edits(edits))
}

#[cfg(any(feature = "preview-external", feature = "preview-engine"))]
fn element_at_source_code_position(
    document_cache: &common::DocumentCache,
    position: &common::VersionedPosition,
) -> Result<common::ElementRcNode> {
    if &document_cache.document_version(position.url()) != position.version() {
        return Err("Document version mismatch.".into());
    }

    let doc = document_cache
        .get_document(position.url())
        .ok_or_else(|| "Document not found".to_string())?;

    let source_file = doc
        .node
        .as_ref()
        .map(|n| n.source_file.clone())
        .ok_or_else(|| "Document had no node".to_string())?;
    let element_position = util::map_position(&source_file, position.offset().into());

    Ok(document_cache.element_at_position(position.url(), &element_position).ok_or_else(|| {
        format!("No element found at the given start position {:?}", &element_position)
    })?)
}

#[cfg(any(feature = "preview-external", feature = "preview-engine"))]
pub fn update_element_properties(
    document_cache: &common::DocumentCache,
    position: common::VersionedPosition,
    properties: Vec<common::PropertyChange>,
) -> Option<lsp_types::WorkspaceEdit> {
    let element = element_at_source_code_position(document_cache, &position).ok()?;

    set_bindings(position.url().clone(), *position.version(), &element, &properties)
}

fn create_workspace_edit_for_remove_binding(
    uri: lsp_types::Url,
    version: SourceFileVersion,
    range: lsp_types::Range,
) -> lsp_types::WorkspaceEdit {
    let edit = lsp_types::TextEdit { range, new_text: String::new() };
    common::create_workspace_edit(uri.clone(), version, vec![edit])
}

pub fn remove_binding(
    uri: lsp_types::Url,
    version: SourceFileVersion,
    element: &common::ElementRcNode,
    property_name: &str,
) -> Result<lsp_types::WorkspaceEdit> {
    let source_file = element.with_element_node(|node| node.source_file.clone());

    let range = find_property_binding_offset(element, property_name)
        .and_then(|offset| {
            element.with_element_node(|node| node.token_at_offset(offset.into()).right_biased())
        })
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

                    return Some(util::map_range(&source_file, rowan::TextRange::new(start, end)));
                }
                if ancestor.kind() == SyntaxKind::Element {
                    // There should have been a binding before the element!
                    break;
                }
            }
            None
        })
        .ok_or_else(|| Into::<common::Error>::into("Could not find range to delete."))?;

    Ok(create_workspace_edit_for_remove_binding(uri, version, range))
}

#[cfg(test)]
mod tests {
    use super::*;

    use crate::language::test::{complex_document_cache, loaded_document_cache};

    fn find_property<'a>(
        properties: &'a [PropertyInformation],
        name: &'_ str,
    ) -> Option<&'a PropertyInformation> {
        properties.iter().find(|p| p.name == name)
    }

    fn properties_at_position_in_cache(
        line: u32,
        character: u32,
        document_cache: &common::DocumentCache,
        url: &lsp_types::Url,
    ) -> Option<(common::ElementRcNode, Vec<PropertyInformation>)> {
        let element =
            document_cache.element_at_position(url, &lsp_types::Position { line, character })?;
        Some((element.clone(), get_properties(&element)))
    }

    fn properties_at_position(
        line: u32,
        character: u32,
    ) -> Option<(
        common::ElementRcNode,
        Vec<PropertyInformation>,
        common::DocumentCache,
        lsp_types::Url,
    )> {
        let (dc, url, _) = complex_document_cache();
        if let Some((e, p)) = properties_at_position_in_cache(line, character, &dc, &url) {
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
        // Accessible property should not be present since the role is none
        assert_eq!(find_property(&result, "accessible-label"), None);
        assert_eq!(find_property(&result, "accessible-action-default"), None);

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

        // On a Button
        let (_, result, _, _) = properties_at_position(48, 4).unwrap();

        assert_eq!(&find_property(&result, "text").unwrap().type_name, "string");
        // Accessible property should not be present since the role is button
        assert_eq!(find_property(&result, "accessible-label").unwrap().type_name, "string");
        // No callbacks
        assert_eq!(find_property(&result, "accessible-action-default"), None);
        assert_eq!(find_property(&result, "clicked"), None);
    }

    #[test]
    fn test_element_information() {
        let (document_cache, url, _) = complex_document_cache();
        let element =
            document_cache.element_at_position(&url, &lsp_types::Position::new(33, 4)).unwrap();

        let result = get_element_information(&element);

        let r = result.range.unwrap();
        assert_eq!(r.start.line, 32);
        assert_eq!(r.start.character, 12);
        assert_eq!(r.end.line, 35);
        assert_eq!(r.end.character, 13);

        assert_eq!(result.type_name, "Text");
    }

    #[test]
    fn test_element_information_empty() {
        let (document_cache, url, _) = loaded_document_cache(
            "component FooBar { property <int> foo; btn := Button {} }".into(),
        );
        let element =
            document_cache.element_at_position(&url, &lsp_types::Position::new(1, 19)).unwrap();
        let result = get_element_information(&element);
        assert_eq!(result.type_name, "");
        assert_eq!(result.id, "root");

        let element =
            document_cache.element_at_position(&url, &lsp_types::Position::new(1, 39)).unwrap();
        let result = get_element_information(&element);
        // Because `Button` is not defined in this scope
        assert_eq!(result.type_name, "<error>");
        assert_eq!(result.id, "btn");
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

        let (dc, url, _) = loaded_document_cache(content);

        let (_, result) = properties_at_position_in_cache(pos_l, pos_c, &dc, &url).unwrap();

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
        let (dc, url, _) = loaded_document_cache(
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
            base2 := Base2 {
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

        let doc = dc.get_document(&url).unwrap();
        let source = &doc.node.as_ref().unwrap().source_file;
        let (l, c) = source.line_column(source.source().unwrap().find("base2 :=").unwrap());
        let (_, result) = properties_at_position_in_cache(l as u32, c as u32, &dc, &url).unwrap();

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
        let (dc, url, _) = loaded_document_cache(
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

        let (_, result) = properties_at_position_in_cache(1, 25, &dc, &url).unwrap();

        let glob_property = find_property(&result, "glob").unwrap();
        assert_eq!(glob_property.type_name, "int");
        let declaration = glob_property.declared_at.as_ref().unwrap();
        assert_eq!(declaration.uri, url);
        assert_eq!(declaration.start_position.line, 2);
        assert_eq!(glob_property.group, "");
        assert_eq!(find_property(&result, "width"), None);

        let (_, result) = properties_at_position_in_cache(8, 4, &dc, &url).unwrap();
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
        let (dc, url, _) =
            loaded_document_cache(r#"export component Demo { Text { text: } }"#.to_string());

        let (_, result) = properties_at_position_in_cache(0, 35, &dc, &url).unwrap();

        let prop = find_property(&result, "text").unwrap();
        assert_eq!(prop.defined_at, None); // The property has no valid definition at this time
    }

    #[test]
    fn test_codeblock_property_declaration() {
        let (dc, url, _) = loaded_document_cache(
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

        let (_, result) = properties_at_position_in_cache(3, 0, &dc, &url).unwrap();
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
        let (dc, url, _) = loaded_document_cache(
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

        let (_, result) = properties_at_position_in_cache(11, 1, &dc, &url).unwrap();
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
        let (dc, url, _) = loaded_document_cache(
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

        let (_, result) = properties_at_position_in_cache(3, 0, &dc, &url).unwrap();
        assert_eq!(find_property(&result, "a").unwrap().type_name, "int");
        assert_eq!(find_property(&result, "b").unwrap().type_name, "int");
        assert_eq!(find_property(&result, "c").unwrap().type_name, "int");
        assert_eq!(find_property(&result, "d").unwrap().type_name, "int");

        let (_, result) = properties_at_position_in_cache(10, 0, &dc, &url).unwrap();
        assert_eq!(find_property(&result, "a"), None);
        assert_eq!(find_property(&result, "b").unwrap().type_name, "int");
        assert_eq!(find_property(&result, "c"), None);
        assert_eq!(find_property(&result, "d").unwrap().type_name, "int");

        let (_, result) = properties_at_position_in_cache(13, 0, &dc, &url).unwrap();
        assert_eq!(find_property(&result, "enabled").unwrap().type_name, "bool");
        assert_eq!(find_property(&result, "pressed"), None);
    }

    fn set_binding_helper(
        property_name: &str,
        new_value: &str,
    ) -> Option<lsp_types::WorkspaceEdit> {
        let (element, _, _, url) = properties_at_position(18, 15).unwrap();
        set_binding(url, None, &element, property_name, new_value.to_string())
    }

    #[test]
    fn test_set_binding_valid_expression_unknown_property() {
        let edit = set_binding_helper("foobar", "1 + 2");

        assert_eq!(edit, None);
    }

    #[test]
    fn test_set_binding_valid_expression_undefined_property() {
        let edit = set_binding_helper("x", "30px");

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
    }

    #[test]
    fn test_set_binding_valid() {
        let edit = set_binding_helper("min-width", "5px");

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
    }
}
