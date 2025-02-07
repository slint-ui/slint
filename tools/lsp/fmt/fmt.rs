// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

use super::writer::TokenWriter;
use i_slint_compiler::parser::{syntax_nodes, NodeOrToken, SyntaxKind, SyntaxNode};

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
}

impl FormatState {
    fn new_line(&mut self) {
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
        SyntaxKind::PropertyChangedCallback => {
            return format_property_changed_callback(node, writer, state);
        }
        SyntaxKind::MemberAccess => {
            return format_member_access(node, writer, state);
        }

        _ => (),
    }

    for n in node.children_with_tokens() {
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
                if let Some(ws) = state.last_removed_whitespace.take() {
                    if state.after_comment {
                        // restore the previously skipped spaces before comment.
                        state.insertion_count += 1;
                        writer.insert_before(t, &ws)?;
                        state.whitespace_to_add = None;
                        return Ok(());
                    }
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
                eprintln!("Inconsistency: expected {elements:?},  found {n:?}");
                fold(n, writer, state)?;
                return Ok(SyntaxMatch::NotFound);
            }
        }
        fold(n, writer, state)?;
    }
    eprintln!("Inconsistency: expected {elements:?},  not found");
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
            &[SyntaxKind::Identifier, SyntaxKind::Element],
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

    for n in sub {
        if n.kind() == SyntaxKind::Whitespace {
            let is_empty_line = n.as_token().map(|n| n.text().contains("\n\n")).unwrap_or(false);
            if is_empty_line && !inserted_newline {
                state.new_line();
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

// Called both for BinaryExpression and SelfAssignment
fn format_binary_expression(
    node: &SyntaxNode,
    writer: &mut impl TokenWriter,
    state: &mut FormatState,
) -> Result<(), std::io::Error> {
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
        while let Some(n) = sub.next() {
            state.skip_all_whitespace = true;
            // `else`
            if n.kind() == SyntaxKind::Identifier {
                state.insert_whitespace(" ");
                fold(n, writer, state)?;
                whitespace_to(&mut sub, SyntaxKind::Expression, writer, state, " ")?;
                continue;
            }
            fold(n, writer, state)?;
        }
        state.whitespace_to_add = None;
        state.new_line();
    } else {
        let _ok = whitespace_to(&mut sub, SyntaxKind::Expression, writer, state, "")?
            && whitespace_to(&mut sub, SyntaxKind::Question, writer, state, " ")?
            && whitespace_to(&mut sub, SyntaxKind::Expression, writer, state, " ")?
            && whitespace_to(&mut sub, SyntaxKind::Colon, writer, state, " ")?
            && whitespace_to(&mut sub, SyntaxKind::Expression, writer, state, " ")?;
        finish_node(sub, writer, state)?;
    }
    Ok(())
}

fn format_expression(
    node: &SyntaxNode,
    writer: &mut impl TokenWriter,
    state: &mut FormatState,
) -> Result<(), std::io::Error> {
    // For expressions, we skip whitespace.
    // Since state.skip_all_whitespace is reset every time we find something that is not a whitespace,
    // we need to set it all the time.

    for n in node.children_with_tokens() {
        state.skip_all_whitespace = true;
        fold(n, writer, state)?;
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
            let is_empty_line = n.as_token().map(|n| n.text().contains("\n\n")).unwrap_or(false);
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

fn format_return_statement(
    node: &SyntaxNode,
    writer: &mut impl TokenWriter,
    state: &mut FormatState,
) -> Result<(), std::io::Error> {
    let mut sub = node.children_with_tokens();
    whitespace_to(&mut sub, SyntaxKind::Identifier, writer, state, "")?;
    if node.child_node(SyntaxKind::Expression).is_some() {
        whitespace_to(&mut sub, SyntaxKind::Identifier, writer, state, " ")?;
    }
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
        .map(|second_last| {
            if second_last.kind() == SyntaxKind::Whitespace {
                second_last.prev_token().map(|n| n.kind() == SyntaxKind::Comma).unwrap_or(false)
            } else {
                second_last.kind() == SyntaxKind::Comma
            }
        })
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
                eprintln!("Inconsistency: unexpected syntax in array.");
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
        eprintln!("Inconsistency: Expect states and ']'");
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
        .map(|second_last| {
            if second_last.kind() == SyntaxKind::Whitespace {
                second_last.prev_token().map(|n| n.kind() == SyntaxKind::Comma).unwrap_or(false)
            } else {
                second_last.kind() == SyntaxKind::Comma
            }
        })
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
                state.new_line();
            } else {
                state.insert_whitespace(" ");
            }

            // are we at the end of literal?
            let at_end = sub
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

            if at_end && indent_with_new_line {
                state.indentation_level -= 1;
                state.whitespace_to_add = None;
                state.new_line();
            }

            continue;
        } else if let SyntaxMatch::Found(SyntaxKind::RBrace) = el {
            break;
        } else {
            eprintln!("Inconsistency: unexpected syntax in object literal.");
            break;
        }
    }
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fmt::writer::FileWriter;
    use i_slint_compiler::diagnostics::BuildDiagnostics;

    // FIXME more descriptive errors when an assertion fails
    #[track_caller]
    fn assert_formatting(unformatted: &str, formatted: &str) {
        // Parse the unformatted string
        let syntax_node = i_slint_compiler::parser::parse(
            String::from(unformatted),
            None,
            &mut BuildDiagnostics::default(),
        );
        // Turn the syntax node into a document
        let doc = syntax_nodes::Document::new(syntax_node).unwrap();
        let mut file = Vec::new();
        format_document(doc, &mut FileWriter { file: &mut file }).unwrap();
        assert_eq!(String::from_utf8(file).unwrap(), formatted);
    }

    #[test]
    fn basic_formatting() {
        assert_formatting("A:=Text{}", "A := Text { }\n");
    }

    #[test]
    fn components() {
        assert_formatting(
            "component   A   {}  export component  B  inherits  Text {  }",
            "component A { }\n\nexport component B inherits Text { }\n",
        );
    }

    #[test]
    fn with_comments() {
        assert_formatting(
            r#"component /* */ Foo // aaa
            inherits  /* x */  // bbb
            Window // ccc
            /*y*/ {   // c
              Foo /*aa*/ /*bb*/ {}
            }
        "#,
            r#"component /* */ Foo // aaa
            inherits  /* x */  // bbb
            Window // ccc
            /*y*/ {   // c
              Foo /*aa*/ /*bb*/ { }
}
"#,
        );

        assert_formatting(
            r#"//xxx
component  C1 {
    // before a property
    property< int> p1;
    property<int > p2; // on the same line of a property
    property <int > p3;
    // After a property
    // ...
}
"#,
            r#"//xxx
component C1 {
    // before a property
    property <int> p1;
    property <int> p2; // on the same line of a property
    property <int> p3;
    // After a property
    // ...
}
"#,
        );
    }

    #[test]
    fn complex_formatting() {
        assert_formatting(
            r#"
Main :=Window{callback some-fn(string,string)->bool;some-fn(a, b)=>{a<=b} property<bool>prop-x;
    VerticalBox {      combo:=ComboBox{}    }
    pure   callback   some-fn  ({x: int},string);  in property <  int >  foo: 42; }
            "#,
            r#"
Main := Window {
    callback some-fn(string, string) -> bool;
    some-fn(a, b) => {
        a <= b
    }
    property <bool> prop-x;
    VerticalBox {
        combo := ComboBox { }
    }

    pure callback some-fn({x: int}, string);
    in property <int> foo: 42;
}
"#,
        );
    }

    #[test]
    fn callback_declaration() {
        assert_formatting(
            r#"
component W inherits Window{
    callback hello( with-name :int , { x: int, y: float} , foo: string );
    callback world   (  )  -> string;
    callback another_callback ;
}
"#,
            r#"
component W inherits Window {
    callback hello(with-name: int, { x: int, y: float}, foo: string);
    callback world() -> string;
    callback another_callback;
}
"#,
        );
    }

    #[test]
    fn parent_access() {
        assert_formatting(
            r#"
Main := Parent{            Child{
    some-prop: parent.foo - 60px;
}}"#,
            r#"
Main := Parent {
    Child {
        some-prop: parent.foo - 60px;
    }
}
"#,
        );
    }

    #[test]
    fn space_with_braces() {
        assert_formatting("Main := Window{}", "Main := Window { }\n");
        // Also in a child
        assert_formatting(
            r#"
Main := Window{Child{}}"#,
            r#"
Main := Window {
    Child { }
}
"#,
        );
        assert_formatting(
            r#"
Main := VerticalLayout{HorizontalLayout{prop:3;}}"#,
            r#"
Main := VerticalLayout {
    HorizontalLayout {
        prop: 3;
    }
}
"#,
        );
    }

    #[test]
    fn binary_expressions() {
        assert_formatting(
            r#"
Main := Some{
    a:3+2;  b:4-7;  c:3*7;  d:3/9;
    e:3==4; f:3!=4; g:3<4;  h:3<=4;
    i:3>4;  j:3>=4; k:3&&4; l:3||4;
}"#,
            r#"
Main := Some {
    a: 3 + 2;
    b: 4 - 7;
    c: 3 * 7;
    d: 3 / 9;
    e: 3 == 4;
    f: 3 != 4;
    g: 3 < 4;
    h: 3 <= 4;
    i: 3 > 4;
    j: 3 >= 4;
    k: 3 && 4;
    l: 3 || 4;
}
"#,
        );

        assert_formatting(
            r#"
Main := Some{
    m: 3 + 8;
    m:3 + 8;
    m: 3+ 8;
    m:3+ 8;
    m: 3 +8;
    m:3 +8;
    m: 3+8;
    m:3+8;
}"#,
            r#"
Main := Some {
    m: 3 + 8;
    m: 3 + 8;
    m: 3 + 8;
    m: 3 + 8;
    m: 3 + 8;
    m: 3 + 8;
    m: 3 + 8;
    m: 3 + 8;
}
"#,
        );
    }

    #[test]
    fn file_with_an_import() {
        assert_formatting(
            r#"
import { Some } from "./here.slint";

A := Some{    padding-left: 10px;    Text{        x: 3px;    }}"#,
            r#"
import { Some } from "./here.slint";

A := Some {
    padding-left: 10px;
    Text {
        x: 3px;
    }
}
"#,
        );
    }

    #[test]
    fn children() {
        // Regression test - children was causing additional newlines
        assert_formatting(
            r#"
A := B {
    C {
        @children
    }
}"#,
            r#"
A := B {
    C {
        @children
    }
}
"#,
        );
    }

    #[test]
    fn for_in() {
        assert_formatting(
            r#"
A := B {  for c   in root.d: T  { e: c.attr; } }
        "#,
            r#"
A := B {
    for c in root.d: T {
        e: c.attr;
    }
}
"#,
        );
    }

    #[test]
    fn for_in_index() {
        // FIXME: there should not be a whitespace before [index]
        assert_formatting(
            r#"
A := B {
    for number   [  index
         ]  in [1,2,3]: C { d:number*index; }
}
        "#,
            r#"
A := B {
    for number[index] in [1, 2, 3]: C {
        d: number * index;
    }
}
"#,
        );
    }

    #[test]
    fn if_element() {
        assert_formatting(
            r#"
component A {  if condition : Text {  }  }
        "#,
            r#"
component A {
    if condition: Text { }
}
"#,
        );
    }

    #[test]
    fn array() {
        assert_formatting(
            r#"
A := B { c: [1,2,3]; }
"#,
            r#"
A := B {
    c: [1, 2, 3];
}
"#,
        );
        assert_formatting(
            r#"
A := B { c: [    1    ]; }
"#,
            r#"
A := B {
    c: [1];
}
"#,
        );
        assert_formatting(
            r#"
A := B { c:   [    ]  ; }
"#,
            r#"
A := B {
    c: [];
}
"#,
        );
        assert_formatting(
            r#"
A := B { c:   [

1,

2

  ]  ; }
"#,
            r#"
A := B {
    c: [1, 2];
}
"#,
        );
    }

    #[test]
    fn states() {
        assert_formatting(
            r#"
component FooBar {
    states [
        dummy1    when    a == true   :{

        }
    ]
}
"#,
            r#"
component FooBar {
    states [
        dummy1 when a == true: {}
    ]
}
"#,
        );

        assert_formatting(
            r#"
component ABC {
    in-out property <bool> b: false;
    in-out property <int> a: 1;
    states[
        is-selected when root.b == root.b: {
            b:false;
        root.a:1;
        }
        is-not-selected when root.b!=root.b: {
            root.a: 1;
        }
    ]    foo := Rectangle { }
}
"#,
            r#"
component ABC {
    in-out property <bool> b: false;
    in-out property <int> a: 1;
    states [
        is-selected when root.b == root.b: {
            b: false;
            root.a: 1;
        }
        is-not-selected when root.b != root.b: {
            root.a: 1;
        }
    ]
    foo := Rectangle { }
}
"#,
        );
    }

    #[test]
    fn state_issue_4850() {
        // #4850
        assert_formatting(
            "export component LspCrashMvp { states [ active: { } inactive: { } ] }",
            r#"export component LspCrashMvp {
    states [
        active: {}
        inactive: {}
    ]
}
"#,
        );
    }

    #[test]
    fn states_transitions() {
        assert_formatting(
            r#"
component FooBar {
    states [
        //comment
        s1 when true: {
    vv: 0;  in { animate vv {  duration: 400ms; }}  out { animate /*...*/  vv { duration:    400ms;   } animate dd { duration: 100ms+400ms ;  easing: ease-out; }   }  }
    ]
}
"#,
            r#"
component FooBar {
    states [
        //comment
        s1 when true: {
            vv: 0;
            in {
                animate vv { duration: 400ms; }
            }
            out {
                animate /*...*/  vv { duration: 400ms; }
                animate dd {
                    duration: 100ms + 400ms;
                    easing: ease-out;
                }
            }
        }
    ]
}
"#,
        );

        assert_formatting(
            "component FooBar {states[foo:{in{animate x{duration:1ms;}}x:0;}]}",
            r"component FooBar {
    states [
        foo: {
            in {
                animate x { duration: 1ms; }
            }
            x: 0;
        }
    ]
}
",
        );
    }

    #[test]
    fn if_else() {
        assert_formatting(
            r#"
A := B { c: true  ||false?  45 + 6:34+ 1; }
"#,
            r#"
A := B {
    c: true || false ? 45 + 6 : 34 + 1;
}
"#,
        );
        assert_formatting(
            r#"A := B { c => { if(!abc){nothing}else   if  (true){if (0== 8) {}    } else{  } } } "#,
            r#"A := B {
    c => {
        if (!abc) {
            nothing
        } else if (true) {
            if (0 == 8) {
            }
        } else {
        }
    }
}
"#,
        );
        assert_formatting(
            r#"A := B { c => { if !abc{nothing}else   if  true{if 0== 8 {}    } else{  } } } "#,
            r#"A := B {
    c => {
        if !abc {
            nothing
        } else if true {
            if 0 == 8 {
            }
        } else {
        }
    }
}
"#,
        );

        assert_formatting(
            "component A { c => { if( a == 1 ){b+=1;} else if (a==2)\n{b+=2;} else if a==3{\nb+=3;\n} else\n if(a==4){ a+=4} return 0;  } }",
            r"component A {
    c => {
        if (a == 1) {
            b += 1;
        } else if (a == 2) {
            b += 2;
        } else if a == 3 {
            b += 3;
        } else if (a == 4) {
            a += 4
        }
        return 0;
    }
}
");
    }

    #[test]
    fn code_block() {
        assert_formatting(
            r#"
component ABC {
    in-out property <bool> logged_in: false;
    function clicked() -> bool {
        if (logged_in) { foo();
            logged_in = false;
        return
        true;
        } else {
            logged_in = false; return false;
        }
    }
}
"#,
            r#"
component ABC {
    in-out property <bool> logged_in: false;
    function clicked() -> bool {
        if (logged_in) {
            foo();
            logged_in = false;
            return true;
        } else {
            logged_in = false;
            return false;
        }
    }
}
"#,
        );
    }

    #[test]
    fn trailing_comma_array() {
        assert_formatting(
            r#"
component ABC {
    in-out property <[int]> ar: [1, ];
    in-out property <[int]> ar: [1, 2, 3, 4, 5,];
    in-out property <[int]> ar2: [1, 2, 3, 4, 5];
}
"#,
            r#"
component ABC {
    in-out property <[int]> ar: [
        1,
    ];
    in-out property <[int]> ar: [
        1,
        2,
        3,
        4,
        5,
    ];
    in-out property <[int]> ar2: [1, 2, 3, 4, 5];
}
"#,
        );
    }

    #[test]
    fn large_array() {
        assert_formatting(
            r#"
component ABC {
    in-out property <[string]> large: ["first string", "second string", "third string", "fourth string", "fifth string"];
    in property <[int]> model: [
                                    1,
                                    2
    ];
}
"#,
            r#"
component ABC {
    in-out property <[string]> large: [
        "first string",
        "second string",
        "third string",
        "fourth string",
        "fifth string"
    ];
    in property <[int]> model: [1, 2];
}
"#,
        );
    }

    #[test]
    fn property_animation() {
        assert_formatting(
            r#"
export component MainWindow inherits Window {
    animate background { duration: 800ms;}
    animate x { duration: 100ms; easing: ease-out-bounce; }
    Rectangle {}
}
"#,
            r#"
export component MainWindow inherits Window {
    animate background { duration: 800ms; }
    animate x {
        duration: 100ms;
        easing: ease-out-bounce;
    }
    Rectangle { }
}
"#,
        );
    }

    #[test]
    fn object_literal() {
        assert_formatting(
            r#"
export component MainWindow inherits Window {
    in property <[TileData]> memory-tiles : [
        { image: @image-url("icons/at.png"), image-visible: false, solved: false, },
        { image: @image-url("icons/at.png"), image-visible: false, solved: false,},
        { image: @image-url("icons/at.png"), image-visible: false, solved: false, some_other_property: 12345 },
        { image: @image-url("icons/at.png"), image-visible: false, solved: false, some_other_property: 12345},
        { image: @image-url("icons/balance-scale.png") },
    ];
}
"#,
            r#"
export component MainWindow inherits Window {
    in property <[TileData]> memory-tiles: [
        {
            image: @image-url("icons/at.png"),
            image-visible: false,
            solved: false,
        },
        {
            image: @image-url("icons/at.png"),
            image-visible: false,
            solved: false,
        },
        {
            image: @image-url("icons/at.png"),
            image-visible: false,
            solved: false,
            some_other_property: 12345
        },
        {
            image: @image-url("icons/at.png"),
            image-visible: false,
            solved: false,
            some_other_property: 12345
        },
        { image: @image-url("icons/balance-scale.png") },
    ];
}
"#,
        );
    }

    #[test]
    fn preserve_empty_lines() {
        assert_formatting(
            r#"
export component MainWindow inherits Rectangle {
    in property <bool> open-curtain;
    callback clicked;

    border-radius: 8px;


    animate background { duration: 800ms; }

    Image {
        y: 8px;
    }


    Image {
        y: 8px;
    }
}
"#,
            r#"
export component MainWindow inherits Rectangle {
    in property <bool> open-curtain;
    callback clicked;

    border-radius: 8px;

    animate background { duration: 800ms; }

    Image {
        y: 8px;
    }

    Image {
        y: 8px;
    }
}
"#,
        );
    }

    #[test]
    fn multiple_property_animation() {
        assert_formatting(
            r#"
export component MainWindow inherits Rectangle {
    animate x , y { duration: 170ms; easing: cubic-bezier(0.17,0.76,0.4,1.75); }
    animate x , y { duration: 170ms;}
}
"#,
            r#"
export component MainWindow inherits Rectangle {
    animate x, y {
        duration: 170ms;
        easing: cubic-bezier(0.17,0.76,0.4,1.75);
    }
    animate x, y { duration: 170ms; }
}
"#,
        );
    }

    #[test]
    fn empty_array() {
        assert_formatting(
            r#"
export component MainWindow2 inherits Rectangle {
    in property <[string]> model: [ ];
}
"#,
            r#"
export component MainWindow2 inherits Rectangle {
    in property <[string]> model: [];
}
"#,
        );
    }

    #[test]
    fn two_way_binding() {
        assert_formatting(
            "export component Foobar{foo<=>xx.bar ; property<int\n>xx   <=>   ff . mm  ; callback doo <=> moo\n;\nproperty  e-e<=>f-f; }",
            r#"export component Foobar {
    foo <=> xx.bar;
    property <int> xx <=> ff.mm;
    callback doo <=> moo;
    property e-e <=> f-f;
}
"#,
        );
    }

    #[test]
    fn function() {
        assert_formatting(
            "export component Foo-bar{ pure\nfunction\n(x  :  int,y:string)->int{ self.y=0;\n\nif(true){return(45); a=0;} return x;  } function a(){/* ddd */}}",
            r#"export component Foo-bar {
    pure function (x: int, y: string) -> int {
        self.y = 0;

        if (true) {
            return (45);
            a = 0;
        }
        return x;
    }
    function a() {
        /* ddd */}
}
"#,
        );
    }

    #[test]
    fn changed() {
        assert_formatting(
            "component X { changed   width=>{ x+=1;  }    changed/*-*/height     =>     {y+=1;} }",
            r#"component X {
    changed width => {
        x += 1;
    }
    changed /*-*/height => {
        y += 1;
    }
}
"#,
        );
    }

    #[test]
    fn access_member() {
        assert_formatting(
            "component X { expr: 42   .log(x) + 41 . log(y) + foo . bar +  21.0.log(0) + 54.   .log(8) ; x: 42px.max(42px . min (0.px)); }",
            r#"component X {
    expr: 42 .log(x) + 41 .log(y) + foo.bar + 21.0.log(0) + 54..log(8);
    x: 42px.max(42px.min(0.px));
}
"#,
        );
    }
}
