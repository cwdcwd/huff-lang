//! Huff AST node types — shared between parser and emitters.
//!
//! Covers the v0 walking-skeleton subset only. Out-of-subset constructs are
//! intentionally absent and the parser is expected to raise a
//! "not yet supported: <feature>" error rather than producing one of these
//! nodes for an unsupported form.

#![forbid(unsafe_code)]

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Span {
    pub start: usize,
    pub end: usize,
}

impl Span {
    pub fn new(start: usize, end: usize) -> Self {
        Self { start, end }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ProgKind {
    Prog,
    Mod,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct File {
    pub kind: ProgKind,
    pub name: String,
    pub items: Vec<Item>,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Item {
    Use(UseDecl),
    Err(ErrDecl),
    Type(TypeDecl),
    State(StateDecl),
    Op(OpDecl),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UseDecl {
    pub name: String,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ErrDecl {
    pub name: String,
    pub fields: Vec<Field>,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TypeDecl {
    Product {
        name: String,
        fields: Vec<Field>,
        span: Span,
    },
    Alias {
        name: String,
        target: Type,
        span: Span,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Field {
    pub name: String,
    pub ty: Type,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StateDecl {
    pub fields: Vec<StateField>,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StateField {
    pub name: String,
    pub ty: Type,
    pub init: Expr,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OpDecl {
    pub name: String,
    pub is_async: bool,
    pub params: Vec<Param>,
    pub return_type: Option<Type>,
    pub error_type: Option<ErrorType>,
    pub body: Vec<Stmt>,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Param {
    pub name: String,
    pub ty: Type,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ErrorType {
    Single(String),
    Union(Vec<String>),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Stmt {
    Let {
        name: String,
        ty: Option<Type>,
        value: Expr,
        span: Span,
    },
    Effect {
        target: EffectTarget,
        span: Span,
    },
    Pre {
        cond: Expr,
        err: Option<ErrCtor>,
        span: Span,
    },
    Expr {
        expr: Expr,
        span: Span,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EffectTarget {
    /// `!io.writeln(x)` and friends — a call expression.
    Call(Expr),
    /// `!stateField = expr`
    Assign { name: String, value: Expr },
    /// `!stateField += expr`
    AddAssign { name: String, value: Expr },
    /// `!stateField -= expr`
    SubAssign { name: String, value: Expr },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ErrCtor {
    pub name: String,
    pub args: Vec<Expr>,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Expr {
    Lit(Lit, Span),
    Name(String, Span),
    Call {
        callee: Box<Expr>,
        args: Vec<Expr>,
        span: Span,
    },
    Member {
        target: Box<Expr>,
        field: String,
        span: Span,
    },
    Binary {
        op: BinOp,
        lhs: Box<Expr>,
        rhs: Box<Expr>,
        span: Span,
    },
    Unary {
        op: UnOp,
        expr: Box<Expr>,
        span: Span,
    },
    Pipeline {
        source: Box<Expr>,
        stages: Vec<PipelineStage>,
        span: Span,
    },
    Closure {
        param: String,
        body: Box<Expr>,
        span: Span,
    },
    /// `call()!` — propagate error.
    Propagate {
        inner: Box<Expr>,
        span: Span,
    },
    /// `~call()` — await an async call.
    Await {
        inner: Box<Expr>,
        span: Span,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PipelineStage {
    pub name: String,
    pub args: Vec<Expr>,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Lit {
    Int(i64),
    Str(String),
    Bool(bool),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BinOp {
    Add,
    Sub,
    Mul,
    Div,
    Mod,
    Eq,
    Ne,
    Lt,
    Gt,
    Le,
    Ge,
    And,
    Or,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UnOp {
    Neg,
    Not,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Type {
    /// `str`, `bool`, `i32`, `u32`, `i64`, `u64`, `f32`, `f64`, `bytes`
    Prim(PrimType),
    /// User-defined type — `User`, `FileId`, etc.
    Named(String),
    /// `[]T`
    List(Box<Type>),
    /// `T?`
    Optional(Box<Type>),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PrimType {
    Str,
    Bool,
    I32,
    U32,
    I64,
    U64,
    F32,
    F64,
    Bytes,
}

impl PrimType {
    pub fn from_keyword(s: &str) -> Option<Self> {
        Some(match s {
            "str" => PrimType::Str,
            "bool" => PrimType::Bool,
            "i32" => PrimType::I32,
            "u32" => PrimType::U32,
            "i64" => PrimType::I64,
            "u64" => PrimType::U64,
            "f32" => PrimType::F32,
            "f64" => PrimType::F64,
            "bytes" => PrimType::Bytes,
            _ => return None,
        })
    }
}
