/*! The sixtyfps language parser

This module is responsible to parse a string onto a syntax tree.

The core of it is the `DefaultParser` class that holds a list of token and
generates a `rowan::GreenNode`

This module has different sub modules with the actual parser functions

*/

use crate::diagnostics::Diagnostics;
pub use rowan::SmolStr;
use std::convert::TryFrom;

mod document;
mod expressions;

/// Each parser submodule would simply do `use super::prelude::*` to import typically used items
mod prelude {
    pub use super::{DefaultParser, Parser, SyntaxKind};
    #[cfg(test)]
    pub use parser_test_macro::parser_test;
}

/// This macro is invoked once, to declare all the token and syntax kind.
/// The purpose of this macro is to declare the token with its regexp at the same place
macro_rules! declare_token_kind {
    ($($token:ident -> $rx:expr ,)*) => {
        #[repr(u16)]
        #[derive(Debug, Copy, Clone, Eq, PartialEq, num_enum::IntoPrimitive, num_enum::TryFromPrimitive)]
        pub enum SyntaxKind {
            Error,
            Eof,

            // Token:
            $($token,)*

            //SyntaxKind:
            Document,
            Component,
            /// Note: This is in fact the same as Component as far as the parser is concerned
            SubElement,
            Element,
            RepeatedElement,
            SignalDeclaration,
            SignalConnection,
            PropertyDeclaration,
            Binding,
            BindingExpression, // the right-hand-side of a binding
            CodeBlock,
            Expression,
            QualifiedTypeName, // wraps Identifiers, like Rectangle or SomeModule.SomeType
            /// foo!bar
            BangExpression,
        }

        fn lexer() -> m_lexer::Lexer {
            m_lexer::LexerBuilder::new()
                .error_token(m_lexer::TokenKind(SyntaxKind::Error.into()))
                .tokens(&[
                    $((m_lexer::TokenKind(SyntaxKind::$token.into()), $rx)),*
                ])
                .build()
        }
    }
}
declare_token_kind! {
    Whitespace -> r"\s+",
    Comment -> r"//.*\n",
    StringLiteral -> r#""[^"]*""#, // FIXME: escapes
    NumberLiteral -> r"[\d]+(\.[\d]*)?",
    Identifier -> r"[\w]+",
    LBrace -> r"\{",
    RBrace -> r"\}",
    LParent -> r"\(",
    RParent -> r"\)",
    LAngle -> r"<",
    RAngle -> r">",
    ColonEqual -> ":=",
    FatArrow -> r"=>",
    Equal -> r"=",
    Colon -> r":",
    Semicolon -> r";",
    Bang -> r"!",
    Dot -> r"\.",
}

impl From<SyntaxKind> for rowan::SyntaxKind {
    fn from(v: SyntaxKind) -> Self {
        rowan::SyntaxKind(v.into())
    }
}

#[derive(Clone, Debug)]
pub struct Token {
    pub kind: SyntaxKind,
    pub text: SmolStr,
    pub offset: usize,
    #[cfg(feature = "proc_macro_span")]
    pub span: Option<proc_macro::Span>,
}

impl Default for Token {
    fn default() -> Self {
        Token {
            kind: SyntaxKind::Eof,
            text: Default::default(),
            offset: 0,
            #[cfg(feature = "proc_macro_span")]
            span: None,
        }
    }
}

impl Token {
    pub fn as_str(&self) -> &str {
        self.text.as_str()
    }
}

mod parser_trait {
    //! module allowing to keep implementation details of the node private
    use super::*;

    // Todo: rename DefaultParser
    pub trait Parser: Sized {
        /// Enter a new node.  The node is going to be finished when
        /// The return value of this function is drop'ed
        ///
        /// (do not re-implement this function, re-implement
        /// start_node_impl and finish_node_impl)
        fn start_node(&mut self, kind: SyntaxKind) -> Node<Self> {
            self.start_node_impl(kind, NodeToken(()));
            Node(self)
        }

