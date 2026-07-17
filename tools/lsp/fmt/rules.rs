// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

//! The Slint formatting rules for the query-based formatter.
//!
//! Global token rules handle punctuation spacing; node rules lay out each
//! construct's body — element, code-block, struct, enum, match, array,
//! animation, transition, states and import bodies — and tighten expression
//! spacing. Boundaries no rule covers fall back to the engine's default (a
//! single space between tokens).

use super::atoms::Atom::*;
use super::engine::{FormatRules, Selection, format_document_with_rules};
use super::writer::TokenWriter;
use i_slint_compiler::parser::{NodeOrToken, SyntaxKind, syntax_nodes};

/// The positions of the `open`/`close` delimiter pair among `children`, when
/// both are present in order.
fn delimiter_positions(
    children: &[NodeOrToken],
    open: SyntaxKind,
    close: SyntaxKind,
) -> Option<(usize, usize)> {
    let open_index = children.iter().position(|child| child.kind() == open)?;
    let close_index = children.iter().rposition(|child| child.kind() == close)?;
    (open_index <= close_index).then_some((open_index, close_index))
}

/// Break the body delimited by `open`/`close` (`{}`, `[]`) so that each item
/// sits on its own line when the node is multiline, and inline otherwise.
/// An empty body closes up tight (`{}`) unless the input already spread it
/// across lines. Items keep one blank line from the input.
fn break_braced_body(selection: &Selection, open: SyntaxKind, close: SyntaxKind) {
    let children: Vec<NodeOrToken> = selection.children().iter().cloned().collect();
    let Some((open_index, close_index)) = delimiter_positions(&children, open, close) else {
        return;
    };
    let edge = if close_index == open_index + 1 {
        selection.empty_softline()
    } else {
        selection.spaced_softline()
    };
    selection.at(children[open_index].clone()).append(IndentStart).append(edge.clone());
    selection.at(children[close_index].clone()).prepend(IndentEnd).prepend(edge);
    for child in &children[(open_index + 1)..close_index] {
        // Separators hug the item before them: a semicolon terminates a code
        // block statement, a comma separates struct/enum members — both are
        // direct children in those bodies, and their spacing comes from the
        // global punctuation rules, not from a per-item break.
        if matches!(child.kind(), SyntaxKind::Semicolon | SyntaxKind::Comma) {
            continue;
        }
        match child {
            NodeOrToken::Node(_) => {
                selection
                    .at(child.clone())
                    .prepend(AllowBlankLines)
                    .prepend(selection.spaced_softline());
            }
            // A stray token child is error recovery: source the grammar
            // could not place. Keep the input's line structure instead of
            // inventing breaks (InputSoftline abstains where the input had
            // no newline).
            NodeOrToken::Token(_) => {
                selection.at(child.clone()).prepend(InputSoftline);
            }
        }
    }
}

