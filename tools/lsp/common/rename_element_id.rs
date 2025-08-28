// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

use i_slint_compiler::parser::{
    syntax_nodes, NodeOrToken, SyntaxKind, SyntaxNode, SyntaxToken, TextRange,
};
use lsp_types::TextEdit;

/// If the token is matching a Element ID, return the list of all element id in the same component
///
/// Useful for DocumentHighlightRequest as well as renaming
pub fn find_element_ids(token: &SyntaxToken, parent: &SyntaxNode) -> Option<Vec<TextRange>> {
    fn is_element_id(tk: &SyntaxToken, parent: &SyntaxNode) -> bool {
        if tk.kind() != SyntaxKind::Identifier {
            return false;
        }
        if parent.kind() == SyntaxKind::SubElement {
            return true;
        };
        if parent.kind() == SyntaxKind::QualifiedName
            && matches!(
                parent.parent().map(|n| n.kind()),
                Some(SyntaxKind::Expression | SyntaxKind::StatePropertyChange)
            )
        {
            let mut c = parent.children_with_tokens();
            if let Some(NodeOrToken::Token(first)) = c.next() {
                return first.text_range() == tk.text_range()
                    && matches!(c.next(), Some(NodeOrToken::Token(second)) if second.kind() == SyntaxKind::Dot);
            }
        }

        false
    }
    if is_element_id(token, parent) {
        // An id: search all use of the id in this Component
        let mut candidate = parent.parent();
        while let Some(c) = candidate {
            if c.kind() == SyntaxKind::Component {
                let mut ranges = Vec::new();
                let mut found_definition = false;
                recurse(
                    &mut ranges,
                    &mut found_definition,
                    c,
                    &i_slint_compiler::parser::normalize_identifier(token.text()),
                );
                fn recurse(
                    ranges: &mut Vec<TextRange>,
                    found_definition: &mut bool,
                    c: SyntaxNode,
                    text: &str,
                ) {
                    for x in c.children_with_tokens() {
                        match x {
                            NodeOrToken::Node(n) => recurse(ranges, found_definition, n, text),
                            NodeOrToken::Token(tk) => {
                                if is_element_id(&tk, &c)
                                    && i_slint_compiler::parser::normalize_identifier(tk.text())
                                        == text
                                {
                                    ranges.push(tk.text_range());
                                    if c.kind() == SyntaxKind::SubElement {
                                        *found_definition = true;
                                    }
                                }
                            }
                        }
                    }
                }
                if !found_definition {
                    return None;
                }
                return Some(ranges);
            }
            candidate = c.parent()
        }
    }
    None
}

#[allow(unused)]
pub fn rename_element_id(
    element: syntax_nodes::SubElement,
    new_name: &str,
) -> Option<Vec<TextEdit>> {
    let edits = if let Some(current_id) = element.child_token(SyntaxKind::Identifier) {
        let all_ids = find_element_ids(&current_id, &element)?;
        all_ids
            .into_iter()
            .map(|r| TextEdit {
                range: crate::util::text_range_to_lsp_range(&element.source_file, r),
                new_text: new_name.into(),
            })
            .collect::<Vec<_>>()
    } else {
        let position = crate::util::text_size_to_lsp_position(
            &element.source_file,
            element.text_range().start(),
        );
        element.text_range().start();
        vec![TextEdit {
            range: lsp_types::Range::new(position, position),
            new_text: format!("{new_name} := "),
        }]
    };
    Some(edits)
}

#[cfg(test)]
mod tests {
    use super::*;
    use i_slint_compiler::parser::TextSize;

