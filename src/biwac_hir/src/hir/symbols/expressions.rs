use std::cell::OnceCell;

use biwac_ast::{BinOperator, BoolLiteral, IntegerLiteral, StringLiteral, UnOperator};
use biwac_span::{Span, TyDefId, ValDefId, VarId};

use crate::{Ident, Stmt};

// ExprId
// function local expression id
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct ExprId(usize);

#[derive(Debug, Clone)]
pub struct Expr {
    pub expr: ExprVal,
    pub id: ExprId,
}

impl ExprId {
    pub fn new(id: usize) -> Self {
        Self(id)
    }
}

#[derive(Debug, Clone)]
pub enum ExprVal {
    Primary(Primary),
    Unary(UnaryExpr),
    Binary(BinaryExpr),
}

impl Expr {
    pub fn span(&self) -> Span {
        match &self.expr {
            ExprVal::Primary(p) => p.span(),
            ExprVal::Unary(u) => u.span.clone(),
            ExprVal::Binary(b) => b.span(),
        }
    }
}

#[derive(Debug, Clone)]
pub enum Primary {
    Literal(Literal),
    Variable(Variable), // TODO: support using external module variables
    FnCall(FnCall),
    MemberAccess(MemberAccess),
    IfExpr(IfExpr),
    Block(BlockExpr),
    MethodCall(MethodCall),
}

#[derive(Debug, Clone)]
pub struct BlockExpr {
    pub stmts: Vec<Stmt>,
    pub expr: Box<Expr>,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub struct IfExpr {
    pub cond: Box<Expr>,
    pub then: BlockExpr,
    // pub else_ifs: Vec<(Expr, BlockExpr)>,
    pub els: BlockExpr,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Variable {
    pub id: VarIdKind,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum VarIdKind {
    Local(VarId),
    Global(ValDefId),
}

impl Primary {
    pub fn span(&self) -> Span {
        match self {
            Self::Literal(l) => l.span(),
            Self::Variable(v) => v.span.clone(),
            Self::FnCall(f) => f.span.clone(),
            Self::MemberAccess(m) => m.span.clone(),
            Self::IfExpr(i) => i.span.clone(),
            Self::Block(b) => b.span.clone(),
            Self::MethodCall(m) => m.span.clone(),
        }
    }
}

#[derive(Debug, Clone)]
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

#[derive(Debug, Clone)]
pub struct StructLiteral {
    pub tid: TyDefId,
    pub members: Vec<(Ident, Expr)>,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub struct FnCall {
    pub callee: Callee,
    pub args: Vec<Expr>,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub enum Callee {
    Var(VarId),
    Fn(ValDefId),
}

#[derive(Debug, Clone)]
pub struct MemberAccess {
    pub left: Box<Expr>,
    pub member: Ident,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub struct MethodCall {
    pub left: Box<Expr>,
    pub method: Ident,
    pub args: Vec<Expr>,
    pub span: Span,
    pub def_id: OnceCell<ValDefId>,
}

#[derive(Debug, Clone)]
pub struct BinaryExpr {
    pub op: BinOperator,
    pub left: Box<Expr>,
    pub right: Box<Expr>,
}

impl BinaryExpr {
    pub fn span(&self) -> Span {
        Span::merge(&self.left.span(), &self.right.span())
    }
}

#[derive(Debug, Clone)]
pub struct UnaryExpr {
    pub op: UnOperator,
    pub right: Box<Expr>,
    pub span: Span,
}
