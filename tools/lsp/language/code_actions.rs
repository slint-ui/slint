// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

// cSpell: ignore rfind unindented

mod import;
#[cfg(any(feature = "preview-builtin", feature = "preview-external"))]
mod show_preview;
#[cfg(test)]
pub mod test;
mod translate;

use super::completion;
use crate::editor_preview::{self, DocumentCache};
use crate::util;

use i_slint_compiler::object_tree::QualifiedTypeName;
use i_slint_compiler::parser::{NodeOrToken, SyntaxKind, SyntaxNode, SyntaxToken, syntax_nodes};
use lsp_types::{ClientCapabilities, CodeActionKind, CodeActionOrCommand, TextEdit, Url};

struct Context<'a> {
    document_cache: &'a DocumentCache,
    token: SyntaxToken,
    node: SyntaxNode,
    uri: Url,
    component: Option<syntax_nodes::Component>,
    snippets: bool,
}

impl<'a> Context<'a> {
    fn new(
        document_cache: &'a DocumentCache,
        token: SyntaxToken,
        client_capabilities: &ClientCapabilities,
    ) -> Option<Self> {
        let node = token.parent();
        let uri = token.source_file.path().to_url()?;
        let component = syntax_nodes::Component::new(node.clone())
            .or_else(|| {
                syntax_nodes::DeclaredIdentifier::new(node.clone())
                    .and_then(|n| n.parent())
                    .and_then(syntax_nodes::Component::new)
            })
            .or_else(|| {
                syntax_nodes::QualifiedName::new(node.clone())
                    .and_then(|n| n.parent())
                    .and_then(syntax_nodes::Element::new)
                    .and_then(|n| n.parent())
                    .and_then(syntax_nodes::Component::new)
            })
            .or_else(|| syntax_nodes::ExportsList::new(node.clone()).and_then(|n| n.Component()))
            .filter(|c| c.child_text(SyntaxKind::Identifier).is_none_or(|t| t != "global"));
        let snippets = has_experimental_client_capability(client_capabilities, "snippetTextEdit");
        Some(Self { document_cache, token, node, uri, component, snippets })
    }

    fn push_edit(
        &self,
        result: &mut Vec<CodeActionOrCommand>,
        title: impl Into<String>,
        kind: Option<CodeActionKind>,
        edits: Vec<TextEdit>,
    ) {
        result.push(CodeActionOrCommand::CodeAction(lsp_types::CodeAction {
            title: title.into(),
            kind,
            edit: editor_preview::editing::create_workspace_edit_from_path(
                self.document_cache,
                self.token.source_file.path(),
                edits,
            ),
            ..Default::default()
        }));
    }
}

fn has_experimental_client_capability(capabilities: &ClientCapabilities, name: &str) -> bool {
    capabilities
        .experimental
        .as_ref()
        .and_then(|o| o.get(name).and_then(|v| v.as_bool()))
        .unwrap_or(false)
}