    #[test]
    fn test_rename_element_id() {
        let source = r#"
component t-a inherits Rectangle { false_rectangle := Image {} }
global G { in-out property<brush> t-a;  }
component Foo inherits Rectangle {
    /// This is a comment
    Rectangle/*X*/ {
        background: t-a.has-hover ? red : G.t-a.transparentize(0.5);
    }

    t-a := TouchArea {  }
    if true: Rectangle/*Y*/ {
        background: t_a.has-hover ? red : green;
    }
    if false: false_rectangle := Rectangle/*Z*/ {
        height: false-rectangle.width;
    }
}
"#;
        let (dc, uri, _) = crate::language::test::loaded_document_cache(source.into());

        let rect_x = dc
            .element_at_offset(&uri, TextSize::new(source.find("Rectangle/*X*/").unwrap() as u32))
            .unwrap();
        let rect_x = rect_x
            .with_element_node(|n| n.parent().and_then(syntax_nodes::SubElement::new).unwrap());

        let edit = crate::common::create_workspace_edit(
            uri.clone(),
            None,
            rename_element_id(rect_x, "name-for-x").unwrap(),
        );
        let renamed = crate::common::text_edit::apply_workspace_edit(&dc, &edit)
            .unwrap()
            .into_iter()
            .next()
            .unwrap()
            .contents;
        assert_eq!(
            renamed,
            r#"
component t-a inherits Rectangle { false_rectangle := Image {} }
global G { in-out property<brush> t-a;  }
component Foo inherits Rectangle {
    /// This is a comment
    name-for-x := Rectangle/*X*/ {
        background: t-a.has-hover ? red : G.t-a.transparentize(0.5);
    }

    t-a := TouchArea {  }
    if true: Rectangle/*Y*/ {
        background: t_a.has-hover ? red : green;
    }
    if false: false_rectangle := Rectangle/*Z*/ {
        height: false-rectangle.width;
    }
}
"#
        );

        let rect_y = dc
            .element_at_offset(&uri, TextSize::new(source.find("Rectangle/*Y*/").unwrap() as u32))
            .unwrap();
        let rect_y = rect_y
            .with_element_node(|n| n.parent().and_then(syntax_nodes::SubElement::new).unwrap());

        let edit = crate::common::create_workspace_edit(
            uri.clone(),
            None,
            rename_element_id(rect_y, "name-for-y").unwrap(),
        );
        let renamed = crate::common::text_edit::apply_workspace_edit(&dc, &edit)
            .unwrap()
            .into_iter()
            .next()
            .unwrap()
            .contents;
        assert_eq!(
            renamed,
            r#"
component t-a inherits Rectangle { false_rectangle := Image {} }
global G { in-out property<brush> t-a;  }
component Foo inherits Rectangle {
    /// This is a comment
    Rectangle/*X*/ {
        background: t-a.has-hover ? red : G.t-a.transparentize(0.5);
    }

    t-a := TouchArea {  }
    if true: name-for-y := Rectangle/*Y*/ {
        background: t_a.has-hover ? red : green;
    }
    if false: false_rectangle := Rectangle/*Z*/ {
        height: false-rectangle.width;
    }
}
"#
        );

        let t_a = dc
            .element_at_offset(&uri, TextSize::new(source.find("TouchArea").unwrap() as u32))
            .unwrap();
        let t_a =
            t_a.with_element_node(|n| n.parent().and_then(syntax_nodes::SubElement::new).unwrap());

        let edit = crate::common::create_workspace_edit(
            uri.clone(),
            None,
            rename_element_id(t_a, "toucharea").unwrap(),
        );
        let renamed = crate::common::text_edit::apply_workspace_edit(&dc, &edit)
            .unwrap()
            .into_iter()
            .next()
            .unwrap()
            .contents;
        assert_eq!(
            renamed,
            r#"
component t-a inherits Rectangle { false_rectangle := Image {} }
global G { in-out property<brush> t-a;  }
component Foo inherits Rectangle {
    /// This is a comment
    Rectangle/*X*/ {
        background: toucharea.has-hover ? red : G.t-a.transparentize(0.5);
    }

    toucharea := TouchArea {  }
    if true: Rectangle/*Y*/ {
        background: toucharea.has-hover ? red : green;
    }
    if false: false_rectangle := Rectangle/*Z*/ {
        height: false-rectangle.width;
    }
}
"#
        );

        let rect_z = dc
            .element_at_offset(&uri, TextSize::new(source.find("Rectangle/*Z*/").unwrap() as u32))
            .unwrap();
        let rect_z = rect_z
            .with_element_node(|n| n.parent().and_then(syntax_nodes::SubElement::new).unwrap());

        let edit = crate::common::create_workspace_edit(
            uri.clone(),
            None,
            rename_element_id(rect_z, "zzz").unwrap(),
        );
        let renamed = crate::common::text_edit::apply_workspace_edit(&dc, &edit)
            .unwrap()
            .into_iter()
            .next()
            .unwrap()
            .contents;
        assert_eq!(
            renamed,
            r#"
component t-a inherits Rectangle { false_rectangle := Image {} }
global G { in-out property<brush> t-a;  }
component Foo inherits Rectangle {
    /// This is a comment
    Rectangle/*X*/ {
        background: t-a.has-hover ? red : G.t-a.transparentize(0.5);
    }

    t-a := TouchArea {  }
    if true: Rectangle/*Y*/ {
        background: t_a.has-hover ? red : green;
    }
    if false: zzz := Rectangle/*Z*/ {
        height: zzz.width;
    }
}
"#
        );
    }
}
