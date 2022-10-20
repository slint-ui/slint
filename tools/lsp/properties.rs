// Copyright © SixtyFPS GmbH <info@slint-ui.com>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-commercial

use i_slint_compiler::diagnostics::Spanned;
use i_slint_compiler::object_tree::{Element, ElementRc};
use i_slint_compiler::parser::{syntax_nodes, SyntaxKind};

#[cfg(target_arch = "wasm32")]
use crate::wasm_prelude::*;

#[derive(serde::Deserialize, serde::Serialize, Debug)]
pub(crate) struct DefinitionInformation {
    property_definition_range: lsp_types::Range,
    expression_range: lsp_types::Range,
}

#[derive(serde::Deserialize, serde::Serialize, Debug)]
pub(crate) struct DeclarationInformation {
    uri: String,
    start_position: lsp_types::Position,
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

fn get_element_properties<'a>(
    element: &'a Element,
    offset_to_position: &'a mut dyn FnMut(u32) -> lsp_types::Position,
) -> impl Iterator<Item = PropertyInformation> + 'a {
    let file = source_file(element);

    element.property_declarations.iter().map(move |(name, value)| {
        let declared_at = file.as_ref().and_then(|file| {
            value.type_node().map(|n| n.text_range().start().into()).map(|p| {
                let uri = lsp_types::Url::from_file_path(file).unwrap_or_else(|_| {
                    lsp_types::Url::parse("file:///)").expect("That should have been valid as URL!")
                });
                DeclarationInformation {
                    uri: uri.to_string(),
                    start_position: offset_to_position(p),
                }
            })
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
    offset_to_position: &mut dyn FnMut(u32) -> lsp_types::Position,
) -> Option<DefinitionInformation> {
    let mut property_definition_range = rowan::TextRange::default();
    let mut expression = rowan::TextRange::default();

    if let Some(token) = element.token_at_offset(offset.into()).right_biased() {
        for ancestor in token.parent_ancestors() {
            if ancestor.kind() == SyntaxKind::BindingExpression {
                // The BindingExpression contains leading and trailing whitespace + `;`
                let expr_range = ancestor
                    .first_child()
                    .expect("A BindingExpression needs to have a child!")
                    .text_range();
                expression = expr_range;
                continue;
            }
            if ancestor.kind() == SyntaxKind::Binding {
                property_definition_range = ancestor.text_range();
                break;
            }
            if ancestor.kind() == SyntaxKind::Element {
                // There should have been a binding before the element!
                break;
            }
        }
    }
    if property_definition_range.start() < expression.start()
        && expression.start() <= expression.end()
        && expression.end() < property_definition_range.end()
    {
        return Some(DefinitionInformation {
            // In the CST, the range end includes the last character, while in the lsp protocol the end of the
            // range is exclusive, i.e. it refers to the first excluded character. Hence the +1 below:
            property_definition_range: crate::util::text_range_to_lsp_range(
                property_definition_range,
                offset_to_position,
            ),
            expression_range: crate::util::text_range_to_lsp_range(expression, offset_to_position),
        });
    } else {
        None
    }
}

fn insert_property_definitions(
    element: &Element,
    properties: &mut Vec<PropertyInformation>,
    offset_to_position: &mut dyn FnMut(u32) -> lsp_types::Position,
) {
    let element_node = element.node.as_ref().expect("Element has to have a node here!");
    let element_range = element_node.text_range();

    for (k, v) in &element.bindings {
        if let Some(span) = &v.borrow().span {
            let offset = span.span().offset as u32;
            if element.source_file().map(|sf| sf.path())
                == span.source_file.as_ref().map(|sf| sf.path())
                && element_range.contains(offset.into())
            {
                if let Some(definition) =
                    find_expression_range(element_node, offset, offset_to_position)
                {
                    insert_property_definition_range(k, properties, definition);
                }
            }
        }
    }
}

fn get_properties(
    element: &ElementRc,
    offset_to_position: &mut dyn FnMut(u32) -> lsp_types::Position,
) -> Vec<PropertyInformation> {
    let mut result = vec![];

    let mut current_element = Some(element.clone());
    while let Some(e) = current_element {
        use i_slint_compiler::langtype::Type;

        result.extend(get_element_properties(&e.borrow(), offset_to_position));

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

    result.extend(get_reserved_properties()); // Add reserved properties last!

    // We can have duplicate properties that were defined and then changed further up the
    // Element tree. So we need to remove duplicates.
    result.sort_by(|a, b| b.name.cmp(&a.name)); // Sort is stable, sort `z` before `a`!
    result.reverse(); // Now the property definition is first and `a` is before `z`
    result.dedup_by(|a, b| a.name == b.name); // LEave the property definition in place, remove
                                              // re-definitions

    insert_property_definitions(&element.borrow(), &mut result, offset_to_position);

    result
}

fn get_element_information(element: &ElementRc) -> Option<ElementInformation> {
    let e = element.borrow();
    Some(ElementInformation { id: e.id.clone(), type_name: format!("{}", e.base_type) })
}

pub(crate) fn query_properties(
    element: &ElementRc,
    offset_to_position: &mut dyn FnMut(u32) -> lsp_types::Position,
) -> Result<QueryPropertyResponse, crate::Error> {
    Ok(QueryPropertyResponse {
        properties: get_properties(&element, offset_to_position),
        element: get_element_information(&element),
        source_uri: source_file(&element.borrow()),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    use crate::test::{complex_document_cache, loaded_document_cache};

    fn find_property<'a>(
        properties: &'a [PropertyInformation],
        name: &'a str,
    ) -> Option<&'a PropertyInformation> {
        properties.iter().find(|p| p.name == name)
    }

    fn properties_at_position_in_cache(
        line: u32,
        character: u32,
        mut dc: crate::server_loop::DocumentCache,
        url: lsp_types::Url,
    ) -> Option<Vec<PropertyInformation>> {
        let element = crate::server_loop::element_at_position(
            &mut dc,
            lsp_types::TextDocumentIdentifier { uri: url.clone() },
            lsp_types::Position { line, character },
        )
        .ok()?;
        Some(get_properties(&element, &mut |offset| {
            dc.byte_offset_to_position(offset, &url).expect("invalid node offset")
        }))
    }

    fn properties_at_position(line: u32, character: u32) -> Option<Vec<PropertyInformation>> {
        let (dc, url, _) = complex_document_cache("fluent");
        properties_at_position_in_cache(line, character, dc, url)
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
        assert_eq!(def_at.expression_range.end.line, def_at.expression_range.start.line);
        // -1 because the lsp range end location is exclusive.
        assert_eq!(
            (def_at.expression_range.end.character - 1 - def_at.expression_range.start.character)
                as usize,
            "lightblue".len()
        );
    }

    #[test]
    fn test_get_property_definition() {
        let (dc, url, _) = loaded_document_cache("fluent",
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
        let result = properties_at_position_in_cache(28, 15, dc, url).unwrap();

        let foo_property = find_property(&result, "foo").unwrap();

        assert_eq!(foo_property.type_name, "int");

        let declaration = foo_property.declared_at.as_ref().unwrap();
        assert_eq!(declaration.uri, file_url.to_string());
        assert_eq!(declaration.start_position.line, 3);
        assert_eq!(declaration.start_position.character, 13); // This should probably point to the start of
                                                              // `property<int> foo = 42`, not to the `<`
    }
}
