// Copyright © SixtyFPS GmbH <info@slint-ui.com>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-commercial

use crate::writer::TokenWriter;
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
        SyntaxKind::CallbackConnection => {
            return format_callback_connection(node, writer, state);
        }
        SyntaxKind::CallbackDeclaration => {
            return format_callback_declaration(node, writer, state);
        }
        SyntaxKind::QualifiedName => {
            return format_qualified_name(node, writer, state);
        }
        SyntaxKind::BinaryExpression => {
            return format_binary_expression(node, writer, state);
        }
        SyntaxKind::Expression => {
            return format_expression(node, writer, state);
        }
        SyntaxKind::ChildrenPlaceholder => {
            return format_children_placeholder(node, writer, state);
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
            if t.kind() == SyntaxKind::Whitespace {
                if state.skip_all_whitespace {
                    writer.with_new_content(t, "")?;
                    return Ok(());
                }
            } else {
                state.after_comment = t.kind() == SyntaxKind::Comment;
                state.skip_all_whitespace = false;
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

fn whitespace_to(
    sub: &mut impl Iterator<Item = NodeOrToken>,
    element: SyntaxKind,
    writer: &mut impl TokenWriter,
    state: &mut FormatState,
    prefix_whitespace: &str,
) -> Result<bool, std::io::Error> {
    whitespace_to_one_of(sub, &[element], writer, state, prefix_whitespace)
}

fn whitespace_to_one_of(
    sub: &mut impl Iterator<Item = NodeOrToken>,
    elements: &[SyntaxKind],
    writer: &mut impl TokenWriter,
    state: &mut FormatState,
    prefix_whitespace: &str,
) -> Result<bool, std::io::Error> {
    state.insert_whitespace(prefix_whitespace);
    for n in sub {
        match n.kind() {
            SyntaxKind::Whitespace | SyntaxKind::Comment => (),
            x if elements.contains(&x) => {
                fold(n, writer, state)?;
                return Ok(true);
            }
            _ => {
                eprintln!("Inconsistency: expected {:?},  found {:?}", elements, n);
                fold(n, writer, state)?;
                return Ok(false);
            }
        }
        fold(n, writer, state)?;
    }
    eprintln!("Inconsistency: expected {:?},  not found", elements);
    Ok(false)
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
    let mut sub = node.children_with_tokens();
    let _ok = whitespace_to(&mut sub, SyntaxKind::DeclaredIdentifier, writer, state, "")?
        && whitespace_to(&mut sub, SyntaxKind::ColonEqual, writer, state, " ")?
        && whitespace_to(&mut sub, SyntaxKind::Element, writer, state, " ")?;

    finish_node(sub, writer, state)?;
    state.new_line();

    Ok(())
}

fn format_element(
    node: &SyntaxNode,
    writer: &mut impl TokenWriter,
    state: &mut FormatState,
) -> Result<(), std::io::Error> {
    let mut sub = node.children_with_tokens();
    if !(whitespace_to(&mut sub, SyntaxKind::QualifiedName, writer, state, "")?
        && whitespace_to(&mut sub, SyntaxKind::LBrace, writer, state, " ")?)
    {
        // There is an error finding the QualifiedName and LBrace when we do this branch.
        finish_node(sub, writer, state)?;
        return Ok(());
    }

    state.indentation_level += 1;
    state.new_line();
    let ins_ctn = state.insertion_count;

    for n in sub {
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
            fold(n, writer, state)?;
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
                    // There is an error finding the Identifier and := when we do this branch.
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
            state.new_line();
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
    let _ok = whitespace_to(&mut sub, SyntaxKind::Identifier, writer, state, "")?
        && whitespace_to(&mut sub, SyntaxKind::LAngle, writer, state, " ")?
        && whitespace_to(&mut sub, SyntaxKind::Type, writer, state, "")?
        && whitespace_to(&mut sub, SyntaxKind::RAngle, writer, state, "")?
        && whitespace_to(&mut sub, SyntaxKind::DeclaredIdentifier, writer, state, " ")?;

    state.skip_all_whitespace = true;
    // FIXME: more formatting
    for s in sub {
        fold(s, writer, state)?;
    }
    state.new_line();
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

fn format_callback_declaration(
    node: &SyntaxNode,
    writer: &mut impl TokenWriter,
    state: &mut FormatState,
) -> Result<(), std::io::Error> {
    let mut sub = node.children_with_tokens();
    let _ok = whitespace_to(&mut sub, SyntaxKind::Identifier, writer, state, "")?
        && whitespace_to(&mut sub, SyntaxKind::DeclaredIdentifier, writer, state, " ")?;

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
            _ => {
                fold(n, writer, state)?;
            }
        }
    }
    state.new_line();
    Ok(())
}

fn format_callback_connection(
    node: &SyntaxNode,
    writer: &mut impl TokenWriter,
    state: &mut FormatState,
) -> Result<(), std::io::Error> {
    let mut sub = node.children_with_tokens();
    let _ok = whitespace_to(&mut sub, SyntaxKind::Identifier, writer, state, "")?;

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
            ],
            writer,
            state,
            " ",
        )?
        && whitespace_to(&mut sub, SyntaxKind::Expression, writer, state, " ")?;

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

fn format_children_placeholder(
    node: &SyntaxNode,
    writer: &mut impl TokenWriter,
    state: &mut FormatState,
) -> Result<(), std::io::Error> {
    // Skips whitespace after a `@children` node.

    for n in node.children_with_tokens() {
        fold(n, writer, state)?;
    }
    state.skip_all_whitespace = true;

    Ok(())
}
#[cfg(test)]
mod tests {
    use super::*;
    use crate::writer::FileWriter;
    use i_slint_compiler::diagnostics::BuildDiagnostics;
    use i_slint_compiler::parser::syntax_nodes;

    // FIXME more descriptive errors when an assertion fails
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
        assert_formatting("A:=Text{}", "A := Text { }");
    }

    #[test]
    fn complex_formatting() {
        assert_formatting(
            r#"
Main :=Window{callback some-fn(string,string)->bool;some-fn(a, b)=>{a<=b} property<bool>prop-x;
    VerticalBox {      combo:=ComboBox{}    }}
            "#,
            r#"
Main := Window {
    callback some-fn(string, string) -> bool;
    some-fn(a, b) => {a <= b}
    property <bool> prop-x;
    VerticalBox {
        combo := ComboBox { }
    }
}"#,
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
}"#,
        );
    }

    #[test]
    fn space_with_braces() {
        assert_formatting(r#"Main := Window{}"#, r#"Main := Window { }"#);
        // Also in a child
        assert_formatting(
            r#"
Main := Window{Child{}}"#,
            r#"
Main := Window {
    Child { }
}"#,
        );
        assert_formatting(
            r#"
Main := VerticalLayout{HorizontalLayout{prop:3;}}"#,
            r#"
Main := VerticalLayout {
    HorizontalLayout {
        prop: 3;
    }
}"#,
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
}"#,
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
}"#,
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
}"#,
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
}"#,
        );
    }
}
