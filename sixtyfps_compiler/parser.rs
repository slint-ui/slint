use std::convert::TryFrom;

mod document;

mod prelude {
    pub use super::{ParseError, Parser, SyntaxKind};
    pub use parser_test_macro::parser_test;
}

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
pub struct Token(SyntaxKind, rowan::SmolStr);

pub type ParseError = String;

pub struct Parser {
    builder: rowan::GreenNodeBuilder<'static>,
    tokens: Vec<Token>,
    cursor: usize,
    errors: Vec<ParseError>,
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
                    *start_offset += t.len;
                    Some(Token(SyntaxKind::try_from(t.kind.0).unwrap(), s))
                })
                .collect()
        }
        Self { builder: Default::default(), tokens: lex(source), cursor: 0, errors: vec![] }
    }

    pub fn start_node(&mut self, kind: SyntaxKind) -> Node {
        self.builder.start_node(kind.into());
        Node(self)
    }

    fn current_token(&self) -> Token {
        self.tokens.get(self.cursor).cloned().unwrap_or(Token(SyntaxKind::Eof, "".into()))
    }

    pub fn peek(&mut self) -> Token {
        self.consume_ws();
        self.current_token()
    }

    pub fn peek_kind(&mut self) -> SyntaxKind {
        self.peek().0
    }

    pub fn consume_ws(&mut self) {
        while self.current_token().0 == SyntaxKind::Whitespace {
            self.consume()
        }
    }

    pub fn consume(&mut self) {
        let t = self.current_token();
        self.builder.token(t.0.into(), t.1);
        self.cursor += 1;
    }

    pub fn expect(&mut self, kind: SyntaxKind) -> bool {
        let t = self.peek();
        if t.0 != kind {
            self.error(format!("Syntax error: expected {:?}", kind)); // FIXME better error
            return false;
        }
        self.consume();
        return true;
    }

    pub fn error(&mut self, e: impl Into<String>) {
        self.errors.push(e.into());
    }

    /// consume everyting until the token
    pub fn until(&mut self, kind: SyntaxKind) {
        // FIXME! match {} () []
        while self.cursor < self.tokens.len() && self.current_token().0 != kind {
            self.consume();
        }
        self.expect(kind);
    }
}

pub fn parse(source: &str) -> (rowan::GreenNode, Vec<ParseError>) {
    let mut p = Parser::new(source);
    document::parse_document(&mut p);
    (p.builder.finish(), p.errors)
}
