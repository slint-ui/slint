use std::convert::TryFrom;

mod document;

mod prelude {
    pub use super::{ParseError, Parser, SyntaxKind};
    pub use parser_test_macro::parser_test;
}

use crate::diagnostics::Diagnostics;

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
            Element,
            Binding,
            CodeStatement,
            CodeBlock,
            Expression,
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
    Identifier -> r"[\w]+",
    RBrace -> r"\}",
    LBrace -> r"\{",
    Equal -> r"=",
    Colon -> r":",
    Semicolon -> r";",
}

impl From<SyntaxKind> for rowan::SyntaxKind {
    fn from(v: SyntaxKind) -> Self {
        rowan::SyntaxKind(v.into())
    }
}

#[derive(Clone, Debug)]
pub struct Token {
    kind: SyntaxKind,
    text: rowan::SmolStr,
    offset: usize,
}

#[derive(Default, Debug, Clone, PartialEq)]
pub struct ParseError(pub String, pub usize);

pub struct Parser {
    builder: rowan::GreenNodeBuilder<'static>,
    tokens: Vec<Token>,
    cursor: usize,
    diags: Diagnostics,
}

#[derive(derive_more::Deref, derive_more::DerefMut)]
pub struct Node<'a>(&'a mut Parser);
impl<'a> Drop for Node<'a> {
    fn drop(&mut self) {
        self.0.builder.finish_node();
    }
}

impl Parser {
    pub fn new(source: &str) -> Self {
        fn lex(source: &str) -> Vec<Token> {
            lexer()
                .tokenize(source)
                .into_iter()
                .scan(0usize, |start_offset, t| {
                    let s: rowan::SmolStr = source[*start_offset..*start_offset + t.len].into();
                    let offset = *start_offset;
                    *start_offset += t.len;
                    Some(Token { kind: SyntaxKind::try_from(t.kind.0).unwrap(), text: s, offset })
                })
                .collect()
        }
        Self {
            builder: Default::default(),
            tokens: lex(source),
            cursor: 0,
            diags: Default::default(),
        }
    }

    pub fn start_node(&mut self, kind: SyntaxKind) -> Node {
        self.builder.start_node(kind.into());
        Node(self)
    }

    fn current_token(&self) -> Token {
        self.tokens.get(self.cursor).cloned().unwrap_or(Token {
            kind: SyntaxKind::Eof,
            text: Default::default(),
            offset: 0,
        })
    }

    pub fn peek(&mut self) -> Token {
        self.consume_ws();
        self.current_token()
    }

    pub fn peek_kind(&mut self) -> SyntaxKind {
        self.peek().kind
    }

    pub fn consume_ws(&mut self) {
        while matches!(self.current_token().kind, SyntaxKind::Whitespace | SyntaxKind::Comment) {
            self.consume()
        }
    }

    pub fn consume(&mut self) {
        let t = self.current_token();
        self.builder.token(t.kind.into(), t.text);
        self.cursor += 1;
    }

    pub fn expect(&mut self, kind: SyntaxKind) -> bool {
        if self.peek_kind() != kind {
            self.error(format!("Syntax error: expected {:?}", kind)); // FIXME better error
            return false;
        }
        self.consume();
        return true;
    }

    pub fn error(&mut self, e: impl Into<String>) {
        self.diags.push_error(e.into(), self.current_token().offset);
    }

    /// consume everyting until the token
    pub fn until(&mut self, kind: SyntaxKind) {
        // FIXME! match {} () []
        while self.cursor < self.tokens.len() && self.current_token().kind != kind {
            self.consume();
        }
        self.expect(kind);
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
//type SyntaxElement = rowan::NodeOrToken<SyntaxNode, SyntaxToken>;

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

pub fn parse(source: &str) -> (SyntaxNode, Diagnostics) {
    let mut p = Parser::new(source);
    document::parse_document(&mut p);
    (SyntaxNode::new_root(p.builder.finish()), p.diags)
}
