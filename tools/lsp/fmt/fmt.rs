// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

// cSpell:ignore Snaf Tatta

use super::writer::TokenWriter;
use i_slint_compiler::parser::{NodeOrToken, SyntaxKind, SyntaxNode, syntax_nodes};

pub fn format_document(
    doc: syntax_nodes::Document,
    writer: &mut impl TokenWriter,
) -> Result<(), std::io::Error> {
    let mut state = FormatState::default();
    format_node(&doc, writer, &mut state)
}

#[derive(Default)]
struct FormatState {
    /// The whitespace have been written, all further whitespace can be skipped
    skip_all_whitespace: bool,
    /// The whitespace to add before the next token
    whitespace_to_add: Option<String>,

    /// If the last token was SyntaxKind::WhiteSpace and it was not emit because of skip_all_whitespace,
    /// this contains the whitespace that was removed, so that it can be added again in case the next
    /// token is a comment
    last_removed_whitespace: Option<String>,

    /// The level of indentation
    indentation_level: u32,

    /// A counter that is incremented when something is inserted
    insertion_count: usize,

    /// a comment has been written followed maybe by some spacing
    after_comment: bool,

    /// When `true`, the formatter is inside a code block being kept on a single
    /// line. `new_line()` then becomes a space, so statements that would
    /// normally insert a trailing newline (e.g. `return` / `let`) stay inline.
    inline_codeblock: bool,
}

impl FormatState {
    fn new_line(&mut self) {
        if self.inline_codeblock {
            self.insert_whitespace(" ");
            return;
        }
        if self.after_comment {
            return;
        }
        self.skip_all_whitespace = true;
        if let Some(x) = &mut self.whitespace_to_add {
            x.insert(0, '\n');
            return;
        }
        let mut new_line = String::from("\n");
        for _ in 0..self.indentation_level {
            new_line += "    ";
        }
        self.whitespace_to_add = Some(new_line);
    }

    fn insert_whitespace(&mut self, arg: &str) {
        if self.after_comment {
            return;
        }
        self.skip_all_whitespace = true;
        if !arg.is_empty() {
            if let Some(ws) = &mut self.whitespace_to_add {
                *ws += arg;
            } else {
                self.whitespace_to_add = Some(arg.into());
            }
        }
    }
}

fn format_node(
    node: &SyntaxNode,
    writer: &mut impl TokenWriter,
    state: &mut FormatState,
) -> Result<(), std::io::Error> {
    match node.kind() {
        SyntaxKind::Component => {
            return format_component(node, writer, state);
        }
        SyntaxKind::Document => {
            return format_document_node(node, writer, state);
        }
        SyntaxKind::Element => {
            return format_element(node, writer, state);
        }
        SyntaxKind::SubElement => {
            return format_sub_element(node, writer, state);
        }
        SyntaxKind::PropertyDeclaration => {
            return format_property_declaration(node, writer, state);
        }
        SyntaxKind::Binding => {
            return format_binding(node, writer, state);
        }
        SyntaxKind::TwoWayBinding => {
            return format_two_way_binding(node, writer, state);
        }
        SyntaxKind::CallbackConnection => {
            return format_callback_connection(node, writer, state);
        }
        SyntaxKind::CallbackDeclaration => {
            return format_callback_declaration(node, writer, state);
        }
        SyntaxKind::Function => {
            return format_function(node, writer, state);
        }
        SyntaxKind::ArgumentDeclaration => {
            return format_argument_declaration(node, writer, state);
        }
        SyntaxKind::QualifiedName => {
            return format_qualified_name(node, writer, state);
        }
        SyntaxKind::SelfAssignment | SyntaxKind::BinaryExpression => {
            return format_binary_expression(node, writer, state);
        }
        SyntaxKind::ConditionalExpression => {
            return format_conditional_expression(node, writer, state);
        }
        SyntaxKind::Expression => {
            return format_expression(node, writer, state);
        }
        SyntaxKind::CodeBlock => {
            return format_codeblock(node, writer, state);
        }
        SyntaxKind::ReturnStatement => {
            return format_return_statement(node, writer, state);
        }
        SyntaxKind::LetStatement => {
            return format_let_statement(node, writer, state);
        }
        SyntaxKind::AtGradient => {
            return format_at_gradient(node, writer, state);
        }
        SyntaxKind::ChildrenPlaceholder => {
            return format_children_placeholder(node, writer, state);
        }
        SyntaxKind::RepeatedElement => {
            return format_repeated_element(node, writer, state);
        }
        SyntaxKind::RepeatedIndex => {
            return format_repeated_index(node, writer, state);
        }
        SyntaxKind::MatchElement => {
            return format_match_element(node, writer, state);
        }
        SyntaxKind::MatchCase | SyntaxKind::WildcardMatchCase => {
            return format_match_case(node, writer, state);
        }
        SyntaxKind::Array => {
            return format_array(node, writer, state);
        }
        SyntaxKind::State => {
            return format_state(node, writer, state);
        }
        SyntaxKind::States => {
            return format_states(node, writer, state);
        }
        SyntaxKind::StatePropertyChange => {
            return format_state_prop_change(node, writer, state);
        }
        SyntaxKind::Transition => {
            return format_transition(node, writer, state);
        }
        SyntaxKind::PropertyAnimation => {
            return format_property_animation(node, writer, state);
        }
        SyntaxKind::ObjectLiteral => {
            return format_object_literal(node, writer, state);
        }
        SyntaxKind::StructDeclaration => {
            return format_struct_declaration(node, writer, state);
        }
        SyntaxKind::EnumDeclaration => {
            return format_enum_declaration(node, writer, state);
        }
        SyntaxKind::ExportsList => {
            return format_exports_list(node, writer, state);
        }
        SyntaxKind::ExportSpecifier => {
            return format_export_specifier(node, writer, state);
        }
        SyntaxKind::ObjectType => {
            return format_object_type(node, writer, state);
        }
        SyntaxKind::ObjectTypeMember => {
            return format_object_type_member(node, writer, state);
        }
        SyntaxKind::PropertyChangedCallback => {
            return format_property_changed_callback(node, writer, state);
        }
        SyntaxKind::MemberAccess => {
            return format_member_access(node, writer, state);
        }
        SyntaxKind::ImportSpecifier => {
            return format_import_specifier(node, writer, state);
        }
        SyntaxKind::UsesSpecifier => {
            return format_uses_specifier(node, writer, state);
        }
        SyntaxKind::ImplementsSpecifier => {
            return format_implements_specifier(node, writer, state);
        }
        _ => (),
    }

    for n in node.children_with_tokens() {
        fold(n, writer, state)?;
    }
    Ok(())
}

fn format_document_node(
    node: &SyntaxNode,
    writer: &mut impl TokenWriter,
    state: &mut FormatState,
) -> Result<(), std::io::Error> {
    for n in node.children_with_tokens() {
        if let Some(t) = n.as_token()
            && t.kind() == SyntaxKind::Whitespace
            && whitespace_has_empty_line(t.text())
            && next_non_trivia_token_kind(&n).is_some_and(|kind| kind != SyntaxKind::Eof)
        {
            // Preserve document-level blank lines as written and override any
            // pending newline inserted by the preceding formatter branch.
            state.skip_all_whitespace = false;
            state.whitespace_to_add = None;
            state.last_removed_whitespace = None;
            state.after_comment = false;
            state.insertion_count += 1;
            writer.no_change(t.clone())?;
            continue;
        }

        // Comments are tokens, but top-level items are nodes. Make sure a preceding
        // comment does not suppress indentation inside the next formatted node.
        if n.as_node().is_some() {
            state.after_comment = false;
        }
        fold(n, writer, state)?;
    }
    Ok(())
}

