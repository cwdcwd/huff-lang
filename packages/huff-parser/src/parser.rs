//! Recursive-descent parser — produces a `huff_ast::File`.
//!
//! v0 walking-skeleton subset only. Out-of-subset constructs raise a clear
//! `not yet supported: <feature>` error rather than half-parsing.

use huff_ast::*;
use thiserror::Error;

use crate::lexer::LexError;
use crate::token::{Token, TokenKind};

#[derive(Debug, Error)]
pub enum ParseError {
    #[error("lex error: {0}")]
    Lex(#[from] LexError),
    #[error("parse error at byte {offset}: {msg}")]
    Generic { offset: usize, msg: String },
    #[error("not yet supported: {0}")]
    NotYetSupported(String),
}

pub fn parse(_src: &str, tokens: &[Token]) -> Result<File, ParseError> {
    let mut p = Parser::new(tokens);
    p.parse_file()
}

struct Parser<'a> {
    toks: &'a [Token],
    pos: usize,
}

impl<'a> Parser<'a> {
    fn new(toks: &'a [Token]) -> Self {
        Self { toks, pos: 0 }
    }

    fn peek(&self) -> &TokenKind {
        &self.toks[self.pos].kind
    }
    fn peek_at(&self, off: usize) -> &TokenKind {
        let i = (self.pos + off).min(self.toks.len() - 1);
        &self.toks[i].kind
    }
    fn span(&self) -> Span {
        self.toks[self.pos].span
    }
    fn bump(&mut self) -> Token {
        let t = self.toks[self.pos].clone();
        if self.pos + 1 < self.toks.len() {
            self.pos += 1;
        }
        t
    }
    fn expect(&mut self, kind: TokenKind) -> Result<Token, ParseError> {
        if std::mem::discriminant(self.peek()) == std::mem::discriminant(&kind) {
            Ok(self.bump())
        } else {
            Err(self.err(&format!("expected {:?}, got {:?}", kind, self.peek())))
        }
    }
    fn eat(&mut self, kind: &TokenKind) -> bool {
        if std::mem::discriminant(self.peek()) == std::mem::discriminant(kind) {
            self.bump();
            true
        } else {
            false
        }
    }
    fn err(&self, msg: &str) -> ParseError {
        ParseError::Generic {
            offset: self.span().start,
            msg: msg.to_string(),
        }
    }
    fn nys(&self, feature: &str) -> ParseError {
        ParseError::NotYetSupported(feature.to_string())
    }

    fn skip_newlines(&mut self) {
        while matches!(self.peek(), TokenKind::Newline) {
            self.bump();
        }
    }

    fn parse_file(&mut self) -> Result<File, ParseError> {
        self.skip_newlines();
        let start = self.span().start;
        let kind = match self.peek() {
            TokenKind::Prog => {
                self.bump();
                ProgKind::Prog
            }
            TokenKind::Mod => {
                self.bump();
                ProgKind::Mod
            }
            TokenKind::Svc => {
                return Err(self.nys("svc"));
            }
            other => {
                return Err(self.err(&format!(
                    "expected `prog` or `mod` at top level, got {:?}",
                    other
                )));
            }
        };
        let name = self.expect_ident("file name")?;
        self.expect(TokenKind::Newline)?;
        self.skip_newlines();
        let mut items = Vec::new();
        if matches!(self.peek(), TokenKind::Indent) {
            self.bump();
            self.skip_newlines();
            while !matches!(self.peek(), TokenKind::Dedent | TokenKind::Eof) {
                items.push(self.parse_item()?);
                self.skip_newlines();
            }
            if matches!(self.peek(), TokenKind::Dedent) {
                self.bump();
            }
        }
        self.skip_newlines();
        let end = self.span().end;
        Ok(File {
            kind,
            name,
            items,
            span: Span::new(start, end),
        })
    }

