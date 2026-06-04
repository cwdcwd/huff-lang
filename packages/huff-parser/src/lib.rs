//! Huff parser — lexer (with indentation pass) plus recursive-descent parser.

#![forbid(unsafe_code)]

pub mod lexer;
pub mod parser;
pub mod token;

pub use lexer::{lex, LexError};
pub use parser::{parse, ParseError};
pub use token::{Token, TokenKind};

use huff_ast::File;

pub fn parse_source(src: &str) -> Result<File, ParseError> {
    let tokens = lex(src).map_err(ParseError::Lex)?;
    parse(src, &tokens)
}