        /// Can only be called by Node::drop
        fn finish_node_impl(&mut self, token: NodeToken);
        /// Can only be called by Self::start_node
        fn start_node_impl(&mut self, kind: SyntaxKind, token: NodeToken);
        fn peek(&mut self) -> Token;
        /// Peek the n'th token, not including whitespaces and comments
        fn nth(&mut self, n: usize) -> SyntaxKind;
        fn consume(&mut self);
        fn error(&mut self, e: impl Into<String>);

        /// Consume the token if it has the right kind, otherwise report a syntax error.
        /// Returns true if the token was consumed.
        fn expect(&mut self, kind: SyntaxKind) -> bool {
            if self.nth(0) != kind {
                self.error(format!("Syntax error: expected {:?}", kind));
                return false;
            }
            self.consume();
            return true;
        }

        /// consume everyting until reaching a token of this kind
        fn until(&mut self, kind: SyntaxKind) {
            // FIXME! match {} () []
            while {
                let k = self.nth(0);
                k != kind && k != SyntaxKind::Eof
            } {
                self.consume();
            }
            self.expect(kind);
        }
    }

    /// A token to proof that start_node_impl and finish_node_impl are only
    /// called from the Node implementation
    ///
    /// Since the constructor is private, it cannot be produced by anything else.
    pub struct NodeToken(());
    /// The return value of `DefaultParser::start_node`. This borrows the parser
    /// and finishes the node on Drop
    #[derive(derive_more::DerefMut)]
    pub struct Node<'a, P: Parser>(&'a mut P);
    impl<'a, P: Parser> Drop for Node<'a, P> {
        fn drop(&mut self) {
            self.0.finish_node_impl(NodeToken(()));
        }
    }
    impl<'a, P: Parser> core::ops::Deref for Node<'a, P> {
        type Target = P;
        fn deref(&self) -> &Self::Target {
            self.0
        }
    }
}
#[doc(inline)]
pub use parser_trait::*;

pub struct DefaultParser {
    builder: rowan::GreenNodeBuilder<'static>,
    tokens: Vec<Token>,
    cursor: usize,
    diags: Diagnostics,
}

impl From<Vec<Token>> for DefaultParser {
    fn from(tokens: Vec<Token>) -> Self {
        Self { builder: Default::default(), tokens, cursor: 0, diags: Default::default() }
    }
}

impl DefaultParser {
    /// Constructor that create a parser from the source code
    pub fn new(source: &str) -> Self {
        fn lex(source: &str) -> Vec<Token> {
            lexer()
                .tokenize(source)
                .into_iter()
                .scan(0usize, |start_offset, t| {
                    let s: rowan::SmolStr = source[*start_offset..*start_offset + t.len].into();
                    let offset = *start_offset;
                    *start_offset += t.len;
                    Some(Token {
                        kind: SyntaxKind::try_from(t.kind.0).unwrap(),
                        text: s,
                        offset,
                        ..Default::default()
                    })
                })
                .collect()
        }
        Self::from(lex(source))
    }

    fn current_token(&self) -> Token {
        self.tokens.get(self.cursor).cloned().unwrap_or_default()
    }

    /// Consume all the whitespace
    pub fn consume_ws(&mut self) {
        while matches!(self.current_token().kind, SyntaxKind::Whitespace | SyntaxKind::Comment) {
            self.consume()
        }
    }
}

impl Parser for DefaultParser {
    fn start_node_impl(&mut self, kind: SyntaxKind, _: NodeToken) {
        self.builder.start_node(kind.into());
    }

    fn finish_node_impl(&mut self, _: NodeToken) {
        self.builder.finish_node();
    }

    fn peek(&mut self) -> Token {
        self.consume_ws();
        self.current_token()
    }