pub fn get_code_actions(
    document_cache: &DocumentCache,
    token: SyntaxToken,
    client_capabilities: &ClientCapabilities,
) -> Option<Vec<CodeActionOrCommand>> {
    let ctx = Context::new(document_cache, token, client_capabilities)?;
    let token = &ctx.token;
    let node = &ctx.node;
    let mut result = Vec::new();

    #[cfg(any(feature = "preview-builtin", feature = "preview-external"))]
    show_preview::add(&ctx, &mut result);

    translate::add(&ctx, &mut result);
    import::add(&ctx, &mut result);

    if token.kind() == SyntaxKind::Identifier
        && node.kind() == SyntaxKind::QualifiedName
        && node.parent().map(|n| n.kind()) == Some(SyntaxKind::Element)
    {
        if ctx.snippets {
            let r = util::text_range_to_lsp_range(
                &token.source_file,
                node.parent().unwrap().text_range(),
                document_cache.format,
            );
            let element = document_cache.element_at_position(&ctx.uri, &r.start);
            let element_indent = element.as_ref().and_then(util::find_element_indent);
            let indented_lines = node
                .parent()
                .unwrap()
                .text()
                .to_string()
                .lines()
                .map(|line| if line.is_empty() { line.to_string() } else { format!("    {line}") })
                .collect::<Vec<String>>();
            let edits = vec![TextEdit::new(
                lsp_types::Range::new(r.start, r.end),
                format!(
                    "${{0:element}} {{\n{}{}\n}}",
                    element_indent.unwrap_or("".into()),
                    indented_lines.join("\n")
                ),
            )];
            ctx.push_edit(&mut result, "Wrap in element", Some(CodeActionKind::REFACTOR), edits);

            // Collect all normal, repeated, and conditional sub-elements and any
            // whitespace in between for substituting the parent element with its
            // sub-elements, dropping its own properties, callbacks etc.
            fn is_sub_element(kind: SyntaxKind) -> bool {
                matches!(
                    kind,
                    SyntaxKind::SubElement
                        | SyntaxKind::RepeatedElement
                        | SyntaxKind::ConditionalElement
                )
            }
            let sub_elements = node
                .parent()
                .unwrap()
                .children_with_tokens()
                .skip_while(|n| !is_sub_element(n.kind()))
                .filter(|n| match n {
                    NodeOrToken::Node(_) => is_sub_element(n.kind()),
                    NodeOrToken::Token(t) => {
                        t.kind() == SyntaxKind::Whitespace
                            && t.next_sibling_or_token().is_some_and(|n| is_sub_element(n.kind()))
                    }
                })
                .collect::<Vec<_>>();

            if match ctx.component {
                // A top-level component element can only be removed if it contains
                // exactly one sub-element (without any condition or assignment)
                // that can substitute the component element.
                Some(_) => {
                    sub_elements.len() == 1
                        && sub_elements.first().and_then(|n| {
                            n.as_node().unwrap().first_child_or_token().map(|n| n.kind())
                        }) == Some(SyntaxKind::Element)
                }
                // Any other element can be removed in favor of one or more sub-elements.
                None => sub_elements.iter().any(|n| n.kind() == SyntaxKind::SubElement),
            } {
                let unindented_lines = sub_elements
                    .iter()
                    .map(|n| match n {
                        NodeOrToken::Node(n) => n
                            .text()
                            .to_string()
                            .lines()
                            .map(|line| line.strip_prefix("    ").unwrap_or(line).to_string())
                            .collect::<Vec<_>>()
                            .join("\n"),
                        NodeOrToken::Token(t) => {
                            t.text().strip_suffix("    ").unwrap_or(t.text()).to_string()
                        }
                    })
                    .collect::<Vec<String>>();
                let edits = vec![TextEdit::new(
                    lsp_types::Range::new(r.start, r.end),
                    unindented_lines.concat(),
                )];
                ctx.push_edit(&mut result, "Remove element", Some(CodeActionKind::REFACTOR), edits);
            }

            // We have already checked that the node is a qualified name of an element.
            // Check whether the element is a direct sub-element of another element
            // meaning that it can be repeated or made conditional.
            if node // QualifiedName
                .parent() // Element
                .unwrap()
                .parent()
                .filter(|n| n.kind() == SyntaxKind::SubElement)
                .and_then(|p| p.parent())
                .is_some_and(|n| n.kind() == SyntaxKind::Element)
            {
                let edits = vec![TextEdit::new(
                    lsp_types::Range::new(r.start, r.start),
                    "for ${1:name}[index] in ${0:model} : ".to_string(),
                )];
                ctx.push_edit(&mut result, "Repeat element", Some(CodeActionKind::REFACTOR), edits);

                let edits = vec![TextEdit::new(
                    lsp_types::Range::new(r.start, r.start),
                    "if ${0:condition} : ".to_string(),
                )];
                ctx.push_edit(
                    &mut result,
                    "Make conditional",
                    Some(CodeActionKind::REFACTOR),
                    edits,
                );
            }
        }
    } else if token.kind() == SyntaxKind::Identifier
        && node.kind() == SyntaxKind::QualifiedName
        && node.parent().map(|n| n.kind()) == Some(SyntaxKind::Expression)
        && node.children_with_tokens().filter(|n| n.kind() == SyntaxKind::Identifier).count() == 1
    {
        // Qualify a bare identifier that doesn't resolve but is an enum value or named color
        // (`red` -> `Colors.red`). Like the import action above, re-derive the lookup error rather
        // than reading diagnostics, so nothing is offered when the identifier does resolve.
        use i_slint_compiler::lookup::LookupObject;
        let suggestions = util::with_lookup_ctx(document_cache, node.clone(), None, |ctx| {
            let name = i_slint_compiler::parser::normalize_identifier(token.text());
            if i_slint_compiler::lookup::global_lookup().lookup(ctx, &name).is_none() {
                i_slint_compiler::lookup::enum_or_color_suggestions(ctx, token.text())
            } else {
                Vec::new()
            }
        })
        .unwrap_or_default();
        if !suggestions.is_empty() {
            let range = util::text_range_to_lsp_range(
                &token.source_file,
                token.text_range(),
                document_cache.format,
            );
            for suggestion in suggestions {
                ctx.push_edit(
                    &mut result,
                    format!("Qualify as '{suggestion}'"),
                    Some(CodeActionKind::QUICKFIX),
                    vec![TextEdit::new(range, suggestion.to_string())],
                );
            }
        }
    } else if token.kind() == SyntaxKind::Identifier
        && let Some(implement_statement) = match node.kind() {
            SyntaxKind::ImplementStatement => syntax_nodes::ImplementStatement::new(node.clone()),
            SyntaxKind::QualifiedName | SyntaxKind::DeclaredIdentifier
                if node.parent().map(|n| n.kind()) == Some(SyntaxKind::ImplementStatement) =>
            {
                node.parent().and_then(syntax_nodes::ImplementStatement::new)
            }
            _ => None,
        }
    {
        let r = util::text_range_to_lsp_range(
            &token.source_file,
            implement_statement.text_range(),
            document_cache.format,
        );
        if let Some(element) = document_cache.element_at_position(&ctx.uri, &r.start) {
            let missing =
                i_slint_compiler::object_tree::missing_interface_members(&element.element);
            if let Some(missing_entry) = missing
                .into_iter()
                .find(|entry| entry.statement.text_range() == implement_statement.text_range())
            {
                let indent = util::find_indent(&implement_statement).unwrap_or_default();
                let insertion: String = missing_entry
                    .declarations
                    .iter()
                    .map(|decl| format!("\n{indent}{decl}"))
                    .collect();
                let insert_pos = util::text_size_to_lsp_position(
                    &token.source_file,
                    implement_statement.text_range().end(),
                    document_cache.format,
                );
                let interface_name =
                    QualifiedTypeName::from_node(implement_statement.QualifiedName());
                ctx.push_edit(
                    &mut result,
                    format!("Add the missing declarations for '{interface_name}'"),
                    Some(CodeActionKind::QUICKFIX),
                    vec![TextEdit::new(lsp_types::Range::new(insert_pos, insert_pos), insertion)],
                );
            }
        }
    }

    completion::match_element::add_code_actions(document_cache, token, &mut result);

    (!result.is_empty()).then_some(result)
}

