use biwac_base::Span;

use crate::{Exprs, Ident, Primary, TypDecl};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Stmt {
    Block(BlockStmt),
    Expr(ExprStmt),
    Return(ReturnStmt),
    If(IfStmt),
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
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IfStmt {
    pub cond: Exprs,
    pub then: BlockStmt,
    pub els: Option<BlockStmt>,
    pub span: Span,
}
