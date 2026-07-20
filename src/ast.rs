/// AST node types for the `OpenSCAD` language.
///
/// Every node carries a [`Span`] for source-location mapping.
use crate::span::Span;
use hyperreal::Real;

/// A complete `OpenSCAD` source file.
#[derive(Debug, Clone, PartialEq)]
pub struct SourceFile {
    pub statements: Vec<Statement>,
    pub span: Span,
}

/// Top-level and block-level statements.
#[derive(Debug, Clone, PartialEq)]
pub enum Statement {
    /// `include <path>`
    Include { path: String, span: Span },
    /// `use <path>`
    Use { path: String, span: Span },
    /// Variable assignment: `name = expr;`
    Assignment {
        name: String,
        expr: Expr,
        span: Span,
    },
    /// Module definition: `module name(params) { body }`
    ModuleDefinition {
        name: String,
        params: Vec<Parameter>,
        body: Vec<Self>,
        span: Span,
    },
    /// Function definition: `function name(params) = expr;`
    FunctionDefinition {
        name: String,
        params: Vec<Parameter>,
        body: Expr,
        span: Span,
    },
    /// Module instantiation: `name(args) { children }` or `name(args);`
    ModuleInstantiation {
        name: String,
        args: Vec<Argument>,
        children: Vec<Self>,
        modifiers: Modifiers,
        span: Span,
    },
    /// `if (cond) { ... } else { ... }`
    IfElse {
        condition: Expr,
        then_body: Vec<Self>,
        else_body: Option<Vec<Self>>,
        span: Span,
    },
    /// A bare block `{ ... }` (rare but valid)
    Block { body: Vec<Self>, span: Span },
    /// Empty statement `;`
    Empty { span: Span },
}

/// Modifier prefixes for module instantiation: `!`, `#`, `%`, `*`
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
#[allow(clippy::struct_excessive_bools)]
pub struct Modifiers {
    /// `!` — root modifier
    pub root: bool,
    /// `#` — highlight/debug modifier
    pub highlight: bool,
    /// `%` — background/transparent modifier
    pub background: bool,
    /// `*` — disable modifier
    pub disable: bool,
}

/// An expression.
#[derive(Debug, Clone, PartialEq)]
pub struct Expr {
    pub kind: ExprKind,
    pub span: Span,
}

impl Expr {
    #[must_use]
    pub const fn new(kind: ExprKind, span: Span) -> Self {
        Self { kind, span }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum ExprKind {
    /// Exact real numeric literal.
    Number(Real),
    /// String literal (already unescaped)
    String(String),
    /// `true`
    BoolTrue,
    /// `false`
    BoolFalse,
    /// `undef`
    Undef,
    /// Variable reference
    Identifier(String),

    /// Unary operation: `-x`, `!x`, `+x`, `~x`
    UnaryOp { op: UnaryOp, operand: Box<Expr> },
    /// Binary operation
    BinaryOp {
        op: BinaryOp,
        left: Box<Expr>,
        right: Box<Expr>,
    },
    /// Ternary: `cond ? then : else`
    Ternary {
        condition: Box<Expr>,
        then_expr: Box<Expr>,
        else_expr: Box<Expr>,
    },

    /// Function call: `name(args)` or `expr(args)`
    FunctionCall {
        callee: Box<Expr>,
        args: Vec<Argument>,
    },
    /// Index access: `expr[index]`
    Index { object: Box<Expr>, index: Box<Expr> },
    /// Member access: `expr.member`
    MemberAccess { object: Box<Expr>, member: String },

    /// Vector/list literal: `[a, b, c]`
    Vector(Vec<Expr>),
    /// Range: `[start : end]` or `[start : step : end]`
    Range {
        start: Box<Expr>,
        step: Option<Box<Expr>>,
        end: Box<Expr>,
    },

    /// `let (assignments) expr`
    Let {
        assignments: Vec<Argument>,
        body: Box<Expr>,
    },
    /// `assert(args) expr`
    Assert {
        args: Vec<Argument>,
        body: Option<Box<Expr>>,
    },
    /// `echo(args) expr`
    Echo {
        args: Vec<Argument>,
        body: Option<Box<Expr>>,
    },

    /// Anonymous function: `function(params) expr`
    AnonymousFunction {
        params: Vec<Parameter>,
        body: Box<Expr>,
    },

    // ── List comprehension elements ──────────────────────────
    /// `for (assignments) expr`
    LcFor {
        assignments: Vec<Argument>,
        body: Box<Expr>,
    },
    /// C-style for: `for (init ; cond ; update) expr`
    LcForC {
        init: Vec<Argument>,
        condition: Box<Expr>,
        update: Vec<Argument>,
        body: Box<Expr>,
    },
    /// `if (cond) expr` / `if (cond) expr else expr` in list comprehension
    LcIf {
        condition: Box<Expr>,
        then_expr: Box<Expr>,
        else_expr: Option<Box<Expr>>,
    },
    /// `let (assignments) expr` in list comprehension
    LcLet {
        assignments: Vec<Argument>,
        body: Box<Expr>,
    },
    /// `each expr`
    LcEach { body: Box<Expr> },
}

/// Unary operators.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UnaryOp {
    Negate,
    Not,
    Plus,
    BinaryNot,
}

/// Binary operators (ordered by precedence, lowest first).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BinaryOp {
    // Logical
    LogicalOr,
    LogicalAnd,
    // Equality
    Equal,
    NotEqual,
    // Comparison
    Less,
    LessEqual,
    Greater,
    GreaterEqual,
    // Bitwise
    BitwiseOr,
    BitwiseAnd,
    // Shift
    ShiftLeft,
    ShiftRight,
    // Arithmetic
    Add,
    Subtract,
    Multiply,
    Divide,
    Modulo,
    // Exponent
    Exponent,
}

/// A function/module parameter: `name` or `name = default`
#[derive(Debug, Clone, PartialEq)]
pub struct Parameter {
    pub name: String,
    pub default: Option<Expr>,
    pub span: Span,
}

/// A function/module argument: positional `expr` or named `name = expr`
#[derive(Debug, Clone, PartialEq)]
pub struct Argument {
    pub name: Option<String>,
    pub value: Expr,
    pub span: Span,
}

impl Statement {
    #[must_use]
    pub const fn span(&self) -> Span {
        match self {
            Self::Include { span, .. }
            | Self::Use { span, .. }
            | Self::Assignment { span, .. }
            | Self::ModuleDefinition { span, .. }
            | Self::FunctionDefinition { span, .. }
            | Self::ModuleInstantiation { span, .. }
            | Self::IfElse { span, .. }
            | Self::Block { span, .. }
            | Self::Empty { span, .. } => *span,
        }
    }
}
