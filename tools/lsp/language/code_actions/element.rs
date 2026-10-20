// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

// cSpell: ignore unindented

use super::Context;
use crate::util;

use i_slint_compiler::parser::{NodeOrToken, SyntaxKind, SyntaxNode};
use lsp_types::{CodeActionKind, CodeActionOrCommand, Range, TextEdit};

pub(super) fn add(ctx: &Context, result: &mut Vec<CodeActionOrCommand>) {
    if !ctx.snippets
        || ctx.token.kind() != SyntaxKind::Identifier
        || ctx.node.kind() != SyntaxKind::QualifiedName
    {
        return;
    }
    let Some(element) = ctx.node.parent().filter(|n| n.kind() == SyntaxKind::Element) else {
        return;
    };
    let range = util::text_range_to_lsp_range(
        &ctx.token.source_file,
        element.text_range(),
        ctx.document_cache.format,
    );
    add_wrap(ctx, &element, range, result);
    add_remove(ctx, &element, range, result);
    add_repeat_and_conditional(ctx, &element, range, result);
}

fn add_wrap(
    ctx: &Context,
    element: &SyntaxNode,
    range: Range,
    result: &mut Vec<CodeActionOrCommand>,
) {
    let element_indent = ctx
        .document_cache
        .element_at_position(&ctx.uri, &range.start)
        .as_ref()
        .and_then(util::find_element_indent);
    let indented_lines = element
        .text()
        .to_string()
        .lines()
        .map(|line| if line.is_empty() { line.to_string() } else { format!("    {line}") })
        .collect::<Vec<String>>();
    let edits = vec![TextEdit::new(
        range,
        format!(
            "${{0:element}} {{\n{}{}\n}}",
            element_indent.unwrap_or("".into()),
            indented_lines.join("\n")
        ),
    )];
    ctx.push_edit(result, "Wrap in element", Some(CodeActionKind::REFACTOR), edits);
}

fn add_remove(
    ctx: &Context,
    element: &SyntaxNode,
    range: Range,
    result: &mut Vec<CodeActionOrCommand>,
) {
    fn is_sub_element(kind: SyntaxKind) -> bool {
        matches!(
            kind,
            SyntaxKind::SubElement | SyntaxKind::RepeatedElement | SyntaxKind::ConditionalElement
        )
    }
    fn is_sub_element_without_id(node: &NodeOrToken) -> bool {
        node.kind() == SyntaxKind::SubElement
            && node.as_node().and_then(|n| n.first_child_or_token()).map(|n| n.kind())
                == Some(SyntaxKind::Element)
    }
    let sub_elements = element
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

    let can_remove = if ctx.component.is_some() {
        matches!(sub_elements.as_slice(), [sub_element] if is_sub_element_without_id(sub_element))
    } else {
        sub_elements.iter().any(|n| n.kind() == SyntaxKind::SubElement)
    };
    if !can_remove {
        return;
    }
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
            NodeOrToken::Token(t) => t.text().strip_suffix("    ").unwrap_or(t.text()).to_string(),
        })
        .collect::<Vec<String>>();
    let edits = vec![TextEdit::new(range, unindented_lines.concat())];
    ctx.push_edit(result, "Remove element", Some(CodeActionKind::REFACTOR), edits);
}

fn add_repeat_and_conditional(
    ctx: &Context,
    element: &SyntaxNode,
    range: Range,
    result: &mut Vec<CodeActionOrCommand>,
) {
    let is_child_of_element = element
        .parent()
        .filter(|n| n.kind() == SyntaxKind::SubElement)
        .and_then(|n| n.parent())
        .is_some_and(|n| n.kind() == SyntaxKind::Element);
    if !is_child_of_element {
        return;
    }
    let insertion = Range::new(range.start, range.start);
    let edits = vec![TextEdit::new(insertion, "for ${1:name}[index] in ${0:model} : ".to_string())];
    ctx.push_edit(result, "Repeat element", Some(CodeActionKind::REFACTOR), edits);

    let edits = vec![TextEdit::new(insertion, "if ${0:condition} : ".to_string())];
    ctx.push_edit(result, "Make conditional", Some(CodeActionKind::REFACTOR), edits);
}

#[cfg(test)]
mod tests {
    use crate::language::code_actions::test::CodeActionRequest;

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
    fn test_code_action_no_remove_component_root_with_repeated_or_conditional_child() {
        for child in ["for item in 3 : Text { }", "if true : Text { }"] {
            let actions = CodeActionRequest::at(&format!(
                "export component Test inherits 🔺Rectangle {{\n    {child}\n}}\n"
            ))
            .snippets()
            .load();
            assert!(!actions.titles().contains(&"Remove element"), "{child}");
        }
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
}