fn fold(
    n: NodeOrToken,
    writer: &mut impl TokenWriter,
    state: &mut FormatState,
) -> std::io::Result<()> {
    match n {
        NodeOrToken::Node(n) => format_node(&n, writer, state),
        NodeOrToken::Token(t) => {
            if t.kind() == SyntaxKind::Eof {
                if state.skip_all_whitespace {
                    writer.with_new_content(t, "\n")?;
                }
                return Ok(());
            } else if t.kind() == SyntaxKind::Whitespace {
                if state.skip_all_whitespace && !state.after_comment {
                    state.last_removed_whitespace = Some(t.text().to_string());
                    writer.with_new_content(t, "")?;
                    return Ok(());
                }
            } else {
                state.after_comment = t.kind() == SyntaxKind::Comment;
                state.skip_all_whitespace = false;
                if let Some(ws) = state.last_removed_whitespace.take()
                    && state.after_comment
                {
                    // restore the previously skipped spaces before comment.
                    state.insertion_count += 1;
                    writer.insert_before(t, &ws)?;
                    state.whitespace_to_add = None;
                    return Ok(());
                }
                if let Some(x) = state.whitespace_to_add.take() {
                    state.insertion_count += 1;
                    writer.insert_before(t, x.as_ref())?;
                    return Ok(());
                }
            }
            state.insertion_count += 1;
            writer.no_change(t)
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum SyntaxMatch {
    NotFound,
    Found(SyntaxKind),
}

impl SyntaxMatch {
    fn is_found(self) -> bool {
        matches!(self, SyntaxMatch::Found(..))
    }
}

fn whitespace_has_empty_line(text: &str) -> bool {
    let mut newlines = 0;
    for b in text.bytes() {
        if b == b'\n' {
            newlines += 1;
            if newlines >= 2 {
                return true;
            }
        }
    }
    false
}

fn next_non_trivia_token_kind(item: &NodeOrToken) -> Option<SyntaxKind> {
    match item {
        NodeOrToken::Node(node) => Some(node.kind()),
        NodeOrToken::Token(token) => {
            let mut current = token.clone();
            loop {
                match current.kind() {
                    SyntaxKind::Whitespace | SyntaxKind::Comment => {
                        current = current.next_token()?;
                    }
                    kind => return Some(kind),
                }
            }
        }
    }
}

fn prev_non_trivia_token_kind(token: i_slint_compiler::parser::SyntaxToken) -> Option<SyntaxKind> {
    let mut current = token;
    loop {
        match current.kind() {
            SyntaxKind::Whitespace | SyntaxKind::Comment => {
                current = current.prev_token()?;
            }
            kind => return Some(kind),
        }
    }
}

fn whitespace_to(
    sub: &mut impl Iterator<Item = NodeOrToken>,
    element: SyntaxKind,
    writer: &mut impl TokenWriter,
    state: &mut FormatState,
    prefix_whitespace: &str,
) -> Result<bool, std::io::Error> {
    whitespace_to_one_of(sub, &[element], writer, state, prefix_whitespace)
        .map(SyntaxMatch::is_found)
}

fn whitespace_to_one_of(
    sub: &mut impl Iterator<Item = NodeOrToken>,
    elements: &[SyntaxKind],
    writer: &mut impl TokenWriter,
    state: &mut FormatState,
    prefix_whitespace: &str,
) -> Result<SyntaxMatch, std::io::Error> {
    state.insert_whitespace(prefix_whitespace);
    for n in sub {
        match n.kind() {
            SyntaxKind::Whitespace | SyntaxKind::Comment => (),
            expected_kind if elements.contains(&expected_kind) => {
                fold(n, writer, state)?;
                return Ok(SyntaxMatch::Found(expected_kind));
            }
            _ => {
                tracing::warn!("Inconsistency: expected {elements:?},  found {n:?}");
                fold(n, writer, state)?;
                return Ok(SyntaxMatch::NotFound);
            }
        }
        fold(n, writer, state)?;
    }
    tracing::warn!("Inconsistency: expected {elements:?},  not found");
    Ok(SyntaxMatch::NotFound)
}

fn finish_node(
    sub: impl Iterator<Item = NodeOrToken>,
    writer: &mut impl TokenWriter,
    state: &mut FormatState,
) -> Result<bool, std::io::Error> {
    // FIXME:  We should check that there are only comments or whitespace in sub
    for n in sub {
        fold(n, writer, state)?;
    }
    Ok(true)
}

fn format_component(
    node: &SyntaxNode,
    writer: &mut impl TokenWriter,
    state: &mut FormatState,
) -> Result<(), std::io::Error> {
    if node.child_token(SyntaxKind::ColonEqual).is_some() {
        // Legacy syntax
        let mut sub = node.children_with_tokens();
        let _ok = whitespace_to(&mut sub, SyntaxKind::DeclaredIdentifier, writer, state, "")?
            && whitespace_to(&mut sub, SyntaxKind::ColonEqual, writer, state, " ")?
            && whitespace_to(&mut sub, SyntaxKind::Element, writer, state, " ")?;

        finish_node(sub, writer, state)?;
        state.new_line();
    } else {
        let mut sub = node.children_with_tokens();
        let _ok = whitespace_to(&mut sub, SyntaxKind::Identifier, writer, state, "")?
            && whitespace_to(&mut sub, SyntaxKind::DeclaredIdentifier, writer, state, " ")?;
        let r = whitespace_to_one_of(
            &mut sub,
            &[SyntaxKind::Identifier, SyntaxKind::UsesSpecifier, SyntaxKind::Element],
            writer,
            state,
            " ",
        )?;
        if r == SyntaxMatch::Found(SyntaxKind::Identifier) {
            whitespace_to(&mut sub, SyntaxKind::Element, writer, state, " ")?;
        }

        finish_node(sub, writer, state)?;
        state.new_line();
    }

    Ok(())
}

fn format_element(
    node: &SyntaxNode,
    writer: &mut impl TokenWriter,
    state: &mut FormatState,
) -> Result<(), std::io::Error> {
    let mut sub = node.children_with_tokens();

    let ok = if node.child_node(SyntaxKind::QualifiedName).is_some() {
        whitespace_to(&mut sub, SyntaxKind::QualifiedName, writer, state, "")?
            && whitespace_to(&mut sub, SyntaxKind::LBrace, writer, state, " ")?
    } else {
        whitespace_to(&mut sub, SyntaxKind::LBrace, writer, state, "")?
    };

    if !ok {
        finish_node(sub, writer, state)?;
        return Ok(());
    }

    state.indentation_level += 1;
    state.new_line();
    let ins_ctn = state.insertion_count;
    let mut inserted_newline = false;

    let mut sub = sub.peekable();
    while let Some(n) = sub.next() {
        if n.kind() != SyntaxKind::Comment
            && let Some(last_removed_whitespace) = state.last_removed_whitespace.take()
        {
            let is_empty_line = whitespace_has_empty_line(&last_removed_whitespace);
            if is_empty_line && !inserted_newline {
                state.new_line();
            }
        }
        if n.kind() == SyntaxKind::Whitespace {
            if state.after_comment {
                // After a comment: check if the next non-trivia child is
                // the closing brace. If so, skip this whitespace so the
                // brace can get a properly indented newline.
                let next_is_rbrace = sub
                    .peek()
                    .and_then(next_non_trivia_token_kind)
                    .is_some_and(|k| k == SyntaxKind::RBrace);
                if next_is_rbrace {
                    state.after_comment = false;
                    state.skip_all_whitespace = true;
                    fold(n, writer, state)?;
                    continue;
                }
            } else {
                let is_empty_line =
                    n.as_token().map(|n| whitespace_has_empty_line(n.text())).unwrap_or(false);
                if is_empty_line {
                    if !inserted_newline {
                        state.new_line();
                    }
                    continue;
                }
            }
        }
        inserted_newline = false;

        if n.kind() == SyntaxKind::RBrace {
            state.indentation_level -= 1;
            state.whitespace_to_add = None;
            if ins_ctn == state.insertion_count {
                state.insert_whitespace(" ");
            } else {
                state.new_line();
            }
            fold(n, writer, state)?;
            state.new_line();
        } else {
            let put_newline_after = n.kind() == SyntaxKind::SubElement;

            fold(n, writer, state)?;

            if put_newline_after {
                state.new_line();
                inserted_newline = true;
            }
        }
    }
    Ok(())
}

fn format_sub_element(
    node: &SyntaxNode,
    writer: &mut impl TokenWriter,
    state: &mut FormatState,
) -> Result<(), std::io::Error> {
    let mut sub = node.children_with_tokens().peekable();

    // Let's decide based on the first child
    match sub.peek() {
        Some(first_node_or_token) => {
            if first_node_or_token.kind() == SyntaxKind::Identifier {
                // In this branch the sub element starts with an identifier, eg.
                // Something := Text {...}
                if !(whitespace_to(&mut sub, SyntaxKind::Identifier, writer, state, "")?
                    && whitespace_to(&mut sub, SyntaxKind::ColonEqual, writer, state, " ")?)
                {
                    // There is an error finding the QualifiedName and LBrace when we do this branch.
                    finish_node(sub, writer, state)?;
                    return Ok(());
                }
                state.insert_whitespace(" ");
            }
            // If the first child was not an identifier, we just fold it
            // (it might be an element, eg. Text {...})
            for s in sub {
                fold(s, writer, state)?;
            }
            Ok(())
        }
        // No children found -> we ignore this node
        None => Ok(()),
    }
}

fn format_property_declaration(
    node: &SyntaxNode,
    writer: &mut impl TokenWriter,
    state: &mut FormatState,
) -> Result<(), std::io::Error> {
    let mut sub = node.children_with_tokens();
    whitespace_to(&mut sub, SyntaxKind::Identifier, writer, state, "")?;
    while let SyntaxMatch::Found(x) = whitespace_to_one_of(
        &mut sub,
        &[SyntaxKind::Identifier, SyntaxKind::LAngle, SyntaxKind::DeclaredIdentifier],
        writer,
        state,
        " ",
    )? {
        match x {
            SyntaxKind::DeclaredIdentifier => break,
            SyntaxKind::LAngle => {
                let _ok = whitespace_to(&mut sub, SyntaxKind::Type, writer, state, "")?
                    && whitespace_to(&mut sub, SyntaxKind::RAngle, writer, state, "")?
                    && whitespace_to(&mut sub, SyntaxKind::DeclaredIdentifier, writer, state, " ")?;
                break;
            }
            _ => continue,
        }
    }
    let need_newline = node.child_node(SyntaxKind::TwoWayBinding).is_none();

    state.skip_all_whitespace = true;
    for s in sub {
        fold(s, writer, state)?;
    }
    if need_newline {
        state.new_line();
    }
    Ok(())
}

fn format_binding(
    node: &SyntaxNode,
    writer: &mut impl TokenWriter,
    state: &mut FormatState,
) -> Result<(), std::io::Error> {
    let mut sub = node.children_with_tokens();
    let _ok = whitespace_to(&mut sub, SyntaxKind::Identifier, writer, state, "")?
        && whitespace_to(&mut sub, SyntaxKind::Colon, writer, state, "")?
        && whitespace_to(&mut sub, SyntaxKind::BindingExpression, writer, state, " ")?;
    // FIXME: more formatting
    for s in sub {
        fold(s, writer, state)?;
    }
    state.new_line();
    Ok(())
}

fn format_two_way_binding(
    node: &SyntaxNode,
    writer: &mut impl TokenWriter,
    state: &mut FormatState,
) -> Result<(), std::io::Error> {
    let mut sub = node.children_with_tokens();
    if node.child_token(SyntaxKind::Identifier).is_some() {
        whitespace_to(&mut sub, SyntaxKind::Identifier, writer, state, "")?;
    }
    let _ok = whitespace_to(&mut sub, SyntaxKind::DoubleArrow, writer, state, " ")?
        && whitespace_to(&mut sub, SyntaxKind::Expression, writer, state, " ")?;
    if node.child_token(SyntaxKind::Semicolon).is_some() {
        whitespace_to(&mut sub, SyntaxKind::Semicolon, writer, state, "")?;
        state.new_line();
    }
    for s in sub {
        fold(s, writer, state)?;
    }
    Ok(())
}

fn format_callback_declaration(
    node: &SyntaxNode,
    writer: &mut impl TokenWriter,
    state: &mut FormatState,
) -> Result<(), std::io::Error> {
    let mut sub = node.children_with_tokens();
    whitespace_to(&mut sub, SyntaxKind::Identifier, writer, state, "")?;
    while whitespace_to_one_of(
        &mut sub,
        &[SyntaxKind::Identifier, SyntaxKind::DeclaredIdentifier],
        writer,
        state,
        " ",
    )? == SyntaxMatch::Found(SyntaxKind::Identifier)
    {}

    while let Some(n) = sub.next() {
        state.skip_all_whitespace = true;
        match n.kind() {
            SyntaxKind::Comma => {
                fold(n, writer, state)?;
                state.insert_whitespace(" ");
            }
            SyntaxKind::Arrow => {
                state.insert_whitespace(" ");
                fold(n, writer, state)?;
                whitespace_to(&mut sub, SyntaxKind::ReturnType, writer, state, " ")?;
            }
            SyntaxKind::TwoWayBinding => {
                fold(n, writer, state)?;
            }
            SyntaxKind::CallbackDeclarationParameter => {
                for n in n.as_node().unwrap().children_with_tokens() {
                    state.skip_all_whitespace = true;
                    match n.kind() {
                        SyntaxKind::Colon => {
                            fold(n, writer, state)?;
                            state.insert_whitespace(" ");
                        }
                        _ => {
                            fold(n, writer, state)?;
                        }
                    }
                }
            }
            _ => {
                fold(n, writer, state)?;
            }
        }
    }
    state.new_line();
    Ok(())
}

fn format_function(
    node: &SyntaxNode,
    writer: &mut impl TokenWriter,
    state: &mut FormatState,
) -> Result<(), std::io::Error> {
    let mut sub = node.children_with_tokens();
    whitespace_to(&mut sub, SyntaxKind::Identifier, writer, state, "")?;
    while whitespace_to_one_of(
        &mut sub,
        &[SyntaxKind::Identifier, SyntaxKind::DeclaredIdentifier],
        writer,
        state,
        " ",
    )? == SyntaxMatch::Found(SyntaxKind::Identifier)
    {}

    while let Some(n) = sub.next() {
        state.skip_all_whitespace = true;
        match n.kind() {
            SyntaxKind::Comma => {
                fold(n, writer, state)?;
                state.insert_whitespace(" ");
            }
            SyntaxKind::Arrow => {
                state.insert_whitespace(" ");
                fold(n, writer, state)?;
                whitespace_to(&mut sub, SyntaxKind::ReturnType, writer, state, " ")?;
            }
            SyntaxKind::CodeBlock => {
                state.insert_whitespace(" ");
                fold(n, writer, state)?;
            }
            _ => {
                fold(n, writer, state)?;
            }
        }
    }
    state.new_line();
    Ok(())
}

fn format_argument_declaration(
    node: &SyntaxNode,
    writer: &mut impl TokenWriter,
    state: &mut FormatState,
) -> Result<(), std::io::Error> {
    for n in node.children_with_tokens() {
        state.skip_all_whitespace = true;
        match n.kind() {
            SyntaxKind::Colon => {
                fold(n, writer, state)?;
                state.insert_whitespace(" ");
            }
            _ => {
                fold(n, writer, state)?;
            }
        }
    }
    Ok(())
}

fn format_callback_connection(
    node: &SyntaxNode,
    writer: &mut impl TokenWriter,
    state: &mut FormatState,
) -> Result<(), std::io::Error> {
    let mut sub = node.children_with_tokens();
    whitespace_to(&mut sub, SyntaxKind::Identifier, writer, state, "")?;

    for s in sub {
        state.skip_all_whitespace = true;
        match s.kind() {
            SyntaxKind::FatArrow => {
                state.insert_whitespace(" ");
                fold(s, writer, state)?;
                state.insert_whitespace(" ");
            }
            SyntaxKind::Comma => {
                fold(s, writer, state)?;
                state.insert_whitespace(" ");
            }
            _ => fold(s, writer, state)?,
        }
    }
    state.new_line();
    Ok(())
}

fn format_qualified_name(
    node: &SyntaxNode,
    writer: &mut impl TokenWriter,
    state: &mut FormatState,
) -> Result<(), std::io::Error> {
    for n in node.children_with_tokens() {
        state.skip_all_whitespace = true;
        fold(n, writer, state)?;
    }
    /*if !node
        .last_token()
        .and_then(|x| x.next_token())
        .map(|x| {
            matches!(
                x.kind(),
                SyntaxKind::LParent
                    | SyntaxKind::RParent
                    | SyntaxKind::Semicolon
                    | SyntaxKind::Comma
            )
        })
        .unwrap_or(false)
    {
        state.insert_whitespace(" ");
    } else {
        state.skip_all_whitespace = true;
    }*/
    Ok(())
}

/// Check whether the whitespace immediately preceding `token` in the token
/// stream contains a newline.
fn has_newline_before(token: &i_slint_compiler::parser::SyntaxToken) -> bool {
    token
        .prev_token()
        .is_some_and(|prev| prev.kind() == SyntaxKind::Whitespace && prev.text().contains('\n'))
}

// Called both for BinaryExpression and SelfAssignment
fn format_binary_expression(
    node: &SyntaxNode,
    writer: &mut impl TokenWriter,
    state: &mut FormatState,
) -> Result<(), std::io::Error> {
    let op_token = node.children_with_tokens().find(|n| {
        !matches!(n.kind(), SyntaxKind::Whitespace | SyntaxKind::Comment | SyntaxKind::Expression)
    });
    let has_newline = op_token.as_ref().and_then(|op| op.as_token()).is_some_and(|op| {
        has_newline_before(op)
            || op
                .next_token()
                .is_some_and(|t| t.kind() == SyntaxKind::Whitespace && t.text().contains('\n'))
    });

    if has_newline {
        let mut seen_first_expr = false;
        for n in node.children_with_tokens() {
            match n.kind() {
                SyntaxKind::Whitespace => {
                    state.skip_all_whitespace = true;
                    fold(n, writer, state)?;
                }
                SyntaxKind::Comment => {
                    fold(n, writer, state)?;
                }
                SyntaxKind::Expression => {
                    if seen_first_expr {
                        let nl = n
                            .as_node()
                            .and_then(|n| n.first_token())
                            .is_some_and(|t| has_newline_before(&t));
                        if nl {
                            state.whitespace_to_add = None;
                            state.indentation_level += 1;
                            state.new_line();
                            state.indentation_level -= 1;
                        }
                    }
                    seen_first_expr = true;
                    fold(n, writer, state)?;
                }
                _ => {
                    if let Some(t) = n.as_token()
                        && has_newline_before(t)
                    {
                        state.whitespace_to_add = None;
                        state.indentation_level += 1;
                        state.new_line();
                        state.indentation_level -= 1;
                    } else {
                        state.insert_whitespace(" ");
                    }
                    fold(n, writer, state)?;
                    state.insert_whitespace(" ");
                }
            }
        }
    } else {
        let mut sub = node.children_with_tokens();
        let _ok = whitespace_to(&mut sub, SyntaxKind::Expression, writer, state, "")?
            && whitespace_to_one_of(
                &mut sub,
                &[
                    SyntaxKind::Plus,
                    SyntaxKind::Minus,
                    SyntaxKind::Star,
                    SyntaxKind::Div,
                    SyntaxKind::AndAnd,
                    SyntaxKind::OrOr,
                    SyntaxKind::EqualEqual,
                    SyntaxKind::NotEqual,
                    SyntaxKind::LAngle,
                    SyntaxKind::LessEqual,
                    SyntaxKind::RAngle,
                    SyntaxKind::GreaterEqual,
                    SyntaxKind::Equal,
                    SyntaxKind::PlusEqual,
                    SyntaxKind::MinusEqual,
                    SyntaxKind::StarEqual,
                    SyntaxKind::DivEqual,
                ],
                writer,
                state,
                " ",
            )?
            .is_found()
            && whitespace_to(&mut sub, SyntaxKind::Expression, writer, state, " ")?;
    }
    Ok(())
}

fn format_conditional_expression(
    node: &SyntaxNode,
    writer: &mut impl TokenWriter,
    state: &mut FormatState,
) -> Result<(), std::io::Error> {
    let has_if = node.child_text(SyntaxKind::Identifier).is_some_and(|x| x == "if");

    let mut sub = node.children_with_tokens();
    if has_if {
        let ok = whitespace_to(&mut sub, SyntaxKind::Identifier, writer, state, "")?
            && whitespace_to(&mut sub, SyntaxKind::Expression, writer, state, " ")?
            && whitespace_to(&mut sub, SyntaxKind::Expression, writer, state, " ")?;
        if !ok {
            finish_node(sub, writer, state)?;
            return Ok(());
        }
        let mut pending_newline = false;
        while let Some(n) = sub.next() {
            state.skip_all_whitespace = true;
            match n.kind() {
                SyntaxKind::Whitespace => {
                    if n.as_token().is_some_and(|t| t.text().contains('\n')) {
                        pending_newline = true;
                    }
                    fold(n, writer, state)?;
                }
                SyntaxKind::Identifier => {
                    // `else` keyword
                    if pending_newline {
                        state.new_line();
                        pending_newline = false;
                    } else {
                        state.insert_whitespace(" ");
                    }
                    fold(n, writer, state)?;
                    whitespace_to(&mut sub, SyntaxKind::Expression, writer, state, " ")?;
                }
                _ => {
                    fold(n, writer, state)?;
                }
            }
        }
        state.whitespace_to_add = None;
        state.new_line();
    } else {
        let has_newline = node.children_with_tokens().any(|n| {
            matches!(n.kind(), SyntaxKind::Question | SyntaxKind::Colon)
                && n.as_token().is_some_and(|t| {
                    has_newline_before(t)
                        || t.next_token().is_some_and(|nt| {
                            nt.kind() == SyntaxKind::Whitespace && nt.text().contains('\n')
                        })
                })
        });

        if has_newline {
            let mut seen_first_expr = false;
            for n in node.children_with_tokens() {
                match n.kind() {
                    SyntaxKind::Whitespace => {
                        state.skip_all_whitespace = true;
                        fold(n, writer, state)?;
                    }
                    SyntaxKind::Comment => {
                        fold(n, writer, state)?;
                    }
                    SyntaxKind::Expression => {
                        if seen_first_expr {
                            let nl = n
                                .as_node()
                                .and_then(|n| n.first_token())
                                .is_some_and(|t| has_newline_before(&t));
                            if nl {
                                state.whitespace_to_add = None;
                                state.indentation_level += 1;
                                state.new_line();
                                state.indentation_level -= 1;
                            }
                        }
                        seen_first_expr = true;
                        fold(n, writer, state)?;
                    }
                    SyntaxKind::Question | SyntaxKind::Colon => {
                        if let Some(t) = n.as_token()
                            && has_newline_before(t)
                        {
                            state.whitespace_to_add = None;
                            state.indentation_level += 1;
                            state.new_line();
                            state.indentation_level -= 1;
                        } else {
                            state.insert_whitespace(" ");
                        }
                        fold(n, writer, state)?;
                        state.insert_whitespace(" ");
                    }
                    _ => {
                        fold(n, writer, state)?;
                    }
                }
            }
        } else {
            let _ok = whitespace_to(&mut sub, SyntaxKind::Expression, writer, state, "")?
                && whitespace_to(&mut sub, SyntaxKind::Question, writer, state, " ")?
                && whitespace_to(&mut sub, SyntaxKind::Expression, writer, state, " ")?
                && whitespace_to(&mut sub, SyntaxKind::Colon, writer, state, " ")?
                && whitespace_to(&mut sub, SyntaxKind::Expression, writer, state, " ")?;
            finish_node(sub, writer, state)?;
        }
    }
    Ok(())
}

fn format_expression(
    node: &SyntaxNode,
    writer: &mut impl TokenWriter,
    state: &mut FormatState,
) -> Result<(), std::io::Error> {
    let children: Vec<_> = node.children_with_tokens().collect();
    let mut saw_non_ws = false;

    for (i, n) in children.iter().enumerate() {
        if n.kind() == SyntaxKind::Whitespace {
            if saw_non_ws
                && children[i + 1..]
                    .iter()
                    .any(|c| !matches!(c.kind(), SyntaxKind::Whitespace | SyntaxKind::Comment))
                && n.as_token().is_some_and(|t| t.text().contains('\n'))
            {
                state.whitespace_to_add = None;
                state.indentation_level += 1;
                state.new_line();
                state.indentation_level -= 1;
            }
            state.skip_all_whitespace = true;
            fold(n.clone(), writer, state)?;
        } else {
            saw_non_ws = true;
            state.skip_all_whitespace = true;
            fold(n.clone(), writer, state)?;
        }
    }
    Ok(())
}

fn format_codeblock(
    node: &SyntaxNode,
    writer: &mut impl TokenWriter,
    state: &mut FormatState,
) -> Result<(), std::io::Error> {
    if node.first_child_or_token().is_none() {
        // empty CodeBlock happens when there is no `else` for example
        return Ok(());
    }

    // Keep a code block on a single line if the source had it on a single line.
    if !node.text().contains_char('\n') {
        return format_codeblock_single_line(node, writer, state);
    }

    let mut sub = node.children_with_tokens();
    if !whitespace_to(&mut sub, SyntaxKind::LBrace, writer, state, "")? {
        finish_node(sub, writer, state)?;
        return Ok(());
    }
    state.indentation_level += 1;
    state.new_line();
    for n in sub {
        state.skip_all_whitespace = true;
        if n.kind() == SyntaxKind::Whitespace {
            let is_empty_line =
                n.as_token().map(|n| whitespace_has_empty_line(n.text())).unwrap_or(false);
            if is_empty_line {
                state.new_line();
            }
        } else if n.kind() == SyntaxKind::RBrace {
            state.indentation_level -= 1;
            state.whitespace_to_add = None;
            state.new_line();
        }

        let is_semicolon = n.kind() == SyntaxKind::Semicolon;

        fold(n, writer, state)?;

        if is_semicolon {
            state.whitespace_to_add = None;
            state.new_line();
        }
    }
    state.skip_all_whitespace = true;
    Ok(())
}

fn format_codeblock_single_line(
    node: &SyntaxNode,
    writer: &mut impl TokenWriter,
    state: &mut FormatState,
) -> Result<(), std::io::Error> {
    // Is there anything between `{` and `}` other than trivia?
    let has_content = node.children_with_tokens().any(|n| {
        !matches!(n.kind(), SyntaxKind::Whitespace | SyntaxKind::LBrace | SyntaxKind::RBrace)
    });

    let mut sub = node.children_with_tokens();
    if !whitespace_to(&mut sub, SyntaxKind::LBrace, writer, state, "")? {
        finish_node(sub, writer, state)?;
        return Ok(());
    }
    let was_inline = std::mem::replace(&mut state.inline_codeblock, true);
    if has_content {
        state.insert_whitespace(" ");
    }
    for n in sub {
        state.skip_all_whitespace = true;
        if n.kind() == SyntaxKind::RBrace {
            state.whitespace_to_add = None;
            if has_content {
                // ensure a space even if the previous token was a comment
                state.after_comment = false;
                state.insert_whitespace(" ");
            }
        }

        let is_semicolon = n.kind() == SyntaxKind::Semicolon;

        fold(n, writer, state)?;

        if is_semicolon {
            state.whitespace_to_add = None;
            state.insert_whitespace(" ");
        }
        // Don't preserve whitespace following a comment verbatim — the next
        // iteration should normalize it to a single space too.
        state.after_comment = false;
    }
    state.skip_all_whitespace = true;
    state.inline_codeblock = was_inline;
    Ok(())
}

fn format_return_statement(
    node: &SyntaxNode,
    writer: &mut impl TokenWriter,
    state: &mut FormatState,
) -> Result<(), std::io::Error> {
    let mut sub = node.children_with_tokens();
    whitespace_to(&mut sub, SyntaxKind::Identifier, writer, state, "")?;
    if node.child_node(SyntaxKind::Expression).is_some() {
        whitespace_to(&mut sub, SyntaxKind::Expression, writer, state, " ")?;
    }
    whitespace_to(&mut sub, SyntaxKind::Semicolon, writer, state, "")?;
    state.new_line();
    finish_node(sub, writer, state)?;
    Ok(())
}

fn format_let_statement(
    node: &SyntaxNode,
    writer: &mut impl TokenWriter,
    state: &mut FormatState,
) -> Result<(), std::io::Error> {
    let mut sub = node.children_with_tokens();
    whitespace_to(&mut sub, SyntaxKind::Identifier, writer, state, "")?; // "let"
    whitespace_to(&mut sub, SyntaxKind::DeclaredIdentifier, writer, state, " ")?;
    // if type annotated
    if node.child_token(SyntaxKind::Colon).is_some() {
        whitespace_to(&mut sub, SyntaxKind::Colon, writer, state, "")?;
        if node.child_node(SyntaxKind::Type).is_some() {
            whitespace_to(&mut sub, SyntaxKind::Type, writer, state, " ")?;
        }
    }
    whitespace_to(&mut sub, SyntaxKind::Equal, writer, state, " ")?;
    whitespace_to(&mut sub, SyntaxKind::Expression, writer, state, " ")?;
    whitespace_to(&mut sub, SyntaxKind::Semicolon, writer, state, "")?;
    state.new_line();
    finish_node(sub, writer, state)?;
    Ok(())
}

fn format_at_gradient(
    node: &SyntaxNode,
    writer: &mut impl TokenWriter,
    state: &mut FormatState,
) -> Result<(), std::io::Error> {
    // ensure that two consecutive expression are separated with space
    let mut seen_expression = false;
    for n in node.children_with_tokens() {
        state.skip_all_whitespace = true;
        if n.kind() == SyntaxKind::Expression {
            if seen_expression {
                state.insert_whitespace(" ");
            }
            seen_expression = true;
        }
        fold(n, writer, state)?;
    }
    Ok(())
}

fn format_children_placeholder(
    node: &SyntaxNode,
    writer: &mut impl TokenWriter,
    state: &mut FormatState,
) -> Result<(), std::io::Error> {
    for n in node.children_with_tokens() {
        fold(n, writer, state)?;
    }
    state.new_line();
    Ok(())
}

fn format_repeated_element(
    node: &SyntaxNode,
    writer: &mut impl TokenWriter,
    state: &mut FormatState,
) -> Result<(), std::io::Error> {
    let mut sub = node.children_with_tokens();
    whitespace_to(&mut sub, SyntaxKind::Identifier, writer, state, "")?;
    whitespace_to(&mut sub, SyntaxKind::DeclaredIdentifier, writer, state, " ")?;

    let (kind, prefix_whitespace) = if node.child_node(SyntaxKind::RepeatedIndex).is_some() {
        (SyntaxKind::RepeatedIndex, "")
    } else {
        (SyntaxKind::Identifier, " ")
    };
    whitespace_to(&mut sub, kind, writer, state, prefix_whitespace)?;

    if kind == SyntaxKind::RepeatedIndex {
        whitespace_to(&mut sub, SyntaxKind::Identifier, writer, state, " ")?;
    }

    whitespace_to(&mut sub, SyntaxKind::Expression, writer, state, " ")?;
    whitespace_to(&mut sub, SyntaxKind::Colon, writer, state, "")?;
    state.insert_whitespace(" ");
    state.skip_all_whitespace = true;

    for s in sub {
        fold(s, writer, state)?;
    }
    Ok(())
}

fn format_match_element(
    node: &SyntaxNode,
    writer: &mut impl TokenWriter,
    state: &mut FormatState,
) -> Result<(), std::io::Error> {
    let mut sub = node.children_with_tokens();
    whitespace_to(&mut sub, SyntaxKind::Identifier, writer, state, "")?;
    whitespace_to(&mut sub, SyntaxKind::Expression, writer, state, " ")?;
    whitespace_to(&mut sub, SyntaxKind::LBrace, writer, state, " ")?;

    state.indentation_level += 1;
    state.new_line();

    for s in sub {
        if s.kind() == SyntaxKind::RBrace {
            state.indentation_level -= 1;
            state.whitespace_to_add = None;
            state.new_line();
            fold(s, writer, state)?;
            state.new_line();
        } else {
            fold(s, writer, state)?;
        }
    }
    Ok(())
}

fn format_match_case(
    node: &SyntaxNode,
    writer: &mut impl TokenWriter,
    state: &mut FormatState,
) -> Result<(), std::io::Error> {
    let mut sub = node.children_with_tokens().peekable();
    whitespace_to(&mut sub, SyntaxKind::Expression, writer, state, "")?;
    whitespace_to(&mut sub, SyntaxKind::Colon, writer, state, "")?;
    if node.child_node(SyntaxKind::SubElement).is_none() {
        whitespace_to(&mut sub, SyntaxKind::LBrace, writer, state, " ")?;
        whitespace_to(&mut sub, SyntaxKind::RBrace, writer, state, " ")?;
        state.skip_all_whitespace = true;
        state.new_line();
    } else {
        state.insert_whitespace(" ");
        state.skip_all_whitespace = true;
        for s in sub {
            fold(s, writer, state)?;
        }
    }
    Ok(())
}

fn format_repeated_index(
    node: &SyntaxNode,
    writer: &mut impl TokenWriter,
    state: &mut FormatState,
) -> Result<(), std::io::Error> {
    let mut sub = node.children_with_tokens();
    whitespace_to(&mut sub, SyntaxKind::LBracket, writer, state, "")?;
    whitespace_to(&mut sub, SyntaxKind::Identifier, writer, state, "")?;
    whitespace_to(&mut sub, SyntaxKind::RBracket, writer, state, "")?;
    Ok(())
}

fn format_array(
    node: &SyntaxNode,
    writer: &mut impl TokenWriter,
    state: &mut FormatState,
) -> Result<(), std::io::Error> {
    let has_trailing_comma = node
        .last_token()
        .and_then(|last| last.prev_token())
        .and_then(prev_non_trivia_token_kind)
        .map(|kind| kind == SyntaxKind::Comma)
        .unwrap_or(false);
    // len of all children
    let len = node.children().fold(0, |acc, e| {
        let mut len = 0;
        e.text().for_each_chunk(|s| len += s.trim().len());
        acc + len
    });
    // add , and 1-space len for each
    // not really accurate, e.g., [1] should have len 1, but due to this
    // it will be 3, but it doesn't matter
    let len = len + (2 * node.children().count());
    let is_large_array = len >= 80;
    let mut sub = node.children_with_tokens().peekable();
    whitespace_to(&mut sub, SyntaxKind::LBracket, writer, state, "")?;

    let is_empty_array = node.children().count() == 0;
    if is_empty_array {
        whitespace_to(&mut sub, SyntaxKind::RBracket, writer, state, "")?;
        return Ok(());
    }

    if is_large_array || has_trailing_comma {
        state.indentation_level += 1;
        state.new_line();
    }

    loop {
        whitespace_to(&mut sub, SyntaxKind::Expression, writer, state, "")?;
        let at_end = sub.peek().map(|next| next.kind() == SyntaxKind::RBracket).unwrap_or(false);
        if at_end && is_large_array {
            state.indentation_level -= 1;
            state.new_line();
        }

        let el = whitespace_to_one_of(
            &mut sub,
            &[SyntaxKind::RBracket, SyntaxKind::Comma],
            writer,
            state,
            "",
        )?;

        match el {
            SyntaxMatch::Found(SyntaxKind::RBracket) => break,
            SyntaxMatch::Found(SyntaxKind::Comma) => {
                let is_trailing_comma = sub
                    .peek()
                    .map(|next| {
                        if next.kind() == SyntaxKind::Whitespace {
                            next.as_token()
                                .and_then(|ws| ws.next_token())
                                .map(|n| n.kind() == SyntaxKind::RBracket)
                                .unwrap_or(false)
                        } else {
                            next.kind() == SyntaxKind::RBracket
                        }
                    })
                    .unwrap_or(false);

                if is_trailing_comma {
                    state.indentation_level -= 1;
                    state.new_line();
                    whitespace_to(&mut sub, SyntaxKind::RBracket, writer, state, "")?;
                    break;
                }
                if is_large_array || has_trailing_comma {
                    state.new_line();
                } else {
                    state.insert_whitespace(" ");
                }
            }
            SyntaxMatch::NotFound | SyntaxMatch::Found(_) => {
                tracing::warn!("Inconsistency: unexpected syntax in array.");
                break;
            }
        }
    }

    Ok(())
}

fn format_state(
    node: &SyntaxNode,
    writer: &mut impl TokenWriter,
    state: &mut FormatState,
) -> Result<(), std::io::Error> {
    let has_when = node.child_text(SyntaxKind::Identifier).is_some_and(|x| x == "when");
    let mut sub = node.children_with_tokens();
    let ok = if has_when {
        whitespace_to(&mut sub, SyntaxKind::DeclaredIdentifier, writer, state, "")?
            && whitespace_to(&mut sub, SyntaxKind::Identifier, writer, state, " ")?
            && whitespace_to(&mut sub, SyntaxKind::Expression, writer, state, " ")?
            && whitespace_to(&mut sub, SyntaxKind::Colon, writer, state, "")?
            && whitespace_to(&mut sub, SyntaxKind::LBrace, writer, state, " ")?
    } else {
        whitespace_to(&mut sub, SyntaxKind::DeclaredIdentifier, writer, state, "")?
            && whitespace_to(&mut sub, SyntaxKind::Colon, writer, state, "")?
            && whitespace_to(&mut sub, SyntaxKind::LBrace, writer, state, " ")?
    };
    if !ok {
        finish_node(sub, writer, state)?;
        return Ok(());
    }

    state.new_line();
    let ins_ctn = state.insertion_count;
    let mut first = true;

    for n in sub {
        if matches!(n.kind(), SyntaxKind::StatePropertyChange | SyntaxKind::Transition) {
            if first {
                // add new line after brace + increase indent
                state.indentation_level += 1;
                state.whitespace_to_add = None;
                state.new_line();
            }
            first = false;
            fold(n, writer, state)?;
        } else if n.kind() == SyntaxKind::RBrace {
            if !first {
                state.indentation_level -= 1;
            }
            state.whitespace_to_add = None;
            if ins_ctn == state.insertion_count {
                state.insert_whitespace("");
            } else {
                state.new_line();
            }
            fold(n, writer, state)?;
            state.new_line();
        } else {
            fold(n, writer, state)?;
        }
    }

    Ok(())
}

fn format_states(
    node: &SyntaxNode,
    writer: &mut impl TokenWriter,
    state: &mut FormatState,
) -> Result<(), std::io::Error> {
    let mut sub = node.children_with_tokens();
    let ok = whitespace_to(&mut sub, SyntaxKind::Identifier, writer, state, "")?
        && whitespace_to(&mut sub, SyntaxKind::LBracket, writer, state, " ")?;

    if !ok {
        tracing::warn!("Inconsistency: Expect states and ']'");
        return Ok(());
    }

    state.indentation_level += 1;
    state.new_line();

    for n in sub {
        if n.kind() == SyntaxKind::RBracket {
            state.whitespace_to_add = None;
            state.indentation_level -= 1;
            state.new_line();
        }
        fold(n, writer, state)?;
    }
    state.new_line();
    Ok(())
}

fn format_state_prop_change(
    node: &SyntaxNode,
    writer: &mut impl TokenWriter,
    state: &mut FormatState,
) -> Result<(), std::io::Error> {
    let mut sub = node.children_with_tokens();
    let _ok = whitespace_to(&mut sub, SyntaxKind::QualifiedName, writer, state, "")?
        && whitespace_to(&mut sub, SyntaxKind::Colon, writer, state, "")?
        && whitespace_to(&mut sub, SyntaxKind::BindingExpression, writer, state, " ")?;

    for n in sub {
        state.skip_all_whitespace = true;
        fold(n, writer, state)?;
    }
    state.new_line();
    Ok(())
}

fn format_transition(
    node: &SyntaxNode,
    writer: &mut impl TokenWriter,
    state: &mut FormatState,
) -> Result<(), std::io::Error> {
    let mut sub = node.children_with_tokens();
    let ok = whitespace_to(&mut sub, SyntaxKind::Identifier, writer, state, "")?
        && whitespace_to(&mut sub, SyntaxKind::LBrace, writer, state, " ")?;

    if !ok {
        finish_node(sub, writer, state)?;
        return Ok(());
    }
    state.indentation_level += 1;
    state.new_line();
    for n in sub {
        if n.kind() == SyntaxKind::RBrace {
            state.indentation_level -= 1;
            state.whitespace_to_add = None;
            state.new_line();
            fold(n, writer, state)?;
            state.new_line();
        } else {
            fold(n, writer, state)?;
        }
    }
    Ok(())
}

fn format_property_animation(
    node: &SyntaxNode,
    writer: &mut impl TokenWriter,
    state: &mut FormatState,
) -> Result<(), std::io::Error> {
    let mut sub = node.children_with_tokens().peekable();
    let _ok = whitespace_to(&mut sub, SyntaxKind::Identifier, writer, state, "")?
        && whitespace_to(&mut sub, SyntaxKind::QualifiedName, writer, state, " ")?;

    loop {
        let next_kind = sub.peek().map(|n| n.kind()).unwrap_or(SyntaxKind::Error);
        match next_kind {
            SyntaxKind::Whitespace | SyntaxKind::Comment => {
                let n = sub.next().unwrap();
                state.skip_all_whitespace = true;
                fold(n, writer, state)?;
                continue;
            }
            SyntaxKind::Comma => {
                whitespace_to(&mut sub, SyntaxKind::Comma, writer, state, "")?;
                continue;
            }
            SyntaxKind::QualifiedName => {
                whitespace_to(&mut sub, SyntaxKind::QualifiedName, writer, state, " ")?;
                continue;
            }
            SyntaxKind::LBrace => {
                whitespace_to(&mut sub, SyntaxKind::LBrace, writer, state, " ")?;
                break;
            }
            _ => break,
        }
    }

    let bindings = node.children().fold(0, |acc, e| {
        if e.kind() == SyntaxKind::Binding {
            return acc + 1;
        }
        acc
    });

    if bindings > 1 {
        state.indentation_level += 1;
        state.new_line();
    } else {
        state.insert_whitespace(" ");
    }

    for n in sub {
        if n.kind() == SyntaxKind::RBrace {
            state.whitespace_to_add = None;
            if bindings > 1 {
                state.indentation_level -= 1;
                state.new_line();
            } else {
                state.insert_whitespace(" ");
            }
            fold(n, writer, state)?;
            state.new_line();
        } else {
            fold(n, writer, state)?;
        }
    }
    Ok(())
}

fn format_object_literal(
    node: &SyntaxNode,
    writer: &mut impl TokenWriter,
    state: &mut FormatState,
) -> Result<(), std::io::Error> {
    let has_trailing_comma = node
        .last_token()
        .and_then(|last| last.prev_token())
        .and_then(prev_non_trivia_token_kind)
        .map(|kind| kind == SyntaxKind::Comma)
        .unwrap_or(false);
    // len of all children
    let len = node.children().fold(0, |acc, e| {
        let mut len = 0;
        e.text().for_each_chunk(|s| len += s.trim().len());
        acc + len
    });
    let is_large_literal = len >= 80;

    let mut sub = node.children_with_tokens().peekable();
    whitespace_to(&mut sub, SyntaxKind::LBrace, writer, state, "")?;
    let indent_with_new_line = is_large_literal || has_trailing_comma;

    if indent_with_new_line {
        state.indentation_level += 1;
        state.new_line();
    } else {
        state.insert_whitespace(" ");
    }

    loop {
        let el = whitespace_to_one_of(
            &mut sub,
            &[SyntaxKind::ObjectMember, SyntaxKind::RBrace],
            writer,
            state,
            "",
        )?;

        if let SyntaxMatch::Found(SyntaxKind::ObjectMember) = el {
            if indent_with_new_line {
                state.after_comment = false;
                state.new_line();
            } else {
                state.insert_whitespace(" ");
            }

            // are we at the end of literal?
            let at_end = sub
                .peek()
                .and_then(next_non_trivia_token_kind)
                .map(|kind| kind == SyntaxKind::RBrace)
                .unwrap_or(false);

            if at_end && indent_with_new_line {
                state.indentation_level -= 1;
                state.whitespace_to_add = None;
                state.new_line();
            }

            continue;
        } else if let SyntaxMatch::Found(SyntaxKind::RBrace) = el {
            break;
        } else {
            tracing::warn!("Inconsistency: unexpected syntax in object literal.");
            break;
        }
    }
    Ok(())
}

fn format_struct_declaration(
    node: &SyntaxNode,
    writer: &mut impl TokenWriter,
    state: &mut FormatState,
) -> Result<(), std::io::Error> {
    let mut sub = node.children_with_tokens();
    let _ok = whitespace_to(&mut sub, SyntaxKind::Identifier, writer, state, "")?
        && whitespace_to(&mut sub, SyntaxKind::DeclaredIdentifier, writer, state, " ")?
        && whitespace_to(&mut sub, SyntaxKind::ObjectType, writer, state, " ")?;
    finish_node(sub, writer, state)?;
    state.new_line();
    Ok(())
}

fn format_enum_declaration(
    node: &SyntaxNode,
    writer: &mut impl TokenWriter,
    state: &mut FormatState,
) -> Result<(), std::io::Error> {
    let has_trailing_comma = node
        .last_token()
        .and_then(|last| last.prev_token())
        .and_then(prev_non_trivia_token_kind)
        .map(|kind| kind == SyntaxKind::Comma)
        .unwrap_or(false);
    let len = node.children().fold(0, |acc, e| {
        let mut len = 0;
        e.text().for_each_chunk(|s| len += s.trim().len());
        acc + len
    });
    let is_large = len >= 80;
    let indent_with_new_line = is_large || has_trailing_comma;

    let mut sub = node.children_with_tokens().peekable();
    // `enum`
    whitespace_to(&mut sub, SyntaxKind::Identifier, writer, state, "")?;
    // name
    whitespace_to(&mut sub, SyntaxKind::DeclaredIdentifier, writer, state, " ")?;
    // `{`
    whitespace_to(&mut sub, SyntaxKind::LBrace, writer, state, " ")?;

    if indent_with_new_line {
        state.indentation_level += 1;
        state.new_line();
    } else {
        state.insert_whitespace(" ");
    }

    loop {
        let el = whitespace_to_one_of(
            &mut sub,
            &[SyntaxKind::EnumValue, SyntaxKind::RBrace],
            writer,
            state,
            "",
        )?;

        match el {
            SyntaxMatch::Found(SyntaxKind::EnumValue) => {
                // Check if the next non-whitespace token is RBrace (no comma after last value)
                let next_is_rbrace = sub
                    .peek()
                    .map(|next| {
                        if next.kind() == SyntaxKind::Whitespace {
                            next.as_token()
                                .and_then(|ws| ws.next_token())
                                .map(|n| n.kind() == SyntaxKind::RBrace)
                                .unwrap_or(false)
                        } else {
                            next.kind() == SyntaxKind::RBrace
                        }
                    })
                    .unwrap_or(false);

                if next_is_rbrace {
                    whitespace_to(&mut sub, SyntaxKind::RBrace, writer, state, " ")?;
                    break;
                }

                let comma = whitespace_to_one_of(
                    &mut sub,
                    &[SyntaxKind::Comma, SyntaxKind::RBrace],
                    writer,
                    state,
                    "",
                )?;
                match comma {
                    SyntaxMatch::Found(SyntaxKind::RBrace) => break,
                    SyntaxMatch::Found(SyntaxKind::Comma) => {
                        let is_trailing = sub
                            .peek()
                            .map(|next| {
                                if next.kind() == SyntaxKind::Whitespace {
                                    next.as_token()
                                        .and_then(|ws| ws.next_token())
                                        .map(|n| n.kind() == SyntaxKind::RBrace)
                                        .unwrap_or(false)
                                } else {
                                    next.kind() == SyntaxKind::RBrace
                                }
                            })
                            .unwrap_or(false);

                        if is_trailing {
                            if indent_with_new_line {
                                state.indentation_level -= 1;
                            }
                            state.new_line();
                            whitespace_to(&mut sub, SyntaxKind::RBrace, writer, state, "")?;
                            break;
                        }
                        if indent_with_new_line {
                            state.new_line();
                        } else {
                            state.insert_whitespace(" ");
                        }
                    }
                    _ => break,
                }
            }
            SyntaxMatch::Found(SyntaxKind::RBrace) => {
                if indent_with_new_line {
                    state.indentation_level -= 1;
                }
                break;
            }
            _ => break,
        }
    }
    state.skip_all_whitespace = true;
    finish_node(sub, writer, state)?;
    state.new_line();
    Ok(())
}

fn format_exports_list(
    node: &SyntaxNode,
    writer: &mut impl TokenWriter,
    state: &mut FormatState,
) -> Result<(), std::io::Error> {
    // ExportsList may contain `export { Foo, Bar }` or `export component ...` etc.
    // Only handle the brace-list case specially; otherwise fall through.
    let has_lbrace = node.children_with_tokens().any(|n| n.kind() == SyntaxKind::LBrace);
    if !has_lbrace {
        // `export component ...` or `export struct ...` — delegate to default
        for n in node.children_with_tokens() {
            fold(n, writer, state)?;
        }
        return Ok(());
    }

    let has_trailing_comma = node
        .children_with_tokens()
        .filter_map(|n| n.into_token())
        .find(|t| t.kind() == SyntaxKind::RBrace)
        .and_then(|rbrace| rbrace.prev_token())
        .and_then(prev_non_trivia_token_kind)
        .map(|kind| kind == SyntaxKind::Comma)
        .unwrap_or(false);
    let is_large = node.text().len() > 80.into();
    let indent_with_new_line = is_large || has_trailing_comma;

    let mut sub = node.children_with_tokens().peekable();
    // `export`
    whitespace_to(&mut sub, SyntaxKind::Identifier, writer, state, "")?;
    // `{`
    whitespace_to(&mut sub, SyntaxKind::LBrace, writer, state, " ")?;

    if indent_with_new_line {
        state.indentation_level += 1;
        state.new_line();
    } else {
        state.insert_whitespace(" ");
    }

    loop {
        let el = whitespace_to_one_of(
            &mut sub,
            &[SyntaxKind::ExportSpecifier, SyntaxKind::RBrace],
            writer,
            state,
            "",
        )?;

        match el {
            SyntaxMatch::Found(SyntaxKind::ExportSpecifier) => {
                // Check if the next non-whitespace token is RBrace (no comma after last specifier)
                let next_is_rbrace = sub
                    .peek()
                    .map(|next| {
                        if next.kind() == SyntaxKind::Whitespace {
                            next.as_token()
                                .and_then(|ws| ws.next_token())
                                .map(|n| n.kind() == SyntaxKind::RBrace)
                                .unwrap_or(false)
                        } else {
                            next.kind() == SyntaxKind::RBrace
                        }
                    })
                    .unwrap_or(false);

                if next_is_rbrace {
                    if indent_with_new_line {
                        writer.insert_content(",")?;
                        state.indentation_level -= 1;
                        state.new_line();
                        whitespace_to(&mut sub, SyntaxKind::RBrace, writer, state, "")?;
                    } else {
                        whitespace_to(&mut sub, SyntaxKind::RBrace, writer, state, " ")?;
                    }
                    break;
                }

                let comma = whitespace_to_one_of(
                    &mut sub,
                    &[SyntaxKind::Comma, SyntaxKind::RBrace],
                    writer,
                    state,
                    "",
                )?;
                match comma {
                    SyntaxMatch::Found(SyntaxKind::RBrace) => {
                        if indent_with_new_line {
                            state.indentation_level -= 1;
                        }
                        break;
                    }
                    SyntaxMatch::Found(SyntaxKind::Comma) => {
                        let is_trailing = sub
                            .peek()
                            .map(|next| {
                                if next.kind() == SyntaxKind::Whitespace {
                                    next.as_token()
                                        .and_then(|ws| ws.next_token())
                                        .map(|n| n.kind() == SyntaxKind::RBrace)
                                        .unwrap_or(false)
                                } else {
                                    next.kind() == SyntaxKind::RBrace
                                }
                            })
                            .unwrap_or(false);

                        if is_trailing {
                            if indent_with_new_line {
                                state.indentation_level -= 1;
                            }
                            state.new_line();
                            whitespace_to(&mut sub, SyntaxKind::RBrace, writer, state, "")?;
                            break;
                        }
                        if indent_with_new_line {
                            state.new_line();
                        } else {
                            state.insert_whitespace(" ");
                        }
                    }
                    _ => break,
                }
            }
            SyntaxMatch::Found(SyntaxKind::RBrace) => {
                if indent_with_new_line {
                    state.indentation_level -= 1;
                }
                break;
            }
            _ => break,
        }
    }
    state.skip_all_whitespace = true;
    finish_node(sub, writer, state)?;
    state.new_line();
    Ok(())
}

fn format_export_specifier(
    node: &SyntaxNode,
    writer: &mut impl TokenWriter,
    state: &mut FormatState,
) -> Result<(), std::io::Error> {
    let has_as = node.children_with_tokens().any(|n| {
        n.kind() == SyntaxKind::Identifier && n.as_token().is_some_and(|t| t.text() == "as")
    });

    let mut sub = node.children_with_tokens();
    whitespace_to(&mut sub, SyntaxKind::ExportIdentifier, writer, state, "")?;
    if has_as {
        whitespace_to(&mut sub, SyntaxKind::Identifier, writer, state, " ")?;
        whitespace_to(&mut sub, SyntaxKind::ExportName, writer, state, " ")?;
    }
    state.skip_all_whitespace = true;
    finish_node(sub, writer, state)?;
    Ok(())
}

fn format_object_type(
    node: &SyntaxNode,
    writer: &mut impl TokenWriter,
    state: &mut FormatState,
) -> Result<(), std::io::Error> {
    // Mirror object literal formatting, but for type definitions
    let has_trailing_comma = node
        .last_token()
        .and_then(|last| last.prev_token())
        .and_then(prev_non_trivia_token_kind)
        .map(|kind| kind == SyntaxKind::Comma)
        .unwrap_or(false);
    let len = node.children().fold(0, |acc, e| {
        let mut len = 0;
        e.text().for_each_chunk(|s| len += s.trim().len());
        acc + len
    });
    let is_large_object = len >= 80;
    let has_comment = node.descendants_with_tokens().any(|n| n.kind() == SyntaxKind::Comment);
    let member_count = node.children().filter(|n| n.kind() == SyntaxKind::ObjectTypeMember).count();

    let mut sub = node.children_with_tokens().peekable();
    whitespace_to(&mut sub, SyntaxKind::LBrace, writer, state, "")?;
    // Trailing commas, long content, or comments keep braces on separate lines.
    let indent_with_new_line = is_large_object || has_trailing_comma || has_comment;

    if indent_with_new_line {
        state.indentation_level += 1;
        state.new_line();
    } else if member_count > 1 {
        // Multi-field inline structs keep a space after `{`.
        state.insert_whitespace(" ");
    }

    loop {
        let el = whitespace_to_one_of(
            &mut sub,
            &[SyntaxKind::ObjectTypeMember, SyntaxKind::RBrace],
            writer,
            state,
            "",
        )?;

        if let SyntaxMatch::Found(SyntaxKind::ObjectTypeMember) = el {
            let at_end = sub
                .peek()
                .and_then(next_non_trivia_token_kind)
                .map(|kind| kind == SyntaxKind::RBrace)
                .unwrap_or(false);

            if indent_with_new_line {
                state.new_line();
            } else if !at_end || member_count > 1 {
                state.insert_whitespace(" ");
            }

            if at_end && indent_with_new_line {
                state.indentation_level -= 1;
                state.whitespace_to_add = None;
                state.new_line();
            }

            continue;
        } else if let SyntaxMatch::Found(SyntaxKind::RBrace) = el {
            break;
        } else {
            tracing::warn!("Inconsistency: unexpected syntax in object type.");
            break;
        }
    }
    Ok(())
}

fn format_object_type_member(
    node: &SyntaxNode,
    writer: &mut impl TokenWriter,
    state: &mut FormatState,
) -> Result<(), std::io::Error> {
    // Format a single struct field: `name: Type` (comma handled if present).
    let mut sub = node.children_with_tokens();
    let _ok = whitespace_to(&mut sub, SyntaxKind::Identifier, writer, state, "")?
        && whitespace_to(&mut sub, SyntaxKind::Colon, writer, state, "")?
        && whitespace_to(&mut sub, SyntaxKind::Type, writer, state, " ")?;
    if node.child_token(SyntaxKind::Comma).is_some() {
        whitespace_to(&mut sub, SyntaxKind::Comma, writer, state, "")?;
    }
    // Strip trailing whitespace unless it belongs to a trailing comment.
    if !state.after_comment {
        state.skip_all_whitespace = true;
    }
    finish_node(sub, writer, state)?;
    Ok(())
}

fn format_property_changed_callback(
    node: &SyntaxNode,
    writer: &mut impl TokenWriter,
    state: &mut FormatState,
) -> Result<(), std::io::Error> {
    let mut sub = node.children_with_tokens().peekable();
    let _ok = whitespace_to(&mut sub, SyntaxKind::Identifier, writer, state, "")?
        && whitespace_to(&mut sub, SyntaxKind::DeclaredIdentifier, writer, state, " ")?
        && whitespace_to(&mut sub, SyntaxKind::FatArrow, writer, state, " ")?
        && whitespace_to(&mut sub, SyntaxKind::CodeBlock, writer, state, " ")?;
    for n in sub {
        fold(n, writer, state)?;
    }
    state.new_line();
    Ok(())
}

fn format_member_access(
    node: &SyntaxNode,
    writer: &mut impl TokenWriter,
    state: &mut FormatState,
) -> Result<(), std::io::Error> {
    let n = syntax_nodes::MemberAccess::from(node.clone());
    // Special case of things like `42 .mod(x)` where a space is needed otherwise it lexes differently
    let need_space = n.Expression().child_token(SyntaxKind::NumberLiteral).is_some_and(|nl| {
        !nl.text().contains('.') && nl.text().chars().last().is_some_and(|c| c.is_numeric())
    });
    let space_before_dot = if need_space { " " } else { "" };
    let mut sub = n.children_with_tokens();
    let _ok = whitespace_to(&mut sub, SyntaxKind::Expression, writer, state, "")?
        && whitespace_to(&mut sub, SyntaxKind::Dot, writer, state, space_before_dot)?
        && whitespace_to(&mut sub, SyntaxKind::Identifier, writer, state, "")?;
    for n in sub {
        fold(n, writer, state)?;
    }
    Ok(())
}

/// Formats an import specifier.
///
/// This catches the whole import line (or multiline), and it checks its size only,
/// to later delegate to import_identifier
///
/// Given this line:
/// ```slint
/// import { foo, bar, } from 'module';
/// ```
///
/// It calculates the size of the line and passes the identifier list to the next formatter:
/// ```slint
/// { foo, bar, }
/// ```
///
/// The formatter will then execute the code, and in this case, output:
/// ```slint
/// {
///     foo,
///     bar,
/// }
/// ```
fn format_import_specifier(
    node: &SyntaxNode,
    writer: &mut impl TokenWriter,
    state: &mut FormatState,
) -> Result<(), std::io::Error> {
    let is_too_long = node.text().len() > 80.into();

    for n in node.children_with_tokens() {
        match n.kind() {
            SyntaxKind::ImportIdentifierList => {
                if let NodeOrToken::Node(n) = n {
                    format_import_identifier(&n, writer, state, is_too_long)?
                };
            }
            _ => {
                fold(n, writer, state)?;
            }
        }
    }

    Ok(())
}

/// Format import list
///
/// If the line is too long, it formats the import identifier in a new line, unless
/// there's a comment or the import has a trailing comma.
///
/// If the line is short, it just adds the spaces around the import identifier, unless
/// the import has a trailing comma.
fn format_import_identifier(
    node: &SyntaxNode,
    writer: &mut impl TokenWriter,
    state: &mut FormatState,
    is_too_long: bool,
) -> Result<(), std::io::Error> {
    let count_commas = node
        .children_with_tokens()
        .filter(|child_token| child_token.kind() == SyntaxKind::Comma)
        .count();
    let mut count_import_identifiers = node
        .children_with_tokens()
        .filter(|child_token| child_token.kind() == SyntaxKind::ImportIdentifier)
        .count();
    let has_comment =
        node.children_with_tokens().any(|child_token| child_token.kind() == SyntaxKind::Comment);
    let has_trailing_comma = count_commas == count_import_identifiers;
    let sub = node.children_with_tokens();

    // With stay_single_line we control whether to split the import statement or not
    let stay_single_line = !is_too_long && !has_trailing_comma && !has_comment;

    let indentation_level = if stay_single_line { 0 } else { 1 };
    for n in sub {
        state.skip_all_whitespace = true;
        match n.kind() {
            SyntaxKind::Whitespace => {
                fold(n, writer, state)?;
            }
            SyntaxKind::LBrace => {
                state.indentation_level += indentation_level;
                fold(n, writer, state)?;
            }
            SyntaxKind::ImportIdentifier => {
                count_import_identifiers -= 1;
                let is_last_import = count_import_identifiers == 0;
                if let Some(children) = n.as_node() {
                    let has_internal_name = children
                        .children_with_tokens()
                        .any(|child| child.kind() == SyntaxKind::InternalName);
                    for nn in children.children_with_tokens() {
                        state.skip_all_whitespace = true;
                        match nn.kind() {
                            SyntaxKind::Whitespace => {
                                fold(nn, writer, state)?;
                            }
                            SyntaxKind::ExternalName => {
                                if stay_single_line {
                                    state.insert_whitespace(" ");
                                } else {
                                    state.new_line();
                                }
                                fold(nn, writer, state)?;

                                // We don't want to add a comma after the
                                // external name if there is an internal name
                                if has_internal_name {
                                    continue;
                                }
                                if !stay_single_line && !has_trailing_comma && is_last_import {
                                    writer.insert_content(",")?;
                                }
                            }
                            SyntaxKind::InternalName => {
                                state.whitespace_to_add = Some(" ".into());
                                fold(nn, writer, state)?;

                                // Internal name is optional, and, if present,
                                // we know that the comma in external has been skipped,
                                // so we can add if needed
                                if !stay_single_line && !has_trailing_comma && is_last_import {
                                    writer.insert_content(",")?;
                                }
                            }
                            _ => {
                                state.whitespace_to_add = Some(" ".into());
                                fold(nn, writer, state)?;
                            }
                        }
                    }
                }
            }
            SyntaxKind::RBrace => {
                state.indentation_level -= indentation_level;
                if !stay_single_line {
                    state.new_line();
                } else {
                    state.insert_whitespace(" ");
                }
                fold(n, writer, state)?;
            }
            _ => {
                state.skip_all_whitespace = true;
                fold(n, writer, state)?;
            }
        }
    }
    Ok(())
}

/// Formats a uses specifier.
///
/// Ensures that the QualifiedName and `from` Identifier are separated by a space.
fn format_uses_specifier(
    node: &SyntaxNode,
    writer: &mut impl TokenWriter,
    state: &mut FormatState,
) -> Result<(), std::io::Error> {
    let sub = node.children_with_tokens();
    for n in sub {
        match n.kind() {
            SyntaxKind::Whitespace => {
                fold(n, writer, state)?;
            }
            SyntaxKind::LBrace => {
                fold(n, writer, state)?;
            }
            SyntaxKind::UsesIdentifier => {
                if let Some(uses_node) = n.as_node() {
                    state.skip_all_whitespace = true;
                    for child in uses_node.children_with_tokens() {
                        match child.kind() {
                            SyntaxKind::Identifier => {
                                state.whitespace_to_add = Some(" ".into());
                                fold(child, writer, state)?;
                            }
                            _ => {
                                fold(child, writer, state)?;
                            }
                        }
                    }
                }
            }
            SyntaxKind::RBrace => {
                fold(n, writer, state)?;
            }
            _ => {
                state.skip_all_whitespace = true;
                fold(n, writer, state)?;
            }
        }
    }
    Ok(())
}

fn format_implements_specifier(
    node: &SyntaxNode,
    writer: &mut impl TokenWriter,
    state: &mut FormatState,
) -> Result<(), std::io::Error> {
    let sub = node.children_with_tokens();
    for n in sub {
        match n.kind() {
            SyntaxKind::Identifier | SyntaxKind::QualifiedName => {
                fold(n, writer, state)?;
                state.insert_whitespace(" ");
            }
            _ => {
                fold(n, writer, state)?;
            }
        }
    }
    Ok(())
}