    fn expect_ident(&mut self, ctx: &str) -> Result<String, ParseError> {
        match self.peek().clone() {
            TokenKind::Ident(s) => {
                self.bump();
                Ok(s)
            }
            other => Err(self.err(&format!("expected identifier ({}), got {:?}", ctx, other))),
        }
    }

    fn parse_item(&mut self) -> Result<Item, ParseError> {
        match self.peek() {
            TokenKind::Use => self.parse_use().map(Item::Use),
            TokenKind::Err => self.parse_err().map(Item::Err),
            TokenKind::Type => self.parse_type_decl().map(Item::Type),
            TokenKind::State => self.parse_state().map(Item::State),
            TokenKind::Auth => Err(self.nys("auth")),
            TokenKind::Op => self.parse_op().map(Item::Op),
            other => Err(self.err(&format!("expected item, got {:?}", other))),
        }
    }

    fn parse_use(&mut self) -> Result<UseDecl, ParseError> {
        let start = self.span().start;
        self.expect(TokenKind::Use)?;
        let name = self.expect_ident("module name")?;
        self.expect(TokenKind::Newline)?;
        let end = self.span().end;
        Ok(UseDecl {
            name,
            span: Span::new(start, end),
        })
    }

    fn parse_err(&mut self) -> Result<ErrDecl, ParseError> {
        let start = self.span().start;
        self.expect(TokenKind::Err)?;
        let name = self.expect_ident("err name")?;
        let mut fields = Vec::new();
        if matches!(self.peek(), TokenKind::LParen) {
            self.bump();
            while !matches!(self.peek(), TokenKind::RParen) {
                let fname = self.expect_ident("err field name")?;
                self.expect(TokenKind::Colon)?;
                let ty = self.parse_type()?;
                fields.push(Field {
                    name: fname,
                    ty,
                    span: Span::default(),
                });
                if !self.eat(&TokenKind::Comma) {
                    break;
                }
            }
            self.expect(TokenKind::RParen)?;
        }
        self.expect(TokenKind::Newline)?;
        let end = self.span().end;
        Ok(ErrDecl {
            name,
            fields,
            span: Span::new(start, end),
        })
    }

    fn parse_type_decl(&mut self) -> Result<TypeDecl, ParseError> {
        let start = self.span().start;
        self.expect(TokenKind::Type)?;
        let name = self.expect_ident("type name")?;
        if matches!(self.peek(), TokenKind::LAngle) {
            return Err(self.nys("generic type parameters"));
        }
        if matches!(self.peek(), TokenKind::Equals) {
            self.bump();
            let target = self.parse_type()?;
            self.expect(TokenKind::Newline)?;
            return Ok(TypeDecl::Alias {
                name,
                target,
                span: Span::new(start, self.span().end),
            });
        }
        // Product type — newline + indent + fields
        self.expect(TokenKind::Newline)?;
        self.skip_newlines();
        self.expect(TokenKind::Indent)?;
        self.skip_newlines();
        let mut fields = Vec::new();
        while !matches!(self.peek(), TokenKind::Dedent | TokenKind::Eof) {
            let fname = self.expect_ident("field name")?;
            self.expect(TokenKind::Colon)?;
            let ty = self.parse_type()?;
            self.expect(TokenKind::Newline)?;
            self.skip_newlines();
            fields.push(Field {
                name: fname,
                ty,
                span: Span::default(),
            });
        }
        self.eat(&TokenKind::Dedent);
        Ok(TypeDecl::Product {
            name,
            fields,
            span: Span::new(start, self.span().end),
        })
    }

