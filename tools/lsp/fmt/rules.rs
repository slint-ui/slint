// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

//! The Slint formatting rules for the query-based formatter.
//!
//! This is the prototype ruleset: global punctuation spacing, indentation
//! bookkeeping for elements, and full formatting for the `states` construct.
//! Boundaries no rule covers fall back to the engine's default (a single
//! space between tokens), so the ruleset is still far from complete.

use super::atoms::Atom;
use super::atoms::Atom::*;
use super::engine::{FormatRules, Selection, format_document_with_rules};
use super::writer::TokenWriter;
use i_slint_compiler::parser::{NodeOrToken, SyntaxKind, syntax_nodes};

/// Break the body delimited by `open`/`close` (`{}`, `[]`) so that each item
/// sits on its own line when the node is multiline, and inline otherwise. A
/// non-empty inline body uses `edge` (usually a spaced softline) around its
/// delimiters; an empty one closes up tight (`{}`). Items in between break with
/// a spaced softline and keep one blank line from the input.
fn break_braced_body(selection: &Selection, open: SyntaxKind, close: SyntaxKind, edge: Atom) {
    let children: Vec<NodeOrToken> = selection.children().iter().cloned().collect();
    let Some(open_index) = children.iter().position(|child| child.kind() == open) else {
        return;
    };
    let Some(close_index) = children.iter().rposition(|child| child.kind() == close) else {
        return;
    };
    if close_index < open_index {
        return;
    }
    // An empty body collapses to `{}` inline (but keeps a broken-open `{`/`}`
    // pair when the input already spread it across lines).
    let edge = if close_index == open_index + 1 { selection.empty_softline() } else { edge };
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
        selection.at(child.clone()).prepend(AllowBlankLines).prepend(selection.spaced_softline());
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
    // A comma hugs the token before it and, after it, breaks with its
    // container: a space inline, a newline once the container spans lines.
    // The softline measures the comma's parent node, so one rule handles
    // every list — arguments, arrays, structs, enums, callback parameters.
    rules.token(SyntaxKind::Comma, |comma| {
        comma.prepend(Antispace).append(comma.spaced_softline());
    });
    rules.token(SyntaxKind::LParent, |paren| {
        paren.append(Antispace);
    });
    rules.token(SyntaxKind::RParent, |paren| {
        paren.prepend(Antispace);
    });
    rules.token(SyntaxKind::LBracket, |bracket| {
        bracket.append(Antispace);
    });
    rules.token(SyntaxKind::RBracket, |bracket| {
        bracket.prepend(Antispace);
    });
    // A member-access or qualified-name dot hugs both neighbors
    // (`self.foo`, `MyModule.Widget`).
    rules.token(SyntaxKind::Dot, |dot| {
        dot.prepend(Antispace).append(Antispace);
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
            document.at(child.clone()).prepend(AllowBlankLines).prepend(Hardline);
        }
    });

    // Element bodies (`Foo { ... }`, and the body of a component, global or
    // interface) break one item per line when multiline, stay inline
    // otherwise.
    rules.node(SyntaxKind::Element, |element| {
        break_braced_body(element, SyntaxKind::LBrace, SyntaxKind::RBrace, element.spaced_softline());
    });

    // Imperative code blocks (function, callback and binding bodies).
    rules.node(SyntaxKind::CodeBlock, |block| {
        break_braced_body(block, SyntaxKind::LBrace, SyntaxKind::RBrace, block.spaced_softline());
    });

    // Struct, enum and object-literal bodies: one member per line when
    // multiline, spaces inside the braces when inline.
    for kind in [SyntaxKind::ObjectType, SyntaxKind::EnumDeclaration, SyntaxKind::ObjectLiteral] {
        rules.node(kind, |body| {
            break_braced_body(body, SyntaxKind::LBrace, SyntaxKind::RBrace, body.spaced_softline());
        });
    }

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
        let Some(open) = children.iter().position(|child| child.kind() == SyntaxKind::LBracket)
        else {
            return;
        };
        let Some(close) = children.iter().rposition(|child| child.kind() == SyntaxKind::RBracket)
        else {
            return;
        };
        array.at(children[open].clone()).append(IndentStart).append(array.empty_softline());
        array.at(children[close].clone()).prepend(IndentEnd).prepend(array.empty_softline());
    });

    // Animation and transition bodies break like element bodies.
    for kind in [SyntaxKind::PropertyAnimation, SyntaxKind::Transition] {
        rules.node(kind, |body| {
            break_braced_body(body, SyntaxKind::LBrace, SyntaxKind::RBrace, body.spaced_softline());
        });
    }

    // `transitions [ ... ]`, one transition per line when multiline.
    rules.node(SyntaxKind::Transitions, |transitions| {
        break_braced_body(
            transitions,
            SyntaxKind::LBracket,
            SyntaxKind::RBracket,
            transitions.spaced_softline(),
        );
    });

    // `import { Foo, Bar } from "..."` keeps spaces inside the braces.
    rules.node(SyntaxKind::ImportIdentifierList, |list| {
        break_braced_body(list, SyntaxKind::LBrace, SyntaxKind::RBrace, list.spaced_softline());
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fmt::writer::FileWriter;
    use i_slint_compiler::diagnostics::BuildDiagnostics;

    fn format_once(source: &str) -> String {
        let document = i_slint_compiler::parser::parse(
            String::from(source),
            None,
            &mut BuildDiagnostics::default(),
        );
        let document = syntax_nodes::Document::new(document).unwrap();
        let mut output = Vec::new();
        format_document_query(document, &mut FileWriter { file: &mut output }).unwrap();
        String::from_utf8(output).unwrap()
    }

    /// Assert the formatted output, and that formatting is idempotent
    /// (formatting the output again changes nothing).
    #[track_caller]
    fn assert_formatting_query(unformatted: &str, formatted: &str) {
        assert_eq!(format_once(unformatted), formatted);
        assert_eq!(format_once(formatted), formatted, "formatting is not idempotent");
    }

    #[test]
    fn colon_and_semicolon_spacing_fires() {
        // The colon and semicolon rules space the punctuation; every other
        // boundary takes the default single space (the doubled space before
        // `y` collapses to one).
        assert_formatting_query("component A { x :1 ;  y:2; }", "component A { x: 1; y: 2; }");
    }

    #[test]
    fn antispace_deletes_even_an_input_newline() {
        // Input newlines only survive where a rule asks for them
        // (InputSoftline); a bare Antispace boundary collapses completely.
        assert_formatting_query("component A { x: 1\n; }", "component A {\n    x: 1;\n}");
    }

    #[test]
    fn single_line_states_stay_single_line() {
        assert_formatting_query(
            "component A { states [ s1 when b: { c: 1; } ] }",
            "component A { states [ s1 when b: { c: 1; } ] }",
        );
    }

    #[test]
    fn multiline_states_normalize() {
        assert_formatting_query(
            "component A {
    states [ s1 when b : {c: 1;} s2 : {
} ]
}",
            "component A {
    states [
        s1 when b: { c: 1; }
        s2: {
        }
    ]
}",
        );
    }

    #[test]
    fn blank_lines_between_states_cap_at_one() {
        assert_formatting_query(
            "component A {
    states [
        s1: { }



        s2: { }
    ]
}",
            "component A {
    states [
        s1: { }

        s2: { }
    ]
}",
        );
    }

    #[test]
    fn empty_states_block() {
        assert_formatting_query("component A { states [] }", "component A { states [ ] }");
        // A multiline empty block keeps one newline; the bracket pair's
        // IndentStart/IndentEnd cancel out in the same gap.
        assert_formatting_query(
            "component A { states [
] }",
            "component A {
    states [
    ]
}",
        );
    }

    #[test]
    fn trailing_comment_stays_on_state_line() {
        assert_formatting_query(
            "component A {
    states [
        s1: { } // trail
            s2: { }
    ]
}",
            "component A {
    states [
        s1: { } // trail
        s2: { }
    ]
}",
        );
    }

    #[test]
    fn own_line_comment_reindents() {
        assert_formatting_query(
            "component A {
    states [
        s1: { }
  // note
        s2: { }
    ]
}",
            "component A {
    states [
        s1: { }
        // note
        s2: { }
    ]
}",
        );
    }

    #[test]
    fn column_zero_comment_keeps_column_zero() {
        // The compiler's syntax tests use column-0 comments whose internal
        // spacing points at columns on the line above; they must not be
        // re-indented.
        assert_formatting_query(
            "component A {
    states [
        s1: { }
//  ^error{x}
        s2: { }
    ]
}",
            "component A {
    states [
        s1: { }
//  ^error{x}
        s2: { }
    ]
}",
        );
    }

    #[test]
    fn blank_line_above_own_line_comment_survives_capped() {
        assert_formatting_query(
            "component A {
    states [
        s1: { }


        // note
        s2: { }
    ]
}",
            "component A {
    states [
        s1: { }

        // note
        s2: { }
    ]
}",
        );
    }

    #[test]
    fn hanging_comment_on_lbrace() {
        // The `{`'s newline transfers past the hanging comment; the alignment
        // before it takes the default single space.
        assert_formatting_query(
            "component A {
    states [
        s1: {  // note
            c: 1;
        }
    ]
}",
            "component A {
    states [
        s1: { // note
            c: 1;
        }
    ]
}",
        );
    }

    #[test]
    fn own_line_comment_before_rbrace_at_inner_level() {
        // The `}`'s IndentEnd anchors after the comment: the comment sits at
        // the body level, the brace at the state level.
        assert_formatting_query(
            "component A {
    states [
        s1: {
            c: 1;
          // done
        }
    ]
}",
            "component A {
    states [
        s1: {
            c: 1;
            // done
        }
    ]
}",
        );
    }

    #[test]
    fn own_line_comment_before_rbracket_at_inner_level() {
        assert_formatting_query(
            "component A {
    states [
        s1: { }
            // end
    ]
}",
            "component A {
    states [
        s1: { }
        // end
    ]
}",
        );
    }

    #[test]
    fn block_comment_before_colon_glues_right() {
        // The colon's Antispace applies after the hanging comment — the
        // accepted gluing behavior.
        assert_formatting_query("component A { x /* c */ : 1; }", "component A { x /* c */: 1; }");
    }

    #[test]
    fn multiple_own_line_comments_each_reindent() {
        assert_formatting_query(
            "component A {
    states [
        s1: { }
    // a
            // b
        s2: { }
    ]
}",
            "component A {
    states [
        s1: { }
        // a
        // b
        s2: { }
    ]
}",
        );
    }

    #[test]
    fn hanging_comment_pair_stays_inline() {
        // Hanging comments (no newline before them) keep their line; the
        // spacing around them takes the default single space.
        assert_formatting_query(
            "component A {
    states [
        s1: { } /* a */ /* b */
        s2: { }
    ]
}",
            "component A {
    states [
        s1: { } /* a */ /* b */
        s2: { }
    ]
}",
        );
    }

    #[test]
    fn file_leading_comment_stays_on_its_line() {
        assert_formatting_query(
            "// header
component A { x: 1; }
",
            "// header
component A { x: 1; }",
        );
    }

    #[test]
    fn trailing_file_comment_stays_on_its_line() {
        assert_formatting_query(
            "component A { x :1; }
// tail
",
            "component A { x: 1; }
// tail
",
        );
    }

    #[test]
    fn blank_between_own_line_comments_survives() {
        assert_formatting_query(
            "component A {
    states [
        s1: { }
        // a


        // b
        s2: { }
    ]
}",
            "component A {
    states [
        s1: { }
        // a

        // b
        s2: { }
    ]
}",
        );
    }

    #[test]
    fn never_move_comment_off_its_line() {
        // The colon's appended Space meets an own-line comment: the comment
        // keeps its own line instead of being hoisted up to `x:`.
        assert_formatting_query(
            "component A {
    x:
    // why
    1;
}",
            "component A {
    x:
    // why
    1;
}",
        );
    }

    #[test]
    fn block_comment_continuation_lines_shift() {
        // Re-indenting the comment from column 2 to column 8 shifts its
        // second line by the same +6, preserving internal alignment.
        assert_formatting_query(
            "component A {
    states [
        s1: { }
  /* long
     note */
        s2: { }
    ]
}",
            "component A {
    states [
        s1: { }
        /* long
           note */
        s2: { }
    ]
}",
        );
    }

    #[test]
    fn ignore_directive_keeps_the_next_binding_verbatim() {
        // The binding flagged with the directive keeps its odd spacing; the
        // one after it formats normally.
        assert_formatting_query(
            "component A {
    // slint-fmt:ignore
    x   :1;
    y :2;
}",
            "component A {
    // slint-fmt:ignore
    x   :1;
    y: 2;
}",
        );
    }

    #[test]
    fn rust_attr_interior_is_left_verbatim() {
        // The odd spacing around the colon inside `@rust-attr(...)` is
        // preserved (it is the opaque-Rust leaf), while everything outside the
        // leaf — the attribute punctuation and the struct field's colon —
        // takes the ruleset's formatting.
        assert_formatting_query(
            "@rust-attr(a : b)
struct S { foo :int }",
            "@rust-attr (a : b) struct S { foo: int }",
        );
    }

    #[test]
    fn string_template_interpolation_is_left_verbatim() {
        // The colon of the ternary interpolated into the template stays
        // verbatim; the binding's own colon is respaced.
        assert_formatting_query(
            "component A { x :\"a\\{ c ? d : e }f\"; }",
            "component A { x: \"a\\{ c ? d : e }f\"; }",
        );
    }

    #[test]
    fn states_nested_in_elements_indent_by_depth() {
        // Nested element bodies break too; the states content lands at the
        // right depth (level 3: two elements + the states bracket).
        assert_formatting_query(
            "component A {
    inner := Rectangle {
        states [ s1: { c: 1; }
            s2: { } ]
    }
}",
            "component A {
    inner := Rectangle {
        states [
            s1: { c: 1; }
            s2: { }
        ]
    }
}",
        );
    }
}