    /// Peek the n'th token, not including whitespaces and comments
    fn nth(&mut self, mut n: usize) -> SyntaxKind {
        self.consume_ws();
        let mut c = self.cursor;
        while n > 0 {
            n -= 1;
            c += 1;
            while c < self.tokens.len()
                && matches!(self.tokens[c].kind, SyntaxKind::Whitespace | SyntaxKind::Comment)
            {
                c += 1;
            }
        }
        self.tokens.get(c).map_or(SyntaxKind::Eof, |x| x.kind)
    }

    /// Consume the current token
    fn consume(&mut self) {
        let t = self.current_token();
        self.builder.token(t.kind.into(), t.text);
        self.cursor += 1;
    }

    /// Reports an error at the current token location
    fn error(&mut self, e: impl Into<String>) {
        let current_token = self.current_token();
        #[allow(unused_mut)]
        let mut span = crate::diagnostics::Span::new(current_token.offset);
        #[cfg(feature = "proc_macro_span")]
        {
            span.span = current_token.span;
        }
        self.diags.push_error(e.into(), span);
    }
}

#[derive(Clone, Copy, Debug, Eq, Ord, Hash, PartialEq, PartialOrd)]
pub enum Language {}
impl rowan::Language for Language {
    type Kind = SyntaxKind;
    fn kind_from_raw(raw: rowan::SyntaxKind) -> Self::Kind {
        SyntaxKind::try_from(raw.0).unwrap()
    }
    fn kind_to_raw(kind: Self::Kind) -> rowan::SyntaxKind {
        kind.into()
    }
}

pub type SyntaxNode = rowan::SyntaxNode<Language>;
pub type SyntaxToken = rowan::SyntaxToken<Language>;

/// Helper functions to easily get the children of a given kind.
/// This traits is only supposed to be implemented on SyntaxNope
pub trait SyntaxNodeEx {
    fn child_node(&self, kind: SyntaxKind) -> Option<SyntaxNode>;
    fn child_token(&self, kind: SyntaxKind) -> Option<SyntaxToken>;
    fn child_text(&self, kind: SyntaxKind) -> Option<String>;
}

impl SyntaxNodeEx for SyntaxNode {
    fn child_node(&self, kind: SyntaxKind) -> Option<SyntaxNode> {
        self.children().find(|n| n.kind() == kind)
    }
    fn child_token(&self, kind: SyntaxKind) -> Option<SyntaxToken> {
        self.children_with_tokens().find(|n| n.kind() == kind).and_then(|x| x.into_token())
    }
    fn child_text(&self, kind: SyntaxKind) -> Option<String> {
        self.children_with_tokens()
            .find(|n| n.kind() == kind)
            .and_then(|x| x.as_token().map(|x| x.text().to_string()))
    }
}

/// Returns a span.  This is implemented for tokens and nodes
pub trait Spanned {
    fn span(&self) -> crate::diagnostics::Span;
}

impl Spanned for SyntaxNode {
    fn span(&self) -> crate::diagnostics::Span {
        // FIXME!  this does not work with proc_macro span
        crate::diagnostics::Span::new(self.text_range().start().into())
    }
}

impl Spanned for SyntaxToken {
    fn span(&self) -> crate::diagnostics::Span {
        // FIXME!  this does not work with proc_macro span
        crate::diagnostics::Span::new(self.text_range().start().into())
    }
}

// Actual parser
pub fn parse(source: &str) -> (SyntaxNode, Diagnostics) {
    let mut p = DefaultParser::new(source);
    document::parse_document(&mut p);
    (SyntaxNode::new_root(p.builder.finish()), p.diags)
}

#[allow(dead_code)]
pub fn parse_tokens(tokens: Vec<Token>) -> (SyntaxNode, Diagnostics) {
    let mut p = DefaultParser::from(tokens);
    document::parse_document(&mut p);
    (SyntaxNode::new_root(p.builder.finish()), p.diags)
}
