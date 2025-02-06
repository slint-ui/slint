// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

use crate::common::{self, Result, SourceFileVersion};
use crate::util;
use i_slint_compiler::diagnostics::Spanned;
use i_slint_compiler::expression_tree::{Expression, Unit};
use i_slint_compiler::langtype::{ElementType, Type};
use i_slint_compiler::object_tree::{Element, ElementRc, PropertyDeclaration, PropertyVisibility};
use i_slint_compiler::parser::{
    syntax_nodes, SyntaxKind, SyntaxNode, SyntaxToken, TextRange, TextSize,
};
use lsp_types::Url;
use smol_str::{SmolStr, ToSmolStr};

use std::collections::HashSet;
use std::path::PathBuf;

#[derive(Clone, Debug)]
pub enum CodeBlockOrExpression {
    CodeBlock(syntax_nodes::CodeBlock),
    Expression(syntax_nodes::Expression),
}

impl CodeBlockOrExpression {
    pub fn new(node: SyntaxNode) -> Option<Self> {
        match node.kind() {
            SyntaxKind::CodeBlock => Some(Self::CodeBlock(node.into())),
            SyntaxKind::Expression => Some(Self::Expression(node.into())),
            _ => None,
        }
    }

    pub fn expression(&self) -> Option<syntax_nodes::Expression> {
        match self {
            CodeBlockOrExpression::CodeBlock(_) => None,
            CodeBlockOrExpression::Expression(expr) => Some(expr.clone()),
        }
    }
}

impl std::ops::Deref for CodeBlockOrExpression {
    type Target = SyntaxNode;
    fn deref(&self) -> &Self::Target {
        match self {
            CodeBlockOrExpression::CodeBlock(cb) => cb,
            CodeBlockOrExpression::Expression(expr) => expr,
        }
    }
}

#[derive(Clone, Debug)]
pub struct DefinitionInformation {
    pub property_definition_range: TextRange,
    pub selection_range: TextRange,
    pub code_block_or_expression: CodeBlockOrExpression,
}

#[derive(Clone, Debug)]
pub struct DeclarationInformation {
    pub path: PathBuf,
    pub start_position: TextSize,
}

#[derive(Clone, Debug)]
pub struct PropertyInformation {
    pub name: SmolStr,
    pub priority: u32,
    pub ty: Type,
    pub declared_at: Option<DeclarationInformation>,
    /// Range of the binding in the element source file, if it exist
    pub defined_at: Option<DefinitionInformation>,
    /// Value of the property, which can be the default set from the base
    pub default_value: Option<Expression>,
    pub group: SmolStr,
    pub group_priority: u32,
}

#[derive(Clone, Debug)]
pub struct ElementInformation {
    pub id: SmolStr,
    pub type_name: SmolStr,
    pub range: TextRange,
}

#[derive(Clone, Debug)]
pub struct QueryPropertyResponse {
    pub properties: Vec<PropertyInformation>,
    pub element: Option<ElementInformation>,
    pub source_uri: String,
    pub source_version: i32,
}

const HIGH_PRIORITY: u32 = 100;
const DEFAULT_PRIORITY: u32 = 1000;

