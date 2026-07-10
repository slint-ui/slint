// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

//! The Slint formatting rules for the query-based formatter.
//!
//! This is the prototype ruleset: global punctuation spacing, indentation
//! bookkeeping for elements, and full formatting for the `states` construct.
//! Every other gap is left untouched by the engine's keep-verbatim default.

use super::atoms::Atom::*;
use super::engine::{FormatRules, format_document_with_rules};
use super::writer::TokenWriter;
use i_slint_compiler::parser::{SyntaxKind, syntax_nodes};

pub fn make_rules() -> FormatRules {
    let mut rules = FormatRules::default();

    // Global punctuation spacing. Fires on every colon/semicolon in the
    // document; node rules override it where they disagree.
    // TODO: this also re-spaces ternary colons (`cond ? a : b` becomes
    // `cond ? a: b`) until a ConditionalExpression rule overrides it.
    rules.token(SyntaxKind::Colon, |colon| {
        colon.prepend(Antispace).append(Space);
    });
    rules.token(SyntaxKind::Semicolon, |semicolon| {
        semicolon.prepend(Antispace);
    });

    // Elements contribute only indentation bookkeeping for now: rules firing
    // inside an element need correct levels even while the element's own
    // spacing is left untouched.
    rules.node(SyntaxKind::Element, |element| {
        element.token(SyntaxKind::LBrace).append(IndentStart);
        element.token(SyntaxKind::RBrace).prepend(IndentEnd);
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
    fn already_formatted_input_is_untouched() {
        let source = "// header

component A {
    x: 1;
}
";
        assert_formatting_query(source, source);
    }

    #[test]
    fn colon_and_semicolon_spacing_fires_file_wide() {
        // Only the punctuation gaps change; the element braces and the
        // spacing between bindings stay as written.
        assert_formatting_query("component A { x :1 ;  y:2; }", "component A { x: 1;  y: 2; }");
    }

    #[test]
    fn antispace_deletes_even_an_input_newline() {
        // Input newlines only survive where a rule asks for them
        // (InputSoftline); a bare Antispace boundary collapses completely.
        assert_formatting_query("component A { x: 1\n; }", "component A { x: 1; }");
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
            "component A { states [
    ] }",
        );
    }

    #[test]
    fn comment_gap_without_atoms_is_untouched() {
        // No rule says anything about the gap around `// c`, so it stays
        // byte-identical while the colons elsewhere are re-spaced.
        assert_formatting_query(
            "component A { x :1; // c
y :2; }",
            "component A { x: 1; // c
y: 2; }",
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
        let source = "component A {
    states [
        s1: { }
//  ^error{x}
        s2: { }
    ]
}";
        assert_formatting_query(source, source);
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
        // The `{`'s newline transfers past the hanging comment; the
        // two-space alignment before it is preserved verbatim.
        let source = "component A {
    states [
        s1: {  // note
            c: 1;
        }
    ]
}";
        assert_formatting_query(source, source);
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
    fn hanging_comment_pair_kept_verbatim() {
        let source = "component A {
    states [
        s1: { } /* a */ /* b */
        s2: { }
    ]
}";
        assert_formatting_query(source, source);
    }

    #[test]
    fn file_leading_comment_untouched() {
        let source = "// header
component A { x: 1; }
";
        assert_formatting_query(source, source);
    }

    #[test]
    fn trailing_file_comment_untouched() {
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
        // keeps its own line (and here its indentation) instead of being
        // hoisted up to `x:`.
        let source = "component A {
    x:
    // why
    1;
}";
        assert_formatting_query(source, source);
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
    fn states_nested_in_unformatted_elements_indent_by_depth() {
        // The inner element's braces are never re-spaced, but their
        // indentation bookkeeping still places the states content at the
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