pub fn make_rules() -> FormatRules {
    let mut rules = FormatRules::default();

    // Global punctuation spacing. Fires on every matching token in the
    // document; node rules override it where they disagree.
    rules.token(SyntaxKind::Colon, |colon| {
        colon.prepend(Antispace).append(Space);
    });
    rules.token(SyntaxKind::Semicolon, |semicolon| {
        semicolon.prepend(Antispace);
    });
    // A comma hugs the token before it and keeps a newline after it only
    // where the input had one. A container-wide break is driven by each
    // body's own per-item rule (which measures the right node); a plain
    // measure of the comma's parent would misfire on declarations whose
    // parameter list shares a node with a multiline body.
    rules.token(SyntaxKind::Comma, |comma| {
        comma.prepend(Antispace).append(InputSoftline);
    });
    // Parentheses and brackets hug their contents.
    for kind in [SyntaxKind::LParent, SyntaxKind::LBracket] {
        rules.token(kind, |open| {
            open.append(Antispace);
        });
    }
    for kind in [SyntaxKind::RParent, SyntaxKind::RBracket] {
        rules.token(kind, |close| {
            close.prepend(Antispace);
        });
    }
    // A member-access or qualified-name dot hugs both neighbors
    // (`self.foo`, `MyModule.Widget`) — except after a bare integer, where
    // gluing would change the expression: `42.log(x)` re-lexes as `42.` `log`.
    rules.token(SyntaxKind::Dot, |dot| {
        dot.append(Antispace);
        for item in dot.iter() {
            let previous_significant_token = std::iter::successors(
                item.as_token().and_then(|token| token.prev_token()),
                |token| token.prev_token(),
            )
            .find(|token| !matches!(token.kind(), SyntaxKind::Whitespace | SyntaxKind::Comment));
            let dot_would_extend_the_number = previous_significant_token.is_some_and(|token| {
                token.kind() == SyntaxKind::NumberLiteral
                    && token.text().bytes().all(|byte| byte.is_ascii_digit())
            });
            if !dot_would_extend_the_number {
                dot.at(item.clone()).prepend(Antispace);
            }
        }
    });
    // `@` binds to the builtin that follows it: `@image-url`, `@children`.
    rules.token(SyntaxKind::At, |at| {
        at.append(Antispace);
    });

    // Spans of foreign syntax the global rules must not touch: the arbitrary
    // Rust tokens inside `@rust-attr(...)` and the expressions interpolated
    // into a string template (the lexer splits a template into StringLiteral
    // tokens with real expression tokens between them, so a stray colon or
    // comma there would otherwise be re-spaced).
    rules.node(SyntaxKind::AtRustAttr, |attr| {
        attr.leaf();
    });
    rules.node(SyntaxKind::StringTemplate, |template| {
        template.leaf();
    });

    // Top-level items (components, structs, enums, imports, exports) each go
    // on their own line, with input blank lines between them preserved.
    rules.node(SyntaxKind::Document, |document| {
        let children: Vec<NodeOrToken> = document.children().iter().cloned().collect();
        for child in children.iter().skip(1) {
            match child {
                NodeOrToken::Node(_) => {
                    document.at(child.clone()).prepend(AllowBlankLines).prepend(Hardline);
                }
                // A stray token child is error recovery: source the grammar
                // could not place. Keep the input's line structure instead
                // of inventing breaks (InputSoftline abstains where the
                // input had no newline).
                NodeOrToken::Token(_) => {
                    document.at(child.clone()).prepend(InputSoftline);
                }
            }
        }
        // The file ends with exactly one newline. Appending to the last item
        // lands the newline in the gap before Eof (Eof itself is not
        // selectable). A document with no significant item — empty, or
        // holding only comments — has nothing to append to and keeps its
        // trailing trivia as written.
        if let Some(last_child) = children.last() {
            document.at(last_child.clone()).append(Hardline);
        }
    });

    // Brace-delimited bodies — elements (`Foo { ... }`, including component,
    // global and interface bodies), imperative code blocks, match-element
    // bodies, struct/enum/object-literal members, animations, transitions and
    // import lists — break one item per line when multiline, stay inline
    // otherwise.
    for kind in [
        SyntaxKind::Element,
        SyntaxKind::CodeBlock,
        SyntaxKind::MatchElement,
        SyntaxKind::ObjectType,
        SyntaxKind::EnumDeclaration,
        SyntaxKind::ObjectLiteral,
        SyntaxKind::PropertyAnimation,
        SyntaxKind::Transition,
        SyntaxKind::ImportIdentifierList,
    ] {
        rules.node(kind, |body| {
            break_braced_body(body, SyntaxKind::LBrace, SyntaxKind::RBrace);
        });
    }

    // `transitions [ ... ]` breaks the same way, inside brackets.
    rules.node(SyntaxKind::Transitions, |transitions| {
        break_braced_body(transitions, SyntaxKind::LBracket, SyntaxKind::RBracket);
    });

    // A property's type sits tight inside its angle brackets:
    // `property <int> foo`, not `property < int > foo`.
    rules.node(SyntaxKind::PropertyDeclaration, |property| {
        property.token(SyntaxKind::LAngle).append(Antispace);
        property.token(SyntaxKind::RAngle).prepend(Antispace);
    });

    // A call, callback/function declaration or index keeps its parenthesis or
    // bracket tight against the name before it: `foo(a, b)`, `arr[i]`.
    for kind in [
        SyntaxKind::FunctionCallExpression,
        SyntaxKind::CallbackDeclaration,
        SyntaxKind::CallbackConnection,
        SyntaxKind::Function,
        SyntaxKind::AtImageUrl,
        SyntaxKind::AtGradient,
        SyntaxKind::AtTr,
        SyntaxKind::AtMarkdown,
        SyntaxKind::AtKeys,
    ] {
        rules.node(kind, |call| {
            call.token(SyntaxKind::LParent).prepend(Antispace);
        });
    }

    // A ternary spaces both operators symmetrically, overriding the global
    // colon rule (which would otherwise hug the colon to the true-branch).
    rules.node(SyntaxKind::ConditionalExpression, |ternary| {
        ternary.token(SyntaxKind::Question).prepend(Space).append(Space);
        ternary.token(SyntaxKind::Colon).prepend(Space).append(Space);
    });
    rules.node(SyntaxKind::IndexExpression, |index| {
        index.token(SyntaxKind::LBracket).prepend(Antispace);
    });
    // A repeated element's index binds tight to the model name: `for x[i] in`.
    rules.node(SyntaxKind::RepeatedIndex, |index| {
        index.prepend(Antispace);
    });

    // A unary operator hugs its operand: `-1`, `!enabled`.
    rules.node(SyntaxKind::UnaryOpExpression, |unary| {
        unary
            .token_matching(|kind| {
                matches!(kind, SyntaxKind::Minus | SyntaxKind::Plus | SyntaxKind::Bang)
            })
            .append(Antispace);
    });

    // Array literals stay tight inline (`[1, 2, 3]`) and break one element
    // per line when multiline; the commas carry the inter-element breaks.
    rules.node(SyntaxKind::Array, |array| {
        let children: Vec<NodeOrToken> = array.children().iter().cloned().collect();
        let Some((open_index, close_index)) =
            delimiter_positions(&children, SyntaxKind::LBracket, SyntaxKind::RBracket)
        else {
            return;
        };
        array.at(children[open_index].clone()).append(IndentStart).append(array.empty_softline());
        array.at(children[close_index].clone()).prepend(IndentEnd).prepend(array.empty_softline());
    });

    // `states [ ... ]`
    //
    // The softlines resolve against the whole `states` block's input span:
    // a block written on one line stays on one line, anything already spread
    // out formats to one state per line.
    rules.node(SyntaxKind::States, |states| {
        states.keyword("states").append(Space);
        // The `[` append also formats the empty block (`states [ ]`); at the
        // first state it collapses with the State prepend below.
        states.token(SyntaxKind::LBracket).append(IndentStart).append(states.spaced_softline());
        states.node(SyntaxKind::State).prepend(AllowBlankLines).prepend(states.spaced_softline());
        states.token(SyntaxKind::RBracket).prepend(IndentEnd).prepend(states.spaced_softline());
    });

    // `pressed when touch.pressed: { ... }` inside `states`
    rules.node(SyntaxKind::State, |state| {
        state.keyword("when").prepend(Space).append(Space);
        // The `:` spacing comes from the global Colon rule.
        state
            .token(SyntaxKind::LBrace)
            .prepend(Space)
            .append(IndentStart)
            .append(state.spaced_softline());
        state
            .node(SyntaxKind::StatePropertyChange)
            .prepend(AllowBlankLines)
            .prepend(state.spaced_softline());
        state
            .node(SyntaxKind::Transition)
            .prepend(AllowBlankLines)
            .prepend(state.spaced_softline());
        state.token(SyntaxKind::RBrace).prepend(IndentEnd).prepend(state.spaced_softline());
    });

    rules
}

/// Format a document with the query-based formatter and the standard Slint
/// rules.
pub fn format_document_query(
    document: syntax_nodes::Document,
    writer: &mut impl TokenWriter,
) -> std::io::Result<()> {
    format_document_with_rules(&document, &make_rules(), writer)
}