// This gets defined accessibility properties...
fn get_reserved_properties<'a>(
    group: &'a str,
    group_priority: u32,
    properties: impl Iterator<Item = (&'static str, Type)> + 'a,
) -> impl Iterator<Item = PropertyInformation> + 'a {
    properties.filter(move |(_, t)| !matches!(t, Type::Callback { .. })).map(move |p| {
        PropertyInformation {
            name: p.0.into(),
            priority: DEFAULT_PRIORITY,
            ty: p.1,
            declared_at: None,
            defined_at: None,
            default_value: None,
            group: group.into(),
            group_priority,
        }
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
    group_priority: u32,
    is_local_element: bool,
    result: &mut Vec<PropertyInformation>,
) {
    result.extend(element.property_declarations.iter().filter_map(move |(name, value)| {
        if !property_is_editable(value, is_local_element) {
            return None;
        }

        let declared_at = value.type_node().as_ref().map(|n| DeclarationInformation {
            path: n.source_file.path().to_path_buf(),
            start_position: n.text_range().start(),
        });
        Some(PropertyInformation {
            name: name.clone(),
            priority: DEFAULT_PRIORITY,
            ty: value.property_type.clone(),
            declared_at,
            defined_at: None,
            default_value: None,
            group: group.into(),
            group_priority,
        })
    }))
}

/// Move left from the start of a `token` to include white-space and comments that go with it.
fn left_extend(token: SyntaxToken) -> SyntaxToken {
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
fn right_extend(token: SyntaxToken) -> SyntaxToken {
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

fn find_code_block_or_expression(
    element: &syntax_nodes::Element,
    offset: u32,
) -> Option<DefinitionInformation> {
    let mut selection_range = None;
    let mut code_block_or_expression = None;
    let mut property_definition_range = None;

    if let Some(token) = element.token_at_offset(offset.into()).right_biased() {
        for ancestor in token.parent_ancestors() {
            if ancestor.kind() == SyntaxKind::BindingExpression {
                // The BindingExpression contains leading and trailing whitespace + `;`
                if let Some(child) = ancestor.first_child() {
                    code_block_or_expression = CodeBlockOrExpression::new(child);
                }
                continue;
            }
            if (ancestor.kind() == SyntaxKind::Binding)
                || (ancestor.kind() == SyntaxKind::PropertyDeclaration)
            {
                property_definition_range = Some(ancestor.text_range());
                selection_range = Some(TextRange::new(
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
        property_definition_range: property_definition_range?,
        selection_range: selection_range?,
        code_block_or_expression: code_block_or_expression?,
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

#[derive(Debug)]
pub enum LayoutKind {
    None,
    HorizontalBox,
    VerticalBox,
    GridLayout,
}

fn insert_property_definitions(
    element: &common::ElementRcNode,
    mut properties: Vec<PropertyInformation>,
) -> Vec<PropertyInformation> {
    fn binding_value(element: &ElementRc, prop: &str, count: &mut usize) -> Expression {
        // prevent infinite recursion while visiting the two-way bindings
        *count += 1;
        if *count > 10 {
            return Expression::Invalid;
        }

        if let Some(binding) = element.borrow().bindings.get(prop) {
            let e = binding.borrow().expression.clone();
            if !matches!(e, Expression::Invalid) {
                return e;
            }
            for nr in &binding.borrow().two_way_bindings {
                let e = binding_value(&nr.element(), nr.name(), count);
                if !matches!(e, Expression::Invalid) {
                    return e;
                }
            }
        }
        match &element.borrow().base_type {
            ElementType::Component(c) => binding_value(&c.root_element, prop, &mut 0),
            ElementType::Builtin(b) => b
                .properties
                .get(prop)
                .and_then(|p| p.default_value.expr(element))
                .unwrap_or_default(),
            _ => Expression::Invalid,
        }
    }

    for prop_info in properties.iter_mut() {
        if let Some(offset) = find_property_binding_offset(element, prop_info.name.as_str()) {
            prop_info.defined_at =
                element.with_element_node(|node| find_code_block_or_expression(node, offset));
        }
        let def_val = binding_value(&element.element, &prop_info.name, &mut 0);
        if !matches!(def_val, Expression::Invalid) {
            prop_info.default_value = Some(def_val);
        }
    }
    properties
}

pub(super) fn get_properties(
    element: &common::ElementRcNode,
    in_layout: LayoutKind,
) -> Vec<PropertyInformation> {
    let mut result = Vec::new();
    add_element_properties(&element.element.borrow(), "", 0, true, &mut result);

    let mut current_element = element.element.clone();
    let mut depth = 0u32;

    let geometry_prop = HashSet::from(["x", "y", "width", "height"]);

    loop {
        depth += 10;
        let base_type = current_element.borrow().base_type.clone();
        match base_type {
            ElementType::Component(c) => {
                current_element = c.root_element.clone();
                add_element_properties(&current_element.borrow(), &c.id, depth, false, &mut result);
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

                    let mut priority = DEFAULT_PRIORITY;

                    if b.name == "Text" && k == "text" {
                        priority = HIGH_PRIORITY;
                    }
                    if b.name == "TextInput"
                        && [SmolStr::new_static("text"), SmolStr::new_static("placeholder")]
                            .contains(k)
                    {
                        priority = HIGH_PRIORITY;
                    }
                    if b.name == "Image" && k == "source" {
                        priority = HIGH_PRIORITY;
                    }

                    Some(PropertyInformation {
                        name: k.clone(),
                        priority,
                        ty: t.ty.clone(),
                        declared_at: None,
                        defined_at: None,
                        default_value: t.default_value.expr(&current_element),
                        group: b.name.clone(),
                        group_priority: depth,
                    })
                }));

                if b.name == "Rectangle" {
                    result.push(PropertyInformation {
                        name: "clip".into(),
                        priority: DEFAULT_PRIORITY,
                        ty: Type::Bool,
                        declared_at: None,
                        defined_at: None,
                        default_value: Some(Expression::BoolLiteral(false)),
                        group: b.name.clone(),
                        group_priority: depth,
                    });

                    result.extend(get_reserved_properties(
                        &b.name,
                        depth,
                        i_slint_compiler::typeregister::RESERVED_DROP_SHADOW_PROPERTIES
                            .iter()
                            .cloned(),
                    ));
                }

                result.push(PropertyInformation {
                    name: "opacity".into(),
                    priority: DEFAULT_PRIORITY,
                    ty: Type::Float32,
                    declared_at: None,
                    defined_at: None,
                    default_value: Some(Expression::NumberLiteral(1.0, Unit::None)),
                    group: b.name.clone(),
                    group_priority: depth,
                });
                result.push(PropertyInformation {
                    name: "visible".into(),
                    priority: DEFAULT_PRIORITY,
                    ty: Type::Bool,
                    declared_at: None,
                    defined_at: None,
                    default_value: Some(Expression::BoolLiteral(true)),
                    group: b.name.clone(),
                    group_priority: depth,
                });

                if b.name == "Image" {
                    result.extend(get_reserved_properties(
                        &b.name,
                        depth,
                        i_slint_compiler::typeregister::RESERVED_ROTATION_PROPERTIES
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

        result.extend(
            get_reserved_properties(
                "geometry",
                depth + 1000,
                i_slint_compiler::typeregister::RESERVED_GEOMETRY_PROPERTIES.iter().cloned(),
            )
            .filter(|p| match in_layout {
                LayoutKind::None => true,
                LayoutKind::HorizontalBox => p.name.as_str() != "x",
                LayoutKind::VerticalBox => p.name.as_str() != "y",
                LayoutKind::GridLayout => !["x", "y"].contains(&p.name.as_str()),
            })
            .map(|mut p| {
                match p.name.as_str() {
                    "x" => p.priority = 200,
                    "y" => p.priority = 300,
                    "width" => p.priority = 400,
                    "height" => p.priority = 500,
                    _ => { /* do nothing */ }
                }
                p
            }),
        );
        result.extend(
            get_reserved_properties(
                "layout",
                depth + 2000,
                i_slint_compiler::typeregister::RESERVED_LAYOUT_PROPERTIES.iter().cloned(),
            )
            // padding arbitrary items is not yet implemented
            .filter(|x| !x.name.starts_with("padding"))
            .map(|mut p| {
                match p.name.as_str() {
                    "min-width" => p.priority = 200,
                    "min-height" => p.priority = 250,
                    "preferred-width" => p.priority = 300,
                    "preferred-height" => p.priority = 350,
                    "max-width" => p.priority = 400,
                    "max-height" => p.priority = 450,
                    "horizontal-stretch" => p.priority = 500,
                    "vertical-stretch" => p.priority = 550,
                    _ => { /* do nothing */ }
                }
                p
            }),
        );
        if matches!(in_layout, LayoutKind::GridLayout) {
            result.extend(get_reserved_properties(
                "layout",
                depth + 2000,
                i_slint_compiler::typeregister::RESERVED_GRIDLAYOUT_PROPERTIES.iter().cloned(),
            ));
        }
        result.push(PropertyInformation {
            name: "accessible-role".into(),
            priority: DEFAULT_PRIORITY - 100,
            ty: Type::Enumeration(
                i_slint_compiler::typeregister::BUILTIN.with(|e| e.enums.AccessibleRole.clone()),
            ),
            declared_at: None,
            defined_at: None,
            default_value: None,
            group: "accessibility".into(),
            group_priority: depth + 10000,
        });
        if current_element.borrow().is_binding_set("accessible-role", true) {
            result.extend(get_reserved_properties(
                "accessibility",
                depth + 10000,
                i_slint_compiler::typeregister::reserved_accessibility_properties(),
            ));
        }
        break;
    }

    result.sort_by_key(|p| p.name.clone());

    insert_property_definitions(element, result)
}

fn find_block_range(element: &common::ElementRcNode) -> Option<TextRange> {
    element.with_element_node(|node| {
        let open_brace = node.child_token(SyntaxKind::LBrace)?;
        let close_brace = node.child_token(SyntaxKind::RBrace)?;

        Some(TextRange::new(open_brace.text_range().start(), close_brace.text_range().end()))
    })
}

fn get_element_information(element: &common::ElementRcNode) -> ElementInformation {
    let range = element.with_decorated_node(|node| util::node_range_without_trailing_ws(&node));
    let e = element.element.borrow();
    let type_name = if matches!(&e.base_type, ElementType::Builtin(b) if b.name == "Empty") {
        SmolStr::default()
    } else {
        e.base_type.to_smolstr()
    };
    ElementInformation { id: e.id.clone(), type_name, range }
}

pub(crate) fn query_properties(
    uri: &Url,
    source_version: SourceFileVersion,
    element: &common::ElementRcNode,
    in_layout: LayoutKind,
) -> Result<QueryPropertyResponse> {
    Ok(QueryPropertyResponse {
        properties: get_properties(element, in_layout),
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
    uri: Url,
    version: SourceFileVersion,
    property: &PropertyInformation,
    new_expression: String,
) -> Option<lsp_types::TextDocumentEdit> {
    property.defined_at.as_ref().map(|defined_at| {
        let range = util::node_to_lsp_range(&defined_at.code_block_or_expression);
        let edit = lsp_types::TextEdit { range, new_text: new_expression };
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
) -> Option<(TextRange, InsertPosition)> {
    let mut previous_property = None;
    let mut property_index = usize::MAX;

    for (i, p) in properties.iter().enumerate() {
        if p.name == property_name {
            property_index = i;
        } else if let Some(defined_at) = &p.defined_at {
            if property_index == usize::MAX {
                previous_property = Some((i, defined_at.selection_range.end()));
            } else {
                if let Some((pi, _)) = previous_property {
                    if (i - property_index) >= (property_index - pi) {
                        break;
                    }
                }
                let p = defined_at.selection_range.start();
                return Some((TextRange::new(p, p), InsertPosition::Before));
            }
        }
    }

    previous_property.map(|(_, pp)| (TextRange::new(pp, pp), InsertPosition::After))
}

fn find_insert_range_for_property(
    block_range: &Option<TextRange>,
    properties: &[PropertyInformation],
    property_name: &str,
) -> Option<(TextRange, InsertPosition)> {
    find_insert_position_relative_to_defined_properties(properties, property_name).or_else(|| {
        // No properties defined yet:
        block_range.map(|r| {
            // Right after the leading `{`...
            let pos = r.start().checked_add(1.into()).unwrap_or(r.start());
            (TextRange::new(pos, pos), InsertPosition::After)
        })
    })
}

fn create_text_document_edit_for_set_binding_on_known_property(
    uri: Url,
    version: SourceFileVersion,
    element: &common::ElementRcNode,
    properties: &[PropertyInformation],
    property_name: &str,
    new_expression: &str,
) -> Option<lsp_types::TextDocumentEdit> {
    let block_range = find_block_range(element);

    find_insert_range_for_property(&block_range, properties, property_name).map(
        |(range, insert_type)| {
            let source_file = element.with_element_node(|n| n.source_file.clone());
            let indent = util::find_element_indent(element).unwrap_or_default();
            let edit = lsp_types::TextEdit {
                range: util::text_range_to_lsp_range(&source_file, range),
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
    uri: Url,
    version: SourceFileVersion,
    element: &common::ElementRcNode,
    property_name: &str,
    new_expression: String,
) -> Option<lsp_types::WorkspaceEdit> {
    set_binding_impl(uri, version, element, property_name, new_expression)
        .map(|edit| common::create_workspace_edit_from_text_document_edits(vec![edit]))
}

pub fn set_binding_impl(
    uri: Url,
    version: SourceFileVersion,
    element: &common::ElementRcNode,
    property_name: &str,
    new_expression: String,
) -> Option<lsp_types::TextDocumentEdit> {
    let properties = get_properties(element, LayoutKind::None);
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
    uri: Url,
    version: SourceFileVersion,
    element: &common::ElementRcNode,
    properties: &[crate::common::PropertyChange],
) -> Option<lsp_types::WorkspaceEdit> {
    let edits = properties
        .iter()
        .filter_map(|p| set_binding_impl(uri.clone(), version, element, &p.name, p.value.clone()))
        .collect::<Vec<_>>();

    (edits.len() == properties.len())
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
    let element_position = util::text_size_to_lsp_position(&source_file, position.offset());

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
    uri: Url,
    version: SourceFileVersion,
    range: lsp_types::Range,
) -> lsp_types::WorkspaceEdit {
    let edit = lsp_types::TextEdit { range, new_text: String::new() };
    common::create_workspace_edit(uri.clone(), version, vec![edit])
}

pub fn remove_binding(
    uri: Url,
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

                    return Some(util::text_range_to_lsp_range(
                        &source_file,
                        TextRange::new(start, end),
                    ));
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
pub mod tests {
    use super::*;

    use crate::language::test::{complex_document_cache, loaded_document_cache};

    fn find_property<'a>(
        properties: &'a [PropertyInformation],
        name: &'_ str,
    ) -> Option<&'a PropertyInformation> {
        properties.iter().find(|p| p.name == name)
    }

    pub fn properties_at_position_in_cache(
        line: u32,
        character: u32,
        document_cache: &common::DocumentCache,
        url: &lsp_types::Url,
    ) -> Option<(common::ElementRcNode, Vec<PropertyInformation>)> {
        let element =
            document_cache.element_at_position(url, &lsp_types::Position { line, character })?;
        Some((element.clone(), get_properties(&element, LayoutKind::None)))
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
        assert_eq!(&find_property(&result, "elapsed-time").unwrap().ty, &Type::Duration);
        // Property of base type:
        assert_eq!(&find_property(&result, "no-frame").unwrap().ty, &Type::Bool);
        // reserved properties:
        assert_eq!(
            &find_property(&result, "accessible-role").unwrap().ty.to_string(),
            "enum AccessibleRole"
        );
        // Accessible property should not be present since the role is none
        assert!(find_property(&result, "accessible-label").is_none());
        assert!(find_property(&result, "accessible-action-default").is_none());

        // Poke deeper:
        let (_, result, _, _) = properties_at_position(21, 30).unwrap();
        let property = find_property(&result, "background").unwrap();

        let def_at = property.defined_at.as_ref().unwrap();
        let def_range = util::node_to_lsp_range(&def_at.code_block_or_expression);
        assert_eq!(def_range.end.line, def_range.start.line);
        // -1 because the lsp range end location is exclusive.
        assert_eq!(
            (def_range.end.character - def_range.start.character) as usize,
            "lightblue".len()
        );

        // On a Button
        let (_, result, _, _) = properties_at_position(48, 4).unwrap();

        assert_eq!(&find_property(&result, "text").unwrap().ty, &Type::String);
        // Accessible property should not be present since the role is button
        assert_eq!(find_property(&result, "accessible-label").unwrap().ty, Type::String);
        // No callbacks
        assert!(find_property(&result, "accessible-action-default").is_none());
        assert!(find_property(&result, "clicked").is_none());
    }

    #[test]
    fn test_element_information() {
        let (document_cache, url, _) = complex_document_cache();
        let element =
            document_cache.element_at_position(&url, &lsp_types::Position::new(33, 4)).unwrap();

        let result = get_element_information(&element);

        let r = util::text_range_to_lsp_range(
            &element.with_element_node(|n| n.source_file.clone()),
            result.range,
        );
        assert_eq!(r.start.line, 32);
        assert_eq!(r.start.character, 12);
        assert_eq!(r.end.line, 35);
        assert_eq!(r.end.character, 13);

        assert_eq!(result.type_name.to_string(), "Text");
    }

    #[test]
    fn test_element_information_empty() {
        let (document_cache, url, _) = loaded_document_cache(
            "component FooBar { property <int> foo; btn := Button {} }".into(),
        );
        let element =
            document_cache.element_at_position(&url, &lsp_types::Position::new(1, 19)).unwrap();
        let result = get_element_information(&element);
        assert_eq!(result.type_name.to_string(), "");
        assert_eq!(result.id, "root");

        let element =
            document_cache.element_at_position(&url, &lsp_types::Position::new(1, 39)).unwrap();
        let result = get_element_information(&element);
        // Because `Button` is not defined in this scope
        assert_eq!(result.type_name.to_string(), "<error>");
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
        let source_file = dc.get_document(&url).unwrap().node.as_ref().unwrap().source_file.clone();

        let (_, result) = properties_at_position_in_cache(pos_l, pos_c, &dc, &url).unwrap();

        let p = find_property(&result, "text").unwrap();
        let definition = p.defined_at.as_ref().unwrap();

        assert_eq!(&definition.code_block_or_expression.text(), "\"text\"");

        let sel_range = util::text_range_to_lsp_range(&source_file, definition.selection_range);
        println!("Actual: (l: {}, c: {}) - (l: {}, c: {}) --- Expected: (l: {sl}, c: {sc}) - (l: {el}, c: {ec})",
            sel_range.start.line,
            sel_range.start.character,
            sel_range.end.line,
            sel_range.end.character,
        );

        assert_eq!(sel_range.start.line, sl);
        assert_eq!(sel_range.start.character, sc);
        assert_eq!(sel_range.end.line, el);
        assert_eq!(sel_range.end.character, ec);
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

        let doc = dc.get_document(&url).unwrap();
        let source = &doc.node.as_ref().unwrap().source_file;
        let (l, c) = source.line_column(source.source().unwrap().find("base2 :=").unwrap());
        let (_, result) = properties_at_position_in_cache(l as u32, c as u32, &dc, &url).unwrap();

        let foo_property = find_property(&result, "foo").unwrap();

        assert_eq!(foo_property.ty, Type::Int32);

        let declaration = foo_property.declared_at.as_ref().unwrap();
        let start_position = util::text_size_to_lsp_position(source, declaration.start_position);
        assert_eq!(declaration.path, source.path());
        assert_eq!(start_position.line, 3);
        assert_eq!(start_position.character, 20); // This should probably point to the start of
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

        let (element_node, result) = properties_at_position_in_cache(1, 25, &dc, &url).unwrap();
        let source = element_node.with_element_node(|n| n.source_file.clone());

        let glob_property = find_property(&result, "glob").unwrap();
        assert_eq!(glob_property.ty, Type::Int32);
        let declaration = glob_property.declared_at.as_ref().unwrap();
        let start_position = util::text_size_to_lsp_position(&source, declaration.start_position);
        assert_eq!(declaration.path, source.path());
        assert_eq!(start_position.line, 2);
        assert_eq!(glob_property.group, "");
        assert!(find_property(&result, "width").is_none());

        let (_, result) = properties_at_position_in_cache(8, 4, &dc, &url).unwrap();
        let abcd_property = find_property(&result, "abcd").unwrap();
        assert_eq!(abcd_property.ty, Type::Int32);
        let declaration = abcd_property.declared_at.as_ref().unwrap();
        let start_position = util::text_size_to_lsp_position(&source, declaration.start_position);
        assert_eq!(declaration.path, source.path());
        assert_eq!(start_position.line, 7);
        assert_eq!(abcd_property.group, "");

        let x_property = find_property(&result, "x").unwrap();
        assert_eq!(x_property.ty, Type::LogicalLength);
        assert!(x_property.defined_at.is_none());
        assert_eq!(x_property.group, "geometry");

        let width_property = find_property(&result, "width").unwrap();
        assert_eq!(width_property.ty, Type::LogicalLength);
        let definition = width_property.defined_at.as_ref().unwrap();
        let expression_range = util::node_to_lsp_range(&definition.code_block_or_expression);
        assert_eq!(expression_range.start.line, 8);
        assert_eq!(width_property.group, "geometry");
    }

    #[test]
    fn test_invalid_property_panic() {
        let (dc, url, _) =
            loaded_document_cache(r#"export component Demo { Text { text: } }"#.to_string());

        let (_, result) = properties_at_position_in_cache(0, 35, &dc, &url).unwrap();

        let prop = find_property(&result, "text").unwrap();
        assert!(prop.defined_at.is_none()); // The property has no valid definition at this time
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
        assert_eq!(find_property(&result, "a1").unwrap().ty, Type::Int32);
        assert_eq!(
            find_property(&result, "a1")
                .unwrap()
                .defined_at
                .as_ref()
                .unwrap()
                .code_block_or_expression
                .text(),
            "{ 1 + 1 }"
        );
        assert_eq!(find_property(&result, "a2").unwrap().ty, Type::Int32);
        assert_eq!(
            find_property(&result, "a2")
                .unwrap()
                .defined_at
                .as_ref()
                .unwrap()
                .code_block_or_expression
                .text(),
            "{ 1 + 2; }"
        );
        assert_eq!(find_property(&result, "a3").unwrap().ty, Type::Int32);
        assert_eq!(
            find_property(&result, "a3")
                .unwrap()
                .defined_at
                .as_ref()
                .unwrap()
                .code_block_or_expression
                .text(),
            "{ 1 + 3 }"
        );
        assert_eq!(find_property(&result, "a4").unwrap().ty, Type::Int32);
        assert_eq!(
            find_property(&result, "a4")
                .unwrap()
                .defined_at
                .as_ref()
                .unwrap()
                .code_block_or_expression
                .text(),
            "{ 1 + 4; }"
        );
        assert_eq!(find_property(&result, "b").unwrap().ty, Type::Int32);
        assert_eq!(
            find_property(&result, "b")
                .unwrap()
                .defined_at
                .as_ref()
                .unwrap()
                .code_block_or_expression
                .text(),
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
        assert_eq!(find_property(&result, "a1").unwrap().ty, Type::Int32);
        assert_eq!(
            find_property(&result, "a1")
                .unwrap()
                .defined_at
                .as_ref()
                .unwrap()
                .code_block_or_expression
                .text(),
            "{ 1 + 1 }"
        );
        assert_eq!(find_property(&result, "a2").unwrap().ty, Type::Int32);
        assert_eq!(
            find_property(&result, "a2")
                .unwrap()
                .defined_at
                .as_ref()
                .unwrap()
                .code_block_or_expression
                .text(),
            "{ 1 + 2; }"
        );
        assert_eq!(find_property(&result, "a3").unwrap().ty, Type::Int32);
        assert_eq!(
            find_property(&result, "a3")
                .unwrap()
                .defined_at
                .as_ref()
                .unwrap()
                .code_block_or_expression
                .text(),
            "{ 1 + 3 }"
        );
        assert_eq!(find_property(&result, "a4").unwrap().ty, Type::Int32);
        assert_eq!(
            find_property(&result, "a4")
                .unwrap()
                .defined_at
                .as_ref()
                .unwrap()
                .code_block_or_expression
                .text(),
            "{ 1 + 4; }"
        );
        assert_eq!(find_property(&result, "b").unwrap().ty, Type::Int32);
        assert_eq!(
            find_property(&result, "b")
                .unwrap()
                .defined_at
                .as_ref()
                .unwrap()
                .code_block_or_expression
                .text(),
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
        assert_eq!(find_property(&result, "a").unwrap().ty, Type::Int32);
        assert_eq!(find_property(&result, "b").unwrap().ty, Type::Int32);
        assert_eq!(find_property(&result, "c").unwrap().ty, Type::Int32);
        assert_eq!(find_property(&result, "d").unwrap().ty, Type::Int32);

        let (_, result) = properties_at_position_in_cache(10, 0, &dc, &url).unwrap();
        assert!(find_property(&result, "a").is_none());
        assert_eq!(find_property(&result, "b").unwrap().ty, Type::Int32);
        assert!(find_property(&result, "c").is_none());
        assert_eq!(find_property(&result, "d").unwrap().ty, Type::Int32);

        let (_, result) = properties_at_position_in_cache(13, 0, &dc, &url).unwrap();
        assert_eq!(find_property(&result, "enabled").unwrap().ty, Type::Bool);
        assert!(find_property(&result, "pressed").is_none());
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
        assert_eq!(&tc.new_text, "\n                x: 30px;");
        assert_eq!(tc.range.start, lsp_types::Position { line: 17, character: 33 });
        assert_eq!(tc.range.end, lsp_types::Position { line: 17, character: 33 });
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
