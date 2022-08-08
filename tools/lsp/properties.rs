// Copyright © SixtyFPS GmbH <info@slint-ui.com>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-commercial

use i_slint_compiler::diagnostics::Spanned;
use i_slint_compiler::object_tree::{Element, ElementRc};

#[derive(serde::Deserialize, serde::Serialize)]
pub(crate) struct PropertyInformation {
    name: String,
    type_name: String,
    declared_at: Option<(String, u32)>,
    defined_at: Option<(u32, u32)>, // Range in the elements source file!
}

#[derive(serde::Deserialize, serde::Serialize)]
pub(crate) struct QueryPropertyResponse {
    properties: Vec<PropertyInformation>,
    source_file: Option<String>,
}

// This gets defined accessibility properties...
fn get_reserved_properties(result: &mut Vec<PropertyInformation>) {
    result.extend(i_slint_compiler::typeregister::reserved_properties().map(|p| {
        PropertyInformation {
            name: p.0.to_string(),
            type_name: format!("{}", p.1),
            declared_at: None,
            defined_at: None,
        }
    }))
}

fn source_file(element: &Element) -> Option<String> {
    element.source_file().map(|sf| sf.path().to_string_lossy().to_string())
}

fn get_element_properties(element: &Element, result: &mut Vec<PropertyInformation>) {
    let file = source_file(element);

    for (name, value) in &element.property_declarations {
        let declared_at = file.as_ref().and_then(|file| {
            value.type_node().map(|n| n.text_range().start().into()).map(|p| (file.clone(), p))
        });
        result.push(PropertyInformation {
            name: name.clone(),
            type_name: format!("{}", value.property_type),
            declared_at,
            defined_at: None,
        });
    }
}

fn insert_property_definition_range(
    property: &str,
    properties: &mut [PropertyInformation],
    range: (u32, u32),
) {
    let index = properties
        .binary_search_by(|p| (p.name[..]).cmp(property))
        .expect("property must be known");
    properties[index].defined_at = Some(range);
}

fn insert_property_definitions(element: &Element, properties: &mut Vec<PropertyInformation>) {
    let node = if let Some(node) = element.node.as_ref() {
        node
    } else {
        return;
    };

    for (k, v) in &element.bindings {
        if let Some(span) = &v.borrow().span {
            let offset: u32 = span.span().offset.try_into().unwrap_or(u32::MAX);
            if element.source_file().map(|sf| sf.path())
                != span.source_file.as_ref().map(|sf| sf.path())
                && node.text_range().contains(offset.into())
            {
                continue; // ignore definitions in files other than the element
            }

            if let Some(token) = node.token_at_offset(offset.into()).left_biased() {
                let range = token.text_range();
                insert_property_definition_range(
                    k,
                    properties,
                    (range.start().into(), range.end().into()),
                );
            };
        }
    }
}

fn get_properties(element: &ElementRc) -> Vec<PropertyInformation> {
    let mut result = vec![];

    get_reserved_properties(&mut result);

    let mut current_element = Some(element.clone());
    while let Some(e) = current_element {
        use i_slint_compiler::langtype::Type;

        get_element_properties(&e.borrow(), &mut result);

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

pub(crate) fn query_properties(element: &ElementRc) -> Result<QueryPropertyResponse, crate::Error> {
    Ok(QueryPropertyResponse {
        properties: get_properties(&element),
        source_file: source_file(&element.borrow()),
    })
}