    fn parse_state(&mut self) -> Result<StateDecl, ParseError> {
        let start = self.span().start;
        self.expect(TokenKind::State)?;
        let mut fields = Vec::new();
        if matches!(self.peek(), TokenKind::Ident(_)) {
            // single-line state
            let name = self.expect_ident("state name")?;
            self.expect(TokenKind::Colon)?;
            let ty = self.parse_type()?;
            self.expect(TokenKind::Equals)?;
            let init = self.parse_expr()?;
            self.expect(TokenKind::Newline)?;
            fields.push(StateField {
                name,
                ty,
                init,
                span: Span::default(),
            });
        } else {
            self.expect(TokenKind::Newline)?;
            self.skip_newlines();
            self.expect(TokenKind::Indent)?;
            self.skip_newlines();
            while !matches!(self.peek(), TokenKind::Dedent | TokenKind::Eof) {
                let name = self.expect_ident("state field name")?;
                self.expect(TokenKind::Colon)?;
                let ty = self.parse_type()?;
                self.expect(TokenKind::Equals)?;
                let init = self.parse_expr()?;
                self.expect(TokenKind::Newline)?;
                self.skip_newlines();
                fields.push(StateField {
                    name,
                    ty,
                    init,
                    span: Span::default(),
                });
            }
            self.eat(&TokenKind::Dedent);
        }
        Ok(StateDecl {
            fields,
            span: Span::new(start, self.span().end),
        })
    }

    fn parse_op(&mut self) -> Result<OpDecl, ParseError> {
        let start = self.span().start;
        self.expect(TokenKind::Op)?;
        let is_async = if matches!(self.peek(), TokenKind::Tilde) {
            self.bump();
            true
        } else {
            false
        };
        let name = self.expect_ident("op name")?;
        if matches!(self.peek(), TokenKind::LAngle) {
            return Err(self.nys("generic operations"));
        }
        self.expect(TokenKind::LParen)?;
        let mut params = Vec::new();
        while !matches!(self.peek(), TokenKind::RParen) {
            let pname = self.expect_ident("param name")?;
            self.expect(TokenKind::Colon)?;
            let ty = self.parse_type()?;
            params.push(Param {
                name: pname,
                ty,
                span: Span::default(),
            });
            if !self.eat(&TokenKind::Comma) {
                break;
            }
        }
        self.expect(TokenKind::RParen)?;

        // Optional return type: at this point, if the token isn't Newline/Bang,
        // it must be a type. `!Err` after RParen with no return type means unit
        // return that can fail — supported.
        let mut return_type: Option<Type> = None;
        let mut error_type: Option<ErrorType> = None;

        if matches!(self.peek(), TokenKind::Bang) {
            // unit return + error
            self.bump();
            error_type = Some(self.parse_error_type()?);
        } else if !matches!(self.peek(), TokenKind::Newline) {
            return_type = Some(self.parse_type()?);
            if matches!(self.peek(), TokenKind::Bang) {
                self.bump();
                error_type = Some(self.parse_error_type()?);
            }
        }
        self.expect(TokenKind::Newline)?;
        self.skip_newlines();

        let body = if matches!(self.peek(), TokenKind::Indent) {
            self.bump();
            self.skip_newlines();
            let mut stmts = Vec::new();
            while !matches!(self.peek(), TokenKind::Dedent | TokenKind::Eof) {
                stmts.push(self.parse_stmt()?);
                self.skip_newlines();
            }
            self.eat(&TokenKind::Dedent);
            stmts
        } else {
            Vec::new()
        };

        Ok(OpDecl {
            name,
            is_async,
            params,
            return_type,
            error_type,
            body,
            span: Span::new(start, self.span().end),
        })
    }

    fn parse_error_type(&mut self) -> Result<ErrorType, ParseError> {
        if matches!(self.peek(), TokenKind::LParen) {
            self.bump();
            let mut variants = Vec::new();
            loop {
                let n = self.expect_ident("error variant")?;
                variants.push(n);
                if matches!(self.peek(), TokenKind::Pipe) {
                    self.bump();
                    continue;
                }
                break;
            }
            self.expect(TokenKind::RParen)?;
            Ok(ErrorType::Union(variants))
        } else {
            let n = self.expect_ident("error type")?;
            Ok(ErrorType::Single(n))
        }
    }

