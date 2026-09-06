use std::cell::OnceCell;

use biwac_span::{Span, VarId};

use crate::{Ident, Path, Stmt, VariantShape};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Primary {
    Literal(Literal),
    Variable(Variable),
    FnCall(FnCall),
    MemberAccess(MemberAccess),
    IfExpr(IfExpr),
    Match(MatchExpr),
    Block(BlockExpr),
    MethodCall(MethodCall),
}

impl Primary {
    pub fn span(&self) -> Span {
        match self {
            Self::Literal(l) => l.span(),
            Self::Variable(v) => v.span().clone(),
            Self::FnCall(f) => f.span.clone(),
            Self::MemberAccess(m) => m.span(),
            Self::IfExpr(i) => i.span.clone(),
            Self::Match(m) => m.span.clone(),
            Self::Block(b) => b.span.clone(),
            Self::MethodCall(m) => m.span.clone(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Variable {
    Path(Path),
    SelfVar(Span),
}

impl Variable {
    pub fn span(&self) -> Span {
        match self {
            Self::Path(path) => path.span(),
            Self::SelfVar(span) => span.clone(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FnCall {
    pub path: Path,
    pub args: Vec<Exprs>,
    pub span: Span,
}

// id . (member | methodcall ) . (member | methodcall) . ...
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MemberAccess {
    pub left: Box<Exprs>,
    pub member: Ident,
}

impl MemberAccess {
    pub fn span(&self) -> Span {
        Span::merge(&self.left.span(), &self.member.span)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MethodCall {
    pub left: Box<Exprs>,
    pub method: Ident,
    pub args: Vec<Exprs>,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Literal {
    Integer(IntegerLiteral),
    // Float(f64),
    String(StringLiteral),
    Bool(BoolLiteral),
    Struct(StructLiteral),
}

impl Literal {
    pub fn span(&self) -> Span {
        match self {
            Self::Integer(i) => i.span.clone(),
            Self::String(s) => s.span.clone(),
            Self::Bool(b) => b.span.clone(),
            Self::Struct(s) => s.span.clone(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IntegerLiteral {
    pub val: u64,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BoolLiteral {
    pub val: bool,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StringLiteral {
    pub val: String,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StructLiteral {
    pub path: Path,
    pub members: Vec<(Ident, Box<Exprs>)>,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Exprs {
    Primary(Primary),
    Unary(UnaryExpr),
    Binary(BinaryExpr),
}

impl Exprs {
    pub fn span(&self) -> Span {
        match self {
            Self::Primary(p) => p.span(),
            Self::Unary(u) => u.span.clone(),
            Self::Binary(b) => b.span(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BinOperator {
    Add, // +
    Sub, // -
    Mul, // *
    Div, // /
    Mod, // %
    Gt,  // >
    Lt,  // <
    Ge,  // >=
    Le,  // <=
    Eq,  // ==
    Ne,  // !=
}

impl std::fmt::Display for BinOperator {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let s = match self {
            Self::Add => "+",
            Self::Sub => "-",
            Self::Mul => "*",
            Self::Div => "/",
            Self::Mod => "%",
            Self::Gt => ">",
            Self::Lt => "<",
            Self::Ge => ">=",
            Self::Le => "<=",
            Self::Eq => "==",
            Self::Ne => "!=",
        };
        f.write_str(s)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BinaryExpr {
    pub op: BinOperator,
    pub left: Box<Exprs>,
    pub right: Box<Exprs>,
}

impl BinaryExpr {
    pub fn span(&self) -> Span {
        Span::merge(&self.left.span(), &self.right.span())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UnOperator {
    Neg, // -
         // Not, // !
}

impl std::fmt::Display for UnOperator {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Neg => f.write_str("-"),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UnaryExpr {
    pub op: UnOperator,
    pub right: Box<Exprs>,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BlockExpr {
    pub stmts: Vec<Stmt>,
    pub expr: Box<Exprs>,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IfExpr {
    pub cond: Box<Exprs>,
    pub then: BlockExpr,
    // pub else_ifs: Vec<(Exprs, BlockExpr)>,
    pub els: BlockExpr,
    pub span: Span,
}

/// `match` の式形。すべてのアームが値を返す。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MatchExpr {
    pub scrutinee: Box<Exprs>,
    pub arms: Vec<MatchExprArm>,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MatchExprArm {
    pub pattern: Pattern,
    pub body: BlockExpr,
    pub span: Span,
}

/// パターン。
///
/// 今回はネストを入れない。バリアントのフィールドに書けるのは
/// 束縛かワイルドカードだけである。
/// これにより照合が「タグの一致 + 1 段の束縛」に閉じ、
/// 決定木を組まずに `SwitchInt` 1 つへ落とせる。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Pattern {
    /// `_`
    Wildcard(Span),
    /// 単独の識別子。
    ///
    /// 束縛なのか unit バリアントなのかは構文からは決まらない。
    /// 名前解決が「その名前がバリアントに解決されるか」を見て振り分ける。
    /// rustc と同じ扱いである。
    Ident(IdentPattern),
    /// `Color::Red` / `Color::Rgb(r, g, b)` / `Color::Named { name = n, alpha }`
    Variant(VariantPattern),
}

impl Pattern {
    pub fn span(&self) -> Span {
        match self {
            Self::Wildcard(span) => span.clone(),
            Self::Ident(b) => b.path.span(),
            Self::Variant(v) => v.span.clone(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IdentPattern {
    /// セグメントが 1 つだけのパス。
    /// バリアントに解決された場合は `resolved_id` に入る。
    pub path: Path,
    /// 束縛だった場合に名前解決が入れる。
    pub var_id: OnceCell<VarId>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VariantPattern {
    pub path: Path,
    pub fields: PatternFields,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PatternFields {
    /// `Color::Red`
    Unit,
    /// `Color::Rgb(r, g, _)`
    Tuple(Vec<Pattern>),
    /// `Color::Named { name = n, alpha }`
    ///
    /// 省略形 (`alpha`) は同名への束縛に展開済みで持つ。
    Struct(Vec<(Ident, Pattern)>),
}

impl PatternFields {
    pub fn shape(&self) -> VariantShape {
        match self {
            Self::Unit => VariantShape::Unit,
            Self::Tuple(_) => VariantShape::Tuple,
            Self::Struct(_) => VariantShape::Struct,
        }
    }
}
