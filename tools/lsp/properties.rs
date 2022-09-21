// Copyright © SixtyFPS GmbH <info@slint-ui.com>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-commercial

use i_slint_compiler::diagnostics::Spanned;
use i_slint_compiler::object_tree::{Element, ElementRc};
use i_slint_compiler::parser::{syntax_nodes, SyntaxKind};

#[derive(serde::Deserialize, serde::Serialize, Debug)]
pub(crate) struct DefinitionInformation {
    start_offset: u32,
    end_offset: u32,
    expression_start: u32,
    expression_end: u32,
}

#[derive(serde::Deserialize, serde::Serialize, Debug)]
pub(crate) struct DeclarationInformation {
    uri: String,
    start_offset: u32,
}

#[derive(serde::Deserialize, serde::Serialize, Debug)]
pub(crate) struct PropertyInformation {
    name: String,
    type_name: String,
    declared_at: Option<DeclarationInformation>,
    defined_at: Option<DefinitionInformation>, // Range in the elements source file!
}

#[derive(serde::Deserialize, serde::Serialize, Debug)]
pub(crate) struct ElementInformation {
    id: String,
    type_name: String,
}

#[derive(serde::Deserialize, serde::Serialize)]
pub(crate) struct QueryPropertyResponse {
    properties: Vec<PropertyInformation>,
    element: Option<ElementInformation>,
    source_uri: Option<String>,
}

impl QueryPropertyResponse {
    pub fn no_element_response(uri: String) -> Self {
        QueryPropertyResponse { properties: vec![], element: None, source_uri: Some(uri) }
    }
}

// This gets defined accessibility properties...
fn get_reserved_properties() -> impl Iterator<Item = PropertyInformation> {
    i_slint_compiler::typeregister::reserved_properties().map(|p| PropertyInformation {
        name: p.0.to_string(),
        type_name: format!("{}", p.1),
        declared_at: None,
        defined_at: None,
    })
}

fn source_file(element: &Element) -> Option<String> {
    element.source_file().map(|sf| sf.path().to_string_lossy().to_string())
}

fn get_element_properties(element: &Element) -> impl Iterator<Item = PropertyInformation> + '_ {
    let file = source_file(element);

    element.property_declarations.iter().map(move |(name, value)| {
        let declared_at = file.as_ref().and_then(|file| {
            value
                .type_node()
                .map(|n| n.text_range().start().into())
                .map(|p| DeclarationInformation { uri: file.clone(), start_offset: p })
        });
        PropertyInformation {
            name: name.clone(),
            type_name: format!("{}", value.property_type),
            declared_at,
            defined_at: None,
        }
    })
}

fn insert_property_definition_range(
    property: &str,
    properties: &mut [PropertyInformation],
    range: DefinitionInformation,
) {
    let index = properties
        .binary_search_by(|p| (p.name[..]).cmp(property))
        .expect("property must be known");
    properties[index].defined_at = Some(range);
}

fn find_expression_range(
    element: &syntax_nodes::Element,
    offset: u32,
) -> Option<DefinitionInformation> {
    let mut result = DefinitionInformation {
        start_offset: 0,
        end_offset: 0,
        expression_start: 0,
        expression_end: 0,
    };
    if let Some(token) = element.token_at_offset(offset.into()).right_biased() {
        for ancestor in token.parent_ancestors() {
            if ancestor.kind() == SyntaxKind::BindingExpression {
                // The BindingExpression contains leading and trailing whitespace + `;`
                let expr_range = ancestor
                    .first_child()
                    .expect("A BindingExpression needs to have a child!")
                    .text_range();
                result.expression_start = expr_range.start().into();
                result.expression_end = expr_range.end().into();
                continue;
            }
            if ancestor.kind() == SyntaxKind::Binding {
                let total_range = ancestor.text_range();
                result.start_offset = total_range.start().into();
                result.end_offset = total_range.end().into();
                break;
            }
            if ancestor.kind() == SyntaxKind::Element {
                // There should have been a binding before the element!
                break;
            }
        }
    }
    if result.start_offset < result.expression_start
        && result.expression_start <= result.expression_end
        && result.expression_end < result.end_offset
    {
        return Some(result);
    } else {
        None
    }
}

fn insert_property_definitions(element: &Element, properties: &mut Vec<PropertyInformation>) {
    let element_node = element.node.as_ref().expect("Element has to have a node here!");
    let element_range = element_node.text_range();

    for (k, v) in &element.bindings {
        if let Some(span) = &v.borrow().span {
            let offset = span.span().offset as u32;
            if element.source_file().map(|sf| sf.path())
                == span.source_file.as_ref().map(|sf| sf.path())
                && element_range.contains(offset.into())
            {
                if let Some(definition) = find_expression_range(element_node, offset) {
                    insert_property_definition_range(k, properties, definition);
                }
            }
        }
    }
}

fn get_properties(element: &ElementRc) -> Vec<PropertyInformation> {
    let mut result: Vec<_> = get_reserved_properties().collect();

    let mut current_element = Some(element.clone());
    while let Some(e) = current_element {
        use i_slint_compiler::langtype::Type;

        result.extend(get_element_properties(&e.borrow()));

        // Go into base_type!
        match &e.borrow().base_type {
            Type::Component(c) => current_element = Some(c.root_element.clone()),
            Type::Builtin(b) => {
                result.extend(b.properties.iter().map(|(k, t)| PropertyInformation {
                    name: k.clone(),
                    type_name: format!("{}", t.ty),
                    declared_at: None,
                    defined_at: None,
                }));
                current_element = None;
            }
            Type::Native(n) => {
                result.extend(n.properties.iter().map(|(k, t)| PropertyInformation {
                    name: k.clone(),
                    type_name: format!("{}", t.ty),
                    declared_at: None,
                    defined_at: None,
                }));
                current_element = None;
            }
            _ => current_element = None,
        }
    }

    result.sort_by(|a, b| a.name.cmp(&b.name));

    insert_property_definitions(&element.borrow(), &mut result);

    result
}

fn get_element_information(element: &ElementRc) -> Option<ElementInformation> {
    let e = element.borrow();
    Some(ElementInformation { id: e.id.clone(), type_name: format!("{}", e.base_type) })
}

pub(crate) fn query_properties(element: &ElementRc) -> Result<QueryPropertyResponse, crate::Error> {
    Ok(QueryPropertyResponse {
        properties: get_properties(&element),
        element: get_element_information(&element),
        source_uri: source_file(&element.borrow()),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    use crate::test::complex_document_cache;

    fn find_property<'a>(
        properties: &'a [PropertyInformation],
        name: &'a str,
    ) -> Option<&'a PropertyInformation> {
        properties.iter().find(|p| p.name == name)
    }

    fn properties_at_position(line: u32, character: u32) -> Option<Vec<PropertyInformation>> {
        let (mut dc, url, _) = complex_document_cache("fluent");
        let element = crate::server_loop::element_at_position(
            &mut dc,
            lsp_types::TextDocumentIdentifier { uri: url },
            lsp_types::Position { line, character },
        )
        .ok()?;
        Some(get_properties(&element))
    }

    #[test]
    fn test_get_properties() {
        let result = properties_at_position(6, 4).unwrap();

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
        let result = properties_at_position(21, 30).unwrap();
        let property = find_property(&result, "background").unwrap();

        let def_at = property.defined_at.as_ref().unwrap();
        assert_eq!((def_at.expression_end - def_at.expression_start) as usize, "lightblue".len());
    }
}