    fn parse_type(&mut self) -> Result<Type, ParseError> {
        let mut ty = self.parse_type_atom()?;
        // postfix `?`
        while matches!(self.peek(), TokenKind::Question) {
            self.bump();
            ty = Type::Optional(Box::new(ty));
        }
        Ok(ty)
    }

    fn parse_type_atom(&mut self) -> Result<Type, ParseError> {
        match self.peek().clone() {
            TokenKind::LBracket => {
                self.bump();
                self.expect(TokenKind::RBracket)?;
                let inner = self.parse_type()?;
                Ok(Type::List(Box::new(inner)))
            }
            TokenKind::Amp => Err(self.nys("borrow type intent (&T)")),
            TokenKind::Plus => Err(self.nys("clone type intent (+T)")),
            TokenKind::Ident(s) => {
                self.bump();
                if let Some(p) = PrimType::from_keyword(&s) {
                    Ok(Type::Prim(p))
                } else if s == "map" {
                    Err(self.nys("map<K, V> type"))
                } else if s == "shared" {
                    Err(self.nys("shared<T> type"))
                } else {
                    Ok(Type::Named(s))
                }
            }
            other => Err(self.err(&format!("expected type, got {:?}", other))),
        }
    }

    fn parse_stmt(&mut self) -> Result<Stmt, ParseError> {
        match self.peek() {
            TokenKind::Let => self.parse_let(),
            TokenKind::Bang => self.parse_effect(),
            TokenKind::Pre => self.parse_pre(),
            _ => {
                let start = self.span().start;
                let expr = self.parse_expr()?;
                self.expect(TokenKind::Newline)?;
                let end = self.span().end;
                Ok(Stmt::Expr {
                    expr,
                    span: Span::new(start, end),
                })
            }
        }
    }

    fn parse_let(&mut self) -> Result<Stmt, ParseError> {
        let start = self.span().start;
        self.expect(TokenKind::Let)?;
        let name = self.expect_ident("let binding name")?;
        let ty = if matches!(self.peek(), TokenKind::Colon) {
            self.bump();
            Some(self.parse_type()?)
        } else {
            None
        };
        self.expect(TokenKind::Equals)?;
        let value = self.parse_expr()?;
        self.expect(TokenKind::Newline)?;
        Ok(Stmt::Let {
            name,
            ty,
            value,
            span: Span::new(start, self.span().end),
        })
    }

    fn parse_effect(&mut self) -> Result<Stmt, ParseError> {
        let start = self.span().start;
        self.expect(TokenKind::Bang)?;
        // !ident.field( ... )       — call effect
        // !ident = expr             — assign
        // !ident += expr            — add-assign
        // !ident -= expr            — sub-assign
        // !ident.field = expr       — also assignment but rejected for v0 except in svc paths;
        //                              we'll accept the simpler ident form.
        let head_start = self.span().start;
        let head_name = match self.peek().clone() {
            TokenKind::Ident(s) => {
                self.bump();
                s
            }
            other => return Err(self.err(&format!("expected name after !, got {:?}", other))),
        };

        // is this a state-field assignment (`!field = ...` / `!field += ...`)?
        match self.peek() {
            TokenKind::Equals => {
                self.bump();
                let value = self.parse_expr()?;
                self.expect(TokenKind::Newline)?;
                return Ok(Stmt::Effect {
                    target: EffectTarget::Assign { name: head_name, value },
                    span: Span::new(start, self.span().end),
                });
            }
            TokenKind::PlusEq => {
                self.bump();
                let value = self.parse_expr()?;
                self.expect(TokenKind::Newline)?;
                return Ok(Stmt::Effect {
                    target: EffectTarget::AddAssign { name: head_name, value },
                    span: Span::new(start, self.span().end),
                });
            }
            TokenKind::MinusEq => {
                self.bump();
                let value = self.parse_expr()?;
                self.expect(TokenKind::Newline)?;
                return Ok(Stmt::Effect {
                    target: EffectTarget::SubAssign { name: head_name, value },
                    span: Span::new(start, self.span().end),
                });
            }
            _ => {}
        }

        // Otherwise build a call expression starting from the name and following .field, then ().
        let mut head: Expr = Expr::Name(head_name, Span::new(head_start, self.span().start));
        while matches!(self.peek(), TokenKind::Dot) {
            self.bump();
            let field = self.expect_ident("member name")?;
            head = Expr::Member {
                target: Box::new(head),
                field,
                span: Span::default(),
            };
        }
        // Must be a call.
        if !matches!(self.peek(), TokenKind::LParen) {
            return Err(self.err("effect must be a call, an assignment, or a +=/-= state mutation"));
        }
        self.bump();
        let mut args = Vec::new();
        while !matches!(self.peek(), TokenKind::RParen) {
            args.push(self.parse_expr()?);
            if !self.eat(&TokenKind::Comma) {
                break;
            }
        }
        self.expect(TokenKind::RParen)?;
        self.expect(TokenKind::Newline)?;
        let call = Expr::Call {
            callee: Box::new(head),
            args,
            span: Span::default(),
        };
        Ok(Stmt::Effect {
            target: EffectTarget::Call(call),
            span: Span::new(start, self.span().end),
        })
    }

