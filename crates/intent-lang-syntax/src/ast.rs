/// Source span for error reporting.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Span {
    pub start: usize,
    pub end: usize,
}

impl Span {
    pub fn new(start: usize, end: usize) -> Self {
        Self { start, end }
    }

    pub fn merge(&self, other: &Span) -> Span {
        Span {
            start: self.start.min(other.start),
            end: self.end.max(other.end),
        }
    }
}

/// A node wrapping a value with its source span.
#[derive(Debug, Clone)]
pub struct Spanned<T> {
    pub node: T,
    pub span: Span,
}

impl<T> Spanned<T> {
    pub fn new(node: T, span: Span) -> Self {
        Self { node, span }
    }
}

// ── Program ──────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct Program {
    pub declarations: Vec<Spanned<Declaration>>,
}

#[derive(Debug, Clone)]
pub enum Declaration {
    Import(ImportDecl),
    Type(TypeDecl),
    Enum(EnumDecl),
    Function(FunctionDecl),
    Intent(IntentDecl),
    Safety(SafetyDecl),
    Theorem(TheoremDecl),
    Axiom(AxiomDecl),
    /// `goal "name" { rationale: ...; stakeholder: ...; measure: ...; realized_by: [...] }`
    Goal(GoalDecl),
    /// `coverage "name" { dimensions: { d1: [...]; d2: [...] } }`
    Coverage(CoverageDecl),
    /// D5: `example Intent "title" { given: {...} expect: {...} }`
    Example(ExampleDecl),
}

// ── Declarations ─────────────────────────────────────────────

#[derive(Debug, Clone)]
pub enum ImportPath {
    /// Plugin import: `import smarthome` or `import finance.currency`
    Plugin(Vec<String>),
    /// File import: `import "./path/to/file.intent"`
    File(String),
}

#[derive(Debug, Clone)]
pub struct ImportDecl {
    pub path: ImportPath,
    pub alias: Option<String>,
}

#[derive(Debug, Clone)]
pub struct TypeDecl {
    pub name: String,
    pub type_params: Vec<String>,
    pub fields: Vec<Field>,
}

#[derive(Debug, Clone)]
pub struct Field {
    pub name: String,
    pub ty: TypeExpr,
}

#[derive(Debug, Clone)]
pub struct EnumDecl {
    pub name: String,
    pub variants: Vec<String>,
}

#[derive(Debug, Clone)]
pub struct FunctionDecl {
    pub name: String,
    pub params: Vec<Param>,
    pub return_type: TypeExpr,
    pub body: Spanned<Expr>,
}

#[derive(Debug, Clone)]
pub struct Param {
    pub name: String,
    pub ty: TypeExpr,
}

#[derive(Debug, Clone)]
pub struct IntentDecl {
    pub name: String,
    pub annotations: Vec<Annotation>,
    pub params: Vec<Param>,
    pub clauses: Vec<Spanned<Clause>>,
    /// D2 (rfc-modeling-integrity): `modifies` frame declaration.
    /// `None` = infer frame from primed fields in ensure/invariant.
    pub modifies: Option<ModifiesSpec>,
}

/// D2: which parts of the observable state an intent may change.
/// Everything outside the frame must stay equal across the transition.
#[derive(Debug, Clone)]
pub enum ModifiesSpec {
    /// `modifies *` — opt out of frame semantics (weak, underspecified intent)
    Wildcard,
    /// `modifies sender.balance, receiver.balance` — explicit frame
    Paths(Vec<Spanned<Expr>>),
}

/// A require/ensure/invariant clause.
///
/// D4: `label` gives the clause a stable, human-readable ID
/// (`ensure debit: ...` → `Intent/debit`); unlabeled clauses fall back
/// to positional IDs (`Intent/ensure[0]`).
///
/// D3: `else_reject` marks a require as a *business rule*: violating it
/// must observably reject the operation and leave all state unchanged.
/// Without the marker a require is a *caller contract* (violation =
/// unspecified behavior).
#[derive(Debug, Clone)]
pub struct Clause {
    pub label: Option<String>,
    pub kind: ClauseKind,
    pub expr: Spanned<Expr>,
    pub else_reject: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ClauseKind {
    Require,
    Ensure,
    Invariant,
}

impl ClauseKind {
    pub fn keyword(&self) -> &'static str {
        match self {
            ClauseKind::Require => "require",
            ClauseKind::Ensure => "ensure",
            ClauseKind::Invariant => "invariant",
        }
    }
}

impl Clause {
    /// Stable clause ID (D4 / acceptance RFC 4.1): label-first, index fallback.
    /// `idx_within_kind` is the 0-based position among clauses of the same kind.
    pub fn stable_id(&self, owner: &str, idx_within_kind: usize) -> String {
        match &self.label {
            Some(l) => format!("{owner}/{l}"),
            None => format!("{owner}/{}[{idx_within_kind}]", self.kind.keyword()),
        }
    }
}

#[derive(Debug, Clone)]
pub struct SafetyDecl {
    pub name: String,
    pub params: Vec<Param>,
    pub invariants: Vec<Spanned<Expr>>,
}

#[derive(Debug, Clone)]
pub struct TheoremDecl {
    pub name: String,
    pub body: Spanned<Expr>,
}

#[derive(Debug, Clone)]
pub struct AxiomDecl {
    pub name: String,
    pub body: Spanned<Expr>,
}

