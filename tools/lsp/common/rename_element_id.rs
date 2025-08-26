// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

use i_slint_compiler::parser::{NodeOrToken, SyntaxKind, SyntaxNode, SyntaxToken, TextRange};

/// If the token is matching a Element ID, return the list of all element id in the same component
///
/// Usefull for DocumentHighlightRequest as well as renaming
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
                recurse(&mut ranges, &mut found_definition, c, token.text());
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
                                if is_element_id(&tk, &c) && tk.text() == text {
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