    fn parse_pre(&mut self) -> Result<Stmt, ParseError> {
        let start = self.span().start;
        self.expect(TokenKind::Pre)?;
        let cond = self.parse_expr_no_pipeline()?;
        let err = if matches!(self.peek(), TokenKind::Colon) {
            self.bump();
            let name = self.expect_ident("error name")?;
            let mut args = Vec::new();
            if matches!(self.peek(), TokenKind::LParen) {
                self.bump();
                while !matches!(self.peek(), TokenKind::RParen) {
                    args.push(self.parse_expr()?);
                    if !self.eat(&TokenKind::Comma) {
                        break;
                    }
                }
                self.expect(TokenKind::RParen)?;
            }
            Some(ErrCtor {
                name,
                args,
                span: Span::default(),
            })
        } else {
            None
        };
        self.expect(TokenKind::Newline)?;
        Ok(Stmt::Pre {
            cond,
            err,
            span: Span::new(start, self.span().end),
        })
    }

    // ----- Expressions -----

    fn parse_expr(&mut self) -> Result<Expr, ParseError> {
        let lhs = self.parse_or()?;
        // pipeline?
        if matches!(self.peek(), TokenKind::Arrow) {
            self.parse_pipeline_tail(lhs)
        } else {
            Ok(lhs)
        }
    }

    /// Same as parse_expr but disallows pipeline (used for `pre` cond, where
    /// trailing `:` would conflict).
    fn parse_expr_no_pipeline(&mut self) -> Result<Expr, ParseError> {
        self.parse_or()
    }

    fn parse_pipeline_tail(&mut self, source: Expr) -> Result<Expr, ParseError> {
        let mut stages = Vec::new();
        while matches!(self.peek(), TokenKind::Arrow) {
            self.bump();
            let name = match self.peek().clone() {
                TokenKind::Ident(s) => {
                    self.bump();
                    s
                }
                other => {
                    return Err(self.err(&format!(
                        "expected pipeline stage name, got {:?}",
                        other
                    )))
                }
            };
            let mut args = Vec::new();
            if matches!(self.peek(), TokenKind::LParen) {
                self.bump();
                while !matches!(self.peek(), TokenKind::RParen) {
                    args.push(self.parse_expr_no_pipeline()?);
                    if !self.eat(&TokenKind::Comma) {
                        break;
                    }
                }
                self.expect(TokenKind::RParen)?;
            }
            stages.push(PipelineStage {
                name,
                args,
                span: Span::default(),
            });
        }
        Ok(Expr::Pipeline {
            source: Box::new(source),
            stages,
            span: Span::default(),
        })
    }