#[cfg(test)]
mod tests {
    use super::test::CodeActionRequest;

    const SUB_ELEMENT: &str = r#"export component Test {
    Rectangle {
        Text {
            text: "Hello";
        }
    }
}
"#;

    const CONDITIONAL_ELEMENT: &str = r#"export component Test {
    if true: Rectangle {
        background: red;

        Text { text: "A"; }

        Text {
            text: "B";
        }
    }
}
"#;

    #[test]
    fn test_code_action_element_needs_snippet_support() {
        for column in 0..=4 {
            CodeActionRequest::at_offset(SUB_ELEMENT, SUB_ELEMENT.find("Text").unwrap() + column)
                .load()
                .assert_empty();
        }
        CodeActionRequest::at_offset(
            CONDITIONAL_ELEMENT,
            CONDITIONAL_ELEMENT.find("Rectangle").unwrap(),
        )
        .load()
        .assert_empty();
    }

    #[test]
    fn test_code_action_sub_element() {
        for column in 0..=4 {
            let actions = CodeActionRequest::at_offset(
                SUB_ELEMENT,
                SUB_ELEMENT.find("Text").unwrap() + column,
            )
            .snippets()
            .load();
            assert_eq!(
                actions.titles(),
                ["Wrap in element", "Repeat element", "Make conditional"],
                "column {column}"
            );
            actions
                .assert(
                    "Wrap in element",
                    Some(lsp_types::CodeActionKind::REFACTOR),
                    r#"export component Test {
    Rectangle {
        ${0:element} {
            Text {
                text: "Hello";
            }
}
    }
}
"#,
                )
                .compiles_with(&[("element", "Rectangle")]);
            actions
                .assert(
                    "Repeat element",
                    Some(lsp_types::CodeActionKind::REFACTOR),
                    r#"export component Test {
    Rectangle {
        for ${1:name}[index] in ${0:model} : Text {
            text: "Hello";
        }
    }
}
"#,
                )
                .compiles_with(&[("name", "item"), ("model", "3")]);
            actions
                .assert(
                    "Make conditional",
                    Some(lsp_types::CodeActionKind::REFACTOR),
                    r#"export component Test {
    Rectangle {
        if ${0:condition} : Text {
            text: "Hello";
        }
    }
}
"#,
                )
                .compiles_with(&[("condition", "true")]);
        }
    }

    #[test]
    fn test_code_action_remove_element_of_conditional() {
        let actions = CodeActionRequest::at_offset(
            CONDITIONAL_ELEMENT,
            CONDITIONAL_ELEMENT.find("Rectangle").unwrap(),
        )
        .snippets()
        .load();
        assert_eq!(actions.titles(), ["Wrap in element", "Remove element"]);
        actions
            .assert(
                "Wrap in element",
                Some(lsp_types::CodeActionKind::REFACTOR),
                r#"export component Test {
    if true: ${0:element} {
        Rectangle {
            background: red;

            Text { text: "A"; }

            Text {
                text: "B";
            }
        }
}
}
"#,
            )
            .compiles_with(&[("element", "Rectangle")]);
        actions
            .assert(
                "Remove element",
                Some(lsp_types::CodeActionKind::REFACTOR),
                r#"export component Test {
    if true: Text { text: "A"; }

    Text {
        text: "B";
    }
}
"#,
            )
            .compiles();
    }

    #[test]
    fn test_code_action_remove_component_root() {
        CodeActionRequest::at("export component Test inherits 🔺Rectangle {\n    Text { }\n}\n")
            .snippets()
            .load()
            .assert(
                "Remove element",
                Some(lsp_types::CodeActionKind::REFACTOR),
                "export component Test inherits Text { }\n",
            )
            .compiles();
    }

    #[test]
    fn test_code_action_remove_element_with_id() {
        CodeActionRequest::at(
            "export component Test {\n    input := 🔺Rectangle {\n        Text { }\n    }\n}\n",
        )
        .snippets()
        .load()
        .assert(
            "Remove element",
            Some(lsp_types::CodeActionKind::REFACTOR),
            "export component Test {\n    input := Text { }\n}\n",
        )
        .compiles();
    }

    #[test]
    fn test_code_action_no_show_preview_for_struct_or_global() {
        CodeActionRequest::at("export struct 🔺NoPreviewForStruct { x: int }")
            .load()
            .assert_empty();
        CodeActionRequest::at("export global 🔺NoPreviewForGlobal {}").load().assert_empty();
    }

    const SIGNATURE_INTERFACE: &str = r#"interface Signature {
    in property <bool> checkable;
    in property <string> text;
    callback clicked();
    public function add(a: int, b: int) -> int;
}

