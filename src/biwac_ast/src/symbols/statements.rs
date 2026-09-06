use std::cell::OnceCell;

use biwac_span::{Span, VarId};

use crate::{Exprs, Ident, Pattern, Primary, TypDecl};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Stmt {
    Block(BlockStmt),
    Expr(ExprStmt),
    Return(ReturnStmt),
    If(IfStmt),
    Match(MatchStmt),
    While(WhileStmt),
    VarDecl(VarDecl),
    Assign(AssignStmt),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExprStmt {
    pub expr: Exprs,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReturnStmt {
    pub expr: Exprs,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AssignStmt {
    pub dst: Primary,
    pub src: Exprs,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BlockStmt {
    pub stmts: Vec<Stmt>,
    pub span: Span,
}

/// `match` の文形。アームの本体はブロック文である。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MatchStmt {
    pub scrutinee: Exprs,
    pub arms: Vec<MatchStmtArm>,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MatchStmtArm {
    pub pattern: Pattern,
    pub body: BlockStmt,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WhileStmt {
    pub cond: Exprs,
    pub stmts: BlockStmt,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VarDecl {
    pub typ: TypDecl,
    pub id: Ident,
    pub init: Exprs,
    pub span: Span,
    pub var_id: OnceCell<VarId>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IfStmt {
    pub cond: Exprs,
    pub then: BlockStmt,
    pub els: Option<BlockStmt>,
    pub span: Span,
}