    fn parse_or(&mut self) -> Result<Expr, ParseError> {
        let mut lhs = self.parse_and()?;
        while matches!(self.peek(), TokenKind::OrOr) {
            self.bump();
            let rhs = self.parse_and()?;
            lhs = Expr::Binary {
                op: BinOp::Or,
                lhs: Box::new(lhs),
                rhs: Box::new(rhs),
                span: Span::default(),
            };
        }
        Ok(lhs)
    }

    fn parse_and(&mut self) -> Result<Expr, ParseError> {
        let mut lhs = self.parse_eq()?;
        while matches!(self.peek(), TokenKind::AndAnd) {
            self.bump();
            let rhs = self.parse_eq()?;
            lhs = Expr::Binary {
                op: BinOp::And,
                lhs: Box::new(lhs),
                rhs: Box::new(rhs),
                span: Span::default(),
            };
        }
        Ok(lhs)
    }

    fn parse_eq(&mut self) -> Result<Expr, ParseError> {
        let mut lhs = self.parse_cmp()?;
        loop {
            let op = match self.peek() {
                TokenKind::EqEq => BinOp::Eq,
                TokenKind::NotEq => BinOp::Ne,
                _ => break,
            };
            self.bump();
            let rhs = self.parse_cmp()?;
            lhs = Expr::Binary {
                op,
                lhs: Box::new(lhs),
                rhs: Box::new(rhs),
                span: Span::default(),
            };
        }
        Ok(lhs)
    }

    fn parse_cmp(&mut self) -> Result<Expr, ParseError> {
        let mut lhs = self.parse_add()?;
        loop {
            let op = match self.peek() {
                TokenKind::LAngle => BinOp::Lt,
                TokenKind::RAngle => BinOp::Gt,
                TokenKind::LtEq => BinOp::Le,
                TokenKind::GtEq => BinOp::Ge,
                _ => break,
            };
            self.bump();
            let rhs = self.parse_add()?;
            lhs = Expr::Binary {
                op,
                lhs: Box::new(lhs),
                rhs: Box::new(rhs),
                span: Span::default(),
            };
        }
        Ok(lhs)
    }

    fn parse_add(&mut self) -> Result<Expr, ParseError> {
        let mut lhs = self.parse_mul()?;
        loop {
            let op = match self.peek() {
                TokenKind::Plus => BinOp::Add,
                TokenKind::Minus => BinOp::Sub,
                _ => break,
            };
            self.bump();
            let rhs = self.parse_mul()?;
            lhs = Expr::Binary {
                op,
                lhs: Box::new(lhs),
                rhs: Box::new(rhs),
                span: Span::default(),
            };
        }
        Ok(lhs)
    }

    fn parse_mul(&mut self) -> Result<Expr, ParseError> {
        let mut lhs = self.parse_unary()?;
        loop {
            let op = match self.peek() {
                TokenKind::Star => BinOp::Mul,
                TokenKind::Slash => BinOp::Div,
                TokenKind::Percent => BinOp::Mod,
                _ => break,
            };
            self.bump();
            let rhs = self.parse_unary()?;
            lhs = Expr::Binary {
                op,
                lhs: Box::new(lhs),
                rhs: Box::new(rhs),
                span: Span::default(),
            };
        }
        Ok(lhs)
    }

