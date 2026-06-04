use huff_ast::Span;

#[derive(Debug, Clone, PartialEq)]
pub struct Token {
    pub kind: TokenKind,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq)]
pub enum TokenKind {
    // Keywords
    Prog,
    Mod,
    Svc,
    Use,
    Err,
    Type,
    State,
    Auth,
    Op,
    Let,
    Pre,
    True,
    False,

    // Punctuation
    Colon,
    Equals,
    LParen,
    RParen,
    LBracket,
    RBracket,
    LBrace,
    RBrace,
    LAngle,
    RAngle,
    Comma,
    Dot,
    Question,
    Bang,
    Tilde,
    Amp,
    Plus,
    Minus,
    Star,
    Slash,
    Percent,
    Pipe,
    Underscore,

    // Multi-char operators
    Arrow,    // ->
    FatArrow, // =>
    EqEq,     // ==
    NotEq,    // !=
    LtEq,     // <=
    GtEq,     // >=
    AndAnd,   // &&
    OrOr,     // ||
    PlusEq,   // +=
    MinusEq,  // -=

    // Literals
    Int(i64),
    Float(f64),
    Str(String),
    Ident(String),

    // Layout
    Newline,
    Indent,
    Dedent,
    Eof,
}
