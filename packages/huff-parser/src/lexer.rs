//! Hand-rolled lexer with a Python-style indentation pass.
//!
//! Two-pass design: the raw scanner produces tokens with explicit `Newline`
//! markers and the indentation level for each line; a second pass converts
//! shifts in indentation into synthetic `Indent`/`Dedent` tokens.

use huff_ast::Span;
use thiserror::Error;

use crate::token::{Token, TokenKind};

#[derive(Debug, Error)]
pub enum LexError {
    #[error("unexpected character {ch:?} at byte offset {offset}")]
    UnexpectedChar { ch: char, offset: usize },
    #[error("unterminated string starting at byte offset {offset}")]
    UnterminatedString { offset: usize },
    #[error("inconsistent indentation at byte offset {offset}")]
    InconsistentIndent { offset: usize },
    #[error("tabs are not allowed for indentation at byte offset {offset}")]
    TabIndent { offset: usize },
}

const INDENT_WIDTH: usize = 2;

pub fn lex(src: &str) -> Result<Vec<Token>, LexError> {
    let bytes = src.as_bytes();
    let mut out = Vec::<Token>::new();
    let mut indent_stack: Vec<usize> = vec![0];
    let mut i = 0usize;
    let mut at_line_start = true;
    let mut paren_depth: i32 = 0;

    while i < bytes.len() {
        if at_line_start && paren_depth == 0 {
            // Measure indentation (spaces only — reject tabs).
            let line_start = i;
            let mut spaces = 0usize;
            while i < bytes.len() && bytes[i] == b' ' {
                i += 1;
                spaces += 1;
            }
            // Tab as indent is an error.
            if i < bytes.len() && bytes[i] == b'\t' {
                return Err(LexError::TabIndent { offset: i });
            }
            // Skip blank or comment-only lines (no indent change).
            if i >= bytes.len() {
                break;
            }
            if bytes[i] == b'\n' {
                i += 1;
                continue;
            }
            if bytes[i] == b'/' && i + 1 < bytes.len() && bytes[i + 1] == b'/' {
                // line comment to end of line
                while i < bytes.len() && bytes[i] != b'\n' {
                    i += 1;
                }
                continue;
            }
            // Compare against indent stack.
            let cur = *indent_stack.last().unwrap();
            if spaces > cur {
                if (spaces - cur) % INDENT_WIDTH != 0 {
                    return Err(LexError::InconsistentIndent { offset: line_start });
                }
                let mut delta = (spaces - cur) / INDENT_WIDTH;
                let mut new_level = cur;
                while delta > 0 {
                    new_level += INDENT_WIDTH;
                    indent_stack.push(new_level);
                    out.push(Token {
                        kind: TokenKind::Indent,
                        span: Span::new(line_start, i),
                    });
                    delta -= 1;
                }
            } else {
                while spaces < *indent_stack.last().unwrap() {
                    indent_stack.pop();
                    out.push(Token {
                        kind: TokenKind::Dedent,
                        span: Span::new(line_start, i),
                    });
                }
                if spaces != *indent_stack.last().unwrap() {
                    return Err(LexError::InconsistentIndent { offset: line_start });
                }
            }
            at_line_start = false;
            continue;
        }

        let c = bytes[i];

        // Skip non-significant whitespace inside a line (regular spaces).
        if c == b' ' {
            i += 1;
            continue;
        }
        if c == b'\t' {
            // Tabs are tolerated mid-line but ignored.
            i += 1;
            continue;
        }

        if c == b'\n' {
            // Inside a paren group, treat newline as ordinary whitespace.
            if paren_depth > 0 {
                i += 1;
                continue;
            }
            // Otherwise, emit a newline token (only if previous wasn't a newline).
            if !matches!(out.last().map(|t| &t.kind), Some(TokenKind::Newline) | None) {
                out.push(Token {
                    kind: TokenKind::Newline,
                    span: Span::new(i, i + 1),
                });
            }
            i += 1;
            at_line_start = true;
            continue;
        }

        if c == b'/' && i + 1 < bytes.len() && bytes[i + 1] == b'/' {
            // line comment
            while i < bytes.len() && bytes[i] != b'\n' {
                i += 1;
            }
            continue;
        }

        // String literal (possibly with interpolation).
        if c == b'"' {
            let start = i;
            i += 1;
            let mut s = String::new();
            let mut has_interp = false;
            let mut segments: Vec<crate::token::InterpSeg> = Vec::new();

            while i < bytes.len() && bytes[i] != b'"' {
                if bytes[i] == b'\\' && i + 1 < bytes.len() {
                    let esc = bytes[i + 1];
                    let ch = match esc {
                        b'n' => '\n',
                        b't' => '\t',
                        b'r' => '\r',
                        b'\\' => '\\',
                        b'"' => '"',
                        b'0' => '\0',
                        b'{' => '{',
                        other => other as char,
                    };
                    s.push(ch);
                    i += 2;
                } else if bytes[i] == b'{' {
                    // Interpolation: collect ident inside braces
                    has_interp = true;
                    // Push accumulated literal segment
                    if !s.is_empty() {
                        segments.push(crate::token::InterpSeg::Lit(std::mem::take(&mut s)));
                    }
                    i += 1; // skip {
                    let var_start = i;
                    while i < bytes.len() && bytes[i] != b'}' && bytes[i] != b'"' && bytes[i] != b'\n' {
                        i += 1;
                    }
                    if i >= bytes.len() || bytes[i] != b'}' {
                        return Err(LexError::UnterminatedString { offset: start });
                    }
                    let var_name = src[var_start..i].to_string();
                    segments.push(crate::token::InterpSeg::Var(var_name));
                    i += 1; // skip }
                } else if bytes[i] == b'\n' {
                    return Err(LexError::UnterminatedString { offset: start });
                } else {
                    s.push(bytes[i] as char);
                    i += 1;
                }
            }
            if i >= bytes.len() {
                return Err(LexError::UnterminatedString { offset: start });
            }
            i += 1; // closing "

            let kind = if has_interp {
                // Push trailing literal segment if any
                if !s.is_empty() {
                    segments.push(crate::token::InterpSeg::Lit(s));
                }
                TokenKind::InterpStr(segments)
            } else {
                TokenKind::Str(s)
            };
            out.push(Token {
                kind,
                span: Span::new(start, i),
            });
            continue;
        }

        // Number literal.
        if c.is_ascii_digit() {
            let start = i;
            while i < bytes.len() && bytes[i].is_ascii_digit() {
                i += 1;
            }
            // Float?
            let mut is_float = false;
            if i < bytes.len() && bytes[i] == b'.' && i + 1 < bytes.len() && bytes[i + 1].is_ascii_digit() {
                is_float = true;
                i += 1;
                while i < bytes.len() && bytes[i].is_ascii_digit() {
                    i += 1;
                }
            }
            let lex_str = &src[start..i];
            let kind = if is_float {
                TokenKind::Float(lex_str.parse().unwrap_or(0.0))
            } else {
                TokenKind::Int(lex_str.parse().unwrap_or(0))
            };
            out.push(Token {
                kind,
                span: Span::new(start, i),
            });
            continue;
        }

        // Identifier or keyword.
        if c.is_ascii_alphabetic() || c == b'_' {
            let start = i;
            while i < bytes.len()
                && (bytes[i].is_ascii_alphanumeric() || bytes[i] == b'_')
            {
                i += 1;
            }
            let word = &src[start..i];
            let kind = match word {
                "prog" => TokenKind::Prog,
                "mod" => TokenKind::Mod,
                "svc" => TokenKind::Svc,
                "use" => TokenKind::Use,
                "err" => TokenKind::Err,
                "type" => TokenKind::Type,
                "state" => TokenKind::State,
                "auth" => TokenKind::Auth,
                "op" => TokenKind::Op,
                "let" => TokenKind::Let,
                "pre" => TokenKind::Pre,
                "true" => TokenKind::True,
                "false" => TokenKind::False,
                "_" => TokenKind::Underscore,
                _ => TokenKind::Ident(word.to_string()),
            };
            out.push(Token {
                kind,
                span: Span::new(start, i),
            });
            continue;
        }

        // Punctuation / operators.
        let start = i;
        let kind = match c {
            b'(' => {
                paren_depth += 1;
                i += 1;
                TokenKind::LParen
            }
            b')' => {
                paren_depth -= 1;
                i += 1;
                TokenKind::RParen
            }
            b'[' => {
                paren_depth += 1;
                i += 1;
                TokenKind::LBracket
            }
            b']' => {
                paren_depth -= 1;
                i += 1;
                TokenKind::RBracket
            }
            b'{' => {
                paren_depth += 1;
                i += 1;
                TokenKind::LBrace
            }
            b'}' => {
                paren_depth -= 1;
                i += 1;
                TokenKind::RBrace
            }
            b',' => {
                i += 1;
                TokenKind::Comma
            }
            b':' => {
                i += 1;
                TokenKind::Colon
            }
            b'.' => {
                i += 1;
                TokenKind::Dot
            }
            b'?' => {
                i += 1;
                TokenKind::Question
            }
            b'~' => {
                i += 1;
                TokenKind::Tilde
            }
            b'&' => {
                if i + 1 < bytes.len() && bytes[i + 1] == b'&' {
                    i += 2;
                    TokenKind::AndAnd
                } else {
                    i += 1;
                    TokenKind::Amp
                }
            }
            b'|' => {
                if i + 1 < bytes.len() && bytes[i + 1] == b'|' {
                    i += 2;
                    TokenKind::OrOr
                } else {
                    i += 1;
                    TokenKind::Pipe
                }
            }
            b'+' => {
                if i + 1 < bytes.len() && bytes[i + 1] == b'=' {
                    i += 2;
                    TokenKind::PlusEq
                } else {
                    i += 1;
                    TokenKind::Plus
                }
            }
            b'-' => {
                if i + 1 < bytes.len() && bytes[i + 1] == b'>' {
                    i += 2;
                    TokenKind::Arrow
                } else if i + 1 < bytes.len() && bytes[i + 1] == b'=' {
                    i += 2;
                    TokenKind::MinusEq
                } else {
                    i += 1;
                    TokenKind::Minus
                }
            }
            b'*' => {
                i += 1;
                TokenKind::Star
            }
            b'/' => {
                i += 1;
                TokenKind::Slash
            }
            b'%' => {
                i += 1;
                TokenKind::Percent
            }
            b'<' => {
                if i + 1 < bytes.len() && bytes[i + 1] == b'=' {
                    i += 2;
                    TokenKind::LtEq
                } else {
                    i += 1;
                    TokenKind::LAngle
                }
            }
            b'>' => {
                if i + 1 < bytes.len() && bytes[i + 1] == b'=' {
                    i += 2;
                    TokenKind::GtEq
                } else {
                    i += 1;
                    TokenKind::RAngle
                }
            }
            b'=' => {
                if i + 1 < bytes.len() && bytes[i + 1] == b'=' {
                    i += 2;
                    TokenKind::EqEq
                } else if i + 1 < bytes.len() && bytes[i + 1] == b'>' {
                    i += 2;
                    TokenKind::FatArrow
                } else {
                    i += 1;
                    TokenKind::Equals
                }
            }
            b'!' => {
                if i + 1 < bytes.len() && bytes[i + 1] == b'=' {
                    i += 2;
                    TokenKind::NotEq
                } else {
                    i += 1;
                    TokenKind::Bang
                }
            }
            other => {
                return Err(LexError::UnexpectedChar {
                    ch: other as char,
                    offset: i,
                })
            }
        };
        out.push(Token {
            kind,
            span: Span::new(start, i),
        });
    }

    // Trailing newline before closing dedents (so layout sees a clean line break).
    if !matches!(out.last().map(|t| &t.kind), Some(TokenKind::Newline) | None) {
        out.push(Token {
            kind: TokenKind::Newline,
            span: Span::new(bytes.len(), bytes.len()),
        });
    }
    while indent_stack.len() > 1 {
        indent_stack.pop();
        out.push(Token {
            kind: TokenKind::Dedent,
            span: Span::new(bytes.len(), bytes.len()),
        });
    }
    out.push(Token {
        kind: TokenKind::Eof,
        span: Span::new(bytes.len(), bytes.len()),
    });
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::token::TokenKind::*;

    fn kinds(src: &str) -> Vec<TokenKind> {
        lex(src).unwrap().into_iter().map(|t| t.kind).collect()
    }

    #[test]
    fn hello_minimal() {
        let src = "prog HelloWorld\n  op Main()\n    !io.writeln(\"Hello World\")\n";
        let ks = kinds(src);
        // Expect: prog Ident NL Indent op Ident LP RP NL Indent Bang Ident Dot Ident LP Str RP NL Dedent Dedent EOF
        assert!(ks.contains(&Prog));
        assert!(ks.contains(&Op));
        assert!(ks.contains(&Indent));
        assert!(ks.iter().filter(|k| matches!(k, Dedent)).count() == 2);
        assert!(matches!(ks.last().unwrap(), Eof));
    }

    #[test]
    fn ignores_blank_lines() {
        let src = "prog X\n\n  op M()\n    !io.writeln(\"hi\")\n\n";
        // Should not crash and should still produce two indents and two dedents
        let ks = kinds(src);
        let n_indent = ks.iter().filter(|k| matches!(k, Indent)).count();
        let n_dedent = ks.iter().filter(|k| matches!(k, Dedent)).count();
        assert_eq!(n_indent, 2);
        assert_eq!(n_dedent, 2);
    }

    #[test]
    fn no_trailing_newline() {
        let src = "prog X\n  op M()\n    !io.writeln(\"hi\")";
        let ks = kinds(src);
        assert!(matches!(ks.last().unwrap(), Eof));
    }

    #[test]
    fn arrow_and_fat_arrow() {
        let src = "names->map(n => n)\n";
        let ks = kinds(src);
        assert!(ks.contains(&Arrow));
        assert!(ks.contains(&FatArrow));
    }

    #[test]
    fn paren_swallows_newlines() {
        let src = "op M(\n  a: str,\n  b: str,\n)\n  let x = a\n";
        // Should not produce indent inside the parameter list.
        let toks = lex(src).unwrap();
        // ensure no extraneous Indent/Dedent before the body
        let body_start = toks.iter().position(|t| matches!(t.kind, RParen)).unwrap();
        // no indent token between LParen and RParen
        let between: Vec<_> = toks[..body_start].iter().filter(|t| matches!(t.kind, Indent | Dedent)).collect();
        assert!(between.is_empty(), "unexpected indents inside params: {:?}", between);
    }

    #[test]
    fn comments_are_skipped() {
        let src = "prog X\n  // hello\n  op M()\n    !io.writeln(\"hi\")\n";
        let ks = kinds(src);
        // Should not contain any comment text as Ident
        assert!(!ks.iter().any(|k| matches!(k, Ident(s) if s == "hello")));
    }

    #[test]
    fn pre_with_compare() {
        let src = "op X(a: u32)\n  pre a > 0 : Bad\n  a\n";
        let ks = kinds(src);
        assert!(ks.contains(&Pre));
        assert!(ks.contains(&RAngle));
        assert!(ks.contains(&Colon));
    }

    #[test]
    fn error_propagation_bang_after_call() {
        let src = "let x = call()!\n";
        let ks = kinds(src);
        // Bang token should appear after RParen.
        let pos_rp = ks.iter().position(|k| matches!(k, RParen)).unwrap();
        assert!(matches!(ks[pos_rp + 1], Bang));
    }
}