#[derive(Debug, Clone)]
pub struct GoalDecl {
    /// Human-readable goal name (a string literal)
    pub name: String,
    /// Annotations preceding the goal, e.g. `@capability("自助售后闭环")`
    /// or `@guardrail("自助售后闭环")` — the annotation name marks the goal's
    /// kind (capability vs guardrail) and its positional string arg names the
    /// theme group used to cluster the goal graph.
    pub annotations: Vec<Annotation>,
    pub rationale: Option<String>,
    /// Free-form list of stakeholder labels (e.g., "compliance", "finance")
    pub stakeholder: Vec<String>,
    pub measure: Option<String>,
    /// Identifier names of safety/intent declarations that realize this goal
    pub realized_by: Vec<String>,
}

#[derive(Debug, Clone)]
pub struct CoverageDecl {
    /// Human-readable coverage scenario name (a string literal)
    pub name: String,
    /// Each dimension: (name, list of values as expressions — usually idents/lits)
    pub dimensions: Vec<CoverageDim>,
}

#[derive(Debug, Clone)]
pub struct CoverageDim {
    pub name: String,
    pub values: Vec<Spanned<Expr>>,
}

/// D5 (rfc-modeling-integrity): specification by example.
///
/// ```intent
/// example TransferSafe "工资转账" {
///   given:  { sender.balance: 100, receiver.balance: 50, amount: 30 }
///   expect: { sender.balance': 70, receiver.balance': 80 }
/// }
/// ```
///
/// Three roles: (1) anti-formalization-drift — `intent check` substitutes
/// the concrete values into every clause via Z3; (2) acceptance seed data;
/// (3) readable documentation for non-programmers. `expect` may cover only
/// part of the post-state; unwritten fields follow frame semantics (D2).
#[derive(Debug, Clone)]
pub struct ExampleDecl {
    /// Name of the intent this example instantiates.
    pub intent: String,
    /// Optional human-readable title.
    pub title: Option<String>,
    /// Pre-state / parameter bindings: path (e.g. `sender.balance`) → literal.
    pub given: Vec<ExampleBinding>,
    /// Post-state expectations: primed path → literal. May be partial.
    pub expect: Vec<ExampleBinding>,
}

#[derive(Debug, Clone)]
pub struct ExampleBinding {
    /// Binding path as an expression (Ident / FieldAccess / Prime thereof).
    pub path: Spanned<Expr>,
    /// The concrete value (literal expression).
    pub value: Spanned<Expr>,
}

#[derive(Debug, Clone)]
pub struct Annotation {
    pub name: String,
    pub args: Vec<AnnotationArg>,
}

#[derive(Debug, Clone)]
pub enum AnnotationArg {
    Positional(Spanned<Expr>),
    Named(String, Spanned<Expr>),
}

// ── Types ────────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TypeExpr {
    Named(String),
    /// `module.TypeName` — qualified type reference
    Qualified(String, String),
    Generic(String, Vec<TypeExpr>),
}

impl std::fmt::Display for TypeExpr {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            TypeExpr::Named(n) => write!(f, "{n}"),
            TypeExpr::Qualified(module, name) => write!(f, "{module}.{name}"),
            TypeExpr::Generic(n, args) => {
                write!(f, "{n}<")?;
                for (i, a) in args.iter().enumerate() {
                    if i > 0 {
                        write!(f, ", ")?;
                    }
                    write!(f, "{a}")?;
                }
                write!(f, ">")
            }
        }
    }
}

// ── Expressions ──────────────────────────────────────────────

#[derive(Debug, Clone)]
pub enum Expr {
    IntLit(i64),
    BoolLit(bool),
    StringLit(String),

    Ident(String),

    /// `x'` or `after(x)` — post-execution value
    Prime(Box<Spanned<Expr>>),

    /// `expr.field`
    FieldAccess(Box<Spanned<Expr>>, String),

    /// `expr[index]`
    Index(Box<Spanned<Expr>>, Box<Spanned<Expr>>),

    BinOp(Box<Spanned<Expr>>, BinOp, Box<Spanned<Expr>>),
    UnaryOp(UnaryOp, Box<Spanned<Expr>>),

    /// `if cond then a else b`
    IfThenElse(Box<Spanned<Expr>>, Box<Spanned<Expr>>, Box<Spanned<Expr>>),

    /// `forall vars, body`
    Forall(Vec<TypedVar>, Box<Spanned<Expr>>),
    /// `exists vars, body`
    Exists(Vec<TypedVar>, Box<Spanned<Expr>>),

    /// `name(args)`
    Call(String, Vec<Spanned<Expr>>),

    Paren(Box<Spanned<Expr>>),
}

#[derive(Debug, Clone)]
pub struct TypedVar {
    pub name: String,
    pub ty: TypeExpr,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BinOp {
    Add,
    Sub,
    Mul,
    Div,
    Mod,
    Eq,
    Neq,
    Lt,
    Le,
    Gt,
    Ge,
    And,
    Or,
    Implies,
}

impl std::fmt::Display for BinOp {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            BinOp::Add => write!(f, "+"),
            BinOp::Sub => write!(f, "-"),
            BinOp::Mul => write!(f, "*"),
            BinOp::Div => write!(f, "/"),
            BinOp::Mod => write!(f, "%"),
            BinOp::Eq => write!(f, "=="),
            BinOp::Neq => write!(f, "!="),
            BinOp::Lt => write!(f, "<"),
            BinOp::Le => write!(f, "<="),
            BinOp::Gt => write!(f, ">"),
            BinOp::Ge => write!(f, ">="),
            BinOp::And => write!(f, "&&"),
            BinOp::Or => write!(f, "||"),
            BinOp::Implies => write!(f, "==>"),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UnaryOp {
    Not,
    Neg,
}