    fn parse_unary(&mut self) -> Result<Expr, ParseError> {
        match self.peek() {
            TokenKind::Minus => {
                self.bump();
                let e = self.parse_unary()?;
                Ok(Expr::Unary {
                    op: UnOp::Neg,
                    expr: Box::new(e),
                    span: Span::default(),
                })
            }
            TokenKind::Bang => {
                // `!` as prefix operator on an expression — `!foo` (logical not)
                // Caution: at *statement* start, `!` is an effect. At expression
                // position (e.g. inside a condition), it's logical not.
                self.bump();
                let e = self.parse_unary()?;
                Ok(Expr::Unary {
                    op: UnOp::Not,
                    expr: Box::new(e),
                    span: Span::default(),
                })
            }
            TokenKind::Tilde => {
                self.bump();
                let inner = self.parse_postfix()?;
                Ok(Expr::Await {
                    inner: Box::new(inner),
                    span: Span::default(),
                })
            }
            _ => self.parse_postfix(),
        }
    }

    fn parse_postfix(&mut self) -> Result<Expr, ParseError> {
        let mut e = self.parse_atom()?;
        loop {
            match self.peek() {
                TokenKind::Dot => {
                    self.bump();
                    let field = self.expect_ident("member name")?;
                    e = Expr::Member {
                        target: Box::new(e),
                        field,
                        span: Span::default(),
                    };
                }
                TokenKind::LParen => {
                    self.bump();
                    let mut args = Vec::new();
                    while !matches!(self.peek(), TokenKind::RParen) {
                        args.push(self.parse_expr()?);
                        if !self.eat(&TokenKind::Comma) {
                            break;
                        }
                    }
                    self.expect(TokenKind::RParen)?;
                    e = Expr::Call {
                        callee: Box::new(e),
                        args,
                        span: Span::default(),
                    };
                }
                TokenKind::Bang => {
                    // Propagation `expr!`. Only valid after a call expression
                    // (we don't enforce that strictly here; typing later).
                    // Disambiguate from binary `!=` (already handled by lexer).
                    self.bump();
                    e = Expr::Propagate {
                        inner: Box::new(e),
                        span: Span::default(),
                    };
                }
                TokenKind::Question => {
                    // `expr?` followed by Indent → match expression (not in v0).
                    if matches!(self.peek_at(1), TokenKind::Newline)
                        && matches!(self.peek_at(2), TokenKind::Indent)
                    {
                        return Err(self.nys("match expressions"));
                    }
                    return Err(self.nys("optional expressions (?)"));
                }
                TokenKind::LBracket => {
                    return Err(self.nys("subscript expressions"));
                }
                _ => break,
            }
        }
        Ok(e)
    }

    fn parse_atom(&mut self) -> Result<Expr, ParseError> {
        let tok = self.peek().clone();
        let span = self.span();
        match tok {
            TokenKind::Int(n) => {
                self.bump();
                Ok(Expr::Lit(Lit::Int(n), span))
            }
            TokenKind::Str(s) => {
                self.bump();
                Ok(Expr::Lit(Lit::Str(s), span))
            }
            TokenKind::True => {
                self.bump();
                Ok(Expr::Lit(Lit::Bool(true), span))
            }
            TokenKind::False => {
                self.bump();
                Ok(Expr::Lit(Lit::Bool(false), span))
            }
            TokenKind::Ident(s) => {
                self.bump();
                // closure?  name => expr
                if matches!(self.peek(), TokenKind::FatArrow) {
                    self.bump();
                    if matches!(self.peek(), TokenKind::Newline) {
                        return Err(self.nys("multi-line closures"));
                    }
                    let body = self.parse_expr()?;
                    return Ok(Expr::Closure {
                        param: s,
                        body: Box::new(body),
                        span: Span::default(),
                    });
                }
                Ok(Expr::Name(s, span))
            }
            TokenKind::LParen => {
                self.bump();
                // closure with multiple params? not supported v0.
                let e = self.parse_expr()?;
                self.expect(TokenKind::RParen)?;
                Ok(e)
            }
            TokenKind::LBracket => Err(self.nys("list literals")),
            TokenKind::LBrace => Err(self.nys("map literals")),
            TokenKind::Underscore => Err(self.nys("wildcard pattern (_)")),
            other => Err(self.err(&format!("expected expression, got {:?}", other))),
        }
    }
}