"#;

    #[test]
    fn test_implement_missing_members_inserts_all_declarations() {
        CodeActionRequest::at(&format!(
            "{SIGNATURE_INTERFACE}export component TestCase {{\n    implement 🔺Signature <=> self;\n}}\n"
        ))
        .experimental().load()
        .assert(
            "Add the missing declarations for 'Signature'",
            Some(lsp_types::CodeActionKind::QUICKFIX),
            &format!(
                r#"{SIGNATURE_INTERFACE}export component TestCase {{
    implement Signature <=> self;
    public function add(a: int, b: int) -> int {{ }}
    in property <bool> checkable;
    callback clicked();
    in property <string> text;
}}
"#
            ),
        ).skip_compile("the function body stays empty");
    }

    #[test]
    fn test_implement_missing_members_includes_inherited_members() {
        CodeActionRequest::at(
            r#"interface A {
    out property <int> speed;
}
interface B inherits A {
    out property <float> direction;
}
export component TestCase {
    implement 🔺B <=> self;
}
"#,
        )
        .experimental()
        .load()
        .assert(
            "Add the missing declarations for 'B'",
            Some(lsp_types::CodeActionKind::QUICKFIX),
            r#"interface A {
    out property <int> speed;
}
interface B inherits A {
    out property <float> direction;
}
export component TestCase {
    implement B <=> self;
    out property <float> direction;
    out property <int> speed;
}
"#,
        )
        .compiles();
    }

    #[test]
    fn test_implement_missing_members_offers_nothing_when_satisfied() {
        CodeActionRequest::at(&format!(
            r#"{SIGNATURE_INTERFACE}export component TestCase {{
    implement 🔺Signature <=> self;

    in property <bool> checkable;
    in property <string> text;
    callback clicked();
    public function add(a: int, b: int) -> int {{ 0 }}
}}
"#
        ))
        .experimental()
        .load()
        .assert_empty();
    }

    #[test]
    fn test_implement_missing_members_skips_members_with_wrong_type() {
        CodeActionRequest::at(&format!(
            "{SIGNATURE_INTERFACE}export component TestCase {{\n    implement 🔺Signature <=> self;\n\n    in property <int> checkable;\n}}\n"
        ))
        .experimental().load()
        .assert(
            "Add the missing declarations for 'Signature'",
            Some(lsp_types::CodeActionKind::QUICKFIX),
            &format!(
                r#"{SIGNATURE_INTERFACE}export component TestCase {{
    implement Signature <=> self;
    public function add(a: int, b: int) -> int {{ }}
    callback clicked();
    in property <string> text;

    in property <int> checkable;
}}
"#
            ),
        ).skip_compile("the wrongly typed member stays");
    }

    #[test]
    fn test_implement_missing_members_skips_members_with_invalid_type() {
        CodeActionRequest::at(&format!(
            "{SIGNATURE_INTERFACE}export component TestCase {{\n    implement 🔺Signature <=> self;\n\n    in property <Nonexistent> text;\n}}\n"
        ))
        .experimental().load()
        .assert(
            "Add the missing declarations for 'Signature'",
            Some(lsp_types::CodeActionKind::QUICKFIX),
            &format!(
                r#"{SIGNATURE_INTERFACE}export component TestCase {{
    implement Signature <=> self;
    public function add(a: int, b: int) -> int {{ }}
    in property <bool> checkable;
    callback clicked();

    in property <Nonexistent> text;
}}
"#
            ),
        ).skip_compile("the invalid member stays");
    }
    #[test]
    fn test_qualify_named_color() {
        CodeActionRequest::at("export component TestCase {\n    init => { debug(🔺red); }\n}\n")
            .load()
            .assert(
                "Qualify as 'Colors.red'",
                Some(lsp_types::CodeActionKind::QUICKFIX),
                "export component TestCase {\n    init => { debug(Colors.red); }\n}\n",
            )
            .compiles();
    }

    #[test]
    fn test_no_qualify_for_resolved_color() {
        CodeActionRequest::at("export component TestCase {\n    property <color> c: 🔺red;\n}\n")
            .load()
            .assert_empty();
    }

    #[test]
    fn test_qualify_value_shared_by_enums() {
        const SOURCE: &str = r#"enum EA { shared_value }
enum EB { shared_value }
export component TestCase {
    init => { debug(shared_value); }
}
"#;
        let actions =
            CodeActionRequest::at_offset(SOURCE, SOURCE.rfind("shared_value").unwrap()).load();
        assert_eq!(
            actions.titles(),
            ["Qualify as 'EA.shared-value'", "Qualify as 'EB.shared-value'"]
        );
        for qualified in ["EA.shared-value", "EB.shared-value"] {
            actions
                .assert(
                    &format!("Qualify as '{qualified}'"),
                    Some(lsp_types::CodeActionKind::QUICKFIX),
                    &SOURCE.replace("(shared_value)", &format!("({qualified})")),
                )
                .compiles();
        }
    }
}
